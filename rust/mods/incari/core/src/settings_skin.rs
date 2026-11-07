//! The look over Horizon's searchable settings screens (Graphics, Controls, Shaders, Studio): the
//! game lays them out and writes their text in the colours `palette` gives it, and asks the skin
//! for each part it draws on by a `horizon/settings/` name. Incari's mark stands in for Horizon's.
use crate::color::{alpha, mix};
use crate::icons::Icon;
use crate::paint::Painter;
use crate::panels;
use crate::skin;
use crate::theme::Theme;

/// A part's look when it can't be used now.
const OFF: f32 = 0.5;

/// The settings screens' text colours (`0xAARRGGBB`) in the theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub text: u32,
    pub soft: u32,
    pub muted: u32,
    pub accent: u32,
    pub note: u32,
    pub positive: u32,
    pub warning: u32,
    pub negative: u32,
}

/// The palette the theme gives the settings screens' text.
pub fn palette(th: &Theme) -> Palette {
    Palette {
        text: th.text,
        soft: mix(th.muted, th.text, 0.5),
        muted: th.muted,
        accent: th.accent,
        note: th.muted,
        positive: th.positive,
        warning: th.warning,
        negative: th.negative,
    }
}

/// The slider track drawn last: its handle, drawn next, lights it up to itself.
#[derive(Default)]
pub struct SettingsSkin {
    track: [f32; 4],
}

impl SettingsSkin {
    /// Draws part `part` (the name after `horizon/settings/`) over `rect` and returns true, or
    /// false for the game to draw it.
    pub fn part(&mut self, p: &mut Painter, part: &str, rect: [f32; 4]) -> bool {
        let th = p.theme;
        let [x, y, w, h] = rect;
        let (base, off) = match part.strip_suffix("_disabled") {
            Some(base) => (base, true),
            None => (part, false),
        };
        let (base, hot) = match base.strip_suffix("_highlighted") {
            Some(base) => (base, true),
            None => (base, false),
        };
        let fade = if off { OFF } else { 1.0 };
        match base {
            // The game's menu background under it is the theme's tint already.
            "veil" => {}
            "panel" => panels::panel(p, x, y, w, h, 1.0),
            "group" => p.round(x, y, w, h, th.radius + 1.0, th.button_off),
            "well" => skin::well(p, rect, th.field, false, 1.0),
            "row_lit" => p.round(x, y, w, h, th.radius, th.button),
            "tab_open" => {
                p.round(x, y, w, h, th.radius, th.button_hot);
                p.round(x, y + 3.0, 2.0, (h - 6.0).max(2.0), 1.0, th.accent);
            }
            "card_open" => p.round(x, y, w, h, th.radius, th.button_hot),
            "rule_head" => p.hairline(x, y, w, th.glass_edge_solid),
            "rule" => p.hairline(x, y, w, th.button_off),
            "focus" => p.outline(x, y, w, h, th.radius, th.accent),
            "scroller" => p.round(x + 2.0, y, (w - 4.0).max(2.0), h, 1.0, 0x1AFF_FFFF),
            "scroller_thumb" => p.round(x + 2.0, y, (w - 4.0).max(2.0), h, 1.0, 0xA6FF_FFFF),
            "mark" => {
                p.icon(Icon::Incari, x, y - 1.0, 9.0, th.accent);
                p.text("Incari", x + 12.0, y, th.text, true);
            }
            // A setting that's on or off is Incari's switch, at the row's right end.
            "checkbox" | "checkbox_selected" => {
                let sx = x + w - panels::SWITCH_WIDTH;
                let sy = y + ((h - panels::SWITCH_HEIGHT) / 2.0).floor();
                panels::toggle(p, sx, sy, base == "checkbox_selected", fade);
            }
            "button" | "button_primary" => skin::button(p, rect, off, hot, 1.0),
            "cycler" => skin::well(p, rect, th.field, hot, fade),
            "slider" => {
                self.track = rect;
                skin::well(p, rect, th.button, hot, fade);
            }
            "slider_handle" => self.handle(p, rect, hot, fade),
            _ => return false,
        }
        true
    }

    /// A slider's handle: a line along the track's foot, lit in the accent up to the value, and a
    /// knob at the value (`rect` is the game's 4 px knob).
    fn handle(&self, p: &mut Painter, [x, _, w, _]: [f32; 4], hot: bool, fade: f32) {
        let [tx, ty, tw, th] = self.track;
        let value = x + (w / 2.0).floor();
        let line = ty + th - 4.0;
        let (left, right) = (tx + 4.0, tx + tw - 4.0);
        let value = value.clamp(left, right);
        p.fill(left, line, right - left, 1.0, alpha(0x33FF_FFFF, fade));
        p.fill(left, line, value - left, 1.0, alpha(p.theme.accent, fade));
        let knob = if hot { 0xFFFF_FFFF } else { p.theme.accent };
        p.round(value - 2.0, line - 2.0, 5.0, 5.0, 3.0, alpha(knob, fade));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::FROST;

    #[test]
    fn the_palette_is_the_themes() {
        let p = palette(&FROST);
        assert_eq!(p.text, FROST.text);
        assert_eq!(p.accent, FROST.accent);
        assert_eq!(p.muted, FROST.muted);
        assert_eq!(p.soft >> 24, 0xFF);
    }
}
