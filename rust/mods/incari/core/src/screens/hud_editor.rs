//! The HUD editor (HudEditorScreen.java): every enabled module's HUD elements as they draw in play,
//! over a light tint. Drag an element to move it; it snaps to the screen's sides and middle and to
//! the other elements (Shift, or the Snap switch, places it freely), and takes the anchor of the
//! third of the screen it's left in. Scroll over it or drag its corner to resize it; right-click it
//! for its module's settings; arrows nudge the chosen one; Ctrl+Z undoes.
use crate::color::alpha;
use crate::framework::hud::{ANCHORS, Anchor};
use crate::framework::setting::Val;
use crate::icons::Icon;
use crate::input::{self, keys};
use crate::panels;
use crate::screen::{Cx, Go, Screen};
use crate::theme::{CONTROL_HEIGHT, GAP as CONTROL_GAP};
use crate::ui;

/// The margin the screen's sides snap at.
const EDGE: f32 = 2.0;
/// How close an edge must come to a line to snap to it.
const SNAP: f32 = 4.0;
/// The space kept between two elements snapped side by side.
const GAP: f32 = 2.0;
/// How far an element's frame stands out around it.
const FRAME: f32 = 2.0;
const HANDLE: f32 = 6.0;
const BAR_HEIGHT: f32 = 28.0;
const BAR_PADDING: f32 = 9.0;
const BUTTON_WIDTH: f32 = 54.0;
const TAG_HEIGHT: f32 = 11.0;
const ANCHOR_BAR: f32 = 14.0;
const EMPTY_WIDTH: f32 = 224.0;
const EMPTY_HEIGHT: f32 = 96.0;
const UNDO_LIMIT: usize = 64;
const MERGE_MILLIS: f64 = 700.0;
const HINT: &str =
    "Drag to move  ·  Scroll to resize  ·  Shift places freely  ·  Right-click for settings";
const SHORT_HINT: &str = "Drag to move  ·  Scroll to resize";
/// The size an element is taken to have before it has drawn.
const UNKNOWN: [f32; 2] = [40.0, 12.0];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Drag {
    None,
    Move,
    Scale,
}

/// An element's placement, to undo to: (module, element), anchor, x, y, scale.
type Placement = ((usize, usize), Val, Val, Val, Val);

pub struct HudEditor {
    selected: Option<usize>,
    drag: Drag,
    grab: [f32; 2],
    start: [f32; 4],
    guide: [Option<f32>; 2],
    snapping: bool,
    undo: Vec<Vec<Placement>>,
    merge: Option<(Drag, usize, f64)>,
}

impl Default for HudEditor {
    fn default() -> Self {
        Self {
            selected: None,
            drag: Drag::None,
            grab: [0.0; 2],
            start: [0.0; 4],
            guide: [None; 2],
            snapping: true,
            undo: Vec::new(),
            merge: None,
        }
    }
}

/// An element on screen this frame.
struct Shown {
    at: (usize, usize),
    rect: [f32; 4],
    size: [f32; 2],
}

impl Screen for HudEditor {
    fn backdrop(&self) -> bool {
        false
    }

    fn see_through(&self) -> bool {
        true
    }

    fn draw(&mut self, cx: &mut Cx) {
        let [w, h] = cx.size;
        let th = cx.p.theme;
        cx.p.gradient(0.0, 0.0, w, h, th.editor_tint_top, th.editor_tint_bottom);
        let shown = self.measure(cx);
        if self.selected.is_some_and(|s| s >= shown.len()) {
            self.selected = None;
            self.drag = Drag::None;
        }
        let mouse = cx.ui.mouse;
        let hovered = if self.drag == Drag::None {
            shown
                .iter()
                .rposition(|e| crate::draw::contains(frame(e.rect), mouse))
        } else {
            self.selected
        };
        self.input(cx, &shown, hovered);
        let shown = self.measure(cx);

        // Guides while moving: the screen's middle faintly, the lines snapped to in the accent,
        // and where it was picked up from.
        if self.drag == Drag::Move {
            let s = cx.p.scale;
            cx.p.fill((w / 2.0).floor(), 0.0, 1.0 / s, h, 0x1FFF_FFFF);
            cx.p.fill(0.0, (h / 2.0).floor(), w, 1.0 / s, 0x1FFF_FFFF);
            if let Some(x) = self.guide[0] {
                cx.p.fill(x, 0.0, 1.0 / s, h, alpha(th.accent, 0.8));
            }
            if let Some(y) = self.guide[1] {
                cx.p.fill(0.0, y, w, 1.0 / s, alpha(th.accent, 0.8));
            }
            let st = frame(self.start);
            cx.p.outline(st[0], st[1], st[2], st[3], th.radius, 0x2EFF_FFFF);
        }
        for (i, e) in shown.iter().enumerate() {
            let [x, y, fw, fh] = frame(e.rect);
            let (fill, glow) = if Some(i) == self.selected {
                (
                    alpha(th.accent, 0.16),
                    if th.normal() { crate::theme::GLOW } else { 0.0 },
                )
            } else if Some(i) == hovered {
                (alpha(th.accent, 0.1), 0.0)
            } else {
                (0x1FFF_FFFF, 0.0)
            };
            cx.p.round_box(
                x,
                y,
                fw,
                fh,
                th.radius,
                fill,
                0,
                glow,
                0.0,
                alpha(th.accent, 0.3),
            );
            let edge = if Some(i) == self.selected {
                th.accent
            } else if Some(i) == hovered {
                alpha(th.accent, 0.6)
            } else {
                0x59FF_FFFF
            };
            cx.p.outline(x, y, fw, fh, th.radius, edge);
            if Some(i) == self.selected {
                let hot = self.drag == Drag::Scale
                    || (self.drag == Drag::None && over_handle(e.rect, mouse));
                let [hx, hy] = handle(e.rect);
                let (f, o) = if hot {
                    (th.accent, th.text)
                } else {
                    (th.text, th.accent)
                };
                cx.p.round_box(hx, hy, HANDLE, HANDLE, HANDLE / 2.0, f, o, 0.0, 0.0, 0);
            }
        }
        if let Some(i) = self.selected.filter(|&i| i < shown.len()) {
            self.anchor_mark(cx, &shown[i]);
            self.tag(cx, &shown[i], true);
        }
        if let Some(i) = hovered.filter(|&i| Some(i) != self.selected && i < shown.len()) {
            self.tag(cx, &shown[i], false);
        }

        // The toolbar: the title, the snap switch, Reset and Done on one glass bar at the top. It
        // fades away while an element is dragged across it.
        let title = "HUD Layout";
        let snap_w = 12.0 + cx.p.width("Snap") + 6.0 + panels::SWITCH_WIDTH;
        let bar_w = BAR_PADDING
            + 12.0
            + 5.0
            + cx.p.width(title)
            + 14.0
            + snap_w
            + 10.0
            + 2.0 * BUTTON_WIDTH
            + CONTROL_GAP
            + 4.0;
        let bar = [((w - bar_w) / 2.0).floor(), 6.0, bar_w, BAR_HEIGHT];
        let over_bar = self.drag != Drag::None
            && self
                .selected
                .and_then(|i| shown.get(i))
                .is_some_and(|e| intersects(e.rect, bar));
        let fade = if over_bar { 0.15 } else { 1.0 };
        panels::panel(cx.p, bar[0], bar[1], bar[2], bar[3], fade);
        let ty = bar[1] + ((BAR_HEIGHT - 7.0) / 2.0).floor();
        cx.p.icon(
            Icon::Layout,
            bar[0] + BAR_PADDING,
            bar[1] + ((BAR_HEIGHT - 12.0) / 2.0).floor(),
            12.0,
            alpha(th.accent, fade),
        );
        cx.p.text(
            title,
            bar[0] + BAR_PADDING + 17.0,
            ty,
            alpha(th.text, fade),
            false,
        );
        let cy = bar[1] + ((BAR_HEIGHT - CONTROL_HEIGHT) / 2.0).floor();
        let mut x = bar[0] + bar_w - 4.0 - BUTTON_WIDTH;
        cx.p.fade = fade;
        let done = cx.host.translate("gui.done");
        if ui::button(
            cx.p,
            cx.ui,
            [x, cy, BUTTON_WIDTH, CONTROL_HEIGHT],
            &done,
            true,
        ) {
            cx.go(Go::Back);
        }
        x -= CONTROL_GAP + BUTTON_WIDTH;
        let reset = if self.selected.is_some() {
            "Reset"
        } else {
            "Reset all"
        };
        if ui::button(
            cx.p,
            cx.ui,
            [x, cy, BUTTON_WIDTH, CONTROL_HEIGHT],
            reset,
            true,
        ) {
            self.reset(cx, &shown);
        }
        x -= 10.0 + snap_w;
        let snap_r = [x, cy, snap_w, CONTROL_HEIGHT];
        let hot = cx.ui.over(snap_r);
        if hot || self.snapping {
            cx.p.round(
                snap_r[0],
                snap_r[1],
                snap_r[2],
                snap_r[3],
                th.radius,
                if hot { th.button } else { 0 },
            );
        }
        cx.p.text("Snap", x + 6.0, ty, th.text, false);
        panels::toggle(
            cx.p,
            x + snap_w - 6.0 - panels::SWITCH_WIDTH,
            cy + 5.0,
            self.snapping,
            fade,
        );
        if hot {
            cx.ui.tooltip(
                "Snap to the screen's sides and middle and to other elements. Hold Shift while \
                 dragging to place freely.",
            );
        }
        if cx.ui.clicked(snap_r) {
            self.snapping = !self.snapping;
        }
        cx.p.fade = 1.0;
        if shown.is_empty() {
            self.empty(cx);
        }
        self.caption(cx, &shown, bar, fade);
    }
}

impl HudEditor {
    /// Where each shown element is.
    fn measure(&self, cx: &Cx) -> Vec<Shown> {
        let screen = cx.size;
        let mut out = Vec::new();
        for (mi, m) in cx.modules.modules.iter().enumerate() {
            if !m.enabled {
                continue;
            }
            for (ei, e) in m.hud.iter().enumerate() {
                let size = cx
                    .modules
                    .sizes
                    .get(&(m.id, e.id))
                    .copied()
                    .unwrap_or(UNKNOWN);
                let size = [size[0].max(1.0), size[1].max(1.0)];
                out.push(Shown {
                    at: (mi, ei),
                    rect: e.bounds(size, screen),
                    size,
                });
            }
        }
        out
    }

    fn input(&mut self, cx: &mut Cx, shown: &[Shown], hovered: Option<usize>) {
        let mouse = cx.ui.mouse;
        let [w, h] = cx.size;
        // Ctrl+Z undoes; arrows nudge; Tab picks the next.
        if let Some(mods) = cx.ui.key_with(keys::Z)
            && mods & (input::CONTROL | input::SUPER) != 0
        {
            self.undo(cx);
        }
        if let Some(i) = self
            .selected
            .filter(|&i| i < shown.len() && self.drag == Drag::None)
        {
            for (code, dx, dy) in [
                (keys::LEFT, -1.0, 0.0),
                (keys::RIGHT, 1.0, 0.0),
                (keys::UP, 0.0, -1.0),
                (keys::DOWN, 0.0, 1.0),
            ] {
                if let Some(mods) = cx.ui.key_with(code) {
                    let step = if mods & input::SHIFT != 0 { 10.0 } else { 1.0 };
                    self.remember(cx, shown[i].at, Some(Drag::Move));
                    let r = shown[i].rect;
                    self.move_to(cx, shown, i, r[0] + dx * step, r[1] + dy * step, true);
                }
            }
        }
        if let Some(mods) = cx.ui.key_with(keys::TAB)
            && !shown.is_empty()
        {
            let n = shown.len();
            let back = mods & input::SHIFT != 0;
            self.selected = Some(match self.selected {
                None if back => n - 1,
                None => 0,
                Some(i) if back => (i + n - 1) % n,
                Some(i) => (i + 1) % n,
            });
        }
        // Right-click: the element's module's settings.
        if let Some(i) = hovered
            && cx.ui.right_clicked(frame(shown[i].rect))
        {
            let id = cx.modules.modules[shown[i].at.0].id;
            cx.open(super::module_settings::ModuleSettings::new(id));
            return;
        }
        // Scroll over an element: its size, about its middle.
        if self.drag == Drag::None
            && let Some(i) = hovered
        {
            let steps = cx.ui.scrolled(frame(shown[i].rect));
            if steps != 0.0 {
                self.selected = Some(i);
                self.remember(cx, shown[i].at, Some(Drag::Scale));
                let e = &mut cx.modules.modules[shown[i].at.0].hud[shown[i].at.1];
                let scale = e.scale() + steps.signum() * 0.25;
                if let Some(s) = e.settings.get_mut("scale") {
                    s.set(Val::Float(scale));
                }
                let r = shown[i].rect;
                let (cxm, cym) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
                let scale = e.scale();
                let (nw, nh) = (
                    (shown[i].size[0] * scale).round(),
                    (shown[i].size[1] * scale).round(),
                );
                self.resized(
                    cx,
                    shown,
                    i,
                    (cxm - nw / 2.0).floor(),
                    (cym - nh / 2.0).floor(),
                );
            }
        }
        // Press: the handle resizes, an element moves, nothing chooses nothing.
        if self.drag == Drag::None
            && let Some(at) = cx.ui.pressed_anywhere()
            && at[1] > 6.0 + BAR_HEIGHT + 22.0
        {
            if let Some(i) = self
                .selected
                .filter(|&i| i < shown.len() && over_handle(shown[i].rect, at))
            {
                cx.ui.eat_press();
                self.pick_up(cx, Drag::Scale, shown, i, at);
            } else if let Some(i) = shown
                .iter()
                .rposition(|e| crate::draw::contains(frame(e.rect), at))
            {
                cx.ui.eat_press();
                self.pick_up(cx, Drag::Move, shown, i, at);
            } else if !shown.is_empty() {
                self.selected = None;
            }
        }
        // Dragging.
        if self.drag != Drag::None
            && let Some(i) = self.selected.filter(|&i| i < shown.len())
        {
            if !cx.ui.down {
                self.drag = Drag::None;
                self.guide = [None; 2];
                return;
            }
            if self.drag == Drag::Move {
                let free = !self.snapping || cx.ui.shift;
                let (x, y) = (
                    (mouse[0] - self.grab[0]).round(),
                    (mouse[1] - self.grab[1]).round(),
                );
                self.move_to(cx, shown, i, x, y, free);
            } else {
                // The corner follows the pointer; the top left stays put.
                let size = shown[i].size;
                let across = (mouse[0] - self.start[0]) / size[0];
                let down = (mouse[1] - self.start[1]) / size[1];
                let e = &mut cx.modules.modules[shown[i].at.0].hud[shown[i].at.1];
                if let Some(s) = e.settings.get_mut("scale") {
                    s.set(Val::Float(across.max(down)));
                }
                let (x, y) = (self.start[0], self.start[1]);
                self.resized(cx, shown, i, x, y);
            }
        }
        let _ = (w, h);
    }

    fn pick_up(&mut self, cx: &mut Cx, kind: Drag, shown: &[Shown], i: usize, at: [f32; 2]) {
        self.drag = kind;
        self.selected = Some(i);
        self.start = shown[i].rect;
        self.grab = [at[0] - self.start[0], at[1] - self.start[1]];
        cx.ui.drag = Some(ui::id("hud-editor", i));
        self.remember(cx, shown[i].at, None);
    }

    /// Puts element `i` with its top left at (`x`, `y`), snapped unless `free`.
    fn move_to(&mut self, cx: &mut Cx, shown: &[Shown], i: usize, x: f32, y: f32, free: bool) {
        let [sw, sh] = cx.size;
        let [_, _, w, h] = shown[i].rect;
        let mut x = x.min(sw - w).max(0.0);
        let mut y = y.min(sh - h).max(0.0);
        self.guide = [None; 2];
        if !free {
            let (bx, gx) = snap(shown, i, x, w, sw, 0);
            let (by, gy) = snap(shown, i, y, h, sh, 1);
            (x, y) = (bx, by);
            self.guide = [gx, gy];
        }
        let (mi, ei) = shown[i].at;
        cx.modules.modules[mi].hud[ei].place(x as i32, y as i32, w as i32, h as i32, cx.size);
        cx.modules.changed();
    }

    /// After a new scale: element `i` placed with its top left near (`x`, `y`), kept on screen.
    fn resized(&mut self, cx: &mut Cx, shown: &[Shown], i: usize, x: f32, y: f32) {
        let (mi, ei) = shown[i].at;
        let e = &mut cx.modules.modules[mi].hud[ei];
        let s = e.scale();
        let (w, h) = (
            (shown[i].size[0] * s).round(),
            (shown[i].size[1] * s).round(),
        );
        let x = x.min(cx.size[0] - w).max(0.0);
        let y = y.min(cx.size[1] - h).max(0.0);
        e.place(x as i32, y as i32, w as i32, h as i32, cx.size);
        cx.modules.changed();
    }

    /// Back to the defaults: the chosen element, or every shown one.
    fn reset(&mut self, cx: &mut Cx, shown: &[Shown]) {
        let mut step = Vec::new();
        for (i, e) in shown.iter().enumerate() {
            if self.selected.is_none_or(|s| s == i) {
                step.push(placement(cx, e.at));
                let (mi, ei) = e.at;
                cx.modules.modules[mi].hud[ei].reset_placement();
            }
        }
        self.push(step);
        self.merge = None;
        cx.modules.changed();
    }

    /// Saves element `at`'s placement to undo to, unless it continues the last step.
    fn remember(&mut self, cx: &Cx, at: (usize, usize), kind: Option<Drag>) {
        let now = cx.p.motion.now;
        let flat = at.0 * 1000 + at.1;
        let merges = kind.is_some()
            && self
                .merge
                .is_some_and(|(k, i, t)| Some(k) == kind && i == flat && now - t < MERGE_MILLIS);
        self.merge = kind.map(|k| (k, flat, now));
        if !merges {
            self.push(vec![placement(cx, at)]);
        }
    }

    fn push(&mut self, step: Vec<Placement>) {
        if step.is_empty() {
            return;
        }
        self.undo.push(step);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
    }

    fn undo(&mut self, cx: &mut Cx) {
        if let Some(step) = self.undo.pop() {
            for ((mi, ei), anchor, x, y, scale) in step {
                let e = &mut cx.modules.modules[mi].hud[ei];
                for (id, v) in [("anchor", anchor), ("x", x), ("y", y), ("scale", scale)] {
                    if let Some(s) = e.settings.get_mut(id) {
                        s.set(v);
                    }
                }
            }
            cx.modules.changed();
        }
        self.merge = None;
    }

    /// The chosen element's anchor: a short accent bar along that side (two at a corner), a dot
    /// in the middle.
    fn anchor_mark(&self, cx: &mut Cx, e: &Shown) {
        let [w, h] = cx.size;
        let anchor = cx.modules.modules[e.at.0].hud[e.at.1].anchor();
        let color = alpha(cx.p.theme.accent, 0.9);
        let (column, row) = (anchor_index(anchor) % 3, anchor_index(anchor) / 3);
        if column == 1 && row == 1 {
            cx.p.round(
                (w / 2.0).floor() - 2.0,
                (h / 2.0).floor() - 2.0,
                4.0,
                4.0,
                2.0,
                color,
            );
            return;
        }
        let bx = [0.0, ((w - ANCHOR_BAR) / 2.0).floor(), w - ANCHOR_BAR][column];
        let by = [0.0, ((h - ANCHOR_BAR) / 2.0).floor(), h - ANCHOR_BAR][row];
        if row != 1 {
            cx.p.fill(
                bx,
                if row == 0 { 0.0 } else { h - 1.0 },
                ANCHOR_BAR,
                1.0,
                color,
            );
        }
        if column != 1 {
            cx.p.fill(
                if column == 0 { 0.0 } else { w - 1.0 },
                by,
                1.0,
                ANCHOR_BAR,
                color,
            );
        }
    }

    /// The element's name on a pill above it (below, at the top of the screen).
    fn tag(&self, cx: &mut Cx, e: &Shown, chosen: bool) {
        let th = cx.p.theme;
        let name = tag_text(cx, e.at);
        let tw = cx.p.width(&name) + 10.0;
        let [ex, ey, ew, eh] = e.rect;
        let x = (ex + ((ew - tw) / 2.0).floor())
            .min(cx.size[0] - tw - 2.0)
            .max(2.0);
        let mut y = ey - FRAME - 3.0 - TAG_HEIGHT;
        if y < 2.0 {
            y = ey + eh + FRAME + 3.0;
        }
        cx.p.round(
            x,
            y,
            tw,
            TAG_HEIGHT,
            TAG_HEIGHT / 2.0,
            if chosen { th.accent } else { th.floating },
        );
        cx.p.text(
            &name,
            x + 5.0,
            y + 2.0,
            if chosen { th.on_accent } else { th.text },
            false,
        );
    }

    /// A line under the toolbar: how to use the editor, or where the element in hand is going.
    fn caption(&self, cx: &mut Cx, shown: &[Shown], bar: [f32; 4], fade: f32) {
        let th = cx.p.theme;
        let chosen = self.selected.and_then(|i| shown.get(i));
        let text = match (self.drag, chosen) {
            (Drag::Move, Some(e)) => {
                let el = &cx.modules.modules[e.at.0].hud[e.at.1];
                let label = ANCHORS[anchor_index(el.anchor())].1;
                format!(
                    "{label}  ·  {}, {}",
                    el.settings.int("x"),
                    el.settings.int("y")
                )
            }
            (Drag::Scale, Some(e)) => {
                let el = &cx.modules.modules[e.at.0].hud[e.at.1];
                format!(
                    "Size {}",
                    crate::framework::hud::format_scale(el.scale() as f64)
                )
            }
            (_, Some(e)) => {
                let el = &cx.modules.modules[e.at.0].hud[e.at.1];
                let label = ANCHORS[anchor_index(el.anchor())].1;
                format!(
                    "{}  ·  {label}  ·  Arrows nudge  ·  Ctrl+Z undoes",
                    tag_text(cx, e.at)
                )
            }
            _ if cx.p.width(HINT) + 24.0 <= cx.size[0] => HINT.to_owned(),
            _ => SHORT_HINT.to_owned(),
        };
        let w = cx.p.width(&text) + 16.0;
        let x = ((cx.size[0] - w) / 2.0).floor();
        let y = bar[1] + BAR_HEIGHT + 5.0;
        cx.p.round(x, y, w, 14.0, 7.0, alpha(th.background, fade));
        cx.p.text(&text, x + 8.0, y + 3.0, alpha(th.muted, fade), false);
    }

    /// With nothing to place: a card that says so, and a way back to turn something on.
    fn empty(&self, cx: &mut Cx) {
        let th = cx.p.theme;
        let x = ((cx.size[0] - EMPTY_WIDTH) / 2.0).floor();
        let y = ((cx.size[1] - EMPTY_HEIGHT) / 2.0).floor();
        panels::panel(cx.p, x, y, EMPTY_WIDTH, EMPTY_HEIGHT, 1.0);
        cx.p.icon(Icon::Layout, x + 12.0, y + 11.0, 12.0, th.accent);
        cx.p.text("Nothing on the HUD yet", x + 30.0, y + 14.0, th.text, false);
        let lines = cx.p.wrap(
            "Turn on a module that shows something on the HUD, then place it here.",
            EMPTY_WIDTH - 24.0,
            3,
        );
        for (i, line) in lines.iter().enumerate() {
            cx.p.text(line, x + 12.0, y + 31.0 + i as f32 * 10.0, th.muted, false);
        }
        let r = [
            x + (EMPTY_WIDTH - 96.0) / 2.0,
            y + EMPTY_HEIGHT - 10.0 - CONTROL_HEIGHT,
            96.0,
            CONTROL_HEIGHT,
        ];
        if ui::button(cx.p, cx.ui, r, "Modules", true) {
            cx.go(Go::Replace(Box::new(super::modules::ModuleList::default())));
        }
    }
}

fn placement(cx: &Cx, at: (usize, usize)) -> Placement {
    let e = &cx.modules.modules[at.0].hud[at.1];
    let v = |id: &str| e.settings.get(id).map_or(Val::None, |s| s.value.clone());
    (at, v("anchor"), v("x"), v("y"), v("scale"))
}

fn tag_text(cx: &Cx, at: (usize, usize)) -> String {
    let m = &cx.modules.modules[at.0];
    let e = &m.hud[at.1];
    let name = if m.hud.len() > 1 {
        format!("{} · {}", m.name, e.name)
    } else {
        m.name.to_owned()
    };
    let scale = e.scale();
    if scale == 1.0 {
        name
    } else {
        format!(
            "{name}  {}",
            crate::framework::hud::format_scale(scale as f64)
        )
    }
}

fn anchor_index(a: Anchor) -> usize {
    a.index()
}

fn frame(r: [f32; 4]) -> [f32; 4] {
    [
        r[0] - FRAME,
        r[1] - FRAME,
        r[2] + 2.0 * FRAME,
        r[3] + 2.0 * FRAME,
    ]
}

fn handle(r: [f32; 4]) -> [f32; 2] {
    [
        r[0] + r[2] + FRAME - HANDLE / 2.0,
        r[1] + r[3] + FRAME - HANDLE / 2.0,
    ]
}

fn over_handle(r: [f32; 4], at: [f32; 2]) -> bool {
    let [hx, hy] = handle(r);
    crate::draw::contains([hx - 2.0, hy - 2.0, HANDLE + 4.0, HANDLE + 4.0], at)
}

fn intersects(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}

/// Element `i`'s start along one axis (0 across, 1 down), snapped to the nearest line within
/// `SNAP`: the screen's sides and middle, the other elements' edges and middles, and `GAP` beside
/// them. Returns the start and the line it snapped to.
fn snap(
    shown: &[Shown],
    i: usize,
    start: f32,
    size: f32,
    screen: f32,
    axis: usize,
) -> (f32, Option<f32>) {
    let mut best = (start, SNAP + 1.0, None);
    let mut consider = |candidate: f32, guide: f32| {
        let d = (candidate - start).abs();
        if d < best.1 {
            best = (candidate, d, Some(guide));
        }
    };
    consider(EDGE, EDGE);
    consider(screen - EDGE - size, screen - EDGE);
    consider(((screen - size) / 2.0).floor(), (screen / 2.0).floor());
    for (j, e) in shown.iter().enumerate() {
        if j == i {
            continue;
        }
        let (other, other_size) = (e.rect[axis], e.rect[2 + axis]);
        consider(other, other);
        consider(other + other_size - size, other + other_size);
        consider(
            other + ((other_size - size) / 2.0).floor(),
            other + (other_size / 2.0).floor(),
        );
        consider(other + other_size + GAP, other + other_size + GAP / 2.0);
        consider(other - GAP - size, other - GAP / 2.0);
    }
    (best.0, best.2)
}
