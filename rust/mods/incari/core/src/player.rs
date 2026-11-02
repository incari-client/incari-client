//! The player on the title's card (TitleProfile.playerView): their skin on the model's boxes,
//! turned toward the pointer as the inventory's player is, drawn from the canvas as faces sorted
//! back to front.
use crate::canvas::{Key, Region};
use crate::paint::Painter;

/// A box of the model: its texture's corner and size in skin pixels, where it is (feet at the
/// origin, y up, the front toward +z), and how far its outer layer stands out.
struct Part {
    tex: [f32; 2],
    size: [f32; 3],
    at: [f32; 3],
    grow: f32,
    head: bool,
}

/// The parts, inner layers first; `arm` is 4 wide, or 3 for slim arms.
fn parts(arm: f32) -> Vec<Part> {
    let p = |tex: [f32; 2], size: [f32; 3], at: [f32; 3], grow: f32, head: bool| Part { tex, size, at, grow, head };
    vec![
        p([0.0, 0.0], [8.0, 8.0, 8.0], [-4.0, 24.0, -4.0], 0.0, true),
        p([16.0, 16.0], [8.0, 12.0, 4.0], [-4.0, 12.0, -2.0], 0.0, false),
        p([40.0, 16.0], [arm, 12.0, 4.0], [-4.0 - arm, 12.0, -2.0], 0.0, false),
        p([32.0, 48.0], [arm, 12.0, 4.0], [4.0, 12.0, -2.0], 0.0, false),
        p([0.0, 16.0], [4.0, 12.0, 4.0], [-4.0, 0.0, -2.0], 0.0, false),
        p([16.0, 48.0], [4.0, 12.0, 4.0], [0.0, 0.0, -2.0], 0.0, false),
        p([32.0, 0.0], [8.0, 8.0, 8.0], [-4.0, 24.0, -4.0], 0.5, true),
        p([16.0, 32.0], [8.0, 12.0, 4.0], [-4.0, 12.0, -2.0], 0.25, false),
        p([40.0, 32.0], [arm, 12.0, 4.0], [-4.0 - arm, 12.0, -2.0], 0.25, false),
        p([48.0, 48.0], [arm, 12.0, 4.0], [4.0, 12.0, -2.0], 0.25, false),
        p([0.0, 32.0], [4.0, 12.0, 4.0], [-4.0, 0.0, -2.0], 0.25, false),
        p([0.0, 48.0], [4.0, 12.0, 4.0], [0.0, 0.0, -2.0], 0.25, false),
    ]
}

/// One face to draw: its corners on screen, its texture and how near it is.
struct Face {
    corners: [[f32; 2]; 4],
    src: [f32; 4],
    depth: f32,
    /// How lit it is, 0 to 1: the inventory's light, front and above.
    shade: f32,
}

/// Draws the player in the box `[x, y, w, h]`, facing the pointer at `mouse`.
pub fn draw(p: &mut Painter, rect: [f32; 4], mouse: [f32; 2]) {
    let Some((skin, slim)) = p.assets.skin.as_ref() else {
        return;
    };
    let slim = *slim;
    let Some(region) = p.canvas.get(Key::Skin, || skin.clone()) else {
        return;
    };
    let [x, y, w, h] = rect;
    let unit = (h / 32.0).min(w / 16.0) * 0.95;
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    // As the inventory's player follows the pointer: the body a little, the head more.
    let ax = ((cx - mouse[0]) / 40.0).atan();
    let ay = ((y + h * 0.15 - mouse[1]) / 40.0).atan();
    let body_yaw = -(ax * 20.0).to_radians();
    let head_yaw = -(ax * 40.0).to_radians();
    let head_pitch = (ay * 20.0).to_radians();
    let mut faces = Vec::with_capacity(72);
    for part in parts(if slim { 3.0 } else { 4.0 }) {
        let (yaw, pitch) = if part.head { (head_yaw, head_pitch) } else { (body_yaw, 0.0) };
        let project = |v: [f32; 3]| -> ([f32; 2], f32) {
            // About the neck for the head, the feet's middle for the rest.
            let pivot_y = if part.head { 24.0 } else { 0.0 };
            let (px, py, pz) = (v[0], v[1] - pivot_y, v[2]);
            let (py, pz) = (py * pitch.cos() - pz * pitch.sin(), py * pitch.sin() + pz * pitch.cos());
            let (px, pz) = (px * yaw.cos() + pz * yaw.sin(), -px * yaw.sin() + pz * yaw.cos());
            ([cx + px * unit, cy + (16.0 - (py + pivot_y)) * unit], pz)
        };
        box_faces(&part, region, project, &mut faces);
    }
    faces.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    for f in faces {
        let c = (f.shade * 255.0).round() as u32;
        let argb = p.faded(0xFF00_0000 | c << 16 | c << 8 | c);
        p.list.quad(f.src, f.corners, argb);
    }
}

/// A box's faces that face the viewer.
fn box_faces(part: &Part, region: Region, project: impl Fn([f32; 3]) -> ([f32; 2], f32), out: &mut Vec<Face>) {
    let [w, h, d] = part.size;
    let g = part.grow;
    let [x0, y0, z0] = [part.at[0] - g, part.at[1] - g, part.at[2] - g];
    let [x1, y1, z1] = [part.at[0] + w + g, part.at[1] + h + g, part.at[2] + d + g];
    let [u, v] = part.tex;
    // Texture rectangles (skin pixels) and the face's corners: top left, top right, bottom
    // right, bottom left as the texture lies.
    // Lit as the inventory's player is: the front full, the sides and back darker, the bottom
    // darkest.
    const SHADES: [f32; 6] = [1.0, 0.7, 0.8, 0.8, 0.95, 0.5];
    let faces: [([f32; 4], [[f32; 3]; 4]); 6] = [
        // Front (+z).
        ([u + d, v + d, w, h], [[x0, y1, z1], [x1, y1, z1], [x1, y0, z1], [x0, y0, z1]]),
        // Back (-z).
        ([u + d + w + d, v + d, w, h], [[x1, y1, z0], [x0, y1, z0], [x0, y0, z0], [x1, y0, z0]]),
        // The player's right (-x).
        ([u, v + d, d, h], [[x0, y1, z0], [x0, y1, z1], [x0, y0, z1], [x0, y0, z0]]),
        // The player's left (+x).
        ([u + d + w, v + d, d, h], [[x1, y1, z1], [x1, y1, z0], [x1, y0, z0], [x1, y0, z1]]),
        // Top.
        ([u + d, v, w, d], [[x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]]),
        // Bottom.
        ([u + d + w, v, w, d], [[x0, y0, z1], [x1, y0, z1], [x1, y0, z0], [x0, y0, z0]]),
    ];
    for ((src, corners), shade) in faces.into_iter().zip(SHADES) {
        let projected = corners.map(&project);
        let pts = projected.map(|c| c.0);
        // Facing the viewer: drawn clockwise on screen.
        let cross = (pts[1][0] - pts[0][0]) * (pts[3][1] - pts[0][1]) - (pts[1][1] - pts[0][1]) * (pts[3][0] - pts[0][0]);
        if cross <= 0.0 {
            continue;
        }
        let depth = projected.iter().map(|c| c.1).sum::<f32>() / 4.0 + g;
        out.push(Face {
            corners: pts,
            src: region.part(src[0], src[1], src[2], src[3]),
            depth,
            shade,
        });
    }
}
