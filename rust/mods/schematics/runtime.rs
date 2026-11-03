// Incari's Schematics for the Rust build (ADR 0066, 0068, 0069): the bits of its game side that are
// plain arithmetic. The module itself (readers, placement, checking, materials, HUD) is the
// client's, `incari_core::schematic`; this file is both the module the bundle adds to the game and,
// through rust/lib.rs, the code its tests run: it uses no game types.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "schematics";

/// What the browser's file picker is asked for with, to tell its file from another asker's.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // the browser's file picker only
pub const PICK_KIND: &str = "incari_schematics";

/// A world block's place in its 16-block section, as the ghost hook packs it: `y << 8 | z << 4 | x`.
#[allow(dead_code)] // the inverse of `cell_block`, which the tests hold it to
pub fn cell(x: i32, y: i32, z: i32) -> u16 {
    ((y & 15) << 8 | (z & 15) << 4 | (x & 15)) as u16
}

/// The world block at `cell` of section `section`.
pub fn cell_block(section: [i32; 3], cell: u16) -> [i32; 3] {
    let c = i32::from(cell);
    [
        section[0] << 4 | (c & 15),
        section[1] << 4 | (c >> 8),
        section[2] << 4 | ((c >> 4) & 15),
    ]
}

/// A box's twelve edges, from block corner `min` to `max`.
pub fn box_edges(min: [i32; 3], max: [i32; 3]) -> [([f64; 3], [f64; 3]); 12] {
    let (a, b) = (min.map(f64::from), max.map(f64::from));
    let p = |x: bool, y: bool, z: bool| {
        [
            if x { b[0] } else { a[0] },
            if y { b[1] } else { a[1] },
            if z { b[2] } else { a[2] },
        ]
    };
    [
        // Along x.
        (p(false, false, false), p(true, false, false)),
        (p(false, true, false), p(true, true, false)),
        (p(false, false, true), p(true, false, true)),
        (p(false, true, true), p(true, true, true)),
        // Along y.
        (p(false, false, false), p(false, true, false)),
        (p(true, false, false), p(true, true, false)),
        (p(false, false, true), p(false, true, true)),
        (p(true, false, true), p(true, true, true)),
        // Along z.
        (p(false, false, false), p(false, false, true)),
        (p(true, false, false), p(true, false, true)),
        (p(false, true, false), p(false, true, true)),
        (p(true, true, false), p(true, true, true)),
    ]
}

/// The translation keys an item's name may be under: blocks' items are named by their block.
pub fn name_keys(id: &str) -> [String; 2] {
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
    [format!("item.{ns}.{path}"), format!("block.{ns}.{path}")]
}

/// Which world the player is in, to see them leave it (the schematic's comparison starts afresh,
/// as Java's LEAVE event does).
#[derive(Default)]
pub struct WorldWatch {
    world: Option<String>,
}

impl WorldWatch {
    /// The world now (`native_world_id`); true when the player just left one.
    pub fn left(&mut self, world: Option<&str>) -> bool {
        if self.world.as_deref() == world {
            return false;
        }
        let was = self.world.is_some();
        self.world = world.map(str::to_owned);
        was
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_pack_and_unpack() {
        for [x, y, z] in [[0, 0, 0], [-1, 70, 33], [15, -64, -17]] {
            let section = [x >> 4, y >> 4, z >> 4];
            assert_eq!(cell_block(section, cell(x, y, z)), [x, y, z]);
        }
        assert_eq!(cell(1, 2, 3), 2 << 8 | 3 << 4 | 1);
    }

    #[test]
    fn a_box_has_twelve_unit_long_edges() {
        let edges = box_edges([0, 0, 0], [1, 2, 3]);
        let lengths: Vec<f64> = edges
            .iter()
            .map(|(a, b)| (0..3).map(|i| (b[i] - a[i]).abs()).sum())
            .collect();
        assert_eq!(
            lengths,
            [1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0, 3.0]
        );
    }

    #[test]
    fn names_and_worlds() {
        assert_eq!(
            name_keys("minecraft:stone"),
            ["item.minecraft.stone", "block.minecraft.stone"]
        );
        let mut w = WorldWatch::default();
        assert!(!w.left(Some("world/a")));
        assert!(!w.left(Some("world/a")));
        assert!(w.left(Some("server/b")));
        assert!(w.left(None));
        assert!(!w.left(None));
    }
}
