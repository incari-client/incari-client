//! The game's binary NBT, gzipped or not, read into plain values: every schematic format is NBT.
//! A file is untrusted: each length is checked against what's left before anything is made, and
//! nesting is capped, so a bad file fails with a message instead of running the tab out of memory.
//!
//! This crate has no inflater: the game side hands one over (`Inflate`), as the map's zlib.

/// The most a file may unpack to: a 4 GB Chromebook has to hold it and the game.
pub const MAX_BYTES: usize = 192 << 20;

const MAX_DEPTH: usize = 512;

/// Raw deflate (no zlib or gzip framing) unpacked to at most `limit` bytes; None if it's broken or
/// unpacks to more.
pub type Inflate = fn(&[u8], usize) -> Option<Vec<u8>>;

/// One value.
#[derive(Clone, Debug, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Bytes(Vec<u8>),
    String(String),
    List(Vec<Tag>),
    Compound(Compound),
    Ints(Vec<i32>),
    Longs(Vec<i64>),
}

impl Tag {
    /// A whole number of any width.
    pub fn number(&self) -> Option<i64> {
        match *self {
            Tag::Byte(v) => Some(v.into()),
            Tag::Short(v) => Some(v.into()),
            Tag::Int(v) => Some(v.into()),
            Tag::Long(v) => Some(v),
            _ => None,
        }
    }

    /// The value as a block property is written: its text, or the number as text.
    pub fn text(&self) -> Option<String> {
        match self {
            Tag::String(s) => Some(s.clone()),
            Tag::Byte(_) | Tag::Short(_) | Tag::Int(_) | Tag::Long(_) => self.number().map(|n| n.to_string()),
            _ => None,
        }
    }
}

/// A compound: its fields in file order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Compound(pub Vec<(String, Tag)>);

impl Compound {
    pub fn get(&self, name: &str) -> Option<&Tag> {
        self.0.iter().find(|(n, _)| n == name).map(|(_, t)| t)
    }

    pub fn has(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// The field names, in file order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(n, _)| n.as_str())
    }

    pub fn compound(&self, name: &str) -> Option<&Compound> {
        match self.get(name) {
            Some(Tag::Compound(c)) => Some(c),
            _ => None,
        }
    }

    /// The field's list, or an empty one.
    pub fn list(&self, name: &str) -> &[Tag] {
        match self.get(name) {
            Some(Tag::List(l)) => l,
            _ => &[],
        }
    }

    /// The field's text, or `fallback` if it isn't text.
    pub fn string<'a>(&'a self, name: &str, fallback: &'a str) -> &'a str {
        match self.get(name) {
            Some(Tag::String(s)) => s,
            _ => fallback,
        }
    }

    /// The field as a whole number of any width, or `fallback`.
    pub fn number(&self, name: &str, fallback: i64) -> i64 {
        self.get(name).and_then(Tag::number).unwrap_or(fallback)
    }

    /// `number` as an i32 (cut as Java's cast does).
    pub fn int(&self, name: &str, fallback: i32) -> i32 {
        self.number(name, fallback.into()) as i32
    }

    /// The field's int array, or a list of ints (as structures keep positions).
    pub fn ints(&self, name: &str) -> Option<Vec<i32>> {
        match self.get(name)? {
            Tag::Ints(v) => Some(v.clone()),
            Tag::List(l) => l
                .iter()
                .map(|t| match t {
                    Tag::Int(v) => Some(*v),
                    _ => None,
                })
                .collect(),
            _ => None,
        }
    }

    pub fn longs(&self, name: &str) -> Option<&[i64]> {
        match self.get(name)? {
            Tag::Longs(v) => Some(v),
            _ => None,
        }
    }

    pub fn bytes(&self, name: &str) -> Option<&[u8]> {
        match self.get(name)? {
            Tag::Bytes(v) => Some(v),
            _ => None,
        }
    }
}

/// The root compound of a file's bytes, unpacked first if they're gzipped.
pub fn read(file: &[u8], inflate: Inflate) -> Result<Compound, String> {
    let unpacked;
    let data = if file.len() >= 2 && file[0] == 0x1F && file[1] == 0x8B {
        unpacked = gunzip(file, inflate)?;
        &unpacked[..]
    } else {
        file
    };
    let mut r = Reader { data, at: 0 };
    if r.u8()? != 10 {
        return Err("Not NBT: the root isn't a compound".into());
    }
    r.utf()?;
    r.compound(0)
}

/// A gzip member's deflated body unpacked (RFC 1952's header skipped).
fn gunzip(file: &[u8], inflate: Inflate) -> Result<Vec<u8>, String> {
    const BAD: &str = "Not gzip, or a broken one";
    if file.len() < 18 || file[2] != 8 {
        return Err(BAD.into());
    }
    let flags = file[3];
    let mut at = 10;
    if flags & 4 != 0 {
        // FEXTRA: a length, then that much.
        let n = usize::from(*file.get(at).ok_or(BAD)?) | usize::from(*file.get(at + 1).ok_or(BAD)?) << 8;
        at += 2 + n;
    }
    for flag in [8, 16] {
        // FNAME, FCOMMENT: zero-terminated.
        if flags & flag != 0 {
            let end = file.get(at..).ok_or(BAD)?.iter().position(|&b| b == 0).ok_or(BAD)?;
            at += end + 1;
        }
    }
    if flags & 2 != 0 {
        at += 2;
    }
    let body = file.get(at..file.len().saturating_sub(8)).ok_or(BAD)?;
    inflate(body, MAX_BYTES).ok_or_else(|| format!("Broken, or over {} MB unpacked", MAX_BYTES >> 20))
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn value(&mut self, kind: u8, depth: usize) -> Result<Tag, String> {
        Ok(match kind {
            1 => Tag::Byte(self.u8()? as i8),
            2 => Tag::Short(self.take(2)?.try_into().map(i16::from_be_bytes).unwrap_or(0)),
            3 => Tag::Int(self.s32()?),
            4 => Tag::Long(self.s64()?),
            5 => Tag::Float(f32::from_bits(self.s32()? as u32)),
            6 => Tag::Double(f64::from_bits(self.s64()? as u64)),
            7 => {
                let n = self.count(1)?;
                Tag::Bytes(self.take(n)?.to_vec())
            }
            8 => Tag::String(self.utf()?),
            9 => Tag::List(self.list(depth)?),
            10 => Tag::Compound(self.compound(depth)?),
            11 => {
                let n = self.count(4)?;
                Tag::Ints(self.take(n * 4)?.as_chunks::<4>().0.iter().map(|c| i32::from_be_bytes(*c)).collect())
            }
            12 => {
                let n = self.count(8)?;
                Tag::Longs(self.take(n * 8)?.as_chunks::<8>().0.iter().map(|c| i64::from_be_bytes(*c)).collect())
            }
            _ => return Err(format!("Bad NBT: tag type {kind}")),
        })
    }

    fn compound(&mut self, depth: usize) -> Result<Compound, String> {
        if depth > MAX_DEPTH {
            return Err("Bad NBT: nested too deep".into());
        }
        let mut fields = Vec::new();
        loop {
            let kind = self.u8()?;
            if kind == 0 {
                return Ok(Compound(fields));
            }
            let name = self.utf()?;
            let value = self.value(kind, depth + 1)?;
            fields.push((name, value));
        }
    }

    fn list(&mut self, depth: usize) -> Result<Vec<Tag>, String> {
        if depth > MAX_DEPTH {
            return Err("Bad NBT: nested too deep".into());
        }
        let kind = self.u8()?;
        // The smallest any element can be is one byte (an empty compound): a list of nothing
        // must be empty.
        let n = self.count(if kind == 0 { 0 } else { 1 })?;
        if kind == 0 && n > 0 {
            return Err("Bad NBT: a list of nothing".into());
        }
        let mut list = Vec::with_capacity(n.min(1 << 16));
        for _ in 0..n {
            list.push(self.value(kind, depth + 1)?);
        }
        Ok(list)
    }

    /// A length prefix for `size`-byte elements, checked against what's left.
    fn count(&mut self, size: usize) -> Result<usize, String> {
        let n = self.s32()?;
        let left = self.data.len() - self.at;
        if n < 0 || (n as usize).saturating_mul(size) > left {
            return Err(format!("Bad NBT: {n} elements past the end"));
        }
        Ok(n as usize)
    }

    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        if self.data.len() - self.at < n {
            return Err("Bad NBT: ends early".into());
        }
        let s = &self.data[self.at..self.at + n];
        self.at += n;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn s32(&mut self) -> Result<i32, String> {
        let b = self.take(4)?;
        Ok(i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn s64(&mut self) -> Result<i64, String> {
        let high = self.s32()? as u32 as u64;
        let low = self.s32()? as u32 as u64;
        Ok((high << 32 | low) as i64)
    }

    /// Java's modified UTF-8, as the game writes names and text: UTF-8 for real text.
    fn utf(&mut self) -> Result<String, String> {
        let b = self.take(2)?;
        let n = usize::from(u16::from_be_bytes([b[0], b[1]]));
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
}

/// Writes NBT, for the tests' fixture files.
#[cfg(test)]
pub mod write {
    use super::{Compound, Tag};

    fn kind(t: &Tag) -> u8 {
        match t {
            Tag::Byte(_) => 1,
            Tag::Short(_) => 2,
            Tag::Int(_) => 3,
            Tag::Long(_) => 4,
            Tag::Float(_) => 5,
            Tag::Double(_) => 6,
            Tag::Bytes(_) => 7,
            Tag::String(_) => 8,
            Tag::List(_) => 9,
            Tag::Compound(_) => 10,
            Tag::Ints(_) => 11,
            Tag::Longs(_) => 12,
        }
    }

    fn utf(out: &mut Vec<u8>, s: &str) {
        out.extend((s.len() as u16).to_be_bytes());
        out.extend(s.as_bytes());
    }

    fn value(out: &mut Vec<u8>, t: &Tag) {
        match t {
            Tag::Byte(v) => out.push(*v as u8),
            Tag::Short(v) => out.extend(v.to_be_bytes()),
            Tag::Int(v) => out.extend(v.to_be_bytes()),
            Tag::Long(v) => out.extend(v.to_be_bytes()),
            Tag::Float(v) => out.extend(v.to_be_bytes()),
            Tag::Double(v) => out.extend(v.to_be_bytes()),
            Tag::Bytes(v) => {
                out.extend((v.len() as i32).to_be_bytes());
                out.extend(v);
            }
            Tag::String(s) => utf(out, s),
            Tag::List(l) => {
                out.push(l.first().map_or(0, kind));
                out.extend((l.len() as i32).to_be_bytes());
                l.iter().for_each(|t| value(out, t));
            }
            Tag::Compound(c) => compound(out, c),
            Tag::Ints(v) => {
                out.extend((v.len() as i32).to_be_bytes());
                v.iter().for_each(|i| out.extend(i.to_be_bytes()));
            }
            Tag::Longs(v) => {
                out.extend((v.len() as i32).to_be_bytes());
                v.iter().for_each(|i| out.extend(i.to_be_bytes()));
            }
        }
    }

    fn compound(out: &mut Vec<u8>, c: &Compound) {
        for (name, t) in &c.0 {
            out.push(kind(t));
            utf(out, name);
            value(out, t);
        }
        out.push(0);
    }

    /// A root compound, unnamed, as the game saves files.
    pub fn file(root: &Compound) -> Vec<u8> {
        let mut out = vec![10];
        utf(&mut out, "");
        compound(&mut out, root);
        out
    }

    /// `data` gzipped with stored (uncompressed) deflate blocks: enough for `stored_inflate`.
    pub fn gzip(data: &[u8]) -> Vec<u8> {
        // FNAME set, to test the header's skipping.
        let mut out = vec![0x1F, 0x8B, 8, 8, 0, 0, 0, 0, 0, 255];
        out.extend(b"castle.nbt\0");
        let chunks: Vec<&[u8]> = if data.is_empty() { vec![&[]] } else { data.chunks(65535).collect() };
        for (i, chunk) in chunks.iter().enumerate() {
            out.push(u8::from(i == chunks.len() - 1));
            let n = chunk.len() as u16;
            out.extend(n.to_le_bytes());
            out.extend((!n).to_le_bytes());
            out.extend(*chunk);
        }
        out.extend([0; 4]); // CRC, unchecked here
        out.extend((data.len() as u32).to_le_bytes());
        out
    }

    /// Raw deflate of stored blocks only: what `gzip` writes.
    pub fn stored_inflate(data: &[u8], limit: usize) -> Option<Vec<u8>> {
        let mut out = Vec::new();
        let mut at = 0;
        loop {
            let last = *data.get(at)? & 1 == 1;
            if data[at] >> 1 & 3 != 0 {
                return None;
            }
            let n = usize::from(u16::from_le_bytes([*data.get(at + 1)?, *data.get(at + 2)?]));
            out.extend(data.get(at + 5..at + 5 + n)?);
            if out.len() > limit {
                return None;
            }
            at += 5 + n;
            if last {
                return Some(out);
            }
        }
    }

    /// Shorthands for building compounds.
    pub fn c(fields: Vec<(&str, Tag)>) -> Tag {
        Tag::Compound(Compound(fields.into_iter().map(|(n, t)| (n.to_owned(), t)).collect()))
    }

    pub fn s(text: &str) -> Tag {
        Tag::String(text.to_owned())
    }

    pub fn root(t: Tag) -> Compound {
        match t {
            Tag::Compound(c) => c,
            _ => panic!("not a compound"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::write::*;
    use super::*;

    fn none(_: &[u8], _: usize) -> Option<Vec<u8>> {
        None
    }

    #[test]
    fn reads_what_it_wrote_plain_and_gzipped() {
        let root = root(c(vec![
            ("Name", s("Castle")),
            ("n", Tag::Short(-3)),
            ("l", Tag::Longs(vec![-1, 5])),
            ("list", Tag::List(vec![Tag::Int(1), Tag::Int(2)])),
            ("inner", c(vec![("b", Tag::Byte(7))])),
            ("empty", Tag::List(vec![])),
            ("d", Tag::Double(1.5)),
        ]));
        let bytes = file(&root);
        assert_eq!(read(&bytes, none).unwrap(), root);
        assert_eq!(read(&gzip(&bytes), stored_inflate).unwrap(), root);
        let r = read(&bytes, none).unwrap();
        assert_eq!(r.string("Name", ""), "Castle");
        assert_eq!(r.int("n", 0), -3);
        assert_eq!(r.ints("list"), Some(vec![1, 2]));
        assert_eq!(r.compound("inner").unwrap().number("b", 0), 7);
    }

    #[test]
    fn refuses_broken_files_without_making_huge_things() {
        assert!(read(&[], none).is_err());
        assert!(read(&[8, 0, 0], none).is_err());
        // A long array claiming two billion entries in a few bytes.
        let mut bad = vec![10, 0, 0, 12, 0, 1, b'l'];
        bad.extend(0x7FFF_FFFFi32.to_be_bytes());
        assert!(read(&bad, none).unwrap_err().contains("past the end"));
        // Cut short.
        let good = file(&root(c(vec![("Name", s("Castle"))])));
        assert!(read(&good[..good.len() - 3], none).is_err());
        // Gzip the inflater can't unpack.
        assert!(read(&gzip(&good), none).is_err());
    }
}
