//! Waypoints (framework's `incari.framework.waypoint`, `WaypointCodec`, ADR 0051): named,
//! coloured marks at a block in one dimension of a world, kept a file a world beside the profiles
//! (`incari/modules/waypoints/<world>-<hash>.json`), in the Java client's exact format, so both
//! builds share them. Waypoints aren't part of a profile: switching profiles keeps them.
//!
//! The world's waypoints are one store for the window's thread (`with`), as Java's
//! `ModuleSystem.waypoints()`: the Waypoints mod draws them, the minimap shows them, the screens
//! change them. Files go through `Storage` (relative to `incari/modules/`), the player through
//! `set_player` (the game side sets it each frame). No game types.
use crate::framework::system::Storage;
use crate::paint::Shape;
use serde_json::Value;
use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};

/// The colours the editor offers, `0xRRGGBB`, in order: each new waypoint takes the next.
pub const COLORS: [u32; 12] = [
    0xFF5A5A, 0xFF9F43, 0xFFD93D, 0x8BE04E, 0x3DDC97, 0x3DD6F5, 0x5B8CFF, 0x9D7BFF, 0xF06FD8,
    0xFFFFFF, 0xA9B4BF, 0x8B5E3C,
];

/// The longest name, in UTF-16 units as Java counts them.
pub const MAX_NAME: usize = 32;

/// The shapes a waypoint can take, in the order the editor offers them (`Shape.MARKERS`).
pub const MARKERS: [Shape; 8] = [
    Shape::Circle,
    Shape::Square,
    Shape::Diamond,
    Shape::Triangle,
    Shape::Star,
    Shape::Heart,
    Shape::Hexagon,
    Shape::Cross,
];

/// The dimensions' ids (`PlayerView.OVERWORLD`...).
pub const OVERWORLD: &str = "minecraft:overworld";
pub const NETHER: &str = "minecraft:the_nether";
pub const END: &str = "minecraft:the_end";

/// How a server's world is named (`Game.world()`).
pub const SERVER_PREFIX: &str = "server/";

/// The name and colour a death's waypoint gets (WaypointsModule).
pub const DEATH_NAME: &str = "Death";
pub const DEATH_COLOR: u32 = COLORS[0];
/// Within this many blocks of a death's waypoint, the player has got back to it.
pub const ARRIVED: f64 = 4.0;
/// The player must first get this far away, so a respawn beside it isn't an arrival.
pub const AWAY: f64 = 8.0;

/// Ticks from the last change to the save: dragging through a colour is one save.
pub const SAVE_DELAY_TICKS: i32 = 20;

/// The format's version (`WaypointCodec.FORMAT`).
pub const FORMAT: i64 = 1;

/// A shape's name in the file (`circle`).
pub fn shape_key(shape: Shape) -> &'static str {
    match shape {
        Shape::Circle => "circle",
        Shape::Square => "square",
        Shape::Diamond => "diamond",
        Shape::Triangle => "triangle",
        Shape::Star => "star",
        Shape::Heart => "heart",
        Shape::Hexagon => "hexagon",
        Shape::Cross => "cross",
        Shape::Arrow => "arrow",
    }
}

/// What the editor calls a shape (`Shape.label`).
pub fn shape_label(shape: Shape) -> &'static str {
    match shape {
        Shape::Circle => "Circle",
        Shape::Square => "Square",
        Shape::Diamond => "Diamond",
        Shape::Triangle => "Triangle",
        Shape::Star => "Star",
        Shape::Heart => "Heart",
        Shape::Hexagon => "Hexagon",
        Shape::Cross => "Cross",
        Shape::Arrow => "Arrow",
    }
}

/// A marker by its file name; anything else is a circle.
pub fn shape_named(key: Option<&str>) -> Shape {
    MARKERS
        .into_iter()
        .find(|s| Some(shape_key(*s)) == key)
        .unwrap_or(Shape::Circle)
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// A named mark at a block. Each is itself (Java compares waypoints by identity): `id` tells two
/// alike apart, and a copy keeps it, so saving an edited copy changes the one it was made from.
#[derive(Clone, Debug, PartialEq)]
pub struct Waypoint {
    id: u64,
    name: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub dimension: String,
    rgb: u32,
    pub icon: Shape,
    /// Shown on the minimap and over the world.
    pub visible: bool,
    /// A beacon's beam rises from it, where beams are on.
    pub beam: bool,
    /// It marks where the player last died (made by the game, replaced at the next death).
    pub death: bool,
    /// It lasts only while the player stays in the world: shown, but never saved to the world's
    /// file (a death's on a server, where one address can be many maps).
    pub temporary: bool,
}

impl Waypoint {
    pub fn new(name: &str, x: i32, y: i32, z: i32, dimension: &str, rgb: u32, icon: Shape) -> Self {
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            name: clip(name),
            x,
            y,
            z,
            dimension: dimension.to_owned(),
            rgb: rgb & 0xFF_FFFF,
            icon,
            visible: true,
            beam: true,
            death: false,
            temporary: false,
        }
    }

    /// Which waypoint this is: the same for its copies.
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Renames it, stripped and cut to `MAX_NAME`.
    pub fn set_name(&mut self, name: &str) {
        self.name = clip(name);
    }

    /// Its colour, `0xRRGGBB`.
    pub fn rgb(&self) -> u32 {
        self.rgb
    }

    pub fn set_rgb(&mut self, rgb: u32) {
        self.rgb = rgb & 0xFF_FFFF;
    }

    /// Its colour with full alpha, for drawing.
    pub fn argb(&self) -> u32 {
        0xFF00_0000 | self.rgb
    }

    /// The squared distance from a point to the middle of its block.
    pub fn distance_squared(&self, px: f64, py: f64, pz: f64) -> f64 {
        let dx = self.x as f64 + 0.5 - px;
        let dy = self.y as f64 + 0.5 - py;
        let dz = self.z as f64 + 0.5 - pz;
        dx * dx + dy * dy + dz * dz
    }
}

/// Java's `String.strip` (its whitespace, not Unicode's: no-break spaces stay), then cut to
/// `MAX_NAME` UTF-16 units, never through a character.
fn clip(name: &str) -> String {
    // Character.isWhitespace: Unicode's spaces but the no-break ones, and the separators U+001C to
    // U+001F, but not U+0085.
    let java_space = |c: char| {
        matches!(c, '\u{1C}'..='\u{1F}')
            || c.is_whitespace() && !matches!(c, '\u{00A0}' | '\u{2007}' | '\u{202F}' | '\u{0085}')
    };
    let stripped = name.trim_matches(java_space);
    let mut units = 0;
    stripped
        .chars()
        .take_while(|c| {
            units += c.len_utf16();
            units <= MAX_NAME
        })
        .collect()
}

/// Java's `String.hashCode`, over UTF-16 units.
pub fn java_hash(text: &str) -> i32 {
    text.encode_utf16()
        .fold(0i32, |h, u| h.wrapping_mul(31).wrapping_add(u as i32))
}

/// A name as a file or folder's: letters, digits, dots and dashes kept (of its first 64 UTF-16
/// units), the rest underscores, and Java's hash of it, so two names that read alike stay apart
/// (`MapMemory.safe`).
pub fn safe_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 9);
    for u in name.encode_utf16().take(64) {
        let plain = matches!(u, 0x61..=0x7A | 0x41..=0x5A | 0x30..=0x39 | 0x2E | 0x2D);
        out.push(if plain { u as u8 as char } else { '_' });
    }
    out.push('-');
    out.push_str(&format!("{:x}", java_hash(name) as u32));
    out
}

/// The file a world's waypoints are kept in, under `incari/modules/` (`Waypoints.path`).
pub fn path(world: &str) -> String {
    format!("waypoints/{}.json", safe_name(world))
}

/// `text` as a JSON string as Gson's `JsonWriter` writes one (HTML-safe off): its escapes, its
/// `\u` in lower case, U+2028 and U+2029 escaped.
fn json_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || c == '\u{2028}' || c == '\u{2029}' => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// The waypoints as the file's text, byte for byte as the Java client writes it.
pub fn write(waypoints: &[Waypoint]) -> String {
    let mut out = String::with_capacity(64 + waypoints.len() * 160);
    out.push_str("{\"format\":1,\"waypoints\":[");
    for (i, w) in waypoints.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"name\":");
        json_string(&mut out, &w.name);
        out.push_str(&format!(
            ",\"x\":{},\"y\":{},\"z\":{},\"dimension\":",
            w.x, w.y, w.z
        ));
        json_string(&mut out, &w.dimension);
        out.push_str(&format!(
            ",\"color\":\"#{:06X}\",\"icon\":\"{}\",\"visible\":{},\"beam\":{},\"death\":{}}}",
            w.rgb,
            shape_key(w.icon),
            w.visible,
            w.beam,
            w.death
        ));
    }
    out.push_str("]}");
    out
}

/// The waypoints in a file's text. Forgiving, as Java's: a waypoint without a name or a position
/// is left out (and told to `problems`), a value missing or out of place takes its default.
pub fn read(text: &str, problems: &mut dyn FnMut(String)) -> Vec<Waypoint> {
    let parsed: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            problems(format!("couldn't be read: {e}"));
            return Vec::new();
        }
    };
    let Some(root) = parsed.as_object() else {
        problems("not a waypoints file".into());
        return Vec::new();
    };
    let Some(entries) = root.get("waypoints").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        match entry.as_object().and_then(entry_waypoint) {
            Some(w) => out.push(w),
            None => problems(format!(
                "left out a waypoint that couldn't be read: {entry}"
            )),
        }
    }
    out
}

/// Gson's `getAsInt` of a number: whole numbers wrap to 32 bits, fractions are cut toward zero.
fn as_int(v: &Value) -> Option<i32> {
    if let Some(i) = v.as_i64() {
        return Some(i as i32);
    }
    if let Some(u) = v.as_u64() {
        return Some(u as i32);
    }
    v.as_f64().map(|f| f.trunc() as i64 as i32)
}

fn entry_waypoint(entry: &serde_json::Map<String, Value>) -> Option<Waypoint> {
    let string = |k: &str| entry.get(k).and_then(Value::as_str);
    let number = |k: &str| entry.get(k).filter(|v| v.is_number()).and_then(as_int);
    let flag =
        |k: &str, otherwise: bool| entry.get(k).and_then(Value::as_bool).unwrap_or(otherwise);
    let name = string("name")?;
    let (x, y, z) = (number("x")?, number("y")?, number("z")?);
    let rgb = string("color")
        .and_then(crate::color::parse_hex)
        .unwrap_or(COLORS[0]);
    let mut w = Waypoint::new(
        name,
        x,
        y,
        z,
        string("dimension").unwrap_or(OVERWORLD),
        rgb,
        shape_named(string("icon")),
    );
    w.visible = flag("visible", true);
    w.beam = flag("beam", true);
    w.death = flag("death", false);
    Some(w)
}

/// The waypoints of the world the player is in, every dimension's (Waypoints.java): opened when
/// the player joins a world, closed when they leave. Changes are saved a second after the last
/// one, or at once by `flush` and on `close`.
#[derive(Default)]
pub struct Waypoints {
    list: Vec<Waypoint>,
    world: Option<String>,
    revision: u64,
    save_countdown: i32,
    /// Problems reading or saving, for the game side to log.
    pub problems: Vec<String>,
}

impl Waypoints {
    pub fn new() -> Self {
        Self {
            save_countdown: -1,
            ..Self::default()
        }
    }

    /// The world whose waypoints these are (`server/<address>`, `world/<folder>`), or None outside
    /// a world.
    pub fn world(&self) -> Option<&str> {
        self.world.as_deref()
    }

    /// Every waypoint of the world, in every dimension, in the order they were made.
    pub fn all(&self) -> &[Waypoint] {
        &self.list
    }

    /// Goes up whenever a waypoint is added, removed or changed, or another world's are opened.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn get(&self, id: u64) -> Option<&Waypoint> {
        self.list.iter().find(|w| w.id == id)
    }

    /// Adds `waypoint` at the end; nothing outside a world, or if it's here already.
    pub fn add(&mut self, waypoint: Waypoint) {
        if self.world.is_none() || self.get(waypoint.id).is_some() {
            return;
        }
        let temporary = waypoint.temporary;
        self.list.push(waypoint);
        self.changed_one(temporary);
    }

    /// Removes waypoint `id`; false if it isn't here.
    pub fn remove(&mut self, id: u64) -> bool {
        let Some(i) = self.list.iter().position(|w| w.id == id) else {
            return false;
        };
        let gone = self.list.remove(i);
        self.changed_one(gone.temporary);
        true
    }

    /// Changes waypoint `id` with `f`; a change is saved. False if it isn't here.
    pub fn update(&mut self, id: u64, f: impl FnOnce(&mut Waypoint)) -> bool {
        let Some(w) = self.list.iter_mut().find(|w| w.id == id) else {
            return false;
        };
        let before = w.clone();
        f(w);
        // Its identity stays, whatever `f` did.
        w.id = id;
        if *w != before {
            // A temporary one's change needs no save, unless it became, or stopped being, one.
            let temporary = before.temporary && w.temporary;
            self.changed_one(temporary);
        }
        true
    }

    /// Takes every value of `edited` (an edited copy of one here), as one change (`Waypoint.set`).
    pub fn set(&mut self, edited: &Waypoint) -> bool {
        self.update(edited.id, |w| *w = edited.clone())
    }

    /// The waypoint marking the player's last death in this world.
    pub fn death(&self) -> Option<&Waypoint> {
        self.list.iter().find(|w| w.death)
    }

    /// Opens `name`'s waypoints, saving and closing any open ones first. A missing or unreadable
    /// file reads as none; an unreadable one isn't overwritten until something changes.
    pub fn open(&mut self, name: &str, storage: &mut dyn Storage) {
        if self.world.as_deref() == Some(name) {
            return;
        }
        self.close(storage);
        self.world = Some(name.to_owned());
        if let Some(text) = storage.read(&path(name)) {
            let problems = &mut self.problems;
            self.list = read(&text, &mut |m| problems.push(format!("{name}: {m}")));
        }
        self.revision += 1;
    }

    /// Saves any change and forgets the world's waypoints: the player left it.
    pub fn close(&mut self, storage: &mut dyn Storage) {
        self.flush(storage);
        self.list.clear();
        self.world = None;
        self.save_countdown = -1;
        self.revision += 1;
    }

    /// Deletes `name`'s waypoints (the game deleted the world); nothing for the world open now.
    pub fn delete(&mut self, name: &str, storage: &mut dyn Storage) {
        if self.world.as_deref() != Some(name) {
            storage.delete(&path(name));
        }
    }

    /// Once a game tick: a due save.
    pub fn tick(&mut self, storage: &mut dyn Storage) {
        if self.save_countdown > 0 {
            self.save_countdown -= 1;
            if self.save_countdown == 0 {
                self.flush(storage);
            }
        }
    }

    /// Saves now if anything changed since the last save; no waypoints left deletes the file.
    pub fn flush(&mut self, storage: &mut dyn Storage) {
        let Some(name) = self.world.as_deref() else {
            return;
        };
        if self.save_countdown < 0 {
            return;
        }
        self.save_countdown = -1;
        // A temporary waypoint is the visit's: never in the file.
        let kept: Vec<Waypoint> = self.list.iter().filter(|w| !w.temporary).cloned().collect();
        if kept.is_empty() {
            storage.delete(&path(name));
        } else {
            storage.write(&path(name), &write(&kept));
        }
    }

    /// A waypoint changed, or the list did. A temporary one's change needs no save: the file never
    /// has it (and an unreadable file isn't overwritten for it).
    fn changed_one(&mut self, temporary: bool) {
        self.revision += 1;
        if !temporary {
            self.save_countdown = SAVE_DELAY_TICKS;
        }
    }
}

/// "Waypoint 1", or the first number from the count on that no waypoint is called.
pub fn next_name(all: &[Waypoint]) -> String {
    (all.len() + 1..)
        .map(|n| format!("Waypoint {n}"))
        .find(|name| all.iter().all(|w| w.name != *name))
        .expect("a free number")
}

/// A new waypoint at a block, not yet added: the next free name, the palette's next colour.
pub fn draft(all: &[Waypoint], x: i32, y: i32, z: i32, dimension: &str) -> Waypoint {
    Waypoint::new(
        &next_name(all),
        x,
        y,
        z,
        dimension,
        COLORS[all.len() % COLORS.len()],
        Shape::Circle,
    )
}

/// "the Overworld", "the Nether", "the End", or a modded dimension's path ("Deep dark").
pub fn dimension_name(dimension: &str) -> String {
    match dimension {
        OVERWORLD => "the Overworld".into(),
        NETHER => "the Nether".into(),
        END => "the End".into(),
        _ => {
            let path = dimension
                .split_once(':')
                .map_or(dimension, |p| p.1)
                .replace('_', " ");
            let mut chars = path.chars();
            match chars.next() {
                None => dimension.to_owned(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        }
    }
}

/// "12 m", "999 m", "1.2 km" (WaypointListScreen.distance).
pub fn distance_text(blocks: f64) -> String {
    if blocks < 1000.0 {
        return format!("{} m", blocks as i32);
    }
    let tenths = (blocks / 100.0) as i32;
    format!("{}.{} km", tenths / 10, tenths % 10)
}

/// The player as waypoints need them: where they are, which way they face, their dimension.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Player {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// The game's yaw: degrees, 0 faces south, 90 west.
    pub yaw: f32,
    pub dimension: String,
}

impl Player {
    /// The compass bearing faced: degrees clockwise from north.
    pub fn heading(&self) -> f32 {
        (self.yaw + 180.0).rem_euclid(360.0)
    }

    /// The block they stand in.
    pub fn block(&self) -> [i32; 3] {
        [
            self.x.floor() as i32,
            self.y.floor() as i32,
            self.z.floor() as i32,
        ]
    }

    /// Whether `w` is shown to them now: visible and in their dimension.
    pub fn sees(&self, w: &Waypoint) -> bool {
        w.visible && w.dimension == self.dimension
    }
}

/// The Waypoints module's death watch (watchForDeath): a death leaves a waypoint where it
/// happened, replacing the last; once the player has been away from it and comes back, it goes.
#[derive(Clone, Copy, Debug, Default)]
pub struct DeathWatch {
    was_dead: bool,
    left_death: bool,
}

impl DeathWatch {
    /// Once a tick, with the module's Mark deaths and Clear deaths on arrival.
    pub fn tick(
        &mut self,
        store: &mut Waypoints,
        player: Option<&Player>,
        dead: bool,
        mark: bool,
        clear: bool,
    ) {
        let dead = player.is_some() && dead;
        if let Some(p) = player
            && dead
            && !self.was_dead
            && mark
            && store.world().is_some()
        {
            if let Some(last) = store.death().map(Waypoint::id) {
                store.remove(last);
            }
            let [x, y, z] = p.block();
            let mut death =
                Waypoint::new(DEATH_NAME, x, y, z, &p.dimension, DEATH_COLOR, Shape::Cross);
            death.death = true;
            // A server's address can be many maps (a network's lobby and its games): there a death
            // is marked for this visit only, never kept in the server's file for the next map.
            death.temporary = store.world().is_some_and(|w| w.starts_with(SERVER_PREFIX));
            store.add(death);
            self.left_death = false;
        }
        self.was_dead = dead;
        let Some(p) = player.filter(|_| !dead && clear) else {
            return;
        };
        let Some(death) = store.death().filter(|d| d.dimension == p.dimension) else {
            return;
        };
        let dx = p.x - (death.x as f64 + 0.5);
        let dy = p.y - death.y as f64;
        let dz = p.z - (death.z as f64 + 0.5);
        let d2 = dx * dx + dy * dy + dz * dz;
        if d2 > AWAY * AWAY {
            self.left_death = true;
        } else if self.left_death && d2 <= ARRIVED * ARRIVED {
            let id = death.id();
            store.remove(id);
            self.left_death = false;
        }
    }
}

// The window's thread only: the screens, the HUD and the ticks all run there.
thread_local! {
    static STORE: RefCell<Waypoints> = RefCell::new(Waypoints::new());
    static PLAYER: RefCell<Option<Player>> = const { RefCell::new(None) };
}

/// Runs `f` with the world's waypoints; None while they're in use further up the call (never
/// nest calls).
pub fn with<R>(f: impl FnOnce(&mut Waypoints) -> R) -> Option<R> {
    STORE.with(|s| s.try_borrow_mut().ok().map(|mut s| f(&mut s)))
}

/// The player, as the game side last set them; None outside a world.
pub fn player() -> Option<Player> {
    PLAYER.with(|p| p.borrow().clone())
}

/// Runs `f` with the player, without copying them (hot path).
pub fn with_player<R>(f: impl FnOnce(Option<&Player>) -> R) -> R {
    PLAYER.with(|p| f(p.borrow().as_ref()))
}

/// The game side, each frame: where the player is now (None outside a world). Hot path: the
/// dimension's text is copied only when it changes.
pub fn set_player(at: Option<([f64; 3], f32, &str)>) {
    PLAYER.with(|cell| {
        let mut slot = cell.borrow_mut();
        match (at, slot.as_mut()) {
            (None, _) => *slot = None,
            (Some((pos, yaw, dim)), Some(p)) => {
                [p.x, p.y, p.z] = pos;
                p.yaw = yaw;
                if p.dimension != dim {
                    dim.clone_into(&mut p.dimension);
                }
            }
            (Some((pos, yaw, dim)), None) => {
                *slot = Some(Player {
                    x: pos[0],
                    y: pos[1],
                    z: pos[2],
                    yaw,
                    dimension: dim.to_owned(),
                })
            }
        }
    });
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    pub(crate) struct Files(pub HashMap<String, String>);

    impl Storage for Files {
        fn read(&self, path: &str) -> Option<String> {
            self.0.get(path).cloned()
        }
        fn write(&mut self, path: &str, text: &str) {
            self.0.insert(path.into(), text.into());
        }
        fn delete(&mut self, path: &str) {
            self.0.remove(path);
        }
    }

    #[test]
    fn writes_the_java_clients_file_byte_for_byte() {
        let mut base = Waypoint::new("Base", 120, 64, -48, OVERWORLD, 0x5B8CFF, Shape::Circle);
        base.death = false;
        let mut fort = Waypoint::new("Fort \"Q\"\n", -1, 70, 2, NETHER, 0x0A0B0C, Shape::Star);
        fort.beam = false;
        let text = write(&[base, fort]);
        // Gson: the escapes trimmed by strip(), colours #RRGGBB in capitals, compact.
        assert_eq!(
            text,
            "{\"format\":1,\"waypoints\":[{\"name\":\"Base\",\"x\":120,\"y\":64,\"z\":-48,\
             \"dimension\":\"minecraft:overworld\",\"color\":\"#5B8CFF\",\"icon\":\"circle\",\
             \"visible\":true,\"beam\":true,\"death\":false},{\"name\":\"Fort \\\"Q\\\"\",\"x\":-1,\
             \"y\":70,\"z\":2,\"dimension\":\"minecraft:the_nether\",\"color\":\"#0A0B0C\",\
             \"icon\":\"star\",\"visible\":true,\"beam\":false,\"death\":false}]}"
        );
        let mut problems = Vec::new();
        let back = read(&text, &mut |m| problems.push(m));
        assert!(problems.is_empty());
        assert_eq!(back.len(), 2);
        assert_eq!(
            (back[1].name(), back[1].icon, back[1].beam, back[1].rgb()),
            ("Fort \"Q\"", Shape::Star, false, 0x0A0B0C)
        );
    }

    #[test]
    fn reads_forgivingly_as_java_does() {
        let text = r##"{"format":2,"waypoints":[
            {"name":"A","x":1.9,"y":64,"z":-3,"color":"#80112233","icon":"arrow","visible":"no"},
            {"x":1,"y":2,"z":3},
            {"name":"B","x":"1","y":2,"z":3},
            5,
            {"name":"C","x":4000000000,"y":0,"z":0,"color":"red","death":true,"extra":[1]}
        ]}"##;
        let mut problems = Vec::new();
        let read = read(text, &mut |m| problems.push(m));
        assert_eq!(problems.len(), 3);
        assert_eq!(read.len(), 2);
        let a = &read[0];
        assert_eq!(
            (a.x, a.y, a.z, a.dimension.as_str()),
            (1, 64, -3, OVERWORLD)
        );
        assert_eq!(
            (a.rgb(), a.icon, a.visible, a.beam, a.death),
            (0x112233, Shape::Circle, true, true, false)
        );
        let c = &read[1];
        assert_eq!(
            (c.x, c.rgb(), c.death),
            (4_000_000_000u64 as i32, COLORS[0], true)
        );
        let mut none = Vec::new();
        assert!(super::read("[1]", &mut |m| none.push(m)).is_empty());
        assert_eq!(none, ["not a waypoints file"]);
        assert!(super::read("{}", &mut |_| panic!()).is_empty());
    }

    #[test]
    fn names_files_as_java_does() {
        // Java: "hello".hashCode() == 99162322, "polygenelubricants".hashCode() == Integer.MIN_VALUE.
        assert_eq!(java_hash("hello"), 99_162_322);
        assert_eq!(java_hash("polygenelubricants"), i32::MIN);
        assert_eq!(
            safe_name("polygenelubricants"),
            "polygenelubricants-80000000"
        );
        // Integer.toHexString(s.hashCode()), from a JVM.
        assert_eq!(
            format!("{:x}", java_hash("world/New World") as u32),
            "27946295"
        );
        assert_eq!(
            format!("{:x}", java_hash("server/play.example.net:25565") as u32),
            "79a2b902"
        );
        // Gson 2.11's JsonElement.toString of the same name, from a JVM.
        let mut s = String::new();
        json_string(&mut s, "Fort \"Q\" <a>&=\u{2028}\u{1}é");
        assert_eq!(s, "\"Fort \\\"Q\\\" <a>&=\\u2028\\u0001é\"");
        assert_eq!(java_hash(""), 0);
        assert_eq!(java_hash("a"), 97);
        assert_eq!(java_hash("ab"), 97 * 31 + 98);
        assert_eq!(
            path("world/New World"),
            format!(
                "waypoints/world_New_World-{:x}.json",
                java_hash("world/New World") as u32
            )
        );
        // A character past the Basic Multilingual Plane is two units: two underscores.
        assert!(safe_name("a😀").starts_with("a__-"));
        assert_eq!(
            safe_name(&"x".repeat(70)).split('-').next().unwrap().len(),
            64
        );
    }

    #[test]
    fn clips_names_as_java_does() {
        let w = Waypoint::new(
            "  Base\t",
            0,
            0,
            0,
            OVERWORLD,
            0xFF00_0000 | 0x123456,
            Shape::Circle,
        );
        assert_eq!((w.name(), w.rgb()), ("Base", 0x123456));
        assert_eq!(clip(&"y".repeat(40)).len(), MAX_NAME);
        assert_eq!(clip("\u{00A0}x"), "\u{00A0}x");
    }

    #[test]
    fn saves_a_second_after_a_change_and_deletes_an_empty_file() {
        let mut files = Files::default();
        let mut store = Waypoints::new();
        store.add(Waypoint::new(
            "Nowhere",
            0,
            0,
            0,
            OVERWORLD,
            0,
            Shape::Circle,
        ));
        assert!(store.all().is_empty(), "nothing outside a world");
        store.open("world/A", &mut files);
        let w = Waypoint::new("Home", 1, 2, 3, OVERWORLD, 0xFF0000, Shape::Heart);
        let id = w.id();
        store.add(w.clone());
        store.add(w);
        assert_eq!(store.all().len(), 1);
        for _ in 0..SAVE_DELAY_TICKS - 1 {
            store.tick(&mut files);
        }
        assert!(files.0.is_empty());
        store.tick(&mut files);
        let file = path("world/A");
        assert!(files.0[&file].contains("\"Home\""));
        // An edited copy saved changes the one it came from.
        let mut copy = store.get(id).unwrap().clone();
        copy.set_name("House");
        let revision = store.revision();
        assert!(store.set(&copy));
        assert!(store.revision() > revision);
        assert_eq!(store.all()[0].name(), "House");
        store.remove(id);
        store.flush(&mut files);
        assert!(!files.0.contains_key(&file));
        // Another world: this one's are saved and let go; coming back reads them.
        store.add(Waypoint::new("Again", 0, 0, 0, OVERWORLD, 0, Shape::Circle));
        store.open("world/B", &mut files);
        assert!(store.all().is_empty());
        store.open("world/A", &mut files);
        assert_eq!(store.all()[0].name(), "Again");
        store.delete("world/A", &mut files);
        assert!(files.0.contains_key(&file), "not the open world's");
        store.close(&mut files);
        store.delete("world/A", &mut files);
        assert!(!files.0.contains_key(&file));
    }

    #[test]
    fn names_drafts_and_places() {
        let mut all = vec![Waypoint::new(
            "Waypoint 2",
            0,
            0,
            0,
            OVERWORLD,
            0,
            Shape::Circle,
        )];
        assert_eq!(next_name(&all), "Waypoint 3");
        all.push(Waypoint::new(
            "Waypoint 3",
            0,
            0,
            0,
            OVERWORLD,
            0,
            Shape::Circle,
        ));
        let d = draft(&all, 1, 2, 3, NETHER);
        assert_eq!(
            (d.name(), d.rgb(), d.icon),
            ("Waypoint 4", COLORS[2], Shape::Circle)
        );
        assert_eq!(dimension_name(NETHER), "the Nether");
        assert_eq!(dimension_name("mymod:deep_dark"), "Deep dark");
        assert_eq!(dimension_name("odd:"), "odd:");
        assert_eq!(distance_text(12.9), "12 m");
        assert_eq!(distance_text(1234.0), "1.2 km");
        assert_eq!(distance_text(34_567.0), "34.5 km");
    }

    #[test]
    fn marks_a_death_and_clears_it_on_the_way_back() {
        let mut files = Files::default();
        let mut store = Waypoints::new();
        store.open("world/A", &mut files);
        let mut watch = DeathWatch::default();
        let at = |x: f64| Player {
            x,
            y: 64.0,
            z: 0.5,
            yaw: 0.0,
            dimension: OVERWORLD.into(),
        };
        watch.tick(&mut store, Some(&at(10.5)), true, true, true);
        watch.tick(&mut store, Some(&at(10.5)), true, true, true);
        assert_eq!(store.all().len(), 1);
        let death = store.death().unwrap();
        assert_eq!(
            (death.name(), death.x, death.icon),
            (DEATH_NAME, 10, Shape::Cross)
        );
        // Respawned beside it: not an arrival.
        watch.tick(&mut store, Some(&at(11.0)), false, true, true);
        assert!(store.death().is_some());
        watch.tick(&mut store, Some(&at(30.0)), false, true, true);
        watch.tick(&mut store, Some(&at(12.0)), false, true, true);
        assert!(store.death().is_none());
        // A second death replaces the first.
        watch.tick(&mut store, Some(&at(1.0)), true, true, false);
        watch.tick(&mut store, Some(&at(1.0)), false, true, false);
        watch.tick(&mut store, Some(&at(2.0)), true, true, false);
        assert_eq!(store.all().len(), 1);
        assert_eq!(store.death().unwrap().x, 2);
    }

    #[test]
    fn on_a_server_a_death_is_marked_for_the_visit_only() {
        let mut files = Files::default();
        let mut store = Waypoints::new();
        store.open("server/play.example.net", &mut files);
        store.add(Waypoint::new(
            "Base",
            0,
            64,
            0,
            OVERWORLD,
            COLORS[1],
            Shape::Circle,
        ));
        let mut watch = DeathWatch::default();
        let p = Player {
            x: 10.5,
            y: 64.0,
            z: 0.5,
            yaw: 0.0,
            dimension: OVERWORLD.into(),
        };
        watch.tick(&mut store, Some(&p), true, true, true);
        assert!(store.death().unwrap().temporary);
        store.flush(&mut files);
        let saved: String = files.0.values().cloned().collect();
        assert!(saved.contains("Base") && !saved.contains("Death"));
        store.close(&mut files);
        store.open("server/play.example.net", &mut files);
        assert_eq!(store.all().len(), 1);

        // In singleplayer it's kept.
        store.open("world/A", &mut files);
        watch.tick(&mut store, Some(&p), false, true, true);
        watch.tick(&mut store, Some(&p), true, true, true);
        assert!(!store.death().unwrap().temporary);
    }

    #[test]
    fn keeps_the_player_without_copying_their_dimension_each_frame() {
        set_player(Some(([1.0, 2.0, 3.0], 90.0, NETHER)));
        set_player(Some(([4.0, 2.0, 3.0], 180.0, NETHER)));
        let p = player().unwrap();
        assert_eq!((p.x, p.heading(), p.dimension.as_str()), (4.0, 0.0, NETHER));
        set_player(None);
        assert!(player().is_none());
    }
}
