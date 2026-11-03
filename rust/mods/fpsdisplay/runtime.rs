// Incari's FPS Display for the Rust build (ADR 0068, 0069), as features/fpsdisplay does it: frames
// drawn in the last second, "120 FPS", green from 60, amber from 30, red below. Its settings and HUD
// element are the module's in `incari_core::modules` (the Java ids), shown in the module menu and
// placed in the HUD editor. No game types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "fpsdisplay";
/// Its HUD element's id.
pub const ELEMENT: &str = "fps";

/// "120 FPS" in `format` (the `format` setting's key).
pub fn text(format: &str, fps: u32) -> String {
    match format {
        "before" => format!("FPS: {fps}"),
        "number" => fps.to_string(),
        _ => format!("{fps} FPS"),
    }
}

/// The widest it says, so its width holds still as the count changes.
pub fn widest(format: &str) -> &'static str {
    match format {
        "before" => "FPS: 000",
        "number" => "000",
        _ => "000 FPS",
    }
}

/// The text's tone (`incari_core::theme::Tone`'s of the same names).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Text,
    Positive,
    Warning,
    Negative,
}

/// The theme's text, or by speed when `by_speed`.
pub fn tone(by_speed: bool, fps: u32) -> Tone {
    match fps {
        _ if !by_speed => Tone::Text,
        60.. => Tone::Positive,
        30.. => Tone::Warning,
        _ => Tone::Negative,
    }
}

type CachedText = (Option<(&'static str, u32)>, String);

// Hot path, don't allocate: made again only when the count changes, about once a second.
thread_local! {
    static TEXT: std::cell::RefCell<CachedText> = const { std::cell::RefCell::new((None, String::new())) };
}

/// Calls `f` with `text(format, fps)`, made again only when one of them changed.
pub fn with_text<R>(format: &'static str, fps: u32, f: impl FnOnce(&str) -> R) -> R {
    TEXT.with(|cell| {
        let mut cached = cell.borrow_mut();
        if cached.0 != Some((format, fps)) {
            *cached = (Some((format, fps)), text(format, fps));
        }
        f(&cached.1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_what_the_java_module_says() {
        assert_eq!(text("after", 120), "120 FPS");
        assert_eq!(text("before", 120), "FPS: 120");
        assert_eq!(text("number", 120), "120");
        assert_eq!(widest("before"), "FPS: 000");
        assert_eq!(with_text("after", 7, str::to_owned), "7 FPS");
    }

    #[test]
    fn colours_by_speed_only_when_asked() {
        assert_eq!(tone(false, 5), Tone::Text);
        assert_eq!(
            [
                tone(true, 60),
                tone(true, 59),
                tone(true, 30),
                tone(true, 29)
            ],
            [Tone::Positive, Tone::Warning, Tone::Warning, Tone::Negative]
        );
    }
}
