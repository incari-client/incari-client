// Incari's Zoom for the Rust build (ADR 0068, 0069), as features/zoom does it: hold (or toggle) the
// key to zoom in, OptiFine's way. Horizon's zoom hook narrows the world's field of view and leaves
// the hand and HUD alone, eases in and out through the field of view as the spyglass does, and
// scales mouse turning. It has no cinematic camera for a mod to turn on, so Smooth turning turns
// slower, as Slower does. Its settings are the module's in `incari_core::modules` (the Java ids).
// No game types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "zoom";

/// The key's state, as the Java module keeps it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Keys {
    /// Flipped by each press (the key's `onPress`).
    pub toggled: bool,
    /// The key is held (it went down with no screen open and hasn't come up).
    pub held: bool,
}

impl Keys {
    /// The module's key went down (`pressed`) or up; `playing` with no screen open.
    pub fn key(&mut self, pressed: bool, playing: bool) {
        if !pressed {
            self.held = false;
        } else if playing {
            self.held = true;
            self.toggled = !self.toggled;
        }
    }

    /// Whether the view's zoomed, for the `mode` setting's key (None while the module is off).
    pub fn active(&mut self, mode: Option<&str>) -> bool {
        let active = match mode {
            None => false,
            Some("hold") => self.held,
            Some(_) => self.toggled,
        };
        if !active {
            self.toggled = false;
        }
        active
    }
}

/// The zoom asked of Horizon for `amount` (the `amount` setting, times closer), `turning` (its
/// key) and `ease`: the factor, whether it eases, and the scale of mouse turning.
pub fn zoom(amount: f32, turning: &str, ease: bool) -> (f32, bool, f32) {
    let factor = amount.clamp(1.0, 10.0);
    let turn = if turning == "usual" {
        1.0
    } else {
        1.0 / factor
    };
    (factor, ease, turn)
}

// The window's thread only: keys and the zoom hook both run there.
thread_local! {
    static KEYS: std::cell::Cell<Keys> = const {
        std::cell::Cell::new(Keys { toggled: false, held: false })
    };
}

/// Runs `f` on the key's state.
pub fn with<R>(f: impl FnOnce(&mut Keys) -> R) -> R {
    KEYS.with(|cell| {
        let mut keys = cell.get();
        let r = f(&mut keys);
        cell.set(keys);
        r
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_or_toggles() {
        let mut k = Keys::default();
        k.key(true, false);
        assert!(!k.active(Some("hold")), "a press on a screen doesn't zoom");
        k.key(true, true);
        assert!(k.active(Some("hold")));
        k.key(false, false);
        assert!(!k.active(Some("hold")), "letting go anywhere ends it");
        k.key(true, true);
        k.key(false, true);
        assert!(k.active(Some("toggle")));
        assert!(!k.active(None));
        assert!(
            !k.active(Some("toggle")),
            "turning it off forgets the toggle"
        );
    }

    #[test]
    fn zooms_four_times_and_turns_slower_by_default() {
        assert_eq!(zoom(4.0, "smooth", false), (4.0, false, 0.25));
        assert_eq!(zoom(10.0, "usual", true), (10.0, true, 1.0));
        assert_eq!(zoom(2.5, "slower", false), (2.5, false, 0.4));
    }

    #[test]
    fn keeps_the_key() {
        with(|k| k.held = true);
        assert!(with(|k| k.held));
        with(|k| *k = Keys::default());
    }
}
