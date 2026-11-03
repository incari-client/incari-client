//! Schematics (features/schematics, ADR 0066): a structure from a file, placed in the world as
//! see-through ghost blocks to build by. Written clean-room: it reads Litematica's files, but none
//! of its code. Everything here is plain logic with no game types: the readers (`formats`), the
//! placement, the comparison with the world (`verifier`), the material list and the module's
//! state (`session`); the game side hands it the game's blocks (`Blocks`) and draws the ghosts.
pub mod data;
pub mod formats;
pub mod hud;
pub mod materials;
pub mod nbt;
pub mod packed;
pub mod placement;
pub mod session;
pub mod spec;
pub mod verifier;

/// The module's id, as the Java one.
pub const ID: &str = "schematics";

/// Where in the game folder the files are.
pub const FOLDER: &str = "schematics";

/// How a structure is mirrored before it's turned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mirror {
    /// As built.
    #[default]
    None,
    /// East and west swap: x runs the other way (the game's FRONT_BACK).
    X,
    /// North and south swap: z runs the other way (the game's LEFT_RIGHT).
    Z,
}

/// A state the game doesn't have, or a block the client can't see (an unloaded chunk, out of the
/// world's height).
pub const UNKNOWN: u32 = u32::MAX;

/// The game's block states and the world's blocks, as the module sees them (framework's
/// `Blocks`): a state is a number the game hands out, the same for the same state while it runs;
/// never saved. Reads of the world must not block: a block in a chunk the client hasn't loaded is
/// `UNKNOWN`.
pub trait Blocks {
    /// The state `name` (`minecraft:oak_stairs`) with these properties, saved by the game at
    /// `data_version`; properties left out take the block's defaults, ones that don't fit the block
    /// are left at them. `UNKNOWN` if the game has no such block. Called once per palette entry.
    fn state(&self, name: &str, properties: &[(String, String)], data_version: i32) -> u32;
    /// `state` mirrored, then turned `turns` quarter turns clockwise seen from above, as the game
    /// turns a structure's blocks.
    fn transform(&self, state: u32, mirror: Mirror, turns: u8) -> u32;
    /// The block at `x, y, z` in the player's world; `UNKNOWN` where the client has none.
    fn at(&self, x: i32, y: i32, z: i32) -> u32;
    /// Air of any kind (cave air too).
    fn is_air(&self, state: u32) -> bool;
    /// One number per block, whatever its state: two states are of the same block when equal.
    fn block(&self, state: u32) -> u32;
    /// The id of the item that places `state` (`minecraft:torch` for a wall torch); empty for none.
    fn item_id(&self, state: u32) -> String;
}

/// `floor` of a coordinate, as the game's block positions.
pub fn floor(v: f64) -> i32 {
    v.floor() as i32
}

#[cfg(test)]
pub mod fake {
    //! Blocks for the tests: states are their text, numbered as first asked for; the world is a map.
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;

    pub struct FakeBlocks {
        names: RefCell<Vec<String>>,
        world: RefCell<HashMap<[i32; 3], u32>>,
        pub loaded: Cell<bool>,
        pub reads: Cell<usize>,
    }

    impl Default for FakeBlocks {
        fn default() -> Self {
            Self {
                names: RefCell::new(vec!["minecraft:air".into()]),
                world: RefCell::default(),
                loaded: Cell::new(true),
                reads: Cell::new(0),
            }
        }
    }

    impl FakeBlocks {
        /// The state for the game's text of it.
        pub fn id(&self, text: &str) -> u32 {
            let mut names = self.names.borrow_mut();
            if let Some(i) = names.iter().position(|n| n == text) {
                return i as u32;
            }
            names.push(text.to_owned());
            names.len() as u32 - 1
        }

        pub fn text(&self, state: u32) -> String {
            self.names.borrow()[state as usize].clone()
        }

        pub fn set(&self, x: i32, y: i32, z: i32, text: &str) {
            let id = self.id(text);
            self.world.borrow_mut().insert([x, y, z], id);
        }
    }

    impl Blocks for FakeBlocks {
        fn state(&self, name: &str, properties: &[(String, String)], _: i32) -> u32 {
            if name.starts_with("missing:") {
                return UNKNOWN;
            }
            let spec = super::spec::BlockSpec {
                name: name.into(),
                properties: properties.to_vec(),
            };
            self.id(&spec.to_string())
        }

        fn transform(&self, state: u32, mirror: Mirror, turns: u8) -> u32 {
            // Only `facing` turns here.
            let text = self.text(state);
            let Some(at) = text.find("facing=") else {
                return state;
            };
            let rest = &text[at + 7..];
            let end = rest.find([',', ']']).unwrap_or(rest.len());
            let order = ["north", "east", "south", "west"];
            let Some(mut i) = order.iter().position(|d| *d == &rest[..end]) else {
                return state;
            };
            i = match (mirror, i) {
                (Mirror::X, 1) => 3,
                (Mirror::X, 3) => 1,
                (Mirror::Z, 0) => 2,
                (Mirror::Z, 2) => 0,
                _ => i,
            };
            let turned = order[(i + turns as usize) % 4];
            self.id(&format!("{}{}{}", &text[..at + 7], turned, &rest[end..]))
        }

        fn at(&self, x: i32, y: i32, z: i32) -> u32 {
            self.reads.set(self.reads.get() + 1);
            if !self.loaded.get() {
                return UNKNOWN;
            }
            self.world.borrow().get(&[x, y, z]).copied().unwrap_or(0)
        }

        fn is_air(&self, state: u32) -> bool {
            state == 0
        }

        fn block(&self, state: u32) -> u32 {
            let text = self.text(state);
            let name = text.split('[').next().unwrap_or("").to_owned();
            self.id(&name)
        }

        fn item_id(&self, state: u32) -> String {
            let text = self.text(state);
            let name = text.split('[').next().unwrap_or("");
            match name {
                "minecraft:water" | "minecraft:air" => String::new(),
                "minecraft:wall_torch" => "minecraft:torch".into(),
                _ => name.to_owned(),
            }
        }
    }
}
