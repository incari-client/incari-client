// Incari's Reach Display for the Rust build (ADR 0068, 0069), as features/reachdisplay does it: how
// far your last hit reached, from your eyes to where the crosshair met the target, "3.04 blocks",
// for a few seconds after it. Its settings and HUD element are the module's in
// `incari_core::modules` (the Java ids). No game types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "reachdisplay";
/// Its HUD element's id.
pub const ELEMENT: &str = "reach";

/// Survival's reach against entities: hits beyond it are worth a second look. The HUD editor's
/// sample too.
pub const SURVIVAL_REACH: f64 = 3.0;

/// "3.04 blocks", with `decimals` places (1 to 3), rounded half up as the Java module does.
pub fn text(reach: f64, decimals: i32) -> String {
    let (scale, places) = match decimals {
        ..=1 => (10, 1),
        2 => (100, 2),
        _ => (1000, 3),
    };
    let whole = (reach * scale as f64 + 0.5).floor() as i64;
    format!("{}.{:0places$} blocks", whole / scale, whole % scale)
}

// The window's thread only: the attack, the frame and the HUD all run there. The last hit's reach
// and its age in seconds.
thread_local! {
    static LAST: std::cell::Cell<Option<(f64, f32)>> = const { std::cell::Cell::new(None) };
    static TEXT: std::cell::RefCell<(Option<(u64, i32)>, String)> =
        const { std::cell::RefCell::new((None, String::new())) };
}

/// A hit `distance` blocks from the eye.
pub fn hit(distance: f64) {
    LAST.with(|l| l.set(Some((distance, 0.0))));
}

/// `seconds` went by.
pub fn advance(seconds: f32) {
    LAST.with(|l| l.set(l.get().map(|(d, age)| (d, age + seconds))));
}

/// The module went off: the last hit is forgotten, as the Java module's `onDisable`.
pub fn clear() {
    LAST.with(|l| l.set(None));
}

/// The last hit's reach while it still shows, for `hide_after` seconds (0 for until the next).
pub fn showing(hide_after: i32) -> Option<f64> {
    let (reach, age) = LAST.with(std::cell::Cell::get)?;
    (hide_after <= 0 || age < hide_after as f32).then_some(reach)
}

/// Amber for a long hit, when marked.
pub fn far(reach: Option<f64>, mark: bool) -> bool {
    mark && reach.is_some_and(|r| r > SURVIVAL_REACH)
}

/// Calls `f` with `text(reach, decimals)`, made again only when one of them changed. Hot path,
/// don't allocate.
pub fn with_text<R>(reach: f64, decimals: i32, f: impl FnOnce(&str) -> R) -> R {
    TEXT.with(|cell| {
        let mut cached = cell.borrow_mut();
        let key = (reach.to_bits(), decimals);
        if cached.0 != Some(key) {
            *cached = (Some(key), text(reach, decimals));
        }
        f(&cached.1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_what_the_java_module_says() {
        assert_eq!(text(3.04, 2), "3.04 blocks");
        assert_eq!(text(2.5, 1), "2.5 blocks");
        assert_eq!(text(2.0, 3), "2.000 blocks");
        assert_eq!(text(3.125, 2), "3.13 blocks");
        assert_eq!(text(0.05, 2), "0.05 blocks");
    }

    #[test]
    fn keeps_the_last_hit_ages_it_and_hides_it() {
        hit(3.2);
        advance(0.5);
        advance(0.25);
        assert_eq!(showing(3), Some(3.2));
        advance(2.25);
        assert_eq!(showing(3), None);
        assert_eq!(showing(0), Some(3.2));
        assert!(far(showing(0), true));
        assert!(!far(showing(0), false));
        assert!(!far(None, true));
        assert_eq!(with_text(3.2, 2, str::to_owned), "3.20 blocks");
        clear();
        assert_eq!(showing(0), None);
    }
}
