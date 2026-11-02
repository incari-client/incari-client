// Incari's No Hurt Cam for the Rust build (ADR 0068, 0069), as features/nohurtcam does it: the
// camera tilts by the module's Shake amount, 0% not at all, when you're hurt. The Java module holds
// the game's Damage Tilt option at that value; here Horizon's hurt tilt hook sets it for the frame,
// so the option itself is untouched. Its settings are the module's in `incari_core::modules`.
// No game types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "nohurtcam";

/// The Damage Tilt in effect for the `shake` setting (a percent): the option the Java module holds.
pub fn tilt(shake: i32) -> f64 {
    f64::from(shake.clamp(0, 100)) / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_the_tilt_at_the_shake_amount() {
        assert_eq!(tilt(0), 0.0);
        assert_eq!(tilt(50), 0.5);
        assert_eq!(tilt(100), 1.0);
    }
}
