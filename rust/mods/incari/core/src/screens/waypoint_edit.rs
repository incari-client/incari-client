//! A waypoint's editor (WaypointEditScreen.java, ADR 0051): its name, its colour from the palette
//! (or any, by hex), its icon, where it is (with a button for where the player stands), whether
//! it's shown and whether it has a beam. A preview at the top shows it as it will look over the
//! world. Changes are made to a copy and kept only on Save; a new waypoint is added only then.
//! Enter saves, Escape cancels.
use crate::client::HostFiles;
use crate::color::{alpha, parse_hex};
use crate::draw::Rect;
use crate::icons::Icon;
use crate::input::keys;
use crate::panels;
use crate::screen::{Cx, Go, Screen};
use crate::theme::{CONTROL_HEIGHT, GAP};
use crate::ui;
use crate::waypoint::{self, COLORS, MARKERS, MAX_NAME, Waypoint};

const PADDING: f32 = 12.0;
const LABEL: f32 = 52.0;
const LINE: f32 = CONTROL_HEIGHT;
const LINE_GAP: f32 = 6.0;
const SWATCH: f32 = 14.0;
const SWATCH_GAP: f32 = 4.0;
const SHAPE_WIDTH: f32 = 26.0;
const SHAPE_GAP: f32 = 5.0;
const BUTTON_WIDTH: f32 = 76.0;
const HERE_WIDTH: f32 = 58.0;
/// The widest a coordinate is: the world border's, and a little.
pub const MAX_COORDINATE: i32 = 30_000_000;
const NAME_HINT: &str = "Name this place";
const AXES: [&str; 3] = ["X", "Y", "Z"];

pub struct WaypointEdit {
    /// The copy being edited (its id is the original's, or the draft's when adding).
    edited: Waypoint,
    adding: bool,
    name: String,
    coordinates: [String; 3],
    hex: String,
    /// The first frame: a new waypoint's name field takes the keys.
    focused: bool,
}

impl WaypointEdit {
    /// The editor on `waypoint`: one of the world's (a copy is edited) or, `adding`, a draft.
    pub fn new(waypoint: Waypoint, adding: bool) -> Self {
        Self {
            name: waypoint.name().to_owned(),
            coordinates: [waypoint.x, waypoint.y, waypoint.z].map(|v| v.to_string()),
            hex: format!("#{:06X}", waypoint.rgb()),
            edited: waypoint,
            adding,
            focused: false,
        }
    }

    /// The coordinates typed, each or None where one isn't a number in range.
    fn typed(&self) -> [Option<i32>; 3] {
        self.coordinates.each_ref().map(|t| parse_coordinate(t))
    }

    fn position(&self) -> Option<[i32; 3]> {
        let [x, y, z] = self.typed();
        Some([x?, y?, z?])
    }

    fn can_save(&self) -> bool {
        self.position().is_some() && !self.edited.name().is_empty()
    }

    fn save(&mut self, cx: &mut Cx) {
        let Some([x, y, z]) = self.position().filter(|_| !self.edited.name().is_empty()) else {
            return;
        };
        (self.edited.x, self.edited.y, self.edited.z) = (x, y, z);
        let (edited, adding) = (self.edited.clone(), self.adding);
        waypoint::with(|s| {
            if adding {
                s.add(edited);
            } else {
                s.set(&edited);
            }
            s.flush(&mut HostFiles(&mut *cx.host));
        });
        cx.go(Go::Back);
    }

    fn delete(&mut self, cx: &mut Cx) {
        let id = self.edited.id();
        waypoint::with(|s| {
            s.remove(id);
            s.flush(&mut HostFiles(&mut *cx.host));
        });
        cx.go(Go::Back);
    }

    /// Where the player stands: the fields take it, and in another dimension the waypoint moves
    /// there with its name and look.
    fn use_here(&mut self) {
        let Some(p) = waypoint::player() else {
            return;
        };
        self.coordinates = p.block().map(|v| v.to_string());
        if self.edited.dimension != p.dimension {
            self.edited.dimension = p.dimension;
            self.edited.death = false;
        }
    }
}

/// Java's `Integer.parseInt` of the stripped text, within the world border.
pub fn parse_coordinate(text: &str) -> Option<i32> {
    let t = text.trim();
    let digits = t.strip_prefix(['+', '-']).unwrap_or(t);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    t.parse::<i32>().ok().filter(|v| v.abs() <= MAX_COORDINATE)
}

/// The panel's size for a window `[w, h]`.
pub fn panel_size(size: [f32; 2]) -> [f32; 2] {
    let n = COLORS.len() as f32;
    let content = SWATCH * n + SWATCH_GAP * (n - 1.0) + 6.0 + 76.0;
    let w = (size[0] - 16.0).min(2.0 * PADDING + LABEL + content);
    let h = (size[1] - 12.0).min(2.0 * PADDING + 14.0 + 34.0 + 5.0 * LINE + 4.0 * LINE_GAP + 12.0 + LINE);
    [w.floor(), h.floor()]
}

impl Screen for WaypointEdit {
    fn draw(&mut self, cx: &mut Cx) {
        let [pw, ph] = panel_size(cx.size);
        let px = ((cx.size[0] - pw) / 2.0).floor();
        let py = ((cx.size[1] - ph) / 2.0).floor();
        panels::panel(cx.p, px, py, pw, ph, 1.0);
        let th = cx.p.theme;
        let left = px + PADDING + LABEL;
        let right = px + pw - PADDING;
        let preview_y = py + PADDING + 14.0;
        let name_id = ui::id("waypoint-name", 0);
        if !self.focused {
            self.focused = true;
            if self.adding {
                // Typing names it at once. The key that opened the editor (N) is typed this frame
                // too: that letter isn't the name's.
                cx.ui.focus = Some(name_id);
                cx.ui.caret = self.name.chars().count();
                cx.ui.typed.clear();
            }
        }
        // Enter saves, wherever the keys are.
        if (cx.ui.key(keys::ENTER) || cx.ui.key(keys::KEYPAD_ENTER)) && self.can_save() {
            self.save(cx);
            return;
        }

        let title = if self.adding { "New waypoint" } else { "Edit waypoint" };
        cx.p.text(title, px + PADDING, py + PADDING, th.text, false);
        let where_ = format!("In {}", waypoint::dimension_name(&self.edited.dimension));
        let ww = cx.p.width(&where_);
        cx.p.text(&where_, px + pw - PADDING - ww, py + PADDING, th.muted, false);
        self.preview(cx, px + PADDING, preview_y, pw - 2.0 * PADDING);

        let label = |cx: &mut Cx, text: &str, y: f32| {
            let muted = cx.p.theme.muted;
            cx.p.text(text, px + PADDING, y + ((LINE - 8.0) / 2.0).floor(), muted, false);
        };
        let mut y = preview_y + 34.0;

        // Name.
        label(cx, "Name", y);
        if ui::text_field(cx.p, cx.ui, name_id, [left, y, right - left, LINE], &mut self.name, NAME_HINT, MAX_NAME) {
            self.edited.set_name(&self.name);
        }
        y += LINE + LINE_GAP;

        // Colour: the palette, then any colour by hex.
        label(cx, "Color", y);
        let mut x = left;
        for rgb in COLORS {
            let r = [x, y + ((LINE - SWATCH) / 2.0).floor(), SWATCH, SWATCH];
            let chosen = self.edited.rgb() == rgb;
            let c = SWATCH / 2.0;
            if chosen || cx.ui.over(r) {
                cx.p.ring(r[0] + c, r[1] + c, c, 1.5, if chosen { th.accent } else { th.muted });
            }
            cx.p.ring(r[0] + c, r[1] + c, c - 3.0, c - 3.0, 0xFF00_0000 | rgb);
            if cx.ui.clicked(r) {
                self.edited.set_rgb(rgb);
            }
            x += SWATCH + SWATCH_GAP;
        }
        let hex_id = ui::id("waypoint-color", 0);
        if cx.ui.focus != Some(hex_id) {
            self.hex = format!("#{:06X}", self.edited.rgb());
        }
        let hex_r = [x + 2.0, y, right - x - 2.0, LINE];
        let field = [hex_r[0], hex_r[1], hex_r[2] - 20.0, hex_r[3]];
        if ui::text_field(cx.p, cx.ui, hex_id, field, &mut self.hex, "", 7)
            && let Some(argb) = parse_hex(&self.hex).filter(|_| self.hex.len() == 7)
        {
            self.edited.set_rgb(argb);
        }
        if parse_hex(&self.hex).filter(|_| self.hex.len() == 7).is_none() {
            cx.p.outline(field[0], field[1], field[2], field[3], th.radius, th.negative);
        }
        let (sx, sy) = (hex_r[0] + hex_r[2] - 16.0, hex_r[1] + 4.0);
        cx.p.fill(sx - 1.0, sy - 1.0, 14.0, 14.0, th.muted);
        cx.p.fill(sx, sy, 12.0, 12.0, self.edited.argb());
        y += LINE + LINE_GAP;

        // Icon.
        label(cx, "Icon", y);
        let mut x = left;
        for shape in MARKERS {
            let r = [x, y, SHAPE_WIDTH, LINE];
            let chosen = self.edited.icon == shape;
            let hot = cx.ui.over(r);
            panels::card(cx.p, r[0], r[1], r[2], r[3], chosen || hot, chosen, 1.0);
            if chosen {
                cx.p.outline(r[0], r[1], r[2], r[3], th.radius + 1.0, th.accent);
            }
            let color = if chosen { self.edited.argb() } else { alpha(self.edited.argb(), 0.75) };
            cx.p.shape(shape, r[0] + r[2] / 2.0, r[1] + r[3] / 2.0, 12.0, color);
            if hot {
                cx.ui.tooltip(waypoint::shape_label(shape));
            }
            if cx.ui.clicked(r) {
                self.edited.icon = shape;
            }
            x += SHAPE_WIDTH + SHAPE_GAP;
        }
        y += LINE + LINE_GAP;

        // Position: x, y and z, and where the player stands.
        label(cx, "Position", y);
        let field_w = ((right - left - HERE_WIDTH - 3.0 * 14.0) / 3.0).floor();
        let typed = self.typed();
        let mut x = left;
        for i in 0..3 {
            let r = [x + 10.0, y, field_w, LINE];
            let muted = th.muted;
            cx.p.text(AXES[i], r[0] - 9.0, r[1] + 6.0, muted, false);
            ui::text_field(cx.p, cx.ui, ui::id("waypoint-xyz", i), r, &mut self.coordinates[i], "", 9);
            if typed[i].is_none() {
                cx.p.outline(r[0], r[1], r[2], r[3], th.radius, th.negative);
            }
            x += 10.0 + field_w + 4.0;
        }
        let here = [x, y, right - x, LINE];
        let player = waypoint::player();
        if ui::menu_button(cx.p, cx.ui, here, "Here", Some(Icon::Target), "", player.is_some()) {
            self.use_here();
        }
        if cx.ui.over(here) {
            cx.ui.tooltip("Where you stand now");
        }
        y += LINE + LINE_GAP;

        // Shown, and the beam.
        label(cx, "Show", y);
        let half = ((right - left - GAP) / 2.0).floor();
        if switch(cx, [left, y, half, LINE], "On the map and screen", self.edited.visible) {
            self.edited.visible = !self.edited.visible;
        }
        if switch(cx, [left + half + GAP, y, half, LINE], "Beacon beam", self.edited.beam) {
            self.edited.beam = !self.edited.beam;
        }

        // Delete (an existing one), Cancel, Save.
        let bottom = py + ph - PADDING - LINE;
        let keep = if self.adding { "Add" } else { "Save" };
        let can_save = self.can_save();
        if ui::button(cx.p, cx.ui, [right - BUTTON_WIDTH, bottom, BUTTON_WIDTH, LINE], keep, can_save) {
            self.save(cx);
            return;
        }
        let cancel = cx.host.translate("gui.cancel");
        let cancel_r = [right - 2.0 * BUTTON_WIDTH - GAP, bottom, BUTTON_WIDTH, LINE];
        if ui::button(cx.p, cx.ui, cancel_r, &cancel, true) {
            cx.go(Go::Back);
        }
        if !self.adding
            && ui::menu_button(cx.p, cx.ui, [px + PADDING, bottom, BUTTON_WIDTH, LINE], "Delete", Some(Icon::Trash), "", true)
        {
            self.delete(cx);
        }
    }
}

impl WaypointEdit {
    /// The waypoint as the world will show it: its icon, and its name and distance on a tag.
    fn preview(&self, cx: &mut Cx, left: f32, top: f32, width: f32) {
        let th = cx.p.theme;
        cx.p.round(left, top, width, 28.0, th.radius + 1.0, th.field);
        let named = !self.edited.name().is_empty();
        let shown_name = if named { self.edited.name() } else { NAME_HINT };
        let far = match (waypoint::player(), self.position()) {
            (Some(p), Some([x, y, z])) if p.dimension == self.edited.dimension => {
                let (dx, dy, dz) = (x as f64 + 0.5 - p.x, y as f64 + 0.5 - p.y, z as f64 + 0.5 - p.z);
                let d = (dx * dx + dy * dy + dz * dz).sqrt();
                if d < 2.0 { "here".to_owned() } else { waypoint::distance_text(d) }
            }
            _ => String::new(),
        };
        let name_w = cx.p.width(shown_name);
        let far_w = if far.is_empty() { 0.0 } else { cx.p.width(&far) + 5.0 };
        let tag_w = 8.0 + name_w + far_w;
        let icon = 12.0;
        let total = icon + 6.0 + tag_w;
        let x = left + ((width - total) / 2.0).floor();
        let cy = top + 14.0;
        let color = if self.edited.visible { self.edited.argb() } else { alpha(self.edited.argb(), 0.4) };
        cx.p.shape(self.edited.icon, x + icon / 2.0, cy, icon, color);
        let tag_x = x + icon + 6.0;
        cx.p.round(tag_x, cy - 7.0, tag_w, 14.0, th.radius, th.background);
        cx.p.text(shown_name, tag_x + 4.0, cy - 4.0, if named { th.text } else { th.muted }, true);
        if !far.is_empty() {
            cx.p.text(&far, tag_x + 4.0 + name_w + 5.0, cy - 4.0, th.muted, true);
        }
    }
}

/// A label and an on/off switch on one card; true when clicked anywhere on it.
fn switch(cx: &mut Cx, r: Rect, label: &str, on: bool) -> bool {
    let hot = cx.ui.over(r);
    panels::card(cx.p, r[0], r[1], r[2], r[3], hot, false, 1.0);
    let th = cx.p.theme;
    let text = cx.p.fit(label, r[2] - 16.0 - panels::SWITCH_WIDTH).into_owned();
    cx.p.text(&text, r[0] + 8.0, r[1] + ((r[3] - 8.0) / 2.0).floor(), th.text, false);
    let sx = r[0] + r[2] - 7.0 - panels::SWITCH_WIDTH;
    panels::toggle(cx.p, sx, r[1] + ((r[3] - panels::SWITCH_HEIGHT) / 2.0).floor(), on, 1.0);
    cx.ui.clicked(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Shape;

    #[test]
    fn reads_coordinates_as_java_does() {
        assert_eq!(parse_coordinate(" -12 "), Some(-12));
        assert_eq!(parse_coordinate("+7"), Some(7));
        assert_eq!(parse_coordinate("30000001"), None);
        assert_eq!(parse_coordinate("1.5"), None);
        assert_eq!(parse_coordinate("-"), None);
        assert_eq!(parse_coordinate(""), None);
    }

    #[test]
    fn fits_the_java_panel() {
        assert_eq!(panel_size([1000.0, 1000.0]), [370.0, 228.0]);
        assert_eq!(panel_size([320.0, 240.0]), [304.0, 228.0]);
    }

    #[test]
    fn saves_only_with_a_name_and_a_place() {
        let mut e = WaypointEdit::new(Waypoint::new("A", 1, 2, 3, "d", 0, Shape::Circle), true);
        assert!(e.can_save());
        e.coordinates[1] = "up".into();
        assert!(!e.can_save());
        e.coordinates[1] = "64".into();
        e.edited.set_name("   ");
        assert!(!e.can_save());
    }
}
