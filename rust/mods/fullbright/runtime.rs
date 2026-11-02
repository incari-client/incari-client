// Incari's Fullbright for the Rust build (ADR 0068, 0069), as features/fullbright does it: caves
// and nights as light as day, for you only, with the Brightness option untouched. Its settings are
// the module's in `incari_core::modules`. Horizon lights the world fully or not at all (its own
// Fullbright extra), so any Brightness lights it fully. No game types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "fullbright";

/// Whether to light the world fully at Brightness `level` (percent): Horizon has no half way.
pub fn lit(level: i32) -> bool {
    level > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lights_at_every_brightness_the_menu_offers() {
        assert!(lit(100) && lit(10));
        assert!(!lit(0));
    }
}
