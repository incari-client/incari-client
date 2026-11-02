//! The emote wheel (ADR 0061; `EmoteWheelScreen.java`): held open by its key (G), the player's
//! emotes round a circle; the one the pointer points toward plays when the key is let go, or when
//! it's clicked. Escape, or letting go with the pointer in the middle, plays nothing. A quick tap
//! of the key, let go before the wheel showed, leaves it open to click an emote; the key again
//! closes it. The game goes on behind it.
//!
//! The game side opens it on the key's press (`open`), tells it the key's state (`key_held`), and
//! plays what it chose (`cosmetics::world::World::take_own`).
use crate::canvas::{Image, Key};
use crate::cosmetics::catalog::Cosmetic;
use crate::cosmetics::{wardrobe, world};
use crate::online;
use crate::screen::{Cx, Go, Screen};
use std::cell::Cell;

const RADIUS: f32 = 78.0;
const INNER: f32 = 24.0;
const LABEL_RADIUS: f32 = 54.0;

/// The wheel's key: G (the Java client's default; HID 10).
pub const KEY: u16 = 10;
/// What the hints call it.
pub const KEY_LABEL: &str = "G";
/// Its name as the game's key events give it.
pub const KEY_NAME: &str = "key.keyboard.g";

/// Canvas picture ids of the wheel's circles.
const CIRCLE: u32 = 0xC05_3000;

thread_local! {
    /// Whether the wheel's key is held, as the game side last heard.
    static HELD: Cell<bool> = const { Cell::new(false) };
}

/// The game side heard the wheel's key go down or up.
pub fn key_held(held: bool) {
    HELD.with(|h| h.set(held));
}

pub struct EmoteWheel {
    emotes: Vec<&'static Cosmetic>,
    hovered: Option<usize>,
    /// Whether the key was let go before the wheel showed: its release is gone, so the wheel
    /// waits for a click instead. Known at the first frame.
    tapped: Option<bool>,
}

impl Default for EmoteWheel {
    fn default() -> Self {
        Self::new()
    }
}

impl EmoteWheel {
    /// The wheel with the player's own emotes.
    pub fn new() -> Self {
        Self { emotes: wardrobe::with(|w| w.emotes()), hovered: None, tapped: None }
    }

    /// The emote under the pointer, for tests.
    pub fn hovered(&self) -> Option<&'static Cosmetic> {
        self.emotes.get(self.hovered?).copied()
    }

    fn play(&mut self, cx: &mut Cx, index: Option<usize>) {
        cx.go(Go::Back);
        if let Some(emote) = index.and_then(|i| self.emotes.get(i)).copied() {
            // One every two seconds; Incari players on the server see it too.
            if online::with(|o| o.play_emote(emote.id)) {
                world::with(|w| w.play_own_later(emote));
            }
        }
    }
}

/// The angle of emote `i` of `count`, clockwise from straight up.
fn angle(i: usize, count: usize) -> f32 {
    std::f32::consts::TAU * i as f32 / count.max(1) as f32
}

/// The emote at `at` on a wheel centred on `centre` with `count` emotes; None in the middle.
pub fn pointed_at(at: [f32; 2], centre: [f32; 2], count: usize) -> Option<usize> {
    let (dx, dy) = (at[0] - centre[0], at[1] - centre[1]);
    if count == 0 || dx * dx + dy * dy < INNER * INNER {
        return None;
    }
    let mut a = dx.atan2(-dy);
    if a < 0.0 {
        a += std::f32::consts::TAU;
    }
    let slice = std::f32::consts::TAU / count as f32;
    Some((a / slice).round() as usize % count)
}

/// A circle `radius` across from its middle and `width` thick (a disc when as thick as wide),
/// painted at the size it shows at.
fn circle(cx: &mut Cx, centre: [f32; 2], radius: f32, width: f32, argb: u32, id: u32) {
    let p = &mut *cx.p;
    let [x, y, size, _] = p.place(centre[0] - radius, centre[1] - radius, radius * 2.0, 0.0);
    let pixels = ((size * p.scale).round() as u32).clamp(2, 1000);
    let inner = (1.0 - width / radius).clamp(0.0, 1.0);
    let Some(r) = p.canvas.get(Key::Picture(CIRCLE + id, pixels as u16), || {
        Image::mask(pixels, pixels, |px, py| {
            let c = pixels as f32 / 2.0;
            let mut inside = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    let dx = (px as f32 + (sx as f32 + 0.5) / 4.0 - c) / c;
                    let dy = (py as f32 + (sy as f32 + 0.5) / 4.0 - c) / c;
                    let d = (dx * dx + dy * dy).sqrt();
                    if d <= 1.0 && d >= inner {
                        inside += 1;
                    }
                }
            }
            inside as f32 / 16.0
        })
    }) else {
        return;
    };
    let argb = p.faded(argb);
    let s = pixels as f32 / p.scale;
    let (gx, gy) = ((x * p.scale).round() / p.scale, (y * p.scale).round() / p.scale);
    p.list.image(r.src(), [gx, gy, s, s], argb);
}

impl Screen for EmoteWheel {
    fn draw(&mut self, cx: &mut Cx) {
        let held = HELD.with(Cell::get);
        let tapped = *self.tapped.get_or_insert(!held);
        let th = cx.p.theme;
        let centre = [cx.size[0] / 2.0, cx.size[1] / 2.0];
        let count = self.emotes.len();
        self.hovered = pointed_at(cx.ui.mouse, centre, count);
        if cx.ui.pressed_anywhere().is_some() {
            cx.ui.eat_press();
            let at = pointed_at(cx.ui.mouse, centre, count);
            self.play(cx, at);
            return;
        }
        if cx.ui.key(KEY) {
            cx.go(Go::Back);
            return;
        }
        if !tapped && !held {
            let at = self.hovered;
            self.play(cx, at);
            return;
        }
        let disc = if th.normal() { 0xB010_1822 } else { th.glass_solid };
        circle(cx, centre, RADIUS, RADIUS, disc, 0);
        circle(cx, centre, RADIUS, 1.0, th.glass_edge, 1);
        circle(cx, centre, INNER, 1.0, th.glass_edge, 2);
        for (i, emote) in self.emotes.iter().enumerate() {
            let a = angle(i, count);
            let x = (centre[0] + a.sin() * LABEL_RADIUS).round();
            let y = (centre[1] - a.cos() * LABEL_RADIUS).round();
            let w = cx.p.width(emote.name);
            let on = Some(i) == self.hovered;
            if on {
                cx.p.round((x - w / 2.0).floor() - 6.0, y - 8.0, w + 12.0, 16.0, 8.0, th.accent);
            }
            cx.p.text(emote.name, (x - w / 2.0).floor(), y - 4.0, if on { th.on_accent } else { th.text }, false);
        }
        let (middle, color) = match self.hovered() {
            Some(e) => (e.name, th.accent),
            None => (if count == 0 { "No emotes" } else { "Emotes" }, th.muted),
        };
        cx.p.centered(middle, centre[0], centre[1] - 4.0, color, false);
        let hint = if tapped {
            format!("Click an emote to play it, or press {KEY_LABEL} to close")
        } else {
            format!("Release {KEY_LABEL} or click to play")
        };
        cx.p.centered(&hint, centre[0], centre[1] + RADIUS + 10.0, th.muted, false);
    }

    fn backdrop(&self) -> bool {
        // The world stays as it is behind the wheel: the player is in the middle of a game.
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pointer_picks_the_emote_it_points_toward() {
        let c = [100.0, 100.0];
        assert_eq!(pointed_at([100.0, 50.0], c, 8), Some(0), "straight up is the first");
        assert_eq!(pointed_at([150.0, 100.0], c, 8), Some(2), "clockwise");
        assert_eq!(pointed_at([100.0, 150.0], c, 8), Some(4));
        assert_eq!(pointed_at([105.0, 105.0], c, 8), None, "the middle plays nothing");
        assert_eq!(pointed_at([100.0, 50.0], c, 0), None);
        assert_eq!(pointed_at([99.0, 40.0], c, 8), Some(0), "just left of up rounds to up");
    }
}
