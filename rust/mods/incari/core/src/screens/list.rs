//! The shape of Incari's menu screens (ListScreen, ControlList): a title and a caption, a glass
//! panel of scrolling rows, and a row of buttons at the bottom. A screen draws its rows through
//! `Rows` each frame; rows out of view aren't drawn and take no clicks.
use crate::draw::Rect;
use crate::panels;
use crate::screen::Cx;
use crate::theme::{CONTROL_HEIGHT, FOOTER_BUTTON_WIDTH, GAP, LINE_GAP, ROW_HEIGHT, ROW_WIDTH};
use crate::ui;

const HEADER_HEIGHT: f32 = 44.0;
const FOOTER_HEIGHT: f32 = 36.0;
/// Space either side of the rows.
pub const EDGE: f32 = 16.0;
const TOOLTIP_WIDTH: f32 = 200.0;

/// A list screen's scroll, kept between frames.
#[derive(Default)]
pub struct List {
    scroll: f32,
    content: f32,
}

/// The rows being drawn this frame.
pub struct Rows {
    /// The rows' left edge and width.
    pub x: f32,
    pub width: f32,
    /// The next row's top, scrolled.
    y: f32,
    top: f32,
    area: Rect,
    content: f32,
}

impl List {
    /// Draws the screen: `title` (with Incari's badge before it when `badge`), `caption`, the
    /// rows `rows` draws, and the footer's buttons (`labels`, each `width` wide or the footer's
    /// default). Returns the footer button pressed.
    pub fn draw(
        &mut self,
        cx: &mut Cx,
        title: &str,
        caption: &str,
        badge: bool,
        rows: impl FnOnce(&mut Cx, &mut Rows),
        labels: &[&str],
        width: Option<f32>,
    ) -> Option<usize> {
        let [w, h] = cx.size;
        let th = cx.p.theme;
        // The header: the title, and the caption under it.
        let lines = if caption.is_empty() { 1.0 } else { 2.0 };
        let block = lines * 9.0 + (lines - 1.0) * LINE_GAP;
        let ty = ((HEADER_HEIGHT - block) / 2.0).floor();
        let title_w = cx.p.width(title);
        let tx = (w / 2.0 - title_w / 2.0).floor();
        cx.p.text(title, tx, ty, th.text, true);
        if badge {
            cx.p.icon(
                crate::icons::Icon::Incari,
                tx - 14.0,
                ty - 2.0,
                11.0,
                th.accent,
            );
        }
        if !caption.is_empty() {
            let shown = cx.p.fit(caption, w - 40.0).into_owned();
            cx.p.centered(&shown, w / 2.0, ty + 9.0 + LINE_GAP, th.muted, true);
        }
        cx.p.hairline(0.0, HEADER_HEIGHT - 1.0, w, th.glass_edge_solid);
        cx.p.hairline(0.0, h - FOOTER_HEIGHT, w, th.glass_edge_solid);
        // The rows, on a glass panel.
        let row_w = ROW_WIDTH.min(w - 2.0 * EDGE).max(0.0);
        let area = [0.0, HEADER_HEIGHT, w, h - HEADER_HEIGHT - FOOTER_HEIGHT];
        let rx = (w / 2.0 - row_w / 2.0).floor();
        panels::panel(
            cx.p,
            rx - 12.0,
            HEADER_HEIGHT + 4.0,
            row_w + 24.0,
            area[3] - 8.0,
            1.0,
        );
        ui::scroll(
            cx.ui,
            area,
            &mut self.scroll,
            self.content + 8.0,
            ROW_HEIGHT,
        );
        let mut r = Rows {
            x: rx,
            width: row_w,
            y: HEADER_HEIGHT + 4.0 - self.scroll.floor(),
            top: HEADER_HEIGHT + 4.0 - self.scroll.floor(),
            area,
            content: 0.0,
        };
        cx.p.clip_start(area);
        rows(cx, &mut r);
        cx.p.clip_end();
        self.content = r.y - r.top;
        ui::scrollbar(
            cx.p,
            [rx + row_w + 6.0, area[1], 0.0, area[3]],
            self.content + 8.0,
            self.scroll,
        );
        // The footer's buttons.
        let width = width.unwrap_or(FOOTER_BUTTON_WIDTH);
        let y = h - FOOTER_HEIGHT + ((FOOTER_HEIGHT - CONTROL_HEIGHT) / 2.0).floor();
        let n = labels.len() as f32;
        let total = n * width + (n - 1.0) * GAP * 2.0;
        let mut x = (w / 2.0 - total / 2.0).floor();
        let mut pressed = None;
        for (i, label) in labels.iter().enumerate() {
            if ui::button(cx.p, cx.ui, [x, y, width, CONTROL_HEIGHT], label, true) {
                pressed = Some(i);
            }
            x += width + GAP * 2.0;
        }
        pressed
    }
}

impl Rows {
    /// The next `height` of the list, if any of it shows.
    fn take(&mut self, height: f32) -> Option<Rect> {
        let r = [self.x, self.y, self.width, height];
        self.y += height;
        self.content += height;
        let a = self.area;
        (r[1] + r[3] > a[1] && r[1] < a[1] + a[3]).then_some(r)
    }

    /// A heading in the accent, between groups of rows.
    pub fn heading(&mut self, cx: &mut Cx, text: &str) {
        if let Some(r) = self.take(ROW_HEIGHT) {
            let shown = cx.p.fit(text, r[2]).into_owned();
            let accent = cx.p.theme.accent;
            cx.p.centered(
                &shown,
                r[0] + r[2] / 2.0,
                r[1] + r[3] - 9.0 - 2.0,
                accent,
                false,
            );
        }
    }

    /// A labelled row with controls `widths` wide at its right: their rectangles, right to
    /// left order undone (left first), or None when it's out of view.
    pub fn row(
        &mut self,
        cx: &mut Cx,
        label: &str,
        tooltip: &str,
        widths: &[f32],
    ) -> Option<Vec<Rect>> {
        let r = self.take(ROW_HEIGHT)?;
        let y = r[1] + ((r[3] - CONTROL_HEIGHT) / 2.0).floor();
        let mut x = r[0] + r[2];
        let mut rects = vec![[0.0; 4]; widths.len()];
        for (i, w) in widths.iter().enumerate().rev() {
            x -= w;
            rects[i] = [x, y, *w, CONTROL_HEIGHT];
            x -= GAP;
        }
        // The label gets what the controls leave; cut, it shows whole in its tooltip.
        let room = x - r[0];
        let text = cx.p.text_color();
        let shown = cx.p.fit(label, room).into_owned();
        let label_y = r[1] + (r[3] / 2.0).floor() - 4.0;
        cx.p.text(&shown, r[0], label_y, text, true);
        let label_r = [r[0], r[1], cx.p.width(&shown), r[3]];
        let cut = shown != label;
        if (cut || !tooltip.is_empty()) && cx.ui.over(crate::draw::intersect(label_r, self.area)) {
            let tip = match (cut, tooltip.is_empty()) {
                (true, false) => format!("{label}\n{tooltip}"),
                (true, true) => label.to_owned(),
                _ => tooltip.to_owned(),
            };
            let _ = TOOLTIP_WIDTH;
            cx.ui.tooltip(tip);
        }
        Some(rects)
    }

    /// A row of `count` controls `height` tall across the list's width (tiles).
    pub fn across(&mut self, count: usize, height: f32) -> Option<Vec<Rect>> {
        let padding = 2.0;
        let r = self.take(height + GAP + 2.0 * padding)?;
        let each = ((r[2] - (count as f32 - 1.0) * GAP) / count as f32).floor();
        Some(
            (0..count)
                .map(|i| {
                    let x = r[0] + i as f32 * (each + GAP);
                    let w = if i == count - 1 {
                        r[0] + r[2] - x
                    } else {
                        each
                    };
                    [x, r[1] + padding, w, height]
                })
                .collect(),
        )
    }

    /// Whether `r` (a row's control) shows, so a click there counts.
    pub fn visible(&self, r: Rect) -> Rect {
        crate::draw::intersect(r, self.area)
    }
}
