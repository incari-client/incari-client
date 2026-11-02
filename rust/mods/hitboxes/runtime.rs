// Incari's Hitboxes for the Rust build (ADR 0068, 0069), as features/hitboxes does it: the box each
// player or mob is hit by, as F3+B draws it, in your colour, opacity and width; the one your
// crosshair is on in its own colour; the eye height and an arrow the way each looks. Only you see
// them. Its settings are the module's in `incari_core::modules`. Horizon's lines are 0.5 to 4
// pixels wide, within 128 blocks, at most 1024; its entity list is mobs and players only (no
// items or arrows), and doesn't hold your own. No game types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "hitboxes";

/// F3+B's eye line and look arrow: red, and blue 2 blocks out, at the boxes' opacity.
pub const EYE: u32 = 0xFF_0000;
pub const LOOK: u32 = 0x00_00FF;
pub const LOOK_LENGTH: f64 = 2.0;
/// The arrow head's sides, in blocks.
pub const HEAD: f64 = 0.25;
/// Horizon keeps lines within 128 blocks.
pub const RANGE: f64 = 128.0;
/// The widest line Horizon draws.
pub const MAX_WIDTH: f32 = 4.0;

/// `rgb` at `percent` opacity, as the Java module's `withOpacity`.
pub fn argb(rgb: u32, percent: i32) -> u32 {
    ((percent.clamp(0, 100) as f32 * 255.0 / 100.0).round() as u32) << 24 | (rgb & 0xFF_FFFF)
}

/// `rgb` at `color`'s opacity: the eye line and look arrow.
pub fn with_alpha_of(color: u32, rgb: u32) -> u32 {
    color & 0xFF00_0000 | rgb
}

/// The lines' width in pixels: the "width" setting, as far as Horizon goes.
pub fn width(setting: f32) -> f32 {
    setting.clamp(0.5, MAX_WIDTH)
}

/// Whether an entity gets a box under the "show" setting: "players" only, or every one Horizon
/// lists ("living", "all": its list has mobs and players only).
pub fn shown(show: &str, player: bool) -> bool {
    show != "players" || player
}

/// A box's twelve edges.
pub fn edges(min: [f64; 3], max: [f64; 3]) -> [([f64; 3], [f64; 3]); 12] {
    let c = |x: bool, y: bool, z: bool| {
        [
            if x { max[0] } else { min[0] },
            if y { max[1] } else { min[1] },
            if z { max[2] } else { min[2] },
        ]
    };
    let mut out = [([0.0; 3], [0.0; 3]); 12];
    let mut n = 0;
    for a in [false, true] {
        for b in [false, true] {
            out[n] = (c(false, a, b), c(true, a, b));
            out[n + 1] = (c(a, false, b), c(a, true, b));
            out[n + 2] = (c(a, b, false), c(a, b, true));
            n += 3;
        }
    }
    out
}

/// The eye height's rectangle: four edges round the box at `eye_y`.
pub fn eye_edges(min: [f64; 3], max: [f64; 3], eye_y: f64) -> [([f64; 3], [f64; 3]); 4] {
    let p = |x: f64, z: f64| [x, eye_y, z];
    [
        (p(min[0], min[2]), p(max[0], min[2])),
        (p(max[0], min[2]), p(max[0], max[2])),
        (p(max[0], max[2]), p(min[0], max[2])),
        (p(min[0], max[2]), p(min[0], min[2])),
    ]
}

/// The look arrow from `from`, by Minecraft's yaw and pitch: its shaft and the two sides of its
/// head, swept back from the tip across the way it looks.
pub fn look_arrow(from: [f64; 3], yaw: f32, pitch: f32) -> [([f64; 3], [f64; 3]); 3] {
    let (yaw, pitch) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    let dir = [
        -yaw.sin() * pitch.cos(),
        -pitch.sin(),
        yaw.cos() * pitch.cos(),
    ];
    let tip: [f64; 3] = std::array::from_fn(|i| from[i] + dir[i] * LOOK_LENGTH);
    // Across the look, level: to the look's right.
    let side = [-yaw.cos(), 0.0, -yaw.sin()];
    let head = |sign: f64| -> [f64; 3] {
        std::array::from_fn(|i| tip[i] - dir[i] * HEAD + side[i] * HEAD * 0.5 * sign)
    };
    [(from, tip), (tip, head(1.0)), (tip, head(-1.0))]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_every_edge_once() {
        let e = edges([0.0; 3], [1.0; 3]);
        for (a, b) in e {
            let diff = (0..3).filter(|&i| a[i] != b[i]).count();
            assert_eq!(diff, 1, "{a:?} {b:?}");
        }
        let mut sorted: Vec<_> = e.iter().map(|(a, b)| format!("{a:?}{b:?}")).collect();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 12);
        assert_eq!(
            eye_edges([0.0; 3], [1.0; 3], 0.5)[0],
            ([0.0, 0.5, 0.0], [1.0, 0.5, 0.0])
        );
    }

    #[test]
    fn looks_south_at_yaw_zero() {
        let [shaft, left, right] = look_arrow([0.0; 3], 0.0, 0.0);
        assert!((shaft.1[2] - 2.0).abs() < 1e-9 && shaft.1[0].abs() < 1e-9);
        assert!(left.1[2] < 2.0 && right.1[2] < 2.0 && (left.1[0] + right.1[0]).abs() < 1e-9);
    }

    #[test]
    fn colors_as_the_java_module_does() {
        assert_eq!(argb(0xFFFF_FFFF, 100), 0xFFFF_FFFF);
        assert_eq!(argb(0xFFFF_5555, 50), 0x80FF_5555);
        assert_eq!(with_alpha_of(0x80FF_FFFF, EYE), 0x80FF_0000);
        assert_eq!(width(2.0), 2.0);
        assert_eq!(width(6.0), MAX_WIDTH);
        assert!(shown("living", false) && shown("all", false) && !shown("players", false));
    }
}
