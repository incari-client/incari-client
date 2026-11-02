//! The world map (WorldMapScreen.java, ADR 0051): the land the map has seen (`MapSource`, the
//! remembered map by default) across the whole screen, dragged about with the mouse and zoomed
//! with the wheel round the pointer. The player is an arrow, each waypoint its icon and name; a
//! click on one edits it, a right-click elsewhere makes a new one there. Center (or Space) comes
//! back to the player; the map's key or Escape closes it. Opened by the Minimap module's key (M).
//!
//! Java gives each 512 by 512 region a texture. Here a region is drawn as runs of fills, made
//! from its colours at a cell size fitting the zoom when it first comes into view (two a frame at
//! most) and again when more of it has been seen, at most twice a second; up to 48 kept while the
//! map is open. (Images on the canvas would be cheaper: they need a canvas key for map tiles,
//! which `canvas::Key` hasn't yet.)
use super::waypoint_edit::WaypointEdit;
use super::waypoint_list::WaypointList;
use crate::draw::Rect;
use crate::icons::Icon;
use crate::map_memory::{HostBytes, MapSource, REGION_BLOCKS, Region, Remembered};
use crate::panels;
use crate::paint::Shape;
use crate::screen::{Cx, Go, Screen};
use crate::theme::{CONTROL_HEIGHT, GAP};
use crate::ui;
use crate::waypoint::{self, Player, Waypoint};
use crate::waypoint_overlay::turned_shape;
use std::collections::HashMap;

/// GUI units a block, the least and the most.
pub const MIN_SCALE: f32 = 1.0 / 8.0;
pub const MAX_SCALE: f32 = 8.0;
/// Each notch of the wheel zooms by this much.
pub const ZOOM_STEP: f32 = 1.25;
/// Regions made a frame at most.
const BUILDS_A_FRAME: usize = 2;
/// Regions kept at most while the map's open.
const KEPT: usize = 48;
/// How long a missing region's absence is believed before it's looked for again.
const MISSING_MILLIS: f64 = 2000.0;
/// How soon a region seen more is made again.
const REMAKE_MILLIS: f64 = 500.0;
/// A cell's least size on screen, GUI units: smaller, a cell takes more blocks.
const MIN_CELL: f32 = 2.0;
/// The land nobody's seen.
const UNSEEN: u32 = 0xFF0E_1319;
/// The lines between regions, faint.
const GRID: u32 = 0x14FF_FFFF;
const BAR_HEIGHT: f32 = 32.0;
const TITLE: &str = "World map";
/// HID's Space.
const SPACE: u16 = 44;

/// A region as runs of one colour along each row of cells.
struct Runs {
    revision: u32,
    /// Blocks a cell's side.
    cell: i32,
    made_at: f64,
    used_at: u64,
    /// (row, first cell, cells, colour).
    runs: Vec<(u16, u16, u16, u32)>,
}

/// A region's runs at `cell` blocks a cell: each cell the colour of its middle block; unseen and
/// clear cells are left out.
fn runs(region: &Region, cell: i32) -> Vec<(u16, u16, u16, u32)> {
    let n = REGION_BLOCKS / cell;
    let mut out = Vec::new();
    for row in 0..n {
        let mut run: Option<(u16, u16, u32)> = None;
        for col in 0..n {
            let color = region
                .color(col * cell + cell / 2, row * cell + cell / 2)
                .filter(|c| c >> 24 != 0);
            match (run, color) {
                (Some((start, len, c)), Some(color)) if c == color => run = Some((start, len + 1, c)),
                (_, color) => {
                    if let Some((start, len, c)) = run {
                        out.push((row as u16, start, len, c));
                    }
                    run = color.map(|c| (col as u16, 1, c));
                }
            }
        }
        if let Some((start, len, c)) = run {
            out.push((row as u16, start, len, c));
        }
    }
    out
}

/// The cell size for a zoom: the fewest blocks (a power of two) whose cell is `MIN_CELL` across.
pub fn cell_for(scale: f32) -> i32 {
    let mut cell = 1;
    while (cell as f32) * scale < MIN_CELL && cell < 64 {
        cell *= 2;
    }
    cell
}

/// Zooms `scale` by the wheel's `steps` (up is in), kept in its range.
pub fn zoomed(scale: f32, steps: f32) -> f32 {
    (scale * ZOOM_STEP.powf(steps)).clamp(MIN_SCALE, MAX_SCALE)
}

pub struct WorldMap {
    source: Box<dyn MapSource>,
    center: [f64; 2],
    scale: f32,
    centered: bool,
    /// Where a left press began on the map, and the pointer at the last frame while dragging.
    drag: Option<([f32; 2], [f32; 2])>,
    dragged: bool,
    regions: HashMap<(i32, i32), Runs>,
    missing: HashMap<(i32, i32), f64>,
    dimension: String,
    frame: u64,
    /// This frame's tags, as left, top, right, bottom.
    tags: Vec<[f32; 4]>,
}

impl Default for WorldMap {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldMap {
    /// The world map of the remembered map.
    pub fn new() -> Self {
        Self::with_source(Box::new(Remembered))
    }

    /// The world map of another source (a test's, or one the game gives).
    pub fn with_source(source: Box<dyn MapSource>) -> Self {
        Self {
            source,
            center: [0.0, 0.0],
            scale: 1.0,
            centered: false,
            drag: None,
            dragged: false,
            regions: HashMap::new(),
            missing: HashMap::new(),
            dimension: String::new(),
            frame: 0,
            tags: Vec::new(),
        }
    }

    fn screen_x(&self, x: f64, w: f32) -> f32 {
        ((x - self.center[0]) * self.scale as f64 + w as f64 / 2.0) as f32
    }

    fn screen_y(&self, z: f64, h: f32) -> f32 {
        ((z - self.center[1]) * self.scale as f64 + h as f64 / 2.0) as f32
    }

    fn world(&self, at: [f32; 2], size: [f32; 2]) -> [f64; 2] {
        [
            self.center[0] + (at[0] as f64 - size[0] as f64 / 2.0) / self.scale as f64,
            self.center[1] + (at[1] as f64 - size[1] as f64 / 2.0) / self.scale as f64,
        ]
    }

    /// Zooms by the wheel's `steps` round `at`: the block under it stays under it.
    pub fn zoom_at(&mut self, at: [f32; 2], size: [f32; 2], steps: f32) {
        let [x, z] = self.world(at, size);
        self.scale = zoomed(self.scale, steps);
        self.center = [
            x - (at[0] as f64 - size[0] as f64 / 2.0) / self.scale as f64,
            z - (at[1] as f64 - size[1] as f64 / 2.0) / self.scale as f64,
        ];
    }

    /// The land: every region in view, its runs made when stale.
    fn land(&mut self, cx: &mut Cx) {
        let size = cx.size;
        let [w, h] = size;
        cx.p.fill(0.0, 0.0, w, h, UNSEEN);
        let [x0, z0] = self.world([0.0, 0.0], size);
        let [x1, z1] = self.world(size, size);
        let region = |v: f64| (v.floor() as i32).div_euclid(REGION_BLOCKS);
        let (rx0, rx1, rz0, rz1) = (region(x0), region(x1), region(z0), region(z1));
        let now = cx.host.now_ms();
        let cell = cell_for(self.scale);
        let mut builds = 0;
        let mut files = HostBytes(&mut *cx.host);
        for rz in rz0..=rz1 {
            for rx in rx0..=rx1 {
                let key = (rx, rz);
                if self.missing.get(&key).is_some_and(|t| now - t < MISSING_MILLIS) {
                    continue;
                }
                let kept = self.regions.get(&key).map(|r| (r.revision, r.cell, r.made_at));
                let mut found = false;
                let mut made: Option<Runs> = None;
                let can_build = builds < BUILDS_A_FRAME;
                self.source.region(&mut files, &self.dimension, rx, rz, &mut |r| {
                    let Some(r) = r else {
                        return;
                    };
                    found = true;
                    let stale = match kept {
                        None => true,
                        Some((revision, c, at)) => c != cell || revision != r.revision && now - at > REMAKE_MILLIS,
                    };
                    if stale && can_build {
                        made = Some(Runs { revision: r.revision, cell, made_at: now, used_at: 0, runs: runs(r, cell) });
                    }
                });
                if !found {
                    self.missing.insert(key, now);
                    continue;
                }
                self.missing.remove(&key);
                if let Some(m) = made {
                    builds += 1;
                    self.regions.insert(key, m);
                }
                let Some(runs) = self.regions.get_mut(&key) else {
                    continue;
                };
                runs.used_at = self.frame;
                let (bx, bz) = ((rx * REGION_BLOCKS) as f64, (rz * REGION_BLOCKS) as f64);
                let c = runs.cell as f64;
                let scale = self.scale as f64;
                let (ox, oz) = (
                    (bx - self.center[0]) * scale + w as f64 / 2.0,
                    (bz - self.center[1]) * scale + h as f64 / 2.0,
                );
                for &(row, start, len, color) in &runs.runs {
                    let top = (oz + row as f64 * c * scale) as f32;
                    let bottom = (oz + (row as f64 + 1.0) * c * scale) as f32;
                    if bottom < 0.0 || top > h {
                        continue;
                    }
                    let left = (ox + start as f64 * c * scale) as f32;
                    let right = (ox + (start as f64 + len as f64) * c * scale) as f32;
                    if right < 0.0 || left > w {
                        continue;
                    }
                    cx.p.fill(left, top, right - left, bottom - top, color);
                }
            }
        }
        // Region lines, faint, for a sense of distance.
        if self.scale >= 0.25 {
            for rx in rx0..=rx1 + 1 {
                let x = self.screen_x((rx * REGION_BLOCKS) as f64, w).round();
                cx.p.fill(x, 0.0, 1.0, h, GRID);
            }
            for rz in rz0..=rz1 + 1 {
                let y = self.screen_y((rz * REGION_BLOCKS) as f64, h).round();
                cx.p.fill(0.0, y, w, 1.0, GRID);
            }
        }
        // Over the most kept, those not shown this frame go, the least lately shown first.
        while self.regions.len() > KEPT {
            let oldest = self
                .regions
                .iter()
                .filter(|r| r.1.used_at != self.frame)
                .min_by_key(|r| r.1.used_at)
                .map(|r| *r.0);
            match oldest {
                Some(k) => {
                    self.regions.remove(&k);
                }
                None => break,
            }
        }
    }

    /// The shown waypoint whose icon is under `at`, nearest first.
    fn waypoint_at<'w>(&self, all: &'w [Waypoint], at: [f32; 2], size: [f32; 2]) -> Option<&'w Waypoint> {
        let mut best = None;
        let mut best_d = 8.0 * 8.0;
        for w in all.iter().filter(|w| w.visible && w.dimension == self.dimension) {
            let dx = self.screen_x(w.x as f64 + 0.5, size[0]) - at[0];
            let dy = self.screen_y(w.z as f64 + 0.5, size[1]) - at[1];
            let d = dx * dx + dy * dy;
            if d < best_d {
                best = Some(w);
                best_d = d;
            }
        }
        best
    }

    /// The dimension's shown waypoints: every icon, and the names on tags where they don't cover
    /// one already written (the hovered one's first).
    fn waypoints(&mut self, cx: &mut Cx, all: &[Waypoint], hovered: Option<&Waypoint>) {
        let [w, h] = cx.size;
        self.tags.clear();
        if let Some(hw) = hovered {
            self.tag(cx, hw, true);
        }
        for wp in all {
            if !wp.visible || wp.dimension != self.dimension || hovered.is_some_and(|h| h.id() == wp.id()) {
                continue;
            }
            let x = self.screen_x(wp.x as f64 + 0.5, w);
            let y = self.screen_y(wp.z as f64 + 0.5, h);
            if x < -60.0 || x > w + 60.0 || y < -20.0 || y > h + 20.0 {
                continue;
            }
            cx.p.shape(wp.icon, x, y, 10.0, wp.argb());
            self.tag(cx, wp, false);
        }
        if let Some(hw) = hovered {
            let (x, y) = (self.screen_x(hw.x as f64 + 0.5, w), self.screen_y(hw.z as f64 + 0.5, h));
            cx.p.shape(hw.icon, x, y, 12.0, hw.argb());
        }
    }

    fn tag(&mut self, cx: &mut Cx, wp: &Waypoint, hot: bool) {
        let th = cx.p.theme;
        let tw = cx.p.width(wp.name()) + 8.0;
        let tx = (self.screen_x(wp.x as f64 + 0.5, cx.size[0]) - tw / 2.0).round();
        let ty = (self.screen_y(wp.z as f64 + 0.5, cx.size[1]) - 20.0).round();
        let covers = |o: &[f32; 4]| tx < o[2] + 2.0 && tx + tw > o[0] - 2.0 && ty < o[3] + 1.0 && ty + 12.0 > o[1] - 1.0;
        if self.tags.iter().any(covers) {
            return;
        }
        self.tags.push([tx, ty, tx + tw, ty + 12.0]);
        cx.p.round(tx, ty, tw, 12.0, th.radius, if hot { th.floating } else { th.background });
        if hot {
            cx.p.outline(tx, ty, tw, 12.0, th.radius, wp.argb());
        }
        cx.p.text(wp.name(), tx + 4.0, ty + 2.0, th.text, false);
    }

    /// The player: an arrow the way they face, ringed so it's found at a glance.
    fn player(&self, cx: &mut Cx, p: &Player) {
        let (x, y) = (self.screen_x(p.x, cx.size[0]), self.screen_y(p.z, cx.size[1]));
        let accent = cx.p.theme.accent;
        let text = cx.p.theme.text;
        cx.p.ring(x, y, 9.0, 9.0, 0x8C0E_1319);
        cx.p.ring(x, y, 9.0, 1.25, accent);
        turned_shape(cx.p, Shape::Arrow, x, y, 11.0, p.heading(), text);
    }

    /// A new waypoint at the block under `at`, in the editor: at the surface's height where the
    /// source knows it, else the player's.
    fn add_at(&self, cx: &mut Cx, all: &[Waypoint], p: &Player, at: [f32; 2]) {
        let [x, z] = self.world(at, cx.size).map(|v| v.floor() as i32);
        let y = self.source.surface(x, z).unwrap_or(p.y.floor() as i32);
        cx.open(WaypointEdit::new(waypoint::draft(all, x, y, z, &p.dimension), true));
    }

    /// A menu button `label` wide enough for its text, right of `right`: its rectangle.
    fn bar_button(cx: &mut Cx, right: f32, label: &str) -> Rect {
        let w = 24.0 + cx.p.width(label) + 10.0;
        [right - w, 6.0, w, CONTROL_HEIGHT]
    }
}

impl Screen for WorldMap {
    fn draw(&mut self, cx: &mut Cx) {
        let Some(player) = waypoint::player() else {
            cx.go(Go::Back);
            return;
        };
        self.frame += 1;
        if self.dimension != player.dimension {
            self.dimension.clone_from(&player.dimension);
            self.regions.clear();
            self.missing.clear();
        }
        if !self.centered {
            self.center = [player.x, player.z];
            self.centered = true;
        }
        let size = cx.size;
        let [w, h] = size;
        let mouse = cx.ui.mouse;
        let all = waypoint::with(|s| s.all().to_vec()).unwrap_or_default();

        // The map's own key closes it, as the inventory's key closes the inventory; Space centres.
        let map_key = cx
            .modules
            .module("minimap")
            .map(|m| m.settings.key("world-map"))
            .filter(|k| k.is_bound());
        if let Some(k) = map_key
            && let Some(i) = cx.ui.keys.iter().position(|(key, _)| *key == k)
        {
            cx.ui.keys.remove(i);
            cx.go(Go::Back);
            return;
        }
        if cx.ui.key(SPACE) {
            self.center = [player.x, player.z];
        }

        // Zoom round the pointer.
        let steps = cx.ui.scrolled([0.0, 0.0, w, h]);
        if steps != 0.0 {
            self.zoom_at(mouse, size, steps);
        }
        // Dragging.
        if let Some((start, last)) = self.drag {
            if cx.ui.down {
                if (mouse[0] - start[0]).abs() + (mouse[1] - start[1]).abs() > 3.0 {
                    self.dragged = true;
                }
                self.center[0] -= (mouse[0] - last[0]) as f64 / self.scale as f64;
                self.center[1] -= (mouse[1] - last[1]) as f64 / self.scale as f64;
                self.drag = Some((start, mouse));
            } else {
                self.drag = None;
                if !self.dragged
                    && let Some(wp) = self.waypoint_at(&all, mouse, size)
                {
                    cx.open(WaypointEdit::new(wp.clone(), false));
                    return;
                }
            }
        }

        self.land(cx);
        let hovered = self.waypoint_at(&all, mouse, size).cloned();
        self.waypoints(cx, &all, hovered.as_ref());
        self.player(cx, &player);

        // The bar along the top: the title and the dimension, the buttons on the right.
        let th = cx.p.theme;
        let pr = th.panel_radius;
        panels::dark_panel(cx.p, -pr, -pr, w + 2.0 * pr, BAR_HEIGHT + pr, 1.0);
        cx.p.shape(Shape::Diamond, 16.0, BAR_HEIGHT / 2.0, 9.0, th.accent);
        let text_y = (BAR_HEIGHT / 2.0).floor() - 4.0;
        cx.p.text(TITLE, 26.0, text_y, th.text, false);
        let tw = cx.p.width(TITLE);
        cx.p.text(&waypoint::dimension_name(&self.dimension), 30.0 + tw, text_y, th.muted, false);
        let done = cx.host.translate("gui.done");
        let done_r = [w - 8.0 - 60.0, 6.0, 60.0, CONTROL_HEIGHT];
        if ui::button(cx.p, cx.ui, done_r, &done, true) {
            cx.go(Go::Back);
        }
        let list_r = Self::bar_button(cx, done_r[0] - GAP, "Waypoints");
        if ui::menu_button(cx.p, cx.ui, list_r, "Waypoints", Some(Icon::Grid), "", true) {
            cx.open(WaypointList::new());
        }
        let center_r = Self::bar_button(cx, list_r[0] - GAP, "Center");
        if ui::menu_button(cx.p, cx.ui, center_r, "Center", Some(Icon::Target), "", true) {
            self.center = [player.x, player.z];
        }

        // Along the bottom: where the pointer is, and how to use the map.
        let [mx, mz] = self.world(mouse, size).map(|v| v.floor() as i64);
        let where_ = format!("X {mx}   Z {mz}   ·   Zoom {}%", (self.scale * 100.0).round() as i32);
        let hint = match &hovered {
            Some(wp) => format!("Click to edit {}", wp.name()),
            None => "Drag to move  ·  Scroll to zoom  ·  Right-click to add a waypoint".to_owned(),
        };
        let bar_y = h - 18.0;
        let where_w = cx.p.width(&where_) + 12.0;
        cx.p.round(6.0, bar_y, where_w, 14.0, th.radius, th.floating);
        cx.p.text(&where_, 12.0, bar_y + 3.0, th.text, false);
        // On a narrow screen the hint is cut short of the position, not drawn over it.
        let hint = cx.p.fit(&hint, w - 6.0 - (6.0 + where_w + 6.0) - 12.0).into_owned();
        let hint_w = cx.p.width(&hint) + 12.0;
        cx.p.round(w - 6.0 - hint_w, bar_y, hint_w, 14.0, th.radius, th.floating);
        cx.p.text(&hint, w - hint_w, bar_y + 3.0, th.muted, false);

        // Presses on the map, below the bar (the buttons took theirs).
        let map = [0.0, BAR_HEIGHT, w, h - BAR_HEIGHT];
        if cx.ui.clicked(map) {
            self.drag = Some((mouse, mouse));
            self.dragged = false;
        }
        if cx.ui.right_clicked(map) {
            match self.waypoint_at(&all, mouse, size) {
                Some(wp) => cx.open(WaypointEdit::new(wp.clone(), false)),
                None => self.add_at(cx, &all, &player, mouse),
            }
        }
    }

    /// The map is the backdrop: opaque, no blur over the world behind.
    fn backdrop(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zooms_round_the_pointer() {
        let mut map = WorldMap::new();
        map.center = [100.0, -50.0];
        let size = [400.0, 300.0];
        let at = [300.0, 75.0];
        let before = map.world(at, size);
        map.zoom_at(at, size, 2.0);
        assert!((map.scale - 1.5625).abs() < 1e-5);
        let after = map.world(at, size);
        assert!((before[0] - after[0]).abs() < 1e-3 && (before[1] - after[1]).abs() < 1e-3);
        assert_eq!(zoomed(7.0, 3.0), MAX_SCALE);
        assert_eq!(zoomed(0.2, -9.0), MIN_SCALE);
    }

    #[test]
    fn takes_more_blocks_a_cell_zoomed_out() {
        assert_eq!(cell_for(8.0), 1);
        assert_eq!(cell_for(1.0), 2);
        assert_eq!(cell_for(0.25), 8);
        assert_eq!(cell_for(MIN_SCALE), 16);
    }

    #[test]
    fn makes_runs_of_one_colour() {
        let mut memory = crate::map_memory::MapMemory::default();
        struct None_;
        impl crate::map_memory::Bytes for None_ {
            fn read(&self, _: &str) -> Option<Vec<u8>> {
                None
            }
            fn write(&mut self, _: &str, _: &[u8]) {}
        }
        memory.open("w", &mut None_);
        let mut colors = [0xFF11_1111u32; 256];
        colors[16..32].copy_from_slice(&[0xFF22_2222; 16]);
        colors[5] = 0;
        memory.record(&mut None_, "d", 0, 0, &colors);
        let region = memory.kept("d", 0, 0).unwrap();
        let r = runs(region, 1);
        // Row 0: 0..5 one colour, a clear block, 6..16 the same colour again.
        assert_eq!(&r[..2], &[(0, 0, 5, 0xFF11_1111), (0, 6, 10, 0xFF11_1111)]);
        assert_eq!(r[2], (1, 0, 16, 0xFF22_2222));
        assert_eq!(runs(region, 16), [(0, 0, 1, 0xFF11_1111)]);
    }
}
