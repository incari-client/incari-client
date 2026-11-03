//! Drawing a HUD element (framework's HudCanvas, ModuleHud.java): an element draws at its own
//! origin and scale, wherever the player placed it; the canvas gives it the theme's tones, the
//! look's glass behind text, items and shapes. `text_element` is the framework's TextElement: one
//! line on the glass, with the element's background and colour settings.
use crate::framework::setting::Settings;
use crate::paint::{Painter, Shape};
use crate::theme::Tone;

/// Space between the glass's edge and a text element's text, across and down.
pub const PAD_X: f32 = 4.0;
pub const PAD_Y: f32 = 3.0;
/// A line of text's height.
pub const LINE: f32 = 9.0;

/// What an element draws on, at its own origin.
pub struct HudCanvas<'a, 'p> {
    pub p: &'a mut Painter<'p>,
    /// The element's own settings (anchor, scale, and its own).
    pub element: &'a Settings,
    /// The module's settings.
    pub module: &'a Settings,
    /// In the HUD editor: an element with nothing to say shows a sample.
    pub editing: bool,
    /// The window in GUI units.
    pub screen: [f32; 2],
}

impl HudCanvas<'_, '_> {
    pub fn text_width(&self, text: &str) -> f32 {
        self.p.width(text)
    }

    /// A line of text in a tone, with the HUD's shadow.
    pub fn text(&mut self, text: &str, x: f32, y: f32, tone: Tone) {
        let argb = self.p.theme.tone(tone);
        self.p.text(text, x, y, argb, true);
    }

    pub fn text_argb(&mut self, text: &str, x: f32, y: f32, argb: u32) {
        self.p.text(text, x, y, argb, true);
    }

    /// A rectangle in a tone: the background tone is the look's tinted glass, rounded.
    pub fn fill(&mut self, x: f32, y: f32, w: f32, h: f32, tone: Tone) {
        let th = self.p.theme;
        if tone == Tone::Background {
            self.p.round(x, y, w, h, th.radius, th.background);
        } else {
            self.p.fill(x, y, w, h, th.tone(tone));
        }
    }

    pub fn fill_argb(&mut self, x: f32, y: f32, w: f32, h: f32, argb: u32) {
        self.p.fill(x, y, w, h, argb);
    }

    pub fn round(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, argb: u32) {
        self.p.round(x, y, w, h, radius, argb);
    }

    /// An item's icon, 16 square.
    pub fn item(&mut self, id: &str, x: f32, y: f32) {
        self.p.item(id, x, y, 16.0);
    }

    /// A sprite of the game's atlas (an effect's icon), `w` by `h`.
    pub fn sprite(&mut self, name: &str, x: f32, y: f32, w: f32, h: f32) {
        self.p
            .sprite(name, [0.0, 0.0, w, h], [x, y, w, h], 0xFFFF_FFFF);
    }

    /// A ring around (`cx`, `cy`), `radius` to its outer edge, `width` thick.
    pub fn ring(&mut self, cx: f32, cy: f32, radius: f32, width: f32, argb: u32) {
        self.p.ring(cx, cy, radius, width, argb);
    }

    /// A filled circle.
    pub fn disc(&mut self, cx: f32, cy: f32, radius: f32, argb: u32) {
        self.p.ring(cx, cy, radius, radius, argb);
    }

    /// Text `scale` times its size.
    pub fn text_scaled(&mut self, text: &str, x: f32, y: f32, argb: u32, scale: f32) {
        self.p.text_scaled(text, x, y, argb, true, scale);
    }

    /// Part `src` of a sprite of the game's over `dst`.
    pub fn sprite_part(&mut self, name: &str, src: [f32; 4], dst: [f32; 4], argb: u32) {
        self.p.sprite(name, src, dst, argb);
    }

    pub fn shape(&mut self, shape: Shape, cx: f32, cy: f32, size: f32, argb: u32) {
        self.p.shape(shape, cx, cy, size, argb);
    }

    /// The element's colour: its custom one when it has `custom-color` on, else the tone's.
    pub fn color(&self, tone: Tone) -> u32 {
        if self.element.bool("custom-color") {
            self.element.color("color")
        } else {
            self.p.theme.tone(tone)
        }
    }

    /// One line of text on the glass (TextElement): returns its size. `widest` holds its width
    /// still as the numbers change.
    pub fn text_element(&mut self, text: &str, widest: Option<&str>, tone: Tone) -> [f32; 2] {
        let background = self.element.bool("background");
        let pad = if background { PAD_X * 2.0 } else { 0.0 };
        let mut width = self.p.width(text);
        if let Some(widest) = widest {
            width = width.max(self.p.width(widest));
        }
        let w = width + pad;
        // The font's line height has a pixel of space below the letters: one pad less below.
        let h = LINE + if background { PAD_Y * 2.0 - 1.0 } else { 0.0 };
        let (mut x, mut y) = (0.0, 0.0);
        if background {
            self.fill(0.0, 0.0, w, h, Tone::Background);
            x = PAD_X;
            y = PAD_Y;
        }
        // Centred in the width `widest` holds.
        x += ((width - self.p.width(text)) / 2.0).floor();
        let argb = self.color(tone);
        self.p.text(text, x, y, argb, true);
        [w, h]
    }
}
