//! The world's waypoints (WaypointListScreen.java, ADR 0051): one glass panel with a search field
//! and New waypoint at the top, and a row for each waypoint, those in the player's dimension first
//! and nearest first, then the other dimensions' under their names. A row shows the waypoint's
//! icon in its colour, its name, where it is and how far; its eye shows or hides it, its pencil (or
//! a click on the row) opens the editor, and its bin deletes it, after a second click to be sure.
//! Opened by the Waypoints module's key or its settings' Manage... button.
use super::waypoint_edit::WaypointEdit;
use crate::client::HostFiles;
use crate::color::alpha;
use crate::draw::{Rect, intersect};
use crate::icons::Icon;
use crate::paint::{Painter, Shape};
use crate::panels;
use crate::screen::{Cx, Go, Screen};
use crate::theme::{CONTROL_HEIGHT, GAP as THEME_GAP};
use crate::ui;
use crate::waypoint::{self, Waypoint};

const PADDING: f32 = 10.0;
const ROW: f32 = 30.0;
const GAP: f32 = 4.0;
const HEADING: f32 = 18.0;
const BUTTON: f32 = 16.0;
const SCROLL_STEP: f32 = 24.0;
/// How long a bin waits for its second click, in milliseconds.
const CONFIRM_MILLIS: f64 = 3000.0;
const NEW: &str = "New waypoint";
const TOOLTIPS: [&str; 4] = ["Hide it", "Show it", "Edit", "Delete"];

/// A line of the list: a dimension's heading, or a waypoint's row with its second line.
enum Line {
    Heading(String),
    Row { id: u64, detail: String },
}

#[derive(Default)]
pub struct WaypointList {
    query: String,
    scroll: f32,
    content: f32,
    lines: Vec<Line>,
    /// The waypoints' revision and the query the lines were made for.
    made: Option<(u64, String)>,
    /// Bins waiting for their second click: the waypoint, and until when.
    confirming: Vec<(u64, f64)>,
}

impl WaypointList {
    pub fn new() -> Self {
        Self::default()
    }

    /// The lines again, from the waypoints as they are now.
    fn rebuild(&mut self, all: &[Waypoint], revision: u64) {
        self.made = Some((revision, self.query.clone()));
        self.lines.clear();
        let player = waypoint::player();
        let here = player
            .as_ref()
            .map_or(waypoint::OVERWORLD, |p| p.dimension.as_str());
        let needle = self.query.trim().to_lowercase();
        let matching = all
            .iter()
            .filter(|w| needle.is_empty() || w.name().to_lowercase().contains(&needle));
        // This dimension's first, nearest first; then each other dimension's, by name.
        let (mut local, mut elsewhere): (Vec<&Waypoint>, Vec<&Waypoint>) =
            matching.partition(|w| w.dimension == here);
        if let Some(p) = &player {
            local.sort_by(|a, b| {
                a.distance_squared(p.x, p.y, p.z)
                    .total_cmp(&b.distance_squared(p.x, p.y, p.z))
            });
        }
        elsewhere.sort_by(|a, b| {
            a.dimension
                .cmp(&b.dimension)
                .then_with(|| a.name().cmp(b.name()))
        });
        let place = |w: &Waypoint| format!("{}, {}, {}", w.x, w.y, w.z);
        for w in local {
            let detail = match &player {
                Some(p) => format!(
                    "{}  ·  {}",
                    place(w),
                    waypoint::distance_text(w.distance_squared(p.x, p.y, p.z).sqrt())
                ),
                None => place(w),
            };
            self.lines.push(Line::Row { id: w.id(), detail });
        }
        let mut dimension: Option<&str> = None;
        for w in elsewhere {
            if dimension != Some(w.dimension.as_str()) {
                dimension = Some(&w.dimension);
                self.lines.push(Line::Heading(format!(
                    "In {}",
                    waypoint::dimension_name(&w.dimension)
                )));
            }
            self.lines.push(Line::Row {
                id: w.id(),
                detail: place(w),
            });
        }
    }
}

/// The screen an action setting's button opens: the Waypoints module's Manage... (for the client's
/// frame, which hears the press through `ModuleSystem::actions`).
pub fn for_action(module: &str, setting: &str) -> Option<Box<dyn Screen>> {
    (module == "waypoints" && setting == "manage")
        .then(|| Box::new(WaypointList::new()) as Box<dyn Screen>)
}

/// The rectangles of a row's eye, pencil and bin.
fn row_buttons(r: Rect) -> [Rect; 3] {
    let right = r[0] + r[2];
    let y = r[1] + ((ROW - BUTTON) / 2.0).floor();
    std::array::from_fn(|i| {
        let x = right - 8.0 - (3 - i) as f32 * BUTTON - (2 - i) as f32 * 4.0;
        [x, y, BUTTON, BUTTON]
    })
}

impl Screen for WaypointList {
    fn draw(&mut self, cx: &mut Cx) {
        let [w, h] = cx.size;
        let pw = (w - 24.0).min(380.0).floor();
        let ph = (h - 20.0).min(300.0).floor();
        let px = ((w - pw) / 2.0).floor();
        let py = ((h - ph) / 2.0).floor();
        panels::panel(cx.p, px, py, pw, ph, 1.0);
        let th = cx.p.theme;
        let (all, revision) =
            waypoint::with(|s| (s.all().to_vec(), s.revision())).unwrap_or_default();
        if self
            .made
            .as_ref()
            .is_none_or(|(r, q)| *r != revision || *q != self.query)
        {
            self.rebuild(&all, revision);
        }
        let player = waypoint::player();

        // The title and the count.
        cx.p.text("Waypoints", px + PADDING, py + PADDING, th.text, false);
        let count = if all.len() == 1 {
            "1 waypoint".to_owned()
        } else {
            format!("{} waypoints", all.len())
        };
        let cw = cx.p.width(&count);
        cx.p.text(
            &count,
            px + pw - PADDING - cw,
            py + PADDING,
            th.muted,
            false,
        );

        // Search, and New waypoint.
        let top = py + PADDING + 14.0;
        let new_w = 24.0 + cx.p.width(NEW) + 10.0;
        let new_r = [px + pw - PADDING - new_w, top, new_w, CONTROL_HEIGHT];
        if ui::menu_button(
            cx.p,
            cx.ui,
            new_r,
            NEW,
            Some(Icon::Plus),
            "",
            player.is_some(),
        ) && let Some(p) = &player
        {
            let [x, y, z] = p.block();
            cx.open(WaypointEdit::new(
                waypoint::draft(&all, x, y, z, &p.dimension),
                true,
            ));
        }
        let search = [
            px + PADDING,
            top,
            pw - 2.0 * PADDING - new_w - THEME_GAP,
            CONTROL_HEIGHT,
        ];
        if ui::text_field(
            cx.p,
            cx.ui,
            ui::id("waypoint-search", 0),
            search,
            &mut self.query,
            "Search waypoints",
            64,
        ) {
            self.scroll = 0.0;
        }

        // Done.
        let done_y = py + ph - PADDING - CONTROL_HEIGHT;
        let done = cx.host.translate("gui.done");
        if ui::button(
            cx.p,
            cx.ui,
            [
                px + ((pw - 120.0) / 2.0).floor(),
                done_y,
                120.0,
                CONTROL_HEIGHT,
            ],
            &done,
            true,
        ) {
            cx.go(Go::Back);
        }

        // The rows, scrolled inside the list's clip.
        let list = [
            px + PADDING,
            top + CONTROL_HEIGHT + 8.0,
            pw - 2.0 * PADDING,
            0.0,
        ];
        let area = [list[0], list[1], list[2], (done_y - 8.0 - list[1]).max(0.0)];
        ui::scroll(cx.ui, area, &mut self.scroll, self.content, SCROLL_STEP);
        let now = cx.host.now_ms();
        self.confirming.retain(|c| c.1 > now);
        cx.p.clip_start([area[0] - 4.0, area[1], area[2] + 8.0, area[3]]);
        let mut y = area[1] - self.scroll.floor();
        let mut act: Option<(u64, usize)> = None;
        for line in &self.lines {
            match line {
                Line::Heading(text) => {
                    cx.p.text(text, area[0] + 2.0, y + HEADING - 13.0, th.accent, false);
                    y += HEADING;
                }
                Line::Row { id, detail } => {
                    let r = [area[0], y, area[2], ROW];
                    y += ROW + GAP;
                    if r[1] + r[3] <= area[1] || r[1] >= area[1] + area[3] {
                        continue;
                    }
                    let Some(wp) = all.iter().find(|w| w.id() == *id) else {
                        continue;
                    };
                    let confirming = self.confirming.iter().any(|c| c.0 == *id);
                    if let Some(i) = row(cx, wp, detail, r, area, confirming) {
                        act = Some((*id, i));
                    }
                }
            }
        }
        cx.p.clip_end();
        self.content = (y + self.scroll.floor() - area[1]).max(0.0);
        if self.lines.is_empty() {
            self.empty(cx, area, !all.is_empty());
        }
        if self.content > area[3] {
            let thumb = (area[3] * area[3] / self.content).max(16.0);
            let max = self.content - area[3];
            let t = area[1] + ((area[3] - thumb) * self.scroll / max).floor();
            cx.p.round(area[0] + area[2] + 3.0, t, 2.0, thumb, 1.0, 0x80FF_FFFF);
        }

        // What a row's press asked for.
        match act {
            Some((id, 0)) => {
                waypoint::with(|s| s.update(id, |w| w.visible = !w.visible));
            }
            Some((id, 2)) => {
                if self.confirming.iter().any(|c| c.0 == id) {
                    self.confirming.retain(|c| c.0 != id);
                    waypoint::with(|s| s.remove(id));
                } else {
                    self.confirming.push((id, now + CONFIRM_MILLIS));
                }
            }
            Some((id, _)) => {
                if let Some(wp) = all.iter().find(|w| w.id() == id) {
                    cx.open(WaypointEdit::new(wp.clone(), false));
                }
            }
            None => {}
        }
    }

    fn closed(&mut self, cx: &mut Cx) {
        waypoint::with(|s| s.flush(&mut HostFiles(&mut *cx.host)));
    }
}

impl WaypointList {
    /// What an empty list says: how to make the first waypoint, or that the search found none.
    fn empty(&self, cx: &mut Cx, area: Rect, some: bool) {
        let th = cx.p.theme;
        let center = area[0] + (area[2] / 2.0).floor();
        let y = area[1] + (area[3] / 2.0).floor() - 16.0;
        if some {
            let text = format!("No waypoints match \"{}\"", self.query.trim());
            let shown = cx.p.fit(&text, area[2]).into_owned();
            cx.p.centered(&shown, center, y, th.muted, true);
            return;
        }
        cx.p.shape(Shape::Diamond, center, y - 10.0, 14.0, th.accent);
        cx.p.centered(
            "No waypoints in this world yet",
            center,
            y + 6.0,
            th.text,
            true,
        );
        let key = cx
            .modules
            .module("waypoints")
            .map(|m| m.settings.key("new"))
            .filter(|k| k.is_bound());
        let hint = match key {
            Some(k) => format!(
                "Press {} in a world, or New waypoint, to mark your spot.",
                k.label()
            ),
            None => "Press New waypoint to mark your spot.".to_owned(),
        };
        let hint = cx.p.fit(&hint, area[2]).into_owned();
        cx.p.centered(&hint, center, y + 20.0, th.muted, true);
    }
}

/// One waypoint's row: the card, its icon, name and detail, and its three buttons. Returns the
/// part pressed: 0 the eye, 1 the pencil (or the row), 2 the bin.
fn row(
    cx: &mut Cx,
    wp: &Waypoint,
    detail: &str,
    r: Rect,
    area: Rect,
    confirming: bool,
) -> Option<usize> {
    let th = cx.p.theme;
    let shown = wp.visible;
    let buttons = row_buttons(r);
    let visible = |r: Rect| intersect(r, area);
    let hover = if cx.ui.over(visible(r)) {
        buttons.iter().position(|b| cx.ui.over(visible(*b)))
    } else {
        None
    };
    panels::card(
        cx.p,
        r[0],
        r[1],
        r[2],
        r[3],
        cx.ui.over(visible(r)) && hover.is_none(),
        false,
        1.0,
    );
    let color = if shown {
        wp.argb()
    } else {
        alpha(wp.argb(), 0.4)
    };
    cx.p.shape(wp.icon, r[0] + 15.0, r[1] + ROW / 2.0, 13.0, color);
    let text_x = r[0] + 30.0;
    let name_right = buttons[0][0] - 8.0;
    let name = cx.p.fit(wp.name(), name_right - text_x).into_owned();
    cx.p.text(
        &name,
        text_x,
        r[1] + 6.0,
        if shown { th.text } else { th.muted },
        false,
    );
    if wp.beam && shown {
        // A sliver of the beam's colour after the name: this one has a beam.
        let beam_x = text_x + cx.p.width(&name) + 5.0;
        if beam_x + 2.0 < name_right {
            cx.p.round(beam_x, r[1] + 5.0, 2.0, 9.0, 1.0, alpha(wp.argb(), 0.7));
        }
    }
    let second = if confirming {
        "Click the bin again to delete".to_owned()
    } else if shown {
        detail.to_owned()
    } else {
        format!("{detail}  ·  hidden")
    };
    let second = cx.p.fit(&second, name_right - text_x).into_owned();
    cx.p.text(
        &second,
        text_x,
        r[1] + 17.0,
        if confirming { th.negative } else { th.muted },
        false,
    );
    let icons = [
        if shown { Icon::Eye } else { Icon::EyeOff },
        Icon::Pencil,
        Icon::Trash,
    ];
    for (i, b) in buttons.iter().enumerate() {
        let hot = hover == Some(i) || i == 2 && confirming;
        let hot_color = if i == 2 && confirming {
            th.negative
        } else {
            th.accent
        };
        icon(cx.p, *b, icons[i], hot, hot_color);
    }
    if let Some(i) = hover {
        let tip = match i {
            0 => TOOLTIPS[if shown { 0 } else { 1 }],
            1 => TOOLTIPS[2],
            _ => TOOLTIPS[3],
        };
        cx.ui.tooltip(tip);
    }
    for (i, b) in buttons.iter().enumerate() {
        if cx.ui.clicked(visible(*b)) {
            return Some(i);
        }
    }
    cx.ui.clicked(visible(r)).then_some(1)
}

fn icon(p: &mut Painter, r: Rect, icon: Icon, hot: bool, hot_color: u32) {
    let th = p.theme;
    if hot {
        p.round(r[0], r[1], r[2], r[3], th.radius, th.button_hot);
    }
    let size = 10.0;
    let color = if hot { hot_color } else { alpha(th.muted, 0.9) };
    p.icon(
        icon,
        r[0] + ((BUTTON - size) / 2.0).floor(),
        r[1] + ((BUTTON - size) / 2.0).floor(),
        size,
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::waypoint::{NETHER, OVERWORLD};

    #[test]
    fn lists_this_dimension_nearest_first_then_the_others_by_name() {
        waypoint::set_player(Some(([0.0, 64.0, 0.0], 0.0, OVERWORLD)));
        let all = vec![
            Waypoint::new("Far", 100, 64, 0, OVERWORLD, 0, Shape::Circle),
            Waypoint::new("Zed", 0, 64, 0, NETHER, 0, Shape::Circle),
            Waypoint::new("Near", 3, 64, 0, OVERWORLD, 0, Shape::Circle),
            Waypoint::new("Alpha", 0, 64, 0, NETHER, 0, Shape::Circle),
        ];
        let mut list = WaypointList::new();
        list.rebuild(&all, 1);
        let names: Vec<String> = list
            .lines
            .iter()
            .map(|l| match l {
                Line::Heading(h) => h.clone(),
                Line::Row { id, detail } => format!(
                    "{} ({detail})",
                    all.iter().find(|w| w.id() == *id).unwrap().name()
                ),
            })
            .collect();
        assert_eq!(
            names,
            [
                "Near (3, 64, 0  ·  3 m)",
                "Far (100, 64, 0  ·  100 m)",
                "In the Nether",
                "Alpha (0, 64, 0)",
                "Zed (0, 64, 0)"
            ]
        );
        list.query = " ne ".into();
        list.rebuild(&all, 1);
        assert_eq!(list.lines.len(), 1);
        waypoint::set_player(None);
    }

    #[test]
    fn places_a_rows_buttons_as_java_does() {
        let b = row_buttons([10.0, 20.0, 300.0, ROW]);
        assert_eq!(b[2], [10.0 + 300.0 - 8.0 - 16.0, 27.0, 16.0, 16.0]);
        assert_eq!(b[0][0], 310.0 - 8.0 - 48.0 - 8.0);
        assert!(
            for_action("waypoints", "manage").is_some() && for_action("waypoints", "new").is_none()
        );
    }
}
