//! Incari's controls (Ui.java and the widgets its screens make), drawn each frame where the
//! screen puts them. Input arrives between frames and is kept here: a press is answered by the
//! control under it as it's next drawn, as the game's own buttons answer a press.
use crate::color::alpha;
use crate::draw::{Rect, contains};
use crate::icons::Icon;
use crate::input::{self, Input, Key, keys};
use crate::motion;
use crate::paint::Painter;
use crate::skin;
use crate::theme::{CONTROL_HEIGHT, GAP};

/// Input since the last frame, and what the controls hold between frames.
#[derive(Default)]
pub struct Ui {
    pub mouse: [f32; 2],
    /// The left button is held.
    pub down: bool,
    /// Shift is held, as the last key or button said.
    pub shift: bool,
    /// A left press waiting for the control under it.
    press: Option<[f32; 2]>,
    /// A right press waiting.
    right_press: Option<[f32; 2]>,
    /// The left button went up since the last frame.
    pub released: bool,
    /// Wheel steps waiting for the list under the pointer (positive is up).
    scroll: f32,
    scroll_at: [f32; 2],
    /// Keys pressed since the last frame, with their modifiers.
    pub keys: Vec<(Key, u32)>,
    /// Text typed since the last frame.
    pub typed: String,
    /// The focused text field.
    pub focus: Option<u64>,
    /// The control being dragged (a slider, a HUD element).
    pub drag: Option<u64>,
    /// The tooltip to draw over everything at the end of the frame.
    tooltip: Option<String>,
    /// The text field's caret position, in characters.
    pub caret: usize,
    /// What's on the clipboard to paste, when the game side has read it.
    pub clipboard: Option<String>,
    /// Text to copy to the clipboard, for the game side.
    pub copy: Option<String>,
}

impl Ui {
    /// Takes an input event. True when it's one a screen of Incari's answers (all of them while
    /// one is open).
    pub fn input(&mut self, event: &Input) {
        match event {
            Input::Move { at } => self.mouse = *at,
            Input::Button { button, pressed, at, modifiers } => {
                self.mouse = *at;
                self.shift = modifiers & input::SHIFT != 0;
                match (*button, *pressed) {
                    (0, true) => {
                        self.press = Some(*at);
                        self.down = true;
                    }
                    (0, false) => {
                        self.down = false;
                        self.released = true;
                    }
                    (1, true) => self.right_press = Some(*at),
                    _ => {}
                }
            }
            Input::Scroll { dy, at, .. } => {
                self.scroll += dy;
                self.scroll_at = *at;
            }
            Input::Key { key, pressed, modifiers, .. } => {
                self.shift = modifiers & input::SHIFT != 0;
                if *pressed {
                    self.keys.push((*key, *modifiers));
                }
            }
            Input::Text { text } => self.typed.push_str(text),
        }
    }

    /// Forgets what the frame didn't use: a press nothing was under, keys nothing read.
    pub fn end_frame(&mut self) {
        self.press = None;
        self.right_press = None;
        self.released = false;
        self.scroll = 0.0;
        self.keys.clear();
        self.typed.clear();
        if !self.down {
            self.drag = None;
        }
    }

    /// Forgets everything held: a screen opens or closes.
    pub fn reset(&mut self) {
        self.end_frame();
        self.down = false;
        self.focus = None;
        self.drag = None;
        self.tooltip = None;
    }

    /// Whether the pointer is over `r`.
    pub fn over(&self, r: Rect) -> bool {
        contains(r, self.mouse)
    }

    /// Takes a left press inside `r`, if there's one waiting.
    pub fn clicked(&mut self, r: Rect) -> bool {
        match self.press {
            Some(at) if contains(r, at) => {
                self.press = None;
                true
            }
            _ => false,
        }
    }

    /// Takes a right press inside `r`.
    pub fn right_clicked(&mut self, r: Rect) -> bool {
        match self.right_press {
            Some(at) if contains(r, at) => {
                self.right_press = None;
                true
            }
            _ => false,
        }
    }

    /// Whether a left press is waiting anywhere (a popup closes on a press outside it).
    pub fn pressed_anywhere(&self) -> Option<[f32; 2]> {
        self.press
    }

    /// Drops the waiting press: it landed on nothing that takes it.
    pub fn eat_press(&mut self) {
        self.press = None;
    }

    /// Takes the wheel's steps when the pointer is over `r`.
    pub fn scrolled(&mut self, r: Rect) -> f32 {
        if self.scroll != 0.0 && contains(r, self.scroll_at) {
            std::mem::take(&mut self.scroll)
        } else {
            0.0
        }
    }

    /// Takes a press of keyboard key `code`.
    pub fn key(&mut self, code: u16) -> bool {
        self.key_with(code).is_some()
    }

    /// Takes a press of keyboard key `code`, with its modifiers.
    pub fn key_with(&mut self, code: u16) -> Option<u32> {
        let i = self.keys.iter().position(|(k, _)| *k == Key::Keyboard(code))?;
        Some(self.keys.remove(i).1)
    }

    /// Shows `text` by the pointer at the end of the frame.
    pub fn tooltip(&mut self, text: impl Into<String>) {
        self.tooltip = Some(text.into());
    }

    /// Draws the frame's tooltip, if one was asked for: last, over everything.
    pub fn draw_tooltip(&mut self, p: &mut Painter, screen: [f32; 2]) {
        let Some(text) = self.tooltip.take() else {
            return;
        };
        let lines = p.wrap(&text, 200.0, 12);
        let w = lines.iter().map(|l| p.width(l)).fold(0.0, f32::max) + 8.0;
        let h = lines.len() as f32 * 10.0 + 6.0;
        let mut x = self.mouse[0] + 12.0;
        let mut y = self.mouse[1] - 12.0;
        if x + w > screen[0] - 2.0 {
            x = (self.mouse[0] - 12.0 - w).max(2.0);
        }
        y = y.clamp(2.0, (screen[1] - h - 2.0).max(2.0));
        let th = p.theme;
        p.round_box(x, y, w, h, th.radius, th.floating, th.glass_edge_solid, 0.0, 0.0, 0);
        for (i, line) in lines.iter().enumerate() {
            p.text(line, x + 4.0, y + 4.0 + i as f32 * 10.0, th.text, true);
        }
    }
}

/// A button that's pressed when clicked.
pub fn button(p: &mut Painter, ui: &mut Ui, r: Rect, label: &str, active: bool) -> bool {
    let hot = active && ui.over(r);
    skin::button(p, r, !active, hot, 1.0);
    let th = p.theme;
    let text = p.fit(label, r[2] - 8.0).into_owned();
    let color = if active { th.text } else { th.muted };
    // The game's Button draws its label with the drop shadow.
    p.centered(&text, r[0] + r[2] / 2.0, r[1] + ((r[3] - 7.0) / 2.0).floor(), color, true);
    active && ui.clicked(r)
}

/// A menu's button: an icon, its label on the left, an optional hint at the right in the muted
/// colour; the label eases a little to the right under the pointer in Normal.
pub fn menu_button(
    p: &mut Painter,
    ui: &mut Ui,
    r: Rect,
    label: &str,
    icon: Option<Icon>,
    hint: &str,
    active: bool,
) -> bool {
    menu_button_ext(p, ui, r, label, icon, hint, active, false)
}

/// `menu_button` with its icon and label centred as one, as the game's buttons beside it are.
#[allow(clippy::too_many_arguments)]
pub fn menu_button_ext(
    p: &mut Painter,
    ui: &mut Ui,
    r: Rect,
    label: &str,
    icon: Option<Icon>,
    hint: &str,
    active: bool,
    centered: bool,
) -> bool {
    const ICON: f32 = 9.0;
    let hot = active && ui.over(r);
    skin::button(p, r, !active, hot, 1.0);
    let th = p.theme;
    let color = if active { th.text } else { th.muted };
    let t = p.motion.ease(motion::key(r[0] + 1.0, r[1], r[2], r[3]), if hot { 1.0 } else { 0.0 });
    let mut x = r[0] + 8.0 + (t * 3.0).round();
    let right = r[0] + r[2] - 8.0;
    let icon_room = if icon.is_some() { ICON + 6.0 } else { 0.0 };
    let hint_width = if hint.is_empty() { 0.0 } else { p.width(hint) };
    // The hint goes first when both don't fit: a narrow screen keeps the label readable.
    let show_hint = hint_width > 0.0 && x + icon_room + p.width(label) + 6.0 + hint_width <= right;
    let mut room = right - x - icon_room;
    if centered {
        let content = p.width(label).min(r[2] - 16.0 - icon_room) + icon_room;
        x = r[0] + ((r[2] - content) / 2.0).floor();
        room = content - icon_room;
    }
    let text_y = r[1] + ((r[3] - 7.0) / 2.0).floor();
    if let Some(icon) = icon {
        p.icon(icon, x, r[1] + ((r[3] - ICON) / 2.0).floor(), ICON, color);
        x += ICON + 6.0;
    }
    if !label.is_empty() {
        let text = p.fit(label, room).into_owned();
        p.text(&text, x, text_y, color, false);
    }
    if show_hint {
        p.text(hint, right - hint_width, text_y, th.muted, false);
    }
    active && ui.clicked(r)
}

/// A line of a sidebar: a label, a value at its right, lit while `chosen`.
pub fn entry(p: &mut Painter, ui: &mut Ui, r: Rect, label: &str, value: &str, chosen: bool) -> bool {
    let th = p.theme;
    let hot = ui.over(r);
    if chosen || hot {
        p.round(r[0], r[1], r[2], r[3], th.radius, if chosen { th.button_hot } else { th.button });
    }
    if chosen {
        p.round(r[0], r[1] + 4.0, 2.0, r[3] - 8.0, 1.0, th.accent);
    }
    let text_y = r[1] + ((r[3] - 7.0) / 2.0).floor();
    p.text(label, r[0] + 7.0, text_y, if chosen { th.text } else { th.muted }, false);
    // A long value (a profile's name) is cut, never drawn over the label; whole under the pointer.
    let label_right = r[0] + 7.0 + p.width(label) + 6.0;
    let shown = p.fit(value, r[0] + r[2] - 6.0 - label_right).into_owned();
    if hot && shown != value {
        ui.tooltip(format!("{label}: {value}"));
    }
    let w = p.width(&shown);
    p.text(&shown, r[0] + r[2] - 6.0 - w, text_y, th.muted, false);
    ui.clicked(r)
}

/// An on/off switch showing only its state (a settings row's toggle).
pub fn switch(p: &mut Painter, ui: &mut Ui, r: Rect, on: bool) -> bool {
    let hot = ui.over(r);
    skin::button(p, r, false, hot, 1.0);
    let th = p.theme;
    let label = if on { "On" } else { "Off" };
    let sx = r[0] + r[2] - 8.0 - crate::panels::SWITCH_WIDTH;
    crate::panels::toggle(p, sx, r[1] + ((r[3] - crate::panels::SWITCH_HEIGHT) / 2.0).floor(), on, 1.0);
    p.text(label, r[0] + 8.0, r[1] + ((r[3] - 7.0) / 2.0).floor(), if on { th.text } else { th.muted }, false);
    ui.clicked(r)
}

/// A slider from 0 to 1 at `value`, showing `label`; the value it's dragged to.
pub fn slider(p: &mut Painter, ui: &mut Ui, id: u64, r: Rect, value: f32, label: &str) -> Option<f32> {
    let hot = ui.over(r) || ui.drag == Some(id);
    skin::well(p, r, p.theme.button, hot, 1.0);
    let track = [r[0] + 4.0, r[2] - 8.0];
    let at = r[0] + 4.0 + value.clamp(0.0, 1.0) * track[1];
    let line = r[1] + r[3] - 4.0;
    p.fill(track[0], line, track[1], 1.0, 0x33FF_FFFF);
    p.fill(track[0], line, at - track[0], 1.0, p.theme.accent);
    let knob = if hot { 0xFFFF_FFFF } else { p.theme.accent };
    p.round(at.floor() - 2.0, line - 2.0, 5.0, 5.0, 3.0, knob);
    let th = p.theme;
    let text = p.fit(label, r[2] - 8.0).into_owned();
    p.centered(&text, r[0] + r[2] / 2.0, r[1] + ((r[3] - 9.0) / 2.0).floor(), th.text, true);
    if ui.clicked(r) {
        ui.drag = Some(id);
    }
    if ui.drag == Some(id) {
        return Some(((ui.mouse[0] - track[0]) / track[1]).clamp(0.0, 1.0));
    }
    None
}

/// A one-line text field editing `text`; true when it changed. Click to focus; Enter or Escape
/// let go of it (Escape is then left for the screen).
pub fn text_field(
    p: &mut Painter,
    ui: &mut Ui,
    id: u64,
    r: Rect,
    text: &mut String,
    hint: &str,
    max: usize,
) -> bool {
    if ui.clicked(r) {
        ui.focus = Some(id);
        ui.caret = text.chars().count();
    }
    let focused = ui.focus == Some(id);
    skin::well(p, r, p.theme.field, focused || ui.over(r), 1.0);
    let mut changed = false;
    if focused {
        changed = edit(ui, text, max);
        if ui.key(keys::ENTER) || ui.key(keys::KEYPAD_ENTER) {
            ui.focus = None;
        }
    }
    let th = p.theme;
    let ty = r[1] + ((r[3] - 7.0) / 2.0).floor();
    let room = r[2] - 8.0;
    p.clip_start(r);
    if text.is_empty() && !focused {
        p.text(hint, r[0] + 4.0, ty, alpha(th.muted, 0.8), true);
    } else {
        // Scrolled so the caret stays in the field.
        let before: String = text.chars().take(ui.caret.min(text.chars().count())).collect();
        let caret_x = p.width(&before);
        let shift = (caret_x - room + 2.0).max(0.0);
        p.text(text, r[0] + 4.0 - shift, ty, th.text, true);
        if focused && (p.motion.now / 500.0) as i64 % 2 == 0 {
            p.fill(r[0] + 4.0 + caret_x - shift, ty - 1.0, 1.0, 10.0, th.text);
        }
    }
    p.clip_end();
    changed
}

/// Applies the frame's typing and editing keys to the focused field's `text`.
fn edit(ui: &mut Ui, text: &mut String, max: usize) -> bool {
    let mut changed = false;
    let len = |t: &String| t.chars().count();
    ui.caret = ui.caret.min(len(text));
    let byte = |t: &String, c: usize| t.char_indices().nth(c).map_or(t.len(), |(i, _)| i);
    let typed = std::mem::take(&mut ui.typed);
    for c in typed.chars() {
        if len(text) < max {
            let at = byte(text, ui.caret);
            text.insert(at, c);
            ui.caret += 1;
            changed = true;
        }
    }
    let pressed = std::mem::take(&mut ui.keys);
    for (key, mods) in pressed {
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
            keys::C if control => ui.copy = Some(text.clone()),
            keys::V if control => {
                if let Some(paste) = ui.clipboard.clone() {
                    for c in paste.chars().filter(|c| !c.is_control()) {
                        if len(text) < max {
                            let at = byte(text, ui.caret);
                            text.insert(at, c);
                            ui.caret += 1;
                            changed = true;
                        }
                    }
                }
            }
            keys::BACKSPACE | keys::DELETE | keys::A | keys::C | keys::V | keys::X => {
                // Taken by the field, so the screen doesn't act on them.
            }
            _ => ui.keys.push((key, mods)),
        }
    }
    changed
}

/// A vertical scrollbar's thumb at the right of `area`, for content `content` tall scrolled
/// `scroll` down.
pub fn scrollbar(p: &mut Painter, area: Rect, content: f32, scroll: f32) {
    if content <= area[3] {
        return;
    }
    let thumb = (area[3] * area[3] / content).max(16.0);
    let top = area[1] + ((area[3] - thumb) * scroll / (content - area[3])).floor();
    p.round(area[0] + area[2] + 3.0, top, 2.0, thumb, 1.0, 0x80FF_FFFF);
}

/// A scrolled list's position: moved by the wheel over `area`, kept inside the content.
pub fn scroll(ui: &mut Ui, area: Rect, scroll: &mut f32, content: f32, step: f32) {
    let steps = ui.scrolled(area);
    let max = (content - area[3]).max(0.0);
    *scroll = (*scroll - steps * step).clamp(0.0, max);
}

/// The footer's buttons, centred in a row at `y`: the index of the one pressed.
pub fn footer(p: &mut Painter, ui: &mut Ui, cx: f32, y: f32, labels: &[&str], width: f32) -> Option<usize> {
    let n = labels.len() as f32;
    let total = n * width + (n - 1.0) * GAP;
    let mut x = (cx - total / 2.0).floor();
    let mut pressed = None;
    for (i, label) in labels.iter().enumerate() {
        if button(p, ui, [x, y, width, CONTROL_HEIGHT], label, true) {
            pressed = Some(i);
        }
        x += width + GAP;
    }
    pressed
}

/// A stable id for a control from its name and an index.
pub fn id(name: &str, index: usize) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
    }
    h ^ index as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_press_goes_to_the_control_under_it_once() {
        let mut ui = Ui::default();
        ui.input(&Input::Button { button: 0, pressed: true, at: [15.0, 15.0], modifiers: 0 });
        assert!(!ui.clicked([100.0, 100.0, 10.0, 10.0]));
        assert!(ui.clicked([10.0, 10.0, 10.0, 10.0]));
        assert!(!ui.clicked([10.0, 10.0, 10.0, 10.0]));
        ui.end_frame();
        assert!(ui.down);
    }

    #[test]
    fn edits_text_at_the_caret() {
        let mut ui = Ui::default();
        let mut text = String::from("Zom");
        ui.caret = 2;
        ui.typed.push('o');
        assert!(edit(&mut ui, &mut text, 64));
        assert_eq!(text, "Zoom");
        ui.keys.push((Key::Keyboard(keys::BACKSPACE), 0));
        ui.keys.push((Key::Keyboard(keys::ESCAPE), 0));
        edit(&mut ui, &mut text, 64);
        assert_eq!(text, "Zom");
        assert!(ui.key(keys::ESCAPE), "Escape is left for the screen");
        ui.clipboard = Some("é!".into());
        ui.keys.push((Key::Keyboard(keys::V), input::CONTROL));
        edit(&mut ui, &mut text, 5);
        assert_eq!(text, "Zoé!m");
    }
}
