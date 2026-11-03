// Incari's Minimap for the Rust build (ADR 0068, 0069), as features/minimap's MapElement draws it:
// the live map in a round or square frame where the player placed it in the HUD editor, turning
// with them or north up, the arrow in the middle, N, E, S and W on the rim and the coordinates on a
// tag below. Horizon composes the terrain (vanilla's map colours, height shading, a floor slice
// under roofs and in the Nether) into its minimap sprite; the element draws that sprite itself, so
// Horizon is asked for the map only while it composes, not while it would draw it in a corner.
// Its settings and HUD element are the module's in `incari_core::modules`. No game types:
// rust/lib.rs tests it.
use std::cell::{Cell, RefCell};

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "minimap";
/// Its HUD element.
pub const ELEMENT: &str = "map";

/// How many blocks the map's width shows at each zoom level (1 to 7), out to in.
pub const ZOOMS: [i32; 7] = [256, 192, 128, 96, 64, 48, 32];

/// The map's size, frame and all, in GUI units at scale 1.
pub const SIZE: f32 = 96.0;
/// The frame's width around the terrain.
pub const FRAME: f32 = 2.0;
/// The player's arrow.
pub const ARROW: f32 = 10.0;
/// The compass letters' badges' radius.
pub const BADGE: f32 = 5.5;
/// The gap between the map and the coordinates below it.
pub const COORDINATES_GAP: f32 = 3.0;
/// A line of text's height.
pub const LINE: f32 = 9.0;
pub const LETTERS: [&str; 4] = ["N", "E", "S", "W"];

/// Horizon's map image is this many pixels across, at 0.5 to 4 blocks a pixel.
pub const MAP_PIXELS: f32 = 128.0;
const FINEST: f32 = 0.5;

/// Blocks across at zoom `level`.
pub fn blocks_across(level: i32) -> i32 {
    ZOOMS[(level.clamp(1, ZOOMS.len() as i32) - 1) as usize]
}

/// Zoom `level` after `steps` presses of the zoom in (+) or out (-) key.
pub fn zoomed(level: i32, steps: i32) -> i32 {
    (level + steps).clamp(1, ZOOMS.len() as i32)
}

/// What Horizon composes for `across` blocks (its blocks a pixel), and the part of its image that
/// shows them (`[x, y, w, h]` in image pixels). Closer than Horizon goes, a square map shows the
/// middle of its image; a round one can't (its image is round), so it stays at the closest.
pub fn view(across: i32, round: bool) -> (f32, [f32; 4]) {
    let bpp = across as f32 / MAP_PIXELS;
    if bpp >= FINEST || round {
        return (bpp.clamp(FINEST, 4.0), [0.0, 0.0, MAP_PIXELS, MAP_PIXELS]);
    }
    let shown = across as f32 / FINEST;
    let at = (MAP_PIXELS - shown) / 2.0;
    (FINEST, [at, at, shown, shown])
}

/// The element's height: the map, and the coordinates' tag when they show.
pub fn height(coordinates: bool) -> f32 {
    if coordinates {
        SIZE + COORDINATES_GAP + LINE + 3.0
    } else {
        SIZE
    }
}

/// The compass bearing the player faces, degrees clockwise from north, from the game's yaw (0
/// faces south).
pub fn heading(yaw: f32) -> f32 {
    (yaw + 180.0).rem_euclid(360.0)
}

/// Compass point `i` (N, E, S, W) on a map turned `rotation` degrees: where it is from the middle,
/// on the rim `at` out (a round rim, or a square one's edge).
pub fn compass_point(i: usize, rotation: f32, at: f32, round: bool) -> [f32; 2] {
    let (sin, cos) = rotation.to_radians().sin_cos();
    let wx = [0.0, 1.0, 0.0, -1.0][i % 4];
    let wz = [-1.0, 0.0, 1.0, 0.0][i % 4];
    let x = wx * cos + wz * sin;
    let y = -wx * sin + wz * cos;
    let reach: f32 = if round {
        1.0
    } else {
        f32::abs(x).max(f32::abs(y))
    };
    [x / reach * at, y / reach * at]
}

/// The way the player faces on a north-up map, as a unit step (x right, y down).
pub fn facing(heading: f32) -> [f32; 2] {
    let (sin, cos) = heading.to_radians().sin_cos();
    [sin, -cos]
}

// The window's thread only: keys, the HUD and Horizon's map all run there.
thread_local! {
    /// The zoom the keys set, with the setting's value it was set from: a session's, since the
    /// client's settings can't be written from a hook. The setting changed in the menu wins.
    static ZOOM: Cell<Option<(i32, i32)>> = const { Cell::new(None) };
    /// Between the frame hook and the HUD hook: Horizon is about to draw its map in a corner.
    static DRAWING: Cell<bool> = const { Cell::new(false) };
    static TEXT: RefCell<Cached> = const { RefCell::new((None, String::new())) };
}

/// The zoom level shown, for the `zoom` setting's `setting`.
pub fn zoom_level(setting: i32) -> i32 {
    match ZOOM.get() {
        Some((from, level)) if from == setting => level,
        _ => setting,
    }
}

/// A zoom key: `steps` in (+) or out (-) from what shows, for this session only (the keys now
/// change the module's setting itself, as Java's do).
#[allow(dead_code)]
pub fn zoom_by(setting: i32, steps: i32) {
    ZOOM.set(Some((setting, zoomed(zoom_level(setting), steps))));
}

/// Whether Horizon is asking for the map to draw it (no) rather than to compose it (yes).
pub fn drawing() -> bool {
    DRAWING.get()
}

/// The frame hook (after Horizon composed) says true; the HUD hook (after it drew) false.
pub fn set_drawing(drawing: bool) {
    DRAWING.set(drawing);
}

type Cached = (Option<[i32; 3]>, String);

/// Calls `f` with "12, 64, -30" for the block `at`, made again only when the block changes.
/// Hot path, don't allocate.
pub fn with_text<R>(at: [i32; 3], f: impl FnOnce(&str) -> R) -> R {
    TEXT.with(|cell| {
        let mut cached = cell.borrow_mut();
        if cached.0 != Some(at) {
            *cached = (Some(at), format!("{}, {}, {}", at[0], at[1], at[2]));
        }
        f(&cached.1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4
    }

    #[test]
    fn zooms_as_the_java_module_does() {
        assert_eq!(blocks_across(4), 96);
        assert_eq!(view(96, true), (0.75, [0.0, 0.0, 128.0, 128.0]));
        assert_eq!(view(32, false), (0.5, [32.0, 32.0, 64.0, 64.0]));
        assert_eq!(view(32, true).0, 0.5);
        assert!(
            ZOOMS
                .iter()
                .all(|&z| (0.5..=4.0).contains(&view(z, false).0))
        );
        assert_eq!(zoomed(7, 1), 7);
        assert_eq!(zoomed(1, -1), 1);
    }

    #[test]
    fn the_keys_zoom_until_the_setting_changes() {
        assert_eq!(zoom_level(4), 4);
        zoom_by(4, 1);
        zoom_by(4, 1);
        assert_eq!(zoom_level(4), 6);
        assert_eq!(zoom_level(2), 2, "set in the menu");
    }

    #[test]
    fn puts_north_where_it_lies() {
        assert!(near(compass_point(0, 0.0, 10.0, true), [0.0, -10.0]));
        assert!(near(compass_point(1, 0.0, 10.0, true), [10.0, 0.0]));
        // Facing south (heading 180), turning with the player: north is at the bottom.
        assert_eq!(heading(0.0), 180.0);
        assert!(near(compass_point(0, 180.0, 10.0, true), [0.0, 10.0]));
        // A square rim at 45 degrees: on the corner.
        assert!(near(compass_point(0, 45.0, 10.0, false), [-10.0, -10.0]));
        assert!(near(facing(90.0), [1.0, 0.0]));
        assert_eq!(height(true), 111.0);
    }

    #[test]
    fn writes_the_block() {
        assert_eq!(with_text([12, 64, -30], str::to_owned), "12, 64, -30");
        set_drawing(true);
        assert!(drawing());
    }
}
