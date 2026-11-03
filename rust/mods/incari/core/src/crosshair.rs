//! Crosshair Editor (features/crosshair): a crosshair of the player's own, drawn in place of the
//! game's (its `hud/crosshair` sprite): a cross, T, circle or dot from sliders, or a 15 by 15 grid.
use crate::framework::setting::{Settings, Val};
use crate::paint::Painter;

const GRID: usize = 15;

/// The presets: style, length, gap, thickness, dot, outline.
const PRESETS: &[(&str, &str, i32, i32, i32, bool, bool)] = &[
    ("classic", "cross", 9, 0, 2, false, false),
    ("competitive", "cross", 6, 3, 2, false, true),
    ("small", "cross", 4, 2, 1, false, true),
    ("dot", "dot", 0, 0, 4, true, true),
    ("circle", "circle", 0, 8, 2, true, true),
    ("t_shape", "t", 7, 3, 2, false, true),
];

/// After setting `id` changed: a preset picked loads its values; a slider moved away from the
/// preset makes it Custom, so picking the preset again loads it.
pub fn changed(s: &mut Settings, id: &str) {
    let chosen = s.choice("preset");
    let preset = PRESETS.iter().find(|p| p.0 == chosen);
    if id == "preset" {
        if let Some(&(_, style, length, gap, thickness, dot, outline)) = preset {
            set_choice(s, "style", style);
            set(s, "length", Val::Int(length.max(1)));
            set(s, "gap", Val::Int(gap));
            set(s, "thickness", Val::Int(thickness));
            set(s, "dot", Val::Bool(dot));
            set(s, "outline", Val::Bool(outline));
        }
        return;
    }
    if let Some(&(_, style, length, gap, thickness, dot, outline)) = preset {
        let shows = s.choice("style") == style
            && s.int("length") == length.max(1)
            && s.int("gap") == gap
            && s.int("thickness") == thickness
            && s.bool("dot") == dot
            && s.bool("outline") == outline;
        if !shows {
            set_choice(s, "preset", "custom");
        }
    }
}

fn set(s: &mut Settings, id: &str, v: Val) {
    if let Some(x) = s.get_mut(id) {
        x.set(v);
    }
}

fn set_choice(s: &mut Settings, id: &str, key: &str) {
    if let Some(x) = s.get_mut(id)
        && let crate::framework::setting::Kind::Choice { choices, .. } = x.kind
        && let Some(i) = choices.iter().position(|c| c.0 == key)
    {
        x.set(Val::Choice(i));
    }
}

/// The crosshair centred on (`cx`, `cy`) in GUI units.
pub fn draw(p: &mut Painter, s: &Settings, on_target: bool, cx: f32, cy: f32) {
    let alpha = ((s.int("opacity") as f32 * 2.55).round() as u32) << 24;
    let rgb = if s.bool("target-color") && on_target {
        s.color("on-target-color")
    } else {
        s.color("color")
    } & 0xFF_FFFF;
    let (argb, shade) = (alpha | rgb, alpha);
    if s.choice("style") == "grid" {
        // In GUI units, as the game's own crosshair is drawn.
        let rows: Vec<u64> = match s.get("grid").map(|g| g.value.clone()) {
            Some(Val::Grid(rows)) => rows,
            _ => return,
        };
        let (ox, oy) = (cx - GRID as f32 / 2.0, cy - GRID as f32 / 2.0);
        let row = |y: i32| {
            if y < 0 || y >= GRID as i32 {
                0
            } else {
                rows[y as usize] << 1
            }
        };
        if s.bool("outline") {
            for y in -1..=GRID as i32 {
                let around = row(y - 1) | row(y + 1) | row(y) << 1 | row(y) >> 1;
                runs(p, around, ox, oy, y, -1.0, shade);
            }
        }
        for (y, r) in rows.iter().enumerate() {
            runs(p, *r, ox, oy, y as i32, 0.0, argb);
        }
        return;
    }
    // In screen pixels, around the middle.
    let px = 1.0 / p.scale;
    p.push(
        (cx * p.scale).round() * px,
        (cy * p.scale).round() * px,
        px,
        |p| {
            if s.bool("outline") {
                shapes(p, s, shade, 1.0);
            }
            shapes(p, s, argb, 0.0);
        },
    );
}

fn shapes(p: &mut Painter, s: &Settings, argb: u32, grow: f32) {
    let t = s.int("thickness") as f32;
    let half = (t / 2.0).floor();
    let g = s.int("gap") as f32;
    let l = s.int("length") as f32;
    let style = s.choice("style");
    let mut rect = |x: f32, y: f32, w: f32, h: f32| {
        p.fill(x - grow, y - grow, w + grow * 2.0, h + grow * 2.0, argb)
    };
    match style {
        "cross" | "t" => {
            rect(-g - l, -half, l, t);
            rect(g + (t - half * 2.0), -half, l, t);
            rect(-half, g + (t - half * 2.0), t, l);
            if style == "cross" {
                rect(-half, -g - l, t, l);
            }
        }
        _ => {}
    }
    if style == "circle" {
        let r = g + t + grow;
        p.ring(0.0, 0.0, r, t + grow * 2.0, argb);
    }
    if s.bool("dot") || style == "dot" {
        p.fill(
            -half - grow,
            -half - grow,
            t + grow * 2.0,
            t + grow * 2.0,
            argb,
        );
    }
}

/// A row's lit cells as runs of quads.
fn runs(p: &mut Painter, row: u64, ox: f32, oy: f32, y: i32, first: f32, argb: u32) {
    let mut i = 0;
    while i < 64 {
        if row >> i & 1 != 0 {
            let mut run = 1;
            while i + run < 64 && row >> (i + run) & 1 != 0 {
                run += 1;
            }
            p.fill(ox + i as f32 + first, oy + y as f32, run as f32, 1.0, argb);
            i += run;
        } else {
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_load_and_a_moved_slider_makes_custom() {
        let mut m = crate::modules::all()
            .into_iter()
            .find(|m| m.id == "crosshair")
            .unwrap();
        set_choice(&mut m.settings, "preset", "competitive");
        changed(&mut m.settings, "preset");
        assert_eq!((m.settings.int("length"), m.settings.int("gap")), (6, 3));
        assert!(m.settings.bool("outline"));
        set(&mut m.settings, "gap", Val::Int(5));
        changed(&mut m.settings, "gap");
        assert_eq!(m.settings.choice("preset"), "custom");
    }
}
