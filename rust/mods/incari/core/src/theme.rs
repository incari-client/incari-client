//! The client's look as values (Theme.java): colours by what they're for, the sizes Incari's
//! screens are laid out with, and the GUI preset. Frost until the player picks a theme of their
//! own (Theme Editor), which `apply` makes from a few colours.
use crate::color::{darken, lighten, luminance, opacity};

/// How much the GUI does to look good (GuiPreset): Potato draws the same shapes solid and still.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Preset {
    /// Solid panels, no blur, no animation. The default.
    #[default]
    Potato,
    /// Frosted, blurred menus, eased hover animations, a light sweep over buttons.
    Normal,
}

impl Preset {
    pub fn name(self) -> &'static str {
        match self {
            Preset::Potato => "potato",
            Preset::Normal => "normal",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Preset::Potato => "Potato",
            Preset::Normal => "Normal",
        }
    }
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "potato" => Some(Preset::Potato),
            "normal" => Some(Preset::Normal),
            _ => None,
        }
    }
}

/// A control's height: buttons, sliders, fields.
pub const CONTROL_HEIGHT: f32 = 20.0;
/// A setting's control's width.
pub const CONTROL_WIDTH: f32 = 150.0;
/// A small button's width (Reset, a module's settings).
pub const SMALL_BUTTON_WIDTH: f32 = 50.0;
/// A footer button's width.
pub const FOOTER_BUTTON_WIDTH: f32 = 150.0;
/// A list row's height, its control and the space below it.
pub const ROW_HEIGHT: f32 = 24.0;
/// A list's row width.
pub const ROW_WIDTH: f32 = 340.0;
/// Space between controls side by side.
pub const GAP: f32 = 4.0;
/// Space between a header's lines of text.
pub const LINE_GAP: f32 = 4.0;
/// How far a hovered control's glow spreads, in Normal.
pub const GLOW: f32 = 4.0;
/// A line of text's height.
pub const LINE: f32 = 9.0;

/// The theme's colours (`0xAARRGGBB`) and corner radii.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub preset: Preset,
    pub text: u32,
    pub muted: u32,
    pub accent: u32,
    pub positive: u32,
    pub warning: u32,
    pub negative: u32,
    pub background: u32,
    pub glass: u32,
    pub glass_solid: u32,
    pub glass_edge: u32,
    pub glass_edge_solid: u32,
    pub button: u32,
    pub button_hot: u32,
    pub button_off: u32,
    pub field: u32,
    pub floating: u32,
    pub tint_top: u32,
    pub tint_bottom: u32,
    pub tint_top_solid: u32,
    pub tint_bottom_solid: u32,
    pub editor_tint_top: u32,
    pub editor_tint_bottom: u32,
    pub on_accent: u32,
    pub radius: f32,
    pub panel_radius: f32,
}

/// Frost, as the client draws it before the player picks a theme.
pub const FROST: Theme = Theme {
    preset: Preset::Potato,
    text: 0xFFFF_FFFF,
    muted: 0xFFB4_BDC6,
    accent: 0xFFA9_DCFF,
    positive: 0xFF8F_E3B0,
    warning: 0xFFFF_D08A,
    negative: 0xFFFF_8F8F,
    background: 0x6B28_3442,
    glass: 0x2EFF_FFFF,
    glass_solid: 0xDB24_2A32,
    glass_edge: 0x4DFF_FFFF,
    glass_edge_solid: 0x24FF_FFFF,
    button: 0x1FFF_FFFF,
    button_hot: 0x47FF_FFFF,
    button_off: 0x0DFF_FFFF,
    field: 0x5910_1820,
    floating: 0xEB18_202A,
    tint_top: 0x8C18_2433,
    tint_bottom: 0xB310_1822,
    tint_top_solid: 0xC214_1B24,
    tint_bottom_solid: 0xDE0E_131A,
    editor_tint_top: 0x2610_1822,
    editor_tint_bottom: 0x4D10_1822,
    on_accent: 0xFF0D_2233,
    radius: 3.0,
    panel_radius: 5.0,
};

impl Default for Theme {
    fn default() -> Self {
        FROST
    }
}

/// The few colours a theme is made from (the Theme Editor's), each `0xRRGGBB`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub accent: u32,
    pub text: u32,
    pub muted: u32,
    pub panel: u32,
    /// The menus' panels' opacity in Potato, 0 to 100.
    pub panel_opacity: i32,
    /// The HUD's glass's opacity, 0 to 100.
    pub hud_opacity: i32,
    /// A control's corner radius in GUI units; panels round 2 more.
    pub radius: i32,
}

/// Frost's own palette.
pub const FROST_PALETTE: Palette = Palette {
    accent: 0xA9DCFF,
    text: 0xFFFFFF,
    muted: 0xB4BDC6,
    panel: 0x242A32,
    panel_opacity: 86,
    hud_opacity: 42,
    radius: 3,
};

impl Theme {
    /// Whether the GUI's costly effects are on.
    pub fn normal(&self) -> bool {
        self.preset == Preset::Normal
    }

    /// A theme from `palette`, as Theme.apply makes one: every other shade from those as Frost's
    /// come from its own. Frost's own palette gives Frost exactly.
    pub fn from_palette(palette: Palette, preset: Preset) -> Self {
        let mut t = FROST;
        t.preset = preset;
        let p = Palette {
            accent: palette.accent & 0xFF_FFFF,
            text: palette.text & 0xFF_FFFF,
            muted: palette.muted & 0xFF_FFFF,
            panel: palette.panel & 0xFF_FFFF,
            ..palette
        };
        if p == FROST_PALETTE {
            return t;
        }
        t.accent = 0xFF00_0000 | p.accent;
        t.text = 0xFF00_0000 | p.text;
        t.muted = 0xFF00_0000 | p.muted;
        t.on_accent = if luminance(p.accent) > 0.5 {
            0xFF00_0000 | darken(p.accent, 0.18)
        } else {
            0xFFFF_FFFF
        };
        t.glass_solid = opacity(p.panel_opacity) | p.panel;
        t.background = opacity(p.hud_opacity) | lighten(p.panel, 0.04);
        t.floating = 0xEB00_0000 | darken(p.panel, 0.75);
        t.field = 0x5900_0000 | darken(p.panel, 0.5);
        t.glass = 0x2E00_0000 | p.text;
        t.glass_edge = 0x4D00_0000 | p.text;
        t.glass_edge_solid = 0x2400_0000 | p.text;
        t.button = 0x1F00_0000 | p.text;
        t.button_hot = 0x4700_0000 | p.text;
        t.button_off = 0x0D00_0000 | p.text;
        t.tint_top = 0x8C00_0000 | darken(p.panel, 0.65);
        t.tint_bottom = 0xB300_0000 | darken(p.panel, 0.5);
        t.tint_top_solid = 0xC200_0000 | darken(p.panel, 0.6);
        t.tint_bottom_solid = 0xDE00_0000 | darken(p.panel, 0.45);
        t.editor_tint_top = 0x2600_0000 | darken(p.panel, 0.5);
        t.editor_tint_bottom = 0x4D00_0000 | darken(p.panel, 0.5);
        t.radius = p.radius.max(0) as f32;
        t.panel_radius = p.radius.max(0) as f32 + 2.0;
        t
    }

    /// A panel's fill: glass in Normal, solid in Potato.
    pub fn panel_fill(&self) -> u32 {
        if self.normal() {
            self.glass
        } else {
            self.glass_solid
        }
    }

    /// A panel's edge.
    pub fn panel_edge(&self) -> u32 {
        if self.normal() {
            self.glass_edge
        } else {
            self.glass_edge_solid
        }
    }

    /// The colour a HUD tone takes.
    pub fn tone(&self, tone: Tone) -> u32 {
        match tone {
            Tone::Text => self.text,
            Tone::Muted => self.muted,
            Tone::Accent => self.accent,
            Tone::Positive => self.positive,
            Tone::Warning => self.warning,
            Tone::Negative => self.negative,
            Tone::Background => self.background,
            Tone::Edge => self.glass_edge,
            Tone::OnAccent => self.on_accent,
        }
    }
}

/// What a HUD colour is for (framework's Tone): the theme picks the colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Text,
    Muted,
    Accent,
    Positive,
    Warning,
    Negative,
    Background,
    Edge,
    OnAccent,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frost_palette_gives_frost() {
        assert_eq!(Theme::from_palette(FROST_PALETTE, Preset::Potato), FROST);
    }

    #[test]
    fn a_palette_makes_every_shade() {
        let t = Theme::from_palette(
            Palette {
                accent: 0xFF8800,
                text: 0xEEEEEE,
                ..FROST_PALETTE
            },
            Preset::Normal,
        );
        assert_eq!(t.accent, 0xFFFF_8800);
        assert_eq!(t.button, 0x1FEE_EEEE);
        assert_eq!(t.on_accent >> 24, 0xFF);
        assert!(t.normal());
    }
}
