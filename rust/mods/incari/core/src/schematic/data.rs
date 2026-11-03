//! A structure to build, whatever file it came from: a box of blocks, each an index into the
//! palette of block states. Index 0 is always `BlockSpec::NOTHING`, no block wanted (outside a
//! `.litematic`'s regions, a structure's void); air in the palette is air wanted. Positions are
//! from the box's lowest corner, `0` to `size - 1`. Read once, never changed.
use super::packed::{Packed, bits_for};
use super::spec::BlockSpec;
use std::collections::HashMap;

/// The most blocks a box may hold, so a schematic fits a Chromebook's memory with the game.
pub const MAX_VOLUME: u64 = 1 << 25;

#[derive(Clone, Debug)]
pub struct Schematic {
    /// Its name, as the file gives it or the file's name.
    pub name: String,
    /// Who made it; empty if the file doesn't say.
    pub author: String,
    /// Which kind of file it came from ("Litematic", "Sponge", "Structure").
    pub format: &'static str,
    /// The game data version its blocks were saved at.
    pub data_version: i32,
    pub size: [i32; 3],
    /// The block states it uses; index 0 is `NOTHING`.
    pub palette: Vec<BlockSpec>,
    blocks: Packed,
    counts: Vec<u32>,
}

impl Schematic {
    /// The palette index at `x, y, z`, which must be in the box. Hot path.
    #[inline]
    pub fn at(&self, x: i32, y: i32, z: i32) -> usize {
        let [sx, _, sz] = self.size;
        self.blocks.get(((y * sz + z) * sx + x) as usize) as usize
    }

    /// How many blocks have palette index `index`.
    pub fn count(&self, index: usize) -> u32 {
        self.counts[index]
    }

    /// How many blocks it wants, air left out.
    pub fn block_count(&self) -> u32 {
        (1..self.counts.len())
            .filter(|&i| !self.palette[i].is_air())
            .map(|i| self.counts[i])
            .sum()
    }
}

/// Builds a schematic block by block; any block not set is `NOTHING`.
pub struct Builder {
    size: [i32; 3],
    palette: Vec<BlockSpec>,
    indices: HashMap<BlockSpec, usize>,
    blocks: Packed,
    name: String,
    author: String,
    format: &'static str,
    data_version: i32,
}

impl Builder {
    /// A box of this size; refused when it's empty or bigger than `MAX_VOLUME`.
    pub fn new(sx: i32, sy: i32, sz: i32) -> Result<Self, String> {
        if sx <= 0 || sy <= 0 || sz <= 0 {
            return Err(format!("It's empty: {sx} x {sy} x {sz}"));
        }
        let volume = sx as u64 * sy as u64 * sz as u64;
        if volume > MAX_VOLUME {
            return Err(format!("Too big to load: {sx} x {sy} x {sz}"));
        }
        let mut indices = HashMap::new();
        indices.insert(BlockSpec::NOTHING, 0);
        Ok(Self {
            size: [sx, sy, sz],
            palette: vec![BlockSpec::NOTHING],
            indices,
            blocks: Packed::zeros(2, volume as usize),
            name: String::new(),
            author: String::new(),
            format: "",
            data_version: 0,
        })
    }

    pub fn describe(&mut self, name: &str, author: &str, format: &'static str, data_version: i32) {
        self.name = name.to_owned();
        self.author = author.to_owned();
        self.format = format;
        self.data_version = data_version;
    }

    /// `spec`'s palette index, added if it's new.
    pub fn index(&mut self, spec: BlockSpec) -> usize {
        if let Some(&i) = self.indices.get(&spec) {
            return i;
        }
        let i = self.palette.len();
        self.palette.push(spec.clone());
        self.indices.insert(spec, i);
        let needed = bits_for(self.palette.len(), 2);
        if needed > self.blocks.bits() {
            self.blocks = self.blocks.widened(needed);
        }
        i
    }

    /// Sets `x, y, z` (in the box) to palette index `index`.
    pub fn set(&mut self, x: i32, y: i32, z: i32, index: usize) {
        let [sx, _, sz] = self.size;
        self.blocks
            .set(((y * sz + z) * sx + x) as usize, index as u32);
    }

    pub fn contains(&self, x: i32, y: i32, z: i32) -> bool {
        let [sx, sy, sz] = self.size;
        x >= 0 && y >= 0 && z >= 0 && x < sx && y < sy && z < sz
    }

    pub fn build(self) -> Schematic {
        let mut counts = vec![0u32; self.palette.len()];
        for i in 0..self.blocks.len() {
            counts[self.blocks.get(i) as usize] += 1;
        }
        Schematic {
            name: self.name,
            author: self.author,
            format: self.format,
            data_version: self.data_version,
            size: self.size,
            palette: self.palette,
            blocks: self.blocks,
            counts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widens_as_the_palette_grows() {
        let mut b = Builder::new(4, 4, 4).unwrap();
        for i in 0..20 {
            let index = b.index(BlockSpec::of(&format!("block_{i}"), Vec::new()));
            b.set(i % 4, i / 16, (i / 4) % 4, index);
        }
        let air = b.index(BlockSpec::of("air", Vec::new()));
        b.set(3, 3, 3, air);
        let s = b.build();
        assert_eq!(s.at(0, 0, 0), 1);
        assert_eq!(s.at(3, 1, 0), 20);
        assert_eq!(s.at(3, 3, 3), air);
        assert_eq!(s.at(2, 2, 2), 0);
        assert_eq!(s.block_count(), 20);
        assert_eq!(s.count(0), 64 - 21);
        assert!(Builder::new(0, 1, 1).is_err());
        assert!(Builder::new(1 << 13, 1 << 13, 2).is_err());
    }
}
