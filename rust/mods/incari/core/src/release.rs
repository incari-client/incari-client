//! The client's own version and repository (IncariBuild), Incari's release versions (Releases.java)
//! and the update notice (UpdateNotice.java, ADR 0060): whether the title screen tells the player a
//! newer Incari Client is out, and leads them to its repository.
//!
//! The Java build writes `incari.properties` into `IncariBuild`; the Rust build reads the same
//! file, compiled into the bundle's data (`BuildInfo::from_properties`). Who names the latest
//! release is the online service (its `hello`), behind `ReleaseSource`: with none, no notice.
use crate::paint::Measure;

/// What the build was made from (`incari.properties`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BuildInfo {
    /// `incari.version`: this tree's Incari Client version (`0.1.6`).
    pub version: String,
    /// `incari.repository`: the public repository, where the notice and the credits lead.
    pub repository: String,
    /// `incari.featured`: the built-in featured servers, `name|address` joined by `;`.
    pub featured: String,
    /// `horizon.repository` and `horizon.revision`: the game this build runs on.
    pub horizon_repository: String,
    pub horizon_revision: String,
    /// `horizon.mixins.repository` and `horizon.mixins.revision`: what applied the mods.
    pub mixins_repository: String,
    pub mixins_revision: String,
    /// A release build (the notice shows only in one).
    pub release: bool,
}

impl BuildInfo {
    pub fn from_properties(text: &str, release: bool) -> Self {
        let get = |key: &str| property(text, key).unwrap_or_default().to_owned();
        Self {
            version: get("incari.version"),
            repository: get("incari.repository"),
            featured: get("incari.featured"),
            horizon_repository: get("horizon.repository"),
            horizon_revision: get("horizon.revision"),
            mixins_repository: get("horizon.mixins.repository"),
            mixins_revision: get("horizon.mixins.revision"),
            release,
        }
    }

    /// The repository's page for `path` at the pinned revision, from a `.git` clone URL.
    pub fn horizon_page(&self, path: &str) -> String {
        page(&self.horizon_repository, &self.horizon_revision, path)
    }

    pub fn mixins_page(&self, path: &str) -> String {
        page(&self.mixins_repository, &self.mixins_revision, path)
    }
}

fn page(repository: &str, revision: &str, path: &str) -> String {
    let base = repository.trim_end_matches(".git");
    if path.is_empty() {
        base.to_owned()
    } else {
        format!("{base}/blob/{revision}/{path}")
    }
}

/// `key`'s value in a `.properties` text: `key=value` lines, `#` and `!` comments. Enough for
/// `incari.properties` (no escapes, no continued lines).
pub fn property<'t>(text: &'t str, key: &str) -> Option<&'t str> {
    text.lines()
        .map(str::trim_start)
        .filter(|l| !l.starts_with('#') && !l.starts_with('!'))
        .filter_map(|l| l.split_once(['=', ':']))
        .find(|(k, _)| k.trim() == key)
        .map(|(_, v)| v.trim())
}

/// At most this many changelog lines, each at most `MAX_NOTE` characters: all fit the notes screen.
pub const MAX_NOTES: usize = 12;
pub const MAX_NOTE: usize = 120;

/// `version` if it's a release version (one to four whole numbers of up to six digits, joined by
/// dots), else None: anything else the service sends is dropped.
pub fn valid(version: &str) -> Option<&str> {
    let parts: Vec<&str> = version.split('.').collect();
    let ok = (1..=4).contains(&parts.len())
        && parts
            .iter()
            .all(|p| (1..=6).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()));
    ok.then_some(version)
}

/// Whether `latest` comes after `current`. Missing parts count as 0, so `1.2` and `1.2.0` are the
/// same; either not a release version is never newer.
pub fn newer(latest: &str, current: &str) -> bool {
    let (Some(a), Some(b)) = (valid(latest), valid(current)) else {
        return false;
    };
    let a: Vec<u32> = a.split('.').map(|p| p.parse().unwrap_or(0)).collect();
    let b: Vec<u32> = b.split('.').map(|p| p.parse().unwrap_or(0)).collect();
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (
            a.get(i).copied().unwrap_or(0),
            b.get(i).copied().unwrap_or(0),
        );
        if x != y {
            return x > y;
        }
    }
    false
}

/// Whether `note` is a changelog line the client shows: 1 to `MAX_NOTE` characters, no control
/// characters and nothing invisible (zero-width or direction marks), which could make it read as
/// something other than it is.
pub fn note(note: &str) -> bool {
    !note.trim().is_empty()
        && note.chars().count() <= MAX_NOTE
        && !note.chars().any(|c| c.is_control() || format_char(c))
}

/// Unicode's format characters (category Cf).
fn format_char(c: char) -> bool {
    matches!(c as u32,
        0xAD | 0x600..=0x605 | 0x61C | 0x6DD | 0x70F | 0x890..=0x891 | 0x8E2 | 0x180E
        | 0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x2064 | 0x2066..=0x206F | 0xFEFF
        | 0xFFF9..=0xFFFB | 0x110BD | 0x110CD | 0x13430..=0x1343F | 0x1BCA0..=0x1BCA3
        | 0x1D173..=0x1D17A | 0xE0001 | 0xE0020..=0xE007F)
}

/// What the online service last said about releases (OnlineService's `latestRelease`,
/// `status() == UPDATE_NEEDED`, `releaseNotes`).
pub trait ReleaseSource {
    /// The latest release the service named, if any.
    fn latest_release(&self) -> Option<String>;
    /// The service no longer speaks this client's protocol.
    fn update_needed(&self) -> bool;
    /// What's new in the latest release, one change a line.
    fn release_notes(&self) -> Vec<String>;
}

/// What the notice says, or None for none: the newer version when the service named one, "" when
/// the service no longer speaks this client's protocol (an update is needed, version unknown).
/// Release builds only, unless `forced` (tests, `?updates=on`).
pub fn newer_release(
    build: &BuildInfo,
    forced: bool,
    source: &dyn ReleaseSource,
) -> Option<String> {
    if !(build.release || forced) || build.repository.is_empty() {
        return None;
    }
    if let Some(latest) = source.latest_release()
        && newer(&latest, &build.version)
    {
        return Some(latest);
    }
    source.update_needed().then(String::new)
}

/// What's new in the release `newer_release` names, checked line by line; empty for none.
pub fn notes(build: &BuildInfo, forced: bool, source: &dyn ReleaseSource) -> Vec<String> {
    match newer_release(build, forced, source) {
        Some(v) if !v.is_empty() => source
            .release_notes()
            .into_iter()
            .filter(|n| note(n))
            .take(MAX_NOTES)
            .collect(),
        _ => Vec::new(),
    }
}

/// The title screen's notice (TitleLook's update button): its label, hint and tooltip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    /// The release it's for ("" when unknown).
    pub version: String,
    pub label: String,
    pub hint: &'static str,
    pub tooltip: String,
    /// What's new, when the service said: the notice opens it, else the repository.
    pub notes: Vec<String>,
}

pub const UPDATE_HINT: &str = "GitHub";
pub const NOTES_HINT: &str = "What's new";

impl Notice {
    /// The notice to show now, if any.
    pub fn now(build: &BuildInfo, forced: bool, source: &dyn ReleaseSource) -> Option<Self> {
        let version = newer_release(build, forced, source)?;
        let notes = notes(build, forced, source);
        let label = if version.is_empty() {
            "A new Incari Client is out".to_owned()
        } else {
            format!("Incari Client {version} is out")
        };
        let (hint, tooltip) = if notes.is_empty() {
            (UPDATE_HINT, "Open Incari Client on GitHub".to_owned())
        } else {
            let mut text = String::from("What's new:");
            for n in &notes {
                text.push_str("\n\u{2022} ");
                text.push_str(n);
            }
            text.push_str("\n\nClick to get it");
            (NOTES_HINT, text)
        };
        Some(Self {
            version,
            label,
            hint,
            tooltip,
            notes,
        })
    }

    /// Where it goes: centred at the top, between the small screens' links on the left (ending at
    /// `links_right`, 0 with the card) and the language and accessibility buttons on the right.
    pub fn rect(&self, measure: &dyn Measure, width: f32, links_right: f32) -> [f32; 4] {
        const ROW: f32 = crate::theme::CONTROL_HEIGHT;
        // The menu button's insets: 8 and the icon (9 + 6) before the label, the hint's gap, 8
        // after.
        let w = 8.0 + 15.0 + measure.width(&self.label) + 14.0 + measure.width(self.hint) + 8.0;
        let (left, right) = (links_right + 6.0, width - 2.0 * ROW - 16.0);
        let x = ((width - w) / 2.0).floor().min(right - w).max(left);
        [x, 6.0, w, ROW]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Service(Option<&'static str>, bool, Vec<String>);

    impl ReleaseSource for Service {
        fn latest_release(&self) -> Option<String> {
            self.0.map(str::to_owned)
        }
        fn update_needed(&self) -> bool {
            self.1
        }
        fn release_notes(&self) -> Vec<String> {
            self.2.clone()
        }
    }

    const PROPERTIES: &str = "# comment\nincari.version=0.1.6\nincari.repository=https://github.com/incari-client/incari-client\n\
        incari.featured=Carrotcraft|wss://eagler.carrot-craft.org\nhorizon.repository=https://github.com/x/horizon-web.git\nhorizon.revision=abc\n";

    #[test]
    fn reads_incari_properties() {
        let b = BuildInfo::from_properties(PROPERTIES, true);
        assert_eq!(b.version, "0.1.6");
        assert_eq!(b.featured, "Carrotcraft|wss://eagler.carrot-craft.org");
        assert_eq!(
            b.horizon_page("LICENSE"),
            "https://github.com/x/horizon-web/blob/abc/LICENSE"
        );
        assert_eq!(property(PROPERTIES, "missing"), None);
    }

    #[test]
    fn compares_releases_as_java_does() {
        assert!(newer("0.2.0", "0.1.6"));
        assert!(newer("1.10", "1.9.9"));
        assert!(!newer("1.2", "1.2.0"));
        assert!(!newer("0.1.4", "0.1.6"));
        assert!(!newer("0.2.0-beta", "0.1.6"));
        assert!(!newer("1.2.3.4.5", "0.1"));
        assert!(!newer("1234567", "0.1"));
        assert!(note("Faster chunk loading"));
        assert!(!note("   "));
        assert!(!note("hidden\u{200B}mark"));
        assert!(!note(&"x".repeat(121)));
    }

    #[test]
    fn shows_the_notice_only_in_a_release_build_with_a_newer_release() {
        let release = BuildInfo::from_properties(PROPERTIES, true);
        let dev = BuildInfo::from_properties(PROPERTIES, false);
        let service = Service(
            Some("0.2.0"),
            false,
            vec!["Faster".into(), "bad\u{202E}".into()],
        );
        assert_eq!(newer_release(&dev, false, &service), None);
        assert_eq!(
            newer_release(&dev, true, &service).as_deref(),
            Some("0.2.0")
        );
        let notice = Notice::now(&release, false, &service).unwrap();
        assert_eq!(notice.label, "Incari Client 0.2.0 is out");
        assert_eq!(notice.hint, NOTES_HINT);
        assert_eq!(notice.notes, ["Faster"]);
        assert_eq!(
            Notice::now(&release, false, &Service(Some("0.1.6"), false, vec![])),
            None
        );
        let needed = Notice::now(&release, false, &Service(None, true, vec![])).unwrap();
        assert_eq!(
            (needed.label.as_str(), needed.hint),
            ("A new Incari Client is out", UPDATE_HINT)
        );
    }
}
