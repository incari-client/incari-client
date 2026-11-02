//! The modules and their profiles (ModuleSystem): which are on, toggle keys, saving the active
//! profile a moment after a change and at once after a key's toggle.
use super::module::Module;
use super::profiles;
use crate::input::Key;
use serde_json::{Map, Value};

/// Where profiles are kept, relative to `incari/modules/` in the game folder.
pub trait Storage {
    fn read(&self, path: &str) -> Option<String>;
    fn write(&mut self, path: &str, text: &str);
    fn delete(&mut self, path: &str);
}

/// Ticks after a change before it's saved: a slider's drag is one save.
const SAVE_DELAY: i32 = 40;

pub struct ModuleSystem {
    pub modules: Vec<Module>,
    names: Vec<String>,
    active: String,
    unknown: Map<String, Value>,
    save_countdown: i32,
    /// Bumped on every change, so screens and the HUD redo what they cached.
    pub revision: u64,
    /// Action settings pressed (module id, setting id), for what does them.
    pub actions: Vec<(&'static str, &'static str)>,
    /// Each HUD element's size at scale 1 as it last drew (module id, element id): the HUD
    /// editor's boxes.
    pub sizes: std::collections::HashMap<(&'static str, &'static str), [f32; 2]>,
}

impl ModuleSystem {
    pub fn new(modules: Vec<Module>) -> Self {
        Self {
            modules,
            names: vec![profiles::DEFAULT.into()],
            active: profiles::DEFAULT.into(),
            unknown: Map::new(),
            save_countdown: -1,
            revision: 0,
            actions: Vec::new(),
            sizes: std::collections::HashMap::new(),
        }
    }

    pub fn module(&self, id: &str) -> Option<&Module> {
        self.modules.iter().find(|m| m.id == id)
    }

    pub fn module_mut(&mut self, id: &str) -> Option<&mut Module> {
        self.modules.iter_mut().find(|m| m.id == id)
    }

    /// Whether module `id` is on.
    pub fn on(&self, id: &str) -> bool {
        self.module(id).is_some_and(|m| m.enabled)
    }

    /// Reads the index and the active profile.
    pub fn load(&mut self, storage: &dyn Storage) {
        let index = storage
            .read(profiles::INDEX)
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or(Value::Null);
        let (names, active) = profiles::read_index(&index);
        self.names = names;
        self.active = active;
        self.apply(storage);
        self.revision += 1;
    }

    fn apply(&mut self, storage: &dyn Storage) {
        let root = storage
            .read(&profiles::file(&self.active))
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or(Value::Null);
        self.unknown = profiles::read(&root, &mut self.modules);
    }

    /// Something changed: saved in a couple of seconds.
    pub fn changed(&mut self) {
        self.revision += 1;
        if self.save_countdown < 0 {
            self.save_countdown = SAVE_DELAY;
        }
    }

    pub fn set_enabled(&mut self, id: &str, on: bool) {
        if let Some(m) = self.module_mut(id)
            && m.enabled != on
            && !(m.always_on && !on)
        {
            m.enabled = on;
            self.changed();
        }
    }

    pub fn toggle(&mut self, id: &str) {
        let on = self.on(id);
        self.set_enabled(id, !on);
    }

    /// A key went down: modules whose toggle key it is turn on or off (saved at once). True
    /// when one did.
    pub fn key_pressed(&mut self, key: Key, storage: &mut dyn Storage) -> bool {
        if !key.is_bound() {
            return false;
        }
        let ids: Vec<&'static str> = self
            .modules
            .iter()
            .filter(|m| !m.always_on && m.settings.key("toggle") == key)
            .map(|m| m.id)
            .collect();
        for id in &ids {
            self.toggle(id);
        }
        if !ids.is_empty() {
            self.flush(storage);
        }
        !ids.is_empty()
    }

    /// Once a tick: saves a change that's had time to settle.
    pub fn tick(&mut self, storage: &mut dyn Storage) {
        if self.save_countdown > 0 {
            self.save_countdown -= 1;
            if self.save_countdown == 0 {
                self.flush(storage);
            }
        }
    }

    /// Saves now, if anything changed.
    pub fn flush(&mut self, storage: &mut dyn Storage) {
        if self.save_countdown < 0 {
            return;
        }
        self.save_countdown = -1;
        self.save(storage);
    }

    fn save(&mut self, storage: &mut dyn Storage) {
        let profile = profiles::write(&self.modules, &self.unknown);
        storage.write(&profiles::file(&self.active), &profiles::pretty(&profile));
        self.write_index(storage);
    }

    fn write_index(&self, storage: &mut dyn Storage) {
        let index = profiles::write_index(&self.names, &self.active);
        storage.write(profiles::INDEX, &profiles::pretty(&index));
    }

    pub fn active_profile(&self) -> &str {
        &self.active
    }

    pub fn profiles(&self) -> &[String] {
        &self.names
    }

    pub fn switch_profile(&mut self, name: &str, storage: &mut dyn Storage) -> bool {
        let Some(found) = self.names.iter().find(|n| n.eq_ignore_ascii_case(name)).cloned() else {
            return false;
        };
        self.save(storage);
        self.save_countdown = -1;
        self.active = found;
        self.apply(storage);
        self.write_index(storage);
        self.revision += 1;
        true
    }

    pub fn create_profile(&mut self, name: &str, storage: &mut dyn Storage) -> bool {
        if !profiles::valid_name(name) || self.names.iter().any(|n| n.eq_ignore_ascii_case(name)) {
            return false;
        }
        self.save(storage);
        self.names.push(name.into());
        self.active = name.into();
        self.save(storage);
        self.save_countdown = -1;
        self.revision += 1;
        true
    }

    pub fn delete_profile(&mut self, name: &str, storage: &mut dyn Storage) -> bool {
        let Some(at) = self.names.iter().position(|n| n.eq_ignore_ascii_case(name)) else {
            return false;
        };
        if self.names.len() == 1 {
            return false;
        }
        let deleted = self.names.remove(at);
        storage.delete(&profiles::file(&deleted));
        if deleted == self.active {
            self.active = self.names[0].clone();
            self.apply(storage);
        }
        self.write_index(storage);
        self.revision += 1;
        true
    }

    /// Switches to (or makes) profile `name` with exactly the modules `on` turned on, and the
    /// settings named `module.setting` set from text (Presets).
    pub fn apply_preset(
        &mut self,
        name: &str,
        on: &[&str],
        settings: &[(&str, &str)],
        storage: &mut dyn Storage,
    ) -> bool {
        let exists = self.names.iter().any(|n| n.eq_ignore_ascii_case(name));
        let ready = if exists {
            self.switch_profile(name, storage)
        } else {
            self.create_profile(name, storage)
        };
        if !ready {
            return false;
        }
        for m in &mut self.modules {
            if !m.always_on {
                m.enabled = on.contains(&m.id);
            }
            for setting in &mut m.settings.all {
                let key = format!("{}.{}", m.id, setting.id);
                if let Some((_, text)) = settings.iter().find(|(k, _)| *k == key) {
                    let value = if *text == "true" || *text == "false" {
                        Value::Bool(*text == "true")
                    } else if let Ok(n) = text.parse::<f64>() {
                        serde_json::Number::from_f64(n).map_or(Value::Null, Value::Number)
                    } else {
                        Value::String((*text).into())
                    };
                    setting.from_json(&value);
                }
            }
        }
        self.changed();
        true
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::framework::module::Category;
    use crate::framework::setting as s;
    use std::collections::HashMap;

    #[derive(Default)]
    pub(crate) struct Files(pub HashMap<String, String>);
    impl Storage for Files {
        fn read(&self, path: &str) -> Option<String> {
            self.0.get(path).cloned()
        }
        fn write(&mut self, path: &str, text: &str) {
            self.0.insert(path.into(), text.into());
        }
        fn delete(&mut self, path: &str) {
            self.0.remove(path);
        }
    }

    fn system() -> ModuleSystem {
        ModuleSystem::new(vec![
            Module::new("zoom", "Zoom", "Zooms.", Category::Visual)
                .setting(s::bool("smooth", "Smooth", true)),
            Module::new("cps", "CPS", "Clicks.", Category::Hud),
        ])
    }

    #[test]
    fn toggle_keys_switch_modules_and_save_at_once() {
        let mut files = Files::default();
        let mut sys = system();
        sys.load(&files);
        sys.module_mut("zoom").unwrap().settings.get_mut("toggle").unwrap().set(s::Val::Key(Key::Keyboard(6)));
        assert!(sys.key_pressed(Key::Keyboard(6), &mut files));
        assert!(sys.on("zoom"));
        assert!(files.0["profiles/Default.json"].contains("keyboard:6"));
        assert!(!sys.key_pressed(Key::None, &mut files));
    }

    #[test]
    fn profiles_keep_their_own_modules() {
        let mut files = Files::default();
        let mut sys = system();
        sys.load(&files);
        sys.set_enabled("cps", true);
        assert!(sys.create_profile("PvP", &mut files));
        sys.set_enabled("cps", false);
        sys.set_enabled("zoom", true);
        assert!(sys.switch_profile("default", &mut files));
        assert!(sys.on("cps") && !sys.on("zoom"));
        assert!(sys.switch_profile("PvP", &mut files));
        assert!(!sys.on("cps") && sys.on("zoom"));
        assert!(sys.delete_profile("PvP", &mut files));
        assert_eq!(sys.active_profile(), "Default");
        assert!(!sys.delete_profile("Default", &mut files));
        let mut reloaded = system();
        reloaded.load(&files);
        assert!(reloaded.on("cps"));
    }

    #[test]
    fn saves_a_change_after_it_settles() {
        let mut files = Files::default();
        let mut sys = system();
        sys.set_enabled("cps", true);
        for _ in 0..SAVE_DELAY - 1 {
            sys.tick(&mut files);
        }
        assert!(files.0.is_empty());
        sys.tick(&mut files);
        assert!(files.0.contains_key("profiles/Default.json"));
    }
}
