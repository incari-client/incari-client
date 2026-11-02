//! The cosmetics' pictures on the client's canvas (`CosmeticTextures.java`'s GUI half): a cape's
//! outer side, a hat's or back cosmetic's card icon, and a model's texture for the Wardrobe's
//! preview, painted by `art` and `models` the first time each is drawn. A moving cape is painted
//! again ten times a second while it's drawn, and costs nothing once it isn't.
//!
//! The world's textures are the game side's (`rust/mods/cosmetics`): painted from the same
//! functions into the fork's entity canvas.
use super::catalog::{self, Cosmetic};
use super::{art, models};
use crate::canvas::{Image, Key, Region};
use crate::paint::Painter;
use std::cell::RefCell;

/// Canvas picture ids of the cosmetics' images, clear of the screenshots' (counted from 1).
const CAPE_FACE: u32 = 0xC05_0000;
const ICON: u32 = 0xC05_1000;
const MODEL: u32 = 0xC05_2000;

/// A moving cape's frames a second.
pub const CAPE_FPS: f64 = 10.0;

thread_local! {
    /// Each moving cape's frame on the canvas, so the one before is let go.
    static FRAMES: RefCell<Vec<(usize, u16)>> = const { RefCell::new(Vec::new()) };
}

fn image(width: usize, height: usize, pixels: &[u32]) -> Image {
    Image { width: width as u32, height: height as u32, rgba: art::rgba(pixels) }
}

/// The frame a moving cape shows at `now_ms`; 0 for one that doesn't move.
pub fn cape_frame(cape: &Cosmetic, now_ms: f64) -> u16 {
    if art::animated(cape.id) {
        // 1 up: frame 0 is a still cape's.
        ((now_ms / 1000.0 * CAPE_FPS) as u64 % u64::from(u16::MAX - 1)) as u16 + 1
    } else {
        0
    }
}

/// Where the cape's outer side (10x16) is on the canvas, painted for `p`'s clock.
pub fn cape_face(p: &mut Painter, cape: &Cosmetic) -> Option<Region> {
    let index = catalog::index(cape);
    let frame = cape_frame(cape, p.motion.now);
    let key = Key::Picture(CAPE_FACE + index as u32, frame);
    if frame != 0 && !p.canvas.has(key) {
        // The last frame's pixels are let go (they stay unused until the canvas starts over).
        FRAMES.with(|f| {
            let mut f = f.borrow_mut();
            match f.iter_mut().find(|(i, _)| *i == index) {
                Some((_, last)) => {
                    p.canvas.forget(Key::Picture(CAPE_FACE + index as u32, *last));
                    *last = frame;
                }
                None => f.push((index, frame)),
            }
        });
    }
    let t = (p.motion.now / 1000.0) as f32;
    p.canvas.get(key, || {
        let mut px = [0u32; 160];
        art::cape_face(cape.id, t, &mut px);
        image(10, 16, &px)
    })
}

/// Where a hat's or back cosmetic's card icon (24 square) is on the canvas.
pub fn icon(p: &mut Painter, cosmetic: &Cosmetic) -> Option<Region> {
    let key = Key::Picture(ICON + catalog::index(cosmetic) as u32, art::ICON as u16);
    p.canvas.get(key, || {
        let mut px = vec![0u32; art::ICON * art::ICON];
        art::icon(cosmetic.id, &mut px);
        image(art::ICON, art::ICON, &px)
    })
}

/// Where a model's 64x64 texture is on the canvas (a hat or back cosmetic; None for the rest).
pub fn model(p: &mut Painter, cosmetic: &Cosmetic) -> Option<Region> {
    let spec = models::spec(cosmetic.id)?;
    let key = Key::Picture(MODEL + catalog::index(cosmetic) as u32, models::TEXTURE as u16);
    p.canvas.get(key, || {
        let mut px = vec![0u32; models::TEXTURE * models::TEXTURE];
        models::texture(cosmetic.id, &spec, &mut px);
        image(models::TEXTURE, models::TEXTURE, &px)
    })
}

/// Draws the cape's outer side as it hangs over `[x, y, w, h]`.
pub fn draw_cape(p: &mut Painter, cape: &Cosmetic, rect: [f32; 4]) {
    if let Some(r) = cape_face(p, cape) {
        let argb = p.faded(0xFFFF_FFFF);
        let dst = p.place(rect[0], rect[1], rect[2], rect[3]);
        p.list.image(r.src(), dst, argb);
    }
}

/// Draws a hat's or back cosmetic's icon over `[x, y, size, size]`.
pub fn draw_icon(p: &mut Painter, cosmetic: &Cosmetic, x: f32, y: f32, size: f32) {
    if let Some(r) = icon(p, cosmetic) {
        let argb = p.faded(0xFFFF_FFFF);
        let dst = p.place(x, y, size, size);
        p.list.image(r.src(), dst, argb);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn still_capes_keep_one_frame_and_moving_ones_ten_a_second() {
        let frost = catalog::by_id("cape.frost").unwrap();
        let tide = catalog::by_id("cape.tide").unwrap();
        assert_eq!(cape_frame(frost, 12_345.0), 0);
        assert_ne!(cape_frame(tide, 0.0), cape_frame(tide, 100.0));
        assert_eq!(cape_frame(tide, 0.0), cape_frame(tide, 99.0));
    }
}
