//! The cosmetics' pictures, painted in code (ADR 0061; `CosmeticArt.java`): Incari's own art, so
//! nothing is shipped as an image and nothing is Mojang's. Capes are painted into the game's 64x32
//! cape layout (the outer side, the inner side, the edges and an elytra), some again and again as
//! time passes; the wardrobe's cards get small icons. Pixels are ARGB. Pure functions.

pub const CAPE_WIDTH: usize = 64;
pub const CAPE_HEIGHT: usize = 32;
/// A wardrobe card's icon, square.
pub const ICON: usize = 24;

/// Whether the cape `id` moves: it's painted again as time passes.
pub fn animated(id: &str) -> bool {
    matches!(
        id,
        "cape.aurora" | "cape.midnight" | "cape.ember" | "cape.tide"
    )
}

fn round(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}

// -------------------------------------------------------------------------------------------------
// Capes
// -------------------------------------------------------------------------------------------------

/// Paints the cape `id` at time `t` (seconds) into `out` (64x32): the outer side at (1, 1), 10x16,
/// as the game's cape model reads it; the inner side darker; the edges; and the elytra's wing at
/// (22, 0), so an elytra worn with it matches.
pub fn cape(id: &str, t: f32, out: &mut [u32]) {
    out.fill(0);
    let w = CAPE_WIDTH;
    for py in 0..16 {
        for px in 0..10 {
            let color = design(id, px as f32 + 0.5, py as f32 + 0.5, t);
            out[(py + 1) * w + px + 1] = color;
            // The inner side faces the player: the same, mirrored and in shade.
            out[(py + 1) * w + 12 + (9 - px)] = shade(color, 0.55);
        }
    }
    let edge = shade(design(id, 5.0, 8.0, t), 0.7);
    for py in 1..=16 {
        out[py * w] = edge;
        out[py * w + 11] = edge;
    }
    out[1..=20].fill(edge);
    for y in 0..22 {
        for x in 22..46 {
            out[y * w + x] = design(
                id,
                (x as f32 - 22.0 + 0.5) * 10.0 / 24.0,
                (y as f32 + 0.5) * 16.0 / 22.0,
                t,
            );
        }
    }
}

/// The cape's outer side alone, 10x16 (a wardrobe card's picture, the preview's cape).
pub fn cape_face(id: &str, t: f32, out: &mut [u32]) {
    for py in 0..16 {
        for px in 0..10 {
            out[py * 10 + px] = design(id, px as f32 + 0.5, py as f32 + 0.5, t);
        }
    }
}

/// One cape's picture at (`px`, `py`) in its 10x16 face, at time `t`: opaque.
pub fn design(id: &str, px: f32, py: f32, t: f32) -> u32 {
    let v = py / 16.0;
    0xFF00_0000
        | match id {
            "cape.frost" => frost(px, py, v),
            "cape.aurora" => aurora(px, py, v, t),
            "cape.midnight" => midnight(px, py, v, t),
            "cape.ember" => ember(px, py, v, t),
            "cape.sakura" => sakura(px, py, v),
            "cape.tide" => tide(px, py, v, t),
            "cape.mono" => mono(px, py),
            _ => 0x80_8080,
        }
}

fn frost(px: f32, py: f32, v: f32) -> u32 {
    let mut color = mix(0x9FD3F5, 0x1E3F66, v);
    if !(1.0..=9.0).contains(&px) || !(1.0..=15.0).contains(&py) {
        color = mix(color, 0xFFFFFF, 0.35);
    }
    // A snowflake in the middle: three strokes and a branch at each arm's end.
    let dx = px - 5.0;
    let dy = py - 6.5;
    let flake = (dx.abs() < 0.6 && dy.abs() < 3.4)
        || (dy.abs() < 0.6 && dx.abs() < 3.4)
        || ((dx - dy).abs() < 0.75 && dx.abs() < 2.4)
        || ((dx + dy).abs() < 0.75 && dx.abs() < 2.4);
    if flake {
        return 0xF4FBFF;
    }
    // Frost glinting further down.
    if py > 11.0 && hash(px as i32, py as i32, 3) < 0.12 {
        return mix(color, 0xFFFFFF, 0.6);
    }
    color
}

fn aurora(px: f32, py: f32, v: f32, t: f32) -> u32 {
    let mut color = mix(0x0E1B33, 0x050A15, v);
    let bands = [0x3CF2A0, 0x3CC8F2, 0xA05CF2];
    for (k, band) in bands.iter().enumerate() {
        let k = k as f32;
        let centre = 3.5 + k * 4.2 + 1.6 * (px * 0.7 + t * 1.3 + k * 2.1).sin();
        let d = (py - centre) / 1.7;
        let glow = (-d * d).exp() * (0.65 + 0.35 * (t * 0.9 + px * 0.5 + k).sin());
        color = add(color, *band, glow * 0.85);
    }
    if hash(px as i32, py as i32, 11) < 0.05 {
        color = mix(color, 0xFFFFFF, 0.5 + 0.5 * (t * 4.0 + px).sin());
    }
    color
}

fn midnight(px: f32, py: f32, v: f32, t: f32) -> u32 {
    let color = mix(0x1F2A5C, 0x05081A, v);
    // A crescent moon in the top right.
    let (mx, my) = (px - 7.0, py - 3.2);
    let (ox, oy) = (px - 7.9, py - 2.5);
    if mx * mx + my * my < 4.4 && ox * ox + oy * oy > 3.0 {
        return 0xF2EFD0;
    }
    if hash(px as i32, py as i32, 7) < 0.1 {
        // Java's 6.283, not quite TAU: the stars twinkle as they do there.
        #[allow(clippy::approx_constant)]
        let phase = hash(px as i32, py as i32, 13) * 6.283;
        let twinkle = 0.5 + 0.5 * (t * 2.6 + phase).sin();
        return mix(color, 0xFFFFFF, 0.35 + 0.65 * twinkle);
    }
    color
}

fn ember(px: f32, py: f32, v: f32, t: f32) -> u32 {
    let flame = noise(px * 0.55, py * 0.45 + t * 2.2);
    let heat = clamp(v * 1.25 + (flame - 0.5) * 0.9 - 0.2);
    let color = if heat < 0.33 {
        mix(0x140806, 0x7A1708, heat / 0.33)
    } else if heat < 0.66 {
        mix(0x7A1708, 0xE5481A, (heat - 0.33) / 0.33)
    } else {
        mix(0xE5481A, 0xFFC94A, (heat - 0.66) / 0.34)
    };
    // Sparks rising.
    let cell = (py + t * 5.0).floor() as i32;
    if hash(px as i32, cell, 5) < 0.045 {
        return 0xFFE38A;
    }
    color
}

fn sakura(px: f32, py: f32, v: f32) -> u32 {
    let mut color = mix(0xFCDCE8, 0xE597B6, v);
    // A branch across, from the bottom left to the top right.
    let branch = ((py - 12.5) + px * 0.8).abs() / 1.28;
    if branch < 0.55 && px < 9.5 {
        color = 0x5E3B3B;
    }
    let blossoms = [
        [2.0, 10.5],
        [5.0, 8.0],
        [8.0, 5.5],
        [3.0, 4.0],
        [7.5, 12.5],
        [1.5, 1.8],
    ];
    for [bx, by] in blossoms {
        let (dx, dy) = (px - bx, py - by);
        let d = dx * dx + dy * dy;
        if d < 0.5 {
            return 0xFFE9A8;
        }
        if d < 2.1 {
            return 0xF4F0F2;
        }
    }
    color
}

fn tide(px: f32, py: f32, v: f32, t: f32) -> u32 {
    let color = mix(0x3D9FE0, 0x0A2552, v);
    for k in 0..5 {
        let k = k as f32;
        let crest = 2.5 + k * 3.0 + 0.9 * (px * 0.9 + t * 2.0 + k * 1.7).sin();
        if (py - crest).abs() < 0.55 {
            return mix(color, 0xD8F1FF, 0.75);
        }
    }
    if hash(px as i32, (py + t).floor() as i32, 9) < 0.04 {
        return mix(color, 0xFFFFFF, 0.5);
    }
    color
}

fn mono(px: f32, py: f32) -> u32 {
    if !(1.0..=9.0).contains(&px) || !(1.0..=15.0).contains(&py) || (px > 4.0 && px < 6.0) {
        return 0xEDEDED;
    }
    0x151515
}

// -------------------------------------------------------------------------------------------------
// Wings
// -------------------------------------------------------------------------------------------------

/// A wing at (`sx`, `sy`) on its panel: `sx` from the root (0) to the tip (1), `sy` from the top
/// (0) to the bottom (1). Transparent outside the wing.
pub fn wing(id: &str, sx: f32, sy: f32) -> u32 {
    match id {
        "back.bat_wings" => bat_wing(sx, sy),
        "back.frost_wings" => frost_wing(sx, sy),
        _ => angel_wing(sx, sy),
    }
}

fn wing_top(sx: f32) -> f32 {
    0.34 - 0.3 * (sx * std::f32::consts::PI * 0.5).sin()
}

fn feather_bottom(sx: f32) -> f32 {
    let scallop = 0.06 * (sx * std::f32::consts::PI * 5.0).sin().abs();
    0.97 - 0.62 * sx.powf(1.15) - scallop
}

fn angel_wing(sx: f32, sy: f32) -> u32 {
    let (top, bottom) = (wing_top(sx), feather_bottom(sx));
    if sy < top || sy > bottom {
        return 0;
    }
    let mut color = mix(
        0xFFFFFF,
        0xDCE3EC,
        clamp((sy - top) / (bottom - top).max(0.05)) * 0.6,
    );
    // Rows of feathers: lines across, and their tips toward the bottom.
    let row = (sy - top) / (bottom - top).max(0.05) * 4.0;
    if row - row.floor() < 0.12 && row > 0.8 {
        color = 0xC9D3E0;
    }
    if sx < 0.12 {
        color = mix(color, 0xB8C3D2, 0.5);
    }
    0xFF00_0000 | color
}

fn frost_wing(sx: f32, sy: f32) -> u32 {
    let (top, bottom) = (wing_top(sx), feather_bottom(sx));
    if sy < top || sy > bottom {
        return 0;
    }
    let facet = sx * 4.0 + sy * 2.5;
    let edge = facet - facet.floor() < 0.09 || sy - top < 0.03 || bottom - sy < 0.03;
    if edge {
        return 0xF0F4_FBFF;
    }
    0xA800_0000 | mix(0xCDEBFF, 0x7FB8E8, clamp(sy))
}

fn bat_wing(sx: f32, sy: f32) -> u32 {
    // The arm along the top, the fingers fanning down from the wrist, the skin scalloped between.
    let top = 0.12 + 0.12 * sx;
    let fingers = sx * 3.0;
    let between = fingers - fingers.floor();
    let bottom = 0.9 - 0.5 * sx - 0.22 * (between * std::f32::consts::PI).sin();
    if sy < top || sy > bottom {
        return 0;
    }
    if sy - top < 0.07 {
        return 0xFF3A_2A40;
    }
    for finger in 1..=3 {
        let end_x = finger as f32 / 3.0;
        let end_y = 0.9 - 0.5 * end_x;
        let along = sx / end_x;
        if along <= 1.0 && (sy - (top + (end_y - top) * along)).abs() < 0.035 {
            return 0xFF22_182A;
        }
    }
    0xFF00_0000 | mix(0x62486E, 0x3E2D47, clamp(sy))
}

// -------------------------------------------------------------------------------------------------
// Icons
// -------------------------------------------------------------------------------------------------

/// The wardrobe card's icon for a hat or a back cosmetic, `ICON` square, into `out`.
pub fn icon(id: &str, out: &mut [u32]) {
    for y in 0..ICON {
        for x in 0..ICON {
            out[y * ICON + x] = icon_pixel(id, x as f32 + 0.5, y as f32 + 0.5);
        }
    }
}

fn icon_pixel(id: &str, x: f32, y: f32) -> u32 {
    match id {
        "hat.crown" => crown_icon(x, y),
        "hat.halo" => halo_icon(x, y),
        "hat.top_hat" => top_hat_icon(x, y),
        "hat.beanie" => beanie_icon(x, y),
        "hat.horns" => horns_icon(x, y),
        "hat.cat_ears" => ears_icon(x, y),
        "back.backpack" => backpack_icon(x, y),
        "back.angel_wings" | "back.frost_wings" | "back.bat_wings" => wings_icon(id, x, y),
        _ => 0,
    }
}

fn crown_icon(x: f32, y: f32) -> u32 {
    let band = (13.0..19.0).contains(&y) && (4.0..20.0).contains(&x);
    let points = [5.0f32, 12.0, 19.0].iter().any(|peak| {
        let dx = (x - peak).abs();
        y < 13.0 && y >= 6.0 + dx * 2.2 && dx < 3.0
    });
    if (x - 12.0).abs() < 1.6 && (y - 16.0).abs() < 1.6 {
        return 0xFFE0_344B;
    }
    if band || points {
        return 0xFF00_0000 | mix(0xFFE58A, 0xD69A1E, (y - 6.0) / 13.0);
    }
    0
}

fn halo_icon(x: f32, y: f32) -> u32 {
    let (dx, dy) = ((x - 12.0) / 9.0, (y - 12.0) / 4.0);
    let d = dx * dx + dy * dy;
    if d < 1.0 && d > 0.45 {
        return 0xFFFF_F0B0;
    }
    if d < 1.4 && d > 0.3 {
        return 0x60FF_E68A;
    }
    0
}

fn top_hat_icon(x: f32, y: f32) -> u32 {
    if (17.0..19.0).contains(&y) && (2.0..22.0).contains(&x) {
        return 0xFF1C_1C22;
    }
    if (4.0..17.0).contains(&y) && (6.0..18.0).contains(&x) {
        return if y >= 13.0 { 0xFFC0_263A } else { 0xFF26_262E };
    }
    0
}

fn beanie_icon(x: f32, y: f32) -> u32 {
    let (dx, dy) = ((x - 12.0) / 9.0, (y - 17.0) / 11.0);
    if y < 17.0 && dx * dx + dy * dy < 1.0 {
        let row = (y / 2.5) as i32;
        return if row & 1 == 0 {
            0xFF7F_C3F2
        } else {
            0xFFEA_F6FF
        };
    }
    if (17.0..20.0).contains(&y) && (3.0..21.0).contains(&x) {
        return 0xFF4F_8FC7;
    }
    let (px, py) = (x - 12.0, y - 5.0);
    if px * px + py * py < 6.0 {
        return 0xFFFF_FFFF;
    }
    0
}

fn horns_icon(x: f32, y: f32) -> u32 {
    for side in [-1.0f32, 1.0] {
        let base_x = 12.0 + side * 5.0;
        let t = (20.0 - y) / 15.0;
        if !(0.0..=1.0).contains(&t) {
            continue;
        }
        let centre = base_x + side * 5.0 * t * t;
        let half = 2.6 * (1.0 - t) + 0.4;
        if (x - centre).abs() < half {
            return 0xFF00_0000 | mix(0xF2E6CC, 0x9C8A6A, t);
        }
    }
    0
}

fn ears_icon(x: f32, y: f32) -> u32 {
    for side in [-1.0f32, 1.0] {
        let cx = 12.0 + side * 6.0;
        let t = (19.0 - y) / 13.0;
        if !(0.0..=1.0).contains(&t) {
            continue;
        }
        let half = 4.5 * (1.0 - t);
        let d = (x - cx).abs();
        if d < half {
            if d < half - 1.6 && t < 0.7 {
                return 0xFFF2_A6BC;
            }
            // A light rim, so dark ears read on a dark card.
            return if d > half - 0.8 || t > 0.92 {
                0xFF9A_9AAE
            } else {
                0xFF4A_4A58
            };
        }
    }
    0
}

fn backpack_icon(x: f32, y: f32) -> u32 {
    if (6.0..18.0).contains(&x) && (4.0..21.0).contains(&y) {
        if y < 9.0 {
            return 0xFF7F_C3F2;
        }
        if (8.0..16.0).contains(&x) && (12.0..18.0).contains(&y) {
            return if x < 8.8 || x > 15.2 || y < 12.8 || y > 17.2 {
                0xFFA9_DCFF
            } else {
                0xFF2C_4A72
            };
        }
        return 0xFF22_385A;
    }
    0
}

fn wings_icon(id: &str, x: f32, y: f32) -> u32 {
    // Both wings, spread from the middle.
    let sx = (x - 12.0).abs() / 11.0;
    let sy = (y - 2.0) / 20.0;
    if (x - 12.0).abs() < 1.0 || !(0.0..=1.0).contains(&sy) || sx > 1.0 {
        return 0;
    }
    let color = wing(id, sx, sy);
    if color >> 24 == 0 {
        0
    } else {
        0xFF00_0000 | color
    }
}

// -------------------------------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------------------------------

/// A colour between `a` and `b` (RGB), `t` of the way.
pub fn mix(a: u32, b: u32, t: f32) -> u32 {
    let k = clamp(t);
    let ch = |shift: u32| {
        let (x, y) = (((a >> shift) & 255) as f32, ((b >> shift) & 255) as f32);
        round(x + (y - x) * k) as u32
    };
    ch(16) << 16 | ch(8) << 8 | ch(0)
}

fn add(base: u32, light: u32, amount: f32) -> u32 {
    let ch = |shift: u32| {
        (((base >> shift) & 255) as i32 + round(((light >> shift) & 255) as f32 * amount)).min(255)
            as u32
    };
    ch(16) << 16 | ch(8) << 8 | ch(0)
}

/// `argb` darker by `factor`, alpha kept.
pub fn shade(argb: u32, factor: f32) -> u32 {
    let ch = |shift: u32| round(((argb >> shift) & 255) as f32 * factor) as u32;
    (argb & 0xFF00_0000) | ch(16) << 16 | ch(8) << 8 | ch(0)
}

pub fn clamp(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

/// A number in [0, 1) for a cell: the same for the same cell and salt (Java's int arithmetic).
pub fn hash(x: i32, y: i32, salt: i32) -> f32 {
    let mut h = x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263))
        .wrapping_add(salt.wrapping_mul(1_274_126_177));
    h = (h ^ ((h as u32) >> 13) as i32).wrapping_mul(1_274_126_177);
    h ^= ((h as u32) >> 16) as i32;
    (h & 0xFF_FFFF) as f32 / 0x100_0000 as f32
}

/// Smooth value noise in [0, 1).
fn noise(x: f32, y: f32) -> f32 {
    let (ix, iy) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - ix as f32, y - iy as f32);
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);
    let a = hash(ix, iy, 1);
    let b = hash(ix + 1, iy, 1);
    let c = hash(ix, iy + 1, 1);
    let d = hash(ix + 1, iy + 1, 1);
    a + (b - a) * sx + (c - a) * sy + (a - b - c + d) * sx * sy
}

/// ARGB pixels as the canvas's RGBA bytes.
pub fn rgba(pixels: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(pixels.len() * 4);
    for p in pixels {
        let [a, r, g, b] = p.to_be_bytes();
        out.extend_from_slice(&[r, g, b, a]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cosmetics::catalog::{self, Slot};

    #[test]
    fn hash_matches_javas_ints() {
        // CosmeticArt.hash(1, 2, 3) and (-5, 7, 11), from the Java client.
        let java = |x: i32, y: i32, salt: i32| {
            let mut h: i32 = x
                .wrapping_mul(374761393)
                .wrapping_add(y.wrapping_mul(668265263))
                .wrapping_add(salt.wrapping_mul(1274126177));
            h = (h ^ ((h as u32 >> 13) as i32)).wrapping_mul(1274126177);
            h ^= (h as u32 >> 16) as i32;
            (h & 0xFFFFFF) as f32 / 16777216.0
        };
        for (x, y, s) in [(1, 2, 3), (-5, 7, 11), (9, 15, 13)] {
            assert_eq!(hash(x, y, s), java(x, y, s));
            assert!((0.0..1.0).contains(&hash(x, y, s)));
        }
    }

    #[test]
    fn every_cape_and_icon_paints() {
        let mut cape_px = vec![0u32; CAPE_WIDTH * CAPE_HEIGHT];
        for c in catalog::in_slot(Slot::Cape) {
            cape(c.id, 1.5, &mut cape_px);
            assert_eq!(
                cape_px[CAPE_WIDTH + 1] >> 24,
                0xFF,
                "{}: the outer side is opaque",
                c.id
            );
            assert_eq!(
                cape_px[CAPE_WIDTH * 31 + 63],
                0,
                "outside the layout stays clear"
            );
        }
        let mut icon_px = vec![0u32; ICON * ICON];
        for c in catalog::in_slot(Slot::Hat).chain(catalog::in_slot(Slot::Back)) {
            icon(c.id, &mut icon_px);
            assert!(icon_px.iter().any(|p| p >> 24 != 0), "{} has an icon", c.id);
        }
        assert_eq!(mix(0x000000, 0xFFFFFF, 0.5), 0x808080);
        assert_eq!(shade(0xFF80_8080, 0.5), 0xFF40_4040);
    }

    #[test]
    fn moving_capes_change_with_time() {
        let (mut a, mut b) = (vec![0u32; 2048], vec![0u32; 2048]);
        cape("cape.ember", 0.0, &mut a);
        cape("cape.ember", 0.7, &mut b);
        assert_ne!(a, b);
        cape("cape.frost", 0.0, &mut a);
        cape("cape.frost", 0.7, &mut b);
        assert_eq!(a, b);
    }
}
