// The Schematics mod's game side (module `incari_schematics_world`): the game's blocks for the
// client's schematic code, the module's keys, its HUD elements, and the ghosts, boxes and edges it
// draws in the world through the fork's hook points. The logic is the client's
// (`incari_core::schematic`); this is its glue. Not tested alone: `./incari build --only rust`
// compiles it.
use crate::game::incari_client as c;
use crate::game::incari_schematics as m;
use crate::game::native_hooks::{NativeFill, NativeGhost, NativeGhosts, NativeLine};
use horizon_world::registry::BlockStateId;
use incari_core::schematic::hud::{self as panels, Items};
use incari_core::schematic::session::{self, Ghosts, Keys, Look, Player, WorldBox};
use incari_core::schematic::{Blocks, Mirror, UNKNOWN};
use std::cell::RefCell;
use std::rc::Rc;

/// The game's blocks and world, as the schematic code reads them.
struct World<'a, 'g>(&'a super::Game<'g>);

impl Blocks for World<'_, '_> {
    fn state(&self, name: &str, properties: &[(String, String)], data_version: i32) -> u32 {
        // Horizon has no data fixer: a state by an old name isn't found, and is left out.
        let _ = data_version;
        let props: Vec<(&str, &str)> = properties.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        // Strictly first; a property this version doesn't know then leaves the block as near as can be.
        self.0
            .native_state_of(name, &props, true)
            .or_else(|| self.0.native_state_of(name, &props, false))
            .map_or(UNKNOWN, |s| s.0)
    }
    fn transform(&self, state: u32, mirror: Mirror, turns: u8) -> u32 {
        let code = match mirror {
            Mirror::None => 0,
            Mirror::X => 1,
            Mirror::Z => 2,
        };
        self.0.native_state_transform(BlockStateId(state), code, turns).0
    }
    fn at(&self, x: i32, y: i32, z: i32) -> u32 {
        self.0.native_state_at([x, y, z]).map_or(UNKNOWN, |s| s.0)
    }
    fn is_air(&self, state: u32) -> bool {
        state != UNKNOWN && self.0.native_state_is_air(BlockStateId(state))
    }
    fn block(&self, state: u32) -> u32 {
        self.0.native_state_default(BlockStateId(state)).0
    }
    fn item_id(&self, state: u32) -> String {
        self.0.native_state_item(BlockStateId(state)).unwrap_or_default()
    }
}

/// Raw deflate for the gzipped files, at most `limit` bytes.
fn inflate(data: &[u8], limit: usize) -> Option<Vec<u8>> {
    miniz_oxide::inflate::decompress_to_vec_with_limit(data, limit).ok()
}

/// The game folder (where the `schematics` folder is).
fn game_dir() -> std::path::PathBuf {
    horizon_os::env::current_dir().unwrap_or_default()
}

/// The chooser's picker (the browser's) and writer.
const PLATFORM: incari_core::screens::file_chooser::Platform = incari_core::screens::file_chooser::Platform {
    pick: |accept| {
        #[cfg(target_arch = "wasm32")]
        horizon_os::web::pick_file(m::PICK_KIND, accept);
        #[cfg(not(target_arch = "wasm32"))]
        let _ = accept;
    },
    take_picked: || {
        #[cfg(target_arch = "wasm32")]
        {
            horizon_os::web::take_picked_named(m::PICK_KIND)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            None
        }
    },
    write: |path, data| {
        if !incari_core::host_extras::safe_path(path) {
            return false;
        }
        let path = game_dir().join(path);
        if let Some(dir) = path.parent() {
            let _ = horizon_os::fs::create_dir_all(dir);
        }
        horizon_os::fs::write(&path, data).is_ok()
    },
    make_folder: |path| {
        if incari_core::host_extras::safe_path(path) {
            let _ = horizon_os::fs::create_dir_all(game_dir().join(path));
        }
    },
};

#[derive(Default)]
struct State {
    installed: bool,
    watch: m::WorldWatch,
    /// This frame's boxes (scratch, kept so a frame allocates nothing once warm).
    boxes: Vec<WorldBox>,
}

// The window's thread only: ticks, frames, keys and the HUD all run there.
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn player(game: &super::Game<'_>) -> Option<Player> {
    game.native_player().map(|p| Player { pos: [p.x, p.y, p.z], yaw: p.yaw })
}

/// Once a tick, the window's.
pub(super) fn tick(game: &mut super::Game<'_>) {
    if !game.native_window() || !c::ready() {
        return;
    }
    let world = game.native_world_id();
    let look = c::settings(m::ID, Look::of);
    let g: &super::Game<'_> = game;
    let blocks = World(g);
    let me = player(g);
    let left = STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        if !state.installed {
            state.installed = true;
            incari_core::screens::file_chooser::install(PLATFORM);
        }
        state.watch.left(world.as_deref())
    });
    let said = session::with(|s| {
        if left {
            s.left_world(&blocks);
        }
        if let Some(look) = &look {
            s.tick(&blocks, me, look, inflate);
        }
        (s.take_messages(), std::mem::take(&mut s.wants_one_layer))
    });
    let Some((messages, one_layer)) = said else {
        return;
    };
    for message in messages {
        game.native_chat(&message);
    }
    if one_layer {
        // Layers: One layer (LAYERS' third choice).
        c::set_setting(m::ID, "layers", incari_core::framework::setting::Val::Choice(2));
    }
}

/// The module's keys, while playing; sneaking reverses Turn, Nudge, Up and Layer.
pub(super) fn key(game: &mut super::Game<'_>, name: &str, pressed: bool) {
    if !pressed || !game.native_playing() {
        return;
    }
    let Some((keys, look)) = c::settings(m::ID, |s| (Keys::of(s), Look::of(s))) else {
        return;
    };
    let sneaking = game.native_mapping("key.sneak").is_some_and(|(down, _)| down);
    let Some(action) = keys.action(incari_core::input::Key::from_name(name), sneaking) else {
        return;
    };
    let g: &super::Game<'_> = game;
    let blocks = World(g);
    let me = player(g);
    session::with(|s| s.act(action, &blocks, me, &look));
}

/// The two HUD elements. Hot path: their lines are cached by the client's code.
pub(super) fn hud(game: &mut super::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    let Some(look) = c::settings(m::ID, Look::of) else {
        return;
    };
    let g: &super::Game<'_> = game;
    c::element(g, batch, m::ID, "status", |canvas| session::with(|s| panels::status(canvas, s, &look)).flatten());
    // An item's name: its own, else its block's (the game's text for a missing key is the key).
    let name = |id: &str| {
        let keys = m::name_keys(id);
        keys.iter().map(|k| g.native_translate(k)).find(|t| !keys.contains(t)).unwrap_or_default()
    };
    let in_world = g.native_player().is_some();
    let carried = |id: &str| in_world.then(|| g.native_item_count(id));
    let items = Items { name: &name, carried: &carried };
    c::element(g, batch, m::ID, "materials", |canvas| session::with(|s| panels::materials(canvas, s, &items)).flatten());
}

/// This frame's boxes round the player, into `STATE.boxes`, then `f` with them.
fn with_boxes<R>(game: &super::Game<'_>, look: &Look, f: impl FnOnce(&[WorldBox]) -> R) -> Option<R> {
    let p = game.native_player()?;
    STATE.with(|cell| {
        let mut state = cell.try_borrow_mut().ok()?;
        let state = &mut *state;
        session::with(|s| s.boxes([p.x, p.y, p.z], look, &mut state.boxes))?;
        Some(f(&state.boxes))
    })
}

/// The schematic's box and its wrong blocks' edges. Horizon keeps 1024 lines a frame: the
/// outline's first, then each problem's while there's room (their fills show them all).
pub(super) fn lines(game: &super::Game<'_>, lines: &mut Vec<NativeLine>) {
    let Some(look) = c::settings(m::ID, Look::of) else {
        return;
    };
    with_boxes(game, &look, |boxes| {
        for b in boxes {
            if lines.len() + 12 > 1024 {
                break;
            }
            lines.extend(m::box_edges(b.min, b.max).map(|(start, end)| NativeLine { start, end, argb: b.argb, width: 2.0 }));
        }
    });
}

/// The ghosts' box, opacity and range this frame, and the problems' fills.
pub(super) fn ghosts(game: &super::Game<'_>, ghosts: &mut NativeGhosts) {
    let Some(look) = c::settings(m::ID, Look::of) else {
        return;
    };
    let shown = session::with(|s| s.loaded().map(|l| (s.ghost_bounds(), Rc::as_ptr(&l.schematic) as u64))).flatten();
    let Some((bounds, source)) = shown else {
        return;
    };
    if look.ghosts != Ghosts::None {
        ghosts.source = source;
        ghosts.bounds = bounds;
        ghosts.alpha = look.alpha();
        ghosts.range = look.range as u32;
    }
    with_boxes(game, &look, |boxes| {
        ghosts.fills.extend(boxes.iter().filter(|b| b.fill != 0).map(|b| NativeFill {
            min: b.min.map(f64::from),
            max: b.max.map(f64::from),
            argb: b.fill,
        }));
    });
}

/// What section `section`'s ghosts were made from.
pub(super) fn ghost_revision(section: [i32; 3], revision: &mut u64) {
    if let Some(r) = session::with(|s| s.ghost_revision(section)) {
        *revision = r;
    }
}

/// Section `section`'s ghosts, while Horizon meshes it.
pub(super) fn ghost_section(game: &super::Game<'_>, section: [i32; 3], ghosts: &mut Vec<NativeGhost>) {
    let Some(look) = c::settings(m::ID, Look::of) else {
        return;
    };
    let blocks = World(game);
    session::with(|s| {
        for at in 0..4096u16 {
            let [x, y, z] = m::cell_block(section, at);
            if let Some(state) = s.ghost_at(x, y, z, &look, &blocks) {
                ghosts.push(NativeGhost { at, state: BlockStateId(state) });
            }
        }
    });
}
