// Incari's Custom Scoreboard for the Rust build (ADR 0068, 0069), as features/customscoreboard does
// it: the server's scoreboard sidebar wherever the player puts it in the HUD editor, at the scale
// they choose. Horizon still draws the sidebar; its `sidebar_place` hook moves and scales it to
// the module's `sidebar` element, which the `hud` hook places by the sidebar's size. No game
// types: rust/lib.rs tests it.
use std::cell::Cell;

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "customscoreboard";
/// Its HUD element, named after the part of the game's HUD it takes over.
pub const ELEMENT: &str = "sidebar";

/// The editor's sample while the server sends no sidebar: its size (Java's `GameParts.measure`),
/// title and lines.
pub const SAMPLE: [f32; 2] = [80.0, 36.0];
pub const SAMPLE_TITLE: &str = "Scoreboard";
pub const SAMPLE_LINES: [(&str, &str); 3] = [("Kills", "12"), ("Deaths", "3"), ("Coins", "250")];

/// The offset that puts a sidebar the game would draw at `bounds` (`[x, y, w, h]`), scaled `s`
/// about its top right as Horizon scales it, with its top left at `at`.
pub fn offset(bounds: [f32; 4], at: [f32; 2], s: f32) -> [f32; 2] {
    let [x, y, w, _] = bounds;
    [at[0] - (x + w - w * s), at[1] - y]
}

/// The sidebar as Horizon last placed it, and where the element wants it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Sidebar {
    /// The game's own place for it, as last drawn.
    bounds: Option<[f32; 4]>,
    /// Drawn since the HUD last asked.
    drawn: bool,
    /// The element's top left and scale, this frame; none while the module is off.
    target: Option<([f32; 2], f32)>,
}

// The window's thread only: the HUD and the sidebar are drawn there.
thread_local! {
    static SIDEBAR: Cell<Sidebar> = const {
        Cell::new(Sidebar { bounds: None, drawn: false, target: None })
    };
}

fn with<R>(f: impl FnOnce(&mut Sidebar) -> R) -> R {
    SIDEBAR.with(|c| {
        let mut s = c.get();
        let r = f(&mut s);
        c.set(s);
        r
    })
}

/// The game is drawing a sidebar at `bounds`: the offset and scale that move it to the element,
/// or none to leave it be.
pub fn drawing(bounds: [f32; 4]) -> Option<([f32; 2], f32)> {
    with(|s| {
        s.bounds = Some(bounds);
        s.drawn = true;
        s.target.map(|(at, scale)| (offset(bounds, at, scale), scale))
    })
}

/// Once a HUD frame, before the element draws: the sidebar's unscaled size when one was drawn
/// since the last frame. Forgets the last target, so the sidebar goes back to the game's place if
/// the element doesn't draw.
pub fn frame() -> Option<[f32; 2]> {
    with(|s| {
        s.target = None;
        let drawn = std::mem::take(&mut s.drawn);
        s.bounds.filter(|_| drawn).map(|b| [b[2], b[3]])
    })
}

/// Where the element is this frame: its top left and scale.
pub fn place(at: [f32; 2], scale: f32) {
    with(|s| s.target = Some((at, scale)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_the_scaled_box_to_the_element() {
        let b = [250.0, 100.0, 69.0, 40.0];
        assert_eq!(offset(b, [250.0, 100.0], 1.0), [0.0, 0.0]);
        assert_eq!(offset(b, [2.0, 2.0], 1.0), [-248.0, -98.0]);
        // Twice the size about its top right: its left edge is 69 further left.
        assert_eq!(offset(b, [181.0, 100.0], 2.0), [0.0, 0.0]);
    }

    #[test]
    fn follows_the_element_only_while_it_draws() {
        assert_eq!(frame(), None);
        assert_eq!(drawing([250.0, 100.0, 69.0, 40.0]), None, "nothing placed yet");
        assert_eq!(frame(), Some([69.0, 40.0]));
        place([2.0, 2.0], 1.0);
        assert_eq!(drawing([250.0, 100.0, 69.0, 40.0]), Some(([-248.0, -98.0], 1.0)));
        assert_eq!(frame(), Some([69.0, 40.0]));
        assert_eq!(frame(), None, "no sidebar since");
        assert_eq!(drawing([250.0, 100.0, 69.0, 40.0]), None, "the element didn't draw");
    }
}
