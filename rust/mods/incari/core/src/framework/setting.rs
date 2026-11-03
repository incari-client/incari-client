//! A module's settings (framework's `setting` package): values the module menu edits and the
//! profiles keep, by id. Ids are config keys and never change once shipped.
use crate::color;
use crate::input::Key;
use serde_json::Value;

/// What a setting holds, and its bounds.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Bool {
        default: bool,
    },
    Int {
        min: i32,
        max: i32,
        default: i32,
    },
    Float {
        min: f32,
        max: f32,
        step: f32,
        default: f32,
    },
    /// Choices by key (as profiles store them, an enum's lowercase name) and label.
    Choice {
        choices: &'static [(&'static str, &'static str)],
        default: usize,
    },
    Color {
        default: u32,
        alpha: bool,
    },
    Key {
        default: Key,
    },
    /// A button that does something, with its label; holds nothing.
    Action {
        label: &'static str,
    },
    /// A square grid of on/off cells (the crosshair editor's), rows as bits.
    Grid {
        size: usize,
        default: &'static [u64],
    },
}

/// A setting's value.
#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Bool(bool),
    Int(i32),
    Float(f32),
    Choice(usize),
    Color(u32),
    Key(Key),
    None,
    Grid(Vec<u64>),
}

/// When a setting shows: always, or while another bool setting (by id) is on or off, or a choice
/// setting has (or hasn't) a key.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shown {
    Always,
    Never,
    When(&'static str, bool),
    Is(&'static str, &'static str),
    Not(&'static str, &'static str),
    /// While a choice setting has one of the keys.
    IsAny(&'static str, &'static [&'static str]),
    /// While a choice setting has none of the keys.
    NotAny(&'static str, &'static [&'static str]),
    /// While a bool setting is on and another bool setting is too.
    Both(&'static str, &'static str),
    /// While a bool setting is on and a choice setting hasn't the key.
    WhenNot(&'static str, &'static str, &'static str),
}

/// One setting.
#[derive(Clone, Debug)]
pub struct Setting {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub kind: Kind,
    pub shown: Shown,
    pub value: Val,
    /// How a number is written ("1.5x", "70%"): `None` for the plain number.
    pub format: Option<fn(f64) -> String>,
}

/// Equal by everything but the format, which a function pointer can't be compared by.
impl PartialEq for Setting {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name == other.name
            && self.description == other.description
            && self.kind == other.kind
            && self.shown == other.shown
            && self.value == other.value
    }
}

impl Setting {
    pub fn new(id: &'static str, name: &'static str, kind: Kind) -> Self {
        let value = default_of(&kind);
        Self {
            id,
            name,
            description: "",
            kind,
            shown: Shown::Always,
            value,
            format: None,
        }
    }

    pub fn described(mut self, sentence: &'static str) -> Self {
        self.description = sentence;
        self
    }

    pub fn hidden(mut self) -> Self {
        self.shown = Shown::Never;
        self
    }

    pub fn shown(mut self, when: Shown) -> Self {
        self.shown = when;
        self
    }

    pub fn formatted(mut self, format: fn(f64) -> String) -> Self {
        self.format = Some(format);
        self
    }

    pub fn is_hidden(&self) -> bool {
        self.shown == Shown::Never
    }

    pub fn is_default(&self) -> bool {
        self.value == default_of(&self.kind)
    }

    pub fn reset(&mut self) {
        self.value = default_of(&self.kind);
    }

    /// Sets `value`, clamped and stepped as the Java settings do. False for a value of the
    /// wrong kind.
    pub fn set(&mut self, value: Val) -> bool {
        let kept = match (&self.kind, value) {
            (Kind::Bool { .. }, Val::Bool(b)) => Val::Bool(b),
            (Kind::Int { min, max, .. }, Val::Int(v)) => Val::Int(v.clamp(*min, *max)),
            (Kind::Float { min, max, step, .. }, Val::Float(v)) => {
                if v.is_nan() {
                    return false;
                }
                let stepped = min + ((v - min) / step).round() * step;
                Val::Float(stepped.clamp(*min, *max))
            }
            (Kind::Choice { choices, .. }, Val::Choice(i)) if i < choices.len() => Val::Choice(i),
            (Kind::Color { alpha, .. }, Val::Color(c)) => {
                Val::Color(if *alpha { c } else { c | 0xFF00_0000 })
            }
            (Kind::Key { .. }, Val::Key(k)) => Val::Key(k),
            (Kind::Grid { size, .. }, Val::Grid(rows)) if rows.len() == *size => Val::Grid(rows),
            _ => return false,
        };
        self.value = kept;
        true
    }

    pub fn as_bool(&self) -> bool {
        matches!(self.value, Val::Bool(true))
    }

    pub fn as_int(&self) -> i32 {
        match self.value {
            Val::Int(v) => v,
            Val::Float(v) => v.round() as i32,
            Val::Choice(v) => v as i32,
            _ => 0,
        }
    }

    pub fn as_float(&self) -> f32 {
        match self.value {
            Val::Float(v) => v,
            Val::Int(v) => v as f32,
            _ => 0.0,
        }
    }

    pub fn as_choice(&self) -> usize {
        match self.value {
            Val::Choice(v) => v,
            _ => 0,
        }
    }

    /// The chosen choice's key.
    pub fn choice_key(&self) -> &'static str {
        match (&self.kind, &self.value) {
            (Kind::Choice { choices, .. }, Val::Choice(i)) => choices[*i].0,
            _ => "",
        }
    }

    pub fn as_color(&self) -> u32 {
        match self.value {
            Val::Color(c) => c,
            _ => 0xFFFF_FFFF,
        }
    }

    pub fn as_key(&self) -> Key {
        match self.value {
            Val::Key(k) => k,
            _ => Key::None,
        }
    }

    /// The value as the module menu writes it beside the setting's name.
    pub fn label(&self) -> String {
        match (&self.kind, &self.value) {
            (_, Val::Bool(b)) => if *b { "On" } else { "Off" }.into(),
            (_, Val::Int(v)) => self.format.map_or_else(|| v.to_string(), |f| f(*v as f64)),
            (Kind::Float { step, .. }, Val::Float(v)) => self
                .format
                .map_or_else(|| trim(*v as f64, decimals(*step)), |f| f(*v as f64)),
            (Kind::Choice { choices, .. }, Val::Choice(i)) => choices[*i].1.into(),
            (_, Val::Color(c)) => color::hex(*c),
            (_, Val::Key(k)) => k.label(),
            (Kind::Action { label }, _) => (*label).into(),
            _ => String::new(),
        }
    }

    /// The value as profiles store it (ProfileCodec's writer); null for an action.
    pub fn to_json(&self) -> Value {
        match &self.value {
            Val::Bool(b) => Value::Bool(*b),
            Val::Int(v) => Value::from(*v),
            Val::Float(v) => serde_json::Number::from_f64(*v as f64)
                .map_or(Value::Null, |n| Value::Number(java_float(n, *v))),
            Val::Choice(_) => Value::String(self.choice_key().into()),
            Val::Color(c) => Value::String(color::hex(*c)),
            Val::Key(k) => Value::String(k.encode()),
            Val::None => Value::Null,
            Val::Grid(rows) => Value::String(
                rows.iter()
                    .map(|r| format!("{r:x}"))
                    .collect::<Vec<_>>()
                    .join(","),
            ),
        }
    }

    /// Reads a stored value (ProfileCodec's reader): false when it isn't one this setting takes.
    pub fn from_json(&mut self, value: &Value) -> bool {
        let read = match (&self.kind, value) {
            (Kind::Bool { .. }, Value::Bool(b)) => Val::Bool(*b),
            (Kind::Int { .. }, Value::Number(n)) => Val::Int(n.as_f64().unwrap_or(0.0) as i32),
            (Kind::Float { .. }, Value::Number(n)) => Val::Float(n.as_f64().unwrap_or(0.0) as f32),
            (Kind::Choice { choices, .. }, Value::String(s)) => {
                match choices.iter().position(|c| c.0 == s) {
                    Some(i) => Val::Choice(i),
                    None => return false,
                }
            }
            (Kind::Color { .. }, Value::String(s)) => match color::parse_hex(s) {
                Some(c) if s.len() == 7 || s.len() == 9 => Val::Color(c),
                _ => return false,
            },
            (Kind::Key { .. }, Value::String(s)) => match Key::decode(s) {
                Some(k) => Val::Key(k),
                None => return false,
            },
            (Kind::Action { .. }, _) => return true,
            (Kind::Grid { size, .. }, Value::String(s)) => {
                let rows: Option<Vec<u64>> = s
                    .split(',')
                    .map(|p| u64::from_str_radix(p, 16).ok())
                    .collect();
                match rows {
                    Some(rows) if rows.len() == *size => Val::Grid(rows),
                    _ => return false,
                }
            }
            _ => return false,
        };
        self.set(read)
    }
}

/// A Java float as Gson writes it: the shortest decimal that reads back as the same float.
fn java_float(n: serde_json::Number, v: f32) -> serde_json::Number {
    let shortest: f64 = format!("{v}").parse().unwrap_or(v as f64);
    serde_json::Number::from_f64(shortest).unwrap_or(n)
}

fn default_of(kind: &Kind) -> Val {
    match kind {
        Kind::Bool { default } => Val::Bool(*default),
        Kind::Int { default, .. } => Val::Int(*default),
        Kind::Float { default, .. } => Val::Float(*default),
        Kind::Choice { default, .. } => Val::Choice(*default),
        Kind::Color { default, alpha } => Val::Color(if *alpha {
            *default
        } else {
            default | 0xFF00_0000
        }),
        Kind::Key { default } => Val::Key(*default),
        Kind::Action { .. } => Val::None,
        Kind::Grid { default, .. } => Val::Grid(default.to_vec()),
    }
}

/// How many decimals a step needs (FloatSetting.decimals).
pub fn decimals(step: f32) -> usize {
    for decimals in 0..4 {
        let scaled = step as f64 * 10f64.powi(decimals as i32);
        if (scaled - scaled.round()).abs() < 1e-4 {
            return decimals;
        }
    }
    4
}

/// `value` with `decimals` decimals (FloatSetting.trim).
pub fn trim(value: f64, decimals: usize) -> String {
    let scale = 10u64.pow(decimals as u32);
    let rounded = (value.abs() * scale as f64).round() as u64;
    let sign = if value < 0.0 && rounded != 0 { "-" } else { "" };
    if decimals == 0 {
        return format!("{sign}{rounded}");
    }
    format!(
        "{sign}{}.{:0width$}",
        rounded / scale,
        rounded % scale,
        width = decimals
    )
}

/// A module's or a HUD element's settings, in the order the menu shows them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Settings {
    pub all: Vec<Setting>,
}

impl Settings {
    pub fn add(&mut self, setting: Setting) -> usize {
        debug_assert!(
            self.index(setting.id).is_none(),
            "two settings {}",
            setting.id
        );
        self.all.push(setting);
        self.all.len() - 1
    }

    pub fn index(&self, id: &str) -> Option<usize> {
        self.all.iter().position(|s| s.id == id)
    }

    pub fn get(&self, id: &str) -> Option<&Setting> {
        self.all.iter().find(|s| s.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Setting> {
        self.all.iter_mut().find(|s| s.id == id)
    }

    pub fn bool(&self, id: &str) -> bool {
        self.get(id).is_some_and(Setting::as_bool)
    }

    pub fn int(&self, id: &str) -> i32 {
        self.get(id).map_or(0, Setting::as_int)
    }

    pub fn float(&self, id: &str) -> f32 {
        self.get(id).map_or(0.0, Setting::as_float)
    }

    pub fn choice(&self, id: &str) -> &'static str {
        self.get(id).map_or("", Setting::choice_key)
    }

    pub fn color(&self, id: &str) -> u32 {
        self.get(id).map_or(0xFFFF_FFFF, Setting::as_color)
    }

    pub fn key(&self, id: &str) -> Key {
        self.get(id).map_or(Key::None, Setting::as_key)
    }

    /// Whether setting `s` shows now, given the others' values.
    pub fn is_visible(&self, s: &Setting) -> bool {
        match s.shown {
            Shown::Always => true,
            Shown::Never => false,
            Shown::When(id, on) => self.bool(id) == on,
            Shown::Is(id, key) => self.choice(id) == key,
            Shown::Not(id, key) => self.choice(id) != key,
            Shown::IsAny(id, keys) => keys.contains(&self.choice(id)),
            Shown::NotAny(id, keys) => !keys.contains(&self.choice(id)),
            Shown::Both(a, b) => self.bool(a) && self.bool(b),
            Shown::WhenNot(on, id, key) => self.bool(on) && self.choice(id) != key,
        }
    }

    pub fn any_shown(&self) -> bool {
        self.all.iter().any(|s| !s.is_hidden())
    }

    pub fn reset_all(&mut self) {
        for s in &mut self.all {
            s.reset();
        }
    }

    /// The settings as a profile's `settings` object.
    pub fn to_json(&self) -> serde_json::Map<String, Value> {
        let mut out = serde_json::Map::new();
        for s in &self.all {
            let v = s.to_json();
            if !v.is_null() {
                out.insert(s.id.into(), v);
            }
        }
        out
    }

    /// Reads a profile's `settings` object; a setting missing or unreadable goes to its default.
    pub fn read_json(&mut self, values: Option<&serde_json::Map<String, Value>>) {
        for s in &mut self.all {
            let read = values
                .and_then(|v| v.get(s.id))
                .is_some_and(|v| !v.is_object() && !v.is_array() && s.from_json(v));
            if !read {
                s.reset();
            }
        }
    }
}

/// Building settings as the Java modules declare them.
pub fn bool(id: &'static str, name: &'static str, default: bool) -> Setting {
    Setting::new(id, name, Kind::Bool { default })
}

pub fn int(id: &'static str, name: &'static str, min: i32, max: i32, default: i32) -> Setting {
    Setting::new(id, name, Kind::Int { min, max, default })
}

pub fn float(
    id: &'static str,
    name: &'static str,
    min: f32,
    max: f32,
    step: f32,
    default: f32,
) -> Setting {
    Setting::new(
        id,
        name,
        Kind::Float {
            min,
            max,
            step,
            default,
        },
    )
}

pub fn choice(
    id: &'static str,
    name: &'static str,
    choices: &'static [(&'static str, &'static str)],
    default: &'static str,
) -> Setting {
    let default = choices.iter().position(|c| c.0 == default).unwrap_or(0);
    Setting::new(id, name, Kind::Choice { choices, default })
}

pub fn color(id: &'static str, name: &'static str, argb: u32, alpha: bool) -> Setting {
    Setting::new(
        id,
        name,
        Kind::Color {
            default: argb,
            alpha,
        },
    )
}

pub fn key(id: &'static str, name: &'static str, default: Key) -> Setting {
    Setting::new(id, name, Kind::Key { default })
}

pub fn action(id: &'static str, name: &'static str, label: &'static str) -> Setting {
    Setting::new(id, name, Kind::Action { label })
}

pub fn grid(id: &'static str, name: &'static str, size: usize, default: &'static [u64]) -> Setting {
    Setting::new(id, name, Kind::Grid { size, default })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPRINT: &[(&str, &str)] = &[("hold", "Hold"), ("toggle", "Toggle")];

    #[test]
    fn clamps_and_steps_as_the_java_settings_do() {
        let mut s = float("scale", "Scale", 0.5, 3.0, 0.25, 1.0);
        s.set(Val::Float(1.3));
        assert_eq!(s.as_float(), 1.25);
        s.set(Val::Float(9.0));
        assert_eq!(s.as_float(), 3.0);
        let mut i = int("size", "Size", 10, 40, 20);
        i.set(Val::Int(99));
        assert_eq!(i.as_int(), 40);
        assert_eq!(trim(1.25, 2), "1.25");
        assert_eq!(trim(-0.001, 1), "0.0");
        assert_eq!(decimals(0.25), 2);
    }

    #[test]
    fn reads_and_writes_profile_values() {
        let mut settings = Settings::default();
        settings.add(key("toggle", "Toggle key", Key::None));
        settings.add(choice("sprint", "Sprint", SPRINT, "hold"));
        settings.add(bool("show-status", "Show status", true));
        settings.add(color("color", "Color", 0xFFFFFF, false));
        settings.add(float("scale", "Scale", 0.5, 3.0, 0.25, 1.0));
        let stored: serde_json::Value = serde_json::json!({
            "toggle": "keyboard:229", "sprint": "toggle", "show-status": "yes", "color": "#80FF0000",
            "scale": 1.5
        });
        settings.read_json(stored.as_object());
        assert_eq!(settings.key("toggle"), Key::Keyboard(229));
        assert_eq!(settings.choice("sprint"), "toggle");
        assert!(
            settings.bool("show-status"),
            "a value of the wrong kind is the default"
        );
        assert_eq!(
            settings.color("color"),
            0xFFFF_0000,
            "an opaque colour stays opaque"
        );
        let written = serde_json::Value::Object(settings.to_json());
        assert_eq!(written["sprint"], "toggle");
        assert_eq!(written["color"], "#FFFF0000");
        assert_eq!(written["scale"], 1.5);
    }

    #[test]
    fn shows_settings_by_the_others() {
        let mut settings = Settings::default();
        settings.add(bool("custom-color", "Custom color", false));
        settings
            .add(color("color", "Color", 0xFFFFFF, false).shown(Shown::When("custom-color", true)));
        let c = settings.get("color").unwrap().clone();
        assert!(!settings.is_visible(&c));
        settings
            .get_mut("custom-color")
            .unwrap()
            .set(Val::Bool(true));
        assert!(settings.is_visible(&c));
    }
}
