// Incari's Toggle Sprint for the Rust build (ADR 0068, 0069), as features/togglesprint does it:
// latch sprint (or always sprint) and optionally sneak, with a HUD line saying what's active.
// Whether the player can sprint is still the game's call: the mod only says the key is held. Its
// settings and HUD element are the module's in `incari_core::modules` (the Java ids and choice
// keys). No game types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "togglesprint";

pub const SPRINTING_TOGGLED: &str = "[Sprinting (Toggled)]";
pub const SPRINTING_ALWAYS: &str = "[Sprinting (Always)]";
pub const SPRINTING_HELD: &str = "[Sprinting (Key Held)]";
pub const SNEAKING_TOGGLED: &str = "[Sneaking (Toggled)]";
pub const SNEAKING_HELD: &str = "[Sneaking (Key Held)]";

/// The keys as the player holds them, the latches the mod keeps, and the modes (the `sprint` and
/// `sneak` settings' keys) they were latched in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Keys {
    pub sprint_held: bool,
    pub sneak_held: bool,
    pub sprint_latched: bool,
    pub sneak_latched: bool,
    pub modes: Option<(&'static str, &'static str)>,
}

impl Keys {
    pub const NONE: Keys = Keys {
        sprint_held: false,
        sneak_held: false,
        sprint_latched: false,
        sneak_latched: false,
        modes: None,
    };

    /// A tick's keys as the game reads them (`sprint`, `sneak` held), in modes `sprint_mode`
    /// (`hold`, `toggle`, `always`) and `sneak_mode` (`hold`, `toggle`): a press flips a latch
    /// when toggling, and a change of mode lets go of both. Returns whether the game should take
    /// sprint and sneak as held.
    pub fn tick(
        &mut self,
        sprint: bool,
        sneak: bool,
        sprint_mode: &'static str,
        sneak_mode: &'static str,
    ) -> (bool, bool) {
        if self.modes != Some((sprint_mode, sneak_mode)) {
            self.unlatch();
            self.modes = Some((sprint_mode, sneak_mode));
        }
        if sprint_mode != "toggle" {
            self.sprint_latched = false;
        } else if sprint && !self.sprint_held {
            self.sprint_latched = !self.sprint_latched;
        }
        if sneak_mode != "toggle" {
            self.sneak_latched = false;
        } else if sneak && !self.sneak_held {
            self.sneak_latched = !self.sneak_latched;
        }
        self.sprint_held = sprint;
        self.sneak_held = sneak;
        (self.sprinting(sprint_mode), self.sneaking(sneak_mode))
    }

    /// Lets go of the latches: death, leaving the world, or the module turned off.
    pub fn unlatch(&mut self) {
        self.sprint_latched = false;
        self.sneak_latched = false;
    }

    /// Whether the game should take sprint as held.
    pub fn sprinting(&self, mode: &str) -> bool {
        match mode {
            "always" => true,
            "toggle" => self.sprint_latched || self.sprint_held,
            _ => self.sprint_held,
        }
    }

    pub fn sneaking(&self, mode: &str) -> bool {
        self.sneak_held || (mode == "toggle" && self.sneak_latched)
    }

    /// The HUD line, or none. Sneaking first: it stops sprinting, so it's what the player is doing.
    pub fn status(&self, sprint_mode: &str, sneak_mode: &str) -> Option<&'static str> {
        if sneak_mode == "toggle" && self.sneak_latched {
            Some(SNEAKING_TOGGLED)
        } else if self.sneak_held {
            Some(SNEAKING_HELD)
        } else if sprint_mode == "always" {
            Some(SPRINTING_ALWAYS)
        } else if sprint_mode == "toggle" && self.sprint_latched {
            Some(SPRINTING_TOGGLED)
        } else if self.sprint_held {
            Some(SPRINTING_HELD)
        } else {
            None
        }
    }
}

// The window's thread only: keys, ticks and the HUD all run there.
thread_local! {
    static HELD: std::cell::Cell<Keys> = const { std::cell::Cell::new(Keys::NONE) };
}

pub fn keys() -> Keys {
    HELD.with(std::cell::Cell::get)
}

pub fn update<R>(f: impl FnOnce(&mut Keys) -> R) -> R {
    HELD.with(|k| {
        let mut keys = k.get();
        let r = f(&mut keys);
        k.set(keys);
        r
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggles_sprint_and_holds_sneak_by_default() {
        let (sprint, sneak) = ("toggle", "hold");
        let mut k = Keys::NONE;
        k.tick(true, false, sprint, sneak);
        assert_eq!(k.tick(false, false, sprint, sneak), (true, false));
        assert_eq!(k.status(sprint, sneak), Some(SPRINTING_TOGGLED));
        k.tick(false, true, sprint, sneak);
        assert_eq!(k.status(sprint, sneak), Some(SNEAKING_HELD));
        assert_eq!(k.tick(false, false, sprint, sneak), (true, false));
        k.tick(true, false, sprint, sneak);
        assert_eq!(k.tick(false, false, sprint, sneak), (false, false));
        assert_eq!(k.status(sprint, sneak), None);
    }

    #[test]
    fn always_sprints_and_latches_sneak() {
        let mut k = Keys::NONE;
        assert_eq!(k.tick(false, false, "always", "toggle"), (true, false));
        assert_eq!(k.status("always", "hold"), Some(SPRINTING_ALWAYS));
        k.tick(false, true, "always", "toggle");
        assert_eq!(k.tick(false, false, "always", "toggle"), (true, true));
        assert_eq!(k.status("always", "toggle"), Some(SNEAKING_TOGGLED));
        k.unlatch();
        assert!(!k.sneaking("toggle"));
    }

    #[test]
    fn a_held_key_and_a_new_mode_dont_keep_the_latch() {
        let mut k = Keys::NONE;
        k.tick(true, false, "toggle", "hold");
        k.tick(true, false, "toggle", "hold");
        assert!(k.sprint_latched, "held over ticks: one press");
        k.tick(false, false, "hold", "hold");
        assert!(!k.sprint_latched, "changing the mode lets go");
        update(|keys| keys.tick(true, false, "toggle", "hold"));
        assert!(keys().sprint_latched);
    }
}
