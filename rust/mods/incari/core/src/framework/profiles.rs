//! Module profiles (framework's `config`): each a JSON file of every module's state, settings and
//! HUD placement, in the Java client's format, so a profile reads the same in either build.
use super::module::Module;
use serde_json::{Map, Value, json};

pub const FORMAT: i64 = 1;
pub const DEFAULT: &str = "Default";
pub const INDEX: &str = "profiles.json";
const MAX_NAME: usize = 24;

/// A profile's file, beside the index.
pub fn file(name: &str) -> String {
    format!("profiles/{name}.json")
}

/// Every module as a profile (ProfileCodec.write), with the entries of modules this build
/// doesn't have kept as they were.
pub fn write(modules: &[Module], unknown: &Map<String, Value>) -> Value {
    let mut entries = unknown.clone();
    for m in modules {
        let mut entry = Map::new();
        entry.insert("enabled".into(), Value::Bool(m.enabled));
        entry.insert("settings".into(), Value::Object(m.settings.to_json()));
        if !m.hud.is_empty() {
            let mut hud = Map::new();
            for e in &m.hud {
                hud.insert(e.id.into(), Value::Object(e.settings.to_json()));
            }
            entry.insert("hud".into(), Value::Object(hud));
        }
        entries.insert(m.id.into(), Value::Object(entry));
    }
    json!({ "format": FORMAT, "modules": entries })
}

/// Reads a profile into the modules (ProfileCodec.read); returns the entries of modules this
/// build doesn't have, to write back as they were.
pub fn read(root: &Value, modules: &mut [Module]) -> Map<String, Value> {
    let entries = root.get("modules").and_then(Value::as_object);
    let mut unknown = Map::new();
    if let Some(entries) = entries {
        for (id, v) in entries {
            if !modules.iter().any(|m| m.id == id) {
                unknown.insert(id.clone(), v.clone());
            }
        }
    }
    for m in modules.iter_mut() {
        let entry = entries.and_then(|e| e.get(m.id)).and_then(Value::as_object);
        m.settings.read_json(
            entry
                .and_then(|e| e.get("settings"))
                .and_then(Value::as_object),
        );
        let hud = entry.and_then(|e| e.get("hud")).and_then(Value::as_object);
        for e in &mut m.hud {
            e.settings
                .read_json(hud.and_then(|h| h.get(e.id)).and_then(Value::as_object));
        }
        m.enabled = m.always_on
            || entry
                .and_then(|e| e.get("enabled"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
    }
    unknown
}

/// The index: the profiles' names and the active one.
pub fn write_index(names: &[String], active: &str) -> Value {
    json!({ "format": FORMAT, "active": active, "profiles": names })
}

/// The index's names (valid and unique) and active profile; `Default` alone without one.
pub fn read_index(root: &Value) -> (Vec<String>, String) {
    let mut names: Vec<String> = Vec::new();
    if let Some(list) = root.get("profiles").and_then(Value::as_array) {
        for n in list.iter().filter_map(Value::as_str) {
            if valid_name(n) && !names.iter().any(|m| m.eq_ignore_ascii_case(n)) {
                names.push(n.into());
            }
        }
    }
    if names.is_empty() {
        names.push(DEFAULT.into());
    }
    let wanted = root
        .get("active")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT);
    let active = names
        .iter()
        .find(|n| n.eq_ignore_ascii_case(wanted))
        .unwrap_or(&names[0])
        .clone();
    (names, active)
}

/// A name a profile may have: up to 24 letters, digits, spaces, `_`, `-` and `.`, not starting
/// with a dot or a space.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME
        && name.trim() == name
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-' | '.'))
}

/// JSON as Gson's pretty printer writes it (two spaces).
pub fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::hud::{Anchor, HudElement};
    use crate::framework::module::Category;
    use crate::framework::setting as s;

    fn modules() -> Vec<Module> {
        vec![
            Module::new("cps", "CPS", "Clicks.", Category::Hud)
                .setting(s::choice(
                    "buttons",
                    "Buttons",
                    &[("both", "Both"), ("left", "Left")],
                    "both",
                ))
                .element(HudElement::text("cps", "CPS", Anchor::Left, 2, -48)),
            Module::new("theme", "Theme", "Colours.", Category::Visual).always_on(),
        ]
    }

    #[test]
    fn reads_what_the_java_client_wrote() {
        let mut m = modules();
        let stored: Value = serde_json::from_str(
            r##"{"format":1,"modules":{
              "cps":{"enabled":true,"settings":{"toggle":"keyboard:6","buttons":"left"},
                     "hud":{"cps":{"anchor":"top_right","x":4,"y":108,"scale":1.5,"background":false}}},
              "schematics":{"enabled":true,"settings":{}}}}"##,
        )
        .unwrap();
        let unknown = read(&stored, &mut m);
        assert!(m[0].enabled && m[1].enabled, "always on stays on");
        assert_eq!(m[0].settings.choice("buttons"), "left");
        assert_eq!(m[0].hud[0].anchor(), Anchor::TopRight);
        assert_eq!(m[0].hud[0].scale(), 1.5);
        assert!(!m[0].hud[0].settings.bool("background"));
        assert!(unknown.contains_key("schematics"));
        let written = write(&m, &unknown);
        assert_eq!(
            written["modules"]["cps"]["hud"]["cps"]["anchor"],
            "top_right"
        );
        assert_eq!(
            written["modules"]["cps"]["settings"]["toggle"],
            "keyboard:6"
        );
        assert_eq!(written["modules"]["schematics"]["enabled"], true);
        let mut again = modules();
        read(&written, &mut again);
        assert_eq!(again, m);
    }

    #[test]
    fn checks_profile_names() {
        assert!(valid_name("QoL"));
        assert!(valid_name("PvP 1.8"));
        assert!(!valid_name(".hidden"));
        assert!(!valid_name(" padded"));
        assert!(!valid_name("a/b"));
        let (names, active) =
            read_index(&json!({"active": "qol", "profiles": ["Default", "QoL", "QoL"]}));
        assert_eq!(names, ["Default", "QoL"]);
        assert_eq!(active, "QoL");
    }
}
