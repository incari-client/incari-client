//! The look's panels and controls drawn by hand (Panels.java): glass panels, cards, switches.
use crate::color::{alpha, mix};
use crate::motion;
use crate::paint::Painter;

/// A switch's size in GUI units.
pub const SWITCH_WIDTH: f32 = 18.0;
pub const SWITCH_HEIGHT: f32 = 10.0;

/// A panel: glass with an edge and, in Normal, a soft shadow below.
pub fn panel(p: &mut Painter, x: f32, y: f32, w: f32, h: f32, fade: f32) {
    let t = p.theme;
    let normal = t.normal();
    p.round_box(
        x,
        y,
        w,
        h,
        t.panel_radius,
        alpha(t.panel_fill(), fade),
        alpha(t.panel_edge(), fade),
        if normal { 6.0 } else { 0.0 },
        2.0,
        alpha(0x4008_1420, fade),
    );
}

/// A dark panel, for what must read over a bright world in either preset.
pub fn dark_panel(p: &mut Painter, x: f32, y: f32, w: f32, h: f32, fade: f32) {
    let t = p.theme;
    let normal = t.normal();
    p.round_box(
        x,
        y,
        w,
        h,
        t.panel_radius,
        alpha(if normal { 0xC21A_2129 } else { t.glass_solid }, fade),
        alpha(t.panel_edge(), fade),
        if normal { 6.0 } else { 0.0 },
        2.0,
        alpha(0x4008_1420, fade),
    );
}

/// An on/off switch at (`x`, `y`): an ice-blue track with a dark knob at the right when on, a
/// faint one with a grey knob at the left when off. In Normal the knob slides.
pub fn toggle(p: &mut Painter, x: f32, y: f32, on: bool, fade: f32) {
    let t = p.motion.ease(
        motion::key(x, y, SWITCH_WIDTH, -SWITCH_HEIGHT),
        if on { 1.0 } else { 0.0 },
    );
    let track = mix(0x2EFF_FFFF, p.theme.accent, t);
    p.round(
        x,
        y,
        SWITCH_WIDTH,
        SWITCH_HEIGHT,
        SWITCH_HEIGHT / 2.0,
        alpha(track, fade),
    );
    let knob = SWITCH_HEIGHT - 2.0;
    let knob_x = x + 1.0 + (t * (SWITCH_WIDTH - 2.0 - knob)).round();
    let color = mix(0xFFC9_D1D9, 0xFF0D_2233, t);
    p.round(knob_x, y + 1.0, knob, knob, knob / 2.0, alpha(color, fade));
}

/// A card: a button-like glass tile that lights toward the pointer, edged in the accent while
/// `on`; in Normal, a glow under the pointer.
pub fn card(p: &mut Painter, x: f32, y: f32, w: f32, h: f32, hot: bool, on: bool, fade: f32) {
    let th = p.theme;
    let t = p
        .motion
        .ease(motion::key(x, y, w, h), if hot { 1.0 } else { 0.0 });
    let normal = th.normal();
    let mut edge = if on {
        th.accent & 0x8CFF_FFFF
    } else {
        th.panel_edge()
    };
    if !normal && hot {
        edge = th.accent;
    }
    p.round_box(
        x,
        y,
        w,
        h,
        th.radius + 1.0,
        alpha(mix(th.button, th.button_hot, t * 0.7), fade),
        alpha(edge, fade),
        if normal && t > 0.0 {
            crate::theme::GLOW
        } else {
            0.0
        },
        0.0,
        alpha(th.accent, 0.22 * t * fade),
    );
}

/// The look's backdrop over a whole screen: the photo out of a world, a cold tint over it all.
pub fn backdrop(p: &mut Painter, w: f32, h: f32, photo: bool) {
    let t = p.theme;
    if photo {
        p.backdrop([0.0, 0.0, w, h], 0xFFFF_FFFF);
    }
    let (top, bottom) = if t.normal() {
        (t.tint_top, t.tint_bottom)
    } else {
        (t.tint_top_solid, t.tint_bottom_solid)
    };
    p.gradient(0.0, 0.0, w, h, top, bottom);
}
