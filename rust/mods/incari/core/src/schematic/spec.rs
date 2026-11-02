//! A block state as a file names it: `minecraft:oak_stairs` and its properties (`facing=north`),
//! not yet looked up in the game. Properties are kept sorted, so the same state is equal however a
//! file ordered them.

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct BlockSpec {
    /// The block's id with its namespace; empty for no block wanted (`NOTHING`).
    pub name: String,
    /// Sorted by name; empty for the block's defaults.
    pub properties: Vec<(String, String)>,
}

impl BlockSpec {
    /// No block wanted: a structure's void, outside a schematic's regions. Never looked up.
    pub const NOTHING: BlockSpec = BlockSpec { name: String::new(), properties: Vec::new() };

    /// `name` with the `minecraft:` namespace if it has none.
    pub fn of(name: &str, mut properties: Vec<(String, String)>) -> Self {
        let id = name.trim();
        let name = if !id.is_empty() && !id.contains(':') { format!("minecraft:{id}") } else { id.to_owned() };
        properties.sort();
        properties.dedup_by(|a, b| a.0 == b.0);
        Self { name, properties }
    }

    /// Parses the game's text for a state, `minecraft:oak_stairs[facing=north,half=bottom]`, as
    /// Sponge schematics keep their palette. Anything after the closing bracket is ignored.
    pub fn parse(text: &str) -> Self {
        let Some(open) = text.find('[') else {
            return Self::of(text, Vec::new());
        };
        let rest = &text[open + 1..];
        let inner = rest.find(']').map_or(rest, |close| &rest[..close]);
        let properties = inner
            .split(',')
            .filter_map(|pair| {
                let (k, v) = pair.split_once('=')?;
                let k = k.trim();
                (!k.is_empty()).then(|| (k.to_owned(), v.trim().to_owned()))
            })
            .collect();
        Self::of(&text[..open], properties)
    }

    /// Air of any kind.
    pub fn is_air(&self) -> bool {
        matches!(self.name.as_str(), "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
    }

    /// Whether it stands for no block wanted.
    pub fn is_nothing(&self) -> bool {
        self.name.is_empty()
    }

    /// One property's value, or `fallback`.
    pub fn property<'a>(&'a self, key: &str, fallback: &'a str) -> &'a str {
        self.properties.iter().find(|(k, _)| k == key).map_or(fallback, |(_, v)| v.as_str())
    }
}

impl std::fmt::Display for BlockSpec {
    /// The game's text for it.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)?;
        if !self.properties.is_empty() {
            f.write_str("[")?;
            for (i, (k, v)) in self.properties.iter().enumerate() {
                if i > 0 {
                    f.write_str(",")?;
                }
                write!(f, "{k}={v}")?;
            }
            f.write_str("]")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_prints_the_games_text() {
        let s = BlockSpec::parse("oak_stairs[half=bottom, facing=north]");
        assert_eq!(s.name, "minecraft:oak_stairs");
        assert_eq!(s.property("facing", ""), "north");
        assert_eq!(s.to_string(), "minecraft:oak_stairs[facing=north,half=bottom]");
        assert_eq!(s, BlockSpec::parse("minecraft:oak_stairs[facing=north,half=bottom]"));
        assert_eq!(BlockSpec::parse("minecraft:stone").to_string(), "minecraft:stone");
        assert_eq!(BlockSpec::parse("mod:thing[a=1").property("a", ""), "1");
        assert!(BlockSpec::parse("cave_air").is_air());
        assert!(BlockSpec::NOTHING.is_nothing());
    }
}
