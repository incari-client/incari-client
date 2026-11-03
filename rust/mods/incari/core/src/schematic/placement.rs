//! Where a schematic stands in the world: its box's lowest corner, and how it's mirrored and then
//! turned. Turning and mirroring keep that corner where it is, so the box only changes shape: the
//! player sees it turn in place rather than swing about a point. Turns are clockwise seen from
//! above, as the game's own rotations: what faced north faces east after one.
use super::Mirror;
use super::data::Schematic;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct Placement {
    pub schematic: Rc<Schematic>,
    min: [i32; 3],
    mirror: Mirror,
    turns: u8,
}

impl Placement {
    pub fn new(schematic: Rc<Schematic>, x: i32, y: i32, z: i32) -> Self {
        Self {
            schematic,
            min: [x, y, z],
            mirror: Mirror::None,
            turns: 0,
        }
    }

    /// Puts the box's lowest corner at `x, y, z`.
    pub fn move_to(&mut self, x: i32, y: i32, z: i32) {
        self.min = [x, y, z];
    }

    pub fn move_by(&mut self, dx: i32, dy: i32, dz: i32) {
        let [x, y, z] = self.min;
        self.min = [x + dx, y + dy, z + dz];
    }

    /// Turns it `quarter_turns` more clockwise (negative: anticlockwise).
    pub fn turn(&mut self, quarter_turns: i32) {
        self.turns = (i32::from(self.turns) + quarter_turns).rem_euclid(4) as u8;
    }

    pub fn set_mirror(&mut self, mirror: Mirror) {
        self.mirror = mirror;
    }

    pub fn mirror(&self) -> Mirror {
        self.mirror
    }

    /// Clockwise quarter turns, 0 to 3.
    pub fn turns(&self) -> u8 {
        self.turns
    }

    /// The box's lowest corner.
    pub fn min(&self) -> [i32; 3] {
        self.min
    }

    /// The placed box's size: x and z swap after an odd number of turns.
    pub fn size(&self) -> [i32; 3] {
        let [sx, sy, sz] = self.schematic.size;
        if self.turns & 1 == 0 {
            [sx, sy, sz]
        } else {
            [sz, sy, sx]
        }
    }

    /// One past the box's last block.
    pub fn max(&self) -> [i32; 3] {
        let (m, s) = (self.min, self.size());
        [m[0] + s[0], m[1] + s[1], m[2] + s[2]]
    }

    pub fn contains(&self, x: i32, y: i32, z: i32) -> bool {
        let (lo, hi) = (self.min, self.max());
        x >= lo[0] && y >= lo[1] && z >= lo[2] && x < hi[0] && y < hi[1] && z < hi[2]
    }

    /// The schematic's palette index for the world block `x, y, z`: 0 (nothing wanted) outside
    /// the box. Hot path: arithmetic only.
    #[inline]
    pub fn index_at(&self, wx: i32, wy: i32, wz: i32) -> usize {
        let (px, py, pz) = (wx - self.min[0], wy - self.min[1], wz - self.min[2]);
        let [sx, sy, sz] = self.schematic.size;
        if py < 0 || py >= sy || px < 0 || pz < 0 {
            return 0;
        }
        // Undo the turn, then the mirror: placed box -> mirrored schematic -> schematic.
        let (mx, mz) = match self.turns {
            1 if px < sz && pz < sx => (pz, sz - 1 - px),
            2 if px < sx && pz < sz => (sx - 1 - px, sz - 1 - pz),
            3 if px < sz && pz < sx => (sx - 1 - pz, px),
            0 if px < sx && pz < sz => (px, pz),
            _ => return 0,
        };
        let lx = if self.mirror == Mirror::X {
            sx - 1 - mx
        } else {
            mx
        };
        let lz = if self.mirror == Mirror::Z {
            sz - 1 - mz
        } else {
            mz
        };
        self.schematic.at(lx, py, lz)
    }

    /// Where the schematic's own block `x, y, z` lands in the world: mirrored, then turned. The
    /// inverse of `index_at`'s lookup.
    pub fn to_world(&self, lx: i32, ly: i32, lz: i32) -> [i32; 3] {
        let [sx, _, sz] = self.schematic.size;
        let mx = if self.mirror == Mirror::X {
            sx - 1 - lx
        } else {
            lx
        };
        let mz = if self.mirror == Mirror::Z {
            sz - 1 - lz
        } else {
            lz
        };
        let (px, pz) = match self.turns {
            1 => (sz - 1 - mz, mx),
            2 => (sx - 1 - mx, sz - 1 - mz),
            3 => (mz, sx - 1 - mx),
            _ => (mx, mz),
        };
        [self.min[0] + px, self.min[1] + ly, self.min[2] + pz]
    }
}

#[cfg(test)]
mod tests {
    use super::super::data::Builder;
    use super::super::spec::BlockSpec;
    use super::*;
    use std::collections::HashSet;

    /// A 4 x 2 x 3 box with a different block in every place.
    fn numbered() -> Rc<Schematic> {
        let mut b = Builder::new(4, 2, 3).unwrap();
        for y in 0..2 {
            for z in 0..3 {
                for x in 0..4 {
                    let i = b.index(BlockSpec::of(&format!("test:b{x}{y}{z}"), Vec::new()));
                    b.set(x, y, z, i);
                }
            }
        }
        Rc::new(b.build())
    }

    #[test]
    fn every_turn_and_mirror_maps_each_block_to_one_place_in_the_box() {
        let s = numbered();
        for mirror in [Mirror::None, Mirror::X, Mirror::Z] {
            for turns in 0..4 {
                let mut p = Placement::new(s.clone(), 100, 64, -50);
                p.set_mirror(mirror);
                p.turn(turns);
                assert_eq!(p.size()[0], if turns % 2 == 0 { 4 } else { 3 });
                let mut places = HashSet::new();
                for y in 0..2 {
                    for z in 0..3 {
                        for x in 0..4 {
                            let [wx, wy, wz] = p.to_world(x, y, z);
                            assert!(p.contains(wx, wy, wz));
                            assert!(places.insert([wx, wy, wz]));
                            assert_eq!(p.index_at(wx, wy, wz), s.at(x, y, z));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn outside_the_box_nothing_is_wanted() {
        let mut p = Placement::new(numbered(), 0, 0, 0);
        p.turn(1);
        assert_eq!(p.index_at(-1, 0, 0), 0);
        assert_eq!(p.index_at(3, 0, 0), 0);
        assert_eq!(p.index_at(0, 2, 0), 0);
        assert_eq!(p.index_at(0, 0, 4), 0);
        assert_ne!(p.index_at(2, 1, 3), 0);
        p.turn(-5);
        assert_eq!(p.turns(), 0);
    }
}
