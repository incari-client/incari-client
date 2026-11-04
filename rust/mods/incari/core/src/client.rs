//! The client: everything Incari keeps between frames, and what the game side calls each frame.
//! The game side draws the list a call leaves in `Client::list`, uploads the canvas's new pixels
//! and does what the call asked of the game through `Host`.
use crate::canvas::{Canvas, Upload};
use crate::draw::DrawList;
use crate::framework::system::{ModuleSystem, Storage};
use crate::input::{Input, Key, keys};
use crate::motion::Motion;
use crate::paint::{Assets, Measure, Painter};
use crate::screen::{Cx, Go, Host, Prefs, Screen};
use crate::skin::Skin;
use crate::theme::{Palette, Theme};
use crate::ui::Ui;

/// The module menu's key (IncariKeys): Right Shift.
pub const MENU_KEY: Key = Key::Keyboard(keys::RIGHT_SHIFT);

/// What the game showed when the client last looked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Root {
    /// The title screen's main page: Incari's title.
    Title,
    /// The pause menu: Incari's.
    Pause,
    /// Incari's own screen in a world (Horizon's mod screen).
    World,
    /// Anything else: the game's own screen, or playing.
    Game,
}

pub struct Client {
    pub canvas: Canvas,
    pub motion: Motion,
    pub assets: Assets,
    pub ui: Ui,
    pub list: DrawList,
    pub modules: ModuleSystem,
    pub prefs: Prefs,
    pub theme: Theme,
    skin: Skin,
    /// Incari's screens over the root, last on top.
    stack: Vec<Box<dyn Screen>>,
    /// The screen at the root, when the game shows one Incari draws (title, pause).
    root: Option<(Root, Box<dyn Screen>)>,
    /// Screen pixels per GUI unit at the last frame.
    scale: f32,
    /// Whether the world is held for Incari's screens (Horizon's mod screen open).
    holding_world: bool,
    /// Whether a world was open at the last frame.
    in_world: bool,
    /// The modules' revision the theme was made at.
    theme_revision: u64,
    /// The crosshair is on a mob or player in reach (Crosshair Editor's colour on target).
    pub on_target: bool,
}

/// Incari's files under the game folder's `incari/`, through the host.
pub struct HostFiles<'a>(pub &'a mut dyn Host);

impl Storage for HostFiles<'_> {
    fn read(&self, path: &str) -> Option<String> {
        self.0
            .read(&format!("modules/{path}"))
            .and_then(|b| String::from_utf8(b).ok())
    }
    fn write(&mut self, path: &str, text: &str) {
        self.0.write(&format!("modules/{path}"), text.as_bytes());
    }
    fn delete(&mut self, path: &str) {
        self.0.delete(&format!("modules/{path}"));
    }
}

impl Client {
    pub fn new(assets: Assets) -> Self {
        Self {
            canvas: Canvas::default(),
            motion: Motion::default(),
            assets,
            ui: Ui::default(),
            list: DrawList::default(),
            modules: ModuleSystem::new(crate::modules::all()),
            prefs: Prefs::default(),
            theme: Theme::default(),
            skin: Skin::default(),
            stack: Vec::new(),
            root: None,
            scale: 0.0,
            holding_world: false,
            in_world: false,
            theme_revision: 0,
            on_target: false,
        }
    }

    /// Whether a text field is focused, so the game side should read the clipboard for it.
    pub fn wants_clipboard(&self) -> bool {
        self.ui.focus.is_some()
    }

    /// Reads the profiles and the client's settings: once, at start.
    pub fn load(&mut self, host: &mut dyn Host) {
        let text =
            |host: &dyn Host, path: &str| host.read(path).and_then(|b| String::from_utf8(b).ok());
        self.prefs = Prefs::read(
            text(host, "look.txt").as_deref(),
            text(host, "settings.txt").as_deref(),
        );
        self.modules.load(&HostFiles(host));
        self.refresh_skin(host);
        self.refresh_theme();
    }

    /// Reads the player's skin again (the title shows, a profile was edited).
    pub fn refresh_skin(&mut self, host: &dyn Host) {
        self.assets.skin = host.own_skin().map(|(rgba, slim)| {
            (
                crate::canvas::Image {
                    width: 64,
                    height: 64,
                    rgba,
                },
                slim,
            )
        });
        self.canvas.forget(crate::canvas::Key::Skin);
    }

    /// The theme from the Theme module's settings and the GUI preset.
    pub fn refresh_theme(&mut self) {
        let palette = self
            .modules
            .module("theme")
            .map_or(crate::theme::FROST_PALETTE, |m| {
                let s = &m.settings;
                if s.get("accent").is_none() {
                    return crate::theme::FROST_PALETTE;
                }
                Palette {
                    accent: s.color("accent") & 0xFF_FFFF,
                    text: s.color("text") & 0xFF_FFFF,
                    muted: s.color("muted") & 0xFF_FFFF,
                    panel: s.color("panel") & 0xFF_FFFF,
                    panel_opacity: s.int("panel-opacity"),
                    hud_opacity: s.int("hud-opacity"),
                    radius: s.int("radius"),
                }
            });
        self.theme = Theme::from_palette(palette, self.prefs.preset);
        self.motion.on = self.theme.normal();
    }

    /// The root the game's screen calls for.
    fn root_for(screen: &str, holding: bool) -> Root {
        match screen {
            "title" => Root::Title,
            "paused" => Root::Pause,
            "mod" if holding => Root::World,
            _ => Root::Game,
        }
    }

    /// Whether Incari draws the game's screen `screen` itself (`native_screen_claim`).
    pub fn claims(&self, screen: &str) -> bool {
        match screen {
            "title" | "paused" => true,
            "mod" => self.holding_world,
            _ => false,
        }
    }

    /// Whether anything of Incari's is showing, so input is Incari's.
    pub fn showing(&self, screen: &str) -> bool {
        self.claims(screen)
            || !self.stack.is_empty() && Self::root_for(screen, self.holding_world) != Root::Game
    }

    /// A frame: Incari's screens over the game's (`native_overlay`). Leaves what to draw in
    /// `self.list`.
    pub fn frame(
        &mut self,
        host: &mut dyn Host,
        measure: &dyn Measure,
        size: [f32; 2],
        scale: f32,
    ) {
        self.list.clear();
        self.begin(host, scale);
        let screen = host.screen().to_owned();
        let root = Self::root_for(&screen, self.holding_world);
        self.in_world = !screen.starts_with("title") && screen != "options" && screen != "loading";
        // The root follows the game: a world opened, the pause menu closed.
        let current = self.root.as_ref().map(|r| r.0);
        if current != Some(root) {
            self.root = match root {
                Root::Title => {
                    self.refresh_skin(host);
                    Some((
                        Root::Title,
                        Box::new(crate::screens::title::Title::new(host.now_ms())),
                    ))
                }
                Root::Pause => Some((
                    Root::Pause,
                    Box::new(crate::screens::pause::Pause::default()),
                )),
                Root::World | Root::Game => None,
            };
            if root == Root::Game {
                // The game's own screen took over (a world loaded, the menu closed elsewhere).
                self.stack.clear();
                self.ui.reset();
            }
        }
        if root == Root::Game {
            self.draw_recording(host, measure, scale);
            self.ui.end_frame();
            return;
        }
        let top_is_stack = !self.stack.is_empty();
        let mut screen_box = if top_is_stack {
            self.stack.pop()
        } else {
            self.root.take().map(|r| r.1)
        };
        let Some(screen) = screen_box.as_mut() else {
            self.ui.end_frame();
            return;
        };
        let go = {
            let mut p = Painter {
                list: &mut self.list,
                canvas: &mut self.canvas,
                assets: &self.assets,
                motion: &mut self.motion,
                measure,
                theme: self.theme,
                scale,
                fade: 1.0,
                origin: [0.0, 0.0],
                zoom: 1.0,
            };
            let in_world = self.in_world;
            // The photo itself is under everything already on the title (`backdrop`).
            if screen.backdrop() && !(root == Root::Title && !top_is_stack) {
                crate::panels::backdrop(&mut p, size[0], size[1], false);
            }
            let mut cx = Cx {
                p: &mut p,
                ui: &mut self.ui,
                modules: &mut self.modules,
                host,
                prefs: &mut self.prefs,
                size,
                in_world,
                go: None,
            };
            if cx.ui.key(keys::ESCAPE) {
                screen.escape(&mut cx);
            }
            screen.draw(&mut cx);
            cx.ui.draw_tooltip(cx.p, size);
            cx.go.take()
        };
        // Put it back where it came from, then do what it asked.
        let screen_box = screen_box.take().expect("drawn");
        if top_is_stack {
            self.stack.push(screen_box);
        } else {
            self.root = Some((root, screen_box));
        }
        if let Some(go) = go {
            self.navigate(go, host, measure, size, scale);
        }
        // Action settings pressed in a module's page: what each opens.
        for (module, setting) in std::mem::take(&mut self.modules.actions) {
            let screen = crate::screens::waypoint_list::for_action(module, setting)
                .or_else(|| crate::screens::file_chooser::for_action(module, setting));
            if let Some(screen) = screen {
                self.ui.reset();
                self.hold(host, true);
                self.stack.push(screen);
            } else {
                crate::schematic::session::button(module, setting);
            }
        }
        self.draw_recording(host, measure, scale);
        self.ui.end_frame();
        self.after(host);
    }

    /// While recording: REC and the time, over everything.
    fn draw_recording(&mut self, host: &dyn Host, measure: &dyn Measure, scale: f32) {
        let recorder = host.recorder();
        if recorder.state() != crate::recorder::State::Recording {
            return;
        }
        let mut p = Painter {
            list: &mut self.list,
            canvas: &mut self.canvas,
            assets: &self.assets,
            motion: &mut self.motion,
            measure,
            theme: self.theme,
            scale,
            fade: 1.0,
            origin: [0.0, 0.0],
            zoom: 1.0,
        };
        crate::recorder::draw_indicator(&mut p, recorder.as_ref());
    }

    /// The frame's start: the clock, a new GUI scale, the clipboard for a focused field.
    fn begin(&mut self, host: &dyn Host, scale: f32) {
        self.motion.now = host.now_ms();
        if self.modules.revision != self.theme_revision {
            // The Theme module's colours may have changed.
            self.theme_revision = self.modules.revision;
            self.refresh_theme();
        }
        if scale != self.scale {
            // Every shape is painted for the new scale's pixels.
            self.scale = scale;
            self.canvas.reset();
        }
        if self.ui.focus.is_some() && self.ui.clipboard.is_none() {
            self.ui.clipboard = Some(host.clipboard());
        }
    }

    /// After a frame or input: copies, saved settings.
    fn after(&mut self, host: &mut dyn Host) {
        if let Some(text) = self.ui.copy.take() {
            host.set_clipboard(&text);
        }
        if self.ui.focus.is_none() {
            self.ui.clipboard = None;
        }
        if self.prefs.dirty {
            self.prefs.dirty = false;
            host.write("look.txt", self.prefs.look_text().as_bytes());
            host.write("settings.txt", self.prefs.settings_text().as_bytes());
            self.refresh_theme();
        }
    }

    fn navigate(
        &mut self,
        go: Go,
        host: &mut dyn Host,
        measure: &dyn Measure,
        size: [f32; 2],
        scale: f32,
    ) {
        self.ui.reset();
        match go {
            Go::Open(screen) => {
                self.hold(host, true);
                self.stack.push(screen);
            }
            Go::Replace(screen) => {
                self.close_top(host, measure, size, scale);
                self.stack.push(screen);
            }
            Go::Back => {
                self.close_top(host, measure, size, scale);
                if self.stack.is_empty() {
                    self.hold(host, false);
                }
            }
            Go::CloseAll => {
                while !self.stack.is_empty() {
                    self.close_top(host, measure, size, scale);
                }
                self.hold(host, false);
            }
            Go::Game(what) => {
                while !self.stack.is_empty() {
                    self.close_top(host, measure, size, scale);
                }
                self.hold(host, false);
                host.open(what);
            }
            Go::GameOver(what) => {
                host.open(what);
            }
            Go::Connect(address) => {
                self.stack.clear();
                host.connect(&address);
            }
        }
        self.modules.flush(&mut HostFiles(host));
    }

    /// Closes the top screen (the root's, when there's none of Incari's over it).
    fn close_top(
        &mut self,
        host: &mut dyn Host,
        measure: &dyn Measure,
        size: [f32; 2],
        scale: f32,
    ) {
        let Some(mut screen) = self.stack.pop() else {
            // Back from the root: the pause menu resumes; the title stays.
            if self.root.as_ref().is_some_and(|r| r.0 == Root::Pause) {
                host.open("resume");
            }
            return;
        };
        let mut p = Painter {
            list: &mut self.list,
            canvas: &mut self.canvas,
            assets: &self.assets,
            motion: &mut self.motion,
            measure,
            theme: self.theme,
            scale,
            fade: 1.0,
            origin: [0.0, 0.0],
            zoom: 1.0,
        };
        let mut cx = Cx {
            p: &mut p,
            ui: &mut self.ui,
            modules: &mut self.modules,
            host,
            prefs: &mut self.prefs,
            size,
            in_world: self.in_world,
            go: None,
        };
        screen.closed(&mut cx);
    }

    /// In a world with no game screen of Incari's root under it (playing), Incari's screens take
    /// the pointer through Horizon's mod screen.
    fn hold(&mut self, host: &mut dyn Host, held: bool) {
        let in_world_playing = matches!(host.screen(), "playing" | "mod");
        if held && !self.holding_world && in_world_playing {
            self.holding_world = true;
            host.hold_world(true);
        } else if !held && self.holding_world {
            self.holding_world = false;
            host.hold_world(false);
        }
    }

    /// Input before the game hears it (`native_overlay_input`): true when Incari took it.
    pub fn input(&mut self, host: &mut dyn Host, event: &Input) -> bool {
        let screen = host.screen().to_owned();
        if self.holding_world && screen != "mod" {
            // Escape or another screen closed Horizon's mod screen under Incari's.
            self.holding_world = false;
            self.stack.clear();
        }
        if self.showing(&screen) {
            // The game keeps its fullscreen and screenshot keys.
            if let Input::Key {
                key: Key::Keyboard(code),
                ..
            } = event
                && matches!(*code, keys::F11 | keys::F2)
            {
                return false;
            }
            self.ui.input(event);
            // Horizon sends a key's text only when nobody took the key, so a key that types
            // into the focused field goes on (its text then comes as `Input::Text`).
            if self.ui.focus.is_some()
                && let Input::Key {
                    name,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                && modifiers & (crate::input::CONTROL | crate::input::SUPER) == 0
                && types_text(name)
            {
                return false;
            }
            return !matches!(
                event,
                Input::Move { .. } | Input::Key { pressed: false, .. }
            );
        }
        // Playing, or a screen of the game's: only Incari's keys.
        if let Input::Key {
            key: Key::Keyboard(crate::recorder::KEY),
            pressed: true,
            repeat: false,
            ..
        } = event
            && self.ui.focus.is_none()
        {
            let text = host
                .read("settings.txt")
                .and_then(|b| String::from_utf8(b).ok())
                .unwrap_or_default();
            let settings = crate::recorder::Settings::read(&text);
            let local = (host.local_time())();
            crate::recorder::toggle(host.recorder().as_mut(), &settings, local);
            return true;
        }
        if let Input::Key {
            key,
            pressed: true,
            repeat: false,
            ..
        } = event
        {
            if *key == MENU_KEY && screen == "playing" {
                self.open_in_world(
                    host,
                    Box::new(crate::screens::modules::ModuleList::default()),
                );
                return true;
            }
            if screen == "playing" && self.modules.key_pressed(*key, &mut HostFiles(host)) {
                return false;
            }
        }
        if let Input::Move { at } = event {
            self.ui.mouse = *at;
        }
        false
    }

    /// Opens an Incari screen over the world being played.
    pub fn open_in_world(&mut self, host: &mut dyn Host, screen: Box<dyn Screen>) {
        self.ui.reset();
        self.hold(host, true);
        self.stack.push(screen);
    }

    /// Once a game tick: saving settled changes.
    pub fn tick(&mut self, host: &mut dyn Host) {
        self.modules.tick(&mut HostFiles(host));
    }

    /// A sprite the game draws by name, in Frost: true when the skin drew it into `self.list`.
    pub fn skin(
        &mut self,
        measure: &dyn Measure,
        scale: f32,
        name: &str,
        rect: [f32; 4],
        color: u32,
    ) -> bool {
        self.list.clear();
        if scale != self.scale {
            self.scale = scale;
            self.canvas.reset();
        }
        let mut p = Painter {
            list: &mut self.list,
            canvas: &mut self.canvas,
            assets: &self.assets,
            motion: &mut self.motion,
            measure,
            theme: self.theme,
            scale,
            fade: 1.0,
            origin: [0.0, 0.0],
            zoom: 1.0,
        };
        if name == "gui/sprites/hud/crosshair"
            && let Some(m) = self
                .modules
                .modules
                .iter()
                .find(|m| m.id == "crosshair" && m.enabled)
        {
            crate::crosshair::draw(
                &mut p,
                &m.settings,
                self.on_target,
                rect[0] + rect[2] / 2.0,
                rect[1] + rect[3] / 2.0,
            );
            return true;
        }
        self.skin.sprite(&mut p, name, rect, color)
    }

    /// The backdrop behind the title screen and the screens over it (`native_backdrop`).
    pub fn backdrop(&mut self, measure: &dyn Measure, size: [f32; 2], scale: f32) {
        self.list.clear();
        if scale != self.scale {
            self.scale = scale;
            self.canvas.reset();
        }
        let mut p = Painter {
            list: &mut self.list,
            canvas: &mut self.canvas,
            assets: &self.assets,
            motion: &mut self.motion,
            measure,
            theme: self.theme,
            scale,
            fade: 1.0,
            origin: [0.0, 0.0],
            zoom: 1.0,
        };
        p.backdrop([0.0, 0.0, size[0], size[1]], 0xFFFF_FFFF);
    }

    /// Whether module `id` is on.
    pub fn on(&self, id: &str) -> bool {
        self.modules.on(id)
    }

    /// Module `id`'s settings, while it's on.
    pub fn settings(&self, id: &str) -> Option<&crate::framework::setting::Settings> {
        self.modules
            .module(id)
            .filter(|m| m.enabled)
            .map(|m| &m.settings)
    }

    /// Whether the HUD editor is open (elements with nothing to say show a sample).
    pub fn editing_hud(&self) -> bool {
        self.stack.last().is_some_and(|s| s.see_through())
    }

    /// HUD element `element` of module `module`, drawn by `draw` at its own origin (it returns
    /// its size at scale 1, or None to draw nothing), then put where the player placed it.
    /// Leaves what to draw in `self.list`; false when the module is off.
    pub fn hud_element(
        &mut self,
        module: &str,
        element: &str,
        measure: &dyn Measure,
        scale: f32,
        screen: [f32; 2],
        draw: impl FnOnce(&mut crate::hud::HudCanvas) -> Option<[f32; 2]>,
    ) -> bool {
        self.list.clear();
        if scale != self.scale {
            self.scale = scale;
            self.canvas.reset();
        }
        let editing = self.editing_hud();
        let Some(m) = self
            .modules
            .modules
            .iter()
            .find(|m| m.id == module && m.enabled)
        else {
            return false;
        };
        let Some(e) = m.hud.iter().find(|e| e.id == element) else {
            return false;
        };
        let (mid, eid) = (m.id, e.id);
        let zoom = e.scale();
        let mut p = Painter {
            list: &mut self.list,
            canvas: &mut self.canvas,
            assets: &self.assets,
            motion: &mut self.motion,
            measure,
            theme: self.theme,
            scale,
            fade: 1.0,
            origin: [0.0, 0.0],
            zoom,
        };
        let mut canvas = crate::hud::HudCanvas {
            p: &mut p,
            element: &e.settings,
            module: &m.settings,
            editing,
            screen,
        };
        let Some(size) = draw(&mut canvas) else {
            self.list.clear();
            return false;
        };
        let [x, y, _, _] = e.bounds(size, screen);
        self.list.translate_since(0, x, y);
        self.modules.sizes.insert((mid, eid), size);
        true
    }

    /// Draws `draw` over the HUD at the screen's origin, in GUI units (the Waypoints mod's
    /// markers). Leaves what to draw in `self.list`.
    pub fn overlay(
        &mut self,
        measure: &dyn Measure,
        scale: f32,
        screen: [f32; 2],
        draw: impl FnOnce(&mut Painter<'_>, [f32; 2]),
    ) {
        self.list.clear();
        if scale != self.scale {
            self.scale = scale;
            self.canvas.reset();
        }
        let mut p = Painter {
            list: &mut self.list,
            canvas: &mut self.canvas,
            assets: &self.assets,
            motion: &mut self.motion,
            measure,
            theme: self.theme,
            scale,
            fade: 1.0,
            origin: [0.0, 0.0],
            zoom: 1.0,
        };
        draw(&mut p, screen);
    }

    /// The canvas's new pixels, for the game side to upload.
    pub fn take_uploads(&mut self) -> Vec<Upload> {
        self.canvas.take_uploads()
    }

    /// A new atlas: the canvas is blank, so everything is painted again as it's next drawn.
    pub fn atlas_replaced(&mut self) {
        self.canvas.reset();
    }
}

/// Whether the key named `name` (`key.keyboard.a`) can type a character.
fn types_text(name: &str) -> bool {
    let Some(key) = name.strip_prefix("key.keyboard.") else {
        return false;
    };
    let keypad = key.strip_prefix("keypad.").unwrap_or(key);
    keypad.chars().count() == 1
        || matches!(
            keypad,
            "space"
                | "apostrophe"
                | "comma"
                | "minus"
                | "period"
                | "slash"
                | "semicolon"
                | "equal"
                | "left.bracket"
                | "right.bracket"
                | "backslash"
                | "grave.accent"
                | "world.1"
                | "world.2"
                | "add"
                | "subtract"
                | "multiply"
                | "divide"
                | "decimal"
        )
}
