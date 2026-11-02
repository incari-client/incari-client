// Incari's Hit Color for the Rust build (ADR 0068, 0069), as features/hitcolor does it: hurt mobs
// and players flash in your colour, not red, at your strength (the game's red is 30%). Only you see
// it. Its settings are the module's in `incari_core::modules`. No game types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "hitcolor";

/// `[r, g, b, strength]`, each 0 to 1, from the "color" and "strength" (percent) settings.
pub fn tint(rgb: u32, strength: i32) -> [f32; 4] {
    let [_, r, g, b] = rgb.to_be_bytes();
    [
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
        strength.clamp(0, 100) as f32 / 100.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flashes_blue_at_the_games_strength_by_default() {
        assert_eq!(
            tint(0xFF5A_B8FF, 30),
            [90.0 / 255.0, 184.0 / 255.0, 1.0, 0.3]
        );
        assert_eq!(tint(0xFF00_0000, 150)[3], 1.0);
    }
}
