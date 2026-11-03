//! Incari Settings (IncariSettingsScreen.java): the way into everything Incari adds (the module
//! menu, the HUD editor, the Wardrobe, screenshots, the recorder, Incari Chat, the account), and
//! the client's own switches.
use super::list::List;
use crate::color::{alpha, mix};
use crate::icons::Icon;
use crate::motion;
use crate::panels;
use crate::screen::{Cx, Go, Screen};
use crate::theme::CONTROL_WIDTH;
use crate::ui;

/// Tall enough for a tile's title and two lines of description.
const TILE: f32 = 44.0;
const BADGE: f32 = 22.0;
const BADGE_ICON: f32 = 12.0;

#[derive(Default)]
pub struct IncariSettings {
    list: List,
}

impl Screen for IncariSettings {
    fn draw(&mut self, cx: &mut Cx) {
        let caption = format!("Incari Client {}", cx.host.build_info().version);
        let recorder = cx.host.recorder();
        let (record_text, record_on) = (
            crate::recorder::tile_text(recorder.as_ref()),
            recorder.supported(),
        );
        let done = cx.host.translate("gui.done");
        let mut opened: Option<usize> = None;
        let pressed = self.list.draw(
            cx,
            "Incari Settings",
            &caption,
            true,
            |cx, rows| {
                rows.heading(cx, "Menus");
                let tiles: [(Icon, &str, &str, bool); 7] = [
                    (
                        Icon::Grid,
                        "Modules",
                        "Turn modules on and off and change their settings.",
                        true,
                    ),
                    (
                        Icon::Layout,
                        "Edit HUD",
                        "Move and resize HUD elements.",
                        true,
                    ),
                    (
                        Icon::Hanger,
                        "Cosmetics",
                        "Capes, hats, wings, trails and emotes.",
                        true,
                    ),
                    (
                        Icon::Camera,
                        "Screenshots",
                        "View, rename and download screenshots.",
                        true,
                    ),
                    (Icon::Target, "Record Screen", record_text, record_on),
                    (
                        Icon::Chat,
                        "Incari Chat",
                        "Friends and messages with Incari players.",
                        true,
                    ),
                    (
                        Icon::Person,
                        "Incari Account",
                        crate::online::account_tile_text(),
                        true,
                    ),
                ];
                for (pair, chunk) in tiles.chunks(2).enumerate() {
                    let Some(rects) = rows.across(chunk.len(), TILE) else {
                        continue;
                    };
                    for (i, (r, (icon, title, text, active))) in rects.iter().zip(chunk).enumerate()
                    {
                        let visible = rows.visible(*r);
                        if tile(cx, *r, visible, *icon, title, text, *active) {
                            opened = Some(pair * 2 + i);
                        }
                    }
                }
                rows.heading(cx, "Settings");
                let half = (CONTROL_WIDTH / 2.0).floor();
                let flags: [(&str, &str, Flag); 3] = [
                    (
                        "Copy screenshots",
                        "Copies each screenshot (F2) to the clipboard too.",
                        |p| &mut p.copy_screenshots,
                    ),
                    (
                        "Show Incari badges",
                        "Incari's badge beside other signed-in Incari players' names.",
                        |p| &mut p.show_badges,
                    ),
                    (
                        "Usage counts",
                        "Once a day, sends Incari a random number, the version and the build, \
                         nothing else. It helps us know how many people play.",
                        |p| &mut p.usage_counts,
                    ),
                ];
                for (i, (label, tip, flag)) in flags.into_iter().enumerate() {
                    let Some(r) = rows.row(cx, label, tip, &[half]) else {
                        continue;
                    };
                    let on = *flag(cx.prefs);
                    if ui::button(cx.p, cx.ui, r[0], if on { "ON" } else { "OFF" }, true) {
                        *flag(cx.prefs) = !on;
                        cx.prefs.dirty = true;
                    }
                    // The account's own badge setting, kept by the service: after Show Incari badges.
                    if i == 1 {
                        let (shows, can) = crate::online::show_my_badge();
                        if let Some(r) = rows.row(
                            cx,
                            crate::online::SHOW_MY_BADGE,
                            crate::online::SHOW_MY_BADGE_TIP,
                            &[half],
                        ) && ui::button(cx.p, cx.ui, r[0], if shows { "ON" } else { "OFF" }, can)
                        {
                            let t = crate::online::with(|o| o.set_show_badge(!shows));
                            crate::online::with(|o| o.forget(t));
                        }
                    }
                }
            },
            &[&done],
            None,
        );
        match opened {
            Some(0) => cx.open(super::modules::ModuleList::default()),
            Some(1) => cx.open(super::hud_editor::HudEditor::default()),
            Some(2) => cx.open(super::wardrobe::Wardrobe::new()),
            Some(4) => {
                let settings = cx
                    .host
                    .read("settings.txt")
                    .and_then(|b| String::from_utf8(b).ok())
                    .unwrap_or_default();
                let local = cx.host.local_time();
                let key = crate::input::Key::Keyboard(crate::recorder::KEY).label();
                cx.open(super::recorder::RecorderScreen::new(
                    cx.host.recorder(),
                    &settings,
                    &key,
                    local,
                ));
            }
            Some(5) => cx.open(super::chat_menu::ChatMenu::default()),
            Some(6) => cx.open(super::account::Account::default()),
            Some(3) => {
                let extras = cx.host.extras();
                cx.open(super::screenshots::Screenshots::new(extras));
            }
            _ => {}
        }
        if pressed.is_some() {
            cx.go(Go::Back);
        }
    }
}

/// One of the client's switches in its settings.
type Flag = fn(&mut crate::screen::Prefs) -> &mut bool;

/// A tile (Ui.Tile): a glass card with an icon in an accent badge, a title and a line or two
/// about what it opens.
fn tile(
    cx: &mut Cx,
    r: [f32; 4],
    visible: [f32; 4],
    icon: Icon,
    title: &str,
    text: &str,
    active: bool,
) -> bool {
    let th = cx.p.theme;
    let hot = active && cx.ui.over(visible);
    panels::card(cx.p, r[0], r[1], r[2], r[3], hot, false, 1.0);
    let t = cx.p.motion.ease(
        motion::key(r[0], r[1], -r[2], r[3]),
        if hot { 1.0 } else { 0.0 },
    );
    let (bx, by) = (r[0] + 8.0, r[1] + ((r[3] - BADGE) / 2.0).floor());
    // The badge brightens toward the accent under the pointer.
    let tint = if active { th.accent } else { th.muted };
    let badge = mix(tint & 0x33FF_FFFF, tint & 0x66FF_FFFF, t);
    cx.p.round(bx, by, BADGE, BADGE, th.radius + 2.0, badge);
    let off = ((BADGE - BADGE_ICON) / 2.0).floor();
    cx.p.icon(icon, bx + off, by + off, BADGE_ICON, tint);
    let tx = bx + BADGE + 8.0;
    let room = r[0] + r[2] - 8.0 - tx;
    let lines = cx.p.wrap(text, room, 2);
    let mut ty = r[1] + ((r[3] - 9.0 - lines.len() as f32 * 10.0) / 2.0).floor();
    let shown = cx.p.fit(title, room).into_owned();
    cx.p.text(
        &shown,
        tx,
        ty,
        if active { th.text } else { th.muted },
        false,
    );
    for line in &lines {
        ty += 10.0;
        cx.p.text(line, tx, ty, alpha(th.muted, 1.0), false);
    }
    active && cx.ui.clicked(visible)
}
