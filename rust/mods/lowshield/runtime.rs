// Incari's Low Shield for the Rust build (ADR 0068, 0069), as features/lowshield does it: your
// shield drawn lower in first person, so it hides less; blocking is unchanged. Horizon's hand pose
// hook moves the drawn item only. Its settings are the module's in `incari_core::modules`.
// No game types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "lowshield";

pub const SHIELD: &str = "minecraft:shield";

/// The hand's units a percent moves the shield, as the Java module measured them: 100% is the
/// shield's height on screen.
pub const UNITS_PER_PERCENT: f32 = 0.0018;

/// How far down to move `item`, held up (`using`) or not, for the `lower` (a percent) and
/// `while-blocking` settings; 0 for anything but a shield.
pub fn drop(item: &str, using: bool, lower: i32, while_blocking: bool) -> f32 {
    if item != SHIELD || (using && !while_blocking) {
        return 0.0;
    }
    lower.clamp(0, 90) as f32 * UNITS_PER_PERCENT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowers_only_shields() {
        assert!((drop(SHIELD, true, 35, true) - 0.063).abs() < 1e-6);
        assert_eq!(drop("minecraft:stone_sword", false, 35, true), 0.0);
        assert_eq!(drop(SHIELD, true, 35, false), 0.0);
        assert!(drop(SHIELD, false, 35, false) > 0.0);
    }
}
