//! The images Incari paints itself (UiAtlas.java): corners, icons, the mark, the backdrop, the
//! wordmark's letters. Painted once at the size they're shown at in screen pixels, packed in
//! shelves into Horizon's mod canvas (`horizon/mod-canvas`, 2048 by 1024), and uploaded by the game
//! side. A new atlas (a resource reload) or a full canvas starts over: everything is painted again
//! as it's next drawn.
use std::collections::HashMap;

/// The mod canvas's size in pixels (Horizon's `MOD_CANVAS_SIZE`).
pub const WIDTH: u32 = 2048;
pub const HEIGHT: u32 = 1024;

/// White pixels with coverage in alpha, or colour: unpremultiplied RGBA, row by row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            rgba: vec![0; (width * height * 4) as usize],
        }
    }

    /// White at coverage `a` (0 to 255) at (`x`, `y`).
    pub fn set_alpha(&mut self, x: u32, y: u32, a: u8) {
        let i = ((y * self.width + x) * 4) as usize;
        self.rgba[i..i + 4].copy_from_slice(&[255, 255, 255, a]);
    }

    pub fn set_argb(&mut self, x: u32, y: u32, argb: u32) {
        let i = ((y * self.width + x) * 4) as usize;
        let [a, r, g, b] = argb.to_be_bytes();
        self.rgba[i..i + 4].copy_from_slice(&[r, g, b, a]);
    }

    pub fn argb(&self, x: u32, y: u32) -> u32 {
        let i = ((y * self.width + x) * 4) as usize;
        let p = &self.rgba[i..i + 4];
        u32::from_be_bytes([p[3], p[0], p[1], p[2]])
    }

    /// A white image of `width` by `height` with each pixel's coverage from `coverage`.
    pub fn mask(width: u32, height: u32, coverage: impl Fn(u32, u32) -> f32) -> Self {
        let mut image = Self::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let a = (coverage(x, y).clamp(0.0, 1.0) * 255.0).round() as u8;
                image.set_alpha(x, y, a);
            }
        }
        image
    }

    /// A PNG as an image.
    pub fn decode_png(bytes: &[u8]) -> Option<Self> {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().ok()?;
        let mut buf = vec![0; reader.output_buffer_size()?];
        let info = reader.next_frame(&mut buf).ok()?;
        let (w, h) = (info.width, info.height);
        let mut image = Self::new(w, h);
        let channels = info.color_type.samples();
        for i in 0..(w * h) as usize {
            let p = &buf[i * channels..i * channels + channels];
            let px = match channels {
                1 => [p[0], p[0], p[0], 255],
                2 => [p[0], p[0], p[0], p[1]],
                3 => [p[0], p[1], p[2], 255],
                _ => [p[0], p[1], p[2], p[3]],
            };
            image.rgba[i * 4..i * 4 + 4].copy_from_slice(&px);
        }
        Some(image)
    }

    /// This image scaled to `width` by `height`, each pixel the average of what it covers.
    pub fn scaled(&self, width: u32, height: u32) -> Self {
        let mut out = Self::new(width, height);
        let (sx, sy) = (
            self.width as f32 / width as f32,
            self.height as f32 / height as f32,
        );
        for y in 0..height {
            for x in 0..width {
                let (x0, y0) = ((x as f32 * sx) as u32, (y as f32 * sy) as u32);
                let x1 = (((x + 1) as f32 * sx).ceil() as u32).clamp(x0 + 1, self.width);
                let y1 = (((y + 1) as f32 * sy).ceil() as u32).clamp(y0 + 1, self.height);
                // Premultiplied while averaged, so transparent pixels don't darken the edges.
                let mut sum = [0f32; 4];
                for py in y0..y1 {
                    for px in x0..x1 {
                        let i = ((py * self.width + px) * 4) as usize;
                        let a = self.rgba[i + 3] as f32 / 255.0;
                        sum[0] += self.rgba[i] as f32 * a;
                        sum[1] += self.rgba[i + 1] as f32 * a;
                        sum[2] += self.rgba[i + 2] as f32 * a;
                        sum[3] += a;
                    }
                }
                let n = ((x1 - x0) * (y1 - y0)) as f32;
                let a = sum[3] / n;
                let i = ((y * width + x) * 4) as usize;
                if a > 0.0 {
                    for c in 0..3 {
                        out.rgba[i + c] = (sum[c] / sum[3]).round().clamp(0.0, 255.0) as u8;
                    }
                }
                out.rgba[i + 3] = (a * 255.0).round() as u8;
            }
        }
        out
    }
}

/// What an image on the canvas is, so it's painted once a size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// A white pixel square, for fills.
    White,
    /// A rounded square's corner image: filled, its one-pixel ring, or its soft shadow (radius
    /// and blur), in screen pixels.
    Fill(u16),
    Ring(u16),
    Shadow(u16, u16),
    /// A vertical band of light, brightest in its middle (a hovered button's sweep).
    Band,
    /// Opaque at the left, fading out smoothly to the right.
    Fade,
    /// Opaque at the top, fading out smoothly to the bottom.
    FadeDown,
    /// A soft round dot (snowflakes).
    Dot,
    /// A ring at a size in pixels, its hole's radius in thousandths of its own.
    Ring2(u16, u16),
    /// A line icon at a size in pixels.
    Icon(crate::icons::Icon, u16),
    /// Incari's mark at a size in pixels.
    Logo(u16),
    /// The title's backdrop photo.
    Backdrop,
    /// The player's skin.
    Skin,
    /// A picture painted at run time (a screenshot), its id and width in pixels.
    Picture(u32, u16),
    /// A letter of the wordmark at a height in pixels.
    Wordmark(char, u16),
    /// A shape (Shapes.java) at a size in pixels.
    Shape(crate::paint::Shape, u16),
}

/// Where an image is on the canvas, in pixels (its transparent border not included).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Region {
    /// The whole image, as a draw command's `src`.
    pub fn src(&self) -> [f32; 4] {
        [self.x as f32, self.y as f32, self.w as f32, self.h as f32]
    }

    /// Part of it, `[x, y, w, h]` in its own pixels.
    pub fn part(&self, x: f32, y: f32, w: f32, h: f32) -> [f32; 4] {
        [self.x as f32 + x, self.y as f32 + y, w, h]
    }
}

/// Pixels to copy into the canvas before the next frame draws.
#[derive(Clone, Debug)]
pub struct Upload {
    pub at: [u32; 2],
    pub image: Image,
}

#[derive(Clone, Copy, Debug)]
struct Shelf {
    y: u32,
    height: u32,
    x: u32,
}

/// The canvas as Incari has packed it.
#[derive(Default)]
pub struct Canvas {
    regions: HashMap<Key, Region>,
    shelves: Vec<Shelf>,
    uploads: Vec<Upload>,
    /// Bumped each time the canvas starts over.
    pub generation: u32,
}

/// A transparent pixel around each image, so sampling at its edge never reaches its neighbour.
const PAD: u32 = 1;

impl Canvas {
    /// Starts over: a new atlas, a full canvas, or a new GUI scale.
    pub fn reset(&mut self) {
        self.regions.clear();
        self.shelves.clear();
        self.uploads.clear();
        self.generation = self.generation.wrapping_add(1);
    }

    /// Where `key` is, painting it with `make` the first time. None if it can't fit even on an
    /// empty canvas.
    pub fn get(&mut self, key: Key, make: impl FnOnce() -> Image) -> Option<Region> {
        if let Some(r) = self.regions.get(&key) {
            return Some(*r);
        }
        let image = make();
        let (w, h) = (image.width, image.height);
        if w == 0 || h == 0 || w + 2 * PAD > WIDTH || h + 2 * PAD > HEIGHT {
            return None;
        }
        let at = match self.place(w + 2 * PAD, h + 2 * PAD) {
            Some(at) => at,
            None => {
                // Full: start over with only what's drawn from now on.
                let generation = self.generation;
                self.reset();
                self.generation = generation.wrapping_add(1);
                self.place(w + 2 * PAD, h + 2 * PAD)?
            }
        };
        let region = Region {
            x: at[0] + PAD,
            y: at[1] + PAD,
            w,
            h,
        };
        self.uploads.push(Upload {
            at: [region.x, region.y],
            image,
        });
        self.regions.insert(key, region);
        Some(region)
    }

    /// Forgets `key`, so it's painted again (its old pixels stay unused until the canvas
    /// starts over).
    pub fn forget(&mut self, key: Key) {
        self.regions.remove(&key);
    }

    /// Whether `key` has been painted.
    pub fn has(&self, key: Key) -> bool {
        self.regions.contains_key(&key)
    }

    /// The pixels painted since the last call, for the game side to upload.
    pub fn take_uploads(&mut self) -> Vec<Upload> {
        std::mem::take(&mut self.uploads)
    }

    /// A spot `w` by `h` on a shelf of about that height, or a new shelf.
    fn place(&mut self, w: u32, h: u32) -> Option<[u32; 2]> {
        // A shelf no more than a third taller than the image, so small corners don't waste the
        // backdrop's rows.
        for shelf in &mut self.shelves {
            if h <= shelf.height && shelf.height <= h + h / 3 + 2 && shelf.x + w <= WIDTH {
                let at = [shelf.x, shelf.y];
                shelf.x += w;
                return Some(at);
            }
        }
        let y = self.shelves.last().map_or(0, |s| s.y + s.height);
        if y + h > HEIGHT {
            return None;
        }
        self.shelves.push(Shelf { y, height: h, x: w });
        Some([0, y])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paints_each_image_once_and_uploads_it() {
        let mut canvas = Canvas::default();
        let a = canvas.get(Key::Fill(3), || Image::new(7, 7)).unwrap();
        let again = canvas
            .get(Key::Fill(3), || panic!("painted twice"))
            .unwrap();
        assert_eq!(a, again);
        assert_eq!((a.x, a.y), (1, 1));
        let b = canvas.get(Key::Ring(3), || Image::new(7, 7)).unwrap();
        assert_eq!((b.x, b.y), (10, 1));
        assert_eq!(canvas.take_uploads().len(), 2);
        assert!(canvas.take_uploads().is_empty());
    }

    #[test]
    fn starts_over_when_full() {
        let mut canvas = Canvas::default();
        let generation = canvas.generation;
        for i in 0..3 {
            canvas
                .get(Key::Shadow(i, 0), || Image::new(1500, 400))
                .unwrap();
        }
        assert_ne!(canvas.generation, generation);
        assert!(canvas.has(Key::Shadow(2, 0)));
        assert!(!canvas.has(Key::Shadow(0, 0)));
    }

    #[test]
    fn scales_down_by_averaging() {
        let mut image = Image::new(2, 2);
        image.set_argb(0, 0, 0xFFFF_0000);
        image.set_argb(1, 0, 0xFFFF_0000);
        let small = image.scaled(1, 1);
        assert_eq!(small.argb(0, 0), 0x80FF_0000);
    }
}
