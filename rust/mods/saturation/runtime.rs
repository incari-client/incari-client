// Incari's Saturation for the Rust build (ADR 0068, 0069), as features/saturation does it:
// saturation, the hidden food value that keeps hunger from dropping, as outlines on the hunger
// bar's shanks, a shank's worth each, from the right. When the outline is gone, hunger starts going
// down. Its settings are the module's in `incari_core::modules` (the Java ids). No game types:
// rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "saturation";

/// How much of a shank is outlined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outline {
    Full,
    Half,
}

/// Shank `index`'s outline (0 the rightmost), as the Java client counts it: saturation in
/// half-shank points, rounded, any at all showing.
pub fn outline(index: usize, saturation: f32) -> Option<Outline> {
    if saturation <= 0.0 {
        return None;
    }
    let points = ((saturation + 0.5).floor() as usize).max(1);
    let first = index * 2 + 1;
    match first.cmp(&points) {
        std::cmp::Ordering::Less => Some(Outline::Full),
        std::cmp::Ordering::Equal => Some(Outline::Half),
        std::cmp::Ordering::Greater => None,
    }
}

/// Shank `i`'s top left (0 the rightmost), for a GUI `width` x `height` HUD: `Hud`'s food row, 39
/// above the bottom and right of the hotbar's middle, 8 apart.
pub fn shank(i: usize, width: f32, height: f32) -> [f32; 2] {
    let right = (width / 2.0).floor() + 91.0;
    [right - i as f32 * 8.0 - 9.0, height - 39.0]
}

/// The outline of a shank at `at` as rectangles: the 9 by 9 shank's edge, or its left half's.
/// Horizon gives no sprite pixels to trace, so it's the shank's box, not its shape.
pub fn edges(at: [f32; 2], outline: Outline) -> [[f32; 4]; 4] {
    let [x, y] = at;
    let w = match outline {
        Outline::Full => 9.0,
        Outline::Half => 5.0,
    };
    [
        [x, y, w, 1.0],
        [x, y + 8.0, w, 1.0],
        [x, y + 1.0, 1.0, 7.0],
        [x + w - 1.0, y + 1.0, 1.0, 7.0],
    ]
}

// The food row as the health hook last saw it drawn (saturation, GUI size), for the HUD hook,
// which comes after it in a frame. Taken, so it's gone when the row isn't drawn (creative, a
// ridden horse's hearts in its place).
thread_local! {
    static ROW: std::cell::Cell<Option<(f32, f32, f32)>> = const { std::cell::Cell::new(None) };
}

pub fn drawn(saturation: f32, width: f32, height: f32) {
    ROW.with(|r| r.set(Some((saturation, width, height))));
}

pub fn take() -> Option<(f32, f32, f32)> {
    ROW.with(std::cell::Cell::take)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outlines_as_the_java_client_counts() {
        assert_eq!(outline(0, 0.0), None);
        // Any saturation at all shows half a shank.
        assert_eq!(outline(0, 0.2), Some(Outline::Half));
        assert_eq!(outline(0, 2.0), Some(Outline::Full));
        assert_eq!(outline(1, 2.0), None);
        assert_eq!(
            [
                outline(0, 5.0),
                outline(1, 5.0),
                outline(2, 5.0),
                outline(3, 5.0)
            ],
            [
                Some(Outline::Full),
                Some(Outline::Full),
                Some(Outline::Half),
                None
            ]
        );
        assert_eq!(outline(9, 20.0), Some(Outline::Full));
    }

    #[test]
    fn finds_the_food_row() {
        assert_eq!(shank(0, 320.0, 240.0), [242.0, 201.0]);
        assert_eq!(shank(9, 320.0, 240.0), [170.0, 201.0]);
        assert_eq!(edges([0.0, 0.0], Outline::Half)[3], [4.0, 1.0, 1.0, 7.0]);
        drawn(4.0, 320.0, 240.0);
        assert_eq!(take(), Some((4.0, 320.0, 240.0)));
        assert_eq!(take(), None);
    }
}
