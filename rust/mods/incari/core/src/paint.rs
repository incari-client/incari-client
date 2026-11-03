//! Drawing in the client's look (RoundRects, Shapes, Logo, the wordmark, text that fits): what a
//! screen or a HUD element draws with. Shapes are painted at the size they show at in screen
//! pixels into the canvas and drawn as nine-slices placed on whole screen pixels, so corners stay
//! smooth at any GUI scale.
use crate::canvas::{Canvas, Image, Key, Region};
use crate::color;
use crate::draw::{DrawList, Rect};
use crate::icons::Icon;
use crate::motion::Motion;
use crate::theme::Theme;

/// The game's font, as the game side measures it.
pub trait Measure {
    /// The width of `text` in GUI units, at scale 1.
    fn width(&self, text: &str) -> f32;
}

/// The images Incari paints from: its mark, the title's backdrop and the wordmark's font, as the
/// game side hands them over (the Incari pack's files).
#[derive(Default)]
pub struct Assets {
    logo: Option<Image>,
    backdrop: Option<Image>,
    wordmark: Option<ab_glyph::FontVec>,
    /// The player's skin (64 by 64) and whether its arms are slim.
    pub skin: Option<(Image, bool)>,
}

impl Assets {
    /// From the files' bytes: `icon_256.png`, `title_backdrop.png` and `michroma.ttf`.
    pub fn load(logo_png: &[u8], backdrop_png: &[u8], wordmark_ttf: &[u8]) -> Self {
        Self {
            logo: Image::decode_png(logo_png),
            backdrop: Image::decode_png(backdrop_png),
            wordmark: ab_glyph::FontVec::try_from_vec(wordmark_ttf.to_vec()).ok(),
            skin: None,
        }
    }

    /// The backdrop's width over its height.
    pub fn backdrop_aspect(&self) -> f32 {
        self.backdrop
            .as_ref()
            .map_or(16.0 / 9.0, |b| b.width as f32 / b.height as f32)
    }
}

/// A small shape (Shapes.java): waypoint icons, the minimap's arrow. White inside a thin dark rim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shape {
    Circle,
    Square,
    Diamond,
    Triangle,
    Star,
    Heart,
    Hexagon,
    Cross,
    Arrow,
}

/// The largest corner made, in screen pixels; larger ones are drawn at this size.
const MAX_CORNER: u32 = 48;
const BAND_WIDTH: u32 = 64;
const DOT_SIZE: u32 = 8;

/// What a frame draws with.
pub struct Painter<'a> {
    pub list: &'a mut DrawList,
    pub canvas: &'a mut Canvas,
    pub assets: &'a Assets,
    pub motion: &'a mut Motion,
    pub measure: &'a dyn Measure,
    pub theme: Theme,
    /// Screen pixels per GUI unit.
    pub scale: f32,
    /// What everything drawn is faded by (a screen easing in).
    pub fade: f32,
    /// Where GUI point (0, 0) is drawn, and how much bigger everything is drawn: a screen
    /// rising in, a HUD element at its own place and scale.
    pub origin: [f32; 2],
    pub zoom: f32,
}

impl Painter<'_> {
    /// `argb` faded by the painter's fade.
    pub fn faded(&self, argb: u32) -> u32 {
        color::alpha(argb, self.fade)
    }

    /// A point in the painter's units to GUI units.
    fn at(&self, x: f32, y: f32) -> (f32, f32) {
        (
            self.origin[0] + x * self.zoom,
            self.origin[1] + y * self.zoom,
        )
    }

    /// A rectangle in the painter's units to GUI units.
    pub fn place(&self, x: f32, y: f32, w: f32, h: f32) -> [f32; 4] {
        let (x, y) = self.at(x, y);
        [x, y, w * self.zoom, h * self.zoom]
    }

    /// Draws what `f` draws at (`x`, `y`) and `scale` times bigger, then goes back.
    pub fn push<R>(&mut self, x: f32, y: f32, scale: f32, f: impl FnOnce(&mut Self) -> R) -> R {
        let (origin, zoom) = (self.origin, self.zoom);
        self.origin = [origin[0] + x * zoom, origin[1] + y * zoom];
        self.zoom = zoom * scale;
        let r = f(self);
        self.origin = origin;
        self.zoom = zoom;
        r
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    /// A screen pixel position back in GUI units.
    fn gui(&self, px: f32) -> f32 {
        px / self.scale
    }

    /// A plain rectangle.
    pub fn fill(&mut self, x: f32, y: f32, w: f32, h: f32, argb: u32) {
        let argb = self.faded(argb);
        let r = self.place(x, y, w, h);
        self.list.fill(r, argb);
    }

    /// A vertical gradient from `top` to `bottom`.
    pub fn gradient(&mut self, x: f32, y: f32, w: f32, h: f32, top: u32, bottom: u32) {
        self.fill(x, y, w, h, bottom);
        let Some(r) = self.canvas.get(Key::FadeDown, fade_down) else {
            return;
        };
        let argb = self.faded(top);
        let dst = self.place(x, y, w, h);
        self.list.image(r.src(), dst, argb);
    }

    /// A filled rectangle with corners of `radius` GUI units.
    pub fn round(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, argb: u32) {
        self.round_box(x, y, w, h, radius, argb, 0, 0.0, 0.0, 0);
    }

    /// A one-pixel outline just inside the rectangle, following the same corners as `round`.
    pub fn outline(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, argb: u32) {
        self.round_box(x, y, w, h, radius, 0, argb, 0.0, 0.0, 0);
    }

    /// A soft shadow or glow around the rectangle, `blur` GUI units wide, its middle left out.
    #[allow(clippy::too_many_arguments)]
    pub fn shadow(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, blur: f32, argb: u32) {
        self.round_box(x, y, w, h, radius, 0, 0, blur, 0.0, argb);
    }

    /// Shadow, fill and edge with the same corners, in that order: the shadow `shadow_y` lower.
    /// A colour of 0 (or a blur of 0) leaves that part out.
    #[allow(clippy::too_many_arguments)]
    pub fn round_box(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        fill: u32,
        edge: u32,
        blur: f32,
        shadow_y: f32,
        shadow: u32,
    ) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let (fill, edge, shadow) = (self.faded(fill), self.faded(edge), self.faded(shadow));
        let [x, y, w, h] = self.place(x, y, w, h);
        let (radius, blur, shadow_y) = (radius * self.zoom, blur * self.zoom, shadow_y * self.zoom);
        let (px, py, pw, ph) = (self.px(x), self.px(y), self.px(w), self.px(h));
        if blur > 0.0 && shadow >> 24 != 0 {
            let b = (self.px(blur) as u32).min(MAX_CORNER / 2);
            let r = (self.px(radius) as u32)
                .min(MAX_CORNER / 2)
                .min((pw.min(ph) / 2.0) as u32);
            let corner = r + b;
            let (sw, sh) = (pw + 2.0 * b as f32, ph + 2.0 * b as f32);
            if sw >= 2.0 * corner as f32 && sh >= 2.0 * corner as f32 {
                let key = Key::Shadow(r as u16, b as u16);
                if let Some(region) = self.canvas.get(key, || corner_image(2, r, b)) {
                    let at = [px - b as f32, py + self.px(shadow_y) - b as f32, sw, sh];
                    self.nine(region, corner, at, shadow, false);
                }
            }
        }
        let r = (self.px(radius) as u32)
            .min(MAX_CORNER)
            .min((pw.min(ph) / 2.0) as u32);
        if fill >> 24 != 0 {
            if r == 0 {
                self.list.fill([x, y, w, h], fill);
            } else if let Some(region) = self
                .canvas
                .get(Key::Fill(r as u16), || corner_image(0, r, 0))
            {
                self.nine(region, r, [px, py, pw, ph], fill, true);
            }
        }
        if edge >> 24 != 0 {
            if r <= 1 {
                self.frame_px([px, py, pw, ph], edge);
            } else if let Some(region) = self
                .canvas
                .get(Key::Ring(r as u16), || corner_image(1, r, 0))
            {
                self.nine(region, r, [px, py, pw, ph], edge, false);
            }
        }
    }

    /// A one-pixel frame inside the rectangle, in screen pixels.
    fn frame_px(&mut self, [x, y, w, h]: [f32; 4], argb: u32) {
        let one = 1.0;
        let g = |p: &Self, r: [f32; 4]| [p.gui(r[0]), p.gui(r[1]), p.gui(r[2]), p.gui(r[3])];
        let sides = [
            [x, y, w, one],
            [x, y + h - one, w, one],
            [x, y + one, one, h - 2.0 * one],
            [x + w - one, y + one, one, h - 2.0 * one],
        ];
        for side in sides {
            let rect = g(self, side);
            self.list.fill(rect, argb);
        }
    }

    /// The image of a corner `corner` pixels a side (its middle row and column stretched over
    /// the edges and middle) over `at`, in screen pixels.
    fn nine(&mut self, region: Region, corner: u32, at: [f32; 4], argb: u32, middle: bool) {
        let c = corner as f32;
        let [x, y, w, h] = at;
        let (mw, mh) = (w - 2.0 * c, h - 2.0 * c);
        let s = region.w as f32 - 2.0 * c;
        let cols = [(0.0, c, x, c), (c, s, x + c, mw), (c + s, c, x + w - c, c)];
        let rows = [(0.0, c, y, c), (c, s, y + c, mh), (c + s, c, y + h - c, c)];
        for (j, &(sy, sh, dy, dh)) in rows.iter().enumerate() {
            for (i, &(sx, sw, dx, dw)) in cols.iter().enumerate() {
                if (i == 1 && j == 1 && !middle) || dw <= 0.0 || dh <= 0.0 {
                    continue;
                }
                let src = region.part(sx, sy, sw, sh);
                let dst = [self.gui(dx), self.gui(dy), self.gui(dw), self.gui(dh)];
                self.list.image(src, dst, argb);
            }
        }
    }

    /// A soft vertical band of light, brightest in its middle: the sweep over a hovered button.
    pub fn band(&mut self, x: f32, y: f32, w: f32, h: f32, argb: u32) {
        let Some(r) = self.canvas.get(Key::Band, band) else {
            return;
        };
        let argb = self.faded(argb);
        let dst = self.place(x, y, w, h);
        self.list.image(r.src(), dst, argb);
    }

    /// `argb` at the rectangle's left edge, fading out smoothly to nothing at its right.
    pub fn fade_right(&mut self, x: f32, y: f32, w: f32, h: f32, argb: u32) {
        let Some(r) = self.canvas.get(Key::Fade, fade_right) else {
            return;
        };
        let argb = self.faded(argb);
        let dst = self.place(x, y, w, h);
        self.list.image(r.src(), dst, argb);
    }

    /// A ring around (`cx`, `cy`), `radius` to its outer edge and `width` thick, anti-aliased.
    pub fn ring(&mut self, cx: f32, cy: f32, radius: f32, width: f32, argb: u32) {
        let [x, y, size, _] = self.place(cx - radius, cy - radius, radius * 2.0, 0.0);
        let pixels = (self.px(size) as u32).clamp(2, 256);
        let inner = (1.0 - width / radius).clamp(0.0, 1.0);
        let key = Key::Ring2(pixels as u16, (inner * 1000.0) as u16);
        let Some(r) = self.canvas.get(key, || {
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
        let argb = self.faded(argb);
        let (px, py) = (self.px(x), self.px(y));
        let dst = [
            self.gui(px),
            self.gui(py),
            self.gui(pixels as f32),
            self.gui(pixels as f32),
        ];
        self.list.image(r.src(), dst, argb);
    }

    /// A soft round dot `size` GUI units across.
    pub fn dot(&mut self, x: f32, y: f32, size: f32, argb: u32) {
        let Some(r) = self.canvas.get(Key::Dot, dot) else {
            return;
        };
        let argb = self.faded(argb);
        let dst = self.place(x, y, size, size);
        self.list.image(r.src(), dst, argb);
    }

    /// A one-pixel line across, in screen pixels: a separator.
    pub fn hairline(&mut self, x: f32, y: f32, w: f32, argb: u32) {
        let argb = self.faded(argb);
        let [x, y, w, _] = self.place(x, y, w, 0.0);
        let rect = [x, self.gui(self.px(y)), w, 1.0 / self.scale];
        self.list.fill(rect, argb);
    }

    /// Icon `icon` `size` GUI units square at (`x`, `y`), in `argb`.
    pub fn icon(&mut self, icon: Icon, x: f32, y: f32, size: f32, argb: u32) {
        if icon == Icon::Incari {
            // The mark leaves room around the tree for the comet's orbit: drawn a little past
            // the icon's square, so the tree stands as tall as the line icons beside it.
            let grow = (size / 4.0).floor().max(1.0);
            self.logo(
                x - grow,
                y - grow,
                size + grow * 2.0,
                (argb >> 24) as f32 / 255.0,
            );
            return;
        }
        let [x, y, size, _] = self.place(x, y, size, 0.0);
        let pixels = (self.px(size) as u32).min(crate::icons::MAX_PIXELS);
        if pixels == 0 || argb >> 24 == 0 {
            return;
        }
        let Some(r) = self
            .canvas
            .get(Key::Icon(icon, pixels as u16), || icon.paint(pixels))
        else {
            return;
        };
        let argb = self.faded(argb);
        let (px, py) = (self.px(x), self.px(y));
        let dst = [
            self.gui(px),
            self.gui(py),
            self.gui(pixels as f32),
            self.gui(pixels as f32),
        ];
        self.list.image(r.src(), dst, argb);
    }

    /// Incari's mark `size` GUI units square at (`x`, `y`), at `alpha` (0 to 1).
    pub fn logo(&mut self, x: f32, y: f32, size: f32, alpha: f32) {
        let [x, y, size, _] = self.place(x, y, size, 0.0);
        let pixels = (self.px(size) as u32).min(256);
        let Some(source) = self.assets.logo.as_ref() else {
            return;
        };
        if pixels == 0 {
            return;
        }
        let Some(r) = self
            .canvas
            .get(Key::Logo(pixels as u16), || source.scaled(pixels, pixels))
        else {
            return;
        };
        let a = ((alpha.clamp(0.0, 1.0) * self.fade) * 255.0).round() as u32;
        let (px, py) = (self.px(x), self.px(y));
        let dst = [
            self.gui(px),
            self.gui(py),
            self.gui(pixels as f32),
            self.gui(pixels as f32),
        ];
        self.list.image(r.src(), dst, a << 24 | 0xFF_FFFF);
    }

    /// The player seen from the front, `w` by `h` (the model's 16 by 32 skin pixels, its
    /// outer layer over it), drawn from their skin.
    pub fn player(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let Some((skin, slim)) = self.assets.skin.as_ref() else {
            return;
        };
        let slim = *slim;
        let Some(r) = self.canvas.get(Key::Skin, || skin.clone()) else {
            return;
        };
        let unit = (w / 16.0).min(h / 32.0);
        let ox = x + (w - 16.0 * unit) / 2.0;
        let oy = y + (h - 32.0 * unit) / 2.0;
        let arm = if slim { 3.0 } else { 4.0 };
        // (source x, y, w, h in the skin, then where on the 16 by 32 front): the inner layer,
        // then the outer.
        let parts = [
            ([8.0, 8.0, 8.0, 8.0], [4.0, 0.0]),
            ([20.0, 20.0, 8.0, 12.0], [4.0, 8.0]),
            ([44.0, 20.0, arm, 12.0], [4.0 - arm, 8.0]),
            ([36.0, 52.0, arm, 12.0], [12.0, 8.0]),
            ([4.0, 20.0, 4.0, 12.0], [4.0, 20.0]),
            ([20.0, 52.0, 4.0, 12.0], [8.0, 20.0]),
            ([40.0, 8.0, 8.0, 8.0], [4.0, 0.0]),
            ([20.0, 36.0, 8.0, 12.0], [4.0, 8.0]),
            ([44.0, 36.0, arm, 12.0], [4.0 - arm, 8.0]),
            ([52.0, 52.0, arm, 12.0], [12.0, 8.0]),
            ([4.0, 36.0, 4.0, 12.0], [4.0, 20.0]),
            ([4.0, 52.0, 4.0, 12.0], [8.0, 20.0]),
        ];
        for ([sx, sy, sw, sh], [fx, fy]) in parts {
            let dst = self.place(ox + fx * unit, oy + fy * unit, sw * unit, sh * unit);
            let argb = self.faded(0xFFFF_FFFF);
            self.list.image(r.part(sx, sy, sw, sh), dst, argb);
        }
    }

    /// The title's backdrop photo over `rect`, cropped to cover it.
    pub fn backdrop(&mut self, rect: Rect, argb: u32) {
        let Some(source) = self.assets.backdrop.as_ref() else {
            return;
        };
        let Some(r) = self.canvas.get(Key::Backdrop, || source.clone()) else {
            return;
        };
        let [x, y, w, h] = rect;
        let aspect = r.w as f32 / r.h as f32;
        // Cover: the photo's middle, cut to the window's shape.
        // Centred across, and down on the peak line (MenuBackdrop.FOCUS_Y).
        let src = if w / h > aspect {
            let sh = r.w as f32 * h / w;
            let top = (r.h as f32 * 0.35 - sh / 2.0).clamp(0.0, r.h as f32 - sh);
            r.part(0.0, top, r.w as f32, sh)
        } else {
            let sw = r.h as f32 * w / h;
            r.part((r.w as f32 - sw) / 2.0, 0.0, sw, r.h as f32)
        };
        let argb = self.faded(argb);
        self.list.image(src, [x, y, w, h], argb);
    }

    /// A letter of the wordmark (Michroma) `size` GUI units tall (the font's size) with its top
    /// at `y`; returns its advance in GUI units.
    pub fn wordmark_letter(&mut self, letter: char, x: f32, y: f32, size: f32, argb: u32) -> f32 {
        use ab_glyph::{Font, ScaleFont};
        let Some(font) = self.assets.wordmark.as_ref() else {
            return 0.0;
        };
        let (x, y) = self.at(x, y);
        let pixels = self.px(size * self.zoom).max(1.0);
        let scaled = font.as_scaled(pixels);
        let id = font.glyph_id(letter);
        let advance = scaled.h_advance(id) / self.scale / self.zoom;
        let glyph = id.with_scale_and_position(pixels, ab_glyph::point(0.0, scaled.ascent()));
        let Some(outline) = font.outline_glyph(glyph) else {
            return advance;
        };
        let bounds = outline.px_bounds();
        let (gw, gh) = (bounds.width().ceil() as u32, bounds.height().ceil() as u32);
        let key = Key::Wordmark(letter, pixels as u16);
        let made = self.canvas.get(key, || {
            let mut image = Image::new(gw.max(1), gh.max(1));
            outline.draw(|gx, gy, c| {
                if gx < image.width && gy < image.height {
                    image.set_alpha(gx, gy, (c.clamp(0.0, 1.0) * 255.0).round() as u8);
                }
            });
            image
        });
        if let Some(r) = made {
            let (px, py) = (
                self.px(x) + bounds.min.x.round(),
                self.px(y) + bounds.min.y.round(),
            );
            let dst = [
                self.gui(px),
                self.gui(py),
                self.gui(r.w as f32),
                self.gui(r.h as f32),
            ];
            let argb = self.faded(argb);
            self.list.image(r.src(), dst, argb);
        }
        advance
    }

    /// The wordmark's letter's advance without drawing it.
    #[allow(dead_code)]
    pub fn wordmark_advance(&self, letter: char, size: f32) -> f32 {
        use ab_glyph::{Font, ScaleFont};
        let Some(font) = self.assets.wordmark.as_ref() else {
            return 0.0;
        };
        let pixels = self.px(size).max(1.0);
        font.as_scaled(pixels).h_advance(font.glyph_id(letter)) / self.scale
    }

    /// Shape `shape` centred on (`cx`, `cy`), `size` GUI units across, its inside in `argb`.
    pub fn shape(&mut self, shape: Shape, cx: f32, cy: f32, size: f32, argb: u32) {
        let (cx, cy) = self.at(cx, cy);
        let pixels = (self.px(size * self.zoom) as u32).min(128);
        if pixels < 2 || argb >> 24 == 0 {
            return;
        }
        let Some(r) = self.canvas.get(Key::Shape(shape, pixels as u16), || {
            paint_shape(shape, pixels)
        }) else {
            return;
        };
        let half = pixels as f32 / 2.0;
        let (px, py) = (self.px(cx) - half, self.px(cy) - half);
        let dst = [
            self.gui(px.round()),
            self.gui(py.round()),
            self.gui(pixels as f32),
            self.gui(pixels as f32),
        ];
        let argb = self.faded(argb);
        self.list.image(r.src(), dst, argb);
    }

    /// A line of the game's text at (`x`, `y`).
    pub fn text(&mut self, text: &str, x: f32, y: f32, argb: u32, shadow: bool) {
        let argb = self.faded(argb);
        let (x, y) = self.at(x, y);
        self.list.text(text, [x, y], argb, shadow, self.zoom);
    }

    /// Text `scale` times its size.
    pub fn text_scaled(&mut self, text: &str, x: f32, y: f32, argb: u32, shadow: bool, scale: f32) {
        let argb = self.faded(argb);
        let (x, y) = self.at(x, y);
        self.list
            .text(text, [x, y], argb, shadow, scale * self.zoom);
    }

    /// Text centred on `cx`.
    pub fn centered(&mut self, text: &str, cx: f32, y: f32, argb: u32, shadow: bool) {
        let w = self.measure.width(text);
        self.text(text, (cx - w / 2.0).floor(), y, argb, shadow);
    }

    /// The theme's text colour.
    pub fn text_color(&self) -> u32 {
        self.theme.text
    }

    pub fn width(&self, text: &str) -> f32 {
        self.measure.width(text)
    }

    /// `text` cut to `width` with "..." at the end if it's wider (Ui.fit).
    pub fn fit<'t>(&self, text: &'t str, width: f32) -> std::borrow::Cow<'t, str> {
        fit(self.measure, text, width)
    }

    /// `text` wrapped to `width`, at most `max_lines` lines, the last ending in "..." when some
    /// was left out (Ui.wrap).
    pub fn wrap(&self, text: &str, width: f32, max_lines: usize) -> Vec<String> {
        wrap(self.measure, text, width, max_lines)
    }

    /// An item's icon, `size` GUI units square.
    pub fn item(&mut self, id: &str, x: f32, y: f32, size: f32) {
        let (x, y) = self.at(x, y);
        self.list.item(id, [x, y], size * self.zoom);
    }

    /// A sprite of the game's by name.
    pub fn sprite(&mut self, name: &str, src: Rect, dst: Rect, argb: u32) {
        let argb = self.faded(argb);
        let dst = self.place(dst[0], dst[1], dst[2], dst[3]);
        self.list.sprite(name, src, dst, argb);
    }

    pub fn clip_start(&mut self, rect: Rect) {
        let rect = self.place(rect[0], rect[1], rect[2], rect[3]);
        self.list.clip_start(rect);
    }

    pub fn clip_end(&mut self) {
        self.list.clip_end();
    }
}

const ELLIPSIS: &str = "...";

/// `text` cut to `width` with "..." at the end if it's wider; whole if it fits; nothing where
/// there's no room even for the "...".
pub fn fit<'t>(measure: &dyn Measure, text: &'t str, width: f32) -> std::borrow::Cow<'t, str> {
    if measure.width(text) <= width {
        return text.into();
    }
    let dots = measure.width(ELLIPSIS);
    if width < dots {
        return "".into();
    }
    let room = width - dots;
    let mut end = 0;
    for (i, c) in text.char_indices() {
        let next = i + c.len_utf8();
        if measure.width(&text[..next]) > room {
            break;
        }
        end = next;
    }
    format!("{}{ELLIPSIS}", text[..end].trim_end()).into()
}

/// `text` wrapped at spaces to `width`, at most `max_lines` lines.
pub fn wrap(measure: &dyn Measure, text: &str, width: f32, max_lines: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split(' ') {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && measure.width(&candidate) > width {
                lines.push(std::mem::take(&mut line));
                line = word.to_owned();
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        // A cut sentence never looks whole.
        if let Some(last) = lines.last_mut() {
            *last = fit(measure, &format!("{last}{ELLIPSIS}"), width).into_owned();
        }
    }
    lines
}

/// A rounded square's corner image: 0 filled, 1 its one-pixel ring, 2 its soft shadow.
fn corner_image(shape: u8, radius: u32, blur: u32) -> Image {
    let c = radius + blur;
    let size = 2 * c + 1;
    let half = size as f32 / 2.0 - blur as f32;
    Image::mask(size, size, |px, py| {
        if shape == 2 {
            // Outside the shape only, fading out over the blur: under a translucent panel any
            // shadow would show through.
            let d = distance(px as f32 + 0.5, py as f32 + 0.5, size, half, radius as f32);
            let outside = (d + 0.5).clamp(0.0, 1.0);
            let fade = 1.0 - (d / blur as f32).clamp(0.0, 1.0);
            return outside * fade * fade;
        }
        let (mut inside, mut inset) = (0, 0);
        for sy in 0..4 {
            for sx in 0..4 {
                let d = distance(
                    px as f32 + (sx as f32 + 0.5) / 4.0,
                    py as f32 + (sy as f32 + 0.5) / 4.0,
                    size,
                    half,
                    radius as f32,
                );
                if d < 0.0 {
                    inside += 1;
                }
                if d < -1.0 {
                    inset += 1;
                }
            }
        }
        (if shape == 0 { inside } else { inside - inset }) as f32 / 16.0
    })
}

/// The signed distance from the point to a rounded square centred in the image.
fn distance(x: f32, y: f32, size: u32, half: f32, radius: f32) -> f32 {
    let qx = (x - size as f32 / 2.0).abs() - (half - radius);
    let qy = (y - size as f32 / 2.0).abs() - (half - radius);
    let (ox, oy) = (qx.max(0.0), qy.max(0.0));
    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - radius
}

fn band() -> Image {
    Image::mask(BAND_WIDTH, 1, |x, _| {
        let s = (std::f32::consts::PI * (x as f32 + 0.5) / BAND_WIDTH as f32).sin();
        s * s
    })
}

fn fade_right() -> Image {
    Image::mask(BAND_WIDTH, 1, |x, _| {
        let t = 1.0 - (x as f32 + 0.5) / BAND_WIDTH as f32;
        t * t * (3.0 - 2.0 * t)
    })
}

fn fade_down() -> Image {
    Image::mask(1, BAND_WIDTH, |_, y| {
        1.0 - (y as f32 + 0.5) / BAND_WIDTH as f32
    })
}

fn dot() -> Image {
    Image::mask(DOT_SIZE, DOT_SIZE, |x, y| {
        let c = DOT_SIZE as f32 / 2.0;
        let d = ((x as f32 + 0.5 - c).powi(2) + (y as f32 + 0.5 - c).powi(2)).sqrt() / c;
        let t = (1.0 - d).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    })
}

/// The rim's colour: a cold near-black, a little see-through.
const RIM: u32 = 0xE010_1820;

fn paint_shape(shape: Shape, pixels: u32) -> Image {
    let mut image = Image::new(pixels, pixels);
    // The rim's width in the shape's units (it spans -1 to 1): about a pixel and a half, more on
    // large icons, so it reads at every size.
    let unit = 2.0 / pixels as f32;
    let rim = (pixels as f32 * 0.07).max(1.25) * unit;
    let rim_alpha = (RIM >> 24) as f32 / 255.0;
    for py in 0..pixels {
        for px in 0..pixels {
            let (mut fill, mut edge) = (0.0, 0.0);
            for sy in 0..4 {
                for sx in 0..4 {
                    let x = (px as f32 + (sx as f32 + 0.5) / 4.0) * unit - 1.0;
                    let y = (py as f32 + (sy as f32 + 0.5) / 4.0) * unit - 1.0;
                    let d = shape_distance(shape, x, y);
                    if d < -rim {
                        fill += 1.0;
                    } else if d < 0.0 {
                        edge += 1.0;
                    }
                }
            }
            let coverage = (fill + edge * rim_alpha) / 16.0;
            let alpha = (coverage.min(1.0) * 255.0).round() as u32;
            let white = if fill + edge == 0.0 {
                0.0
            } else {
                fill / (fill + edge * rim_alpha)
            };
            let c = |shift: u32| {
                ((255.0 * white + (RIM >> shift & 0xFF) as f32 * (1.0 - white)).round() as u32)
                    << shift
            };
            image.set_argb(px, py, alpha << 24 | c(16) | c(8) | c(0));
        }
    }
    image
}

/// The signed distance from (x, y) (each -1 to 1, y down) to the shape's edge: negative inside.
fn shape_distance(shape: Shape, x: f32, y: f32) -> f32 {
    use crate::icons::{length, rbox};
    match shape {
        Shape::Circle => length(x, y) - 0.92,
        Shape::Square => rbox(x, y, 0.78, 0.78, 0.2),
        Shape::Diamond => polygon(&[0.0, -0.97, 0.8, 0.0, 0.0, 0.97, -0.8, 0.0], x, y),
        Shape::Triangle => polygon(&[0.0, -0.9, 0.95, 0.75, -0.95, 0.75], x, y),
        Shape::Star => polygon(&star(), x, y),
        Shape::Heart => heart(x, y),
        Shape::Hexagon => polygon(&regular(6, 0.95, 0.95), x, y),
        Shape::Cross => {
            let c = 0.5f32.sqrt();
            let (u, v) = ((x + y) * c, (x - y) * c);
            rbox(u, v, 1.0, 0.27, 0.2).min(rbox(v, u, 1.0, 0.27, 0.2))
        }
        Shape::Arrow => polygon(&[0.0, -0.95, 0.72, 0.8, 0.0, 0.42, -0.72, 0.8], x, y),
    }
}

fn regular(sides: usize, rx: f32, ry: f32) -> Vec<f32> {
    let mut points = Vec::with_capacity(sides * 2);
    for i in 0..sides {
        let angle =
            -std::f32::consts::FRAC_PI_2 + i as f32 * 2.0 * std::f32::consts::PI / sides as f32;
        points.push(angle.cos() * rx);
        points.push(angle.sin() * ry);
    }
    points
}

fn star() -> Vec<f32> {
    let mut points = Vec::with_capacity(20);
    for i in 0..10 {
        let angle = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
        let radius = if i % 2 == 0 { 1.0 } else { 0.44 };
        points.push(angle.cos() * radius);
        // A little lower, so the star's weight sits in the middle.
        points.push(angle.sin() * radius + 0.06);
    }
    points
}

/// Inigo Quilez's heart, its point down, scaled to fit.
fn heart(x: f32, y: f32) -> f32 {
    let scale = 1.6;
    let hx = x.abs() / scale;
    let hy = (0.9 - y) / scale;
    let d = if hy + hx > 1.0 {
        let (dx, dy) = (hx - 0.25, hy - 0.75);
        (dx * dx + dy * dy).sqrt() - 2.0f32.sqrt() / 4.0
    } else {
        let (ax, ay) = (hx, hy - 1.0);
        let m = 0.5 * (hx + hy).max(0.0);
        let (bx, by) = (hx - m, hy - m);
        (ax * ax + ay * ay).min(bx * bx + by * by).sqrt() * (hx - hy).signum()
    };
    d * scale
}

/// Inigo Quilez's distance to a polygon given as x, y pairs.
fn polygon(v: &[f32], x: f32, y: f32) -> f32 {
    let n = v.len() / 2;
    let mut d = (x - v[0]).powi(2) + (y - v[1]).powi(2);
    let mut sign = 1.0;
    let mut j = n - 1;
    for i in 0..n {
        let (vix, viy, vjx, vjy) = (v[2 * i], v[2 * i + 1], v[2 * j], v[2 * j + 1]);
        let (ex, ey) = (vjx - vix, vjy - viy);
        let (wx, wy) = (x - vix, y - viy);
        let t = ((wx * ex + wy * ey) / (ex * ex + ey * ey)).clamp(0.0, 1.0);
        let (bx, by) = (wx - ex * t, wy - ey * t);
        d = d.min(bx * bx + by * by);
        let c = [y >= viy, y < vjy, ex * wy > ey * wx];
        if c.iter().all(|&b| b) || c.iter().all(|&b| !b) {
            sign = -sign;
        }
        j = i;
    }
    sign * d.sqrt()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Six GUI units a character, as a stand-in for the game's font.
    pub(crate) struct Mono;
    impl Measure for Mono {
        fn width(&self, text: &str) -> f32 {
            text.chars().count() as f32 * 6.0
        }
    }

    #[test]
    fn fits_and_wraps_as_ui_does() {
        assert_eq!(fit(&Mono, "Toggle Sprint", 200.0), "Toggle Sprint");
        assert_eq!(fit(&Mono, "Toggle Sprint", 60.0), "Toggle...");
        assert_eq!(fit(&Mono, "Toggle", 12.0), "");
        let lines = wrap(
            &Mono,
            "Sprint and sneak without holding the keys.",
            120.0,
            2,
        );
        assert_eq!(lines, ["Sprint and sneak", "without holding t..."]);
    }

    #[test]
    fn corners_are_smooth_and_hollow_where_they_should_be() {
        let fill = corner_image(0, 6, 0);
        assert_eq!(fill.width, 13);
        assert_eq!(fill.argb(6, 6) >> 24, 255);
        assert_eq!(fill.argb(0, 0) >> 24, 0);
        let ring = corner_image(1, 6, 0);
        assert_eq!(ring.argb(6, 6) >> 24, 0);
        assert_eq!(ring.argb(6, 0) >> 24, 255);
        let shadow = corner_image(2, 4, 4);
        assert_eq!(shadow.argb(8, 8) >> 24, 0);
    }

    #[test]
    fn shapes_have_a_dark_rim_and_white_middle() {
        for shape in [
            Shape::Circle,
            Shape::Star,
            Shape::Heart,
            Shape::Arrow,
            Shape::Cross,
        ] {
            let image = paint_shape(shape, 32);
            let middle = image.argb(16, 17);
            assert_eq!(middle & 0xFF_FFFF, 0xFF_FFFF, "{shape:?}");
            assert_eq!(image.argb(0, 0) >> 24, 0, "{shape:?}");
        }
    }
}
