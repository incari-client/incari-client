//! The waypoints over the world, drawn every frame (WaypointOverlay.java): on screen, the icon in
//! place with a glass tag of name and distance above; off screen, an arrow at the screen's edge
//! pointing the way to turn, beside the icon and distance. Farther ones are drawn first, so nearer
//! tags are on top; one the player stands at isn't drawn. Beams are drawn on the HUD too, under
//! the icons (Java draws the game's beacon beam in the world: Horizon has no hook for one yet).
//!
//! A hot path: the buffers grow only when there are more waypoints than before, and a distance's
//! text is made again only when the number it says changes.
use crate::canvas::{Image, Key};
use crate::framework::setting::Settings;
use crate::paint::{Painter, Shape};
use crate::waypoint::{Player, Waypoint, Waypoints};
use std::collections::HashMap;

/// An icon's size over the world at Size 1, in GUI units.
pub const ICON: f32 = 9.0;
/// How far in from the screen's edge the arrows sit.
pub const EDGE_MARGIN: f32 = 22.0;
/// How far up from the screen's bottom: above the hotbar, hearts and hunger.
pub const BOTTOM_MARGIN: f32 = 62.0;
/// The least room between two edge arrows on the left or right side, at Size 1.
pub const EDGE_GAP_SIDE: f32 = 16.0;
/// And along the top or bottom, where each one's distance is written beside the next.
pub const EDGE_GAP_ACROSS: f32 = 36.0;
/// Within this many blocks across (and 3 up or down), the player is at the waypoint.
pub const HERE: f64 = 1.5;
/// With "When looked at", a waypoint this near the crosshair (GUI units at Size 1) shows its name.
pub const LOOK_RADIUS: f32 = 36.0;
/// A HUD beam's height above its base, in blocks: past the top of any world.
pub const BEAM_RISE: f64 = 384.0;
/// A HUD beam's width in blocks (a beacon beam's glow), and its thinnest and widest on screen.
const BEAM_BLOCKS: f32 = 0.5;
const BEAM_MIN: f32 = 1.0;
const BEAM_MAX: f32 = 14.0;

/// When a waypoint's name and distance are written beside its icon (WaypointsModule.Labels).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Labels {
    Always,
    LookedAt,
    Never,
}

/// The Waypoints module's settings, as the overlay reads them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub labels: Labels,
    pub distance: bool,
    pub beams: bool,
    pub arrows: bool,
    pub size: f32,
}

impl Look {
    /// From the module's settings (the Java ids).
    pub fn of(s: &Settings) -> Self {
        Self {
            labels: match s.choice("labels") {
                "looked_at" | "looked-at" => Labels::LookedAt,
                "never" => Labels::Never,
                _ => Labels::Always,
            },
            distance: s.bool("distance"),
            beams: s.bool("beams"),
            arrows: s.bool("arrows"),
            size: s.float("size").clamp(0.5, 2.0),
        }
    }
}

/// What the game gives the overlay each frame.
pub struct View<'a> {
    pub player: &'a Player,
    /// The window in GUI units.
    pub screen: [f32; 2],
    /// Where a world point shows on the HUD, in GUI units; None behind the camera
    /// (Horizon's `native_project`).
    pub project: &'a dyn Fn([f64; 3]) -> Option<[f32; 2]>,
}

/// A waypoint's distance as text, made again only when what it says changes.
#[derive(Clone, Debug)]
pub struct DistanceText {
    said: i32,
    text: String,
}

impl Default for DistanceText {
    fn default() -> Self {
        Self { said: i32::MIN, text: String::new() }
    }
}

impl DistanceText {
    /// "12 m", "999 m", "1.2 km", "34.5 km".
    pub fn of(&mut self, blocks: f32) -> &str {
        let key = if blocks < 1000.0 { blocks as i32 } else { -((blocks / 100.0) as i32) };
        if key != self.said {
            self.said = key;
            self.text = if key >= 0 {
                format!("{key} m")
            } else {
                format!("{}.{} km", -key / 10, -key % 10)
            };
        }
        &self.text
    }
}

/// One waypoint this frame.
#[derive(Clone, Copy, Debug, Default)]
struct Shown {
    index: usize,
    at: [f32; 2],
    distance: f32,
    on_screen: bool,
    /// The way to it from the screen's middle, a unit step (for the edge arrows).
    way: [f32; 2],
}

/// One off-screen waypoint's place along the edges.
#[derive(Clone, Copy, Debug, Default)]
struct Edge {
    along: f32,
    way: [f32; 2],
    shown: usize,
}

/// The rectangle the edge arrows sit on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edges {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

impl Edges {
    /// For a screen `[w, h]` at Size `scale`: inset, and kept above the hotbar and hearts.
    pub fn new(screen: [f32; 2], scale: f32) -> Self {
        let margin = EDGE_MARGIN * scale;
        let (left, top) = (margin, margin);
        Self {
            left,
            right: (screen[0] - margin).max(left + 1.0),
            top,
            bottom: (screen[1] - margin.max(BOTTOM_MARGIN)).max(top + 1.0),
        }
    }

    fn width(&self) -> f32 {
        self.right - self.left
    }

    fn height(&self) -> f32 {
        self.bottom - self.top
    }

    /// Where the line from `center` along unit `way` meets the rectangle.
    pub fn hit(&self, center: [f32; 2], way: [f32; 2]) -> [f32; 2] {
        let reach_x = if way[0].abs() < 1e-4 {
            f32::MAX
        } else {
            ((if way[0] > 0.0 { self.right } else { self.left }) - center[0]) / way[0]
        };
        let reach_y = if way[1].abs() < 1e-4 {
            f32::MAX
        } else {
            ((if way[1] > 0.0 { self.bottom } else { self.top }) - center[1]) / way[1]
        };
        let reach = reach_x.min(reach_y).max(0.0);
        [center[0] + way[0] * reach, center[1] + way[1] * reach]
    }

    /// How far round the rectangle `p` on it is, clockwise from its top left.
    pub fn along(&self, p: [f32; 2]) -> f32 {
        let (w, h) = (self.width(), self.height());
        let [x, y] = p;
        if y <= self.top + 0.5 {
            x - self.left
        } else if x >= self.right - 0.5 {
            w + (y - self.top)
        } else if y >= self.bottom - 0.5 {
            w + h + (self.right - x)
        } else {
            2.0 * w + h + (self.bottom - y)
        }
    }

    /// Whether the place `along` the edges is on the left or right side.
    pub fn on_side(&self, along: f32) -> bool {
        let (w, h) = (self.width(), self.height());
        let at = along % (2.0 * (w + h));
        (at > w && at < w + h) || at > 2.0 * w + h
    }

    /// The point `along` the edges.
    pub fn point(&self, along: f32) -> [f32; 2] {
        let (w, h) = (self.width(), self.height());
        let at = along % (2.0 * (w + h));
        if at < w {
            [self.left + at, self.top]
        } else if at < w + h {
            [self.right, self.top + (at - w)]
        } else if at < 2.0 * w + h {
            [self.right - (at - w - h), self.bottom]
        } else {
            [self.left, self.bottom - (at - 2.0 * w - h)]
        }
    }
}

/// Spreads places along the edges (sorted) so neighbours keep `gap(previous)` apart, in order.
pub fn spread(along: &mut [f32], gap: impl Fn(f32) -> f32) {
    for k in 1..along.len() {
        along[k] = along[k].max(along[k - 1] + gap(along[k - 1]));
    }
}

/// The way to a point behind the camera, from the player's facing: straight behind points down,
/// the way to turn round.
pub fn way_behind(player: &Player, target: [f64; 3]) -> [f32; 2] {
    let bearing = (target[0] - player.x).atan2(-(target[2] - player.z)).to_degrees() as f32;
    let turn = (bearing - player.heading()).to_radians();
    let (dx, dy) = (turn.sin(), -turn.cos());
    if dx.abs() < 1e-3 { [0.0, 1.0] } else { [dx, dy] }
}

/// Whether the player stands at `w`: drawn over their view, it would only be in the way.
pub fn is_here(w: &Waypoint, player: &Player) -> bool {
    let dx = w.x as f64 + 0.5 - player.x;
    let dz = w.z as f64 + 0.5 - player.z;
    let dy = w.y as f64 - player.y;
    dx * dx + dz * dz < HERE * HERE && dy.abs() < 3.0
}

/// The overlay's buffers and texts, kept between frames.
#[derive(Default)]
pub struct Overlay {
    shown: Vec<Shown>,
    edges: Vec<Edge>,
    along: Vec<f32>,
    texts: HashMap<u64, DistanceText>,
    texts_revision: u64,
}

impl Overlay {
    /// Draws the world's waypoints over the HUD.
    pub fn draw(&mut self, p: &mut Painter, store: &Waypoints, look: &Look, view: &View) {
        let player = view.player;
        if store.revision() != self.texts_revision {
            // Forget the texts of waypoints that are gone.
            self.texts_revision = store.revision();
            self.texts.retain(|id, _| store.get(*id).is_some());
        }
        let [w, h] = view.screen;
        let center = [w / 2.0, h / 2.0];
        self.shown.clear();
        for (index, wp) in store.all().iter().enumerate() {
            if !player.sees(wp) || is_here(wp, player) {
                continue;
            }
            let point = [wp.x as f64 + 0.5, wp.y as f64 + 0.5, wp.z as f64 + 0.5];
            let projected = (view.project)(point);
            let distance = wp.distance_squared(player.x, player.y, player.z).sqrt() as f32;
            let (at, on_screen, way) = match projected {
                Some(at) => {
                    let inside = at[0] >= 0.0 && at[0] <= w && at[1] >= 0.0 && at[1] <= h;
                    let (dx, dy) = (at[0] - center[0], at[1] - center[1]);
                    let length = (dx * dx + dy * dy).sqrt();
                    let way = if length < 1e-3 { [0.0, 1.0] } else { [dx / length, dy / length] };
                    (at, inside, way)
                }
                None => ([0.0, 0.0], false, way_behind(player, point)),
            };
            // By distance, farthest first: a handful of waypoints, no sort's allocation.
            let at_index = self.shown.iter().position(|s| s.distance < distance).unwrap_or(self.shown.len());
            self.shown.insert(at_index, Shown { index, at, distance, on_screen, way });
        }
        let scale = look.size;
        if look.beams {
            for s in &self.shown {
                let wp = &store.all()[s.index];
                if wp.beam {
                    beam(p, wp, view);
                }
            }
        }
        for i in 0..self.shown.len() {
            let s = self.shown[i];
            if s.on_screen {
                let wp = &store.all()[s.index];
                self.on_screen(p, wp, s, look, view.screen, scale);
            }
        }
        if look.arrows {
            self.at_edges(p, store, look, view.screen, scale);
        }
    }

    fn on_screen(&mut self, p: &mut Painter, wp: &Waypoint, s: Shown, look: &Look, screen: [f32; 2], scale: f32) {
        let named = match look.labels {
            Labels::Always => true,
            Labels::Never => false,
            Labels::LookedAt => {
                let (dx, dy) = (s.at[0] - screen[0] / 2.0, s.at[1] - screen[1] / 2.0);
                let r = LOOK_RADIUS * scale;
                dx * dx + dy * dy < r * r
            }
        };
        let th = p.theme;
        let text = self.texts.entry(wp.id()).or_default();
        p.push(s.at[0], s.at[1], scale, |p| {
            p.shape(wp.icon, 0.0, 0.0, ICON, wp.argb());
            if !named {
                return;
            }
            let name = wp.name();
            let far = if look.distance { text.of(s.distance) } else { "" };
            let name_w = p.width(name).round();
            let far_w = if far.is_empty() { 0.0 } else { p.width(far).round() };
            let width = 8.0 + name_w + if far_w == 0.0 { 0.0 } else { 5.0 + far_w };
            let height = crate::theme::LINE + 4.0;
            let left = -(width / 2.0).trunc();
            let top = -(ICON / 2.0).round() - 3.0 - height;
            p.round(left, top, width, height, th.radius, th.background);
            p.text(name, left + 4.0, top + 3.0, th.text, true);
            if far_w > 0.0 {
                p.text(far, left + 4.0 + name_w + 5.0, top + 3.0, th.muted, true);
            }
        });
    }

    /// For each waypoint off screen, an arrow at the edge pointing the way to it, with its icon and
    /// distance just inside; arrows that would overlap are spread along the edge, in order.
    fn at_edges(&mut self, p: &mut Painter, store: &Waypoints, look: &Look, screen: [f32; 2], scale: f32) {
        let edges = Edges::new(screen, scale);
        let center = [screen[0] / 2.0, screen[1] / 2.0];
        self.edges.clear();
        for (i, s) in self.shown.iter().enumerate() {
            if s.on_screen {
                continue;
            }
            let along = edges.along(edges.hit(center, s.way));
            let at = self.edges.iter().position(|e| e.along > along).unwrap_or(self.edges.len());
            self.edges.insert(at, Edge { along, way: s.way, shown: i });
        }
        self.along.clear();
        self.along.extend(self.edges.iter().map(|e| e.along));
        spread(&mut self.along, |prev| {
            (if edges.on_side(prev) { EDGE_GAP_SIDE } else { EDGE_GAP_ACROSS }) * scale
        });
        let th = p.theme;
        for k in 0..self.edges.len() {
            let e = self.edges[k];
            let s = self.shown[e.shown];
            let wp = &store.all()[s.index];
            let [x, y] = edges.point(self.along[k]);
            let [dx, dy] = e.way;
            let turn = dx.atan2(-dy).to_degrees();
            let text = self.texts.entry(wp.id()).or_default();
            p.push(x, y, scale, |p| {
                turned_shape(p, Shape::Arrow, dx * 7.0, dy * 7.0, 8.0, turn, wp.argb());
                p.shape(wp.icon, -dx * 4.0, -dy * 4.0, ICON - 2.0, wp.argb());
                if look.distance && look.labels != Labels::Never {
                    let far = text.of(s.distance);
                    let far_w = p.width(far);
                    let line = crate::theme::LINE;
                    // Inside the icon, on the side toward the middle, centred on the line to it.
                    let tx = (-dx * 14.0 - far_w / 2.0 - dx * far_w / 2.0).round();
                    let ty = (-dy * 14.0 - line / 2.0 - dy * line / 2.0).round();
                    p.text(far, tx, ty, th.text, true);
                }
            });
        }
    }
}

/// A beam rising from `wp`, on the HUD: a band from its base up past the world's top, as wide as a
/// beacon's glow is where it stands, never thinner than a unit so a far one still shows.
fn beam(p: &mut Painter, wp: &Waypoint, view: &View) {
    let base = [wp.x as f64 + 0.5, wp.y as f64, wp.z as f64 + 0.5];
    let (Some(b), Some(up), Some(t)) = (
        (view.project)(base),
        (view.project)([base[0], base[1] + 1.0, base[2]]),
        (view.project)([base[0], base[1] + BEAM_RISE, base[2]]),
    ) else {
        return;
    };
    let per_block = ((up[0] - b[0]).powi(2) + (up[1] - b[1]).powi(2)).sqrt();
    let width = (per_block * BEAM_BLOCKS).clamp(BEAM_MIN, BEAM_MAX);
    let argb = crate::color::alpha(wp.argb(), 0.45);
    band(p, b, t, width, argb);
    band(p, b, t, (width * 0.4).max(BEAM_MIN * 0.75), crate::color::alpha(wp.argb(), 0.8));
}

/// A band `width` wide from `from` to `to`, GUI units (not the painter's origin: beams are placed
/// in the world, not by a HUD element).
pub fn band(p: &mut Painter, from: [f32; 2], to: [f32; 2], width: f32, argb: u32) {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    let length = (dx * dx + dy * dy).sqrt();
    if length < 1e-3 || argb >> 24 == 0 {
        return;
    }
    let (nx, ny) = (-dy / length * width / 2.0, dx / length * width / 2.0);
    let Some(r) = white(p) else {
        return;
    };
    let corners = [
        [from[0] + nx, from[1] + ny],
        [to[0] + nx, to[1] + ny],
        [to[0] - nx, to[1] - ny],
        [from[0] - nx, from[1] - ny],
    ];
    let argb = p.faded(argb);
    p.list.quad(r, corners, argb);
}

/// A white pixel of the canvas, as a draw's source.
fn white(p: &mut Painter) -> Option<[f32; 4]> {
    let r = p.canvas.get(Key::White, || {
        let mut image = Image::new(4, 4);
        for y in 0..4 {
            for x in 0..4 {
                image.set_argb(x, y, 0xFFFF_FFFF);
            }
        }
        image
    })?;
    // Its middle: whatever size it was made at, white there.
    Some(r.part(r.w as f32 / 2.0 - 0.5, r.h as f32 / 2.0 - 0.5, 1.0, 1.0))
}

/// `shape` centred on (`cx`, `cy`) and turned `degrees` clockwise (its tip up at 0), `size` GUI
/// units across. The painter draws shapes upright only, so this draws its image as a turned quad.
pub fn turned_shape(p: &mut Painter, shape: Shape, cx: f32, cy: f32, size: f32, degrees: f32, argb: u32) {
    if degrees.abs() < 0.01 {
        p.shape(shape, cx, cy, size, argb);
        return;
    }
    let pixels = ((size * p.zoom * p.scale).round() as u32).min(128);
    if pixels < 2 || argb >> 24 == 0 {
        return;
    }
    let key = Key::Shape(shape, pixels as u16);
    if !p.canvas.has(key) {
        // Painted by the painter, far off the screen, so its image is on the canvas.
        let origin = p.origin;
        p.origin = [-10_000.0, -10_000.0];
        p.shape(shape, 0.0, 0.0, size, argb);
        p.origin = origin;
        if !p.canvas.has(key) {
            return;
        }
    }
    let Some(r) = p.canvas.get(key, || Image::new(pixels, pixels)) else {
        return;
    };
    let [x, y, ..] = p.place(cx, cy, 0.0, 0.0);
    let half = pixels as f32 / 2.0 / p.scale;
    let (sin, cos) = degrees.to_radians().sin_cos();
    let turn = |u: f32, v: f32| [x + u * cos - v * sin, y + u * sin + v * cos];
    let corners = [turn(-half, -half), turn(half, -half), turn(half, half), turn(-half, half)];
    let argb = p.faded(argb);
    p.list.quad(r.src(), corners, argb);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_distances_as_java_does() {
        let mut t = DistanceText::default();
        assert_eq!(t.of(12.7), "12 m");
        assert_eq!(t.of(999.9), "999 m");
        assert_eq!(t.of(1234.0), "1.2 km");
        assert_eq!(t.of(34_567.0), "34.5 km");
    }

    #[test]
    fn walks_round_the_edges_clockwise() {
        let e = Edges::new([400.0, 300.0], 1.0);
        assert_eq!(e, Edges { left: 22.0, right: 378.0, top: 22.0, bottom: 238.0 });
        let c = [200.0, 150.0];
        // Up, right, down, left of the middle.
        let up = e.along(e.hit(c, [0.0, -1.0]));
        let right = e.along(e.hit(c, [1.0, 0.0]));
        let down = e.along(e.hit(c, [0.0, 1.0]));
        let left = e.along(e.hit(c, [-1.0, 0.0]));
        assert!(up < right && right < down && down < left);
        assert!(!e.on_side(up) && e.on_side(right) && !e.on_side(down) && e.on_side(left));
        for a in [up, right, down, left] {
            let p = e.point(a);
            assert!((e.along(p) - a).abs() < 0.01);
        }
        assert_eq!(e.hit(c, [0.0, 1.0]), [200.0, 238.0]);
    }

    #[test]
    fn spreads_crowded_arrows_in_order() {
        let mut along = [10.0, 12.0, 13.0, 100.0];
        spread(&mut along, |_| 16.0);
        assert_eq!(along, [10.0, 26.0, 42.0, 100.0]);
    }

    #[test]
    fn points_the_way_to_turn_for_what_is_behind() {
        // Facing north (yaw 180): something due south is straight behind, east is to the right.
        let p = Player { x: 0.0, y: 0.0, z: 0.0, yaw: 180.0, dimension: String::new() };
        let behind = way_behind(&p, [0.0, 0.0, 10.0]);
        assert!((behind[0]).abs() < 1e-3 && behind[1] > 0.99);
        let east = way_behind(&p, [10.0, 0.0, 0.0]);
        assert!(east[0] > 0.99);
    }

    #[test]
    fn leaves_out_the_one_the_player_stands_at() {
        let p = Player { x: 10.4, y: 64.0, z: 10.6, yaw: 0.0, dimension: String::new() };
        let w = |x, y| Waypoint::new("w", x, y, 10, "", 0, Shape::Circle);
        assert!(is_here(&w(10, 65), &p));
        assert!(!is_here(&w(10, 68), &p));
        assert!(!is_here(&w(13, 64), &p));
    }
}
