//! A module's file chooser (FileChooserScreen.java, `Game.chooseFile`): the files in one folder of
//! the game folder, newest first, each with a button to load it. On the desktop the folder opens
//! beside the game; in the browser "Add files" opens the file picker instead, and the files picked
//! are copied into the folder. Schematics' Choose... opens it (`for_action`).
use super::list::List;
use crate::host_extras::{FileEntry, HostExtras};
use crate::screen::{Cx, Go, Screen};
use crate::theme::SMALL_BUTTON_WIDTH;
use crate::ui;
use std::cell::Cell;

/// A file bigger than this isn't loaded: it would only be refused after a long read.
const MAX_SIZE: usize = 64 << 20;
/// Three side by side fit the narrowest screen, 320 GUI units.
const FOOTER_BUTTON: f32 = 90.0;

/// What the chooser needs beyond `HostExtras`, given by the game side once (`install`): the
/// browser's file picker, and writing a picked file into the folder.
#[derive(Clone, Copy)]
pub struct Platform {
    /// Opens the picker for files of these types (`.litematic,.schem,.nbt`); in the browser only
    /// during a click.
    pub pick: fn(accept: &str),
    /// A file picked since the last call: its name and bytes.
    pub take_picked: fn() -> Option<(String, Vec<u8>)>,
    /// Writes `data` to `path` under the game folder, making its folder; false if it couldn't.
    pub write: fn(path: &str, data: &[u8]) -> bool,
    /// Makes folder `path` under the game folder if it isn't there.
    pub make_folder: fn(path: &str),
}

thread_local! {
    static PLATFORM: Cell<Option<Platform>> = const { Cell::new(None) };
}

/// The game side's picker and writer, once.
pub fn install(platform: Platform) {
    PLATFORM.with(|p| p.set(Some(platform)));
}

fn platform() -> Option<Platform> {
    PLATFORM.with(Cell::get)
}

/// The screen an action button opens: Schematics' Choose....
pub fn for_action(module: &str, setting: &str) -> Option<Box<dyn Screen>> {
    (module == crate::schematic::ID && setting == "choose").then(|| {
        Box::new(FileChooser::new(
            "Choose a schematic",
            crate::schematic::FOLDER,
            &crate::schematic::formats::EXTENSIONS,
            crate::schematic::session::choose,
        )) as Box<dyn Screen>
    })
}

pub struct FileChooser {
    list: List,
    title: String,
    folder: String,
    extensions: Vec<String>,
    chosen: fn(&str, Vec<u8>),
    files: Vec<FileEntry>,
    /// None until the first draw hands it over (`Host::extras`).
    extras: Option<Box<dyn HostExtras>>,
    /// Said in place of the default caption, until things are fine again.
    caption: Option<String>,
}

impl FileChooser {
    pub fn new(title: &str, folder: &str, extensions: &[&str], chosen: fn(&str, Vec<u8>)) -> Self {
        Self {
            list: List::default(),
            title: title.to_owned(),
            folder: folder.to_owned(),
            extensions: extensions.iter().map(|e| e.to_ascii_lowercase()).collect(),
            chosen,
            files: Vec::new(),
            extras: None,
            caption: None,
        }
    }

    /// Over `extras` now (the tests).
    pub fn with_extras(mut self, extras: Box<dyn HostExtras>) -> Self {
        self.extras = Some(extras);
        self.list_files();
        self
    }

    fn accepted(&self, name: &str) -> bool {
        let lower = name.to_ascii_lowercase();
        self.extensions.iter().any(|e| lower.ends_with(e.as_str()))
    }

    /// Reads the folder's files again: newest first, then by name.
    fn list_files(&mut self) {
        let Some(extras) = &self.extras else {
            return;
        };
        let mut files: Vec<FileEntry> = extras
            .list_files(&self.folder)
            .into_iter()
            .filter(|f| self.accepted(&f.name))
            .collect();
        files.sort_by(|a, b| {
            b.modified_ms
                .total_cmp(&a.modified_ms)
                .then_with(|| a.name.cmp(&b.name))
        });
        self.files = files;
    }

    fn shows_folders(&self) -> bool {
        self.extras.as_ref().is_some_and(|e| e.shows_folders())
    }

    fn default_caption(&self) -> String {
        let kinds = self.extensions.join(", ");
        if self.shows_folders() {
            format!("Put {kinds} files in the folder to see them here.")
        } else {
            format!("Add {kinds} files from your computer.")
        }
    }

    /// File `name` read and handed on; false (and the caption says why) if it couldn't be.
    fn choose(&mut self, name: &str) -> bool {
        let bytes = self
            .extras
            .as_ref()
            .and_then(|e| e.read_file(&format!("{}/{name}", self.folder)));
        match bytes {
            None => {
                self.caption = Some(format!("Couldn't read {name}"));
                false
            }
            Some(b) if b.len() > MAX_SIZE => {
                self.caption = Some(format!("{name} is too big: over {} MB", MAX_SIZE >> 20));
                false
            }
            Some(b) => {
                (self.chosen)(name, b);
                true
            }
        }
    }

    /// A file picked in the browser: copied into the folder, if it's one of the kinds.
    fn picked(&mut self, name: &str, data: &[u8]) {
        let name = name.rsplit(['/', '\\']).next().unwrap_or(name);
        let ok = self.accepted(name)
            && crate::host_extras::safe_path(name)
            && platform().is_some_and(|p| (p.write)(&format!("{}/{name}", self.folder), data));
        self.caption =
            (!ok).then(|| format!("{name} wasn't added: only {}", self.extensions.join(", ")));
        self.list_files();
    }
}

impl Screen for FileChooser {
    fn draw(&mut self, cx: &mut Cx) {
        if self.extras.is_none() {
            self.extras = Some(cx.host.extras());
            self.list_files();
        }
        if let Some((name, data)) = platform().and_then(|p| (p.take_picked)()) {
            self.picked(&name, &data);
        }
        let caption = self
            .caption
            .clone()
            .unwrap_or_else(|| self.default_caption());
        let open = if self.shows_folders() {
            "Open folder"
        } else {
            "Add files"
        };
        let cancel = cx.host.translate("gui.cancel");
        let files = &self.files;
        let mut load = None;
        let pressed = self.list.draw(
            cx,
            &self.title,
            &caption,
            false,
            |cx, rows| {
                if files.is_empty() {
                    rows.heading(cx, "No files yet");
                }
                for (i, file) in files.iter().enumerate() {
                    let Some(r) = rows.row(cx, &file.name, "", &[SMALL_BUTTON_WIDTH]) else {
                        continue;
                    };
                    if ui::button(cx.p, cx.ui, rows.visible(r[0]), "Load", true) {
                        load = Some(i);
                    }
                }
            },
            &[open, "Refresh", cancel.as_str()],
            Some(FOOTER_BUTTON),
        );
        if let Some(i) = load {
            let name = self.files[i].name.clone();
            if self.choose(&name) {
                cx.go(Go::Back);
            }
        }
        match pressed {
            Some(0) => {
                if let Some(p) = platform() {
                    (p.make_folder)(&self.folder);
                }
                if self.shows_folders() {
                    let folder = self.folder.clone();
                    if let Some(e) = self.extras.as_mut() {
                        e.open_path(&folder);
                    }
                } else if let Some(p) = platform() {
                    (p.pick)(&self.extensions.join(","));
                }
            }
            Some(1) => {
                self.caption = None;
                self.list_files();
            }
            Some(_) => cx.go(Go::Back),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_extras::NoExtras;
    use std::cell::RefCell;

    thread_local! {
        static CHOSEN: RefCell<Vec<(String, usize)>> = const { RefCell::new(Vec::new()) };
    }

    fn chosen(name: &str, bytes: Vec<u8>) {
        CHOSEN.with(|c| c.borrow_mut().push((name.to_owned(), bytes.len())));
    }

    struct Folder(Vec<(&'static str, f64)>);

    impl HostExtras for Folder {
        fn list_files(&self, folder: &str) -> Vec<FileEntry> {
            assert_eq!(folder, "schematics");
            self.0
                .iter()
                .map(|(n, t)| FileEntry {
                    name: (*n).into(),
                    modified_ms: *t,
                })
                .collect()
        }
        fn read_file(&self, path: &str) -> Option<Vec<u8>> {
            (path == "schematics/castle.litematic").then(|| vec![1, 2, 3])
        }
        fn file_exists(&self, _: &str) -> bool {
            false
        }
        fn rename_file(&mut self, _: &str, _: &str) -> bool {
            false
        }
        fn delete_file(&mut self, _: &str) -> bool {
            false
        }
        fn shows_folders(&self) -> bool {
            true
        }
        fn open_path(&mut self, _: &str) {}
        fn download(&mut self, _: &str, _: Vec<u8>) -> bool {
            false
        }
        fn set_clipboard_image(&mut self, _: &[u8]) -> bool {
            false
        }
        fn open_link(&mut self, _: &str) {}
    }

    fn chooser(files: Vec<(&'static str, f64)>) -> FileChooser {
        FileChooser::new(
            "Choose a schematic",
            "schematics",
            &[".litematic", ".schem", ".nbt"],
            chosen,
        )
        .with_extras(Box::new(Folder(files)))
    }

    #[test]
    fn lists_the_kinds_newest_first() {
        let c = chooser(vec![
            ("old.schem", 1.0),
            ("notes.txt", 9.0),
            ("castle.litematic", 5.0),
            ("B.NBT", 5.0),
        ]);
        let names: Vec<&str> = c.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["B.NBT", "castle.litematic", "old.schem"]);
        assert_eq!(
            c.default_caption(),
            "Put .litematic, .schem, .nbt files in the folder to see them here."
        );
    }

    #[test]
    fn loading_hands_the_bytes_on_or_says_why_not() {
        let mut c = chooser(vec![("castle.litematic", 1.0), ("gone.schem", 2.0)]);
        assert!(c.choose("castle.litematic"));
        assert_eq!(
            CHOSEN.with(|x| x.borrow().clone()),
            [("castle.litematic".to_owned(), 3)]
        );
        assert!(!c.choose("gone.schem"));
        assert_eq!(c.caption.as_deref(), Some("Couldn't read gone.schem"));
        // Picked with no platform to write it: not added.
        c.picked("dir/hut.schem", &[1]);
        assert_eq!(
            c.caption.as_deref(),
            Some("hut.schem wasn't added: only .litematic, .schem, .nbt")
        );
    }

    #[test]
    fn the_browser_adds_files_and_only_schematics_choose_opens_it() {
        let c =
            FileChooser::new("T", "schematics", &[".nbt"], chosen).with_extras(Box::new(NoExtras));
        assert_eq!(c.default_caption(), "Add .nbt files from your computer.");
        assert!(for_action("schematics", "choose").is_some());
        assert!(
            for_action("schematics", "turn").is_none()
                && for_action("waypoints", "choose").is_none()
        );
    }
}
