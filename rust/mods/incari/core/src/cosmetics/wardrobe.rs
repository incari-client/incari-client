//! The player's own cosmetics (ADR 0061; `:online`'s `Wardrobe`): what they wear, what they own,
//! their saved outfits, and whether they see other players' cosmetics. Kept on this device in
//! `online/wardrobe.json` under `incari/` (the Java client's file, same keys), so what's worn
//! shows at once and without an account; while signed in, the account's wardrobe on the service
//! is the truth and this follows it.
//!
//! Changes apply here at once, so the Wardrobe never waits on the service; `sync`, once a frame,
//! sends `equip` at most once a second (the Java `OnlineService` tick's part).
//!
//! No files here: `load` takes the file's text and `take_unsaved` hands the text to write.
use super::catalog::{self, Cosmetic, Slot};
use super::loadout::Loadout;
use crate::online::{Failure, Online, Outcome, Status, Ticket};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::HashSet;

/// The file, under the game folder's `incari/`.
pub const FILE: &str = "online/wardrobe.json";
/// The most outfits kept.
pub const MAX_OUTFITS: usize = 12;
/// The longest outfit name.
pub const MAX_OUTFIT_NAME: usize = 24;
/// At least this long between two `equip`s: the wardrobe's clicks are gathered into one.
pub const EQUIP_INTERVAL_MILLIS: f64 = 1_000.0;

/// A loadout the player saved under a name, to put on again in one click.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outfit {
    pub name: String,
    pub loadout: Loadout,
}

/// What `sync` needs of the online service (`Online`; a stand-in in the tests).
pub trait Service {
    fn status(&self) -> Status;
    fn greeted(&self) -> bool;
    /// The account's name, kept while reconnecting; None once signed out.
    fn account_name(&self) -> Option<&str>;
    /// (owned, equipped) as the service last said.
    fn wardrobe(&self) -> Option<&(Vec<String>, Vec<String>)>;
    fn wardrobe_serial(&self) -> u64;
    fn equip(&mut self, ids: &[String]) -> Ticket;
    fn take_result(&mut self, ticket: Ticket) -> Option<Outcome>;
}

impl Service for Online {
    fn status(&self) -> Status {
        Online::status(self)
    }
    fn greeted(&self) -> bool {
        Online::greeted(self)
    }
    fn account_name(&self) -> Option<&str> {
        Online::account_name(self)
    }
    fn wardrobe(&self) -> Option<&(Vec<String>, Vec<String>)> {
        Online::wardrobe(self)
    }
    fn wardrobe_serial(&self) -> u64 {
        Online::wardrobe_serial(self)
    }
    fn equip(&mut self, ids: &[String]) -> Ticket {
        Online::equip(self, ids)
    }
    fn take_result(&mut self, ticket: Ticket) -> Option<Outcome> {
        Online::take_result(self, ticket)
    }
}

#[derive(Debug)]
pub struct Wardrobe {
    loaded: bool,
    worn: Loadout,
    /// The non-free cosmetics the account owns, as the service last said.
    owned: HashSet<String>,
    outfits: Vec<Outfit>,
    show_others: bool,
    /// The account's wardrobe arrived on this connection: what's worn is then the account's.
    synced: bool,
    /// It arrived at all since signing in (a reconnect keeps changes made while away).
    account_seen: bool,
    /// What's worn changed here and the service hasn't heard yet.
    unsent: bool,
    revision: u64,
    /// Changed since the file was last handed out.
    dirty: bool,
    // The service's side.
    serial: u64,
    last_status: Status,
    equipping: Option<(Ticket, Loadout)>,
    next_equip: f64,
}

impl Default for Wardrobe {
    fn default() -> Self {
        Self {
            loaded: false,
            worn: Loadout::EMPTY,
            owned: HashSet::new(),
            outfits: Vec::new(),
            show_others: true,
            synced: false,
            account_seen: false,
            unsent: false,
            revision: 0,
            dirty: false,
            serial: 0,
            last_status: Status::Off,
            equipping: None,
            next_equip: 0.0,
        }
    }
}

impl Wardrobe {
    // ---------------------------------------------------------------------------------------------
    // What the screens read
    // ---------------------------------------------------------------------------------------------

    /// What the player wears now.
    pub fn worn(&self) -> Loadout {
        self.worn
    }

    /// Whether the player may wear (or play) `cosmetic`.
    pub fn owns(&self, cosmetic: &Cosmetic) -> bool {
        cosmetic.free || self.owned.contains(cosmetic.id)
    }

    /// The emotes the player owns, in the catalog's order: the emote wheel's.
    pub fn emotes(&self) -> Vec<&'static Cosmetic> {
        catalog::in_slot(Slot::Emote).filter(|c| self.owns(c)).collect()
    }

    /// The saved outfits, oldest first.
    pub fn outfits(&self) -> &[Outfit] {
        &self.outfits
    }

    /// Whether other players' cosmetics are drawn (the player may find them distracting).
    pub fn show_others(&self) -> bool {
        self.show_others
    }

    /// Whether other Incari players see what's worn: signed in, with the account's wardrobe here.
    pub fn shared(&self) -> bool {
        self.synced
    }

    /// Goes up whenever anything here changes.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    // ---------------------------------------------------------------------------------------------
    // What the screens change
    // ---------------------------------------------------------------------------------------------

    /// Puts `cosmetic` on, in place of what was in its slot. Ignored for one not owned.
    pub fn wear(&mut self, cosmetic: &'static Cosmetic) {
        if self.owns(cosmetic) && cosmetic.slot.worn() {
            self.change(self.worn.with(cosmetic));
        }
    }

    /// Takes off what's worn in `slot`.
    pub fn take_off(&mut self, slot: Slot) {
        self.change(self.worn.without(slot));
    }

    /// Puts on `loadout` whole (an outfit, or nothing), leaving out what isn't owned.
    pub fn wear_all(&mut self, loadout: Loadout) {
        let owned = self.owned_filter(loadout);
        self.change(owned);
    }

    /// Saves what's worn as an outfit called `name` (cut to `MAX_OUTFIT_NAME`), replacing one of
    /// the same name. False if there's no room for another.
    pub fn save_outfit(&mut self, name: &str) -> bool {
        let mut trimmed: String = name.trim().chars().take(MAX_OUTFIT_NAME).collect();
        if trimmed.is_empty() {
            trimmed = format!("Outfit {}", self.outfits.len() + 1);
        }
        let worn = self.worn;
        if let Some(o) = self.outfits.iter_mut().find(|o| o.name.to_lowercase() == trimmed.to_lowercase()) {
            *o = Outfit { name: trimmed, loadout: worn };
            self.saved();
            return true;
        }
        if self.outfits.len() >= MAX_OUTFITS {
            return false;
        }
        self.outfits.push(Outfit { name: trimmed, loadout: worn });
        self.saved();
        true
    }

    /// Forgets the outfit at `index`.
    pub fn delete_outfit(&mut self, index: usize) {
        if index < self.outfits.len() {
            self.outfits.remove(index);
            self.saved();
        }
    }

    /// Draws other players' cosmetics, or not.
    pub fn set_show_others(&mut self, show: bool) {
        if show != self.show_others {
            self.show_others = show;
            self.saved();
        }
    }

    fn owned_filter(&self, loadout: Loadout) -> Loadout {
        loadout.owned_by(|c| c.free || self.owned.contains(c.id))
    }

    fn change(&mut self, next: Loadout) {
        if next == self.worn {
            return;
        }
        self.worn = next;
        self.unsent = true;
        self.saved();
    }

    // ---------------------------------------------------------------------------------------------
    // The service's side
    // ---------------------------------------------------------------------------------------------

    /// The account's wardrobe arrived. On signing in, what the account wears replaces what's worn
    /// here, unless the account wears nothing yet and this device does: then the device's is kept
    /// and sent, so what a player picked before making an account carries over. Later (a
    /// reconnect, or another session's change), a change made here and not yet sent wins. True
    /// when what's worn here should be sent.
    pub fn from_account(&mut self, owned: &[String], equipped: Loadout) -> bool {
        self.owned = owned.iter().cloned().collect();
        let keep = if self.account_seen { self.unsent } else { equipped.is_empty() && !self.worn.is_empty() };
        self.synced = true;
        self.account_seen = true;
        self.worn = if keep { self.owned_filter(self.worn) } else { self.owned_filter(equipped) };
        self.unsent = keep;
        self.saved();
        self.unsent
    }

    /// The service took what was sent; `equipped` is what it keeps.
    pub fn sent(&mut self, sent: Loadout, equipped: Loadout) {
        if self.worn == sent {
            self.unsent = false;
            if equipped != self.worn {
                self.worn = self.owned_filter(equipped);
                self.saved();
            }
        }
    }

    /// What's worn here, if the service hasn't heard of it yet and is to.
    pub fn unsent(&self) -> Option<Loadout> {
        (self.synced && self.unsent).then_some(self.worn)
    }

    /// Signed out: the device keeps what was worn, less what only the account owned.
    pub fn signed_out(&mut self) {
        self.synced = false;
        self.account_seen = false;
        self.unsent = false;
        self.owned.clear();
        self.worn = self.owned_filter(self.worn);
        self.saved();
    }

    /// The connection dropped: the account's wardrobe is asked for again when it's back.
    pub fn disconnected(&mut self) {
        if self.synced {
            self.synced = false;
            self.revision += 1;
        }
    }

    /// Once a frame, after the service's tick: the account's wardrobe taken when it arrives,
    /// sign-outs and dropped connections noticed, and what's worn sent (at most once a second).
    pub fn sync(&mut self, service: &mut dyn Service, now: f64) {
        let status = service.status();
        if self.last_status == Status::SignedIn && status != Status::SignedIn {
            if service.account_name().is_none() {
                self.signed_out();
            } else {
                self.disconnected();
            }
            // An answer to an equip on the old connection is never coming.
            self.equipping = None;
        }
        self.last_status = status;
        if let Some((ticket, loadout)) = self.equipping
            && let Some(outcome) = service.take_result(ticket)
        {
            self.equipping = None;
            match outcome {
                Ok(_) => {
                    let equipped = service.wardrobe().map_or(loadout, |w| Loadout::of(&w.1));
                    self.sent(loadout, equipped);
                }
                // The service and this client disagree on what's owned: it sends its wardrobe again.
                Err(Failure::Rejected) => self.sent(loadout, loadout),
                // Offline or limited: still unsent, so it goes again later.
                Err(_) => {}
            }
        }
        let serial = service.wardrobe_serial();
        if serial != self.serial {
            self.serial = serial;
            if status == Status::SignedIn
                && let Some((owned, equipped)) = service.wardrobe().cloned()
            {
                self.from_account(&owned, Loadout::of(&equipped));
            }
        }
        if status == Status::SignedIn
            && service.greeted()
            && self.equipping.is_none()
            && now >= self.next_equip
            && let Some(loadout) = self.unsent()
        {
            self.next_equip = now + EQUIP_INTERVAL_MILLIS;
            let ticket = service.equip(&loadout.ids());
            self.equipping = Some((ticket, loadout));
        }
    }

    // ---------------------------------------------------------------------------------------------
    // The file
    // ---------------------------------------------------------------------------------------------

    /// Whether `load` was called.
    pub fn loaded(&self) -> bool {
        self.loaded
    }

    /// Reads `wardrobe.json`'s text (None: no file). A broken file costs the player their
    /// outfits, not their game.
    pub fn load(&mut self, text: Option<&str>) {
        self.loaded = true;
        let Some(json) = text.and_then(|t| serde_json::from_str::<Value>(t).ok()) else {
            return;
        };
        let strings = |v: &Value, key: &str| -> Vec<String> {
            v.get(key)
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|e| match e {
                            Value::String(s) => Some(s.clone()),
                            Value::Number(n) => Some(n.to_string()),
                            Value::Bool(b) => Some(b.to_string()),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        self.owned = strings(&json, "owned").into_iter().collect();
        self.worn = self.owned_filter(Loadout::of(&strings(&json, "worn")));
        if let Some(list) = json.get("outfits").and_then(Value::as_array) {
            for each in list {
                if self.outfits.len() >= MAX_OUTFITS {
                    break;
                }
                if let Some(name) = each.get("name").and_then(Value::as_str) {
                    self.outfits.push(Outfit { name: name.to_owned(), loadout: Loadout::of(&strings(each, "worn")) });
                }
            }
        }
        if let Some(show) = json.get("showOthers").and_then(Value::as_bool) {
            self.show_others = show;
        }
        self.revision += 1;
    }

    /// The file's text as Java writes it.
    pub fn file_text(&self) -> String {
        let mut owned: Vec<&String> = self.owned.iter().collect();
        owned.sort();
        let outfits: Vec<Value> = self
            .outfits
            .iter()
            .map(|o| json!({"name": o.name, "worn": o.loadout.ids()}))
            .collect();
        json!({"worn": self.worn.ids(), "owned": owned, "outfits": outfits, "showOthers": self.show_others})
            .to_string()
    }

    /// The file's text, when it changed since the last call: the caller writes it to `FILE`.
    pub fn take_unsaved(&mut self) -> Option<String> {
        std::mem::take(&mut self.dirty).then(|| self.file_text())
    }

    fn saved(&mut self) {
        self.revision += 1;
        self.dirty = true;
    }
}

thread_local! {
    // The window's thread only, as the online service.
    static WARDROBE: RefCell<Wardrobe> = RefCell::new(Wardrobe::default());
}

/// Runs `f` with the player's wardrobe (one for the window's thread, as `online::with`).
pub fn with<R>(f: impl FnOnce(&mut Wardrobe) -> R) -> R {
    WARDROBE.with(|w| f(&mut w.borrow_mut()))
}

/// Reads the file through `read` the first time: whoever draws first (a screen with its `Host`,
/// or the game side) loads it.
pub fn ensure_loaded(read: impl FnOnce() -> Option<Vec<u8>>) {
    with(|w| {
        if !w.loaded() {
            let text = read().and_then(|b| String::from_utf8(b).ok());
            w.load(text.as_deref());
        }
    });
}

/// For a screen: the file read once through `host`, and written when it changed.
pub fn load_and_save(host: &mut dyn crate::screen::Host) {
    ensure_loaded(|| host.read(FILE));
    if let Some(text) = with(Wardrobe::take_unsaved) {
        host.write(FILE, text.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::online::Answer;

    fn c(id: &str) -> &'static Cosmetic {
        catalog::by_id(id).unwrap()
    }

    #[test]
    fn the_file_reads_back_as_written() {
        let mut w = Wardrobe::default();
        w.load(None);
        w.wear(c("cape.ember"));
        w.wear(c("hat.halo"));
        assert!(w.save_outfit("  Party  "));
        w.set_show_others(false);
        let text = w.take_unsaved().expect("changed");
        assert!(w.take_unsaved().is_none());
        let mut back = Wardrobe::default();
        back.load(Some(&text));
        assert_eq!(back.worn(), w.worn());
        assert_eq!(back.outfits(), w.outfits());
        assert_eq!(back.outfits()[0].name, "Party");
        assert!(!back.show_others());
        let mut broken = Wardrobe::default();
        broken.load(Some("{not json"));
        assert!(broken.worn().is_empty() && broken.show_others());
    }

    #[test]
    fn outfits_replace_by_name_and_stop_at_twelve() {
        let mut w = Wardrobe::default();
        for i in 0..MAX_OUTFITS {
            assert!(w.save_outfit(&format!("o{i}")));
        }
        assert!(!w.save_outfit("more"));
        w.wear(c("cape.mono"));
        assert!(w.save_outfit("O3"), "same name, any case: replaced");
        assert_eq!(w.outfits()[3].loadout.ids(), ["cape.mono"]);
        w.delete_outfit(0);
        assert!(w.save_outfit(""));
        assert_eq!(w.outfits().last().unwrap().name, "Outfit 12");
        assert!(!w.save_outfit(&"x".repeat(40)), "full again");
        w.delete_outfit(0);
        assert!(w.save_outfit(&"x".repeat(40)));
        assert_eq!(w.outfits().last().unwrap().name.len(), MAX_OUTFIT_NAME);
    }

    #[test]
    fn the_account_wins_unless_it_wears_nothing() {
        let mut w = Wardrobe::default();
        w.wear(c("cape.frost"));
        assert!(w.from_account(&[], Loadout::EMPTY), "the device's carries over");
        assert_eq!(w.unsent(), Some(w.worn()));
        w.sent(w.worn(), w.worn());
        assert_eq!(w.unsent(), None);
        w.signed_out();
        assert!(!w.from_account(&[], Loadout::of(&["hat.crown"])), "a new sign-in: the account's");
        assert_eq!(w.worn().ids(), ["hat.crown"]);
        // A reconnect: a change made while away wins.
        w.disconnected();
        w.wear(c("hat.beanie"));
        assert!(w.from_account(&[], Loadout::of(&["hat.crown"])));
        assert_eq!(w.worn().ids(), ["hat.beanie"]);
    }

    /// A service that answers what the test says.
    #[derive(Default)]
    struct Stub {
        status: Option<Status>,
        name: Option<String>,
        wardrobe: Option<(Vec<String>, Vec<String>)>,
        serial: u64,
        sent: Vec<Vec<String>>,
        results: Vec<(Ticket, Outcome)>,
    }

    impl Service for Stub {
        fn status(&self) -> Status {
            self.status.unwrap_or(Status::SignedOut)
        }
        fn greeted(&self) -> bool {
            true
        }
        fn account_name(&self) -> Option<&str> {
            self.name.as_deref()
        }
        fn wardrobe(&self) -> Option<&(Vec<String>, Vec<String>)> {
            self.wardrobe.as_ref()
        }
        fn wardrobe_serial(&self) -> u64 {
            self.serial
        }
        fn equip(&mut self, ids: &[String]) -> Ticket {
            self.sent.push(ids.to_vec());
            self.sent.len() as Ticket
        }
        fn take_result(&mut self, ticket: Ticket) -> Option<Outcome> {
            let i = self.results.iter().position(|r| r.0 == ticket)?;
            Some(self.results.remove(i).1)
        }
    }

    #[test]
    fn sync_sends_once_a_second_and_follows_sign_outs() {
        let mut w = Wardrobe::default();
        let mut s = Stub { status: Some(Status::SignedIn), name: Some("me".into()), ..Default::default() };
        w.wear(c("cape.tide"));
        s.wardrobe = Some((vec![], vec![]));
        s.serial = 1;
        w.sync(&mut s, 0.0);
        assert!(w.shared());
        assert_eq!(s.sent, [vec!["cape.tide".to_owned()]], "the device's carried over");
        w.wear(c("cape.mono"));
        w.sync(&mut s, 100.0);
        assert_eq!(s.sent.len(), 1, "one at a time");
        s.wardrobe = Some((vec![], vec!["cape.tide".into()]));
        s.results.push((1, Ok(Answer::Done)));
        w.sync(&mut s, 500.0);
        assert_eq!(s.sent.len(), 1, "a second apart");
        w.sync(&mut s, 1_000.0);
        assert_eq!(s.sent.last().unwrap(), &vec!["cape.mono".to_owned()]);
        // Signed out elsewhere: device-only again, what's worn kept.
        s.status = Some(Status::SignedOut);
        s.name = None;
        w.sync(&mut s, 2_000.0);
        assert!(!w.shared());
        assert_eq!(w.worn().ids(), ["cape.mono"]);
    }
}
