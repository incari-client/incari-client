// Incari's Block Outline for the Rust build (ADR 0068, 0069), as features/blockoutline does it: the
// block you look at outlined in your colour, opacity and width, for you only. Its settings are the
// module's in `incari_core::modules`. Horizon draws a mod's outline (a "rim") along the block's
// selection shape, depth-tested, 0.5 to 4 pixels wide, and has no fill, so widths past 4 draw at 4
// and Fill does nothing yet. No game types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "blockoutline";

/// The widest rim Horizon draws.
pub const MAX_WIDTH: f32 = 4.0;

/// `rgb` (the "color" setting) at `percent` opacity, never fully clear, as the Java module's.
pub fn argb(rgb: u32, percent: i32) -> u32 {
    let alpha = ((percent.clamp(0, 100) as f32 * 255.0 / 100.0).round() as u32).max(1);
    alpha << 24 | (rgb & 0xFF_FFFF)
}

/// The rim's width in pixels: the "width" setting, as far as Horizon goes.
pub fn width(setting: f32) -> f32 {
    setting.clamp(0.5, MAX_WIDTH)
}

// The block the game would outline, kept from its highlight hook (where the game's own outline is
// skipped) for the next frame's visuals hook, which comes first in a frame: a frame late, and
// gone a frame after the player looks away.
thread_local! {
    static TARGET: std::cell::Cell<Option<[i32; 3]>> = const { std::cell::Cell::new(None) };
}

pub fn aim(pos: [i32; 3]) {
    TARGET.with(|t| t.set(Some(pos)));
}

pub fn take() -> Option<[i32; 3]> {
    TARGET.with(std::cell::Cell::take)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_as_the_java_module_does() {
        assert_eq!(argb(0xFFA9_DCFF, 90), 0xE6A9_DCFF);
        assert_eq!(argb(0x00_0000, 100), 0xFF00_0000);
        assert_eq!(argb(0xFF_FFFF, 0) >> 24, 1);
        assert_eq!(width(2.5), 2.5);
        assert_eq!(width(6.0), MAX_WIDTH);
    }

    #[test]
    fn draws_each_aim_once() {
        aim([1, 2, 3]);
        assert_eq!(take(), Some([1, 2, 3]));
        assert_eq!(take(), None);
    }
}
