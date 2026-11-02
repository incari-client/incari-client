// Incari's Low Fire for the Rust build (ADR 0068, 0069), as features/lowfire does it: the
// first-person fire overlay drawn lower while you burn, so it hides less. Its settings are the
// module's in `incari_core::modules`. No game types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "lowfire";

/// How far down, in flame heights, for the `lower` setting (a percent).
pub fn lower(percent: i32) -> f32 {
    percent.clamp(0, 25) as f32 / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowers_by_the_percent() {
        assert_eq!(lower(10), 0.1);
        assert_eq!(lower(25), 0.25);
    }
}
