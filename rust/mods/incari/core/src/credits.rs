//! What the credits screen lists (Credits.java): Incari and the third-party software the Rust
//! build adds, each under its own license, with where its source and license are. Horizon's own
//! dependencies are Horizon's to credit (its `THIRD_PARTY_NOTICES.md`), as the game's own
//! libraries are Mojang's in the Java build.
use crate::release::BuildInfo;

/// One entry: a piece of software in this build, under its own license.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Credit {
    pub name: &'static str,
    /// The version in this build, shown after the name; None when it isn't known here.
    pub version: Option<String>,
    /// Who holds its copyright.
    pub authors: &'static str,
    /// The license's short name, as SPDX names it where it can.
    pub license: &'static str,
    /// Where the license's full text is.
    pub license_text: String,
    /// Where its source code is.
    pub source: String,
}

impl Credit {
    /// The name with the version after it, if there is one.
    pub fn title(&self) -> String {
        match &self.version {
            Some(v) => format!("{} {v}", self.name),
            None => self.name.to_owned(),
        }
    }
}

pub const GPL_3: &str = "https://www.gnu.org/licenses/gpl-3.0.html";
pub const APACHE_2: &str = "https://www.apache.org/licenses/LICENSE-2.0";
pub const MIT: &str = "https://opensource.org/license/mit";
pub const OFL_1_1: &str = "https://openfontlicense.org/open-font-license-official-text/";

/// The pause menu's Report Bugs: a new issue on the repository, from its bug template.
pub fn report_bugs(build: &BuildInfo) -> String {
    format!("{}/issues/new?template=bug_report.yml", build.repository)
}

/// The pause menu's Give Feedback: a new discussion in the repository's Feedback category.
pub fn feedback(build: &BuildInfo) -> String {
    format!("{}/discussions/new?category=feedback", build.repository)
}

/// Everything this build credits: Incari first, then the game it runs on, then what Incari's
/// client compiles in, then the look's fonts.
pub fn all(build: &BuildInfo) -> Vec<Credit> {
    let c = |name, authors, license, license_text: &str, source: &str| Credit {
        name,
        version: None,
        authors,
        license,
        license_text: license_text.to_owned(),
        source: source.to_owned(),
    };
    let repo = build.repository.as_str();
    let mut all = vec![
        c(
            "Incari Client",
            "The Incari Client contributors",
            "GPL-3.0-or-later, with an exception for Minecraft",
            &format!("{repo}/blob/main/LICENSE-EXCEPTION.txt"),
            repo,
        ),
        c(
            "Horizon",
            "The Horizon contributors",
            "MIT",
            &build.horizon_page("LICENSE"),
            &build.horizon_page(""),
        ),
        c(
            "horizon-mixins",
            "The Horizon contributors",
            "MIT",
            &build.mixins_page("LICENSE"),
            &build.mixins_page(""),
        ),
        // incari_core's own dependencies: PNGs, the wordmark's font, profiles' JSON.
        c(
            "png",
            "The image-rs Developers",
            "MIT OR Apache-2.0",
            APACHE_2,
            "https://github.com/image-rs/image-png",
        ),
        c(
            "ab_glyph",
            "Alex Butler",
            "Apache-2.0",
            APACHE_2,
            "https://github.com/alexheretic/ab-glyph",
        ),
        c(
            "serde_json",
            "Erick Tryzelaar, David Tolnay",
            "MIT OR Apache-2.0",
            MIT,
            "https://github.com/serde-rs/json",
        ),
    ];
    // The look's fonts, in Incari's pack with their licences beside them.
    for (name, authors, source) in [
        (
            "Red Hat Display",
            "The Red Hat Project Authors",
            "https://github.com/RedHatOfficial/RedHatFont",
        ),
        (
            "Michroma",
            "The Michroma Project Authors",
            "https://github.com/googlefonts/Michroma-font",
        ),
        (
            "Inter",
            "The Inter Project Authors",
            "https://github.com/rsms/inter",
        ),
        (
            "Nunito",
            "The Nunito Project Authors",
            "https://github.com/googlefonts/nunito",
        ),
        (
            "Poppins",
            "The Poppins Project Authors",
            "https://github.com/itfoundry/Poppins",
        ),
        (
            "JetBrains Mono",
            "The JetBrains Mono Project Authors",
            "https://github.com/JetBrains/JetBrainsMono",
        ),
    ] {
        all.push(c(name, authors, "OFL-1.1", OFL_1_1, source));
    }
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incari_comes_first_with_its_source_offer() {
        let build = BuildInfo {
            repository: "https://github.com/incari-client/incari-client".into(),
            horizon_repository: "https://github.com/x/horizon-web.git".into(),
            horizon_revision: "abc".into(),
            mixins_repository: "https://github.com/x/horizon-mixins".into(),
            mixins_revision: "def".into(),
            ..BuildInfo::default()
        };
        let all = all(&build);
        assert_eq!(all[0].title(), "Incari Client");
        assert_eq!(all[0].source, build.repository);
        assert_eq!(all[1].source, "https://github.com/x/horizon-web");
        assert!(all.iter().all(|c| c.license_text.starts_with("https://")));
        assert_eq!(
            report_bugs(&build),
            "https://github.com/incari-client/incari-client/issues/new?template=bug_report.yml"
        );
    }
}
