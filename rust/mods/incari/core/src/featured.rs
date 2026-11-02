//! Featured servers (FeaturedServers.java, ADR 0062): pinned atop the server list, marked
//! Sponsored so it's plain they're there because they're featured, and kept out of the player's
//! own list: they can't be edited, deleted or moved, and they're never written to `servers.dat`,
//! so they come back each time the list loads. The list is built in (`incari.featured` in
//! `incari.properties`); the online service's `hello` can replace it for a run
//! (`FeaturedSource`).
//!
//! Horizon draws its own server list: the game side hands it this list through the fork's
//! `multiplayer::set_featured` hook, and draws the label after each featured name with
//! `draw_label`.
use crate::paint::Painter;

/// The label on each featured entry.
pub const LABEL: &str = "Sponsored";

/// A featured server: what the list calls it and where it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub address: String,
}

/// Who may name the featured servers for a run (the online service).
pub trait FeaturedSource {
    /// The servers named, or None to keep the built-in list.
    fn featured_servers(&self) -> Option<Vec<Entry>>;
}

/// The built-in list, or the service's if it named one.
pub fn entries(built: &str, source: Option<&dyn FeaturedSource>) -> Vec<Entry> {
    source
        .and_then(|s| s.featured_servers())
        .unwrap_or_else(|| parse(built))
}

/// `incari.featured`'s `name|address` entries, joined by `;`; malformed ones left out.
pub fn parse(built: &str) -> Vec<Entry> {
    built
        .split(';')
        .filter_map(|part| {
            let bar = part.find('|')?;
            (bar > 0 && bar < part.len() - 1).then(|| Entry {
                name: part[..bar].trim().to_owned(),
                address: part[bar + 1..].trim().to_owned(),
            })
        })
        .collect()
}

/// Whether a saved server (`name`, `address`) is a copy of a featured one that an earlier build
/// saved, to leave out of the list (and drop from `servers.dat` at its next save). `own`: the
/// entry carries the player's mark (`incari_own`), so it's theirs even with a featured server's
/// name and address.
pub fn is_saved_copy(name: &str, address: &str, own: bool, entries: &[Entry]) -> bool {
    !own && entries
        .iter()
        .any(|e| e.name == name && e.address.eq_ignore_ascii_case(address))
}

/// The Sponsored label after a featured server's name, at (`x`, `y`): the accent's text on a
/// faint accent pill. Its width.
pub fn draw_label(p: &mut Painter, x: f32, y: f32) -> f32 {
    let accent = p.theme.accent;
    let width = p.width(LABEL) + 8.0;
    p.round(x, y - 1.0, width, 10.0, 3.0, accent & 0x40FF_FFFF);
    p.text(LABEL, x + 4.0, y, accent, false);
    width
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_built_in_list() {
        let list =
            parse("Carrotcraft|wss://eagler.carrot-craft.org; Two | two.example:25565;bad;|x;y|");
        assert_eq!(
            list,
            [
                Entry {
                    name: "Carrotcraft".into(),
                    address: "wss://eagler.carrot-craft.org".into()
                },
                Entry {
                    name: "Two".into(),
                    address: "two.example:25565".into()
                },
            ]
        );
        assert!(parse("").is_empty());
    }

    #[test]
    fn the_service_replaces_the_list() {
        struct Named;
        impl FeaturedSource for Named {
            fn featured_servers(&self) -> Option<Vec<Entry>> {
                Some(vec![])
            }
        }
        assert!(entries("A|a", Some(&Named)).is_empty());
        assert_eq!(entries("A|a", None).len(), 1);
    }

    #[test]
    fn old_saved_copies_are_dropped_but_the_players_own_kept() {
        let list = parse("Carrotcraft|wss://eagler.carrot-craft.org");
        assert!(is_saved_copy(
            "Carrotcraft",
            "WSS://eagler.carrot-craft.org",
            false,
            &list
        ));
        assert!(!is_saved_copy(
            "Carrotcraft",
            "wss://eagler.carrot-craft.org",
            true,
            &list
        ));
        assert!(!is_saved_copy(
            "Mine",
            "wss://eagler.carrot-craft.org",
            false,
            &list
        ));
    }
}
