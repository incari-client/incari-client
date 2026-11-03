//! A grid setting's editor (GridScreen.java): click or drag to draw, right-click to erase, with
//! the result beside it at its real size.
use crate::framework::setting::Val;
use crate::panels;
use crate::screen::{Cx, Go, Screen};
use crate::theme::{CONTROL_HEIGHT, GAP, SMALL_BUTTON_WIDTH};
use crate::ui;

const LARGEST_CELL: f32 = 12.0;
const PAD: f32 = 8.0;

pub struct Grid {
    module: &'static str,
    setting: &'static str,
    /// What a drag paints: on or off.
    painting: Option<bool>,
}

impl Grid {
    pub fn new(module: &'static str, setting: &'static str) -> Self {
        Self {
            module,
            setting,
            painting: None,
        }
    }
}

impl Screen for Grid {
    fn draw(&mut self, cx: &mut Cx) {
        let Some(s) = cx
            .modules
            .module(self.module)
            .and_then(|m| m.settings.get(self.setting))
        else {
            cx.go(Go::Back);
            return;
        };
        let (name, mut rows) = match &s.value {
            Val::Grid(rows) => (s.name, rows.clone()),
            _ => return,
        };
        let size = rows.len();
        let [w, h] = cx.size;
        let th = cx.p.theme;
        let room = (w - 120.0).min(h - 90.0);
        let cell = (room / size as f32).floor().clamp(3.0, LARGEST_CELL);
        let side = cell * size as f32;
        let (gx, gy) = (((w - side) / 2.0).floor(), 36.0);
        cx.p.centered(name, w / 2.0, 12.0, th.text, true);
        cx.p.centered(
            "Click or drag to draw, right-click to erase",
            w / 2.0,
            22.0,
            th.muted,
            true,
        );
        panels::panel(
            cx.p,
            gx - PAD,
            gy - PAD,
            side + PAD * 2.0,
            side + PAD * 2.0,
            1.0,
        );
        let middle = size / 2;
        let on = |rows: &[u64], x: usize, y: usize| rows[y] >> x & 1 != 0;
        // Painting: a press starts it (drawing or erasing), a drag carries it on.
        let at = |m: [f32; 2]| {
            let (x, y) = (((m[0] - gx) / cell).floor(), ((m[1] - gy) / cell).floor());
            (x >= 0.0 && y >= 0.0 && (x as usize) < size && (y as usize) < size)
                .then_some((x as usize, y as usize))
        };
        let area = [gx, gy, side, side];
        let mut changed = false;
        if cx.ui.right_clicked(area)
            && let Some((x, y)) = at(cx.ui.mouse)
        {
            rows[y] &= !(1 << x);
            changed = true;
        }
        if cx.ui.clicked(area)
            && let Some((x, y)) = at(cx.ui.mouse)
        {
            self.painting = Some(!on(&rows, x, y));
            cx.ui.drag = Some(ui::id("grid", 0));
        }
        if !cx.ui.down {
            self.painting = None;
        }
        if let (Some(paint), Some((x, y))) = (self.painting, at(cx.ui.mouse)) {
            let was = on(&rows, x, y);
            if was != paint {
                rows[y] ^= 1 << x;
                changed = true;
            }
        }
        for y in 0..size {
            for x in 0..size {
                let color = if on(&rows, x, y) {
                    th.accent
                } else if x == middle || y == middle {
                    0x30FF_FFFF
                } else {
                    0x18FF_FFFF
                };
                cx.p.fill(
                    gx + x as f32 * cell,
                    gy + y as f32 * cell,
                    cell - 1.0,
                    cell - 1.0,
                    color,
                );
            }
        }
        if let Some((x, y)) = at(cx.ui.mouse) {
            cx.p.outline(
                gx + x as f32 * cell - 1.0,
                gy + y as f32 * cell - 1.0,
                cell + 1.0,
                cell + 1.0,
                0.0,
                th.text,
            );
        }
        // Beside the grid, at the size it's drawn: one cell a GUI unit.
        let (px, py) = (
            gx + side + PAD * 3.0,
            gy + (side / 2.0).floor() - (size / 2) as f32,
        );
        panels::dark_panel(
            cx.p,
            px - 6.0,
            py - 6.0,
            size as f32 + 12.0,
            size as f32 + 12.0,
            1.0,
        );
        for y in 0..size {
            for x in 0..size {
                if on(&rows, x, y) {
                    cx.p.fill(px + x as f32, py + y as f32, 1.0, 1.0, th.text);
                }
            }
        }
        let bw = SMALL_BUTTON_WIDTH + 10.0;
        let by = gy + side + PAD + 4.0;
        let left = (w / 2.0 - (bw * 3.0 + GAP * 2.0) / 2.0).floor();
        let mut reset = false;
        if ui::button(cx.p, cx.ui, [left, by, bw, CONTROL_HEIGHT], "Clear", true) {
            rows.iter_mut().for_each(|r| *r = 0);
            changed = true;
        }
        if ui::button(
            cx.p,
            cx.ui,
            [left + bw + GAP, by, bw, CONTROL_HEIGHT],
            "Reset",
            true,
        ) {
            reset = true;
        }
        let done = cx.host.translate("gui.done");
        if ui::button(
            cx.p,
            cx.ui,
            [left + (bw + GAP) * 2.0, by, bw, CONTROL_HEIGHT],
            &done,
            true,
        ) {
            cx.go(Go::Back);
        }
        if (changed || reset)
            && let Some(s) = cx
                .modules
                .module_mut(self.module)
                .and_then(|m| m.settings.get_mut(self.setting))
        {
            if reset {
                s.reset();
            } else {
                s.set(Val::Grid(rows));
            }
            cx.modules.changed();
        }
    }
}
