//! Incari's screens: a stack of them over the game, each drawn every frame and answering the
//! input kept in `Ui`. A screen asks for what's next (another screen, back, one of the game's own
//! screens) through `Nav`, done after it has drawn.
use crate::framework::system::ModuleSystem;
use crate::paint::Painter;
use crate::theme::Preset;
use crate::ui::Ui;

/// What Incari needs of the game, given by the game side each call.
pub trait Host {
    /// Milliseconds since some start, steadily rising.
    fn now_ms(&self) -> f64;
    /// The game's screen showing (Horizon's `native_ui_screen`: `title`, `paused`, `playing`...).
    fn screen(&self) -> &str;
    /// Opens one of the game's screens or does one of its menu actions (`native_open`).
    fn open(&mut self, what: &str) -> bool;
    /// Joins a server from the title screen.
    fn connect(&mut self, address: &str) -> bool;
    /// In a world: takes the pointer from the game for Incari's screens, or gives it back.
    fn hold_world(&mut self, held: bool);
    /// The game's text for `key`, in its language.
    fn translate(&self, key: &str) -> String;
    /// The player's name.
    fn player_name(&self) -> String;
    /// The game's version ("26.3").
    fn game_version(&self) -> String;
    /// Whether the world played is a server's.
    fn on_server(&self) -> bool;
    /// The key the game's Friends mapping is bound to (O, HID usage 18, by default).
    fn friends_key(&self) -> crate::input::Key {
        crate::input::Key::Keyboard(18)
    }
    /// A file of Incari's under the game folder's `incari/`.
    fn read(&self, path: &str) -> Option<Vec<u8>>;
    fn write(&mut self, path: &str, data: &[u8]);
    fn delete(&mut self, path: &str);
    /// The player's skin (64 by 64 RGBA) and whether its arms are slim.
    fn own_skin(&self) -> Option<(Vec<u8>, bool)>;
    /// Files under the game folder beyond `incari/`, downloads, the image clipboard and links.
    fn extras(&self) -> Box<dyn crate::host_extras::HostExtras>;
    /// What the build was made from (`incari.properties`).
    fn build_info(&self) -> crate::release::BuildInfo;
    /// What the online service said about releases; None without one.
    fn releases(&self) -> Option<&dyn crate::release::ReleaseSource> {
        None
    }
    /// The page's screen recorder; none where there is none.
    fn recorder(&self) -> Box<dyn crate::recorder::Recorder> {
        Box::new(crate::recorder::NoRecorder)
    }
    /// The local time (year, month, day, hour, minute, second), for a recording's name.
    fn local_time(&self) -> fn() -> [u32; 6] {
        || [1970, 1, 1, 0, 0, 0]
    }
    /// The clipboard's text.
    fn clipboard(&self) -> String;
    fn set_clipboard(&mut self, text: &str);
}

/// What a screen asks for next.
pub enum Go {
    /// Another screen over this one.
    Open(Box<dyn Screen>),
    /// This screen closes; the one under it shows.
    Back,
    /// This screen is replaced.
    Replace(Box<dyn Screen>),
    /// Every Incari screen closes.
    CloseAll,
    /// One of the game's screens or actions (`Host::open`), after closing Incari's.
    Game(&'static str),
    /// One of the game's screens or actions, Incari's screens staying under it.
    GameOver(&'static str),
    /// Joins a server.
    Connect(String),
}

/// What a screen draws with and acts on.
pub struct Cx<'a, 'p> {
    pub p: &'a mut Painter<'p>,
    pub ui: &'a mut Ui,
    pub modules: &'a mut ModuleSystem,
    pub host: &'a mut dyn Host,
    pub prefs: &'a mut Prefs,
    /// The window in GUI units.
    pub size: [f32; 2],
    /// Whether a world is open (the menu's backdrop is the world, not the photo).
    pub in_world: bool,
    pub go: Option<Go>,
}

impl Cx<'_, '_> {
    pub fn go(&mut self, go: Go) {
        self.go = Some(go);
    }

    pub fn open(&mut self, screen: impl Screen + 'static) {
        self.go = Some(Go::Open(Box::new(screen)));
    }
}

/// The client's own settings beside the modules (Look, ClientSettings).
#[derive(Clone, Debug, PartialEq)]
pub struct Prefs {
    pub preset: Preset,
    pub copy_screenshots: bool,
    pub show_badges: bool,
    pub usage_counts: bool,
    /// Changed since read: saved by the client.
    pub dirty: bool,
    /// `settings.txt`'s lines this doesn't know (the recorder's), written back as they were.
    pub rest: String,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            preset: Preset::Potato,
            copy_screenshots: false,
            show_badges: true,
            usage_counts: true,
            dirty: false,
            rest: String::new(),
        }
    }
}

impl Prefs {
    /// `look.txt` (`preset=normal`) and `settings.txt` (`key=value` lines), as the Java client
    /// keeps them.
    pub fn read(look: Option<&str>, settings: Option<&str>) -> Self {
        let mut prefs = Self::default();
        let lines = |text: Option<&str>| -> Vec<(String, String)> {
            text.unwrap_or("")
                .lines()
                .filter_map(|l| l.split_once('='))
                .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
                .collect()
        };
        for (k, v) in lines(look) {
            if k == "preset"
                && let Some(p) = Preset::parse(&v)
            {
                prefs.preset = p;
            }
        }
        for (k, v) in lines(settings) {
            let on = v == "true";
            match k.as_str() {
                "copy-screenshots" => prefs.copy_screenshots = on,
                "show-badges" => prefs.show_badges = on,
                "usage-counts" => prefs.usage_counts = on,
                _ => {
                    prefs.rest.push_str(&format!("{k}={v}\n"));
                }
            }
        }
        prefs
    }

    pub fn look_text(&self) -> String {
        format!("preset={}\n", self.preset.name())
    }

    pub fn settings_text(&self) -> String {
        format!(
            "copy-screenshots={}\nshow-badges={}\nusage-counts={}\n{}",
            self.copy_screenshots, self.show_badges, self.usage_counts, self.rest
        )
    }
}

/// A screen of Incari's.
pub trait Screen {
    /// Draws the screen and answers the frame's input.
    fn draw(&mut self, cx: &mut Cx);
    /// Escape: back, unless the screen takes it.
    fn escape(&mut self, cx: &mut Cx) {
        cx.go(Go::Back);
    }
    /// Whether the look's backdrop (the photo or the world under the tint) is drawn behind it.
    fn backdrop(&self) -> bool {
        true
    }
    /// Whether the game's pause menu or world shows behind it unblurred (the HUD editor).
    fn see_through(&self) -> bool {
        false
    }
    /// Called when it closes.
    fn closed(&mut self, _cx: &mut Cx) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_java_clients_look_and_settings() {
        let prefs = Prefs::read(
            Some("preset=normal\n"),
            Some("copy-screenshots=true\nshow-badges=false\n"),
        );
        assert_eq!(prefs.preset, Preset::Normal);
        assert!(prefs.copy_screenshots && !prefs.show_badges && prefs.usage_counts);
        assert_eq!(
            Prefs::read(Some(&prefs.look_text()), Some(&prefs.settings_text())),
            prefs
        );
    }
}
