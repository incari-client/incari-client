// Incari's CPS for the Rust build (ADR 0068, 0069), as features/cps does it: clicks in the last
// second, left, right or both, "12 | 4 CPS". Its settings and HUD element are the module's in
// `incari_core::modules` (the Java ids), shown in the module menu and placed in the HUD editor.
// This file is both the module the bundle adds to the game and, through rust/lib.rs, the code its
// tests run: it uses no game types.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "cps";

/// "12 | 4 CPS" for `buttons` (the `buttons` setting's key).
pub fn text(buttons: &str, left: u32, right: u32) -> String {
    match buttons {
        "left" => format!("{left} CPS"),
        "right" => format!("{right} CPS"),
        _ => format!("{left} | {right} CPS"),
    }
}

/// The widest it says, so its width holds still as the counts change.
pub fn widest(buttons: &str) -> &'static str {
    if buttons == "both" {
        "00 | 00 CPS"
    } else {
        "00 CPS"
    }
}

/// The last text made, and what it was made for.
type CachedText = (Option<(&'static str, u32, u32)>, String);

// Hot path, don't allocate: the text is made again only when a count changes.
thread_local! {
    static TEXT: std::cell::RefCell<CachedText> = const { std::cell::RefCell::new((None, String::new())) };
}

/// Calls `f` with `text(buttons, left, right)`, made again only when one of them changed.
pub fn with_text<R>(buttons: &'static str, left: u32, right: u32, f: impl FnOnce(&str) -> R) -> R {
    TEXT.with(|cell| {
        let mut cached = cell.borrow_mut();
        if cached.0 != Some((buttons, left, right)) {
            *cached = (Some((buttons, left, right)), text(buttons, left, right));
        }
        f(&cached.1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_what_the_java_module_says() {
        assert_eq!(text("both", 12, 4), "12 | 4 CPS");
        assert_eq!(text("left", 12, 4), "12 CPS");
        assert_eq!(text("right", 12, 4), "4 CPS");
        assert_eq!(widest("left"), "00 CPS");
    }

    #[test]
    fn makes_the_text_again_only_when_a_count_changes() {
        let first = with_text("both", 3, 1, |t| (t.to_owned(), t.as_ptr()));
        assert_eq!(first.0, "3 | 1 CPS");
        assert_eq!(with_text("both", 3, 1, |t| t.as_ptr()), first.1);
        assert_eq!(with_text("left", 3, 1, str::to_owned), "3 CPS");
    }
}
