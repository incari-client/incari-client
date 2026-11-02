//! A module's settings (ModuleSettingsScreen, SettingControls): a row a setting, its control and a
//! Reset at the right, then each HUD element's settings under a heading; Edit HUD, Reset All and
//! Done below. Generated from the module's settings alone.
use super::list::{List, Rows};
use crate::color;
use crate::framework::setting::{Kind, Setting, Settings, Val};
use crate::input::{Input, Key, keys};
use crate::screen::{Cx, Go, Screen};
use crate::theme::{CONTROL_WIDTH, FOOTER_BUTTON_WIDTH, GAP, SMALL_BUTTON_WIDTH};
use crate::ui;

pub struct ModuleSettings {
    module: &'static str,
    list: List,
    /// The key setting waiting for a key: its element (None for the module's own) and id.
    capturing: Option<(Option<usize>, &'static str)>,
    /// Colour fields' text while typed in, by the setting's control id.
    typing: Vec<(u64, String)>,
}

impl ModuleSettings {
    pub fn new(module: &'static str) -> Self {
        Self {
            module,
            list: List::default(),
            capturing: None,
            typing: Vec::new(),
        }
    }
}

impl Screen for ModuleSettings {
    fn escape(&mut self, cx: &mut Cx) {
        if let Some((element, id)) = self.capturing.take() {
            // Escape while capturing unbinds the key.
            if let Some(s) = setting_mut(cx, self.module, element, id) {
                s.set(Val::Key(Key::None));
            }
            cx.modules.changed();
            return;
        }
        cx.go(Go::Back);
    }

    fn draw(&mut self, cx: &mut Cx) {
        let Some(m) = cx.modules.module(self.module) else {
            cx.go(Go::Back);
            return;
        };
        let (name, description) = (m.name, m.description);
        let hud = !m.hud.is_empty();
        self.capture(cx);
        let mut labels = vec![];
        if hud {
            labels.push("Edit HUD");
        }
        labels.push("Reset All");
        let done = cx.host.translate("gui.done");
        labels.push(&done);
        // Three buttons share the footer's width when the module has a HUD element to place.
        let width = if hud { (FOOTER_BUTTON_WIDTH * 2.0 / 3.0).floor() } else { FOOTER_BUTTON_WIDTH };
        let module = self.module;
        let capturing = self.capturing;
        let mut capture = None;
        let typing = &mut self.typing;
        let pressed = self.list.draw(
            cx,
            name,
            description,
            false,
            |cx, rows| {
                let settings = cx.modules.module(module).map(|m| m.settings.clone()).unwrap_or_default();
                if let Some(c) = rows_for(cx, rows, module, None, &settings, capturing, typing) {
                    capture = Some(c);
                }
                let elements: Vec<_> = cx
                    .modules
                    .module(module)
                    .map(|m| m.hud.iter().map(|e| (e.name, e.settings.clone())).collect())
                    .unwrap_or_default();
                for (i, (element, settings)) in elements.into_iter().enumerate() {
                    if settings.any_shown() {
                        rows.heading(cx, &format!("HUD: {element}"));
                        if let Some(c) = rows_for(cx, rows, module, Some(i), &settings, capturing, typing) {
                            capture = Some(c);
                        }
                    }
                }
            },
            &labels,
            Some(width),
        );
        if capture.is_some() {
            self.capturing = capture;
        }
        let pressed = pressed.map(|i| if hud { i } else { i + 1 });
        match pressed {
            Some(0) => cx.open(super::hud_editor::HudEditor::default()),
            Some(1) => {
                if let Some(m) = cx.modules.module_mut(module) {
                    m.settings.reset_all();
                    for e in &mut m.hud {
                        e.settings.reset_all();
                    }
                }
                cx.modules.changed();
            }
            Some(_) => cx.go(Go::Back),
            None => {}
        }
    }
}

impl ModuleSettings {
    /// While a key setting waits: the next key or mouse button binds it.
    fn capture(&mut self, cx: &mut Cx) {
        let Some((element, id)) = self.capturing else {
            return;
        };
        let key = if let Some(i) = cx.ui.keys.iter().position(|(k, _)| *k != Key::Keyboard(keys::ESCAPE)) {
            Some(cx.ui.keys.remove(i).0)
        } else if let Some(at) = cx.ui.pressed_anywhere() {
            cx.ui.eat_press();
            let _ = at;
            Some(Key::Mouse(0))
        } else {
            None
        };
        if let Some(key) = key {
            self.capturing = None;
            if let Some(s) = setting_mut(cx, self.module, element, id) {
                s.set(Val::Key(key));
            }
            cx.modules.changed();
        }
    }
}

/// The setting `id` of module `module` (or of its HUD element `element`).
fn setting_mut<'a>(cx: &'a mut Cx, module: &str, element: Option<usize>, id: &str) -> Option<&'a mut Setting> {
    let m = cx.modules.module_mut(module)?;
    match element {
        Some(i) => m.hud.get_mut(i)?.settings.get_mut(id),
        None => m.settings.get_mut(id),
    }
}

/// The rows of `settings`; returns a key setting clicked to capture.
fn rows_for(
    cx: &mut Cx,
    rows: &mut Rows,
    module: &'static str,
    element: Option<usize>,
    settings: &Settings,
    capturing: Option<(Option<usize>, &'static str)>,
    typing: &mut Vec<(u64, String)>,
) -> Option<(Option<usize>, &'static str)> {
    let mut capture = None;
    for (n, s) in settings.all.iter().enumerate() {
        if !settings.is_visible(s) {
            continue;
        }
        let control_id = ui::id(module, element.map_or(0, |e| e + 1) * 1000 + n);
        if let Kind::Action { label } = s.kind {
            // As wide as another row's control and its Reset together: it holds nothing to reset.
            let Some(r) = rows.row(cx, s.name, s.description, &[CONTROL_WIDTH + GAP + SMALL_BUTTON_WIDTH]) else {
                continue;
            };
            if ui::button(cx.p, cx.ui, r[0], label, true) {
                cx.modules.actions.push((module, s.id));
            }
            continue;
        }
        let Some(r) = rows.row(cx, s.name, s.description, &[CONTROL_WIDTH, SMALL_BUTTON_WIDTH]) else {
            continue;
        };
        let mut set: Option<Val> = None;
        match &s.kind {
            Kind::Bool { .. } => {
                let label = if s.as_bool() { "ON" } else { "OFF" };
                if ui::button(cx.p, cx.ui, r[0], label, true) {
                    set = Some(Val::Bool(!s.as_bool()));
                }
            }
            Kind::Int { min, max, .. } => {
                let span = (max - min).max(1) as f32;
                let at = (s.as_int() - min) as f32 / span;
                if let Some(v) = ui::slider(cx.p, cx.ui, control_id, r[0], at, &s.label()) {
                    set = Some(Val::Int(min + (v * span).round() as i32));
                }
            }
            Kind::Float { min, max, .. } => {
                let span = (max - min).max(f32::EPSILON);
                let at = (s.as_float() - min) / span;
                if let Some(v) = ui::slider(cx.p, cx.ui, control_id, r[0], at, &s.label()) {
                    set = Some(Val::Float(min + v * span));
                }
            }
            Kind::Choice { choices, .. } => {
                if ui::button(cx.p, cx.ui, r[0], &s.label(), true) {
                    set = Some(Val::Choice((s.as_choice() + 1) % choices.len()));
                }
                if cx.ui.right_clicked(r[0]) {
                    set = Some(Val::Choice((s.as_choice() + choices.len() - 1) % choices.len()));
                }
            }
            Kind::Color { alpha, .. } => {
                set = color_field(cx, control_id, r[0], s.as_color(), *alpha, typing).map(Val::Color);
            }
            Kind::Key { .. } => {
                let waiting = capturing == Some((element, s.id));
                let name = s.as_key().label();
                let label = if waiting { format!("> {name} <") } else { name };
                if ui::button(cx.p, cx.ui, r[0], &label, true) {
                    capture = Some((element, s.id));
                }
            }
            Kind::Grid { .. } => {
                if ui::button(cx.p, cx.ui, r[0], "Edit...", true) {
                    cx.open(super::grid::Grid::new(module, s.id));
                }
            }
            Kind::Action { .. } => {}
        }
        if ui::button(cx.p, cx.ui, r[1], "Reset", !s.is_default()) {
            if let Some(s) = setting_mut(cx, module, element, s.id) {
                s.reset();
            }
            cx.modules.changed();
        }
        if let Some(v) = set {
            if let Some(s) = setting_mut(cx, module, element, s.id) {
                s.set(v);
            }
            if element.is_none() {
                crate::modules::changed(cx.modules, module, s.id);
            }
            cx.modules.changed();
        }
    }
    let _ = Input::Move { at: [0.0, 0.0] };
    capture
}

/// A field for a colour as `#RRGGBB` (`#AARRGGBB` with alpha) with a swatch of it at its right:
/// the colour typed, once it's a whole one.
fn color_field(
    cx: &mut Cx,
    id: u64,
    r: [f32; 4],
    argb: u32,
    with_alpha: bool,
    typing: &mut Vec<(u64, String)>,
) -> Option<u32> {
    let shown = if with_alpha {
        color::hex(argb)
    } else {
        format!("#{:06X}", argb & 0xFF_FFFF)
    };
    let at = match typing.iter().position(|t| t.0 == id) {
        Some(i) => i,
        None => {
            typing.push((id, shown.clone()));
            typing.len() - 1
        }
    };
    if cx.ui.focus != Some(id) && typing[at].1 != shown {
        typing[at].1 = shown;
    }
    let mut text = std::mem::take(&mut typing[at].1);
    let field = [r[0], r[1], r[2] - 20.0, r[3]];
    let changed = ui::text_field(cx.p, cx.ui, id, field, &mut text, "", if with_alpha { 9 } else { 7 });
    let parsed = color::parse_hex(&text).filter(|_| text.len() == if with_alpha { 9 } else { 7 });
    typing[at].1 = text;
    let (sx, sy) = (r[0] + r[2] - 16.0, r[1] + 4.0);
    let muted = cx.p.theme.muted;
    cx.p.fill(sx - 1.0, sy - 1.0, 14.0, 14.0, muted);
    cx.p.fill(sx, sy, 12.0, 12.0, argb | if with_alpha { 0 } else { 0xFF00_0000 });
    if changed { parsed } else { None }
}
