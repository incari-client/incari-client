// Incari Client's game side (ADR 0069): the client (`incari_core`) kept for the window's thread,
// what it asks of the game through Horizon's `native_*` helpers, and its draw lists replayed into
// the HUD batch. Not tested alone: `./incari build --only rust` compiles it.
use horizon_client::hud::{HudAtlas, HudBatch, MOD_CANVAS_SPRITE, rgba};
use horizon_client::text::Text;
use incari_core::client::Client;
use incari_core::draw::{Cmd, DrawList};
use incari_core::input::{Input, Key};
use incari_core::screen::Host;
use std::cell::{Cell, RefCell};

// The window's thread only: in the browser it's the page's, where a lock that waits traps.
thread_local! {
    static CLIENT: RefCell<Option<Client>> = const { RefCell::new(None) };
    static START: Cell<Option<horizon_os::time::Instant>> = const { Cell::new(None) };
    /// HUD elements drawn while the HUD editor is open, kept for over the editor's blur.
    static DEFERRED: RefCell<Vec<DrawList>> = const { RefCell::new(Vec::new()) };
    /// What the build was made from, read once.
    static BUILD: std::cell::OnceCell<incari_core::release::BuildInfo> = const { std::cell::OnceCell::new() };
    /// Downloads, image copies, folders and links asked for, done once the call is over.
    static EXTRA: RefCell<Vec<Extra>> = const { RefCell::new(Vec::new()) };
}

/// Incari's online service (ADR 0058): the protocol is `incari_core::online`, the socket is here.
/// Only the threaded browser build has a WebSocket client; elsewhere the service stays off and the
/// account screens say so.
mod online_link {
    use incari_core::online::{self, Effect};
    use incari_core::screen::Host;

    /// `incari.online.url` from the bundled `incari.properties` (empty turns accounts off).
    fn url() -> String {
        if !cfg!(all(target_arch = "wasm32", target_feature = "atomics")) {
            return String::new();
        }
        let text = std::str::from_utf8(horizon_client::native_data!("incari_client", "incari.properties"));
        incari_core::release::property(text.unwrap_or(""), "incari.online.url")
            .unwrap_or_default()
            .to_owned()
    }

    fn epoch_ms() -> i64 {
        horizon_os::time::SystemTime::now()
            .duration_since(horizon_os::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64)
    }

    /// Once, when the client loads: the service, resuming the saved session.
    pub(super) fn install(game_version: &str, session: Option<Vec<u8>>, utc_offset_ms: i64) {
        let session = session.and_then(|b| String::from_utf8(b).ok());
        let seed = epoch_ms() as u64 ^ 0x9E37_79B9_7F4A_7C15;
        let service = online::Online::new(&url(), &online::build_name(game_version), session.as_deref(), seed);
        online::install(service);
        online::set_utc_offset(utc_offset_ms);
    }

    /// Once a frame: what the socket did, the clock, then what the service asks.
    pub(super) fn pump(host: &mut dyn Host) {
        let now = host.now_ms();
        online::with(|o| {
            for up in transport::take() {
                match up {
                    transport::Up::Opened(c) => o.opened(c),
                    transport::Up::Text(c, text) => o.received(c, &text),
                    transport::Up::Closed(c, why) => o.closed(c, &why),
                }
            }
            o.tick(now, epoch_ms());
            for effect in o.take_effects() {
                match effect {
                    Effect::Connect { connection, url } => transport::connect(connection, url),
                    Effect::Send { connection, text } => transport::send(connection, text),
                    Effect::Close { connection } => transport::close(connection),
                    Effect::SaveSession(text) => host.write("online/session.json", text.as_bytes()),
                    Effect::DeleteSession => host.delete("online/session.json"),
                }
            }
            for warning in o.take_warnings() {
                log::warn!("Incari: {warning}");
            }
        });
    }

    /// The socket, on a worker: the page's thread may not wait. Frames cross by channels; the
    /// window's thread only sends and drains them (neither waits).
    #[cfg(all(target_arch = "wasm32", target_feature = "atomics"))]
    mod transport {
        use horizon_os::net::{Message, WebSocket};
        use std::cell::RefCell;
        use std::sync::mpsc::{Receiver, Sender, channel};

        pub(super) enum Up {
            Opened(u32),
            Text(u32, String),
            Closed(u32, String),
        }
        enum Down {
            Text(String),
            Close,
        }

        thread_local! {
            static UP: (Sender<Up>, Receiver<Up>) = channel();
            static DOWN: RefCell<Option<(u32, Sender<Down>)>> = const { RefCell::new(None) };
        }

        pub(super) fn connect(connection: u32, url: String) {
            let up = UP.with(|(tx, _)| tx.clone());
            let (tx, rx) = channel::<Down>();
            // Replacing the sender ends the last socket's writer (its recv fails), which shuts it.
            DOWN.with(|d| *d.borrow_mut() = Some((connection, tx)));
            let worker_up = up.clone();
            let spawned = horizon_os::thread::Builder::new().name("incari-online".into()).spawn(move || {
                let up = worker_up;
                let socket = match WebSocket::connect_url(&url, horizon_os::time::Duration::from_secs(15)) {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = up.send(Up::Closed(connection, e.to_string()));
                        return;
                    }
                };
                let _ = up.send(Up::Opened(connection));
                let reader = socket.try_clone();
                let reader_up = up.clone();
                let _ = horizon_os::thread::spawn(move || {
                    loop {
                        let text = match reader.recv() {
                            Ok(Some(Message::Text(text))) => text,
                            Ok(Some(Message::Binary(data))) => String::from_utf8_lossy(&data).into_owned(),
                            Ok(None) | Err(_) => {
                                let _ = reader_up.send(Up::Closed(connection, "closed".into()));
                                break;
                            }
                        };
                        if reader_up.send(Up::Text(connection, text)).is_err() {
                            break;
                        }
                    }
                });
                while let Ok(down) = horizon_os::sync::recv(&rx) {
                    match down {
                        Down::Text(text) => {
                            if socket.send(Message::Text(text)).is_err() {
                                break;
                            }
                        }
                        Down::Close => break,
                    }
                }
                socket.shutdown();
            });
            if let Err(e) = spawned {
                let _ = up.send(Up::Closed(connection, e.to_string()));
            }
        }

        pub(super) fn send(connection: u32, text: String) {
            DOWN.with(|d| {
                if let Some((c, tx)) = &*d.borrow()
                    && *c == connection
                {
                    let _ = tx.send(Down::Text(text));
                }
            });
        }

        pub(super) fn close(connection: u32) {
            DOWN.with(|d| {
                let mut d = d.borrow_mut();
                if d.as_ref().is_some_and(|(c, _)| *c == connection)
                    && let Some((_, tx)) = d.take()
                {
                    let _ = tx.send(Down::Close);
                }
            });
        }

        pub(super) fn take() -> Vec<Up> {
            UP.with(|(_, rx)| rx.try_iter().collect())
        }
    }

    /// No WebSocket client here: the URL is empty, so the service never asks for one.
    #[cfg(not(all(target_arch = "wasm32", target_feature = "atomics")))]
    mod transport {
        #[allow(dead_code)] // never made: there is no socket here
        pub(super) enum Up {
            Opened(u32),
            Text(u32, String),
            Closed(u32, String),
        }
        pub(super) fn connect(_: u32, _: String) {}
        pub(super) fn send(_: u32, _: String) {}
        pub(super) fn close(_: u32) {}
        pub(super) fn take() -> Vec<Up> {
            Vec::new()
        }
    }
}

thread_local! {
    /// When to connect, joins and leaves, the tab list.
    static TRACKER: RefCell<incari_core::online::game::WorldTracker> = RefCell::new(Default::default());
    /// The chat menu waits for the chat screen to close.
    static OPEN_CHAT: Cell<bool> = const { Cell::new(false) };
    /// The GUI's size at the last frame (input positions are measured against it).
    static GUI_SIZE: Cell<[f32; 2]> = const { Cell::new([0.0, 0.0]) };
}

/// `incari.properties`, compiled in.
fn build() -> incari_core::release::BuildInfo {
    BUILD.with(|b| {
        b.get_or_init(|| {
            let text = std::str::from_utf8(horizon_client::native_data!("incari_client", "incari.properties"));
            incari_core::release::BuildInfo::from_properties(text.unwrap_or(""), !cfg!(debug_assertions))
        })
        .clone()
    })
}

/// The page's recorder as the game last told it (`native_recorder`): starting and stopping wait
/// for the call to end, as the rest of `Extra`.
struct PageRecorder {
    supported: bool,
    state: u32,
    error: String,
    elapsed_ms: u64,
}

impl incari_core::recorder::Recorder for PageRecorder {
    fn supported(&self) -> bool {
        self.supported
    }
    fn start(&mut self, o: incari_core::recorder::Options, file_name: &str) {
        let start = Extra::Record(o, file_name.to_owned());
        EXTRA.with(|e| e.borrow_mut().push(start));
        self.state = 1;
    }
    fn stop(&mut self) {
        EXTRA.with(|e| e.borrow_mut().push(Extra::StopRecording));
    }
    fn state(&self) -> incari_core::recorder::State {
        match self.state {
            1 => incari_core::recorder::State::Starting,
            2 => incari_core::recorder::State::Recording,
            _ => incari_core::recorder::State::Idle,
        }
    }
    fn error(&self) -> Option<String> {
        (!self.error.is_empty()).then(|| self.error.clone())
    }
    fn elapsed_ms(&self) -> u64 {
        self.elapsed_ms
    }
}

/// The local time, for a recording's name.
fn local_time() -> [u32; 6] {
    use chrono::{Datelike, Timelike};
    let now = chrono::Local::now();
    [now.year() as u32, now.month(), now.day(), now.hour(), now.minute(), now.second()]
}

/// What the screens asked of the page or the desktop.
enum Extra {
    Record(incari_core::recorder::Options, String),
    StopRecording,
    Download(String, Vec<u8>),
    Image(Vec<u8>),
    Path(String),
    Link(String),
}

/// The game folder (where F2 writes `screenshots/`).
fn game_dir() -> std::path::PathBuf {
    horizon_os::env::current_dir().unwrap_or_default()
}

/// Files under the game folder for Incari's screens (the screenshots gallery), every path checked
/// to stay inside it.
struct Extras;

impl incari_core::host_extras::HostExtras for Extras {
    fn list_files(&self, folder: &str) -> Vec<incari_core::host_extras::FileEntry> {
        use incari_core::host_extras::{FileEntry, safe_path};
        if !safe_path(folder) {
            return Vec::new();
        }
        let Ok(dir) = horizon_os::fs::read_dir(game_dir().join(folder)) else {
            return Vec::new();
        };
        dir.flatten()
            .filter_map(|e| {
                let m = e.metadata().ok().filter(|m| m.is_file())?;
                let modified_ms = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(horizon_os::time::UNIX_EPOCH).ok())
                    .map_or(0.0, |d| d.as_secs_f64() * 1000.0);
                Some(FileEntry {
                    name: e.file_name().to_string_lossy().into_owned(),
                    modified_ms,
                })
            })
            .collect()
    }
    fn read_file(&self, path: &str) -> Option<Vec<u8>> {
        incari_core::host_extras::safe_path(path)
            .then(|| horizon_os::fs::read(game_dir().join(path)).ok())
            .flatten()
    }
    fn file_exists(&self, path: &str) -> bool {
        incari_core::host_extras::safe_path(path) && horizon_os::fs::metadata(game_dir().join(path)).is_ok()
    }
    fn rename_file(&mut self, from: &str, to: &str) -> bool {
        use incari_core::host_extras::safe_path as ok;
        ok(from) && ok(to) && horizon_os::fs::rename(game_dir().join(from), game_dir().join(to)).is_ok()
    }
    fn delete_file(&mut self, path: &str) -> bool {
        if !incari_core::host_extras::safe_path(path) {
            return false;
        }
        let path = game_dir().join(path);
        horizon_os::fs::remove_file(&path).is_ok() || horizon_os::fs::metadata(&path).is_err()
    }
    fn shows_folders(&self) -> bool {
        !cfg!(target_arch = "wasm32")
    }
    fn open_path(&mut self, path: &str) {
        if incari_core::host_extras::safe_path(path) {
            EXTRA.with(|e| e.borrow_mut().push(Extra::Path(path.to_owned())));
        }
    }
    fn download(&mut self, name: &str, data: Vec<u8>) -> bool {
        EXTRA.with(|e| e.borrow_mut().push(Extra::Download(name.to_owned(), data)));
        cfg!(target_arch = "wasm32")
    }
    fn set_clipboard_image(&mut self, png: &[u8]) -> bool {
        EXTRA.with(|e| e.borrow_mut().push(Extra::Image(png.to_vec())));
        cfg!(target_arch = "wasm32")
    }
    fn open_link(&mut self, url: &str) {
        EXTRA.with(|e| e.borrow_mut().push(Extra::Link(url.to_owned())));
    }
}

thread_local! {
    /// The Eaglercraft server's pause menu as the client takes it, made once a menu.
    static SERVER_MENU: RefCell<Option<std::sync::Arc<incari_core::server_menu::ServerMenu>>> =
        const { RefCell::new(None) };
}

/// Horizon's copy of an Eaglercraft server's pause menu in the client's types.
fn server_menu(menu: &horizon_protocol::eagler::PauseMenu) -> incari_core::server_menu::ServerMenu {
    use incari_core::server_menu::{ServerButton, ServerMenu};
    let button = |b: &horizon_protocol::eagler::PauseButton| ServerButton { text: b.text.clone(), url: b.url.clone() };
    ServerMenu {
        serial: menu.serial,
        server_info: menu.server_info.as_ref().map(button),
        discord: menu.discord.as_ref().map(button),
        icon_keys: menu.icon_keys.clone(),
        icons: menu
            .icons
            .iter()
            .map(|i| incari_core::canvas::Image { width: i.width, height: i.height, rgba: i.rgba.clone() })
            .collect(),
    }
}

/// The game's font, measured as the HUD draws it.
struct Font<'a>(&'a HudAtlas);

impl incari_core::paint::Measure for Font<'_> {
    fn width(&self, text: &str) -> f32 {
        HudBatch::text_width(self.0, text)
    }
}

/// What a call asks of the game that needs it mutably, done once the call is over.
enum Request {
    Open(&'static str),
    Connect(String),
    Hold(bool),
    Copy(String),
}

/// The game as Incari's client sees it during a call.
struct Shell<'a, 'g> {
    game: &'a super::Game<'g>,
    screen: &'static str,
    clipboard: String,
    requests: Vec<Request>,
}

/// `incari/` in the game folder.
fn folder() -> std::path::PathBuf {
    horizon_os::env::current_dir().unwrap_or_default().join("incari")
}

impl Host for Shell<'_, '_> {
    fn now_ms(&self) -> f64 {
        let start = START.with(|s| {
            s.get().unwrap_or_else(|| {
                let now = horizon_os::time::Instant::now();
                s.set(Some(now));
                now
            })
        });
        start.elapsed().as_secs_f64() * 1000.0
    }
    fn screen(&self) -> &str {
        self.screen
    }
    fn open(&mut self, what: &str) -> bool {
        let what: &'static str = match what {
            "singleplayer" => "singleplayer",
            "multiplayer" => "multiplayer",
            "options" => "options",
            "language" => "language",
            "accessibility" => "accessibility",
            "packs" => "packs",
            "credits" => "credits",
            "quit" => "quit",
            "resume" => "resume",
            "pause" => "pause",
            "disconnect" => "disconnect",
            "advancements" => "advancements",
            "stats" => "stats",
            "lan" => "lan",
            "friends" => "friends",
            "links" => "links",
            "additions" => "additions",
            "report_bugs" => "report_bugs",
            "feedback" => "feedback",
            "social" => "social",
            "profile" => "profile",
            "title" => "title",
            _ => return false,
        };
        self.requests.push(Request::Open(what));
        true
    }
    fn connect(&mut self, address: &str) -> bool {
        self.requests.push(Request::Connect(address.to_owned()));
        true
    }
    fn hold_world(&mut self, held: bool) {
        self.requests.push(Request::Hold(held));
    }
    fn translate(&self, key: &str) -> String {
        self.game.native_translate(key)
    }
    fn player_name(&self) -> String {
        self.game.native_player_name()
    }
    fn game_version(&self) -> String {
        horizon_core::VANILLA_VERSION.to_owned()
    }
    fn on_server(&self) -> bool {
        self.game.native_external_server()
    }
    fn server_menu(&self) -> Option<std::sync::Arc<incari_core::server_menu::ServerMenu>> {
        let menu = self.game.native_eagler_pause_menu()?;
        Some(SERVER_MENU.with(|cache| {
            let mut cache = cache.borrow_mut();
            match &*cache {
                Some(kept) if kept.serial == menu.serial => kept.clone(),
                _ => {
                    let kept = std::sync::Arc::new(server_menu(&menu));
                    *cache = Some(kept.clone());
                    kept
                }
            }
        }))
    }
    fn pause_server_button(&self) -> Option<(String, &'static str)> {
        self.game.native_pause_server_button()
    }
    fn friends_key(&self) -> Key {
        Key::from_name(self.game.native_bound_key("key.friends"))
    }
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        horizon_os::fs::read(folder().join(path)).ok()
    }
    fn write(&mut self, path: &str, data: &[u8]) {
        let path = folder().join(path);
        if let Some(dir) = path.parent() {
            let _ = horizon_os::fs::create_dir_all(dir);
        }
        if let Err(e) = horizon_os::fs::write(&path, data) {
            log::warn!("Incari: couldn't save {}: {e}", path.display());
        }
    }
    fn delete(&mut self, path: &str) {
        let _ = horizon_os::fs::remove_file(folder().join(path));
    }
    fn own_skin(&self) -> Option<(Vec<u8>, bool)> {
        self.game.native_own_skin()
    }
    fn recorder(&self) -> Box<dyn incari_core::recorder::Recorder> {
        let (supported, state, error, elapsed_ms) = self.game.native_recorder();
        Box::new(PageRecorder { supported, state, error, elapsed_ms })
    }
    fn local_time(&self) -> fn() -> [u32; 6] {
        local_time
    }
    fn extras(&self) -> Box<dyn incari_core::host_extras::HostExtras> {
        Box::new(Extras)
    }
    fn build_info(&self) -> incari_core::release::BuildInfo {
        build()
    }
    fn clipboard(&self) -> String {
        self.clipboard.clone()
    }
    fn set_clipboard(&mut self, text: &str) {
        self.requests.push(Request::Copy(text.to_owned()));
    }
}

/// Runs `f` with the client, made and loaded the first time; None while it's in use (a skin draw
/// inside one of its own calls).
fn with<R>(game: &super::Game<'_>, f: impl FnOnce(&mut Client, &mut Shell<'_, '_>) -> R) -> (Option<R>, Vec<Request>) {
    let mut shell = Shell {
        game,
        screen: game.native_ui_screen(),
        clipboard: String::new(),
        requests: Vec::new(),
    };
    let r = CLIENT.with(|cell| {
        let mut slot = cell.try_borrow_mut().ok()?;
        if slot.is_none() {
            let assets = incari_core::paint::Assets::load(
                horizon_client::native_data!("incari_client", "icon_256.png"),
                horizon_client::native_data!("incari_client", "title_backdrop.png"),
                horizon_client::native_data!("incari_client", "michroma.ttf"),
            );
            let mut client = Client::new(assets);
            client.load(&mut shell);
            let session = shell.read("online/session.json");
            online_link::install(horizon_core::VANILLA_VERSION, session, game.native_utc_offset_ms());
            *slot = Some(client);
            install_skin();
            install_featured();
        }
        Some(f(slot.as_mut()?, &mut shell))
    });
    (r, shell.requests)
}

/// Does what a call asked of the game.
fn run(game: &mut super::Game<'_>, requests: Vec<Request>) {
    for r in requests {
        match r {
            Request::Open(what) => {
                if !game.native_open(what) {
                    log::warn!("Incari: the game couldn't open {what} here");
                }
            }
            Request::Connect(address) => {
                game.native_connect(&address);
            }
            Request::Hold(true) => {
                game.native_mod_screen_open("incari_client");
            }
            Request::Hold(false) => game.native_mod_screen_close("incari_client"),
            Request::Copy(text) => game.native_set_clipboard(&text),
        }
    }
    for x in EXTRA.with(|e| std::mem::take(&mut *e.borrow_mut())) {
        match x {
            Extra::Download(name, data) => {
                game.native_download(&name, data);
            }
            Extra::Image(png) => {
                game.native_set_clipboard_image(&png);
            }
            Extra::Path(path) => game.native_open_path(&game_dir().join(path).to_string_lossy()),
            Extra::Link(url) => {
                game.native_open_url(&url);
            }
            Extra::Record(o, name) => game.native_recorder_start(
                o.frames_per_second,
                o.video_kilobits,
                o.game_volume,
                o.microphone,
                o.microphone_volume,
                &name,
            ),
            Extra::StopRecording => game.native_recorder_stop(),
        }
    }
}

/// Opens an Incari screen over the world (a mod's key).
pub(super) fn open(game: &mut super::Game<'_>, screen: Box<dyn incari_core::screen::Screen>) {
    let (_, requests) = with(game, |client, shell| client.open_in_world(shell, screen));
    run(game, requests);
}

/// Draws with the client's painter over the HUD, at the screen's origin. Hot path.
pub(super) fn overlay(
    game: &super::Game<'_>,
    batch: &mut HudBatch,
    draw: impl FnOnce(&mut incari_core::paint::Painter<'_>, [f32; 2]),
) {
    let atlas = game.native_atlas();
    let (screen, scale) = ([batch.width, batch.height], batch.scale);
    let _ = with(game, |client, _| {
        client.overlay(&Font(atlas), scale, screen, draw);
        replay(&client.list, atlas, batch, Some(game));
    });
}

/// An F2 screenshot was saved: copied too, with Copy screenshots on.
pub(super) fn screenshot_saved(game: &mut super::Game<'_>, path: &std::path::Path) {
    let copy = CLIENT.with(|c| c.try_borrow().ok().and_then(|c| c.as_ref().map(|c| c.prefs.copy_screenshots)));
    if copy == Some(true)
        && let Ok(png) = horizon_os::fs::read(path)
    {
        game.native_set_clipboard_image(&png);
    }
}

/// Module `id`'s settings while it's on (None when it's off): what each feature's hooks read.
/// Ids and setting ids are the Java client's (`incari_core::modules`).
pub(super) fn settings<R>(id: &str, f: impl FnOnce(&incari_core::framework::setting::Settings) -> R) -> Option<R> {
    CLIENT.with(|cell| {
        let c = cell.try_borrow().ok()?;
        c.as_ref()?.settings(id).map(f)
    })
}

/// Sets module `id`'s setting `setting` from a hook (a key that zooms the minimap): kept in the
/// profile like a change in the menu. False when the client is busy or has no such setting.
pub(super) fn set_setting(id: &str, setting: &str, value: incari_core::framework::setting::Val) -> bool {
    CLIENT.with(|cell| {
        let Ok(mut c) = cell.try_borrow_mut() else {
            return false;
        };
        let Some(c) = c.as_mut() else {
            return false;
        };
        let set = c
            .modules
            .module_mut(id)
            .and_then(|m| m.settings.get_mut(setting))
            .is_some_and(|s| s.set(value));
        if set {
            c.modules.changed();
        }
        set
    })
}

/// Turns module `id` on or off from a hook (Old Textures following the pack picked in the game's
/// own screen).
pub(super) fn set_enabled(id: &str, on: bool) {
    CLIENT.with(|cell| {
        if let Ok(mut c) = cell.try_borrow_mut()
            && let Some(c) = c.as_mut()
        {
            c.modules.set_enabled(id, on);
        }
    });
}

/// Whether the client has read its profiles (the first frame has run).
pub(super) fn ready() -> bool {
    CLIENT.with(|cell| cell.try_borrow().ok().is_some_and(|c| c.is_some()))
}

/// Whether module `id` is on.
pub(super) fn on(id: &str) -> bool {
    settings(id, |_| ()).is_some()
}

/// HUD element `element` of module `module`, where the player placed it: `draw` draws it at its
/// own origin and returns its size (None for nothing to show). Hot path: once a frame an element.
pub(super) fn element(
    game: &super::Game<'_>,
    batch: &mut HudBatch,
    module: &str,
    element: &str,
    draw: impl FnOnce(&mut incari_core::hud::HudCanvas) -> Option<[f32; 2]>,
) {
    let atlas = game.native_atlas();
    let (screen, scale) = ([batch.width, batch.height], batch.scale);
    let _ = with(game, |client, _| {
        if client.hud_element(module, element, &Font(atlas), scale, screen, draw) {
            if client.editing_hud() {
                // The menu blur covers what the HUD drew: the editor's elements go over it.
                let list = std::mem::take(&mut client.list);
                DEFERRED.with(|d| d.borrow_mut().push(list));
            } else {
                replay(&client.list, atlas, batch, Some(game));
            }
        }
    });
}

/// Whether Incari draws the game's screen `screen` itself.
pub(super) fn claims(screen: &str) -> bool {
    CLIENT.with(|cell| {
        cell.try_borrow()
            .ok()
            .and_then(|c| c.as_ref().map(|c| c.claims(screen)))
            .unwrap_or(screen == "title")
    })
}

/// The menus' photo under the title screen.
pub(super) fn backdrop(game: &super::Game<'_>, batch: &mut HudBatch) {
    let atlas = game.native_atlas();
    let (size, scale) = ([batch.width, batch.height], batch.scale);
    let _ = with(game, |client, _| {
        client.backdrop(&Font(atlas), size, scale);
        replay(&client.list, atlas, batch, None);
    });
}

/// A frame of Incari's screens over the game's.
pub(super) fn frame(game: &mut super::Game<'_>, batch: &mut HudBatch) {
    let wants_clipboard = CLIENT.with(|c| c.try_borrow().ok().and_then(|c| c.as_ref().map(Client::wants_clipboard)));
    let clipboard = if wants_clipboard == Some(true) { game.native_clipboard() } else { String::new() };
    let (size, scale) = ([batch.width, batch.height], batch.scale);
    let on_target = game.native_targeted_entity().is_some();
    // Incari's smooth font: shadows half a GUI pixel away, as SmoothFont's.
    game.native_atlas().font.set_smooth_offset(true);
    // AutoGuiScale: Auto is one step below the largest scale that fits, leaving a Chromebook's
    // small screen room for the game around the GUI.
    horizon_client::hud::GUI_AUTO_STEP_DOWN.store(1, std::sync::atomic::Ordering::Relaxed);
    let (uploads, requests) = {
        let view: &super::Game<'_> = game;
        let atlas = view.native_atlas();
        let (uploads, requests) = with(view, |client, shell| {
            shell.clipboard = clipboard;
            client.on_target = on_target;
            online_link::pump(shell);
            if OPEN_CHAT.with(Cell::get) && view.native_ui_screen() == "playing" {
                OPEN_CHAT.with(|c| c.set(false));
                client.open_in_world(shell, Box::new(incari_core::screens::chat_menu::ChatMenu::default()));
            }
            client.frame(shell, &Font(atlas), size, scale);
            settings_palette(&client.theme);
            replay(&client.list, atlas, batch, Some(view));
            for list in DEFERRED.with(|d| std::mem::take(&mut *d.borrow_mut())) {
                replay(&list, atlas, batch, Some(view));
            }
            // Incari Chat's button on the chat screen, and its toasts.
            if view.native_ui_screen() == "chat" {
                incari_core::online::game::chat_button_frame(client, &Font(atlas), size, scale);
                replay(&client.list, atlas, batch, Some(view));
            }
            incari_core::online::toast::frame(client, &Font(atlas), size, scale, shell.now_ms());
            replay(&client.list, atlas, batch, Some(view));
            client.take_uploads()
        });
        (uploads.unwrap_or_default(), requests)
    };
    GUI_SIZE.with(|g| g.set(size));
    for u in uploads {
        game.native_canvas_upload(u.at, [u.image.width, u.image.height], u.image.rgba);
    }
    run(game, requests);
    for (name, data) in incari_core::online::take_downloads() {
        game.native_download(&name, data);
    }
}

/// Input before the game hears it: true when Incari took it.
pub(super) fn input(game: &mut super::Game<'_>, input: &super::native_hooks::NativeInput<'_>) -> bool {
    use super::native_hooks::NativeInput as N;
    let event = match *input {
        N::Move { at } => Input::Move { at },
        N::Button { button, pressed, at, modifiers } => Input::Button { button, pressed, at, modifiers },
        N::Scroll { dx, dy, at } => Input::Scroll { dx, dy, at },
        N::Key { name, pressed, repeat, modifiers } => Input::Key {
            key: Key::from_name(name),
            name: name.to_owned(),
            pressed,
            repeat,
            modifiers,
        },
        N::Text { text } => Input::Text { text: text.to_owned() },
    };
    // A server's dialog takes the keys and clicks, as any game screen does in the Java build;
    // `native_ui_screen` still names the screen under it (`playing`), so ask.
    if game.native_dialog_open() && !matches!(input, N::Move { .. }) {
        return false;
    }
    // Incari Chat: its button on the chat screen, and the Friends key (O by default).
    let screen = game.native_ui_screen();
    if let N::Button { button: 0, pressed: true, at, .. } = *input
        && screen == "chat"
        && incari_core::online::game::chat_button_hit(&Font(game.native_atlas()), GUI_SIZE.with(Cell::get), at)
    {
        game.native_open("resume");
        OPEN_CHAT.with(|c| c.set(true));
        return true;
    }
    if let N::Key { name, pressed: true, repeat: false, .. } = *input
        && name == game.native_bound_key("key.friends")
        && matches!(screen, "playing" | "title" | "paused")
    {
        let (_, requests) = with(game, |client, shell| {
            client.open_in_world(shell, Box::new(incari_core::screens::chat_menu::ChatMenu::default()))
        });
        run(game, requests);
        return true;
    }
    let (taken, requests) = with(game, |client, shell| client.input(shell, &event));
    run(game, requests);
    taken.unwrap_or(false)
}

/// Once a tick.
pub(super) fn tick(game: &mut super::Game<'_>) {
    let (_, requests) = with(game, |client, shell| client.tick(shell));
    run(game, requests);
    // Who's here, for the online service: the world, the server and its tab list.
    let title = game.native_ui_screen() == "title";
    let world = game
        .native_world_id()
        .map(|id| id.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3)));
    let address = if game.native_external_server() { game.native_server_address() } else { None };
    let name = game.native_player_name();
    incari_core::online::with(|o| {
        TRACKER.with(|t| {
            t.borrow_mut().tick(o, title, world, address.as_deref().map(|a| (a, name.as_str())), || {
                game.native_tab_players()
            })
        })
    });
}

/// The atlas was made again: the canvas is blank.
pub(super) fn atlas_replaced() {
    CLIENT.with(|cell| {
        if let Ok(mut c) = cell.try_borrow_mut()
            && let Some(c) = c.as_mut()
        {
            c.atlas_replaced();
        }
    });
}

/// Incari's featured servers atop the game's server list, each with its Sponsored label.
fn install_featured() {
    let servers = incari_core::featured::parse(&build().featured)
        .into_iter()
        .map(|e| (e.name, e.address))
        .collect();
    let label = |batch: &mut HudBatch, atlas: &HudAtlas, _index: usize, x: f32, y: f32| {
        CLIENT.with(|cell| {
            let Ok(mut slot) = cell.try_borrow_mut() else { return };
            let Some(c) = slot.as_mut() else { return };
            c.overlay(&Font(atlas), batch.scale, [batch.width, batch.height], |p, _| {
                incari_core::featured::draw_label(p, x, y);
            });
            replay(&c.list, atlas, batch, None);
        });
    };
    horizon_client::multiplayer::set_featured(servers, Some(std::rc::Rc::new(label)));
}

/// The theme's colours for the text of Horizon's settings screens (Graphics and the like), whose
/// shapes the skin draws (`horizon/settings/` names). Hot path: a copy, no allocation.
fn settings_palette(theme: &incari_core::theme::Theme) {
    let p = incari_core::settings_skin::palette(theme);
    horizon_client::graphics_style::set_palette(Some(horizon_client::graphics_style::Palette {
        text: p.text,
        soft: p.soft,
        muted: p.muted,
        accent: p.accent,
        note: p.note,
        positive: p.positive,
        warning: p.warning,
        negative: p.negative,
    }));
}

/// Frost over the game's own widgets, by sprite name (`hud::set_skin`).
fn install_skin() {
    horizon_client::hud::set_skin(Some(std::rc::Rc::new(
        |batch: &mut HudBatch, atlas: &HudAtlas, name: &str, rect: [f32; 4], color: u32| {
            CLIENT.with(|cell| {
                let Ok(mut slot) = cell.try_borrow_mut() else {
                    return false;
                };
                let Some(client) = slot.as_mut() else {
                    return false;
                };
                if !client.skin(&Font(atlas), batch.scale, name, rect, color) {
                    return false;
                }
                replay(&client.list, atlas, batch, None);
                true
            })
        },
    )));
}

/// Draws `list` into `batch`. Items need the game; without it they're left out.
fn replay(list: &DrawList, atlas: &HudAtlas, batch: &mut HudBatch, game: Option<&super::Game<'_>>) {
    let canvas = atlas.sprites.get(MOD_CANVAS_SPRITE).copied();
    let mut clips: Vec<(usize, [f32; 4])> = Vec::new();
    let tint = |argb: u32| {
        let [a, r, g, b] = argb.to_be_bytes();
        rgba(r, g, b, a)
    };
    for cmd in &list.cmds {
        match cmd {
            Cmd::Fill { rect, argb } => batch.fill(atlas, rect[0], rect[1], rect[2], rect[3], tint(*argb)),
            Cmd::Image { src, dst, argb } => {
                let Some(mut r) = canvas else { continue };
                let (du, dv) = ((r.u1 - r.u0) / r.w as f32, (r.v1 - r.v0) / r.h as f32);
                r.u0 += src[0] * du;
                r.v0 += src[1] * dv;
                r.u1 = r.u0 + src[2] * du;
                r.v1 = r.v0 + src[3] * dv;
                batch.rect_uv(dst[0], dst[1], dst[2], dst[3], r, tint(*argb));
            }
            Cmd::Quad { src, corners, argb } => {
                let Some(r) = canvas else { continue };
                let (du, dv) = ((r.u1 - r.u0) / r.w as f32, (r.v1 - r.v0) / r.h as f32);
                let (u0, v0) = (r.u0 + src[0] * du, r.v0 + src[1] * dv);
                let (u1, v1) = (u0 + src[2] * du, v0 + src[3] * dv);
                batch.quad(*corners, [[u0, v0], [u1, v0], [u1, v1], [u0, v1]], tint(*argb));
            }
            Cmd::Sprite { name, src, dst, argb } => {
                let Some(mut r) = atlas.sprites.get(list.str(name)).copied() else {
                    continue;
                };
                // No size: the whole sprite, as big as its pack made it.
                let src: incari_core::draw::Rect = if src[2] <= 0.0 || src[3] <= 0.0 { [0.0, 0.0, r.w as f32, r.h as f32] } else { *src };
                let (du, dv) = ((r.u1 - r.u0) / r.w as f32, (r.v1 - r.v0) / r.h as f32);
                r.u0 += src[0] * du;
                r.v0 += src[1] * dv;
                r.u1 = r.u0 + src[2] * du;
                r.v1 = r.v0 + src[3] * dv;
                batch.rect_uv(dst[0], dst[1], dst[2], dst[3], r, tint(*argb));
            }
            Cmd::Text { text, at, argb, shadow, scale } => {
                let start = batch.vertices.len();
                batch.text_styled(atlas, &Text::literal(list.str(text)), at[0], at[1], tint(*argb), *shadow);
                if *scale != 1.0 {
                    batch.scale_since(start, at[0], at[1], *scale, *scale);
                }
            }
            Cmd::Item { id, at, size } => {
                if let Some(game) = game {
                    game.native_item(batch, list.str(id), *at, *size);
                }
            }
            Cmd::ClipStart { rect } => clips.push((batch.vertices.len(), *rect)),
            Cmd::ClipEnd => {
                if let Some((start, r)) = clips.pop() {
                    batch.clip_since(start, r[0], r[1], r[0] + r[2], r[1] + r[3]);
                }
            }
        }
    }
}
