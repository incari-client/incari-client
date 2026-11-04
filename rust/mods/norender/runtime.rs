// Incari's No Render for the Rust build (ADR 0068, 0069), as features/norender does it: items on
// the ground hidden or lying still, a stack as one item, and XP orbs, falling blocks, fireworks and
// the particles you pick hidden, on this client only. Horizon asks once a frame
// (`native_no_render`); hidden particles are never made. Its settings are the module's in
// `incari_core::modules`. No game types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "norender";

/// What the settings hide, flat, as Horizon's `NativeNoRender` takes it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hide {
    pub items_hidden: bool,
    pub items_still: bool,
    pub one_copy: bool,
    pub orbs: bool,
    pub falling_blocks: bool,
    pub fireworks: bool,
    pub growth: bool,
    pub breaking: bool,
    pub explosions: bool,
    pub all_particles: bool,
}

/// The "items" choice ("shown", "still", "hidden") and the "one-copy" setting as Horizon takes
/// them: (hidden, still, one copy). One copy means nothing while the items are hidden.
pub fn items(choice: &str, one_copy: bool) -> (bool, bool, bool) {
    let hidden = choice == "hidden";
    (hidden, choice == "still", one_copy && !hidden)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_follow_the_choice() {
        assert_eq!(items("hidden", true), (true, false, false));
        assert_eq!(items("still", true), (false, true, true));
        assert_eq!(items("shown", false), (false, false, false));
    }

    #[test]
    fn an_unknown_choice_draws_items_as_the_game_does() {
        assert_eq!(items("", false), (false, false, false));
    }
}
