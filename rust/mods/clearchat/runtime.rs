// Incari's Clear Chat for the Rust build (ADR 0068, 0069), as features/clearchat does it: no dark
// box behind chat. The Java module holds the game's Text Background Opacity at 0%; here the box is
// cleared each frame and options.txt is untouched, so the player's own value is back once it's
// off. Its toggle is the module's in `incari_core::modules`. No game types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "clearchat";

/// The chat box's opacity while on; off gives it back to the game's option.
pub fn opacity(on: bool) -> Option<f64> {
    on.then_some(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clears_only_while_on() {
        assert_eq!(opacity(true), Some(0.0));
        assert_eq!(opacity(false), None);
    }
}
