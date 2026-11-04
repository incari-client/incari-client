//! The pause menu in the client's look (PauseLook.java, PauseMenuHooks.java): the game's buttons
//! gathered on one glass card in the middle of the screen, the title at the card's top left, and
//! Incari Settings with Incari's badge, the way into everything Incari adds. Report Bugs and Give
//! Feedback go to Incari's repository, not Mojang's, asking first as the Java build does.
//!
//! Under Incari Settings, the server's own: an Eaglercraft server's buttons (ServerPauseMenu.java)
//! in place of the game's Server Links or pause screen dialog button, which shows otherwise; and
//! the Eaglercraft server's pictures beside the game's buttons and the title.
use super::confirm_link::{Answer, ConfirmLink};
use crate::credits;
use crate::draw::DrawList;
use crate::icons::Icon;
use crate::panels;
use crate::screen::{Cx, Go, Screen};
use crate::server_menu::{ICON_GAP, ICON_SIZE, ServerMenu};
use crate::theme::CONTROL_HEIGHT;
use crate::ui;

const PADDING: f32 = 12.0;
const HEADER: f32 = 9.0 + 9.0;
const WIDE: f32 = 204.0;
const HALF: f32 = 98.0;
const ROW: f32 = 24.0;

/// What an icon of the row does.
#[derive(Clone, Copy)]
enum Link {
    Bugs,
    Feedback,
    Friends,
}

/// The server's row(s) under Incari Settings.
enum Extra {
    None,
    /// An Eaglercraft server's buttons (text, link, picture key), side by side when both fit.
    Eagler(Vec<(String, Option<String>, &'static str)>, bool),
    /// The game's: its label and what `Host::open` does for it.
    Game(String, &'static str),
}

impl Extra {
    fn rows(&self) -> f32 {
        match self {
            Extra::None => 0.0,
            Extra::Eagler(_, true) => 1.0,
            Extra::Eagler(buttons, false) => buttons.len() as f32,
            Extra::Game(..) => 1.0,
        }
    }
}

#[derive(Default)]
pub struct Pause {
    /// Report Bugs' or Give Feedback's link, asked about over the menu.
    confirm: Option<ConfirmLink>,
}

/// One of the game's buttons, with the Eaglercraft server's pictures for `key` beside its label.
fn game_button(
    cx: &mut Cx,
    menu: Option<&ServerMenu>,
    r: [f32; 4],
    label: &str,
    key: Option<&str>,
) -> bool {
    let clicked = ui::button(cx.p, cx.ui, r, label, true);
    if let (Some(menu), Some(key)) = (menu, key) {
        let width = cx.p.width(&cx.p.fit(label, r[2] - 8.0));
        menu.draw_beside(cx.p, key, r[0] + r[2] / 2.0, width, r[1], r[3]);
    }
    clicked
}

/// One of the Eaglercraft server's buttons: its text and pictures centred as one.
fn server_button(cx: &mut Cx, menu: &ServerMenu, r: [f32; 4], text: &str, key: &str) -> bool {
    let clicked = ui::button(cx.p, cx.ui, r, "", true);
    let room = r[2] - 8.0 - (menu.content_width(0.0, key));
    let text = cx.p.fit(text, room).into_owned();
    let text_width = cx.p.width(&text);
    let mut x = r[0] + ((r[2] - menu.content_width(text_width, key)) / 2.0).floor();
    let icon_y = r[1] + ((r[3] - ICON_SIZE) / 2.0).floor();
    if menu.has(key, 'L') {
        menu.draw_icon(cx.p, key, 'L', x, icon_y);
        x += ICON_SIZE + ICON_GAP;
    }
    let color = cx.p.theme.text;
    cx.p.text(&text, x, r[1] + ((r[3] - 7.0) / 2.0).floor(), color, true);
    if menu.has(key, 'R') {
        menu.draw_icon(cx.p, key, 'R', x + text_width + ICON_GAP, icon_y);
    }
    clicked
}

impl Screen for Pause {
    fn escape(&mut self, cx: &mut Cx) {
        if self.confirm.take().is_none() {
            cx.go(Go::Game("resume"));
        }
    }

    fn draw(&mut self, cx: &mut Cx) {
        if let Some(confirm) = &self.confirm {
            match confirm.draw(cx) {
                Some(Answer::Open) => {
                    cx.host.extras().open_link(&confirm.url);
                    self.confirm = None;
                }
                Some(_) => self.confirm = None,
                None => {}
            }
            return;
        }
        let [w, h] = cx.size;
        let th = cx.p.theme;
        let t = |cx: &Cx, key: &str| cx.host.translate(key);
        let menu = cx.host.server_menu();
        let menu = menu.as_deref();
        let extra = match menu.filter(|m| m.active()) {
            Some(m) => {
                let buttons: Vec<_> = m
                    .buttons()
                    .into_iter()
                    .map(|(b, key)| (b.text.clone(), b.safe_url().map(str::to_owned), key))
                    .collect();
                // Side by side if both fit in half the width.
                let halves = buttons.len() == 2
                    && buttons
                        .iter()
                        .all(|(text, _, key)| m.content_width(cx.p.width(text), key) <= HALF - 8.0);
                Extra::Eagler(buttons, halves)
            }
            None => match cx.host.pause_server_button() {
                Some((label, what)) => Extra::Game(label, what),
                None => Extra::None,
            },
        };
        // The game's menu: Back to Game; Advancements, Statistics; the icons; Incari Settings;
        // the server's; Options, World Options; Save and Quit (Disconnect on a server).
        let rows = 6.0 + extra.rows();
        let width = WIDE + 2.0 * PADDING;
        let height = HEADER + rows * ROW - 4.0 + 2.0 * PADDING;
        let x = ((w - width) / 2.0).floor();
        let y = ((h - height) / 2.0).floor().max(6.0);
        panels::panel(cx.p, x, y, width, height, 1.0);
        let title = t(cx, "menu.game");
        cx.p.text(&title, x + PADDING, y + PADDING, th.text, true);
        if let Some(menu) = menu {
            let tw = cx.p.width(&title);
            menu.draw_beside(cx.p, "title", x + PADDING + tw / 2.0, tw, y + PADDING, 8.0);
        }
        let left = x + PADDING;
        let mut top = y + PADDING + HEADER;
        let mut go = None;
        let back = t(cx, "menu.returnToGame");
        if game_button(
            cx,
            menu,
            [left, top, WIDE, CONTROL_HEIGHT],
            &back,
            Some("backToGame"),
        ) {
            go = Some(Go::Game("resume"));
        }
        top += ROW;
        let advancements = t(cx, "gui.advancements");
        if game_button(
            cx,
            menu,
            [left, top, HALF, CONTROL_HEIGHT],
            &advancements,
            Some("achievements"),
        ) {
            go = Some(Go::Game("advancements"));
        }
        let stats = t(cx, "gui.stats");
        if game_button(
            cx,
            menu,
            [left + WIDE - HALF, top, HALF, CONTROL_HEIGHT],
            &stats,
            Some("statistics"),
        ) {
            go = Some(Go::Game("stats"));
        }
        top += ROW;
        // The icon row (PauseScreen's): report bugs, feedback, friends. Incari has no Player
        // Reporting button.
        let icons = [
            ("gui/sprites/pause_menu/bug", "menu.reportBugs", Link::Bugs),
            (
                "gui/sprites/pause_menu/social_interactions",
                "menu.sendFeedback",
                Link::Feedback,
            ),
            (
                "gui/sprites/friends/friends",
                "gui.friends.open.tooltip",
                Link::Friends,
            ),
        ];
        let mut ix = left + ((WIDE - (icons.len() as f32 * 24.0 - 4.0)) / 2.0).floor();
        for (sprite, tip, action) in icons {
            let r = [ix, top, 20.0, 20.0];
            let hot = cx.ui.over(r);
            crate::skin::button(cx.p, r, false, hot, 1.0);
            // SpriteIconButton's: the whole sprite at 15 by 15, whatever its size (13, 20, 16).
            cx.p.sprite(
                sprite,
                DrawList::WHOLE,
                [ix + 3.0, top + 3.0, 15.0, 15.0],
                0xFFFF_FFFF,
            );
            if hot {
                let tip = t(cx, tip);
                cx.ui.tooltip(tip);
            }
            if cx.ui.clicked(r) {
                let build = cx.host.build_info();
                match action {
                    Link::Bugs => {
                        self.confirm = Some(ConfirmLink::new(credits::report_bugs(&build)))
                    }
                    Link::Feedback => {
                        self.confirm = Some(ConfirmLink::new(credits::feedback(&build)))
                    }
                    Link::Friends => go = Some(Go::GameOver("friends")),
                }
            }
            ix += 24.0;
        }
        // Incari Settings under the icons (PauseMenuHooks.addButtons), then the server's, then
        // the game's own.
        top += ROW;
        if ui::menu_button_ext(
            cx.p,
            cx.ui,
            [left, top, WIDE, CONTROL_HEIGHT],
            "Incari Settings",
            Some(Icon::Incari),
            "",
            true,
            true,
        ) {
            go = Some(Go::Open(Box::new(
                super::incari_settings::IncariSettings::default(),
            )));
        }
        top += ROW;
        match (&extra, menu) {
            (Extra::Eagler(buttons, halves), Some(m)) => {
                for (i, (text, url, key)) in buttons.iter().enumerate() {
                    let r = if *halves {
                        let bx = if i == 0 { left } else { left + WIDE - HALF };
                        [bx, top, HALF, CONTROL_HEIGHT]
                    } else {
                        [left, top, WIDE, CONTROL_HEIGHT]
                    };
                    // Opened without asking, as the Java build's do; only web links.
                    if server_button(cx, m, r, text, key)
                        && let Some(url) = url
                    {
                        cx.host.extras().open_link(url);
                    }
                    if !*halves || i == buttons.len() - 1 {
                        top += ROW;
                    }
                }
            }
            (Extra::Game(label, what), _) => {
                if ui::button(cx.p, cx.ui, [left, top, WIDE, CONTROL_HEIGHT], label, true) {
                    go = Some(Go::GameOver(what));
                }
                top += ROW;
            }
            _ => {}
        }
        let options = t(cx, "menu.options");
        if game_button(
            cx,
            menu,
            [left, top, HALF, CONTROL_HEIGHT],
            &options,
            Some("options"),
        ) {
            go = Some(Go::GameOver("options"));
        }
        let world = t(cx, "options.worldOptions.button");
        if game_button(
            cx,
            menu,
            [left + WIDE - HALF, top, HALF, CONTROL_HEIGHT],
            &world,
            None,
        ) {
            go = Some(Go::GameOver("lan"));
        }
        top += ROW;
        let quit = t(
            cx,
            if cx.host.on_server() {
                "menu.disconnect"
            } else {
                "menu.returnToMenu"
            },
        );
        if game_button(
            cx,
            menu,
            [left, top, WIDE, CONTROL_HEIGHT],
            &quit,
            Some("disconnect"),
        ) {
            go = Some(Go::Game("disconnect"));
        }
        if let Some(go) = go {
            cx.go(go);
        }
    }
}
