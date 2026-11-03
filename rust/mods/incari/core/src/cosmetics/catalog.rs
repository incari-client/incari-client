//! Every cosmetic this client can draw (ADR 0061; `:online`'s `Catalog` and `Cosmetic`), in the
//! wardrobe's order. The service keeps the same ids (`docs/online-protocol.md`); an id it sends
//! that isn't here (a newer client's) is left out, so a newer catalog never breaks this one.

/// Where a cosmetic is worn. An account wears at most one per slot; emotes are played, not worn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Slot {
    Cape,
    Hat,
    Back,
    Nametag,
    Trail,
    Emote,
}

impl Slot {
    pub const ALL: [Slot; 6] = [
        Slot::Cape,
        Slot::Hat,
        Slot::Back,
        Slot::Nametag,
        Slot::Trail,
        Slot::Emote,
    ];

    /// The ids' prefix: `cape` in `cape.frost`.
    pub fn prefix(self) -> &'static str {
        match self {
            Slot::Cape => "cape",
            Slot::Hat => "hat",
            Slot::Back => "back",
            Slot::Nametag => "nametag",
            Slot::Trail => "trail",
            Slot::Emote => "emote",
        }
    }

    /// What the wardrobe's tab says.
    pub fn label(self) -> &'static str {
        match self {
            Slot::Cape => "Capes",
            Slot::Hat => "Hats",
            Slot::Back => "Wings & Back",
            Slot::Nametag => "Name Tags",
            Slot::Trail => "Trails",
            Slot::Emote => "Emotes",
        }
    }

    /// Whether a cosmetic in this slot is worn (and sent with `equip`); emotes aren't.
    pub fn worn(self) -> bool {
        self != Slot::Emote
    }

    /// Its place in `ALL`.
    pub fn index(self) -> usize {
        self as usize
    }
}

/// One cosmetic: something an account wears that other Incari players see, or an emote it plays.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Cosmetic {
    /// `<slot>.<name>`; never changes.
    pub id: &'static str,
    pub slot: Slot,
    /// What the wardrobe calls it.
    pub name: &'static str,
    /// Whether every account owns it.
    pub free: bool,
}

const fn free(id: &'static str, slot: Slot, name: &'static str) -> Cosmetic {
    Cosmetic {
        id,
        slot,
        name,
        free: true,
    }
}

use Slot::*;

/// Every cosmetic, in the wardrobe's order.
pub static ALL: [Cosmetic; 37] = [
    // Capes: the client's own art (`art::cape`), some of it moving.
    free("cape.frost", Cape, "Frost"),
    free("cape.aurora", Cape, "Aurora"),
    free("cape.midnight", Cape, "Midnight"),
    free("cape.ember", Cape, "Ember"),
    free("cape.sakura", Cape, "Sakura"),
    free("cape.tide", Cape, "Tide"),
    free("cape.mono", Cape, "Mono"),
    // Hats: small models on the head, hidden under a helmet.
    free("hat.crown", Hat, "Crown"),
    free("hat.halo", Hat, "Halo"),
    free("hat.top_hat", Hat, "Top Hat"),
    free("hat.beanie", Hat, "Beanie"),
    free("hat.horns", Hat, "Horns"),
    free("hat.cat_ears", Hat, "Cat Ears"),
    // On the back: hidden while an elytra is worn.
    free("back.angel_wings", Back, "Angel Wings"),
    free("back.frost_wings", Back, "Frost Wings"),
    free("back.bat_wings", Back, "Bat Wings"),
    free("back.backpack", Back, "Backpack"),
    // Name tags: the name's colour over the head.
    free("nametag.ice", Nametag, "Ice"),
    free("nametag.gold", Nametag, "Gold"),
    free("nametag.mint", Nametag, "Mint"),
    free("nametag.rose", Nametag, "Rose"),
    free("nametag.violet", Nametag, "Violet"),
    free("nametag.frost", Nametag, "Frost Fade"),
    free("nametag.sunset", Nametag, "Sunset Fade"),
    free("nametag.aurora", Nametag, "Aurora Fade"),
    // Trails: a few particles behind the player as they move.
    free("trail.snow", Trail, "Snowflakes"),
    free("trail.hearts", Trail, "Hearts"),
    free("trail.sparkles", Trail, "Sparkles"),
    free("trail.notes", Trail, "Notes"),
    // Emotes: played from the emote wheel, seen by Incari players nearby.
    free("emote.wave", Emote, "Wave"),
    free("emote.clap", Emote, "Clap"),
    free("emote.cheer", Emote, "Cheer"),
    free("emote.salute", Emote, "Salute"),
    free("emote.facepalm", Emote, "Facepalm"),
    free("emote.shrug", Emote, "Shrug"),
    free("emote.dab", Emote, "Dab"),
    free("emote.bow", Emote, "Bow"),
];

/// Every cosmetic, in the wardrobe's order.
pub fn all() -> &'static [Cosmetic] {
    &ALL
}

/// The cosmetic with this id, or None if this client has none by it.
pub fn by_id(id: &str) -> Option<&'static Cosmetic> {
    ALL.iter().find(|c| c.id == id)
}

/// The cosmetics in `slot`, in the wardrobe's order.
pub fn in_slot(slot: Slot) -> impl Iterator<Item = &'static Cosmetic> {
    ALL.iter().filter(move |c| c.slot == slot)
}

/// Its place in `ALL`: a small number for a cosmetic's textures and caches.
pub fn index(cosmetic: &Cosmetic) -> usize {
    ALL.iter().position(|c| c.id == cosmetic.id).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_in_their_slot() {
        for (i, c) in ALL.iter().enumerate() {
            assert!(
                c.id.starts_with(&format!("{}.", c.slot.prefix())),
                "{}",
                c.id
            );
            assert!(
                ALL[..i].iter().all(|o| o.id != c.id),
                "two cosmetics are {}",
                c.id
            );
            assert_eq!(index(c), i);
        }
        let counts: Vec<usize> = Slot::ALL.iter().map(|s| in_slot(*s).count()).collect();
        assert_eq!(counts, [7, 6, 4, 8, 4, 8], "ADR 0061's 37");
        assert_eq!(by_id("hat.halo").map(|c| c.name), Some("Halo"));
        assert!(by_id("hat.unknown").is_none());
    }
}
