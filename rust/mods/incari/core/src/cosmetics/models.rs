//! The hats and back cosmetics as models (ADR 0061; `CosmeticModels.java`): each a few boxes,
//! described here, with a 64x64 texture painted from the same description, so the boxes and
//! their pictures never disagree. Coordinates are the player model's, in pixels with +y down: a
//! hat's from the head's pivot (the head's box is -4..4 across, -8..0 up), a back cosmetic's from
//! the body's (the body's back is at z = 2).
//!
//! `mesh` lays a model out as quads in its anchor part's frame (the game side divides by 16 and
//! draws them under the head or body, `native_draw_quad`; the Wardrobe's preview projects them).
//! Pure: the same model and state always give the same quads.
use super::art;

/// The texture's side.
pub const TEXTURE: usize = 64;

/// Where a cosmetic sits on the player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    Head,
    Body,
}

/// How a cosmetic is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// Opaque, with see-through pixels cut out.
    Cutout,
    /// Partly see-through.
    Translucent,
    /// Glowing: lit by itself, at full brightness.
    Glow,
}

/// What a box is painted with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Material {
    Gold,
    GoldGem,
    Halo,
    Felt,
    FeltBand,
    Knit,
    Pom,
    Bone,
    Ear,
    Pack,
    Pocket,
    Wing,
}

impl Material {
    /// The length of the Java enum constant's name: its grain's salt.
    fn salt_len(self) -> i32 {
        match self {
            Material::Gold
            | Material::Halo
            | Material::Felt
            | Material::Knit
            | Material::Bone
            | Material::Pack
            | Material::Wing => 4,
            Material::GoldGem => 8,
            Material::Pocket => 6,
            Material::FeltBand => 9,
            Material::Pom | Material::Ear => 3,
        }
    }
}

/// A box: where, how big, how much bigger than that, what it's painted with; mirrored.
#[derive(Clone, Copy, Debug)]
struct Box3 {
    at: [f32; 3],
    size: [i32; 3],
    grow: f32,
    material: Material,
    mirror: bool,
}

/// A part: its name, where it turns about, how it's turned (x, y, z radians), its boxes.
#[derive(Clone, Debug)]
struct Part {
    name: &'static str,
    offset: [f32; 3],
    rotation: [f32; 3],
    boxes: Vec<Box3>,
}

/// A cosmetic's model as described.
#[derive(Clone, Debug)]
pub struct Spec {
    pub anchor: Anchor,
    pub look: Look,
    parts: Vec<Part>,
}

fn bx(x: f32, y: f32, z: f32, w: i32, h: i32, d: i32, grow: f32, material: Material) -> Box3 {
    Box3 {
        at: [x, y, z],
        size: [w, h, d],
        grow,
        material,
        mirror: false,
    }
}

fn part(name: &'static str, offset: [f32; 3], rotation: [f32; 3], boxes: Vec<Box3>) -> Part {
    Part {
        name,
        offset,
        rotation,
        boxes,
    }
}

/// How far back the wings are swept at rest, in radians.
const WING_SWEEP: f32 = 0.28;

/// The model of cosmetic `id`, or None for one that isn't a model (capes, name tags...).
pub fn spec(id: &str) -> Option<Spec> {
    use Material::*;
    let zero = [0.0; 3];
    let one = |anchor, look, p: Part| Spec {
        anchor,
        look,
        parts: vec![p],
    };
    Some(match id {
        "hat.crown" => {
            // A band round the top of the head, outside the skin's hat layer, its top velvet.
            let mut boxes = vec![bx(-4.0, -10.0, -4.0, 8, 2, 8, 0.6, GoldGem)];
            for [x, z] in [
                [-4.6, -4.6],
                [3.6, -4.6],
                [-4.6, 3.6],
                [3.6, 3.6],
                [-0.5, -4.6],
                [-0.5, 3.6],
                [-4.6, -0.5],
                [3.6, -0.5],
            ] {
                boxes.push(bx(x, -12.6, z, 1, 2, 1, 0.05, Gold));
            }
            one(Anchor::Head, Look::Cutout, part("crown", zero, zero, boxes))
        }
        "hat.halo" => one(
            Anchor::Head,
            Look::Glow,
            part(
                "ring",
                [0.0, -12.5, 0.0],
                zero,
                vec![
                    bx(-3.0, -0.5, -3.5, 6, 1, 1, 0.0, Halo),
                    bx(-3.0, -0.5, 2.5, 6, 1, 1, 0.0, Halo),
                    bx(-3.5, -0.5, -3.0, 1, 1, 6, 0.0, Halo),
                    bx(2.5, -0.5, -3.0, 1, 1, 6, 0.0, Halo),
                ],
            ),
        ),
        "hat.top_hat" => one(
            Anchor::Head,
            Look::Cutout,
            part(
                "hat",
                zero,
                zero,
                vec![
                    bx(-5.0, -9.0, -5.0, 10, 1, 10, 0.1, Felt),
                    bx(-4.0, -15.0, -4.0, 8, 6, 8, -0.5, FeltBand),
                ],
            ),
        ),
        "hat.beanie" => one(
            Anchor::Head,
            Look::Cutout,
            part(
                "beanie",
                zero,
                zero,
                vec![
                    bx(-4.0, -9.0, -4.0, 8, 3, 8, 0.7, Knit),
                    bx(-1.0, -12.0, -1.0, 2, 2, 2, 0.3, Pom),
                ],
            ),
        ),
        "hat.horns" => Spec {
            anchor: Anchor::Head,
            look: Look::Cutout,
            parts: vec![horn("left", 1.0), horn("right", -1.0)],
        },
        "hat.cat_ears" => Spec {
            anchor: Anchor::Head,
            look: Look::Cutout,
            parts: vec![ear("left", 1.0), ear("right", -1.0)],
        },
        "back.angel_wings" | "back.bat_wings" => wings(Look::Cutout),
        "back.frost_wings" => wings(Look::Translucent),
        "back.backpack" => one(
            Anchor::Body,
            Look::Cutout,
            part(
                "pack",
                zero,
                zero,
                vec![
                    bx(-4.0, 1.0, 2.0, 8, 10, 4, 0.0, Pack),
                    bx(-3.0, 5.0, 6.0, 6, 4, 1, 0.0, Pocket),
                ],
            ),
        ),
        _ => return None,
    })
}

fn horn(name: &'static str, side: f32) -> Part {
    part(
        name,
        [side * 2.5, -8.0, -1.0],
        [-0.2, 0.0, side * 0.45],
        vec![
            bx(-1.0, -3.0, -1.0, 2, 3, 2, 0.0, Material::Bone),
            bx(-0.5 + side * 0.4, -5.5, -0.5, 1, 3, 1, 0.0, Material::Bone),
        ],
    )
}

fn ear(name: &'static str, side: f32) -> Part {
    part(
        name,
        [side * 2.6, -8.2, -0.5],
        [0.0, 0.0, side * 0.28],
        vec![
            bx(-1.5, -3.0, -0.5, 3, 3, 1, 0.0, Material::Ear),
            bx(-0.5, -4.0, -0.5, 1, 1, 1, 0.0, Material::Ear),
        ],
    )
}

/// Two wings, panels with nothing but a front and a back, from the shoulder blades, out past the
/// shoulders and up past them, swept back a little.
fn wings(look: Look) -> Spec {
    let wing = |mirror: bool| Box3 {
        at: [if mirror { -16.0 } else { 0.0 }, -9.0, 0.0],
        size: [16, 16, 0],
        grow: 0.0,
        material: Material::Wing,
        mirror,
    };
    Spec {
        anchor: Anchor::Body,
        look,
        parts: vec![
            part(
                "left_wing",
                [1.0, 3.0, 2.4],
                [0.0, -WING_SWEEP, -0.12],
                vec![wing(false)],
            ),
            part(
                "right_wing",
                [-1.0, 3.0, 2.4],
                [0.0, WING_SWEEP, 0.12],
                vec![wing(true)],
            ),
        ],
    }
}

// -------------------------------------------------------------------------------------------------
// The texture
// -------------------------------------------------------------------------------------------------

/// Where each box's faces start on the texture, in the order the boxes are described: packed on
/// shelves, as the Java client lays them out.
fn tex_offsets(spec: &Spec) -> Vec<[i32; 2]> {
    let (mut sx, mut sy, mut sh) = (0, 0, 0);
    let mut out = Vec::new();
    for p in &spec.parts {
        for b in &p.boxes {
            let [w, h, d] = b.size;
            let (width, height) = (2 * d + 2 * w, d + h);
            if sx + width > TEXTURE as i32 {
                sx = 0;
                sy += sh;
                sh = 0;
            }
            out.push([sx, sy]);
            sx += width;
            sh = sh.max(height);
        }
    }
    out
}

/// Paints the model's 64x64 texture (ARGB) for cosmetic `id`.
pub fn texture(id: &str, spec: &Spec, out: &mut [u32]) {
    out.fill(0);
    let boxes = spec.parts.iter().flat_map(|p| p.boxes.iter());
    for (b, [u, v]) in boxes.zip(tex_offsets(spec)) {
        let [w, h, d] = b.size;
        let faces = [
            (u + d, v, w, d),
            (u + d + w, v, w, d),
            (u, v + d, d, h),
            (u + d, v + d, w, h),
            (u + d + w, v + d, d, h),
            (u + 2 * d + w, v + d, w, h),
        ];
        for (face, (x0, y0, fw, fh)) in faces.into_iter().enumerate() {
            for y in 0..fh {
                for x in 0..fw {
                    let (px, py) = (x0 + x, y0 + y);
                    if (px as usize) < TEXTURE && (py as usize) < TEXTURE {
                        out[py as usize * TEXTURE + px as usize] =
                            color(id, b.material, face, x, y, fw, fh);
                    }
                }
            }
        }
    }
}

/// A face's pixel: face 0 and 1 the bottom and top, 3 the front, 5 the back.
fn color(
    id: &str,
    material: Material,
    face: usize,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> u32 {
    use Material::*;
    let flat = face <= 1;
    let row = if height <= 1 {
        0.0
    } else {
        y as f32 / (height - 1) as f32
    };
    let grain = art::hash(x, y, face as i32 + material.salt_len() * 7);
    let opaque = |rgb: u32| 0xFF00_0000 | rgb;
    match material {
        Gold | GoldGem => {
            if material == GoldGem && flat {
                return opaque(art::mix(0x8E1B2E, 0x6A1222, grain));
            }
            if material == GoldGem
                && face == 3
                && (x as f32 - (width - 1) as f32 / 2.0).abs() < 1.0
                && y == 0
            {
                return 0xFFE0_344B;
            }
            let gold = art::mix(0xFFE58A, 0xD69A1E, if flat { 0.1 } else { row });
            opaque(if grain < 0.08 {
                art::mix(gold, 0xFFFFFF, 0.6)
            } else {
                gold
            })
        }
        Halo => opaque(art::mix(0xFFF6D2, 0xFFDB7A, grain * 0.6)),
        Felt => opaque(art::mix(0x2A2A33, 0x1A1A20, grain)),
        FeltBand => {
            if !flat && y >= height - 2 {
                0xFFC0_263A
            } else {
                opaque(art::mix(0x2A2A33, 0x1A1A20, grain))
            }
        }
        Knit => {
            if !flat && y == height - 1 {
                return 0xFF4F_8FC7;
            }
            let stripe = if flat { (x + y) / 2 } else { y };
            if stripe & 1 == 0 {
                0xFF7F_C3F2
            } else {
                0xFFEA_F6FF
            }
        }
        Pom => opaque(art::mix(0xFFFFFF, 0xDDE8F2, grain)),
        Bone => opaque(art::mix(
            0xF2E6CC,
            0x9C8A6A,
            if flat { 0.3 } else { 1.0 - row },
        )),
        Ear => {
            if face == 3 && x > 0 && x < width - 1 && y > 0 {
                0xFFF2_A6BC
            } else {
                0xFF4A_4A58
            }
        }
        Pack => {
            if face == 1 {
                return 0xFF7F_C3F2;
            }
            let seam = x == 0 || y == 0 || x == width - 1 || y == height - 1;
            if seam {
                0xFF3B_5B8A
            } else {
                opaque(art::mix(0x22385A, 0x1B2D49, grain))
            }
        }
        Pocket => {
            if x == 0 || y == 0 || x == width - 1 || y == height - 1 {
                0xFFA9_DCFF
            } else {
                0xFF2C_4A72
            }
        }
        Wing => {
            // The back (south) face has the root on its left; the front shows it mirrored, so
            // the two sides of the panel line up.
            let sx = (x as f32 + 0.5) / width as f32;
            let sy = (y as f32 + 0.5) / height as f32;
            art::wing(id, if face == 5 { sx } else { 1.0 - sx }, sy)
        }
    }
}

// -------------------------------------------------------------------------------------------------
// The mesh
// -------------------------------------------------------------------------------------------------

/// What a model is posed from: the wearer's frame, as `CosmeticModel.setupAnim` and
/// `CosmeticsLayer` read it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    /// `ageInTicks`.
    pub age: f32,
    pub crouching: bool,
    pub fall_flying: bool,
    /// A chestplate (or other chest armour) is worn: a back cosmetic stands a pixel out.
    pub chest_armor: bool,
    /// A cape hangs from the shoulders, turned back this far (radians, `6 + capeLean / 2 +
    /// capeFlap` degrees): a back cosmetic hangs behind it and swings with it.
    pub cape_swing: Option<f32>,
}

/// A face to draw: its corners in the anchor part's frame (pixels, +y down) in the game's polygon
/// order, each with its texture position (pixels of the 64x64 texture).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    pub pos: [[f32; 3]; 4],
    pub uv: [[f32; 2]; 4],
}

/// An affine transform, rows of a 3x4 matrix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine(pub [[f32; 4]; 3]);

impl Affine {
    pub const IDENTITY: Affine = Affine([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ]);

    pub fn translation(t: [f32; 3]) -> Self {
        let mut m = Self::IDENTITY;
        for (row, v) in m.0.iter_mut().zip(t) {
            row[3] = v;
        }
        m
    }

    /// `Quaternionf.rotationZYX(z, y, x)`: x first, then y, then z.
    pub fn rotation_zyx(x: f32, y: f32, z: f32) -> Self {
        Self::rotation_z(z)
            .then(&Self::rotation_y(y))
            .then(&Self::rotation_x(x))
    }

    pub fn rotation_x(a: f32) -> Self {
        let (s, c) = a.sin_cos();
        Affine([[1.0, 0.0, 0.0, 0.0], [0.0, c, -s, 0.0], [0.0, s, c, 0.0]])
    }

    pub fn rotation_y(a: f32) -> Self {
        let (s, c) = a.sin_cos();
        Affine([[c, 0.0, s, 0.0], [0.0, 1.0, 0.0, 0.0], [-s, 0.0, c, 0.0]])
    }

    pub fn rotation_z(a: f32) -> Self {
        let (s, c) = a.sin_cos();
        Affine([[c, -s, 0.0, 0.0], [s, c, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]])
    }

    pub fn scale(s: [f32; 3]) -> Self {
        Affine([
            [s[0], 0.0, 0.0, 0.0],
            [0.0, s[1], 0.0, 0.0],
            [0.0, 0.0, s[2], 0.0],
        ])
    }

    /// `self * other`: `other` applied first (a pose stack's next transform).
    pub fn then(&self, other: &Affine) -> Affine {
        let (a, b) = (&self.0, &other.0);
        let mut out = [[0.0; 4]; 3];
        for (i, row) in out.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = a[i][0] * b[0][j]
                    + a[i][1] * b[1][j]
                    + a[i][2] * b[2][j]
                    + if j == 3 { a[i][3] } else { 0.0 };
            }
        }
        Affine(out)
    }

    pub fn apply(&self, p: [f32; 3]) -> [f32; 3] {
        let m = &self.0;
        [0, 1, 2].map(|i| m[i][0] * p[0] + m[i][1] * p[1] + m[i][2] * p[2] + m[i][3])
    }
}

/// Lays `spec` out as quads in its anchor's frame, posed for `state`, into `out` (cleared first).
/// Per player per frame: nothing is made but what `out` grows to once.
pub fn mesh(id: &str, spec: &Spec, state: &State, out: &mut Vec<Quad>) {
    out.clear();
    let mut anchor = Affine::IDENTITY;
    if spec.anchor == Anchor::Body {
        if state.chest_armor {
            // A chestplate stands a pixel out from the body.
            anchor = anchor.then(&Affine::translation([0.0, 0.0, 1.0]));
        }
        if let Some(swing) = state.cape_swing
            && id != "back.backpack"
        {
            // Behind the cape, about its top edge at the body's back (z = 2), a pixel and a gap out.
            anchor = anchor
                .then(&Affine::translation([0.0, 0.0, 2.0]))
                .then(&Affine::rotation_x(swing))
                .then(&Affine::translation([0.0, 0.0, -2.0 + 1.0]));
        }
    }
    let offsets = tex_offsets(spec);
    let mut boxes = offsets.iter();
    for p in &spec.parts {
        let (mut offset, mut rotation) = (p.offset, p.rotation);
        pose_part(p.name, state, &mut offset, &mut rotation);
        let m = anchor
            .then(&Affine::translation(offset))
            .then(&Affine::rotation_zyx(rotation[0], rotation[1], rotation[2]));
        for b in &p.boxes {
            let Some(&tex) = boxes.next() else { break };
            cube(b, tex, &m, out);
        }
    }
}

/// `CosmeticModel.setupAnim`: the halo turns and bobs; wings beat at rest, fold back while
/// sneaking and spread while gliding, never swept forward far enough to reach the arms.
fn pose_part(name: &str, state: &State, offset: &mut [f32; 3], rotation: &mut [f32; 3]) {
    let age = state.age;
    match name {
        "ring" => {
            rotation[1] = age * 0.04;
            offset[1] += (age * 0.08).sin() * 0.4;
        }
        "left_wing" | "right_wing" => {
            let beat = (age * 0.12).sin() * 0.08 + 0.08;
            let fold = if state.crouching { 0.25 } else { 0.0 };
            let spread = if state.fall_flying { 0.2 } else { 0.0 };
            let sweep = (beat + fold - spread).max(-WING_SWEEP + 0.1);
            if name == "left_wing" {
                rotation[1] -= sweep;
            } else {
                rotation[1] += sweep;
            }
        }
        _ => {}
    }
}

/// A cosmetic's box as the game's `ModelPart.Cube` makes it.
fn cube(b: &Box3, tex: [i32; 2], m: &Affine, out: &mut Vec<Quad>) {
    let g = if b.size[2] == 0 { 0.0 } else { b.grow };
    box_quads(b.at, b.size, g, tex, b.mirror, m, out);
}

/// The game's `ModelPart.Cube` of `size` pixels from `at`, grown by `grow`, its faces from texture
/// position `tex`, under `m`: its polygons (a panel with no depth has only its front and back),
/// each vertex with its texture position, as the game lays them out. The player's own boxes in
/// the Wardrobe's preview are made the same way.
pub fn box_quads(
    at: [f32; 3],
    size: [i32; 3],
    g: f32,
    [u, v]: [i32; 2],
    mirror: bool,
    m: &Affine,
    out: &mut Vec<Quad>,
) {
    let [w, h, d] = size;
    let (mut x0, y0, z0) = (at[0] - g, at[1] - g, at[2] - g);
    let (mut x1, y1, z1) = (
        at[0] + w as f32 + g,
        at[1] + h as f32 + g,
        at[2] + d as f32 + g,
    );
    if mirror {
        std::mem::swap(&mut x0, &mut x1);
    }
    let v7 = [x0, y0, z0];
    let v0 = [x1, y0, z0];
    let v1 = [x1, y1, z0];
    let v2 = [x0, y1, z0];
    let v3 = [x0, y0, z1];
    let v4 = [x1, y0, z1];
    let v5 = [x1, y1, z1];
    let v6 = [x0, y1, z1];
    let (u, v, w, h, d) = (u as f32, v as f32, w as f32, h as f32, d as f32);
    let (f4, f5, f6, f7, f8, f9) = (
        u,
        u + d,
        u + d + w,
        u + d + w + w,
        u + d + w + d,
        u + d + w + d + w,
    );
    let (f10, f11, f12) = (v, v + d, v + d + h);
    let panel = size[2] == 0;
    let faces: [([[f32; 3]; 4], [f32; 4], bool); 6] = [
        ([v4, v3, v7, v0], [f5, f10, f6, f11], !panel),
        ([v1, v2, v6, v5], [f6, f11, f7, f10], !panel),
        ([v7, v3, v6, v2], [f4, f11, f5, f12], !panel),
        ([v0, v7, v2, v1], [f5, f11, f6, f12], true),
        ([v4, v0, v1, v5], [f6, f11, f8, f12], !panel),
        ([v3, v4, v5, v6], [f8, f11, f9, f12], true),
    ];
    for (corners, [u1, v1, u2, v2], shown) in faces {
        if !shown {
            continue;
        }
        let mut pos = corners.map(|c| m.apply(c));
        let mut uv = [[u2, v1], [u1, v1], [u1, v2], [u2, v2]];
        if mirror {
            pos.reverse();
            uv.reverse();
        }
        out.push(Quad { pos, uv });
    }
}

/// Cosmetic `cosmetic`'s place among those that are models (the hats, then the back cosmetics,
/// in the catalog's order): where the game side keeps its texture.
pub fn number(cosmetic: &super::catalog::Cosmetic) -> Option<usize> {
    use super::catalog::{self, Slot};
    catalog::all()
        .iter()
        .filter(|c| matches!(c.slot, Slot::Hat | Slot::Back))
        .position(|c| c.id == cosmetic.id)
}

/// Each cosmetic's model by catalog place (None: not made yet), and the last mesh's quads.
type Made = (Vec<Option<Option<Spec>>>, Vec<Quad>);

thread_local! {
    /// Each cosmetic's model as described, made the first time it's drawn, and the quads of the
    /// last mesh: drawing a player's hat each frame makes nothing.
    static MADE: std::cell::RefCell<Made> = const { std::cell::RefCell::new((Vec::new(), Vec::new())) };
}

/// Runs `f` with `cosmetic`'s model and its quads posed for `state` (`mesh`), from a cache the
/// window's thread keeps; None for a cosmetic that isn't a model. Per player per frame.
pub fn with_mesh<R>(
    cosmetic: &super::catalog::Cosmetic,
    state: &State,
    f: impl FnOnce(&Spec, &[Quad]) -> R,
) -> Option<R> {
    let index = super::catalog::index(cosmetic);
    MADE.with(|made| {
        let mut made = made.borrow_mut();
        let (specs, quads) = &mut *made;
        if specs.len() <= index {
            specs.resize_with(index + 1, || None);
        }
        let spec = specs[index]
            .get_or_insert_with(|| spec(cosmetic.id))
            .as_ref()?;
        mesh(cosmetic.id, spec, state, quads);
        Some(f(spec, quads))
    })
}

/// A quad as `native_draw_quad` takes one: corners in its order (texture top left, bottom left,
/// bottom right, top right) and its texture rectangle `[u0, v0, u1, v1]`, by turning the quad's
/// corners round (never reversing them, so it faces the same way).
pub fn quad_for_rect(q: &Quad) -> ([[f32; 3]; 4], [f32; 4]) {
    for k in 0..4 {
        let at = |i: usize| q.uv[(k + i) % 4];
        let (a, b, c, d) = (at(0), at(1), at(2), at(3));
        if a[0] == b[0] && b[1] == c[1] && c[0] == d[0] && d[1] == a[1] {
            let pos = [0, 1, 2, 3].map(|i| q.pos[(k + i) % 4]);
            return (pos, [a[0], a[1], c[0], c[1]]);
        }
    }
    (q.pos, [q.uv[1][0], q.uv[1][1], q.uv[3][0], q.uv[3][1]])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cosmetics::catalog::{self, Slot};

    #[test]
    fn every_hat_and_back_cosmetic_is_a_model_on_its_texture() {
        let mut quads = Vec::new();
        let mut tex = vec![0u32; TEXTURE * TEXTURE];
        for c in catalog::in_slot(Slot::Hat).chain(catalog::in_slot(Slot::Back)) {
            let spec = spec(c.id).expect(c.id);
            assert_eq!(spec.anchor == Anchor::Head, c.slot == Slot::Hat);
            mesh(c.id, &spec, &State::default(), &mut quads);
            assert!(!quads.is_empty());
            texture(c.id, &spec, &mut tex);
            for q in &quads {
                for [u, v] in q.uv {
                    assert!(
                        (0.0..=TEXTURE as f32).contains(&u) && (0.0..=TEXTURE as f32).contains(&v),
                        "{}",
                        c.id
                    );
                }
            }
            // Something painted under every face's middle (wings may be clear at their edges).
            if spec.parts[0].boxes[0].material != Material::Wing {
                for q in &quads {
                    let u = (q.uv[0][0] + q.uv[2][0]) / 2.0;
                    let v = (q.uv[0][1] + q.uv[2][1]) / 2.0;
                    assert_ne!(tex[v as usize * TEXTURE + u as usize] >> 24, 0, "{}", c.id);
                }
            }
        }
        assert!(spec("cape.frost").is_none());
        assert_eq!(number(catalog::by_id("hat.crown").unwrap()), Some(0));
        assert_eq!(number(catalog::by_id("back.backpack").unwrap()), Some(9));
        assert_eq!(number(catalog::by_id("cape.frost").unwrap()), None);
    }

    #[test]
    fn boxes_have_six_faces_and_panels_two() {
        let mut quads = Vec::new();
        mesh(
            "hat.beanie",
            &spec("hat.beanie").unwrap(),
            &State::default(),
            &mut quads,
        );
        assert_eq!(quads.len(), 12);
        mesh(
            "back.bat_wings",
            &spec("back.bat_wings").unwrap(),
            &State::default(),
            &mut quads,
        );
        assert_eq!(quads.len(), 4);
    }

    #[test]
    fn a_cape_or_chestplate_moves_a_back_cosmetic_out() {
        let spec = spec("back.angel_wings").unwrap();
        let (mut plain, mut moved) = (Vec::new(), Vec::new());
        mesh("back.angel_wings", &spec, &State::default(), &mut plain);
        mesh(
            "back.angel_wings",
            &spec,
            &State {
                chest_armor: true,
                ..State::default()
            },
            &mut moved,
        );
        assert!((moved[0].pos[0][2] - plain[0].pos[0][2] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn rect_quads_keep_their_corners_turning() {
        let spec = spec("hat.crown").unwrap();
        let mut quads = Vec::new();
        mesh("hat.crown", &spec, &State::default(), &mut quads);
        for q in &quads {
            let (pos, [u0, v0, u1, v1]) = quad_for_rect(q);
            let uv = [[u0, v0], [u0, v1], [u1, v1], [u1, v0]];
            // Each corner keeps its own texture position.
            for (p, t) in pos.iter().zip(uv) {
                let i = q.pos.iter().position(|o| o == p).unwrap();
                assert_eq!(q.uv[i], t);
            }
        }
    }

    #[test]
    fn affine_matches_the_rotation_order() {
        let m = Affine::rotation_zyx(0.0, std::f32::consts::FRAC_PI_2, 0.0);
        let p = m.apply([1.0, 0.0, 0.0]);
        assert!((p[2] + 1.0).abs() < 1e-5, "y turns +x to -z: {p:?}");
        let t = Affine::translation([1.0, 2.0, 3.0]).then(&Affine::scale([2.0; 3]));
        assert_eq!(t.apply([1.0, 1.0, 1.0]), [3.0, 4.0, 5.0]);
    }
}
