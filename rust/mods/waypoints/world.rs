// The Waypoints mod's game side (module `incari_waypoints_world`): the world's waypoints and
// remembered map opened as the player joins and closed as they leave, the player handed to the
// client each frame, deaths marked, the overlay drawn and the keys bound. The logic is the
// client's (`incari_core::waypoint`, `waypoint_overlay`, `map_memory`); this is its glue. Not
// tested alone: `./incari build --only rust` compiles it.
use crate::game::incari_client as c;
use crate::game::incari_waypoints as m;
use incari_core::framework::system::Storage;
use incari_core::input::Key;
use incari_core::map_memory::{self, Bytes, Zlib};
use incari_core::waypoint::{self, DeathWatch};
use incari_core::waypoint_overlay::{Look, Overlay, View};
use std::cell::RefCell;

/// `incari/` in the game folder.
fn folder() -> std::path::PathBuf {
    horizon_os::env::current_dir().unwrap_or_default().join("incari")
}

fn read(path: &str) -> Option<Vec<u8>> {
    horizon_os::fs::read(folder().join(path)).ok()
}

fn write(path: &str, data: &[u8]) {
    let path = folder().join(path);
    if let Some(dir) = path.parent() {
        let _ = horizon_os::fs::create_dir_all(dir);
    }
    if let Err(e) = horizon_os::fs::write(&path, data) {
        log::warn!("Incari: couldn't save {}: {e}", path.display());
    }
}

/// The waypoints' files, under `incari/modules/` as the profiles are.
struct ModuleFiles;

impl Storage for ModuleFiles {
    fn read(&self, path: &str) -> Option<String> {
        read(&format!("modules/{path}")).and_then(|b| String::from_utf8(b).ok())
    }
    fn write(&mut self, path: &str, text: &str) {
        write(&format!("modules/{path}"), text.as_bytes());
    }
    fn delete(&mut self, path: &str) {
        let _ = horizon_os::fs::remove_file(folder().join("modules").join(path));
    }
}

/// The remembered map's files, under `incari/map/`.
struct MapFiles;

impl Bytes for MapFiles {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        read(path)
    }
    fn write(&mut self, path: &str, data: &[u8]) {
        write(path, data);
    }
}

/// zlib at its fastest level, as the Java client deflates the map's regions.
const ZLIB: Zlib = Zlib {
    deflate: |bytes| miniz_oxide::deflate::compress_to_vec_zlib(bytes, 1),
    inflate: |bytes| miniz_oxide::inflate::decompress_to_vec_zlib(bytes).ok(),
};

#[derive(Default)]
struct State {
    tracker: m::WorldTracker,
    changes: Vec<m::Change>,
    deaths: DeathWatch,
    overlay: Overlay,
    zlib: bool,
}

// The window's thread only: ticks, frames, keys and the HUD all run there.
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Once a tick, the window's: the world followed, deaths watched, due saves.
pub(super) fn tick(game: &mut super::Game<'_>) {
    if !game.native_window() || !c::ready() {
        return;
    }
    // Planned fork helper: `server/<address>` or `world/<folder>`, as `Game.world()` names it.
    let world = game.native_world_id();
    let dead = game.native_ui_screen() == "dead";
    let watch = c::settings(m::ID, |s| (s.bool("deaths"), s.bool("clear-deaths")));
    let remember = c::settings("minimap", |s| s.bool("remember")).unwrap_or(true);
    STATE.with(|cell| {
        let Ok(mut state) = cell.try_borrow_mut() else {
            return;
        };
        let state = &mut *state;
        if !state.zlib {
            state.zlib = true;
            map_memory::with(|mm| mm.set_zlib(ZLIB));
        }
        state.changes.clear();
        state.tracker.follow(world.as_deref(), &mut state.changes);
        for change in state.changes.drain(..) {
            match change {
                m::Change::Leave => {
                    waypoint::with(|w| w.close(&mut ModuleFiles));
                    map_memory::with(|mm| mm.close(&mut MapFiles));
                }
                // Before anything joining reads them.
                m::Change::Join(name) => {
                    waypoint::with(|w| w.open(&name, &mut ModuleFiles));
                    map_memory::with(|mm| mm.open(&name, &mut MapFiles));
                }
            }
        }
        waypoint::with(|w| {
            if let Some((mark, clear)) = watch {
                waypoint::with_player(|p| state.deaths.tick(w, p, dead, mark, clear));
            }
            w.tick(&mut ModuleFiles);
            for problem in w.problems.drain(..) {
                log::warn!("Incari: waypoints: {problem}");
            }
        });
        map_memory::with(|mm| {
            mm.set_remember(remember, &MapFiles);
            mm.tick(&mut MapFiles);
            for problem in mm.problems.drain(..) {
                log::warn!("Incari: map: {problem}");
            }
        });
    });
}

/// Once a frame: where the player is (no copy of their dimension unless it changed).
pub(super) fn frame(game: &mut super::Game<'_>) {
    if !game.native_window() {
        return;
    }
    let player = game.native_player();
    waypoint::set_player(player.as_ref().map(|p| ([p.x, p.y, p.z], p.yaw, p.dimension)));
}

/// The module's keys, while playing.
pub(super) fn key(game: &mut super::Game<'_>, name: &str, pressed: bool) {
    if !pressed || !game.native_playing() {
        return;
    }
    let Some((new, list)) = c::settings(m::ID, |s| (s.key("new"), s.key("list"))) else {
        return;
    };
    let Some(action) = m::action(Key::from_name(name), Key::None, new, list) else {
        return;
    };
    let in_world = waypoint::with(|w| w.world().is_some()).unwrap_or(false);
    if !in_world {
        return;
    }
    let screen: Box<dyn incari_core::screen::Screen> = match action {
        m::Action::List => Box::new(incari_core::screens::waypoint_list::WaypointList::new()),
        m::Action::New => {
            let Some(p) = waypoint::player() else {
                return;
            };
            let [x, y, z] = p.block();
            let Some(draft) = waypoint::with(|w| waypoint::draft(w.all(), x, y, z, &p.dimension)) else {
                return;
            };
            Box::new(incari_core::screens::waypoint_edit::WaypointEdit::new(draft, true))
        }
    };
    // Planned client helper: opens an Incari screen over the world.
    c::open(game, screen);
}

/// The waypoints over the world. Hot path: once a frame, nothing allocated once warm.
pub(super) fn hud(game: &mut super::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    let Some(look) = c::settings(m::ID, Look::of) else {
        return;
    };
    let game: &super::Game<'_> = game;
    let project = |at: [f64; 3]| game.native_project(at);
    // Planned client helper: draws with the client's painter over the HUD, at the screen's origin.
    c::overlay(game, batch, |p, screen| {
        waypoint::with_player(|player| {
            let Some(player) = player else {
                return;
            };
            let view = View { player, screen, project: &project };
            waypoint::with(|store| {
                STATE.with(|cell| {
                    if let Ok(mut state) = cell.try_borrow_mut() {
                        state.overlay.draw(p, store, &look, &view);
                    }
                })
            });
        });
    });
}

/// A chunk Horizon's minimap scanned, as the map shades it: remembered for the world map.
pub(super) fn map_tile(dimension: &str, chunk: [i32; 2], colors: &[u32; 256]) {
    map_memory::with(|m| m.record(&mut MapFiles, dimension, chunk[0], chunk[1], colors));
}
