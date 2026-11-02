//! What a player wears (ADR 0061; `:online`'s `Loadout`): at most one cosmetic in each worn slot.
//! Read from the ids the service sends, forgivingly: an id this client doesn't know, an emote, or a
//! second cosmetic for a slot is left out. Small and `Copy`: comparing two costs nothing.
use super::catalog::{self, Cosmetic, Slot};

/// The worn slots, in `Slot`'s order.
const WORN: usize = 5;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Loadout {
    by_slot: [Option<&'static Cosmetic>; WORN],
}

impl Loadout {
    /// Nothing worn.
    pub const EMPTY: Loadout = Loadout { by_slot: [None; WORN] };

    /// The loadout these ids describe, leaving out what this client can't draw.
    pub fn of<S: AsRef<str>>(ids: &[S]) -> Self {
        let mut out = Self::EMPTY;
        for id in ids {
            if let Some(c) = catalog::by_id(id.as_ref())
                && c.slot.worn()
                && out.by_slot[c.slot.index()].is_none()
            {
                out.by_slot[c.slot.index()] = Some(c);
            }
        }
        out
    }

    /// What's worn in `slot`.
    pub fn in_slot(&self, slot: Slot) -> Option<&'static Cosmetic> {
        if slot.worn() { self.by_slot[slot.index()] } else { None }
    }

    /// Whether `cosmetic` is worn.
    pub fn wears(&self, cosmetic: &Cosmetic) -> bool {
        self.in_slot(cosmetic.slot).is_some_and(|c| c.id == cosmetic.id)
    }

    /// This loadout with `cosmetic` in its slot, in place of what was there; itself for an emote.
    pub fn with(mut self, cosmetic: &'static Cosmetic) -> Self {
        if cosmetic.slot.worn() {
            self.by_slot[cosmetic.slot.index()] = Some(cosmetic);
        }
        self
    }

    /// This loadout with nothing in `slot`.
    pub fn without(mut self, slot: Slot) -> Self {
        if slot.worn() {
            self.by_slot[slot.index()] = None;
        }
        self
    }

    /// This loadout with only what `owns` allows.
    pub fn owned_by(mut self, owns: impl Fn(&Cosmetic) -> bool) -> Self {
        for slot in &mut self.by_slot {
            if slot.is_some_and(|c| !owns(c)) {
                *slot = None;
            }
        }
        self
    }

    /// What's worn, in slot order.
    pub fn worn(&self) -> impl Iterator<Item = &'static Cosmetic> + '_ {
        self.by_slot.iter().flatten().copied()
    }

    /// The ids worn, in slot order: what `equip` sends.
    pub fn ids(&self) -> Vec<String> {
        self.worn().map(|c| c.id.to_owned()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.by_slot.iter().all(Option::is_none)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_forgivingly_and_keeps_one_a_slot() {
        let l = Loadout::of(&["hat.crown", "hat.halo", "emote.wave", "cape.unknown", "cape.tide"]);
        assert_eq!(l.ids(), ["cape.tide", "hat.crown"], "slot order, the first of a slot, no emotes");
        let crown = catalog::by_id("hat.crown").unwrap();
        assert!(l.wears(crown));
        let halo = catalog::by_id("hat.halo").unwrap();
        let l2 = l.with(halo);
        assert!(l2.wears(halo) && !l2.wears(crown));
        assert_eq!(l2.without(Slot::Hat).without(Slot::Cape), Loadout::EMPTY);
        assert!(Loadout::of::<&str>(&[]).is_empty());
        assert_eq!(l.with(catalog::by_id("emote.bow").unwrap()), l, "emotes aren't worn");
        assert_eq!(l.owned_by(|c| c.slot != Slot::Hat).ids(), ["cape.tide"]);
    }
}
