//! HUD elements' placement (framework's `hud`): an anchor of nine, an offset from it in GUI units
//! and a scale, the player's to change in the HUD editor and kept in the profile.
use super::setting::{self as s, Settings, Val};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

/// The anchors as a choice setting stores them.
pub const ANCHORS: &[(&str, &str)] = &[
    ("top_left", "Top left"),
    ("top", "Top"),
    ("top_right", "Top right"),
    ("left", "Left"),
    ("center", "Center"),
    ("right", "Right"),
    ("bottom_left", "Bottom left"),
    ("bottom", "Bottom"),
    ("bottom_right", "Bottom right"),
];

const ALL: [Anchor; 9] = [
    Anchor::TopLeft,
    Anchor::Top,
    Anchor::TopRight,
    Anchor::Left,
    Anchor::Center,
    Anchor::Right,
    Anchor::BottomLeft,
    Anchor::Bottom,
    Anchor::BottomRight,
];

impl Anchor {
    pub fn index(self) -> usize {
        ALL.iter().position(|a| *a == self).unwrap_or(0)
    }

    pub fn from_index(i: usize) -> Self {
        ALL[i.min(8)]
    }

    pub fn key(self) -> &'static str {
        ANCHORS[self.index()].0
    }

    fn column(self) -> i32 {
        self.index() as i32 % 3
    }

    fn row(self) -> i32 {
        self.index() as i32 / 3
    }

    /// The left edge of an element `size` wide `offset` from this anchor on a `screen` wide.
    pub fn x(self, offset: i32, size: i32, screen: i32) -> i32 {
        place(self.column(), offset, size, screen)
    }

    pub fn y(self, offset: i32, size: i32, screen: i32) -> i32 {
        place(self.row(), offset, size, screen)
    }

    /// The offset that puts an element `size` wide at `start`.
    pub fn offset_x(self, start: i32, size: i32, screen: i32) -> i32 {
        offset(self.column(), start, size, screen)
    }

    pub fn offset_y(self, start: i32, size: i32, screen: i32) -> i32 {
        offset(self.row(), start, size, screen)
    }

    /// The anchor of the screen's ninth the element's middle is in.
    pub fn nearest(x: i32, y: i32, w: i32, h: i32, screen_w: i32, screen_h: i32) -> Self {
        let column = third(x * 2 + w, screen_w * 2);
        let row = third(y * 2 + h, screen_h * 2);
        ALL[(row * 3 + column) as usize]
    }
}

fn third(at: i32, size: i32) -> i32 {
    if size <= 0 {
        return 0;
    }
    (at * 3 / size).clamp(0, 2)
}

fn offset(third: i32, start: i32, size: i32, screen: i32) -> i32 {
    match third {
        0 => start,
        1 => start - (screen - size) / 2,
        _ => screen - size - start,
    }
}

fn place(third: i32, offset: i32, size: i32, screen: i32) -> i32 {
    let start = match third {
        0 => offset,
        1 => (screen - size) / 2 + offset,
        _ => screen - size - offset,
    };
    // An element bigger than the screen stays at its start rather than off the far side.
    start.min(screen - size).max(0)
}

/// "1.5x" (HudElement.formatScale).
pub fn format_scale(value: f64) -> String {
    let hundredths = (value * 100.0).round() as i64;
    let mut text = format!("{}.{:02}", hundredths / 100, hundredths % 100);
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text + "x"
}

/// What kind of element it is, for the framework's own drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// Its module draws all of it.
    Custom,
    /// One line of text on the glass (TextElement): background, custom colour and colour
    /// settings of its own.
    Text,
}

/// A HUD element: its id, name and placement settings (`anchor`, `x`, `y`, `scale`, then its
/// own).
#[derive(Clone, Debug, PartialEq)]
pub struct HudElement {
    pub id: &'static str,
    pub name: &'static str,
    pub look: Look,
    pub settings: Settings,
}

impl HudElement {
    pub fn new(id: &'static str, name: &'static str, anchor: Anchor, x: i32, y: i32) -> Self {
        let mut settings = Settings::default();
        settings.add(s::choice("anchor", "Position", ANCHORS, anchor.key()));
        settings.add(s::int("x", "Offset across", -4096, 4096, x).hidden());
        settings.add(s::int("y", "Offset down", -4096, 4096, y).hidden());
        settings.add(s::float("scale", "Scale", 0.5, 3.0, 0.25, 1.0).formatted(format_scale));
        Self {
            id,
            name,
            look: Look::Custom,
            settings,
        }
    }

    /// One line of text on the glass (TextElement).
    pub fn text(id: &'static str, name: &'static str, anchor: Anchor, x: i32, y: i32) -> Self {
        let mut e = Self::new(id, name, anchor, x, y);
        e.look = Look::Text;
        e.settings.add(
            s::bool("background", "Background", true).described("Draws a panel behind the text."),
        );
        e.settings.add(
            s::bool("custom-color", "Custom color", false)
                .described("Off: the text takes the theme's colors."),
        );
        e.settings.add(
            s::color("color", "Text color", 0xFFFFFF, false)
                .shown(s::Shown::When("custom-color", true)),
        );
        e
    }

    /// Adds a setting of the element's own.
    pub fn with(mut self, setting: s::Setting) -> Self {
        self.settings.add(setting);
        self
    }

    pub fn anchor(&self) -> Anchor {
        Anchor::from_index(self.settings.get("anchor").map_or(0, s::Setting::as_choice))
    }

    pub fn scale(&self) -> f32 {
        self.settings.float("scale")
    }

    /// Where it is on a screen `screen` big, given its unscaled size: `[x, y, w, h]`.
    pub fn bounds(&self, size: [f32; 2], screen: [f32; 2]) -> [f32; 4] {
        let s = self.scale();
        let w = (size[0] * s).round() as i32;
        let h = (size[1] * s).round() as i32;
        let a = self.anchor();
        let (sw, sh) = (screen[0] as i32, screen[1] as i32);
        let x = a.x(self.settings.int("x"), w, sw);
        let y = a.y(self.settings.int("y"), h, sh);
        [x as f32, y as f32, w as f32, h as f32]
    }

    /// Moves it to `[x, y]` (its scaled size `w`, `h`), from the nearest anchor.
    pub fn place(&mut self, x: i32, y: i32, w: i32, h: i32, screen: [f32; 2]) {
        let (sw, sh) = (screen[0] as i32, screen[1] as i32);
        let a = Anchor::nearest(x, y, w, h, sw, sh);
        let set = |settings: &mut Settings, id: &str, v: Val| {
            if let Some(s) = settings.get_mut(id) {
                s.set(v);
            }
        };
        set(&mut self.settings, "anchor", Val::Choice(a.index()));
        set(&mut self.settings, "x", Val::Int(a.offset_x(x, w, sw)));
        set(&mut self.settings, "y", Val::Int(a.offset_y(y, h, sh)));
    }

    /// Back where the module put it, at its first scale.
    pub fn reset_placement(&mut self) {
        for id in ["anchor", "x", "y", "scale"] {
            if let Some(s) = self.settings.get_mut(id) {
                s.reset();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_by_anchor_as_the_java_framework_does() {
        assert_eq!(Anchor::TopRight.x(4, 50, 320), 266);
        assert_eq!(Anchor::Center.x(10, 50, 320), 145);
        assert_eq!(Anchor::Left.y(-48, 15, 240), 64);
        assert_eq!(
            Anchor::BottomRight.x(0, 500, 320),
            0,
            "too big stays at its start"
        );
        assert_eq!(Anchor::nearest(300, 5, 10, 10, 320, 240), Anchor::TopRight);
        assert_eq!(Anchor::Right.offset_x(266, 50, 320), 4);
    }

    #[test]
    fn moving_an_element_picks_its_nearest_anchor() {
        let mut e = HudElement::text("fps", "FPS", Anchor::Left, 2, -64);
        assert_eq!(
            e.bounds([40.0, 14.0], [320.0, 240.0]),
            [2.0, 49.0, 40.0, 14.0]
        );
        e.place(270, 220, 40, 14, [320.0, 240.0]);
        assert_eq!(e.anchor(), Anchor::BottomRight);
        assert_eq!(
            e.bounds([40.0, 14.0], [320.0, 240.0]),
            [270.0, 220.0, 40.0, 14.0]
        );
        e.reset_placement();
        assert_eq!(e.anchor(), Anchor::Left);
        assert_eq!(format_scale(1.5), "1.5x");
        assert_eq!(format_scale(1.0), "1x");
        assert_eq!(format_scale(0.75), "0.75x");
    }
}
