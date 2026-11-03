//! The Schematics module's state (SchematicsModule.java), on the window's thread: one schematic at
//! a time, kept until another is chosen or it's unloaded, whatever world the player goes to. The
//! game side ticks it, hands it the chosen file, the module's keys and its action buttons, asks it
//! what to draw as ghosts and boxes, and shows its messages.
use super::data::Schematic;
use super::materials::Materials;
use super::nbt::Inflate;
use super::placement::Placement;
use super::verifier::{self, Verifier};
use super::{Blocks, Mirror, UNKNOWN, floor, formats};
use crate::framework::setting::Settings;
use std::cell::RefCell;
use std::rc::Rc;

pub const TITLE: &str = "Schematics";
/// At most this many boxes a frame, nearest sections first: thousands would slow a Chromebook.
pub const MAX_BOXES: usize = 1024;
/// Ticks between the material list's counts.
pub const MATERIAL_TICKS: u64 = 10;

/// What's shown as ghosts (choice `ghosts`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ghosts {
    Missing,
    All,
    None,
}

/// Which of the schematic's layers are drawn (choice `layers`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layers {
    All,
    Below,
    One,
}

/// The module's settings as drawing and checking read them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub ghosts: Ghosts,
    /// 10 to 90.
    pub opacity: i32,
    pub range: i32,
    pub layers: Layers,
    pub outline: bool,
    /// ARGB, opaque.
    pub outline_color: u32,
    pub highlight: bool,
    pub wrong_color: u32,
    pub state_color: u32,
    pub extra_color: u32,
}

impl Look {
    pub fn of(s: &Settings) -> Self {
        let opaque = |id: &str| 0xFF00_0000 | (s.color(id) & 0x00FF_FFFF);
        Self {
            ghosts: match s.choice("ghosts") {
                "all" => Ghosts::All,
                "none" => Ghosts::None,
                _ => Ghosts::Missing,
            },
            opacity: s.int("opacity").clamp(10, 90),
            range: s.int("range").clamp(16, 128),
            layers: match s.choice("layers") {
                "below" => Layers::Below,
                "one" => Layers::One,
                _ => Layers::All,
            },
            outline: s.bool("outline"),
            outline_color: opaque("outline-color"),
            highlight: s.bool("highlight"),
            wrong_color: opaque("wrong-color"),
            state_color: opaque("state-color"),
            extra_color: opaque("extra-color"),
        }
    }

    /// The ghosts' alpha, 0 to 255.
    pub fn alpha(&self) -> u8 {
        (self.opacity as f32 * 255.0 / 100.0).round() as u8
    }
}

impl Default for Look {
    /// The module's defaults.
    fn default() -> Self {
        Self {
            ghosts: Ghosts::Missing,
            opacity: 45,
            range: 48,
            layers: Layers::All,
            outline: true,
            outline_color: 0xFF55_AAFF,
            highlight: true,
            wrong_color: 0xFFFF_3333,
            state_color: 0xFFFF_AA00,
            extra_color: 0xFFFF_55FF,
        }
    }
}

/// Where the player is: their feet, and Minecraft's yaw (0 faces south, 90 west).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Player {
    pub pos: [f64; 3],
    pub yaw: f32,
}

impl Player {
    pub fn block(&self) -> [i32; 3] {
        self.pos.map(floor)
    }
}

/// What a button or key asks of the schematic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Its corner to where the player stands.
    Here,
    /// Quarter turns clockwise (negative: anticlockwise).
    Turn(i32),
    /// East-west, then north-south, then not.
    Mirror,
    /// A block the way the player faces; back with `true`.
    Nudge(bool),
    /// Blocks up (negative: down).
    Raise(i32),
    /// The layer drawn, up or down.
    Layer(i32),
    Unload,
}

impl Action {
    /// The module's action button `setting` (`here`, `turn`, `mirror`, `unload`).
    pub fn of_button(setting: &str) -> Option<Self> {
        Some(match setting {
            "here" => Self::Here,
            "turn" => Self::Turn(1),
            "mirror" => Self::Mirror,
            "unload" => Self::Unload,
            _ => return None,
        })
    }
}

/// The module's keys (`here-key`...), from their settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Keys {
    pub here: crate::input::Key,
    pub turn: crate::input::Key,
    pub mirror: crate::input::Key,
    pub nudge: crate::input::Key,
    pub raise: crate::input::Key,
    pub layer: crate::input::Key,
}

impl Keys {
    pub fn of(s: &Settings) -> Self {
        Self {
            here: s.key("here-key"),
            turn: s.key("turn-key"),
            mirror: s.key("mirror-key"),
            nudge: s.key("nudge-key"),
            raise: s.key("raise-key"),
            layer: s.key("layer-key"),
        }
    }

    /// What pressing `key` does; sneaking reverses Turn, Nudge, Up and Layer.
    pub fn action(&self, key: crate::input::Key, sneaking: bool) -> Option<Action> {
        use crate::input::Key;
        if key == Key::None {
            return None;
        }
        let way = if sneaking { -1 } else { 1 };
        [
            (self.here, Action::Here),
            (self.turn, Action::Turn(way)),
            (self.mirror, Action::Mirror),
            (self.nudge, Action::Nudge(sneaking)),
            (self.raise, Action::Raise(way)),
            (self.layer, Action::Layer(way)),
        ]
        .into_iter()
        .find(|(k, _)| *k == key)
        .map(|(_, a)| a)
    }
}

/// A schematic loaded and placed: what's worked out from it once, and again as it moves.
pub struct Loaded {
    pub schematic: Rc<Schematic>,
    pub placement: Placement,
    /// The game's state for each palette index, as the file has it; UNKNOWN where the game has none.
    base_states: Vec<u32>,
    /// `base_states` mirrored and turned as placed.
    pub states: Vec<u32>,
    pub materials: Materials,
    pub verifier: Verifier,
    /// Kinds of block the game doesn't have, left out.
    pub unknown_kinds: usize,
}

impl Loaded {
    pub fn new(schematic: Rc<Schematic>, placement: Placement, blocks: &dyn Blocks) -> Self {
        let mut unknown_kinds = 0;
        let base_states: Vec<u32> = schematic
            .palette
            .iter()
            .map(|spec| {
                if spec.is_nothing() {
                    return UNKNOWN;
                }
                let state = blocks.state(&spec.name, &spec.properties, schematic.data_version);
                if state == UNKNOWN {
                    unknown_kinds += 1;
                }
                state
            })
            .collect();
        let materials = Materials::new(&schematic, &base_states, blocks);
        let verifier = Verifier::new(&placement, base_states.len());
        let mut loaded = Self {
            states: base_states.clone(),
            schematic,
            placement,
            base_states,
            materials,
            verifier,
            unknown_kinds,
        };
        loaded.place(blocks);
        loaded
    }

    /// Works the states out again for how it's placed now, and starts comparing afresh.
    pub fn place(&mut self, blocks: &dyn Blocks) {
        let (mirror, turns) = (self.placement.mirror(), self.placement.turns());
        for (state, &base) in self.states.iter_mut().zip(&self.base_states) {
            *state = if base == UNKNOWN || blocks.is_air(base) {
                base
            } else {
                blocks.transform(base, mirror, turns)
            };
        }
        self.verifier = Verifier::new(&self.placement, self.states.len());
    }
}

/// A box to draw in the world: from block corner `min` to `max` (one past the last block), edges in
/// `argb`, faces filled in `fill` (0 for none).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldBox {
    pub min: [i32; 3],
    pub max: [i32; 3],
    pub argb: u32,
    pub fill: u32,
}

#[derive(Default)]
pub struct Session {
    loaded: Option<Loaded>,
    /// The layer drawn, in world y, while Layers isn't All.
    layer: i32,
    /// Goes up whenever what the ghosts show might have changed everywhere.
    ghost_revision: u64,
    ticks: u64,
    /// The settings as last seen: a change to Ghosts or Layers is acted on.
    seen: Option<Look>,
    /// A file chosen in the chooser, loaded at the next tick.
    pending: Option<(String, Vec<u8>)>,
    /// Action buttons pressed in the module's page.
    queued: Vec<Action>,
    /// What the player is told, `Schematics: ...`.
    messages: Vec<String>,
    /// Goes up whenever what the HUD elements say might have changed.
    pub hud_revision: u64,
    /// Asked of the game side: set Layers to One layer (a layer key pressed while it's All).
    pub wants_one_layer: bool,
}

thread_local! {
    static SESSION: RefCell<Session> = RefCell::new(Session { ghost_revision: 1, ..Session::default() });
}

/// Runs `f` with the window's session; None while it's in use.
pub fn with<R>(f: impl FnOnce(&mut Session) -> R) -> Option<R> {
    SESSION.with(|s| s.try_borrow_mut().ok().map(|mut s| f(&mut s)))
}

/// The chooser picked `name`: loaded at the next tick.
pub fn choose(name: &str, bytes: Vec<u8>) {
    with(|s| s.pending = Some((name.to_owned(), bytes)));
}

/// Module `module`'s action button `setting` was pressed in its page (the client's actions);
/// false if it isn't Schematics'.
pub fn button(module: &str, setting: &str) -> bool {
    if module != super::ID {
        return false;
    }
    let Some(action) = Action::of_button(setting) else {
        return false;
    };
    with(|s| s.queued.push(action)).is_some()
}

impl Session {
    pub fn loaded(&self) -> Option<&Loaded> {
        self.loaded.as_ref()
    }

    pub fn is_loaded(&self) -> bool {
        self.loaded.is_some()
    }

    /// The layer drawn, in world y.
    pub fn layer(&self) -> i32 {
        self.layer
    }

    /// What the player is to be told since the last call.
    pub fn take_messages(&mut self) -> Vec<String> {
        std::mem::take(&mut self.messages)
    }

    fn say(&mut self, text: String) {
        self.messages.push(format!("{TITLE}: {text}"));
    }

    fn changed(&mut self) {
        self.ghost_revision += 1;
        self.hud_revision += 1;
    }

    /// Reads `bytes`, the file `name`, and places it where the player stands.
    pub fn load(
        &mut self,
        name: &str,
        bytes: &[u8],
        inflate: Inflate,
        blocks: &dyn Blocks,
        player: Option<Player>,
    ) {
        let schematic = match formats::read(name, bytes, inflate) {
            Ok(s) => Rc::new(s),
            Err(e) => {
                self.say(format!("Couldn't load {name}: {e}"));
                return;
            }
        };
        let [x, y, z] = player.map_or([0, 64, 0], |p| p.block());
        let loaded = Loaded::new(
            schematic.clone(),
            Placement::new(schematic.clone(), x, y, z),
            blocks,
        );
        let [sx, sy, sz] = schematic.size;
        let mut message = format!(
            "{}, {sx} x {sy} x {sz}, placed where you stand.",
            schematic.name
        );
        match loaded.unknown_kinds {
            0 => {}
            1 => message.push_str(" 1 block isn't in this version: left out."),
            n => message.push_str(&format!(" {n} blocks aren't in this version: left out.")),
        }
        self.loaded = Some(loaded);
        self.layer = y;
        self.say(message);
        self.changed();
    }

    /// Does what a button or key asks.
    pub fn act(
        &mut self,
        action: Action,
        blocks: &dyn Blocks,
        player: Option<Player>,
        look: &Look,
    ) {
        let Some(now) = self.loaded.as_mut() else {
            return;
        };
        let p = &mut now.placement;
        match action {
            Action::Here => {
                let Some(player) = player else {
                    return;
                };
                let [x, y, z] = player.block();
                p.move_to(x, y, z);
            }
            Action::Nudge(back) => {
                let Some(player) = player else {
                    return;
                };
                // The nearest of north, east, south, west: heading clockwise from north.
                let heading = player.yaw + 180.0;
                let facing = ((heading / 90.0).round() as i32).rem_euclid(4);
                let step = if back { -1 } else { 1 };
                let dx = match facing {
                    1 => step,
                    3 => -step,
                    _ => 0,
                };
                let dz = match facing {
                    2 => step,
                    0 => -step,
                    _ => 0,
                };
                p.move_by(dx, 0, dz);
            }
            Action::Raise(by) => {
                p.move_by(0, by, 0);
                self.layer += by;
            }
            Action::Turn(by) => p.turn(by),
            Action::Mirror => p.set_mirror(match p.mirror() {
                Mirror::None => Mirror::X,
                Mirror::X => Mirror::Z,
                Mirror::Z => Mirror::None,
            }),
            Action::Layer(by) => {
                if look.layers == Layers::All {
                    self.wants_one_layer = true;
                }
                self.layer = clamp_layer(&now.placement, self.layer + by);
                self.changed();
                return;
            }
            Action::Unload => {
                self.loaded = None;
                self.changed();
                return;
            }
        }
        // It moved, turned or mirrored: everything's worked out again.
        now.place(blocks);
        self.layer = clamp_layer(&now.placement, self.layer);
        self.changed();
    }

    /// The world changed under it (the player left it): what was found is forgotten, and looked
    /// for again.
    pub fn left_world(&mut self, blocks: &dyn Blocks) {
        if let Some(now) = self.loaded.as_mut() {
            now.place(blocks);
            self.changed();
        }
    }

    /// Once a tick: a chosen file loaded, buttons pressed done, the settings followed, the world
    /// compared, and the material list counted every half second.
    pub fn tick(
        &mut self,
        blocks: &dyn Blocks,
        player: Option<Player>,
        look: &Look,
        inflate: Inflate,
    ) {
        if let Some((name, bytes)) = self.pending.take() {
            self.load(&name, &bytes, inflate, blocks, player);
        }
        for action in std::mem::take(&mut self.queued) {
            self.act(action, blocks, player, look);
        }
        if let Some(seen) = self.seen
            && (seen.ghosts != look.ghosts
                || seen.layers != look.layers
                || seen.range != look.range)
        {
            if seen.layers != look.layers
                && look.layers != Layers::All
                && let (Some(now), Some(p)) = (&self.loaded, player)
            {
                // Start at the player's feet: the layer they're likely building.
                self.layer = clamp_layer(&now.placement, p.block()[1]);
            }
            self.changed();
        }
        self.seen = Some(*look);
        let (Some(now), Some(player)) = (self.loaded.as_mut(), player) else {
            return;
        };
        let world = verifier::World {
            placement: &now.placement,
            states: &now.states,
            blocks,
        };
        now.verifier.tick(&world, player.block(), look.range);
        self.ticks += 1;
        if self.ticks.is_multiple_of(MATERIAL_TICKS) {
            now.materials.refresh(&now.verifier);
            self.hud_revision += 1;
        }
    }

    /// Whether world layer `y` is drawn.
    pub fn shows(&self, y: i32, look: &Look) -> bool {
        match look.layers {
            Layers::All => true,
            Layers::Below => y <= self.layer,
            Layers::One => y == self.layer,
        }
    }

    /// The box the ghosts are in: lowest corner, and one past the highest.
    pub fn ghost_bounds(&self) -> Option<([i32; 3], [i32; 3])> {
        self.loaded
            .as_ref()
            .map(|l| (l.placement.min(), l.placement.max()))
    }

    /// A number that changes whenever what `ghost_at` gives in section `section` might have.
    pub fn ghost_revision(&self, section: [i32; 3]) -> u64 {
        let local = self
            .loaded
            .as_ref()
            .and_then(|l| l.verifier.section(section[0], section[1], section[2]))
            .map_or(0, |s| s.revision & 0xFFFF_FFFF);
        self.ghost_revision << 32 | local
    }

    /// The state to draw a ghost of at `x, y, z`, if any. Called while a section is meshed.
    pub fn ghost_at(
        &self,
        x: i32,
        y: i32,
        z: i32,
        look: &Look,
        blocks: &dyn Blocks,
    ) -> Option<u32> {
        let now = self.loaded.as_ref()?;
        if look.ghosts == Ghosts::None || !self.shows(y, look) {
            return None;
        }
        let index = now.placement.index_at(x, y, z);
        let want = *now.states.get(index).filter(|_| index != 0)?;
        if want == UNKNOWN || blocks.is_air(want) {
            return None;
        }
        if look.ghosts == Ghosts::All {
            return Some(want);
        }
        let have = blocks.at(x, y, z);
        (have != UNKNOWN && blocks.is_air(have)).then_some(want)
    }

    /// The boxes to draw this frame round `camera`: the schematic's outline, and its wrong and
    /// extra blocks within range, nearest sections first, at most `MAX_BOXES`.
    pub fn boxes(&self, camera: [f64; 3], look: &Look, out: &mut Vec<WorldBox>) {
        out.clear();
        let Some(now) = &self.loaded else {
            return;
        };
        if look.outline {
            out.push(WorldBox {
                min: now.placement.min(),
                max: now.placement.max(),
                argb: look.outline_color,
                fill: 0,
            });
        }
        if !look.highlight {
            return;
        }
        let c = camera.map(floor);
        let cs = c.map(|v| v >> 4);
        let reach = look.range;
        let reach_sq = i64::from(reach) * i64::from(reach);
        let rings = (reach >> 4) + 1;
        let v = &now.verifier;
        // Ring by ring outward, so the cap leaves out the farthest.
        for ring in 0..=rings {
            for sy in cs[1] - ring..=cs[1] + ring {
                for sz in cs[2] - ring..=cs[2] + ring {
                    for sx in cs[0] - ring..=cs[0] + ring {
                        if out.len() >= MAX_BOXES {
                            return;
                        }
                        let edge = (sx - cs[0])
                            .abs()
                            .max((sy - cs[1]).abs())
                            .max((sz - cs[2]).abs());
                        if edge != ring {
                            continue;
                        }
                        let Some(section) = v.section(sx, sy, sz) else {
                            continue;
                        };
                        for &issue in section.issues() {
                            if out.len() >= MAX_BOXES {
                                return;
                            }
                            let at = (issue >> 3) as i32;
                            let x = (sx << 4) | (at & 15);
                            let y = (sy << 4) | (at >> 8);
                            let z = (sz << 4) | ((at >> 4) & 15);
                            let d = [x - c[0], y - c[1], z - c[2]].map(i64::from);
                            if !self.shows(y, look)
                                || d[0] * d[0] + d[1] * d[1] + d[2] * d[2] > reach_sq
                            {
                                continue;
                            }
                            let rgb = match issue & 7 {
                                verifier::WRONG_BLOCK => look.wrong_color,
                                verifier::WRONG_STATE => look.state_color,
                                _ => look.extra_color,
                            } & 0x00FF_FFFF;
                            out.push(WorldBox {
                                min: [x, y, z],
                                max: [x + 1, y + 1, z + 1],
                                argb: 0xFF00_0000 | rgb,
                                fill: 0x4000_0000 | rgb,
                            });
                        }
                    }
                }
            }
        }
    }
}

fn clamp_layer(p: &Placement, y: i32) -> i32 {
    y.clamp(p.min()[1], p.max()[1] - 1)
}

#[cfg(test)]
mod tests {
    use super::super::fake::FakeBlocks;
    use super::super::formats::fixtures::sponge;
    use super::super::nbt::write::stored_inflate;
    use super::*;

    /// A Sponge schematic 3 x 2 x 1: stone, stone and stairs facing north along the bottom; air,
    /// air and glass on top.
    pub fn tower() -> Vec<u8> {
        sponge(
            3,
            [3, 2, 1],
            &[
                ("minecraft:air", 0),
                ("minecraft:stone", 1),
                ("minecraft:oak_stairs[facing=north]", 2),
                ("minecraft:glass", 3),
            ],
            &[1, 1, 2, 0, 0, 3],
        )
    }

    const PLAYER: Player = Player {
        pos: [100.5, 64.0, 200.5],
        yaw: 0.0,
    };

    fn loaded(blocks: &FakeBlocks) -> Session {
        let mut s = Session {
            ghost_revision: 1,
            ..Session::default()
        };
        s.load(
            "tower.schem",
            &tower(),
            stored_inflate,
            blocks,
            Some(PLAYER),
        );
        s
    }

    /// Every ghost in the box, as `name at x,y,z`.
    fn ghosts(s: &Session, look: &Look, blocks: &FakeBlocks) -> Vec<String> {
        let (lo, hi) = s.ghost_bounds().unwrap();
        let mut out = Vec::new();
        for y in lo[1]..hi[1] {
            for z in lo[2]..hi[2] {
                for x in lo[0]..hi[0] {
                    if let Some(state) = s.ghost_at(x, y, z, look, blocks) {
                        out.push(format!("{} at {x},{y},{z}", blocks.text(state)));
                    }
                }
            }
        }
        out
    }

    fn ticks(s: &mut Session, n: u64, blocks: &FakeBlocks, look: &Look) {
        for _ in 0..n {
            s.tick(blocks, Some(PLAYER), look, stored_inflate);
        }
    }

    #[test]
    fn choosing_a_file_loads_it_where_the_player_stands() {
        let blocks = FakeBlocks::default();
        let mut s = loaded(&blocks);
        assert_eq!(
            s.take_messages(),
            ["Schematics: tower, 3 x 2 x 1, placed where you stand."]
        );
        let look = Look::default();
        assert_eq!(
            ghosts(&s, &look, &blocks),
            [
                "minecraft:stone at 100,64,200",
                "minecraft:stone at 101,64,200",
                "minecraft:oak_stairs[facing=north] at 102,64,200",
                "minecraft:glass at 102,65,200",
            ]
        );
        let mut boxes = Vec::new();
        s.boxes([100.0, 64.0, 200.0], &look, &mut boxes);
        assert_eq!(
            boxes,
            [WorldBox {
                min: [100, 64, 200],
                max: [103, 66, 201],
                argb: 0xFF55AAFF,
                fill: 0
            }]
        );
    }

    #[test]
    fn built_blocks_stop_being_ghosts_and_wrong_ones_are_boxed() {
        let blocks = FakeBlocks::default();
        let mut s = loaded(&blocks);
        let look = Look {
            outline: false,
            ..Look::default()
        };
        blocks.set(100, 64, 200, "minecraft:stone");
        blocks.set(101, 64, 200, "minecraft:dirt");
        blocks.set(102, 64, 200, "minecraft:oak_stairs[facing=south]");
        blocks.set(100, 65, 200, "minecraft:torch");
        ticks(&mut s, verifier::NEAR_TICKS + 1, &blocks, &look);
        assert_eq!(
            ghosts(&s, &look, &blocks),
            ["minecraft:glass at 102,65,200"]
        );
        let mut boxes = Vec::new();
        s.boxes([100.0, 64.0, 200.0], &look, &mut boxes);
        let drawn: Vec<([i32; 3], u32)> = boxes.iter().map(|b| (b.min, b.argb)).collect();
        assert_eq!(
            drawn,
            [
                ([101, 64, 200], 0xFFFF3333),
                ([102, 64, 200], 0xFFFFAA00),
                ([100, 65, 200], 0xFFFF55FF)
            ]
        );
        s.boxes(
            [100.0, 64.0, 200.0],
            &Look {
                highlight: false,
                ..look
            },
            &mut boxes,
        );
        assert!(boxes.is_empty());
    }

    #[test]
    fn turning_moves_the_ghosts_round_its_corner() {
        let blocks = FakeBlocks::default();
        let mut s = loaded(&blocks);
        let look = Look::default();
        s.act(Action::Turn(1), &blocks, Some(PLAYER), &look);
        // Turned clockwise, the row that ran east now runs south, and the stairs face east.
        let g = ghosts(&s, &look, &blocks);
        assert!(g.contains(&"minecraft:stone at 100,64,200".to_owned()));
        assert!(g.contains(&"minecraft:stone at 100,64,201".to_owned()));
        assert!(
            g.contains(&"minecraft:oak_stairs[facing=east] at 100,64,202".to_owned()),
            "{g:?}"
        );
        assert_eq!(s.ghost_bounds(), Some(([100, 64, 200], [101, 66, 203])));
        s.act(Action::Mirror, &blocks, Some(PLAYER), &look);
        assert_eq!(s.loaded().unwrap().placement.mirror(), Mirror::X);
        s.act(Action::Unload, &blocks, Some(PLAYER), &look);
        assert!(!s.is_loaded());
    }

    #[test]
    fn keys_nudge_the_way_the_player_faces_and_layers_follow() {
        let blocks = FakeBlocks::default();
        let mut s = loaded(&blocks);
        let look = Look::default();
        // Yaw 0 faces south (+z); 90 west (-x).
        s.act(Action::Nudge(false), &blocks, Some(PLAYER), &look);
        assert_eq!(s.loaded().unwrap().placement.min(), [100, 64, 201]);
        s.act(
            Action::Nudge(true),
            &blocks,
            Some(Player {
                yaw: 90.0,
                ..PLAYER
            }),
            &look,
        );
        assert_eq!(s.loaded().unwrap().placement.min(), [101, 64, 201]);
        s.act(Action::Raise(2), &blocks, Some(PLAYER), &look);
        assert_eq!(s.loaded().unwrap().placement.min(), [101, 66, 201]);
        // A layer key while All: One layer is asked for, the layer stays in the box.
        s.act(Action::Layer(5), &blocks, Some(PLAYER), &look);
        assert!(s.wants_one_layer);
        assert_eq!(s.layer(), 67);
        let one = Look {
            layers: Layers::One,
            ..look
        };
        assert!(s.shows(67, &one) && !s.shows(66, &one));
        let keys = Keys {
            turn: crate::input::Key::from_name("key.keyboard.r"),
            ..Keys::default()
        };
        assert_eq!(
            keys.action(crate::input::Key::from_name("key.keyboard.r"), true),
            Some(Action::Turn(-1))
        );
        assert_eq!(keys.action(crate::input::Key::None, false), None);
    }

    #[test]
    fn the_material_list_counts_whats_left() {
        let blocks = FakeBlocks::default();
        let mut s = loaded(&blocks);
        let look = Look::default();
        ticks(&mut s, MATERIAL_TICKS, &blocks, &look);
        let m = &s.loaded().unwrap().materials;
        assert_eq!(m.kinds_needed(), 3);
        blocks.set(100, 64, 200, "minecraft:stone");
        blocks.set(101, 64, 200, "minecraft:stone");
        ticks(&mut s, MATERIAL_TICKS, &blocks, &look);
        let l = s.loaded().unwrap();
        assert_eq!(l.materials.kinds_needed(), 2);
        assert_eq!(l.verifier.right(), 2);
        assert_eq!(l.schematic.block_count(), 4);
    }

    #[test]
    fn a_broken_file_says_why_and_buttons_queue() {
        let blocks = FakeBlocks::default();
        let mut s = Session::default();
        s.load("junk.schem", b"junk", stored_inflate, &blocks, Some(PLAYER));
        assert_eq!(
            s.take_messages(),
            ["Schematics: Couldn't load junk.schem: Not a schematic, or a broken one"]
        );
        assert!(
            button("schematics", "turn")
                && !button("schematics", "choose")
                && !button("zoom", "turn")
        );
        assert_eq!(Action::of_button("unload"), Some(Action::Unload));
    }
}
