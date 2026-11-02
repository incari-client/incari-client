//! The player in 3D wearing a loadout (ADR 0061; `PlayerPreview.java`): the Wardrobe's preview,
//! and the title card's player while they wear something. The Java client draws it with the
//! game's player renderer; here it's drawn on the canvas as `player.rs` draws the title's player:
//! the skin on the model's boxes, the cape, the hat and back models (`models`), faces lit, sorted
//! back to front and projected. Dragged to turn; left alone, it turns to show what was picked last
//! (the back for a cape or wings, the front otherwise). Emotes play on it.
use super::catalog::{Cosmetic, Slot};
use super::loadout::Loadout;
use super::models::{self, Affine, Anchor, Look, Quad};
use super::{emotes, textures};
use crate::canvas::{Key, Region};
use crate::paint::Painter;
use crate::ui::Ui;

/// Blocks of height the preview's height holds: a player is 1.8, a halo or wings stand above.
const BLOCKS: f32 = 2.6;
/// Turned to show the front, a little from the side (degrees).
pub const FRONT: f32 = -28.0;
/// Turned to show the back: a cape, wings.
pub const BACK: f32 = 152.0;
/// The middle of what's shown, from the feet to a halo's top: 1.12 blocks up, in model pixels
/// (the feet are at y = 24, +y down).
const MIDDLE_Y: f32 = 24.0 - 1.12 * 16.0;
/// Looking down at the model a little (radians).
const TILT: f32 = 0.12;
/// After a drag, how long the preview stays where it was left before turning back.
const DRAG_HOLD_MS: f64 = 4_000.0;

/// A box of the player's skin: its part, corner, size, growth and texture position.
type SkinBox<'a> = (&'a Affine, [f32; 3], [i32; 3], f32, [i32; 2]);

/// What a face is painted from.
#[derive(Clone, Copy)]
enum Source {
    Skin,
    /// A cape's outer side (10x16), times a shade.
    Cape(f32),
    /// A hat's or back cosmetic's 64x64 texture; glowing.
    Model(bool),
}

/// One face to draw: its corners on screen, its picture and how near it is.
struct Face {
    corners: [[f32; 2]; 4],
    src: [f32; 4],
    depth: f32,
    argb: u32,
}

/// A part's frame and the parts' poses, as the game's player model sets them up.
struct Rig {
    head: Affine,
    body: Affine,
    right_arm: Affine,
    left_arm: Affine,
    right_leg: Affine,
    left_leg: Affine,
}

pub struct PlayerPreview {
    yaw: f32,
    target: f32,
    dragged_at: Option<f64>,
    dragging: Option<f32>,
    emote: Option<(&'static Cosmetic, f64)>,
    quads: Vec<Quad>,
    faces: Vec<Face>,
}

impl Default for PlayerPreview {
    fn default() -> Self {
        Self { yaw: FRONT, target: FRONT, dragged_at: None, dragging: None, emote: None, quads: Vec::new(), faces: Vec::new() }
    }
}

impl PlayerPreview {
    /// Turns to show `slot` best: capes and wings from behind, the rest from the front.
    pub fn show(&mut self, slot: Slot) {
        self.target = if matches!(slot, Slot::Cape | Slot::Back) { BACK } else { FRONT };
        self.dragged_at = None;
    }

    /// Stands turned as `show` would, without turning there.
    pub fn face(&mut self, slot: Slot) {
        self.show(slot);
        self.yaw = self.target;
    }

    /// Plays `emote` now (`now_ms`, the painter's clock), from the front.
    pub fn play(&mut self, emote: &'static Cosmetic, now_ms: f64) {
        self.emote = Some((emote, now_ms));
        self.target = FRONT;
        self.dragged_at = None;
    }

    /// The emote playing, if any.
    pub fn playing(&self) -> Option<&'static Cosmetic> {
        self.emote.map(|e| e.0)
    }

    /// Draws the player wearing `loadout` in `rect`, turned by a drag inside it.
    pub fn draw(&mut self, p: &mut Painter, ui: &mut Ui, rect: [f32; 4], loadout: Loadout) {
        let now = p.motion.now;
        if self.dragging.is_none() && ui.clicked(rect) {
            self.dragging = Some(ui.mouse[0]);
            self.dragged_at = Some(now);
        }
        if let Some(last) = self.dragging {
            if ui.down {
                self.yaw -= (ui.mouse[0] - last) * 2.5;
                self.target = self.yaw;
                self.dragging = Some(ui.mouse[0]);
                self.dragged_at = Some(now);
            } else {
                self.dragging = None;
            }
        }
        if self.dragged_at.is_none_or(|at| now - at > DRAG_HOLD_MS) {
            self.yaw += (self.target - self.yaw) * 0.12;
        }
        let age = (now / 50.0) as f32;
        let pose = match self.emote {
            Some((e, start)) => {
                let pose = emotes::pose(e.id, ((now - start) / 50.0) as f32);
                if pose.is_none() {
                    self.emote = None;
                }
                pose
            }
            None => None,
        };
        self.render(p, rect, loadout, age, self.yaw.to_radians(), TILT, pose);
    }

    /// Draws the player standing still in `rect`, turned `yaw` degrees (the title card's: facing
    /// the pointer as `player.rs` does).
    pub fn draw_still(&mut self, p: &mut Painter, rect: [f32; 4], loadout: Loadout, yaw: f32, pitch: f32) {
        let age = (p.motion.now / 50.0) as f32;
        self.render(p, rect, loadout, age, yaw.to_radians(), pitch, None);
    }

    #[allow(clippy::too_many_arguments)]
    fn render(&mut self, p: &mut Painter, rect: [f32; 4], loadout: Loadout, age: f32, yaw: f32, tilt: f32, pose: Option<emotes::Pose>) {
        let Some((skin, slim)) = p.assets.skin.as_ref() else {
            return;
        };
        let slim = *slim;
        let Some(skin_region) = p.canvas.get(Key::Skin, || skin.clone()) else {
            return;
        };
        let [x, y, w, h] = rect;
        let unit = (h / (BLOCKS * 16.0)).min(w / (BLOCKS * 8.0));
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        let view = Affine::rotation_x(tilt).then(&Affine::rotation_y(yaw)).then(&Affine::translation([0.0, -MIDDLE_Y, 0.0]));
        let rig = rig(age, slim, pose);
        self.faces.clear();
        let mut quads = std::mem::take(&mut self.quads);

        // The player's own boxes, inner layers then outer.
        let arm = if slim { 3 } else { 4 };
        let right_x = if slim { -2.0 } else { -3.0 };
        let boxes: [SkinBox; 12] = [
            (&rig.head, [-4.0, -8.0, -4.0], [8, 8, 8], 0.0, [0, 0]),
            (&rig.body, [-4.0, 0.0, -2.0], [8, 12, 4], 0.0, [16, 16]),
            (&rig.right_arm, [right_x, -2.0, -2.0], [arm, 12, 4], 0.0, [40, 16]),
            (&rig.left_arm, [-1.0, -2.0, -2.0], [arm, 12, 4], 0.0, [32, 48]),
            (&rig.right_leg, [-2.0, 0.0, -2.0], [4, 12, 4], 0.0, [0, 16]),
            (&rig.left_leg, [-2.0, 0.0, -2.0], [4, 12, 4], 0.0, [16, 48]),
            (&rig.head, [-4.0, -8.0, -4.0], [8, 8, 8], 0.5, [32, 0]),
            (&rig.body, [-4.0, 0.0, -2.0], [8, 12, 4], 0.25, [16, 32]),
            (&rig.right_arm, [right_x, -2.0, -2.0], [arm, 12, 4], 0.25, [40, 32]),
            (&rig.left_arm, [-1.0, -2.0, -2.0], [arm, 12, 4], 0.25, [48, 48]),
            (&rig.right_leg, [-2.0, 0.0, -2.0], [4, 12, 4], 0.25, [0, 32]),
            (&rig.left_leg, [-2.0, 0.0, -2.0], [4, 12, 4], 0.25, [0, 48]),
        ];
        for (part, at, size, grow, tex) in boxes {
            quads.clear();
            models::box_quads(at, size, grow, tex, false, &view.then(part), &mut quads);
            for q in &quads {
                self.push(q, skin_region, Source::Skin, [cx, cy], unit, grow);
            }
        }

        let backpack = loadout.in_slot(Slot::Back).is_some_and(|b| b.id == "back.backpack");
        // The cape hangs from the body's back, swung out a little (the preview's lean is 6).
        let swing = (6.0f32 + 6.0 / 2.0).to_radians();
        if let Some(cape) = loadout.in_slot(Slot::Cape).filter(|_| !backpack)
            && let Some(face) = textures::cape_face(p, cape)
        {
            let m = view.then(&rig.body).then(&Affine::translation([0.0, 0.0, 2.0])).then(&Affine::rotation_x(swing));
            for (q, shade) in cape_quads(&m) {
                self.push(&q, face, Source::Cape(shade), [cx, cy], unit, 0.1);
            }
        }
        let state = models::State {
            age,
            cape_swing: loadout.in_slot(Slot::Cape).filter(|_| !backpack).map(|_| swing),
            ..models::State::default()
        };
        for cosmetic in [loadout.in_slot(Slot::Hat), loadout.in_slot(Slot::Back)].into_iter().flatten() {
            let (Some(spec), Some(region)) = (models::spec(cosmetic.id), textures::model(p, cosmetic)) else {
                continue;
            };
            models::mesh(cosmetic.id, &spec, &state, &mut quads);
            let anchor = if spec.anchor == Anchor::Head { &rig.head } else { &rig.body };
            let m = view.then(anchor);
            let glow = spec.look == Look::Glow;
            for q in &quads {
                let q = Quad { pos: q.pos.map(|c| m.apply(c)), uv: q.uv };
                self.push(&q, region, Source::Model(glow), [cx, cy], unit, 0.2);
            }
        }
        self.quads = quads;

        self.faces.sort_by(|a, b| b.depth.total_cmp(&a.depth));
        for f in &self.faces {
            let argb = p.faded(f.argb);
            p.list.quad(f.src, f.corners.map(|c| {
                let [gx, gy, _, _] = p.place(c[0], c[1], 0.0, 0.0);
                [gx, gy]
            }), argb);
        }
    }

    /// Projects a quad (view space, model pixels) and keeps it when it faces the viewer.
    fn push(&mut self, q: &Quad, region: Region, source: Source, [cx, cy]: [f32; 2], unit: f32, bias: f32) {
        let [a, b, c] = [q.pos[0], q.pos[1], q.pos[2]];
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let mut n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len < 1e-6 {
            // A corner doubled: the other three.
            return;
        }
        n = n.map(|v| v / len);
        // The viewer is at -z: a face turned away isn't drawn.
        if n[2] >= 0.0 {
            return;
        }
        // Lit from above and in front, as the game's entity light falls.
        let light = [0.3f32, -0.8, -0.6];
        let lit = (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]).max(0.0) / 1.044;
        let (shade, glow) = match source {
            Source::Skin => (1.0, false),
            Source::Cape(s) => (s, false),
            Source::Model(g) => (1.0, g),
        };
        let bright = if glow { 1.0 } else { (0.55 + 0.45 * lit) * shade };
        let v = (bright.clamp(0.0, 1.0) * 255.0).round() as u32;
        let argb = 0xFF00_0000 | v << 16 | v << 8 | v;
        let (umin, umax) = (q.uv.iter().map(|t| t[0]).fold(f32::MAX, f32::min), q.uv.iter().map(|t| t[0]).fold(f32::MIN, f32::max));
        let (vmin, vmax) = (q.uv.iter().map(|t| t[1]).fold(f32::MAX, f32::min), q.uv.iter().map(|t| t[1]).fold(f32::MIN, f32::max));
        let project = |p: [f32; 3]| [cx + p[0] * unit, cy + p[1] * unit];
        let mut corners = [[0.0; 2]; 4];
        if umax - umin < 1e-4 || vmax - vmin < 1e-4 {
            // A solid colour: one texel over the whole face, corners as they come.
            for (i, p) in q.pos.iter().enumerate() {
                corners[i] = project(*p);
            }
        } else {
            // The canvas draws a rectangle of its pixels onto four corners: top left, top right,
            // bottom right, bottom left of the rectangle, whichever corners of the quad they are.
            for (p, t) in q.pos.iter().zip(q.uv) {
                let right = (t[0] - umax).abs() < (t[0] - umin).abs();
                let bottom = (t[1] - vmax).abs() < (t[1] - vmin).abs();
                let i = match (right, bottom) {
                    (false, false) => 0,
                    (true, false) => 1,
                    (true, true) => 2,
                    (false, true) => 3,
                };
                corners[i] = project(*p);
            }
        }
        let depth = q.pos.iter().map(|p| p[2]).sum::<f32>() / 4.0 - bias;
        let src = region.part(umin, vmin, (umax - umin).max(1.0), (vmax - vmin).max(1.0));
        self.faces.push(Face { corners, src, depth, argb });
    }
}

thread_local! {
    /// The title card's player while they wear something.
    static TITLE: std::cell::RefCell<PlayerPreview> = std::cell::RefCell::new(PlayerPreview::default());
}

/// The title card's player in `rect` (`TitleProfile.playerView`): wearing what the player wears
/// while they wear anything (`Cosmetics.wearsAny`), else `player::draw`'s. Turned toward the
/// pointer as the inventory's player is.
pub fn draw_title(cx: &mut crate::screen::Cx, rect: [f32; 4]) {
    super::wardrobe::load_and_save(cx.host);
    let worn = super::wardrobe::with(|w| w.worn());
    let mouse = cx.ui.mouse;
    if worn.is_empty() {
        crate::player::draw(cx.p, rect, mouse);
        return;
    }
    let [x, y, w, h] = rect;
    let ax = ((x + w / 2.0 - mouse[0]) / 40.0).atan();
    let ay = ((y + h * 0.15 - mouse[1]) / 40.0).atan();
    // The model's 2.6 blocks hold a halo and wings: a little larger box than `player::draw`'s.
    let grown = [x - w * 0.2, y - h * 0.22, w * 1.4, h * 1.3];
    TITLE.with(|t| t.borrow_mut().draw_still(cx.p, grown, worn, -ax * 20.0, TILT + ay * 0.2));
}

/// The game's player model posed standing at `age` (`AnimationUtils.bobArms`), with an emote's
/// pose over it. Model pixels, +y down, the feet at y = 24.
fn rig(age: f32, slim: bool, pose: Option<emotes::Pose>) -> Rig {
    let part = |at: [f32; 3], r: [f32; 3]| Affine::translation(at).then(&Affine::rotation_zyx(r[0], r[1], r[2]));
    let bob_z = (age * 0.09).cos() * 0.05 + 0.05;
    let bob_x = (age * 0.067).sin() * 0.05;
    let (w, head, right, left) = match pose {
        Some(p) => (p.weight, p.head, p.right_arm, p.left_arm),
        None => (0.0, None, None, None),
    };
    let head = emotes::Pose::blend(w, [0.0; 3], head);
    let right = emotes::Pose::blend(w, [bob_x, 0.0, bob_z], right);
    let left = emotes::Pose::blend(w, [-bob_x, 0.0, -bob_z], left);
    let arm_y = if slim { 2.5 } else { 2.0 };
    Rig {
        head: part([0.0; 3], head),
        body: Affine::IDENTITY,
        right_arm: part([-5.0, arm_y, 0.0], right),
        left_arm: part([5.0, arm_y, 0.0], left),
        right_leg: part([-1.9, 12.0, 0.0], [0.0; 3]),
        left_leg: part([1.9, 12.0, 0.0], [0.0; 3]),
    }
}

/// The cape as a box from its top edge (10 across, 16 down, 1 thick) under `m`: its outer side
/// from the cape's face, the inner side the same in shade, the edges the face's middle colour.
fn cape_quads(m: &Affine) -> Vec<(Quad, f32)> {
    let p = |x: f32, y: f32, z: f32| m.apply([x, y, z]);
    let face = |a, b, c, d, uv: [[f32; 2]; 4], shade: f32| (Quad { pos: [a, b, c, d], uv }, shade);
    // Both sides show the design's left at the player's left (+x), as the game's cape does.
    let middle = [[5.0, 8.0]; 4];
    vec![
        // Outer, facing away from the player (+z).
        face(p(-5.0, 0.0, 1.0), p(5.0, 0.0, 1.0), p(5.0, 16.0, 1.0), p(-5.0, 16.0, 1.0), [[10.0, 0.0], [0.0, 0.0], [0.0, 16.0], [10.0, 16.0]], 1.0),
        // Inner, facing the player's back.
        face(p(5.0, 0.0, 0.0), p(-5.0, 0.0, 0.0), p(-5.0, 16.0, 0.0), p(5.0, 16.0, 0.0), [[0.0, 0.0], [10.0, 0.0], [10.0, 16.0], [0.0, 16.0]], 0.55),
        // The edges: the sides, the bottom and the top.
        face(p(5.0, 0.0, 0.0), p(5.0, 16.0, 0.0), p(5.0, 16.0, 1.0), p(5.0, 0.0, 1.0), middle, 0.7),
        face(p(-5.0, 0.0, 0.0), p(-5.0, 0.0, 1.0), p(-5.0, 16.0, 1.0), p(-5.0, 16.0, 0.0), middle, 0.7),
        face(p(-5.0, 16.0, 0.0), p(-5.0, 16.0, 1.0), p(5.0, 16.0, 1.0), p(5.0, 16.0, 0.0), middle, 0.7),
        face(p(-5.0, 0.0, 0.0), p(5.0, 0.0, 0.0), p(5.0, 0.0, 1.0), p(-5.0, 0.0, 1.0), middle, 0.7),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn showing_a_cape_turns_to_the_back() {
        let mut preview = PlayerPreview::default();
        preview.face(Slot::Cape);
        assert_eq!(preview.yaw, BACK);
        preview.show(Slot::Hat);
        assert_eq!(preview.target, FRONT);
        assert_eq!(preview.yaw, BACK, "turns there as it's drawn");
    }

    #[test]
    fn the_capes_faces_turn_outward() {
        for (q, _) in cape_quads(&Affine::IDENTITY) {
            let [a, b, c] = [q.pos[0], q.pos[1], q.pos[2]];
            let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            let centre = q.pos.iter().fold([0.0f32; 3], |s, p| [s[0] + p[0] / 4.0, s[1] + p[1] / 4.0, s[2] + p[2] / 4.0]);
            let out = [centre[0], centre[1] - 8.0, centre[2] - 0.5];
            assert!(n[0] * out[0] + n[1] * out[1] + n[2] * out[2] > 0.0, "{:?}", q.pos);
        }
    }

    #[test]
    fn the_players_boxes_face_out() {
        // The head's front (the game's north face) faces the viewer at -z.
        let mut quads = Vec::new();
        models::box_quads([-4.0, -8.0, -4.0], [8, 8, 8], 0.0, [0, 0], false, &Affine::IDENTITY, &mut quads);
        let front = &quads[3];
        assert!(front.pos.iter().all(|p| p[2] == -4.0));
        assert_eq!(front.uv[1], [8.0, 8.0], "the face's texture");
    }
}
