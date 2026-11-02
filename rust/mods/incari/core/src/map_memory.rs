//! What the map has seen (MapMemory.java, ADR 0051), for the world map: every chunk's 256 colours
//! as the map drew them, by region of 32 by 32 chunks, for each world and dimension. With the
//! minimap's Remember explored map on, regions are kept in the Java client's files
//! (`incari/map/<world>/<dimension>/<x>.<z>.map`), so both builds share them, and read back when
//! the world map reaches them again; off, only this visit's are kept, as many as fit.
//!
//! A region file: `IMAP`, a version, a bit for each of its 1024 chunks that's been seen, and each
//! seen chunk's colours as RGB (0, 0, 0 for nothing there: black is stored a step lighter), all
//! zlib-deflated. The game side gives the deflater (`Zlib`), since this crate has none.
//!
//! Where the colours come from is the game's: Horizon must hand each chunk its map draws to
//! `record` (its pending hook is in `rust/mods/waypoints/horizon-mixin.toml`). The world map reads them through `MapSource`.
//! No game types.
use crate::waypoint::safe_name;
use std::cell::RefCell;
use std::collections::HashMap;

/// Chunks a region's side.
pub const REGION_CHUNKS: i32 = 32;
/// Blocks a region's side.
pub const REGION_BLOCKS: i32 = REGION_CHUNKS * 16;
const MAGIC: u32 = 0x494D_4150;
const VERSION: u32 = 1;
/// Regions kept in memory at most: some 1 MB each when seen whole.
const KEPT: usize = 24;
/// Ticks between saves of what changed: half a minute.
const SAVE_TICKS: u32 = 600;

/// Files under the game folder's `incari/`, as bytes.
pub trait Bytes {
    fn read(&self, path: &str) -> Option<Vec<u8>>;
    fn write(&mut self, path: &str, data: &[u8]);
}

/// The client's files through its host.
pub struct HostBytes<'a>(pub &'a mut dyn crate::screen::Host);

impl Bytes for HostBytes<'_> {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        self.0.read(path)
    }
    fn write(&mut self, path: &str, data: &[u8]) {
        self.0.write(path, data);
    }
}

/// zlib, from the game side (Horizon has `miniz_oxide`): deflate at the fastest level, and
/// inflate (None for what isn't zlib).
#[derive(Clone, Copy)]
pub struct Zlib {
    pub deflate: fn(&[u8]) -> Vec<u8>,
    pub inflate: fn(&[u8]) -> Option<Vec<u8>>,
}

/// A region of one dimension.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub dimension: String,
    pub x: i32,
    pub z: i32,
}

/// A region's chunks' colours (`0xAARRGGBB`, 0 for nothing there), those seen; a number that goes
/// up as more are.
#[derive(Clone, Debug)]
pub struct Region {
    chunks: Vec<Option<Box<[u32; 256]>>>,
    pub revision: u32,
    dirty: bool,
    used_at: u64,
}

impl Default for Region {
    fn default() -> Self {
        Self {
            chunks: vec![None; (REGION_CHUNKS * REGION_CHUNKS) as usize],
            revision: 0,
            dirty: false,
            used_at: 0,
        }
    }
}

impl Region {
    /// Chunk `cx, cz` within the region (0 to 31 each), or None if unseen.
    pub fn chunk(&self, cx: i32, cz: i32) -> Option<&[u32; 256]> {
        self.chunks[(cx + cz * REGION_CHUNKS) as usize].as_deref()
    }

    /// The colour of block `bx, bz` within the region (0 to 511 each); None where unseen.
    pub fn color(&self, bx: i32, bz: i32) -> Option<u32> {
        let chunk = self.chunk(bx >> 4, bz >> 4)?;
        Some(chunk[((bz & 15) * 16 + (bx & 15)) as usize])
    }

    /// Puts `from`'s chunks in where this has none.
    pub fn merge(&mut self, from: &Region) {
        let mut added = false;
        for (into, from) in self.chunks.iter_mut().zip(&from.chunks) {
            if into.is_none() && from.is_some() {
                into.clone_from(from);
                added = true;
            }
        }
        if added {
            self.revision += 1;
        }
    }

    /// The file's bytes before deflating.
    pub fn encode(&self) -> Vec<u8> {
        let seen = self.chunks.iter().filter(|c| c.is_some()).count();
        let mut out = Vec::with_capacity(8 + 128 + seen * 768);
        out.extend_from_slice(&MAGIC.to_be_bytes());
        out.extend_from_slice(&VERSION.to_be_bytes());
        let mut bits = [0u8; 128];
        for (i, c) in self.chunks.iter().enumerate() {
            if c.is_some() {
                bits[i >> 3] |= 1 << (i & 7);
            }
        }
        out.extend_from_slice(&bits);
        for chunk in self.chunks.iter().flatten() {
            for &color in chunk.iter() {
                // Clear is 0, 0, 0; a black block a step lighter, so it isn't read as clear.
                let stored = if color >> 24 == 0 {
                    0
                } else if color & 0xFF_FFFF == 0 {
                    0x01_0101
                } else {
                    color
                };
                out.extend_from_slice(&[(stored >> 16) as u8, (stored >> 8) as u8, stored as u8]);
            }
        }
        out
    }

    /// A region from a file's bytes after inflating; None for what isn't one this build reads.
    pub fn decode(bytes: &[u8]) -> Option<Region> {
        let word = |at: usize| Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
        if word(0)? != MAGIC || word(4)? != VERSION {
            return None;
        }
        let bits = bytes.get(8..136)?;
        let mut region = Region::default();
        let mut at = 136;
        for i in 0..region.chunks.len() {
            if bits[i >> 3] & (1 << (i & 7)) == 0 {
                continue;
            }
            let rgb = bytes.get(at..at + 768)?;
            at += 768;
            let mut chunk = Box::new([0u32; 256]);
            for (j, c) in chunk.iter_mut().enumerate() {
                let color = (rgb[3 * j] as u32) << 16 | (rgb[3 * j + 1] as u32) << 8 | rgb[3 * j + 2] as u32;
                *c = if color == 0 { 0 } else { 0xFF00_0000 | color };
            }
            region.chunks[i] = Some(chunk);
        }
        Some(region)
    }
}

/// A remembered region's file, under `incari/`.
pub fn file(world: &str, key: &Key) -> String {
    format!("map/{}/{}/{}.{}.map", safe_name(world), safe_name(&key.dimension), key.x, key.z)
}

/// The remembered map of the world played.
pub struct MapMemory {
    regions: HashMap<Key, Region>,
    world: Option<String>,
    remember: bool,
    uses: u64,
    ticks: u32,
    saving: bool,
    zlib: Option<Zlib>,
    /// Problems reading or saving, for the game side to log.
    pub problems: Vec<String>,
}

impl Default for MapMemory {
    fn default() -> Self {
        Self {
            regions: HashMap::new(),
            world: None,
            remember: true,
            uses: 0,
            ticks: 0,
            saving: false,
            zlib: None,
            problems: Vec::new(),
        }
    }
}

impl MapMemory {
    /// The game side's zlib; without it nothing is read or saved.
    pub fn set_zlib(&mut self, zlib: Zlib) {
        self.zlib = Some(zlib);
    }

    /// The world whose map this is.
    pub fn world(&self) -> Option<&str> {
        self.world.as_deref()
    }

    /// The minimap's Remember explored map. Turned on, what's been seen this visit is kept from
    /// now on, joined with what the files had.
    pub fn set_remember(&mut self, on: bool, files: &dyn Bytes) {
        if on && !self.remember {
            let keys: Vec<Key> = self.regions.keys().cloned().collect();
            for key in keys {
                let saved = self.read(files, &key);
                if let Some(region) = self.regions.get_mut(&key) {
                    if let Some(saved) = saved {
                        region.merge(&saved);
                    }
                    region.dirty = true;
                }
            }
        }
        self.remember = on;
    }

    /// The player joined `world`; its map is kept under its own folder.
    pub fn open(&mut self, world: &str, files: &mut dyn Bytes) {
        if self.world.as_deref() == Some(world) {
            return;
        }
        self.close(files);
        self.world = Some(world.to_owned());
    }

    /// The player left: what changed is saved, and the regions let go.
    pub fn close(&mut self, files: &mut dyn Bytes) {
        self.save(files);
        self.regions.clear();
        self.world = None;
    }

    /// Once a game tick: what changed is saved every half minute, a region a tick.
    pub fn tick(&mut self, files: &mut dyn Bytes) {
        self.ticks = self.ticks.wrapping_add(1);
        if self.ticks.is_multiple_of(SAVE_TICKS) {
            self.saving = true;
        }
        if self.saving {
            let next = self
                .remember
                .then(|| self.regions.iter().find(|r| r.1.dirty).map(|r| r.0.clone()))
                .flatten();
            self.saving = match next {
                Some(key) => {
                    self.write(files, &key);
                    true
                }
                None => false,
            };
        }
    }

    /// The map drew chunk `chunk_x, chunk_z` of `dimension`: its 256 colours, row by row.
    pub fn record(&mut self, files: &mut dyn Bytes, dimension: &str, chunk_x: i32, chunk_z: i32, colors: &[u32; 256]) {
        if self.world.is_none() {
            return;
        }
        let Some(region) = self.region(files, dimension, chunk_x >> 5, chunk_z >> 5, true) else {
            return;
        };
        let index = ((chunk_x & 31) + (chunk_z & 31) * REGION_CHUNKS) as usize;
        match &mut region.chunks[index] {
            Some(kept) if **kept == *colors => return,
            Some(kept) => **kept = *colors,
            slot => *slot = Some(Box::new(*colors)),
        }
        region.revision += 1;
        region.dirty = true;
    }

    /// Region `x, z` of `dimension`, read from its file if it isn't in memory; None with no world
    /// open, or nothing seen there and `make` false.
    pub fn region(&mut self, files: &mut dyn Bytes, dimension: &str, x: i32, z: i32, make: bool) -> Option<&mut Region> {
        self.world.as_ref()?;
        let key = Key { dimension: dimension.to_owned(), x, z };
        self.uses += 1;
        if !self.regions.contains_key(&key) {
            let read = if self.remember { self.read(files, &key) } else { None };
            let region = match read {
                Some(r) => r,
                None if make => Region::default(),
                None => return None,
            };
            self.regions.insert(key.clone(), Region { used_at: self.uses, ..region });
            self.evict(files);
        }
        let region = self.regions.get_mut(&key)?;
        region.used_at = self.uses;
        Some(region)
    }

    /// The region in memory, without reading its file.
    pub fn kept(&self, dimension: &str, x: i32, z: i32) -> Option<&Region> {
        self.regions.get(&Key { dimension: dimension.to_owned(), x, z })
    }

    /// Saves every region that changed.
    pub fn save(&mut self, files: &mut dyn Bytes) {
        let keys: Vec<Key> = self.regions.keys().cloned().collect();
        for key in keys {
            self.write(files, &key);
        }
    }

    fn evict(&mut self, files: &mut dyn Bytes) {
        while self.regions.len() > KEPT {
            let Some(oldest) = self.regions.iter().min_by_key(|r| r.1.used_at).map(|r| r.0.clone()) else {
                return;
            };
            self.write(files, &oldest);
            self.regions.remove(&oldest);
        }
    }

    fn write(&mut self, files: &mut dyn Bytes, key: &Key) {
        let (Some(world), Some(zlib)) = (self.world.as_deref(), self.zlib) else {
            return;
        };
        let Some(region) = self.regions.get_mut(key) else {
            return;
        };
        if !self.remember || !region.dirty {
            return;
        }
        region.dirty = false;
        files.write(&file(world, key), &(zlib.deflate)(&region.encode()));
    }

    fn read(&mut self, files: &dyn Bytes, key: &Key) -> Option<Region> {
        let (world, zlib) = (self.world.as_deref()?, self.zlib?);
        let path = file(world, key);
        let bytes = files.read(&path)?;
        let region = (zlib.inflate)(&bytes).and_then(|raw| Region::decode(&raw));
        if region.is_none() {
            self.problems.push(format!("Not a map region this build reads: {path}"));
        }
        region
    }
}

/// What the world map draws from: each region's colours, the surface's height where it's known.
pub trait MapSource {
    /// Calls `f` with region `rx, rz` of `dimension`, or None where nothing's been seen.
    fn region(&mut self, files: &mut dyn Bytes, dimension: &str, rx: i32, rz: i32, f: &mut dyn FnMut(Option<&Region>));
    /// The surface's height (the highest block that blocks motion) at a block of a loaded chunk.
    fn surface(&self, x: i32, z: i32) -> Option<i32> {
        let _ = (x, z);
        None
    }
}

/// The remembered map of the window's thread, as the world map's source.
pub struct Remembered;

impl MapSource for Remembered {
    fn region(&mut self, files: &mut dyn Bytes, dimension: &str, rx: i32, rz: i32, f: &mut dyn FnMut(Option<&Region>)) {
        let done = with(|m| f(m.region(files, dimension, rx, rz, false).map(|r| &*r)));
        if done.is_none() {
            f(None);
        }
    }
}

// The window's thread only.
thread_local! {
    static MEMORY: RefCell<MapMemory> = RefCell::new(MapMemory::default());
}

/// Runs `f` with the remembered map; None while it's in use further up the call.
pub fn with<R>(f: impl FnOnce(&mut MapMemory) -> R) -> Option<R> {
    MEMORY.with(|m| m.try_borrow_mut().ok().map(|mut m| f(&mut m)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Files(HashMap<String, Vec<u8>>);

    impl Bytes for Files {
        fn read(&self, path: &str) -> Option<Vec<u8>> {
            self.0.get(path).cloned()
        }
        fn write(&mut self, path: &str, data: &[u8]) {
            self.0.insert(path.into(), data.to_vec());
        }
    }

    /// No compression, for the tests: the format inside is what's checked.
    const PLAIN: Zlib = Zlib { deflate: |b| b.to_vec(), inflate: |b| Some(b.to_vec()) };

    fn chunk(color: u32) -> [u32; 256] {
        let mut c = [color; 256];
        c[1] = 0xFF00_0000;
        c[2] = 0;
        c
    }

    #[test]
    fn writes_the_java_region_format() {
        let mut region = Region::default();
        region.chunks[0] = Some(Box::new(chunk(0xFF12_3456)));
        region.chunks[9] = Some(Box::new(chunk(0xFF65_4321)));
        let bytes = region.encode();
        assert_eq!(&bytes[..8], b"IMAP\0\0\0\x01");
        assert_eq!((bytes[8], bytes[9]), (0b1, 0b10));
        assert_eq!(bytes.len(), 8 + 128 + 2 * 768);
        // Black is stored a step lighter; clear as nothing.
        assert_eq!(&bytes[136..142], &[0x12, 0x34, 0x56, 1, 1, 1]);
        assert_eq!(&bytes[142..145], &[0, 0, 0]);
        let back = Region::decode(&bytes).unwrap();
        assert_eq!(back.color(0, 0), Some(0xFF12_3456));
        assert_eq!(back.color(1, 0), Some(0xFF01_0101));
        assert_eq!(back.color(2, 0), Some(0));
        assert_eq!(back.color(9 * 16, 0), Some(0xFF65_4321));
        assert_eq!(back.color(16, 0), None);
        assert!(Region::decode(b"IMAP\0\0\0\x02").is_none());
        assert!(Region::decode(&bytes[..200]).is_none());
    }

    #[test]
    fn names_files_as_java_does() {
        let key = Key { dimension: "minecraft:overworld".into(), x: -1, z: 3 };
        let f = file("world/A", &key);
        assert!(f.starts_with("map/world_A-"));
        assert!(f.contains("/minecraft_overworld-"));
        assert!(f.ends_with("/-1.3.map"));
    }

    #[test]
    fn remembers_and_reads_back_what_was_seen() {
        let mut files = Files::default();
        let mut memory = MapMemory::default();
        memory.set_zlib(PLAIN);
        memory.record(&mut files, "d", 0, 0, &chunk(0xFF00_FF00));
        assert!(memory.kept("d", 0, 0).is_none(), "no world open");
        memory.open("world/A", &mut files);
        memory.record(&mut files, "d", -1, 33, &chunk(0xFF00_FF00));
        let region = memory.kept("d", -1, 1).unwrap();
        assert_eq!((region.revision, region.color(31 * 16, 16)), (1, Some(0xFF00_FF00)));
        memory.record(&mut files, "d", -1, 33, &chunk(0xFF00_FF00));
        assert_eq!(memory.kept("d", -1, 1).unwrap().revision, 1, "the same colours change nothing");
        for _ in 0..SAVE_TICKS {
            memory.tick(&mut files);
        }
        assert_eq!(files.0.len(), 1);
        memory.close(&mut files);
        memory.open("world/A", &mut files);
        let read = memory.region(&mut files, "d", -1, 1, false).unwrap();
        assert_eq!(read.color(31 * 16, 16), Some(0xFF00_FF00));
        assert!(memory.region(&mut files, "d", 5, 5, false).is_none());
    }

    #[test]
    fn keeps_a_few_regions_saving_those_it_lets_go() {
        let mut files = Files::default();
        let mut memory = MapMemory::default();
        memory.set_zlib(PLAIN);
        memory.open("w", &mut files);
        for i in 0..KEPT as i32 + 3 {
            memory.record(&mut files, "d", i * 32, 0, &chunk(0xFF11_1111));
        }
        assert_eq!(memory.regions.len(), KEPT);
        assert_eq!(files.0.len(), 3);
        assert!(memory.kept("d", 0, 0).is_none());
    }

    #[test]
    fn forgets_with_remember_off_and_joins_the_file_when_turned_on() {
        let mut files = Files::default();
        let mut memory = MapMemory::default();
        memory.set_zlib(PLAIN);
        memory.open("w", &mut files);
        memory.record(&mut files, "d", 0, 0, &chunk(0xFF11_1111));
        memory.close(&mut files);
        memory.set_remember(false, &files);
        memory.open("w", &mut files);
        memory.record(&mut files, "d", 1, 0, &chunk(0xFF22_2222));
        memory.save(&mut files);
        let file = file("w", &Key { dimension: "d".into(), x: 0, z: 0 });
        assert!(Region::decode(&files.0[&file]).unwrap().chunk(1, 0).is_none(), "not saved while off");
        memory.set_remember(true, &files);
        memory.save(&mut files);
        let saved = Region::decode(&files.0[&file]).unwrap();
        assert!(saved.chunk(0, 0).is_some() && saved.chunk(1, 0).is_some());
    }
}
