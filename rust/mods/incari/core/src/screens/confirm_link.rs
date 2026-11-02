//! ConfirmLinkScreen (trusted), drawn over the screen that asks: "Do you want to open this link
//! or copy it to your clipboard?", the link, and Open in Browser, Copy to Clipboard and Cancel.
//! Incari asks itself, so its screens stay under the question; the game side only opens the link.
use crate::screen::Cx;
use crate::theme::CONTROL_HEIGHT;
use crate::ui;

/// What the player answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Open,
    Copy,
    Cancel,
}

pub struct ConfirmLink {
    pub url: String,
}

impl ConfirmLink {
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }

    /// Draws the question over the whole screen; the answer once given. Copy leaves the link on
    /// the clipboard already. Escape is the asking screen's to answer (`Screen::escape`: Cancel).
    pub fn draw(&self, cx: &mut Cx) -> Option<Answer> {
        let [w, h] = cx.size;
        let th = cx.p.theme;
        let title = cx.host.translate("chat.link.confirmTrusted");
        for (n, line) in cx.p.wrap(&title, w - 50.0, 3).iter().enumerate() {
            cx.p.centered(line, w / 2.0, 70.0 + n as f32 * 9.0, th.text, true);
        }
        let lines = cx.p.wrap(&self.url, w - 50.0, 6);
        for (n, line) in lines.iter().enumerate() {
            cx.p.centered(line, w / 2.0, 90.0 + n as f32 * 9.0, th.text, true);
        }
        let low = h / 6.0 + 96.0;
        let y = (90.0 + lines.len() as f32 * 9.0 + 12.0).clamp(low, (h - 24.0).max(low));
        let labels = ["chat.link.open", "gui.copy_link_to_clipboard", "gui.cancel"]
            .map(|k| cx.host.translate(k));
        let xs = [w / 2.0 - 50.0 - 105.0, w / 2.0 - 50.0, w / 2.0 + 55.0];
        let answers = [Answer::Open, Answer::Copy, Answer::Cancel];
        let mut answer = None;
        for i in 0..3 {
            if ui::button(
                cx.p,
                cx.ui,
                [xs[i].floor(), y, 100.0, CONTROL_HEIGHT],
                &labels[i],
                true,
            ) {
                answer = Some(answers[i]);
            }
        }
        if answer == Some(Answer::Copy) {
            cx.ui.copy = Some(self.url.clone());
        }
        answer
    }
}
