//! The schematic files the client reads, told apart by what's in them (ADR 0066):
//!
//! - `.litematic`: gzipped NBT with one or more regions, each its own box with its own palette,
//!   merged here into one box around them all. A region's size may be negative on an axis: it then
//!   runs the other way from its position. Its blocks are palette indices packed tightly into
//!   longs, `max(2, ceil(log2(palette)))` bits each, in the order `(y * sz + z) * sx + x`. Where
//!   the regions don't reach, nothing is wanted.
//! - Sponge `.schem` (WorldEdit's, Axiom's), versions 1 to 3: a palette of the game's state text to
//!   numbers, the blocks as those numbers in variable-length bytes, `(y * length + z) * width + x`.
//!   Version 3 moves both into a `Blocks` compound and the whole under a `Schematic` one.
//! - The game's structure `.nbt`: a size, a palette (or several: the first is used, as the game
//!   does for shipwrecks) and a list of the blocks there are; a place not listed is its void.
//!
//! Pre-1.13 MCEdit `.schematic` files are refused with a message.
use super::data::{Builder, MAX_VOLUME, Schematic};
use super::nbt::{self, Compound, Inflate, Tag};
use super::spec::BlockSpec;

/// The files the chooser lists.
pub const EXTENSIONS: [&str; 3] = [".litematic", ".schem", ".nbt"];

/// What version 1 Sponge files were written at, as they don't say: 1.13.2.
const SPONGE_V1_DATA: i32 = 1631;

/// The schematic in `bytes`, a file called `file_name`; Err says why not in the player's words.
pub fn read(file_name: &str, bytes: &[u8], inflate: Inflate) -> Result<Schematic, String> {
    let root = nbt::read(bytes, inflate).map_err(|_| "Not a schematic, or a broken one".to_owned())?;
    let name = base_name(file_name);
    if root.compound("Regions").is_some() {
        return litematic(&root, name);
    }
    if let Some(body) = sponge_body(&root) {
        return sponge(body, name);
    }
    if root.has("size") && root.has("blocks") {
        return structure(&root, name);
    }
    if root.has("Materials") || root.has("Blocks") {
        return Err("Old schematics from before 1.13 aren't supported".into());
    }
    Err("Not a schematic this client knows".into())
}

/// `castle.litematic` without its folder or extension.
pub fn base_name(file_name: &str) -> &str {
    let name = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    match name.rfind('.') {
        Some(dot) if dot > 0 && EXTENSIONS.contains(&name[dot..].to_ascii_lowercase().as_str()) => &name[..dot],
        _ => name,
    }
}

/// Whether the chooser lists `name`.
pub fn accepted(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    EXTENSIONS.iter().any(|e| lower.ends_with(e))
}

fn xyz(tag: Option<&Compound>) -> Option<[i32; 3]> {
    let t = tag?;
    (t.has("x") && t.has("y") && t.has("z")).then(|| [t.int("x", 0), t.int("y", 0), t.int("z", 0)])
}

/// A palette entry: `Name` and `Properties`, or the newer `id` and `properties`.
fn spec(entry: &Compound) -> BlockSpec {
    let name = entry.string("Name", entry.string("id", "minecraft:air"));
    let props = entry.compound("Properties").or_else(|| entry.compound("properties"));
    let properties = props
        .map(|p| p.0.iter().filter_map(|(k, v)| Some((k.clone(), v.text()?))).collect())
        .unwrap_or_default();
    BlockSpec::of(name, properties)
}

fn named<'a>(metadata: Option<&'a Compound>, fallback: &'a str) -> (&'a str, &'a str) {
    let name = metadata.map_or("", |m| m.string("Name", "").trim());
    (if name.is_empty() { fallback } else { name }, metadata.map_or("", |m| m.string("Author", "")))
}

struct Region<'a> {
    min: [i64; 3],
    size: [i32; 3],
    tag: &'a Compound,
}

fn litematic(root: &Compound, fallback: &str) -> Result<Schematic, String> {
    let regions = root.compound("Regions").ok_or("Not a litematic: no regions")?;
    let mut boxes = Vec::new();
    let (mut lo, mut hi) = ([i64::MAX; 3], [i64::MIN; 3]);
    for (name, tag) in &regions.0 {
        let Tag::Compound(region) = tag else {
            continue;
        };
        let (Some(position), Some(size)) = (xyz(region.compound("Position")), xyz(region.compound("Size"))) else {
            return Err(format!("Region {name} has no position or size"));
        };
        if size.contains(&0) {
            continue;
        }
        // The lower end on each axis: the position, or past it when the size is negative.
        let min: [i64; 3] =
            std::array::from_fn(|i| i64::from(position[i]) + if size[i] < 0 { i64::from(size[i]) + 1 } else { 0 });
        let abs = size.map(|s| s.unsigned_abs() as i32);
        if abs.iter().map(|&s| s as u64).product::<u64>() > MAX_VOLUME {
            return Err(format!("Region {name} is too big to load"));
        }
        for i in 0..3 {
            lo[i] = lo[i].min(min[i]);
            hi[i] = hi[i].max(min[i] + i64::from(abs[i]));
        }
        boxes.push(Region { min, size: abs, tag: region });
    }
    if boxes.is_empty() {
        return Err("It has no blocks".into());
    }
    let span: [i64; 3] = std::array::from_fn(|i| hi[i] - lo[i]);
    if span.iter().any(|&s| s > i64::from(i32::MAX)) {
        return Err("Too big to load".into());
    }
    let mut b = Builder::new(span[0] as i32, span[1] as i32, span[2] as i32)?;
    let (name, author) = named(root.compound("Metadata"), fallback);
    b.describe(name, author, "Litematic", root.int("MinecraftDataVersion", 0));
    for r in &boxes {
        let offset: [i32; 3] = std::array::from_fn(|i| (r.min[i] - lo[i]) as i32);
        region(&mut b, r, offset)?;
    }
    Ok(b.build())
}

fn region(b: &mut Builder, r: &Region, offset: [i32; 3]) -> Result<(), String> {
    let entries = r.tag.list("BlockStatePalette");
    if entries.is_empty() {
        return Err("A region has no palette".into());
    }
    let indices: Vec<usize> = entries
        .iter()
        .map(|e| match e {
            Tag::Compound(c) => b.index(spec(c)),
            _ => 0,
        })
        .collect();
    let [sx, sy, sz] = r.size;
    let volume = sx as usize * sy as usize * sz as usize;
    let bits = super::packed::bits_for(indices.len(), 2);
    let states = r.tag.longs("BlockStates").unwrap_or(&[]);
    let packed = super::packed::Packed::over(bits, volume, states.iter().map(|&l| l as u64).collect())
        .ok_or("A region's blocks are cut short")?;
    let mut i = 0;
    for y in 0..sy {
        for z in 0..sz {
            for x in 0..sx {
                let index = packed.get(i) as usize;
                i += 1;
                let Some(&to) = indices.get(index) else {
                    return Err("A region's blocks are past its palette".into());
                };
                b.set(offset[0] + x, offset[1] + y, offset[2] + z, to);
            }
        }
    }
    Ok(())
}

/// The compound holding a Sponge schematic: the root, or version 3's `Schematic` in it.
fn sponge_body(root: &Compound) -> Option<&Compound> {
    if let Some(inner) = root.compound("Schematic")
        && inner.has("Width")
    {
        return Some(inner);
    }
    (root.has("Width") && root.has("Palette")).then_some(root)
}

fn sponge(body: &Compound, fallback: &str) -> Result<Schematic, String> {
    // Sizes are unsigned shorts: a 40000-long schematic reads as negative.
    let unsigned = |v: i32| if v < 0 { v & 0xFFFF } else { v };
    let (w, h, l) = (unsigned(body.int("Width", 0)), unsigned(body.int("Height", 0)), unsigned(body.int("Length", 0)));
    let blocks = body.compound("Blocks");
    let palette = blocks.map_or_else(|| body.compound("Palette"), |b| b.compound("Palette"));
    let data = blocks.map_or_else(|| body.bytes("BlockData"), |b| b.bytes("Data"));
    let (Some(palette), Some(data)) = (palette, data) else {
        return Err("It has no palette or blocks".into());
    };
    let mut b = Builder::new(w, h, l)?;
    let version = body.int("Version", 1);
    if !(1..=3).contains(&version) {
        return Err(format!("Sponge version {version} isn't known"));
    }
    let (name, author) = named(body.compound("Metadata"), fallback);
    b.describe(name, author, "Sponge", body.int("DataVersion", SPONGE_V1_DATA));
    let highest = palette.0.iter().map(|(_, v)| v.number().unwrap_or(-1)).max().unwrap_or(-1);
    if !(0..=1 << 20).contains(&highest) {
        return Err("Its palette is empty or broken".into());
    }
    let mut indices = vec![0usize; highest as usize + 1];
    for (key, v) in &palette.0 {
        if let Some(n) = v.number().filter(|&n| n >= 0) {
            indices[n as usize] = b.index(BlockSpec::parse(key));
        }
    }
    let mut at = 0;
    for y in 0..h {
        for z in 0..l {
            for x in 0..w {
                // A variable-length number: seven bits a byte, low first, the top bit for "more".
                let mut value = 0usize;
                let mut shift = 0;
                loop {
                    let Some(&byte) = data.get(at) else {
                        return Err("Its blocks are cut short".into());
                    };
                    at += 1;
                    value |= usize::from(byte & 0x7F) << shift;
                    if byte & 0x80 == 0 {
                        break;
                    }
                    shift += 7;
                    if shift > 28 {
                        return Err("Its blocks are broken".into());
                    }
                }
                let Some(&to) = indices.get(value) else {
                    return Err("Its blocks are past its palette".into());
                };
                b.set(x, y, z, to);
            }
        }
    }
    Ok(b.build())
}

fn structure(root: &Compound, name: &str) -> Result<Schematic, String> {
    let size = root.ints("size").filter(|s| s.len() == 3).ok_or("It has no size")?;
    let mut palette = root.list("palette");
    if palette.is_empty()
        && let Some(Tag::List(first)) = root.list("palettes").first()
    {
        palette = first;
    }
    if palette.is_empty() {
        return Err("It has no palette".into());
    }
    let mut b = Builder::new(size[0], size[1], size[2])?;
    b.describe(name, root.string("author", ""), "Structure", root.int("DataVersion", 0));
    let indices: Vec<usize> = palette
        .iter()
        .map(|e| match e {
            Tag::Compound(c) => b.index(spec(c)),
            _ => 0,
        })
        .collect();
    for block in root.list("blocks") {
        let Tag::Compound(block) = block else {
            continue;
        };
        let pos = block.ints("pos").filter(|p| p.len() == 3);
        let state = block.int("state", -1);
        let (Some(pos), true) = (pos, state >= 0 && (state as usize) < indices.len()) else {
            return Err("A block is broken".into());
        };
        if !b.contains(pos[0], pos[1], pos[2]) {
            return Err("A block is outside its size".into());
        }
        b.set(pos[0], pos[1], pos[2], indices[state as usize]);
    }
    Ok(b.build())
}

#[cfg(test)]
pub mod fixtures {
    //! Small schematic files built in the tests, one of each format.
    use super::super::nbt::Tag;
    use super::super::nbt::write::*;

    /// `values`, `bits` each, packed as a litematic does: bit by bit, entry `i`'s bit `b` at
    /// stream bit `i * bits + b`, low bits of each long first (written apart from `Packed`).
    pub fn pack(values: &[u32], bits: u32) -> Vec<i64> {
        (0..super::super::packed::words_for(bits, values.len()))
            .map(|w| {
                let mut word = 0u64;
                for bit in 0..64 {
                    let at = w * 64 + bit;
                    let (entry, offset) = (at / bits as usize, at % bits as usize);
                    if entry < values.len() && values[entry] >> offset & 1 == 1 {
                        word |= 1 << bit;
                    }
                }
                word as i64
            })
            .collect()
    }

    fn entry(name: &str, props: &[(&str, &str)]) -> Tag {
        if props.is_empty() {
            c(vec![("Name", s(name))])
        } else {
            c(vec![("Name", s(name)), ("Properties", c(props.iter().map(|(k, v)| (*k, s(v))).collect()))])
        }
    }

    fn xyz(v: [i32; 3]) -> Tag {
        c(vec![("x", Tag::Int(v[0])), ("y", Tag::Int(v[1])), ("z", Tag::Int(v[2]))])
    }

    /// A litematic region: palette `[air, ...]`, blocks given by index in `(y*sz+z)*sx+x` order.
    pub fn region(position: [i32; 3], size: [i32; 3], palette: &[(&str, &[(&str, &str)])], blocks: &[u32]) -> Tag {
        let bits = super::super::packed::bits_for(palette.len(), 2);
        c(vec![
            ("Position", xyz(position)),
            ("Size", xyz(size)),
            ("BlockStatePalette", Tag::List(palette.iter().map(|(n, p)| entry(n, p)).collect())),
            ("BlockStates", Tag::Longs(pack(blocks, bits))),
            ("TileEntities", Tag::List(vec![])),
        ])
    }

    pub fn litematic(name: &str, regions: Vec<(&str, Tag)>) -> Vec<u8> {
        gzip(&file(&root(c(vec![
            ("Version", Tag::Int(6)),
            ("MinecraftDataVersion", Tag::Int(4556)),
            ("Metadata", c(vec![("Name", s(name)), ("Author", s("Tester"))])),
            ("Regions", c(regions)),
        ]))))
    }

    /// A Sponge v`version` schematic, `w` x `h` x `l`, blocks as palette numbers.
    pub fn sponge(version: i32, size: [i32; 3], palette: &[(&str, i32)], blocks: &[u32]) -> Vec<u8> {
        let mut data = Vec::new();
        for &v in blocks {
            let mut v = v;
            loop {
                let byte = (v & 0x7F) as u8;
                v >>= 7;
                if v == 0 {
                    data.push(byte);
                    break;
                }
                data.push(byte | 0x80);
            }
        }
        let pal = c(palette.iter().map(|(k, n)| (*k, Tag::Int(*n))).collect());
        let sizes = vec![
            ("Version", Tag::Int(version)),
            ("DataVersion", Tag::Int(3465)),
            ("Width", Tag::Short(size[0] as i16)),
            ("Height", Tag::Short(size[1] as i16)),
            ("Length", Tag::Short(size[2] as i16)),
        ];
        let body = if version == 3 {
            let mut f = sizes;
            f.push(("Blocks", c(vec![("Palette", pal), ("Data", Tag::Bytes(data))])));
            c(vec![("Schematic", c(f))])
        } else {
            let mut f = sizes;
            f.push(("Palette", pal));
            f.push(("BlockData", Tag::Bytes(data)));
            f.push(("Metadata", c(vec![("Name", s("Hut"))])));
            c(f)
        };
        gzip(&file(&root(body)))
    }

    /// A structure `.nbt` (not gzipped: the game's are, but either reads).
    pub fn structure(size: [i32; 3], palette: &[(&str, &[(&str, &str)])], blocks: &[([i32; 3], i32)]) -> Vec<u8> {
        file(&root(c(vec![
            ("DataVersion", Tag::Int(3953)),
            ("size", Tag::List(size.iter().map(|&v| Tag::Int(v)).collect())),
            ("palette", Tag::List(palette.iter().map(|(n, p)| entry(n, p)).collect())),
            (
                "blocks",
                Tag::List(
                    blocks
                        .iter()
                        .map(|(p, st)| {
                            c(vec![
                                ("pos", Tag::List(p.iter().map(|&v| Tag::Int(v)).collect())),
                                ("state", Tag::Int(*st)),
                            ])
                        })
                        .collect(),
                ),
            ),
            ("entities", Tag::List(vec![])),
        ])))
    }
}

#[cfg(test)]
mod tests {
    use super::super::nbt::write::{c, file, gzip, root, stored_inflate};
    use super::fixtures::*;
    use super::{Schematic, Tag, accepted, base_name, read};

    fn name_at(s: &Schematic, x: i32, y: i32, z: i32) -> String {
        s.palette[s.at(x, y, z)].to_string()
    }

    #[test]
    fn packed_entries_match_the_packer() {
        for bits in 1..=32 {
            let values: Vec<u32> = (0..300u64).map(|i| ((i * 2654435761) & ((1 << bits) - 1)) as u32).collect();
            let words: Vec<u64> = pack(&values, bits).into_iter().map(|w| w as u64).collect();
            let p = super::super::packed::Packed::over(bits, values.len(), words).unwrap();
            for (i, &v) in values.iter().enumerate() {
                assert_eq!(p.get(i), v, "bits {bits}, entry {i}");
            }
        }
    }

    #[test]
    fn reads_a_litematic_merging_its_regions() {
        const STONE: &[(&str, &str)] = &[];
        const STAIRS: &[(&str, &str)] = &[("facing", "east"), ("half", "bottom")];
        // A 2x1x2 region at the origin, and a 1x2x1 one running the other way from x = 3.
        let a = region(
            [0, 0, 0],
            [2, 1, 2],
            &[("minecraft:air", STONE), ("minecraft:stone", STONE), ("minecraft:oak_stairs", STAIRS)],
            &[1, 2, 0, 1],
        );
        let b = region([3, 0, 0], [-1, 2, 1], &[("minecraft:air", STONE), ("dirt", STONE)], &[1, 1]);
        let bytes = litematic("Castle", vec![("Main", a), ("Tower", b)]);
        let s = read("x/castle.litematic", &bytes, stored_inflate).unwrap();
        assert_eq!((s.name.as_str(), s.author.as_str(), s.format, s.data_version), ("Castle", "Tester", "Litematic", 4556));
        assert_eq!(s.size, [4, 2, 2]);
        assert_eq!(name_at(&s, 0, 0, 0), "minecraft:stone");
        assert_eq!(name_at(&s, 1, 0, 0), "minecraft:oak_stairs[facing=east,half=bottom]");
        assert_eq!(name_at(&s, 0, 0, 1), "minecraft:air");
        assert_eq!(name_at(&s, 3, 1, 0), "minecraft:dirt");
        // Between the regions, and above the first: nothing wanted.
        assert!(s.palette[s.at(2, 0, 0)].is_nothing());
        assert!(s.palette[s.at(0, 1, 0)].is_nothing());
        assert_eq!(s.block_count(), 5);
    }

    #[test]
    fn reads_sponge_versions_and_a_structure() {
        let palette = [("minecraft:air", 0), ("minecraft:oak_log[axis=y]", 1), ("minecraft:glass", 200)];
        // 200 takes two bytes as a variable-length number.
        for version in [1, 2, 3] {
            let s = read("hut.schem", &sponge(version, [2, 1, 2], &palette, &[1, 200, 0, 1]), stored_inflate).unwrap();
            assert_eq!(s.size, [2, 1, 2]);
            assert_eq!(name_at(&s, 0, 0, 0), "minecraft:oak_log[axis=y]");
            assert_eq!(name_at(&s, 1, 0, 0), "minecraft:glass");
            assert_eq!(name_at(&s, 1, 0, 1), "minecraft:oak_log[axis=y]");
            assert_eq!(s.name, if version == 3 { "hut" } else { "Hut" });
            assert_eq!(s.format, "Sponge");
        }
        const NONE: &[(&str, &str)] = &[];
        let bytes = structure([2, 2, 1], &[("minecraft:stone", NONE), ("minecraft:torch", NONE)], &[([0, 0, 0], 0), ([1, 1, 0], 1)]);
        let s = read("well.nbt", &bytes, stored_inflate).unwrap();
        assert_eq!((s.name.as_str(), s.format), ("well", "Structure"));
        assert_eq!(name_at(&s, 1, 1, 0), "minecraft:torch");
        assert!(s.palette[s.at(1, 0, 0)].is_nothing());
    }

    #[test]
    fn refuses_what_it_cant_read() {
        let old = gzip(&file(&root(c(vec![("Materials", super::super::nbt::write::s("Alpha")), ("Blocks", Tag::Bytes(vec![]))]))));
        assert!(read("old.schematic", &old, stored_inflate).unwrap_err().contains("before 1.13"));
        assert!(read("a.nbt", b"hello", stored_inflate).unwrap_err().contains("broken"));
        let other = file(&root(c(vec![("Data", Tag::Int(1))])));
        assert!(read("a.nbt", &other, stored_inflate).unwrap_err().contains("knows"));
        // Blocks past the palette, a structure block outside its size, blocks cut short.
        let bad = region([0, 0, 0], [1, 1, 1], &[("minecraft:air", &[])], &[3]);
        assert!(read("a.litematic", &litematic("A", vec![("R", bad)]), stored_inflate).is_err());
        let outside = structure([1, 1, 1], &[("minecraft:stone", &[])], &[([1, 0, 0], 0)]);
        assert!(read("a.nbt", &outside, stored_inflate).unwrap_err().contains("outside"));
        let short = sponge(2, [2, 2, 2], &[("minecraft:stone", 0)], &[0, 0]);
        assert!(read("a.schem", &short, stored_inflate).unwrap_err().contains("cut short"));
    }

    #[test]
    fn names_and_extensions() {
        assert_eq!(base_name("schematics/Castle.LITEMATIC"), "Castle");
        assert_eq!(base_name("C:\\x\\hut.schem"), "hut");
        assert_eq!(base_name("notes.txt"), "notes.txt");
        assert_eq!(base_name(".nbt"), ".nbt");
        assert!(accepted("a.Schem") && accepted("b.nbt") && !accepted("c.schematic"));
    }
}
