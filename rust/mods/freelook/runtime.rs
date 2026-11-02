// Incari's Freelook for the Rust build (ADR 0068, 0069), as features/freelook does it: hold (or
// toggle) the key to look around without turning your player; letting go puts the view back.
// Horizon's camera hooks turn the view only: the server, movement and aiming keep the player's.
// Its settings are the module's in `incari_core::modules` (the Java ids), in the module menu.
// No game types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "freelook";

/// Horizon's view (`native_set_camera_type`: 0 first person, 1 behind, 2 in front) for the `view`
/// setting's key.
pub fn camera_type(view: &str) -> u8 {
    match view {
        "front" => 2,
        "first" => 0,
        _ => 1,
    }
}

/// Degrees a mouse unit turns (`Entity.turn`).
pub const DEGREES: f64 = 0.15;

/// What a tick asks of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    None,
    /// Freelook started: set the view to this (Horizon's camera type), if it isn't already.
    Start(u8),
    /// Freelook stopped: put the view back to this.
    Stop(u8),
}

/// Freelook's state, as the Java module keeps it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Freelook {
    /// Flipped by each press (the key's `onPress`).
    pub toggled: bool,
    /// The key is held (it went down with no screen open and hasn't come up).
    pub held: bool,
    pub active: bool,
    /// The camera's angles are still to be taken from the player's on the next frame.
    pub fresh: bool,
    pub yaw: f32,
    pub pitch: f32,
    /// The view to go back to.
    pub before: u8,
}

impl Freelook {
    pub const OFF: Self = Self {
        toggled: false,
        held: false,
        active: false,
        fresh: false,
        yaw: 0.0,
        pitch: 0.0,
        before: 0,
    };

    /// The module's key went down (`pressed`) or up; `playing` with no screen open. Binds fire
    /// only with no screen open, as the game's do.
    pub fn key(&mut self, pressed: bool, playing: bool) {
        if !pressed {
            self.held = false;
        } else if playing {
            self.held = true;
            self.toggled = !self.toggled;
        }
    }

    /// Once a tick. `mode` is the `mode` setting's key, None while the module is off; `player`
    /// whether there's a player; `view` the camera Freelook wants and `now` the camera now.
    pub fn tick(
        &mut self,
        mode: Option<&str>,
        playing: bool,
        player: bool,
        view: u8,
        now: u8,
    ) -> Change {
        let want = match mode {
            None => false,
            Some(mode) => {
                player
                    && if mode == "hold" {
                        self.held && playing
                    } else {
                        self.toggled
                    }
            }
        };
        let change = if want == self.active {
            Change::None
        } else if want {
            self.active = true;
            self.fresh = true;
            self.before = now;
            if view == now {
                Change::None
            } else {
                Change::Start(view)
            }
        } else {
            self.active = false;
            self.toggled = false;
            if now == self.before {
                Change::None
            } else {
                Change::Stop(self.before)
            }
        };
        if mode.is_none() {
            // Off: as the Java module's onDisable.
            self.toggled = false;
        }
        change
    }

    /// The mouse turned `dx`, `dy` (positive right and down): true when Freelook took it.
    pub fn turn(&mut self, dx: f64, dy: f64, invert: bool) -> bool {
        if !self.active {
            return false;
        }
        let dy = if invert { -dy } else { dy };
        self.yaw += (dx * DEGREES) as f32;
        self.pitch = (self.pitch + (dy * DEGREES) as f32).clamp(-90.0, 90.0);
        true
    }

    /// The camera's rotation this frame, arriving as the player's.
    pub fn camera(&mut self, yaw: &mut f32, pitch: &mut f32) {
        if !self.active {
            return;
        }
        if self.fresh {
            (self.yaw, self.pitch) = (*yaw, *pitch);
            self.fresh = false;
        }
        (*yaw, *pitch) = (self.yaw, self.pitch);
    }
}

// The window's thread only: keys, ticks, the mouse and the camera all run there.
thread_local! {
    static STATE: std::cell::Cell<Freelook> = const { std::cell::Cell::new(Freelook::OFF) };
}

/// Runs `f` on Freelook's state.
pub fn with<R>(f: impl FnOnce(&mut Freelook) -> R) -> R {
    STATE.with(|cell| {
        let mut state = cell.get();
        let r = f(&mut state);
        cell.set(state);
        r
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_the_key_to_look_from_behind_and_puts_the_view_back() {
        let mut f = Freelook::OFF;
        f.key(true, true);
        assert_eq!(
            f.tick(Some("hold"), true, true, camera_type("back"), 0),
            Change::Start(1)
        );
        let (mut yaw, mut pitch) = (10.0, 80.0);
        f.camera(&mut yaw, &mut pitch);
        assert!(f.turn(100.0, 100.0, false));
        f.camera(&mut yaw, &mut pitch);
        assert_eq!((yaw, pitch), (25.0, 90.0));
        assert!(f.turn(0.0, 100.0, true));
        assert_eq!(f.pitch, 75.0);
        f.key(false, true);
        assert_eq!(f.tick(Some("hold"), true, true, 1, 1), Change::Stop(0));
        assert!(!f.turn(1.0, 1.0, false));
    }

    #[test]
    fn toggles_and_stops_when_turned_off() {
        let mut f = Freelook::OFF;
        f.key(true, false);
        assert!(!f.toggled, "a press on a screen does nothing");
        f.key(true, true);
        f.key(false, true);
        assert_eq!(f.tick(Some("toggle"), true, true, 0, 0), Change::None);
        assert!(f.active);
        assert_eq!(f.tick(None, true, true, 0, 0), Change::None);
        assert!(!f.active && !f.toggled);
        assert_eq!(camera_type("front"), 2);
    }

    #[test]
    fn keeps_its_state() {
        with(|f| f.held = true);
        assert!(with(|f| f.held));
        with(|f| *f = Freelook::OFF);
    }
}
