// Incari's Old Textures for the Rust build (ADR 0068, 0069), as features/oldtextures does it: the
// game's Programmer Art pack (pre-1.14 textures) is the module's switch (`follows_pack` in
// `incari_core::modules`). Turning the module on or off turns the pack with it; each change
// reloads resources. No game types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "oldtextures";

/// What changes, from the module's and the pack's state as last seen (`last`, none before the
/// first frame) and now: the module follows the pack at first and when the pack is switched in
/// Resource Packs; the pack follows the module when the module is turned on or off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Pack(bool),
    Module(bool),
}

pub fn change(last: Option<(bool, bool)>, on: bool, pack: bool) -> Option<Change> {
    match last {
        None if on != pack => Some(Change::Module(pack)),
        Some((was, _)) if was != on && on != pack => Some(Change::Pack(on)),
        Some((was, had)) if was == on && had != pack && on != pack => Some(Change::Module(pack)),
        _ => None,
    }
}

// The window's thread only: the frame hook runs there.
thread_local! {
    static LAST: std::cell::Cell<Option<(bool, bool)>> = const { std::cell::Cell::new(None) };
}

/// Once a frame: what to change, if anything. The state seen is what it will be after it.
pub fn follow(on: bool, pack: bool) -> Option<Change> {
    let c = change(LAST.get(), on, pack);
    let after = match c {
        Some(Change::Pack(p)) => (on, p),
        Some(Change::Module(m)) => (m, pack),
        None => (on, pack),
    };
    LAST.set(Some(after));
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_module_follows_the_pack_and_the_pack_the_module() {
        assert_eq!(
            change(None, false, true),
            Some(Change::Module(true)),
            "at first, the pack's state"
        );
        assert_eq!(change(None, true, true), None);
        assert_eq!(
            change(Some((false, false)), true, false),
            Some(Change::Pack(true))
        );
        assert_eq!(
            change(Some((true, true)), false, true),
            Some(Change::Pack(false))
        );
        assert_eq!(
            change(Some((false, false)), false, true),
            Some(Change::Module(true)),
            "switched in Resource Packs"
        );
        assert_eq!(change(Some((true, true)), true, true), None);
    }

    #[test]
    fn follows_from_frame_to_frame() {
        assert_eq!(follow(false, true), Some(Change::Module(true)));
        assert_eq!(follow(true, true), None);
        assert_eq!(follow(false, true), Some(Change::Pack(false)));
        assert_eq!(follow(false, false), None);
    }
}
