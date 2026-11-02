//! Incari's credits and the licenses of what it adds to the game (CreditsScreen.java), reached
//! from the game's Credits & Attribution screen. Each entry links to its source and its license,
//! which is what GPL-3.0 asks a build to come with.
//!
//! HeaderAndFooterLayout's shape: the title in a 33-unit header, the notice and the entries
//! centred in a column 300 wide that scrolls, Done (200 wide) in a 33-unit footer.
use super::confirm_link::{Answer, ConfirmLink};
use crate::credits::Credit;
use crate::host_extras::HostExtras;
use crate::screen::{Cx, Go, Screen};
use crate::theme::CONTROL_HEIGHT;
use crate::ui;

pub const TITLE: &str = "Incari & Open Source";
const NOTICE: &str = "Incari Client is free software that runs on Minecraft: Java Edition. It is not an official \
     Minecraft product, and is not approved by or associated with Mojang or Microsoft. This build contains the \
     software below, each under its own license.";
const HEADER: f32 = 33.0;
const FOOTER: f32 = 33.0;
const BODY_WIDTH: f32 = 300.0;
const BUTTON_WIDTH: f32 = 103.0;
const BUTTON_SPACING: f32 = 4.0;
/// Between the notice and each entry (LinearLayout's spacing).
const SPACING: f32 = 12.0;
/// Between an entry's title, its line and its buttons.
const ENTRY_SPACING: f32 = 2.0;
const LINE: f32 = 9.0;
const MAX_LINES: usize = 12;

pub struct Credits {
    credits: Vec<Credit>,
    extras: Box<dyn HostExtras>,
    scroll: f32,
    confirm: Option<ConfirmLink>,
}

impl Credits {
    /// `credits`: `crate::credits::all(&build)`.
    pub fn new(credits: Vec<Credit>, extras: Box<dyn HostExtras>) -> Self {
        Self {
            credits,
            extras,
            scroll: 0.0,
            confirm: None,
        }
    }
}

impl Screen for Credits {
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
        cx.p.centered(
            TITLE,
            w / 2.0,
            ((HEADER - LINE) / 2.0).floor(),
            th.text,
            true,
        );
        cx.p.hairline(0.0, HEADER - 1.0, w, th.glass_edge_solid);
        cx.p.hairline(0.0, h - FOOTER, w, th.glass_edge_solid);

        // The column: the notice, then each entry's title, its authors and license, and its links.
        let body = BODY_WIDTH.min(w - 20.0);
        let notice = cx.p.wrap(NOTICE, body, MAX_LINES);
        let lines: Vec<Vec<String>> = self
            .credits
            .iter()
            .map(|c| {
                cx.p.wrap(
                    &format!("{} \u{b7} {}", c.authors, c.license),
                    body,
                    MAX_LINES,
                )
            })
            .collect();
        let entry_height =
            |n: usize| LINE + ENTRY_SPACING + n as f32 * LINE + ENTRY_SPACING + CONTROL_HEIGHT;
        let content = notice.len() as f32 * LINE
            + lines
                .iter()
                .map(|l| SPACING + entry_height(l.len()))
                .sum::<f32>();
        let area = [0.0, HEADER, w, (h - HEADER - FOOTER).max(0.0)];
        ui::scroll(cx.ui, area, &mut self.scroll, content, 18.0);
        // Centred when it fits, as ScrollableLayout in HeaderAndFooterLayout's middle.
        let mut y = (HEADER + ((area[3] - content) / 2.0).max(0.0) - self.scroll).floor();
        let mid = w / 2.0;
        // Done first, so a press on it isn't taken by a link scrolled under the footer.
        let done = cx.host.translate("gui.done");
        let fy = h - FOOTER + ((FOOTER - CONTROL_HEIGHT) / 2.0).floor();
        // The game's own Credits & Attribution beside Done: on Horizon Incari's credits come first
        // from the title (in Java the game's screen has a button to them).
        let game_credits = cx.host.translate("credits_and_attribution.screen.title");
        if ui::button(cx.p, cx.ui, [(mid - 154.0).floor(), fy, 150.0, CONTROL_HEIGHT], &game_credits, true) {
            cx.go(Go::Game("credits"));
        }
        if ui::button(cx.p, cx.ui, [(mid + 4.0).floor(), fy, 150.0, CONTROL_HEIGHT], &done, true) {
            cx.go(Go::Back);
        }
        let mut open = None;
        cx.p.clip_start(area);
        for line in &notice {
            cx.p.centered(line, mid, y, th.text, true);
            y += LINE;
        }
        for (i, credit) in self.credits.iter().enumerate() {
            y += SPACING;
            let bottom = y + entry_height(lines[i].len());
            if bottom < area[1] || y > area[1] + area[3] {
                y = bottom;
                continue;
            }
            let title = credit.title();
            cx.p.centered(&title, mid, y, th.text, true);
            y += LINE + ENTRY_SPACING;
            for line in &lines[i] {
                cx.p.centered(line, mid, y, th.muted, true);
                y += LINE;
            }
            y += ENTRY_SPACING;
            let x = (mid - BUTTON_WIDTH - BUTTON_SPACING / 2.0).floor();
            let source = [x, y, BUTTON_WIDTH, CONTROL_HEIGHT];
            let license = [
                x + BUTTON_WIDTH + BUTTON_SPACING,
                y,
                BUTTON_WIDTH,
                CONTROL_HEIGHT,
            ];
            // Only the part in view takes a click.
            let visible = |r| crate::draw::intersect(r, area);
            if ui::button(cx.p, cx.ui, source, "Source", true) && cx.ui.over(visible(source)) {
                open = Some(credit.source.clone());
            }
            if ui::button(cx.p, cx.ui, license, "License", true) && cx.ui.over(visible(license)) {
                open = Some(credit.license_text.clone());
            }
            y += CONTROL_HEIGHT;
        }
        cx.p.clip_end();
        ui::scrollbar(
            cx.p,
            [(mid + body / 2.0 + 6.0).min(w - 8.0), area[1], 0.0, area[3]],
            content,
            self.scroll,
        );

        if let Some(url) = open {
            self.confirm = Some(ConfirmLink::new(url));
        }
    }

    fn escape(&mut self, cx: &mut Cx) {
        if self.confirm.take().is_none() {
            cx.go(Go::Back);
        }
    }
}
