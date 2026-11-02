//! What's new in a newer Incari Client (ReleaseNotesScreen.java), opened from the title screen's
//! update notice: its changelog as the online service gave it, one change a line, and the way to
//! the repository to get it. FormScreen's shape: one glass panel in the middle, 236 wide inside.
use super::confirm_link::{Answer, ConfirmLink};
use crate::host_extras::HostExtras;
use crate::panels;
use crate::screen::{Cx, Go, Screen};
use crate::theme::{CONTROL_HEIGHT, GAP as THEME_GAP};
use crate::ui;

const CONTENT_WIDTH: f32 = 236.0;
const PADDING: f32 = 14.0;
const LINE: f32 = 10.0;
const GAP: f32 = 6.0;

pub struct ReleaseNotes {
    title: String,
    notes: Vec<String>,
    /// The built-in repository (never the service's: ADR 0060).
    repository: String,
    extras: Box<dyn HostExtras>,
    confirm: Option<ConfirmLink>,
}

impl ReleaseNotes {
    pub fn new(
        version: &str,
        notes: Vec<String>,
        repository: &str,
        extras: Box<dyn HostExtras>,
    ) -> Self {
        Self {
            title: format!("Incari Client {version} is out"),
            notes,
            repository: repository.to_owned(),
            extras,
            confirm: None,
        }
    }
}

impl Screen for ReleaseNotes {
    fn draw(&mut self, cx: &mut Cx) {
        if let Some(confirm) = &self.confirm {
            match confirm.draw(cx) {
                Some(Answer::Open) => {
                    self.extras.open_link(&confirm.url);
                    self.confirm = None;
                }
                Some(_) => self.confirm = None,
                None => {}
            }
            return;
        }
        let [w, h] = cx.size;
        let th = cx.p.theme;
        // Lay out: "What's new:", each change wrapped, a gap, the buttons.
        // Each line with its height down the stack.
        let mut lines: Vec<(String, u32, f32)> = Vec::new();
        let mut cursor = 0.0;
        let mut text = |cx: &mut Cx, t: &str, color: u32, cursor: &mut f32| {
            for line in cx.p.wrap(t, CONTENT_WIDTH, 8) {
                lines.push((line, color, *cursor));
                *cursor += LINE;
            }
            *cursor += GAP - 2.0;
        };
        text(cx, "What's new:", th.muted, &mut cursor);
        for note in &self.notes {
            text(cx, &format!("\u{2022} {note}"), th.text, &mut cursor);
        }
        cursor += 4.0;
        let buttons_y = cursor;
        cursor += CONTROL_HEIGHT + GAP;
        let panel_height = cursor + 2.0 * PADDING + LINE + GAP;
        let top = ((h - panel_height) / 2.0).floor().max(4.0);
        let left = (w / 2.0 - CONTENT_WIDTH / 2.0).floor();
        panels::panel(
            cx.p,
            left - PADDING,
            top,
            CONTENT_WIDTH + 2.0 * PADDING,
            panel_height,
            1.0,
        );
        cx.p.centered(&self.title, w / 2.0, top + PADDING, th.text, true);
        let base = top + PADDING + LINE + GAP;
        for (line, color, y) in &lines {
            cx.p.text(line, left, base + y, *color, false);
        }
        let by = base + buttons_y;
        let each = ((CONTENT_WIDTH - THEME_GAP) / 2.0).floor();
        if ui::button(
            cx.p,
            cx.ui,
            [left, by, each, CONTROL_HEIGHT],
            "Get It on GitHub",
            true,
        ) {
            self.confirm = Some(ConfirmLink::new(self.repository.clone()));
        }
        let back = cx.host.translate("gui.back");
        let bx = left + each + THEME_GAP;
        if ui::button(
            cx.p,
            cx.ui,
            [bx, by, CONTENT_WIDTH - each - THEME_GAP, CONTROL_HEIGHT],
            &back,
            true,
        ) {
            cx.go(Go::Back);
        }
    }

    fn escape(&mut self, cx: &mut Cx) {
        if self.confirm.take().is_none() {
            cx.go(Go::Back);
        }
    }
}
