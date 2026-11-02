//! What the screenshot gallery, the credits and the update notice need of the game beyond `Host`:
//! files anywhere under the game folder (`screenshots/`), the page's downloads and image
//! clipboard, the desktop's file manager, and the browser opening a link. The game side
//! implements it; until `Host` hands one over (`Host::extras`, planned), screens take it as a
//! constructor argument.
//!
//! Paths are relative to the game folder, `/`-separated, and never leave it (`safe_path`).

/// A file in a folder: its name and when it was last written.
#[derive(Clone, Debug, PartialEq)]
pub struct FileEntry {
    pub name: String,
    /// Milliseconds since 1970; 0 when the platform keeps no times (the name sorts them then).
    pub modified_ms: f64,
}

pub trait HostExtras {
    /// The files (not folders) in `folder` under the game folder; empty when there's no such
    /// folder.
    fn list_files(&self, folder: &str) -> Vec<FileEntry>;
    /// File `path` under the game folder.
    fn read_file(&self, path: &str) -> Option<Vec<u8>>;
    fn file_exists(&self, path: &str) -> bool;
    /// Renames `from` to `to` (both under the game folder); false when it couldn't.
    fn rename_file(&mut self, from: &str, to: &str) -> bool;
    /// Deletes `path`; true when it's gone (or was never there).
    fn delete_file(&mut self, path: &str) -> bool;
    /// Whether the platform can show a folder or file in its file manager (the desktop); the
    /// browser downloads instead.
    fn shows_folders(&self) -> bool;
    /// Shows `path` (a folder or file under the game folder) in the file manager.
    fn open_path(&mut self, path: &str);
    /// Gives the player `data` as a file named `name` (the browser's download); false if the
    /// platform can't.
    fn download(&mut self, name: &str, data: Vec<u8>) -> bool;
    /// Puts a PNG on the clipboard as a picture; false if the platform can't (the desktop).
    fn set_clipboard_image(&mut self, png: &[u8]) -> bool;
    /// Opens `url` in the browser, without asking: Incari asks first (`ConfirmLink`).
    fn open_link(&mut self, url: &str);
}

/// Whether `path` is one a `HostExtras` acts on: relative, `/`-separated, no `..`, `.` or empty
/// parts, no `\` or `:`. The game side checks every path with it.
pub fn safe_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':', '\0'])
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// A `HostExtras` that has nothing and can do nothing: a platform without files, and tests.
#[derive(Default)]
pub struct NoExtras;

impl HostExtras for NoExtras {
    fn list_files(&self, _folder: &str) -> Vec<FileEntry> {
        Vec::new()
    }
    fn read_file(&self, _path: &str) -> Option<Vec<u8>> {
        None
    }
    fn file_exists(&self, _path: &str) -> bool {
        false
    }
    fn rename_file(&mut self, _from: &str, _to: &str) -> bool {
        false
    }
    fn delete_file(&mut self, _path: &str) -> bool {
        false
    }
    fn shows_folders(&self) -> bool {
        false
    }
    fn open_path(&mut self, _path: &str) {}
    fn download(&mut self, _name: &str, _data: Vec<u8>) -> bool {
        false
    }
    fn set_clipboard_image(&mut self, _png: &[u8]) -> bool {
        false
    }
    fn open_link(&mut self, _url: &str) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_stay_in_the_game_folder() {
        assert!(safe_path("screenshots"));
        assert!(safe_path("screenshots/2026-11-01_12.00.00.png"));
        for bad in [
            "",
            "/etc/passwd",
            "../x",
            "screenshots/../../x",
            "a//b",
            "a/./b",
            "C:x",
            "a\\b",
            "a/",
        ] {
            assert!(!safe_path(bad), "{bad}");
        }
    }
}
