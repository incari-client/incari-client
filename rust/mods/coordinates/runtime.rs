// Incari's Coordinates for the Rust build (ADR 0068, 0069), as features/coordinates does it: the
// block you're in, which way you face, and the matching spot in the other dimension, label beside
// value on the glass. Horizon's native API has no biome yet, so that row isn't here. Its settings
// and HUD element are the module's in `incari_core::modules` (the Java ids). No game types:
// rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "coordinates";
/// Its HUD element's id.
pub const ELEMENT: &str = "position";

pub const NETHER: &str = "minecraft:the_nether";
pub const OVERWORLD: &str = "minecraft:overworld";

/// Between a row's label and its value.
pub const LABEL_GAP: f32 = 4.0;

/// The block a coordinate is in: rounded down, so -0.5 is block -1.
pub fn block(coordinate: f64) -> i32 {
    coordinate.floor() as i32
}

/// Which eighth of the compass the player faces, 0 north, 2 east, from Minecraft's yaw (0 faces
/// south, 90 west): the Java module's bearing, yaw + 180.
pub fn sector(yaw: f32) -> usize {
    let heading = (yaw + 180.0).rem_euclid(360.0);
    ((heading / 45.0).round() as usize) % 8
}

/// The compass point of `sector`, with the axis that way runs along.
pub fn facing(sector: usize) -> &'static str {
    [
        "North (-Z)",
        "Northeast",
        "East (+X)",
        "Southeast",
        "South (+Z)",
        "Southwest",
        "West (-X)",
        "Northwest",
    ][sector % 8]
}

/// What the rows are made from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spot {
    pub at: [i32; 3],
    pub sector: usize,
    pub nether: bool,
    pub overworld: bool,
}

/// The HUD editor's stand-in with no world: the Java sample player, at 0 0 0 in the Overworld
/// facing north.
pub const SAMPLE: Spot = Spot {
    at: [0, 0, 0],
    sector: 0,
    nether: false,
    overworld: true,
};

/// What the settings ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shown {
    pub one_line: bool,
    pub facing: bool,
    pub other: bool,
}

pub const MAX_ROWS: usize = 5;

/// A row: its label ("" for none) and its value.
pub type Row = (&'static str, String);

/// The rows for `spot`, up to `MAX_ROWS`.
pub fn rows(spot: Spot, shown: Shown) -> Vec<Row> {
    let [x, y, z] = spot.at;
    let mut rows = Vec::with_capacity(MAX_ROWS);
    if shown.one_line {
        rows.push(("", format!("X {x}  Y {y}  Z {z}")));
    } else {
        rows.extend([("X", x.to_string()), ("Y", y.to_string()), ("Z", z.to_string())]);
    }
    if shown.facing {
        rows.push(("", facing(spot.sector).to_owned()));
    }
    if shown.other && spot.nether {
        rows.push(("Overworld", format!("{}, {}", x * 8, z * 8)));
    } else if shown.other && spot.overworld {
        rows.push(("Nether", format!("{}, {}", x.div_euclid(8), z.div_euclid(8))));
    }
    rows
}

/// A value's tone: a row without a label after the first is quieter (the facing, as the Java
/// module draws it).
pub fn muted(row: usize, label: &str) -> bool {
    label.is_empty() && row >= 1
}

type Cached = (Option<(Spot, Shown)>, Vec<Row>);

// Hot path, don't allocate: made again only when the block, the facing or a setting changes.
thread_local! {
    static ROWS: std::cell::RefCell<Cached> = const { std::cell::RefCell::new((None, Vec::new())) };
}

/// Calls `f` with `rows(spot, shown)`, made again only when one of them changed.
pub fn with_rows<R>(spot: Spot, shown: Shown, f: impl FnOnce(&[Row]) -> R) -> R {
    ROWS.with(|cell| {
        let mut cached = cell.borrow_mut();
        if cached.0 != Some((spot, shown)) {
            *cached = (Some((spot, shown)), rows(spot, shown));
        }
        f(&cached.1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spot(at: [i32; 3], nether: bool) -> Spot {
        Spot {
            at,
            sector: 0,
            nether,
            overworld: !nether,
        }
    }

    const ALL: Shown = Shown {
        one_line: false,
        facing: true,
        other: true,
    };

    #[test]
    fn faces_as_the_java_module_does() {
        // Minecraft's yaw: 180 faces north, -90 east, 0 south, 90 west.
        assert_eq!(facing(sector(180.0)), "North (-Z)");
        assert_eq!(facing(sector(-90.0)), "East (+X)");
        assert_eq!(facing(sector(0.0)), "South (+Z)");
        assert_eq!(facing(sector(90.0)), "West (-X)");
        assert_eq!(facing(sector(-135.0)), "Northeast");
        assert_eq!(facing(sector(-180.0 - 720.0)), "North (-Z)");
    }

    #[test]
    fn rounds_down_to_the_block() {
        assert_eq!([block(-0.5), block(0.5), block(-16.0)], [-1, 0, -16]);
    }

    #[test]
    fn shows_the_other_dimension() {
        let rows = rows(spot([-9, 64, 17], false), ALL);
        assert_eq!(rows.len(), MAX_ROWS);
        assert_eq!(rows[4], ("Nether", "-2, 2".to_owned()));
        let line = Shown {
            one_line: true,
            facing: false,
            other: true,
        };
        assert_eq!(
            super::rows(spot([3, 70, -1], true), line),
            [("", "X 3  Y 70  Z -1".to_owned()), ("Overworld", "24, -8".to_owned())]
        );
        let end = Spot {
            overworld: false,
            ..spot([0, 0, 0], false)
        };
        assert_eq!(super::rows(end, line).len(), 1);
        assert_eq!(super::rows(SAMPLE, ALL)[3], ("", "North (-Z)".to_owned()));
    }

    #[test]
    fn quietens_the_unlabelled_rows_after_the_first() {
        assert!(!muted(0, ""));
        assert!(muted(1, ""));
        assert!(!muted(3, "Nether"));
    }

    #[test]
    fn makes_the_rows_again_only_when_something_changed() {
        let s = spot([1, 2, 3], false);
        let first = with_rows(s, ALL, |r| r.as_ptr());
        assert_eq!(with_rows(s, ALL, |r| r.as_ptr()), first);
        let line = Shown {
            one_line: true,
            facing: false,
            other: false,
        };
        assert_eq!(with_rows(s, line, <[_]>::len), 1);
    }
}
