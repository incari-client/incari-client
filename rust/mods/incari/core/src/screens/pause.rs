//! The pause menu in the client's look (PauseLook.java, PauseMenuHooks.java): the game's buttons
//! gathered on one glass card in the middle of the screen, the title at the card's top left, and
//! Incari Settings with Incari's badge, the way into everything Incari adds.
use crate::icons::Icon;
use crate::panels;
use crate::screen::{Cx, Go, Screen};
use crate::theme::CONTROL_HEIGHT;
use crate::ui;

const PADDING: f32 = 12.0;
const HEADER: f32 = 9.0 + 9.0;
const WIDE: f32 = 204.0;
const HALF: f32 = 98.0;
const ROW: f32 = 24.0;

#[derive(Default)]
pub struct Pause;

impl Screen for Pause {
    fn escape(&mut self, cx: &mut Cx) {
        cx.go(Go::Game("resume"));
    }

    fn draw(&mut self, cx: &mut Cx) {
        let [w, h] = cx.size;
        let th = cx.p.theme;
        let t = |cx: &Cx, key: &str| cx.host.translate(key);
        // The game's menu: Back to Game; Advancements, Statistics; the icons; Incari Settings;
        // Options, World Options; Save and Quit (Disconnect on a server).
        let rows = 6.0;
        let width = WIDE + 2.0 * PADDING;
        let height = HEADER + rows * ROW - 4.0 + 2.0 * PADDING;
        let x = ((w - width) / 2.0).floor();
        let y = ((h - height) / 2.0).floor().max(6.0);
        panels::panel(cx.p, x, y, width, height, 1.0);
        let title = t(cx, "menu.game");
        cx.p.text(&title, x + PADDING, y + PADDING, th.text, true);
        let left = x + PADDING;
        let mut top = y + PADDING + HEADER;
        let mut go = None;
        if ui::button(
            cx.p,
            cx.ui,
            [left, top, WIDE, CONTROL_HEIGHT],
            &t(cx, "menu.returnToGame"),
            true,
        ) {
            go = Some(Go::Game("resume"));
        }
        top += ROW;
        if ui::button(
            cx.p,
            cx.ui,
            [left, top, HALF, CONTROL_HEIGHT],
            &t(cx, "gui.advancements"),
            true,
        ) {
            go = Some(Go::Game("advancements"));
        }
        if ui::button(
            cx.p,
            cx.ui,
            [left + WIDE - HALF, top, HALF, CONTROL_HEIGHT],
            &t(cx, "gui.stats"),
            true,
        ) {
            go = Some(Go::Game("stats"));
        }
        top += ROW;
        // The icon row (PauseScreen's): report bugs, feedback, friends. Incari has no Player
        // Reporting button.
        let icons = [
            (
                "gui/sprites/pause_menu/bug",
                "menu.reportBugs",
                "report_bugs",
            ),
            (
                "gui/sprites/pause_menu/social_interactions",
                "menu.sendFeedback",
                "feedback",
            ),
            (
                "gui/sprites/friends/friends",
                "gui.friends.open.tooltip",
                "friends",
            ),
        ];
        let mut ix = left + ((WIDE - (icons.len() as f32 * 24.0 - 4.0)) / 2.0).floor();
        for (sprite, tip, action) in icons {
            let r = [ix, top, 20.0, 20.0];
            let hot = cx.ui.over(r);
            crate::skin::button(cx.p, r, false, hot, 1.0);
            cx.p.sprite(
                sprite,
                [0.0, 0.0, 15.0, 15.0],
                [ix + 3.0, top + 3.0, 15.0, 15.0],
                0xFFFF_FFFF,
            );
            if hot {
                let tip = t(cx, tip);
                cx.ui.tooltip(tip);
            }
            if cx.ui.clicked(r) {
                go = Some(Go::GameOver(action));
            }
            ix += 24.0;
        }
        // Incari Settings under the icons (PauseMenuHooks.addButtons), then the game's own.
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
        if ui::button(
            cx.p,
            cx.ui,
            [left, top, HALF, CONTROL_HEIGHT],
            &t(cx, "menu.options"),
            true,
        ) {
            go = Some(Go::GameOver("options"));
        }
        let world = t(cx, "options.worldOptions.button");
        if ui::button(
            cx.p,
            cx.ui,
            [left + WIDE - HALF, top, HALF, CONTROL_HEIGHT],
            &world,
            true,
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
        if ui::button(cx.p, cx.ui, [left, top, WIDE, CONTROL_HEIGHT], &quit, true) {
            go = Some(Go::Game("disconnect"));
        }
        if let Some(go) = go {
            cx.go(go);
        }
    }
}
