//! The shape of the account screens (FormScreen.java, ADR 0058): one glass panel in the middle, a
//! title, then lines of text, labelled fields and buttons stacked down it, and a line for what
//! happened last. A screen lists its contents each frame in a `Stack`; `Form::show` lays them out,
//! sizes the panel to fit, centres it (scrolling it when it's taller than the window), draws them
//! and says which button was pressed. Enter presses the screen's main button; Escape goes back.
//!
//! While a request is out the form is busy: the status line says what it waits for, and the
//! outcome is taken (`poll`) on a later frame.
use crate::draw::Rect;
use crate::input::{self, Key, keys};
use crate::online::{self, Failure, Outcome, Ticket};
use crate::paint::Painter;
use crate::panels;
use crate::screen::Cx;
use crate::theme::{CONTROL_HEIGHT, GAP as THEME_GAP};
use crate::ui::{self, Ui};

pub const CONTENT_WIDTH: f32 = 236.0;
const PADDING: f32 = 14.0;
const LINE: f32 = 10.0;
const GAP: f32 = 6.0;
const ITEM_BUTTON: f32 = 58.0;
const TOGGLE_WIDTH: f32 = 52.0;

/// A colour of the theme's, picked when drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Text,
    Muted,
    Accent,
    Positive,
    Warning,
    Negative,
}

impl Tone {
    pub fn color(self, p: &Painter) -> u32 {
        let th = p.theme;
        match self {
            Tone::Text => th.text,
            Tone::Muted => th.muted,
            Tone::Accent => th.accent,
            Tone::Positive => th.positive,
            Tone::Warning => th.warning,
            Tone::Negative => th.negative,
        }
    }
}

/// What a field holds and shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Plain,
    /// An Incari username: only the characters one may hold.
    Username,
    /// A password: starred whole, never kept anywhere but the form.
    Password,
}

enum Item {
    Text(String, Tone),
    Centred(String, Tone),
    Gap(f32),
    Field { label: String, max: usize, kind: FieldKind },
    Button { label: String, id: usize, active: bool },
    Row { labels: Vec<String>, id: usize },
    Entry { label: String, buttons: Vec<String>, id: usize },
    Toggle { label: String, on: bool, id: usize },
    Choice { label: String, id: usize },
    Status,
    /// Room the screen draws in itself: its top comes back in `Shown::spaces`.
    Space(f32),
}

/// The contents of one frame, top to bottom. Each button gets the next id.
#[derive(Default)]
pub struct Stack {
    items: Vec<Item>,
    next: usize,
    submit: Option<usize>,
    focus: Option<String>,
}

impl Stack {
    fn id(&mut self, n: usize) -> usize {
        let id = self.next;
        self.next += n;
        id
    }

    /// Wrapped text across the panel.
    pub fn text(&mut self, text: impl Into<String>, tone: Tone) {
        self.items.push(Item::Text(text.into(), tone));
    }

    /// One line of text, centred.
    pub fn centred(&mut self, text: impl Into<String>, tone: Tone) {
        self.items.push(Item::Centred(text.into(), tone));
    }

    /// Space down the panel.
    pub fn gap(&mut self, height: f32) {
        self.items.push(Item::Gap(height));
    }

    /// A labelled text field across the panel; its text is the form's, by label.
    pub fn field(&mut self, label: &str, max: usize) {
        self.items.push(Item::Field { label: label.into(), max, kind: FieldKind::Plain });
    }

    pub fn username_field(&mut self, label: &str) {
        self.items.push(Item::Field { label: label.into(), max: online::rules::MAX_USERNAME, kind: FieldKind::Username });
    }

    pub fn password_field(&mut self, label: &str) {
        self.items.push(Item::Field { label: label.into(), max: online::rules::MAX_PASSWORD, kind: FieldKind::Password });
    }

    /// A button across the panel: its id.
    pub fn button(&mut self, label: &str) -> usize {
        self.button_active(label, true)
    }

    pub fn button_active(&mut self, label: &str, active: bool) -> usize {
        let id = self.id(1);
        self.items.push(Item::Button { label: label.into(), id, active });
        id
    }

    /// Buttons side by side across the panel: the first one's id, the others' following.
    pub fn row(&mut self, labels: &[&str]) -> usize {
        let id = self.id(labels.len());
        self.items.push(Item::Row { labels: labels.iter().map(|l| (*l).to_owned()).collect(), id });
        id
    }

    /// A name on the left, cut to fit, and buttons for it on the right: the first button's id.
    pub fn entry(&mut self, label: &str, buttons: &[&str]) -> usize {
        let id = self.id(buttons.len());
        self.items.push(Item::Entry {
            label: label.into(),
            buttons: buttons.iter().map(|b| (*b).to_owned()).collect(),
            id,
        });
        id
    }

    /// A setting: its name on the left, an on/off switch on the right. Pressed when it's flipped.
    pub fn toggle(&mut self, label: &str, on: bool) -> usize {
        let id = self.id(1);
        self.items.push(Item::Toggle { label: label.into(), on, id });
        id
    }

    /// A button that steps through choices, showing the current one (`Ui.choice`).
    pub fn choice(&mut self, current: &str) -> usize {
        let id = self.id(1);
        self.items.push(Item::Choice { label: current.into(), id });
        id
    }

    /// The status line: what the last request came to, or what one waits for.
    pub fn status_line(&mut self) {
        self.items.push(Item::Status);
    }

    /// Room for the screen's own drawing.
    pub fn space(&mut self, height: f32) {
        self.items.push(Item::Space(height));
    }

    /// What Enter presses.
    pub fn submit(&mut self, id: usize) {
        self.submit = Some(id);
    }

    /// The field focused when the screen first shows.
    pub fn focus(&mut self, label: &str) {
        self.focus = Some(label.into());
    }
}

/// What a frame's form came to.
#[derive(Default)]
pub struct Shown {
    /// The button pressed (or the toggle flipped, the choice stepped).
    pub pressed: Option<usize>,
    /// Shift was held for the press (a choice steps back).
    pub back: bool,
    /// Each `space`'s top left, in order.
    pub spaces: Vec<[f32; 2]>,
}

/// What a form keeps between frames.
#[derive(Default)]
pub struct Form {
    /// What's typed in each field, by label: kept while the layout changes.
    fields: Vec<(String, String)>,
    status: String,
    tone: Option<Tone>,
    busy: Option<Ticket>,
    scroll: f32,
    focused: bool,
}

impl Form {
    /// The text in the field labelled `label`.
    pub fn value(&self, label: &str) -> &str {
        self.fields.iter().find(|(l, _)| l == label).map_or("", |(_, v)| v.as_str())
    }

    pub fn set_value(&mut self, label: &str, value: &str) {
        match self.fields.iter_mut().find(|(l, _)| l == label) {
            Some((_, v)) => *v = value.to_owned(),
            None => self.fields.push((label.to_owned(), value.to_owned())),
        }
    }

    /// Whether a request is out.
    pub fn busy(&self) -> bool {
        self.busy.is_some()
    }

    /// The request out, if any.
    pub fn ticket(&self) -> Option<Ticket> {
        self.busy
    }

    /// Request `ticket` went out: the status line says `waiting` until it's answered.
    pub fn sending(&mut self, ticket: Ticket, waiting: &str) {
        self.busy = Some(ticket);
        self.status(waiting, Tone::Muted);
    }

    /// The outcome of the request out, once it's in.
    pub fn poll(&mut self) -> Option<Outcome> {
        let ticket = self.busy?;
        let outcome = online::with(|o| o.take_result(ticket))?;
        self.busy = None;
        Some(outcome)
    }

    /// The outcome's in: true if it worked; if not, the status line says why.
    pub fn answered(&mut self, outcome: &Outcome) -> bool {
        match outcome {
            Ok(_) => {
                self.status("", Tone::Muted);
                true
            }
            Err(f) => {
                self.failed(*f);
                false
            }
        }
    }

    pub fn failed(&mut self, failure: Failure) {
        self.status(failure.message(), Tone::Negative);
    }

    /// Says `text` on the status line.
    pub fn status(&mut self, text: &str, tone: Tone) {
        self.status = text.to_owned();
        self.tone = Some(tone);
    }

    /// A problem with what was typed, before anything is sent.
    pub fn problem(&mut self, text: &str) {
        self.status(text, Tone::Negative);
    }

    /// The screen closes: nobody waits for its request any more.
    pub fn close(&mut self) {
        if let Some(t) = self.busy.take() {
            online::with(|o| o.forget(t));
        }
    }

    /// Lays out, draws and answers the frame's `stack` under `title`.
    pub fn show(&mut self, cx: &mut Cx, title: &str, stack: Stack) -> Shown {
        let [width, height] = cx.size;
        let left = (width / 2.0 - CONTENT_WIDTH / 2.0).floor();
        let mut lines: Vec<Vec<String>> = Vec::new();
        let mut cursor = 0.0;
        for item in &stack.items {
            cursor += match item {
                Item::Text(text, _) => {
                    let wrapped = cx.p.wrap(text, CONTENT_WIDTH, 64);
                    let h = wrapped.len() as f32 * LINE + GAP - 2.0;
                    lines.push(wrapped);
                    h
                }
                Item::Centred(..) => LINE + GAP - 2.0,
                Item::Gap(h) | Item::Space(h) => *h,
                Item::Field { .. } => LINE + CONTROL_HEIGHT + GAP,
                Item::Entry { .. } => CONTROL_HEIGHT + GAP - 2.0,
                Item::Button { .. } | Item::Row { .. } | Item::Toggle { .. } | Item::Choice { .. } => CONTROL_HEIGHT + GAP,
                Item::Status => 2.0 * LINE + GAP,
            };
        }
        let panel_height = cursor + 2.0 * PADDING + LINE + GAP;
        let top = ((height - panel_height) / 2.0).floor().max(4.0);
        // A short screen (a Chromebook fullscreen at GUI scale 3) scrolls the panel instead of
        // cutting off its bottom buttons.
        let max_scroll = (top + panel_height + 4.0 - height).max(0.0);
        if max_scroll > 0.0 {
            let steps = cx.ui.scrolled([0.0, 0.0, width, height]);
            self.scroll -= (steps * 12.0).round();
        }
        self.scroll = self.scroll.clamp(0.0, max_scroll);
        let panel_top = top - self.scroll;
        panels::panel(cx.p, left - PADDING, panel_top, CONTENT_WIDTH + 2.0 * PADDING, panel_height, 1.0);
        let th = cx.p.theme;
        cx.p.centered(title, width / 2.0, panel_top + PADDING, th.text, false);

        // Tab moves between the fields; the first frame focuses the one asked for.
        let field_ids: Vec<(u64, &str)> = stack
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Field { label, .. } => Some((ui::id(label, 7), label.as_str())),
                _ => None,
            })
            .collect();
        if !self.focused {
            self.focused = true;
            if let Some(label) = &stack.focus {
                cx.ui.focus = Some(ui::id(label, 7));
                cx.ui.caret = self.value(label).chars().count();
            }
        }
        if !field_ids.is_empty() && cx.ui.key(keys::TAB) {
            let at = field_ids.iter().position(|(id, _)| Some(*id) == cx.ui.focus);
            let next = match at {
                Some(i) if cx.ui.shift => (i + field_ids.len() - 1) % field_ids.len(),
                Some(i) => (i + 1) % field_ids.len(),
                None => 0,
            };
            cx.ui.focus = Some(field_ids[next].0);
            cx.ui.caret = self.value(field_ids[next].1).chars().count();
        }
        let enter = cx.ui.key(keys::ENTER) || cx.ui.key(keys::KEYPAD_ENTER);

        let mut shown = Shown::default();
        let base = panel_top + PADDING + LINE + GAP;
        let mut y = base;
        let mut wrapped = lines.into_iter();
        for item in stack.items {
            match item {
                Item::Text(_, tone) => {
                    let color = tone.color(cx.p);
                    let lines = wrapped.next().unwrap_or_default();
                    for line in &lines {
                        cx.p.text(line, left, y, color, false);
                        y += LINE;
                    }
                    y += GAP - 2.0;
                }
                Item::Centred(text, tone) => {
                    let color = tone.color(cx.p);
                    cx.p.centered(&text, width / 2.0, y, color, false);
                    y += LINE + GAP - 2.0;
                }
                Item::Gap(h) => y += h,
                Item::Space(h) => {
                    shown.spaces.push([left, y]);
                    y += h;
                }
                Item::Field { label, max, kind } => {
                    cx.p.text(&label, left, y, th.muted, false);
                    y += LINE;
                    let mut value = self.value(&label).to_owned();
                    let r = [left, y, CONTENT_WIDTH, CONTROL_HEIGHT];
                    if field(cx.p, cx.ui, ui::id(&label, 7), r, &mut value, "", max, kind) {
                        self.set_value(&label, &value);
                    }
                    y += CONTROL_HEIGHT + GAP;
                }
                Item::Button { label, id, active } => {
                    if ui::button(cx.p, cx.ui, [left, y, CONTENT_WIDTH, CONTROL_HEIGHT], &label, active) {
                        shown.pressed = Some(id);
                    }
                    y += CONTROL_HEIGHT + GAP;
                }
                Item::Row { labels, id } => {
                    let n = labels.len() as f32;
                    let each = ((CONTENT_WIDTH - (n - 1.0) * THEME_GAP) / n).floor();
                    for (i, label) in labels.iter().enumerate() {
                        let x = left + i as f32 * (each + THEME_GAP);
                        let w = if i + 1 == labels.len() { CONTENT_WIDTH - i as f32 * (each + THEME_GAP) } else { each };
                        if ui::button(cx.p, cx.ui, [x, y, w, CONTROL_HEIGHT], label, true) {
                            shown.pressed = Some(id + i);
                        }
                    }
                    y += CONTROL_HEIGHT + GAP;
                }
                Item::Entry { label, buttons, id } => {
                    let n = buttons.len() as f32;
                    let buttons_width = n * ITEM_BUTTON + (n - 1.0) * THEME_GAP;
                    let shown_label = cx.p.fit(&label, CONTENT_WIDTH - buttons_width - 6.0).into_owned();
                    cx.p.text(&shown_label, left, y + ((CONTROL_HEIGHT - 8.0) / 2.0).floor(), th.text, false);
                    for (i, b) in buttons.iter().enumerate() {
                        let x = left + CONTENT_WIDTH - buttons_width + i as f32 * (ITEM_BUTTON + THEME_GAP);
                        if ui::button(cx.p, cx.ui, [x, y, ITEM_BUTTON, CONTROL_HEIGHT], b, true) {
                            shown.pressed = Some(id + i);
                        }
                    }
                    y += CONTROL_HEIGHT + GAP - 2.0;
                }
                Item::Toggle { label, on, id } => {
                    cx.p.text(&label, left, y + ((CONTROL_HEIGHT - 8.0) / 2.0).floor(), th.text, false);
                    let r = [left + CONTENT_WIDTH - TOGGLE_WIDTH, y, TOGGLE_WIDTH, CONTROL_HEIGHT];
                    if ui::switch(cx.p, cx.ui, r, on) {
                        shown.pressed = Some(id);
                    }
                    y += CONTROL_HEIGHT + GAP;
                }
                Item::Choice { label, id } => {
                    let r = [left, y, CONTENT_WIDTH, CONTROL_HEIGHT];
                    let back = cx.ui.shift;
                    if ui::button(cx.p, cx.ui, r, &label, true) {
                        shown.pressed = Some(id);
                        shown.back = back;
                    } else if cx.ui.right_clicked(r) {
                        shown.pressed = Some(id);
                        shown.back = true;
                    }
                    y += CONTROL_HEIGHT + GAP;
                }
                Item::Status => {
                    if !self.status.is_empty() {
                        let color = self.tone.unwrap_or(Tone::Muted).color(cx.p);
                        for (i, line) in cx.p.wrap(&self.status, CONTENT_WIDTH, 2).iter().enumerate() {
                            cx.p.centered(line, width / 2.0, y + i as f32 * LINE, color, false);
                        }
                    }
                    y += 2.0 * LINE + GAP;
                }
            }
        }
        if shown.pressed.is_none()
            && enter
            && !self.busy()
            && let Some(submit) = stack.submit
        {
            shown.pressed = Some(submit);
        }
        shown
    }
}

/// A one-line field editing `text`: click to focus; typing, Backspace, Delete, the arrows, Home,
/// End and Ctrl+V (Ctrl+C too, but not from a password). Enter and Tab are left for the form.
/// True when the text changed.
pub fn field(p: &mut Painter, ui: &mut Ui, id: u64, r: Rect, text: &mut String, hint: &str, max: usize, kind: FieldKind) -> bool {
    if ui.clicked(r) {
        ui.focus = Some(id);
        ui.caret = text.chars().count();
    }
    let focused = ui.focus == Some(id);
    crate::skin::well(p, r, p.theme.field, focused || ui.over(r), 1.0);
    let changed = focused && edit(ui, text, max, kind);
    let th = p.theme;
    let ty = r[1] + ((r[3] - 7.0) / 2.0).floor();
    let room = r[2] - 8.0;
    let shown: std::borrow::Cow<str> = if kind == FieldKind::Password {
        "*".repeat(text.chars().count()).into()
    } else {
        text.as_str().into()
    };
    p.clip_start(r);
    if text.is_empty() && !focused {
        p.text(hint, r[0] + 4.0, ty, crate::color::alpha(th.muted, 0.8), false);
    } else {
        let before: String = shown.chars().take(ui.caret.min(text.chars().count())).collect();
        let caret_x = p.width(&before);
        let shift = (caret_x - room + 2.0).max(0.0);
        p.text(&shown, r[0] + 4.0 - shift, ty, th.text, false);
        if focused && (p.motion.now / 500.0) as i64 % 2 == 0 {
            p.fill(r[0] + 4.0 + caret_x - shift, ty - 1.0, 1.0, 10.0, th.text);
        }
    }
    p.clip_end();
    changed
}

fn allowed(kind: FieldKind, c: char) -> bool {
    !c.is_control() && (kind != FieldKind::Username || online::rules::username_character(c))
}

/// Puts `c` at the caret if the field takes it.
fn insert(ui: &mut Ui, text: &mut String, c: char, max: usize, kind: FieldKind) -> bool {
    if !allowed(kind, c) || text.chars().count() >= max {
        return false;
    }
    let at = text.char_indices().nth(ui.caret).map_or(text.len(), |(i, _)| i);
    text.insert(at, c);
    ui.caret += 1;
    true
}

/// The frame's typing and editing keys on the focused field.
fn edit(ui: &mut Ui, text: &mut String, max: usize, kind: FieldKind) -> bool {
    let mut changed = false;
    let len = |t: &String| t.chars().count();
    let byte = |t: &String, c: usize| t.char_indices().nth(c).map_or(t.len(), |(i, _)| i);
    ui.caret = ui.caret.min(len(text));
    for c in std::mem::take(&mut ui.typed).chars() {
        changed |= insert(ui, text, c, max, kind);
    }
    for (key, mods) in std::mem::take(&mut ui.keys) {
        let Key::Keyboard(code) = key else {
            ui.keys.push((key, mods));
            continue;
        };
        let control = mods & (input::CONTROL | input::SUPER) != 0;
        match code {
            keys::BACKSPACE if ui.caret > 0 => {
                let at = byte(text, ui.caret - 1);
                text.remove(at);
                ui.caret -= 1;
                changed = true;
            }
            keys::DELETE if ui.caret < len(text) => {
                let at = byte(text, ui.caret);
                text.remove(at);
                changed = true;
            }
            keys::LEFT => ui.caret = ui.caret.saturating_sub(1),
            keys::RIGHT => ui.caret = (ui.caret + 1).min(len(text)),
            keys::HOME => ui.caret = 0,
            keys::END => ui.caret = len(text),
            keys::A if control => ui.caret = len(text),
            keys::C if control => {
                if kind != FieldKind::Password {
                    ui.copy = Some(text.clone());
                }
            }
            keys::V if control => {
                if let Some(paste) = ui.clipboard.clone() {
                    for c in paste.chars() {
                        changed |= insert(ui, text, c, max, kind);
                    }
                }
            }
            keys::BACKSPACE | keys::DELETE | keys::A | keys::C | keys::V | keys::X => {}
            _ => ui.keys.push((key, mods)),
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_username_field_takes_only_a_usernames_characters() {
        let mut ui = Ui::default();
        let mut text = String::new();
        ui.typed.push_str("Frost-1 _x");
        assert!(edit(&mut ui, &mut text, 16, FieldKind::Username));
        assert_eq!(text, "Frost1_x");
        ui.clipboard = Some("ab!cdefghijklmnopq".into());
        ui.keys.push((Key::Keyboard(keys::V), input::CONTROL));
        edit(&mut ui, &mut text, 16, FieldKind::Username);
        assert_eq!(text, "Frost1_xabcdefgh", "cut at the most a username holds");
    }

    #[test]
    fn a_password_is_never_copied_and_enter_is_left_for_the_form() {
        let mut ui = Ui::default();
        let mut text = String::from("secret pass");
        ui.keys.push((Key::Keyboard(keys::C), input::CONTROL));
        ui.keys.push((Key::Keyboard(keys::ENTER), 0));
        edit(&mut ui, &mut text, 128, FieldKind::Password);
        assert!(ui.copy.is_none());
        assert!(ui.key(keys::ENTER));
    }

    #[test]
    fn values_are_kept_by_label() {
        let mut form = Form::default();
        form.set_value("Password", "x");
        form.set_value("Password", "y");
        assert_eq!(form.value("Password"), "y");
        assert_eq!(form.value("Other"), "");
    }
}
