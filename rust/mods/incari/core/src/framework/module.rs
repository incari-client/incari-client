//! A client module (framework's `Module`): its id, name, description, category, settings (the
//! toggle key first) and HUD elements. What it does lives with the game side's hooks, which read
//! its settings here; the module menu, the HUD editor and the profiles work from this alone.
use super::hud::HudElement;
use super::setting::{self as s, Settings};
use crate::input::Key;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Hud,
    Combat,
    Movement,
    Visual,
    Utility,
}

pub const CATEGORIES: [Category; 5] = [
    Category::Hud,
    Category::Combat,
    Category::Movement,
    Category::Visual,
    Category::Utility,
];

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Category::Hud => "HUD",
            Category::Combat => "Combat",
            Category::Movement => "Movement",
            Category::Visual => "Visual",
            Category::Utility => "Utility",
        }
    }
}

/// A button a module adds to the title screen's row (MenuButton): an id, a label and an icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuButton {
    pub id: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub category: Category,
    pub settings: Settings,
    pub hud: Vec<HudElement>,
    pub enabled: bool,
    pub always_on: bool,
    /// A built-in resource pack it turns on and off with it (Old Textures).
    pub pack: Option<&'static str>,
    pub menu_buttons: Vec<MenuButton>,
}

impl Module {
    pub fn new(
        id: &'static str,
        name: &'static str,
        description: &'static str,
        category: Category,
    ) -> Self {
        let mut settings = Settings::default();
        settings.add(s::key("toggle", "Toggle key", Key::None).described("Turns the module on and off."));
        Self {
            id,
            name,
            description,
            category,
            settings,
            hud: Vec::new(),
            enabled: false,
            always_on: false,
            pack: None,
            menu_buttons: Vec::new(),
        }
    }

    pub fn setting(mut self, setting: s::Setting) -> Self {
        self.settings.add(setting);
        self
    }

    pub fn element(mut self, element: HudElement) -> Self {
        debug_assert!(self.hud.iter().all(|e| e.id != element.id));
        self.hud.push(element);
        self
    }

    pub fn always_on(mut self) -> Self {
        self.always_on = true;
        self.enabled = true;
        if let Some(toggle) = self.settings.get_mut("toggle") {
            toggle.shown = s::Shown::Never;
        }
        self
    }

    pub fn follows_pack(mut self, pack: &'static str) -> Self {
        self.pack = Some(pack);
        self
    }

    pub fn menu_button(mut self, id: &'static str, label: &'static str, icon: &'static str) -> Self {
        self.menu_buttons.push(MenuButton { id, label, icon });
        self
    }

    pub fn element_index(&self, id: &str) -> Option<usize> {
        self.hud.iter().position(|e| e.id == id)
    }

    /// Whether the menu's settings page has anything to show (more than the toggle key).
    pub fn has_settings(&self) -> bool {
        self.settings.any_shown() || self.hud.iter().any(|e| e.settings.any_shown())
    }
}
