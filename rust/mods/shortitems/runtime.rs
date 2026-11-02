// Incari's Short Items for the Rust build (ADR 0068, 0069), as features/shortitems does it: held
// items drawn smaller in first person, in either hand, as 1.8 PvP packs drew swords; reach and hits
// are unchanged. Its id and settings are the module's in `incari_core::modules`: still Short
// Swords' `shortswords`, so profiles keep it. No game types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "shortswords";

/// The scale to draw an item at for the `size` (a percent) and `while-blocking` settings; 1
/// leaves it be.
pub fn scale(size: i32, using: bool, while_using: bool) -> f32 {
    if using && !while_using {
        return 1.0;
    }
    size.clamp(40, 100) as f32 / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_at_the_size() {
        assert_eq!(scale(70, false, true), 0.7);
        assert_eq!(scale(70, true, true), 0.7);
        assert_eq!(scale(70, true, false), 1.0);
    }
}
