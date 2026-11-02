//! The world the player is in, as the service hears of it, and which players in its tab list are
//! Incari players (ADR 0058; WorldPlayers.java). Each name is looked up once as it appears; the
//! service's `seen`, `gone`, `cosmetics` and `badge` events keep the answers current.
use super::rules;
use std::collections::{HashMap, HashSet};

/// At most this many names a lookup (the service's limit).
pub const MAX_LOOKUP: usize = 256;
/// At least this long between lookups: the service allows ten a minute.
pub const LOOKUP_INTERVAL_MILLIS: f64 = 6_500.0;

/// A player's UUID, as the game holds it (most significant bits first).
pub type Uuid = u128;

/// An Incari player in the world (`IncariPlayer`): an id for this server only, whether they're a
/// friend, what they wear, and whether they show Incari's badge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IncariPlayer {
    pub pid: String,
    pub friend: bool,
    pub cosmetics: Vec<String>,
    pub badge: bool,
}

/// Where the player is: the server's hash, their name there, and the plain address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub server_hash: String,
    pub name: String,
    pub address: String,
}

#[derive(Default)]
pub struct WorldPlayers {
    place: Option<Place>,
    /// The UUIDs under each name (an NPC often takes a player's), in the order they came.
    ids: HashMap<String, Vec<Uuid>>,
    names: HashMap<Uuid, String>,
    /// The Incari players by name; each one's name by pid.
    players: HashMap<String, IncariPlayer>,
    names_by_pid: HashMap<String, String>,
    /// Names in the tab list not asked about yet, in order.
    unasked: Vec<String>,
    unasked_set: HashSet<String>,
    next_lookup: f64,
    revision: u64,
}

fn version(id: Uuid) -> u32 {
    ((id >> 76) & 0xF) as u32
}

impl WorldPlayers {
    /// Joined a server through `address` as `name`; forgets the last world.
    pub fn join(&mut self, address: &str, name: &str) {
        self.clear();
        self.place = Some(Place { server_hash: rules::server_hash(address), name: name.into(), address: address.into() });
        self.revision += 1;
    }

    /// Left the world (or is in singleplayer, which the service never hears of).
    pub fn leave(&mut self) {
        self.clear();
        self.place = None;
        self.revision += 1;
    }

    fn clear(&mut self) {
        self.ids.clear();
        self.names.clear();
        self.players.clear();
        self.names_by_pid.clear();
        self.unasked.clear();
        self.unasked_set.clear();
        self.next_lookup = 0.0;
    }

    pub fn place(&self) -> Option<&Place> {
        self.place.as_ref()
    }

    fn ask(&mut self, name: &str) {
        if self.unasked_set.insert(name.to_owned()) {
            self.unasked.push(name.to_owned());
        }
    }

    fn unask(&mut self, name: &str) {
        if self.unasked_set.remove(name) {
            self.unasked.retain(|n| n != name);
        }
    }

    fn own_name(&self, name: &str) -> bool {
        self.place.as_ref().is_some_and(|p| p.name == name)
    }

    /// The tab list now: new names are queued for a lookup, players who left forgotten. Cheap when
    /// nothing changed.
    pub fn tab_list(&mut self, now: &HashMap<Uuid, String>) {
        if self.place.is_none() || *now == self.names {
            return;
        }
        let old: Vec<(Uuid, String)> = self.names.iter().map(|(k, v)| (*k, v.clone())).collect();
        for (id, name) in old {
            if now.get(&id) == Some(&name) {
                continue;
            }
            self.names.remove(&id);
            let owner = self.uuid_of(&name);
            let remaining = self.ids.get_mut(&name).map(|same| {
                same.retain(|u| *u != id);
                same.len()
            });
            if remaining.is_some_and(|n| n > 0) {
                // Someone else by that name is still here: an NPC leaving takes nobody's badge. If
                // the account's player left, look the name up again rather than give the account
                // to whoever is left.
                if owner == Some(id) {
                    if let Some(gone) = self.players.remove(&name) {
                        self.names_by_pid.remove(&gone.pid);
                    }
                    if !self.own_name(&name) {
                        self.ask(&name);
                    }
                }
                self.revision += 1;
                continue;
            }
            self.ids.remove(&name);
            self.unask(&name);
            if let Some(gone) = self.players.remove(&name) {
                self.names_by_pid.remove(&gone.pid);
                self.revision += 1;
            }
        }
        for (id, name) in now {
            if self.names.contains_key(id) {
                continue;
            }
            self.names.insert(*id, name.clone());
            let same = self.ids.entry(name.clone()).or_default();
            same.push(*id);
            let count = same.len();
            if count == 1 && !self.own_name(name) {
                self.ask(name);
            } else if count > 1 {
                // Which of them the account is may change.
                self.revision += 1;
            }
        }
    }

    /// The UUID an account under `name` goes with: of several, the first that looks like a
    /// player's (version 4 online mode, 3 offline), since copying NPCs use others; else the first.
    fn uuid_of(&self, name: &str) -> Option<Uuid> {
        let same = self.ids.get(name)?;
        same.iter().copied().find(|u| matches!(version(*u), 3 | 4)).or_else(|| same.first().copied())
    }

    /// The names to look up now (at most `MAX_LOOKUP`), empty if none or too soon. Taken: if the
    /// lookup fails, `ask_again`.
    pub fn take_lookup(&mut self, now: f64) -> Vec<String> {
        if self.place.is_none() || self.unasked.is_empty() || now < self.next_lookup {
            return Vec::new();
        }
        self.next_lookup = now + LOOKUP_INTERVAL_MILLIS;
        let n = self.unasked.len().min(MAX_LOOKUP);
        let batch: Vec<String> = self.unasked.drain(..n).collect();
        for name in &batch {
            self.unasked_set.remove(name);
        }
        batch
    }

    /// A lookup that didn't get through: names still in the tab list are asked again later.
    pub fn ask_again(&mut self, batch: &[String]) {
        for name in batch {
            if self.ids.contains_key(name) && !self.players.contains_key(name) {
                self.ask(name);
            }
        }
    }

    /// Every name is asked about again: after a reconnect or sign-in, the service forgot them.
    pub fn ask_all(&mut self) {
        if self.place.is_none() {
            return;
        }
        let mut names: Vec<String> = self.ids.keys().filter(|n| !self.own_name(n)).cloned().collect();
        names.sort();
        for name in names {
            self.ask(&name);
        }
        if !self.players.is_empty() {
            self.players.clear();
            self.names_by_pid.clear();
            self.revision += 1;
        }
        self.next_lookup = 0.0;
    }

    /// `name` is an Incari player here; ignored for a name no longer in the tab list.
    pub fn seen(&mut self, name: &str, player: IncariPlayer) {
        if !self.ids.contains_key(name) {
            return;
        }
        let pid = player.pid.clone();
        if let Some(old) = self.players.insert(name.to_owned(), player) {
            self.names_by_pid.remove(&old.pid);
        }
        self.names_by_pid.insert(pid, name.to_owned());
        self.revision += 1;
    }

    /// The account with this pid left, signed out, or blocked the player.
    pub fn gone(&mut self, pid: &str) {
        if let Some(name) = self.names_by_pid.remove(pid) {
            self.players.remove(&name);
            self.revision += 1;
        }
    }

    /// The account with this pid wears other cosmetics now.
    pub fn cosmetics(&mut self, pid: &str, cosmetics: Vec<String>) {
        if let Some(name) = self.names_by_pid.get(pid)
            && let Some(p) = self.players.get_mut(name)
        {
            p.cosmetics = cosmetics;
            self.revision += 1;
        }
    }

    /// An Incari player here showed or hid the badge.
    pub fn badge(&mut self, pid: &str, badge: bool) {
        if let Some(name) = self.names_by_pid.get(pid)
            && let Some(p) = self.players.get_mut(name)
            && p.badge != badge
        {
            p.badge = badge;
            self.revision += 1;
        }
    }

    /// The UUID of the player whose account has this pid.
    pub fn uuid(&self, pid: &str) -> Option<Uuid> {
        self.uuid_of(self.names_by_pid.get(pid)?)
    }

    /// The Incari player with this UUID; of several players under one name only the account's.
    pub fn player(&self, id: Uuid) -> Option<&IncariPlayer> {
        let name = self.names.get(&id)?;
        if self.uuid_of(name) != Some(id) {
            return None;
        }
        self.players.get(name)
    }

    /// The Incari players in the world by name.
    pub fn players(&self) -> &HashMap<String, IncariPlayer> {
        &self.players
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const V4: Uuid = 4 << 76;
    const V2: Uuid = 2 << 76;

    fn player(pid: &str) -> IncariPlayer {
        IncariPlayer { pid: pid.into(), friend: false, cosmetics: vec![], badge: true }
    }

    #[test]
    fn names_are_looked_up_once_and_not_the_players_own() {
        let mut w = WorldPlayers::default();
        w.join("play.example.com", "Me");
        let tab: HashMap<Uuid, String> = [(V4 | 1, "Me".to_owned()), (V4 | 2, "Steve".to_owned())].into();
        w.tab_list(&tab);
        assert_eq!(w.take_lookup(0.0), ["Steve"]);
        assert!(w.take_lookup(100.0).is_empty());
        w.seen("Steve", player("p1"));
        assert_eq!(w.player(V4 | 2).unwrap().pid, "p1");
        w.gone("p1");
        assert!(w.player(V4 | 2).is_none());
    }

    #[test]
    fn an_npc_by_the_same_name_has_no_badge() {
        let mut w = WorldPlayers::default();
        w.join("a", "Me");
        let tab: HashMap<Uuid, String> = [(V2 | 9, "Steve".to_owned()), (V4 | 2, "Steve".to_owned())].into();
        w.tab_list(&tab);
        w.seen("Steve", player("p1"));
        assert!(w.player(V2 | 9).is_none());
        assert!(w.player(V4 | 2).is_some());
        // The player leaves, the NPC stays: asked about again rather than given the account.
        let tab: HashMap<Uuid, String> = [(V2 | 9, "Steve".to_owned())].into();
        w.tab_list(&tab);
        assert!(w.players().is_empty());
        assert_eq!(w.take_lookup(0.0), ["Steve"]);
    }

    #[test]
    fn answers_after_leaving_are_ignored() {
        let mut w = WorldPlayers::default();
        w.join("a", "Me");
        w.seen("Ghost", player("p"));
        assert!(w.players().is_empty());
    }
}
