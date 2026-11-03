//! The client's line icons (Icons.java): each a few strokes and shapes described as distances,
//! painted anti-aliased at the size they're shown at in screen pixels, white, tinted when drawn.
use crate::canvas::Image;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    /// Singleplayer: a block.
    Cube,
    /// Multiplayer: the world.
    Globe,
    /// Modules.
    Grid,
    /// Options.
    Gear,
    /// A step from what goes in to what comes out.
    Arrow,
    /// The HUD's layout: a screen's frame with two panels placed in it.
    Layout,
    Plus,
    Pencil,
    Trash,
    Eye,
    EyeOff,
    /// Where the player is.
    Target,
    /// Screenshots.
    Camera,
    /// Themes.
    Palette,
    Chat,
    Check,
    Close,
    /// A circle struck through.
    Block,
    /// Report.
    Flag,
    Person,
    PersonAdd,
    Send,
    /// Cosmetics, the wardrobe: a coat hanger.
    Hanger,
    /// Incari's mark, in its own colours: drawn from its image (`Painter::logo`), not painted here.
    Incari,
}

/// The largest icon painted, in pixels.
pub const MAX_PIXELS: u32 = 128;

impl Icon {
    /// The icon `pixels` square, white with coverage in alpha.
    pub fn paint(self, pixels: u32) -> Image {
        // Strokes at least a pixel and a third wide, however small the icon.
        let stroke = (1.3 / pixels as f32).max(0.075);
        Image::mask(pixels, pixels, |px, py| {
            let mut inside = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    let u = (px as f32 + (sx as f32 + 0.5) / 4.0) / pixels as f32;
                    let v = (py as f32 + (sy as f32 + 0.5) / 4.0) / pixels as f32;
                    if self.distance(u, v, stroke) < 0.0 {
                        inside += 1;
                    }
                }
            }
            inside as f32 / 16.0
        })
    }

    /// The icon's shape as a distance from (`u`, `v`) in the unit square: negative inside.
    fn distance(self, u: f32, v: f32, stroke: f32) -> f32 {
        let (x, y) = (u - 0.5, v - 0.5);
        match self {
            Icon::Cube => {
                let p = [
                    0.0, -0.42, 0.37, -0.21, 0.37, 0.21, 0.0, 0.42, -0.37, 0.21, -0.37, -0.21,
                ];
                let mut d = f32::MAX;
                for i in 0..6 {
                    let j = (i + 1) % 6;
                    d = d.min(segment(
                        x,
                        y,
                        p[i * 2],
                        p[i * 2 + 1],
                        p[j * 2],
                        p[j * 2 + 1],
                    ));
                }
                d = d.min(segment(x, y, 0.0, 0.0, -0.37, -0.21));
                d = d.min(segment(x, y, 0.0, 0.0, 0.37, -0.21));
                d = d.min(segment(x, y, 0.0, 0.0, 0.0, 0.42));
                d - stroke * 0.5
            }
            Icon::Globe => {
                let ring = (length(x, y) - 0.4).abs();
                let meridian = (length(x / 0.45, y) - 0.4).abs() * 0.45;
                let equator = segment(x, y, -0.4, 0.0, 0.4, 0.0);
                ring.min(meridian.min(equator)) - stroke * 0.5
            }
            Icon::Grid => {
                let mut d = f32::MAX;
                for i in 0..4 {
                    let cx = if i % 2 == 0 { -0.22 } else { 0.22 };
                    let cy = if i < 2 { -0.22 } else { 0.22 };
                    d = d.min(rbox(x - cx, y - cy, 0.16, 0.16, 0.06));
                }
                d
            }
            Icon::Gear => {
                let r = length(x, y);
                let body = (r - 0.31).max(0.12 - r);
                let mut teeth = f32::MAX;
                for i in 0..8 {
                    let angle = std::f32::consts::FRAC_PI_4 * i as f32;
                    let (ax, ay) = (angle.cos(), angle.sin());
                    teeth = teeth
                        .min(segment(x, y, ax * 0.25, ay * 0.25, ax * 0.42, ay * 0.42) - 0.065);
                }
                body.min(teeth).max(0.12 - r)
            }
            Icon::Arrow => {
                let shaft = segment(x, y, -0.38, 0.0, 0.34, 0.0);
                let upper = segment(x, y, 0.38, 0.0, 0.12, -0.24);
                let lower = segment(x, y, 0.38, 0.0, 0.12, 0.24);
                shaft.min(upper.min(lower)) - stroke * 0.6
            }
            Icon::Layout => {
                let frame = rbox(x, y, 0.44, 0.36, 0.1).abs() - stroke * 0.5;
                let upper = rbox(x + 0.15, y + 0.15, 0.16, 0.075, 0.04);
                let lower = rbox(x - 0.13, y - 0.15, 0.18, 0.075, 0.04);
                frame.min(upper.min(lower))
            }
            Icon::Plus => {
                segment(x, y, -0.34, 0.0, 0.34, 0.0).min(segment(x, y, 0.0, -0.34, 0.0, 0.34))
                    - stroke * 0.65
            }
            Icon::Pencil => {
                let body = segment(x, y, -0.16, 0.16, 0.3, -0.3) - 0.1;
                let tip = segment(x, y, -0.4, 0.4, -0.2, 0.2) - stroke * 0.6;
                // A notch across the body: where the cap meets it.
                let cap = segment(x, y, 0.1, -0.28, 0.28, -0.1) - stroke * 0.35;
                body.max(-cap).min(tip)
            }
            Icon::Trash => {
                let can = rbox(x, y - 0.1, 0.26, 0.32, 0.06).abs() - stroke * 0.5;
                let lid = segment(x, y, -0.38, -0.3, 0.38, -0.3) - stroke * 0.55;
                let handle = segment(x, y, -0.1, -0.41, 0.1, -0.41) - stroke * 0.55;
                let lines = segment(x, y, -0.09, -0.06, -0.09, 0.26)
                    .min(segment(x, y, 0.09, -0.06, 0.09, 0.26))
                    - stroke * 0.45;
                can.min(lid).min(handle.min(lines))
            }
            Icon::Eye => eye(x, y, stroke),
            Icon::EyeOff => {
                let slash = segment(x, y, -0.38, -0.38, 0.38, 0.38) - stroke * 0.55;
                // A gap either side of the slash, so it reads as struck through.
                let gap = segment(x, y, -0.38, -0.38, 0.38, 0.38) - stroke * 1.6;
                eye(x, y, stroke).max(-gap).min(slash)
            }
            Icon::Target => {
                let ring = (length(x, y) - 0.26).abs() - stroke * 0.5;
                let ticks = segment(x, y, 0.0, -0.46, 0.0, -0.3)
                    .min(segment(x, y, 0.0, 0.3, 0.0, 0.46))
                    .min(segment(x, y, -0.46, 0.0, -0.3, 0.0))
                    .min(segment(x, y, 0.3, 0.0, 0.46, 0.0))
                    - stroke * 0.5;
                ring.min(ticks).min(length(x, y) - 0.07)
            }
            Icon::Camera => {
                let body = rbox(x, y - 0.05, 0.42, 0.27, 0.07).abs() - stroke * 0.5;
                let lens = (length(x, y - 0.05) - 0.14).abs() - stroke * 0.5;
                let hump = rbox(x - 0.13, y + 0.27, 0.11, 0.05, 0.03);
                body.min(lens).min(hump)
            }
            Icon::Chat => {
                let bubble = rbox(x, y + 0.06, 0.4, 0.27, 0.12).abs() - stroke * 0.5;
                // The tail, cut out of the bubble's lower edge and drawn as two strokes.
                let tail = segment(x, y, -0.2, 0.33, -0.28, 0.46)
                    .min(segment(x, y, -0.28, 0.46, -0.02, 0.33))
                    - stroke * 0.5;
                bubble.min(tail)
            }
            Icon::Check => {
                segment(x, y, -0.34, 0.02, -0.1, 0.26).min(segment(x, y, -0.1, 0.26, 0.36, -0.26))
                    - stroke * 0.7
            }
            Icon::Close => {
                segment(x, y, -0.28, -0.28, 0.28, 0.28).min(segment(x, y, -0.28, 0.28, 0.28, -0.28))
                    - stroke * 0.7
            }
            Icon::Block => {
                let ring = (length(x, y) - 0.38).abs() - stroke * 0.55;
                let slash = segment(x, y, -0.27, -0.27, 0.27, 0.27) - stroke * 0.55;
                ring.min(slash)
            }
            Icon::Flag => {
                let pole = segment(x, y, -0.3, -0.42, -0.3, 0.44) - stroke * 0.55;
                let p = [
                    -0.3, -0.36, 0.34, -0.36, 0.18, -0.16, 0.34, 0.04, -0.3, 0.04,
                ];
                let mut d = f32::MAX;
                for i in 0..4 {
                    d = d.min(segment(
                        x,
                        y,
                        p[i * 2],
                        p[i * 2 + 1],
                        p[i * 2 + 2],
                        p[i * 2 + 3],
                    ));
                }
                pole.min(d - stroke * 0.5)
            }
            Icon::Person => person(x, y, stroke),
            Icon::Incari => f32::MAX,
            Icon::Hanger => {
                // A hook over a wide triangle, its bottom bar straight.
                let hook = if x < 0.0 && y > -0.27 {
                    f32::MAX
                } else {
                    (length(x, y + 0.27) - 0.1).abs() - stroke * 0.5
                };
                let neck = segment(x, y, 0.0, -0.17, 0.0, -0.1) - stroke * 0.5;
                let left = segment(x, y, 0.0, -0.1, -0.42, 0.22);
                let right = segment(x, y, 0.0, -0.1, 0.42, 0.22);
                let bar = segment(x, y, -0.42, 0.22, 0.42, 0.22);
                hook.min(neck).min(left.min(right).min(bar) - stroke * 0.55)
            }
            Icon::PersonAdd => {
                let body = person(x + 0.12, y, stroke);
                let plus = segment(x, y, 0.2, -0.08, 0.44, -0.08)
                    .min(segment(x, y, 0.32, -0.2, 0.32, 0.04))
                    - stroke * 0.55;
                body.min(plus)
            }
            Icon::Send => {
                let p = [
                    -0.38, -0.34, 0.4, 0.0, -0.38, 0.34, -0.22, 0.0, -0.38, -0.34,
                ];
                let mut d = f32::MAX;
                for i in 0..4 {
                    d = d.min(segment(
                        x,
                        y,
                        p[i * 2],
                        p[i * 2 + 1],
                        p[i * 2 + 2],
                        p[i * 2 + 3],
                    ));
                }
                d = d.min(segment(x, y, -0.22, 0.0, 0.1, 0.0));
                d - stroke * 0.5
            }
            Icon::Palette => {
                let rim = (length(x, y) - 0.38).abs() - stroke * 0.5;
                // The thumb hole, a gap in the rim at the lower right.
                let hole = length(x - 0.2, y - 0.2) - 0.1;
                let wells = length(x + 0.17, y + 0.12)
                    .min(length(x, y + 0.22))
                    .min(length(x - 0.17, y + 0.12))
                    .min(length(x + 0.2, y - 0.1))
                    - 0.065;
                rim.max(-hole).min(wells.min(hole.abs() - stroke * 0.5))
            }
        }
    }

    /// The menu icon a module's title-screen button names (MenuIcons.of).
    pub fn named(name: &str) -> Option<Self> {
        Some(match name {
            "cube" => Icon::Cube,
            "globe" => Icon::Globe,
            "grid" => Icon::Grid,
            "gear" => Icon::Gear,
            "arrow" => Icon::Arrow,
            "layout" => Icon::Layout,
            "camera" => Icon::Camera,
            "palette" => Icon::Palette,
            "chat" => Icon::Chat,
            "person" => Icon::Person,
            "hanger" => Icon::Hanger,
            "target" => Icon::Target,
            "incari" => Icon::Incari,
            _ => return None,
        })
    }
}

/// A person: a head, and shoulders as the top of a ring.
fn person(x: f32, y: f32, stroke: f32) -> f32 {
    let head = (length(x, y + 0.17) - 0.15).abs() - stroke * 0.5;
    let shoulders = ((length(x, y - 0.44) - 0.32).abs()).max(y - 0.4) - stroke * 0.5;
    head.min(shoulders)
}

/// An eye: an almond's outline and its pupil.
fn eye(x: f32, y: f32, stroke: f32) -> f32 {
    let almond = (length(x, y - 0.42) - 0.6).max(length(x, y + 0.42) - 0.6);
    (almond.abs() - stroke * 0.5).min(length(x, y) - 0.12)
}

pub(crate) fn length(x: f32, y: f32) -> f32 {
    (x * x + y * y).sqrt()
}

/// The distance from (x, y) to the segment from a to b.
pub(crate) fn segment(x: f32, y: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let (px, py, dx, dy) = (x - ax, y - ay, bx - ax, by - ay);
    let t = ((px * dx + py * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    length(px - dx * t, py - dy * t)
}

/// The distance from (x, y) to a rounded box of half-size (hx, hy) centred on the origin.
pub(crate) fn rbox(x: f32, y: f32, hx: f32, hy: f32, radius: f32) -> f32 {
    let qx = x.abs() - (hx - radius);
    let qy = y.abs() - (hy - radius);
    length(qx.max(0.0), qy.max(0.0)) + qx.max(qy).min(0.0) - radius
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_paints_something() {
        for icon in [
            Icon::Cube,
            Icon::Globe,
            Icon::Grid,
            Icon::Gear,
            Icon::Arrow,
            Icon::Layout,
            Icon::Plus,
            Icon::Pencil,
            Icon::Trash,
            Icon::Eye,
            Icon::EyeOff,
            Icon::Target,
            Icon::Camera,
            Icon::Palette,
            Icon::Chat,
            Icon::Check,
            Icon::Close,
            Icon::Block,
            Icon::Flag,
            Icon::Person,
            Icon::PersonAdd,
            Icon::Send,
            Icon::Hanger,
        ] {
            let image = icon.paint(18);
            let covered = image.rgba.chunks(4).filter(|p| p[3] > 0).count();
            assert!(covered > 10, "{icon:?} painted {covered} pixels");
        }
    }
}
