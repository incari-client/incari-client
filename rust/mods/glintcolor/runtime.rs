// Incari's Enchant Glint for the Rust build (ADR 0068, 0069), as features/glintcolor does it: the
// enchantment shimmer on items and armour in your colour; its speed and strength stay the game's
// options. Only you see it. Its settings are the module's in `incari_core::modules`. Horizon tints
// the game's purple glint by the colour (white is the game's own), where the Java module repaints
// the glint's brightness in it, so a colour shows a little purpler here. No game types:
// rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "glintcolor";

/// `[r, g, b]`, each 0 to 1, from the "color" setting.
pub fn tint(rgb: u32) -> [f32; 3] {
    let [_, r, g, b] = rgb.to_be_bytes();
    [r, g, b].map(|c| f32::from(c) / 255.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tints_in_the_chosen_color() {
        assert_eq!(tint(0xFF7F_C8FF), [127.0 / 255.0, 200.0 / 255.0, 1.0]);
        assert_eq!(tint(0xFFFF_FFFF), [1.0; 3]);
    }
}
