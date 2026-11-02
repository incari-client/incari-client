//! What Incari draws in a frame, as a list the game side replays through Horizon's `native_*`
//! helpers: fills, parts of the canvas (shapes, icons, the backdrop), the game's text, items and
//! clipping. Kept between frames and cleared, so a frame allocates nothing once warm.
use std::ops::Range;

/// `[x, y, w, h]` in GUI units.
pub type Rect = [f32; 4];

/// Whether `p` is inside `r`.
pub fn contains(r: Rect, p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[1] >= r[1] && p[0] < r[0] + r[2] && p[1] < r[1] + r[3]
}

/// `r` with `by` taken off each side.
pub fn inset(r: Rect, by: f32) -> Rect {
    [r[0] + by, r[1] + by, (r[2] - 2.0 * by).max(0.0), (r[3] - 2.0 * by).max(0.0)]
}

/// The part of `a` inside `b`.
pub fn intersect(a: Rect, b: Rect) -> Rect {
    let x0 = a[0].max(b[0]);
    let y0 = a[1].max(b[1]);
    let x1 = (a[0] + a[2]).min(b[0] + b[2]);
    let y1 = (a[1] + a[3]).min(b[1] + b[3]);
    [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)]
}

/// One thing to draw.
#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    /// A plain rectangle.
    Fill { rect: Rect, argb: u32 },
    /// Part `src` (canvas pixels) of the canvas over `dst`, multiplied by `argb`.
    Image { src: Rect, dst: Rect, argb: u32 },
    /// Part `src` (canvas pixels) of the canvas over any four-cornered shape (`corners` match
    /// `src`'s top left, top right, bottom right, bottom left).
    Quad { src: Rect, corners: [[f32; 2]; 4], argb: u32 },
    /// A sprite of the game's atlas by name (`gui/sprites/...`), part `src` of it over `dst`.
    Sprite { name: Range<u32>, src: Rect, dst: Rect, argb: u32 },
    /// A line of the game's text, `scale` times its size from its top left.
    Text { text: Range<u32>, at: [f32; 2], argb: u32, shadow: bool, scale: f32 },
    /// An item's icon, `size` square.
    Item { id: Range<u32>, at: [f32; 2], size: f32 },
    /// Only what's drawn until the matching `ClipEnd` and inside `rect` shows.
    ClipStart { rect: Rect },
    ClipEnd,
}

/// A frame's commands and the text they name.
#[derive(Default, Debug)]
pub struct DrawList {
    pub cmds: Vec<Cmd>,
    pub text: String,
    clips: Vec<Rect>,
}

impl DrawList {
    pub fn clear(&mut self) {
        self.cmds.clear();
        self.text.clear();
        self.clips.clear();
    }

    /// The text of a command's range.
    pub fn str(&self, range: &Range<u32>) -> &str {
        &self.text[range.start as usize..range.end as usize]
    }

    fn intern(&mut self, s: &str) -> Range<u32> {
        let start = self.text.len() as u32;
        self.text.push_str(s);
        start..self.text.len() as u32
    }

    /// The clip in effect, if any.
    pub fn clip(&self) -> Option<Rect> {
        self.clips.last().copied()
    }

    pub fn fill(&mut self, rect: Rect, argb: u32) {
        if argb >> 24 == 0 || rect[2] <= 0.0 || rect[3] <= 0.0 {
            return;
        }
        self.cmds.push(Cmd::Fill { rect, argb });
    }

    pub fn image(&mut self, src: Rect, dst: Rect, argb: u32) {
        if argb >> 24 == 0 || dst[2] <= 0.0 || dst[3] <= 0.0 {
            return;
        }
        self.cmds.push(Cmd::Image { src, dst, argb });
    }

    pub fn quad(&mut self, src: Rect, corners: [[f32; 2]; 4], argb: u32) {
        if argb >> 24 != 0 {
            self.cmds.push(Cmd::Quad { src, corners, argb });
        }
    }

    pub fn sprite(&mut self, name: &str, src: Rect, dst: Rect, argb: u32) {
        if argb >> 24 == 0 {
            return;
        }
        let name = self.intern(name);
        self.cmds.push(Cmd::Sprite { name, src, dst, argb });
    }

    pub fn text(&mut self, text: &str, at: [f32; 2], argb: u32, shadow: bool, scale: f32) {
        if text.is_empty() || argb >> 24 < 4 {
            return;
        }
        let text = self.intern(text);
        self.cmds.push(Cmd::Text {
            text,
            at,
            argb,
            shadow,
            scale,
        });
    }

    pub fn item(&mut self, id: &str, at: [f32; 2], size: f32) {
        let id = self.intern(id);
        self.cmds.push(Cmd::Item { id, at, size });
    }

    /// Moves what was drawn since command `start` by (`dx`, `dy`): an element drawn at its own
    /// origin, put where it goes once its size is known.
    pub fn translate_since(&mut self, start: usize, dx: f32, dy: f32) {
        for cmd in &mut self.cmds[start..] {
            match cmd {
                Cmd::Fill { rect, .. } | Cmd::ClipStart { rect } => {
                    rect[0] += dx;
                    rect[1] += dy;
                }
                Cmd::Image { dst, .. } | Cmd::Sprite { dst, .. } => {
                    dst[0] += dx;
                    dst[1] += dy;
                }
                Cmd::Text { at, .. } | Cmd::Item { at, .. } => {
                    at[0] += dx;
                    at[1] += dy;
                }
                Cmd::Quad { corners, .. } => {
                    for c in corners {
                        c[0] += dx;
                        c[1] += dy;
                    }
                }
                Cmd::ClipEnd => {}
            }
        }
    }

    /// Clips what follows to `rect` (inside any clip already in effect) until `clip_end`.
    pub fn clip_start(&mut self, rect: Rect) {
        let rect = match self.clip() {
            Some(outer) => intersect(rect, outer),
            None => rect,
        };
        self.clips.push(rect);
        self.cmds.push(Cmd::ClipStart { rect });
    }

    pub fn clip_end(&mut self) {
        if self.clips.pop().is_some() {
            self.cmds.push(Cmd::ClipEnd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_text_in_one_buffer_and_skips_what_wouldnt_show() {
        let mut list = DrawList::default();
        list.text("Singleplayer", [1.0, 2.0], 0xFFFF_FFFF, false, 1.0);
        list.fill([0.0, 0.0, 10.0, 10.0], 0x00FF_FFFF);
        list.text("", [0.0, 0.0], 0xFFFF_FFFF, false, 1.0);
        assert_eq!(list.cmds.len(), 1);
        let Cmd::Text { text, .. } = &list.cmds[0] else {
            panic!()
        };
        assert_eq!(list.str(text), "Singleplayer");
        list.clear();
        assert!(list.cmds.is_empty() && list.text.is_empty());
    }

    #[test]
    fn nested_clips_stay_inside_each_other() {
        let mut list = DrawList::default();
        list.clip_start([0.0, 0.0, 100.0, 100.0]);
        list.clip_start([50.0, 50.0, 100.0, 100.0]);
        assert_eq!(list.clip(), Some([50.0, 50.0, 50.0, 50.0]));
        list.clip_end();
        list.clip_end();
        list.clip_end();
        assert_eq!(list.cmds.len(), 4);
    }
}
