// Incari's Waypoints for the Rust build (ADR 0051, 0068, 0069), as features/waypoints's
// WaypointsModule: what follows the world played and the module's keys. The waypoints, their
// file, the death watch and the drawing are the client's (`incari_core::waypoint`,
// `incari_core::waypoint_overlay`); `world.rs` is the game side. No game types: rust/lib.rs tests
// it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "waypoints";

/// What changed about the world played since the last tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// The player joined a world (`server/<address>`, `world/<folder>`).
    Join(String),
    /// The player left it.
    Leave,
}

/// Follows the world played, tick to tick (IncariModules.trackWorld): a dimension change keeps
/// it, another server replaces it.
#[derive(Clone, Debug, Default)]
pub struct WorldTracker {
    world: Option<String>,
}

impl WorldTracker {
    /// The world now (None outside one): what to do about it, at most a leave then a join.
    /// Hot path: copies the name only when it changes.
    pub fn follow(&mut self, now: Option<&str>, out: &mut Vec<Change>) {
        if self.world.as_deref() == now {
            return;
        }
        if self.world.take().is_some() {
            out.push(Change::Leave);
        }
        if let Some(world) = now {
            self.world = Some(world.to_owned());
            out.push(Change::Join(world.to_owned()));
        }
    }
}

/// What a key does, by the module's keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// A new waypoint where the player stands, in the editor.
    New,
    /// The world's waypoints.
    List,
}

/// The action of `key` (a key going down while playing), given the New waypoint and Waypoint list
/// keys; `none` is an unbound key, which binds nothing.
pub fn action<K: PartialEq + Copy>(key: K, none: K, new: K, list: K) -> Option<Action> {
    if key == none {
        None
    } else if key == new {
        Some(Action::New)
    } else if key == list {
        Some(Action::List)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_joins_and_leaves() {
        let mut t = WorldTracker::default();
        let mut out = Vec::new();
        t.follow(None, &mut out);
        assert!(out.is_empty());
        t.follow(Some("server/a"), &mut out);
        assert_eq!(out, [Change::Join("server/a".into())]);
        out.clear();
        t.follow(Some("server/a"), &mut out);
        assert!(out.is_empty());
        t.follow(Some("server/b"), &mut out);
        assert_eq!(out, [Change::Leave, Change::Join("server/b".into())]);
        out.clear();
        t.follow(None, &mut out);
        assert_eq!(out, [Change::Leave]);
    }

    #[test]
    fn binds_the_modules_keys() {
        assert_eq!(action(17, 0, 17, 24), Some(Action::New));
        assert_eq!(action(24, 0, 17, 24), Some(Action::List));
        assert_eq!(action(5, 0, 17, 24), None);
        assert_eq!(action(0, 0, 0, 24), None);
    }
}
