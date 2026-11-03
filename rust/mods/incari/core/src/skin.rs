//! Frost over the game's own widgets (FrostSkin.java): the game lays out and labels every screen
//! of its own, and this redraws the parts it draws them on, by sprite name, so a control drawn
//! anywhere in the game takes the look. Horizon asks it for each sprite drawn by name.
use crate::color::{alpha, mix};
use crate::motion;
use crate::paint::Painter;
use crate::panels;

/// How long a hovered button's light sweep takes to cross it, in milliseconds.
const SWEEP_MILLIS: f64 = 650.0;

/// The slider track drawn last: its handle, drawn next, lights the track up to itself.
#[derive(Default)]
pub struct Skin {
    track: [f32; 4],
}

impl Skin {
    /// Draws sprite `name` over `rect` in the look and returns true, or false for the game to draw
    /// it. `color`'s alpha fades it.
    pub fn sprite(&mut self, p: &mut Painter, name: &str, rect: [f32; 4], color: u32) -> bool {
        let Some(name) = name
            .strip_prefix("gui/sprites/")
            .or_else(|| name.strip_prefix("gui/"))
        else {
            return false;
        };
        let [x, y, w, h] = rect;
        let fade = (color >> 24) as f32 / 255.0;
        let th = p.theme;
        match name {
            "widget/button" => button(p, rect, false, false, fade),
            "widget/button_highlighted" => button(p, rect, false, true, fade),
            "widget/button_disabled" => button(p, rect, true, false, fade),
            "widget/slider" | "widget/slider_highlighted" => {
                self.track = rect;
                well(p, rect, th.button, name.ends_with("highlighted"), fade);
            }
            "widget/slider_handle" | "widget/slider_handle_highlighted" => {
                self.handle(p, rect, name.ends_with("highlighted"), fade)
            }
            "widget/text_field" | "widget/text_field_highlighted" => {
                well(p, rect, th.field, name.ends_with("highlighted"), fade)
            }
            "widget/checkbox" | "widget/checkbox_highlighted" => {
                well(p, rect, th.field, name.ends_with("highlighted"), fade)
            }
            "widget/checkbox_selected" | "widget/checkbox_selected_highlighted" => {
                well(p, rect, th.field, name.ends_with("highlighted"), fade);
                p.round(
                    x + 4.0,
                    y + 4.0,
                    w - 8.0,
                    h - 8.0,
                    2.0,
                    alpha(th.accent, fade),
                );
            }
            "widget/scroller" => {
                p.round(
                    x + 2.0,
                    y,
                    (w - 4.0).max(2.0),
                    h,
                    2.0,
                    alpha(0xA6FF_FFFF, fade),
                );
            }
            "widget/scroller_background" => {
                p.round(
                    x + 2.0,
                    y,
                    (w - 4.0).max(2.0),
                    h,
                    2.0,
                    alpha(0x1AFF_FFFF, fade),
                );
            }
            "widget/tab" => tab(p, rect, false, false, fade),
            "widget/tab_highlighted" => tab(p, rect, false, true, fade),
            "widget/tab_selected" => tab(p, rect, true, false, fade),
            "widget/tab_selected_highlighted" => tab(p, rect, true, true, fade),
            // The game's tooltip sprites reach 9 units past the padded text on every side.
            "tooltip/background" => p.round(
                x + 7.0,
                y + 7.0,
                w - 14.0,
                h - 14.0,
                th.radius,
                alpha(th.floating, fade),
            ),
            "tooltip/frame" => p.outline(
                x + 7.0,
                y + 7.0,
                w - 14.0,
                h - 14.0,
                th.radius,
                alpha(th.glass_edge_solid, fade),
            ),
            // The sprites reach 3 units past the slot's well on every side.
            "container/slot_highlight_back" => p.round(
                x + 3.0,
                y + 3.0,
                w - 6.0,
                h - 6.0,
                2.0,
                alpha(th.accent & 0x33FF_FFFF, fade),
            ),
            "container/slot_highlight_front" => p.outline(
                x + 3.0,
                y + 3.0,
                w - 6.0,
                h - 6.0,
                2.0,
                alpha(th.accent & 0xB3FF_FFFF, fade),
            ),
            "hud/hotbar" => {
                p.round_box(x, y, w, h, 4.0, th.background, 0x1FFF_FFFF, 0.0, 0.0, 0);
                for i in 0..9 {
                    p.round(
                        x + 2.0 + i as f32 * 20.0,
                        y + 2.0,
                        18.0,
                        18.0,
                        3.0,
                        0x14FF_FFFF,
                    );
                }
            }
            // 24 by 23 around a slot whose well is 20 square, 2 in from the left and 3 from the top.
            "hud/hotbar_selection" => p.round_box(
                x + 2.0,
                y + 3.0,
                20.0,
                20.0,
                3.0,
                th.accent & 0x29FF_FFFF,
                th.accent,
                if th.normal() { 3.0 } else { 0.0 },
                0.0,
                th.accent & 0x59FF_FFFF,
            ),
            "hud/hotbar_offhand_left" => slot_card(p, x, y + 1.0, 22.0, 22.0),
            "hud/hotbar_offhand_right" => slot_card(p, x + w - 22.0, y + 1.0, 22.0, 22.0),
            "toast/advancement" | "toast/recipe" | "toast/system" | "toast/tutorial"
            | "toast/now_playing" => {
                // The game writes recipe and tutorial toasts in dark text, so theirs is light
                // frost; the rest are dark, edged in the accent for an advancement.
                let light = name == "toast/recipe" || name == "toast/tutorial";
                let edge = if name == "toast/advancement" {
                    th.accent & 0x99FF_FFFF
                } else if light {
                    0xFFFF_FFFF
                } else {
                    th.glass_edge_solid
                };
                let fill = if light { 0xF2E6_ECF2 } else { th.floating };
                p.round_box(
                    x + 1.0,
                    y + 1.0,
                    w - 2.0,
                    h - 2.0,
                    4.0,
                    alpha(fill, fade),
                    alpha(edge, fade),
                    0.0,
                    0.0,
                    0,
                );
            }
            "hud/effect_background" | "hud/effect_background_ambient" => {
                let edge = if name.ends_with("ambient") {
                    th.accent & 0x80FF_FFFF
                } else {
                    0x1FFF_FFFF
                };
                p.round_box(
                    x,
                    y,
                    w,
                    h,
                    3.0,
                    alpha(th.background, fade),
                    alpha(edge, fade),
                    0.0,
                    0.0,
                    0,
                );
            }
            "popup/background" => panels::panel(p, x, y, w, h, fade),
            // Textures drawn by name: lists' wells, backgrounds, separators.
            "menu_list_background" | "inworld_menu_list_background" => p.fill(
                x,
                y,
                w,
                h,
                if th.normal() {
                    0x1A00_0000
                } else {
                    0x2E00_0000
                },
            ),
            "menu_background" | "inworld_menu_background" => {
                // The backdrop is drawn already (the title's, or the world's blur under the tint):
                // the game's dirt-dark texture becomes the tint.
                let (top, bottom) = if th.normal() {
                    (th.tint_top, th.tint_bottom)
                } else {
                    (th.tint_top_solid, th.tint_bottom_solid)
                };
                p.gradient(x, y, w, h, top, bottom);
            }
            "header_separator" | "inworld_header_separator" => {
                p.hairline(x, y + h - 1.0, w, th.glass_edge_solid)
            }
            "footer_separator" | "inworld_footer_separator" => {
                p.hairline(x, y, w, th.glass_edge_solid)
            }
            _ => return false,
        }
        true
    }

    /// A slider's handle, as a line along the track's foot: lit in the accent up to the value, a
    /// knob at the value.
    fn handle(&self, p: &mut Painter, [x, y, w, h]: [f32; 4], hot: bool, fade: f32) {
        let value = x + (w / 2.0).floor();
        let line = y + h - 4.0;
        let [tx, ty, tw, th] = self.track;
        if ty == y && th == h {
            let (left, right) = (tx + 4.0, tx + tw - 4.0);
            p.fill(left, line, right - left, 1.0, alpha(0x33FF_FFFF, fade));
            p.fill(
                left,
                line,
                (value.min(right) - left).max(0.0),
                1.0,
                alpha(p.theme.accent, fade),
            );
        }
        let knob = if hot { 0xFFFF_FFFF } else { p.theme.accent };
        p.round(value - 2.0, line - 2.0, 5.0, 5.0, 3.0, alpha(knob, fade));
    }
}

/// A button: glass that lights up toward the pointer, with a glow and a light sweep in Normal.
pub fn button(p: &mut Painter, [x, y, w, h]: [f32; 4], off: bool, hot: bool, fade: f32) {
    let th = p.theme;
    if off {
        p.round_box(
            x,
            y,
            w,
            h,
            th.radius,
            alpha(th.button_off, fade),
            alpha(th.glass_edge_solid, fade * 0.6),
            0.0,
            0.0,
            0,
        );
        return;
    }
    let t = p
        .motion
        .ease(motion::key(x, y, w, h), if hot { 1.0 } else { 0.0 });
    let since = p.motion.since();
    let normal = th.normal();
    let edge = if normal {
        mix(th.glass_edge, th.accent & 0x99FF_FFFF, t)
    } else if hot {
        th.accent
    } else {
        th.glass_edge_solid
    };
    p.round_box(
        x,
        y,
        w,
        h,
        th.radius,
        alpha(mix(th.button, th.button_hot, t), fade),
        alpha(edge, fade),
        if normal && t > 0.0 {
            crate::theme::GLOW
        } else {
            0.0
        },
        0.0,
        alpha(th.accent, 0.32 * t * fade),
    );
    if normal && hot && since < SWEEP_MILLIS {
        // A band of light crossing the button once, left to right, after the pointer reaches it.
        let mut q = (since / SWEEP_MILLIS) as f32;
        q = 1.0 - (1.0 - q) * (1.0 - q);
        let band = (w / 3.0).floor().max(16.0);
        let left = x - band + (q * (w + band)).round();
        p.clip_start([x + 1.0, y + 1.0, w - 2.0, h - 2.0]);
        p.band(left, y, band, h, alpha(0x38FF_FFFF, fade));
        p.clip_end();
    }
}

/// A slider's track or a field's well, edged in the accent while focused or hovered.
pub fn well(p: &mut Painter, [x, y, w, h]: [f32; 4], fill: u32, hot: bool, fade: f32) {
    let edge = if hot {
        p.theme.accent
    } else {
        p.theme.panel_edge()
    };
    p.round_box(
        x,
        y,
        w,
        h,
        p.theme.radius,
        alpha(fill, fade),
        alpha(edge, fade),
        0.0,
        0.0,
        0,
    );
}

/// A tab: the chosen one lit with an accent line under it.
fn tab(p: &mut Painter, [x, y, w, h]: [f32; 4], chosen: bool, hot: bool, fade: f32) {
    let th = p.theme;
    let inset = 2.0;
    let fill = if chosen {
        th.button_hot
    } else if hot {
        th.button
    } else {
        th.button_off
    };
    let edge = if !chosen && hot && !th.normal() {
        alpha(th.accent, fade)
    } else {
        0
    };
    p.round_box(
        x + inset,
        y + inset,
        w - 2.0 * inset,
        h - 2.0 * inset,
        th.radius,
        alpha(fill, fade),
        edge,
        0.0,
        0.0,
        0,
    );
    if chosen {
        p.round(
            x + (w / 4.0).floor(),
            y + h - inset - 2.0,
            (w / 2.0).floor(),
            2.0,
            1.0,
            alpha(th.accent, fade),
        );
    }
}

/// A lone slot on the HUD's glass: the offhand's.
fn slot_card(p: &mut Painter, x: f32, y: f32, w: f32, h: f32) {
    let bg = p.theme.background;
    p.round_box(x, y, w, h, 4.0, bg, 0x1FFF_FFFF, 0.0, 0.0, 0);
    p.round(x + 2.0, y + 2.0, w - 4.0, h - 4.0, 3.0, 0x14FF_FFFF);
}
