//! The title screen in the client's look (TitleLook.java): the backdrop under a cold tint that
//! deepens toward the menu, Incari's wordmark over a column of menu buttons on the left, the player
//! on a glass card on the right, and in Normal, falling snow and the menu rising in.
use crate::color::alpha;
use crate::icons::Icon;
use crate::motion::ease_out;
use crate::panels;
use crate::screen::{Cx, Go, Screen};
use crate::theme::{CONTROL_HEIGHT, GAP};
use crate::ui;

const NAME: &str = "INCARI";
const WORDMARK_HEIGHT: f32 = 26.0;
const WORDMARK_SIZE: f32 = 18.0;
const LETTER_SPACING: f32 = 3.0;
const ROW: f32 = CONTROL_HEIGHT;
const ROW_GAP: f32 = 5.0;
/// How long the menu takes to rise in, in Normal.
const RISE_MILLIS: f64 = 550.0;

pub struct Title {
    shown_at: f64,
    snow: Snowfall,
}

impl Title {
    pub fn new(now: f64) -> Self {
        Self {
            shown_at: now,
            snow: Snowfall::new(),
        }
    }
}

impl Screen for Title {
    fn backdrop(&self) -> bool {
        false
    }

    fn escape(&mut self, _cx: &mut Cx) {}

    fn draw(&mut self, cx: &mut Cx) {
        let [width, height] = cx.size;
        let normal = cx.p.theme.normal();
        let th = cx.p.theme;
        // The photo is drawn already (`Client::backdrop`): its tint, deeper toward the menu.
        cx.p.fill(0.0, 0.0, width, height, if normal { 0x1E10_1822 } else { 0x4010_1822 });
        cx.p.fade_right(0.0, 0.0, (width * 0.6).round(), height, if normal { 0x8C0C_131B } else { 0xB00C_131B });
        if normal {
            self.snow.draw(cx, width, height);
        }
        let shown = if normal {
            ((cx.p.motion.now - self.shown_at) / RISE_MILLIS).min(1.0) as f32
        } else {
            1.0
        };
        let eased = ease_out(shown);
        cx.p.origin = [0.0, (1.0 - eased) * 8.0];
        cx.p.fade = eased;

        let column_width = (width * 0.27).round().clamp(124.0, 168.0);
        let column_x = (width * 0.07).round().max(14.0);
        let rows = 4.0;
        let block = WORDMARK_HEIGHT + 18.0 + rows * ROW + (rows - 1.0) * ROW_GAP;
        let wordmark_y = ((height - block) / 2.0 - 6.0).floor().max(8.0);
        self.wordmark(cx, column_x, wordmark_y);

        let mut y = wordmark_y + WORDMARK_HEIGHT + 24.0;
        let p = &mut *cx.p;
        let singleplayer = cx.host.translate("menu.singleplayer");
        if ui::menu_button(p, cx.ui, [column_x, y, column_width, ROW], &singleplayer, Some(Icon::Cube), "", true) {
            cx.go = Some(Go::Game("singleplayer"));
        }
        y += ROW + ROW_GAP;
        let multiplayer = cx.host.translate("menu.multiplayer");
        if ui::menu_button(p, cx.ui, [column_x, y, column_width, ROW], &multiplayer, Some(Icon::Globe), "", true) {
            cx.go = Some(Go::Game("multiplayer"));
        }
        y += ROW + ROW_GAP;
        // Incari Settings and modules' own buttons on a row under Multiplayer.
        let mut extra: Vec<(&str, &str, Option<Icon>)> = vec![("incari", "Incari Settings", Some(Icon::Incari))];
        for m in cx.modules.modules.iter().filter(|m| m.enabled) {
            for b in &m.menu_buttons {
                extra.push((b.id, b.label, Icon::named(b.icon)));
            }
        }
        let count = extra.len().min(3);
        let each = ((column_width - (count as f32 - 1.0) * ROW_GAP) / count as f32).floor();
        for (i, (id, label, icon)) in extra.iter().take(count).enumerate() {
            let w = if i == count - 1 {
                column_width - (count as f32 - 1.0) * (each + ROW_GAP)
            } else {
                each
            };
            let r = [column_x + i as f32 * (each + ROW_GAP), y, w, ROW];
            if ui::menu_button(p, cx.ui, r, label, *icon, "", true) && *id == "incari" {
                cx.go = Some(Go::Open(Box::new(super::incari_settings::IncariSettings::default())));
            }
        }
        y += ROW + ROW_GAP;
        let options = cx.host.translate("menu.options");
        if ui::menu_button(p, cx.ui, [column_x, y, column_width, ROW], &options, Some(Icon::Gear), "", true) {
            cx.go = Some(Go::Game("options"));
        }

        self.card(cx, width, height, column_x, eased);
        cx.p.origin = [0.0, 0.0];
        cx.p.fade = 1.0;

        // The game's language and accessibility buttons, small, in the top right corner.
        let lang = [width - 2.0 * ROW - 10.0, 6.0, ROW, ROW];
        if icon_button(cx, lang, "gui/sprites/icon/language", "options.language") {
            cx.go = Some(Go::Game("language"));
        }
        let access = [width - ROW - 6.0, 6.0, ROW, ROW];
        if icon_button(cx, access, "gui/sprites/icon/accessibility", "options.accessibility") {
            cx.go = Some(Go::Game("accessibility"));
        }
        // The copyright, a link to the credits, and the version.
        let copyright = cx.host.translate("title.credits");
        let cw = cx.p.width(&copyright);
        let cr = [width - cw - 6.0, height - 12.0, cw, 10.0];
        let hot = cx.ui.over(cr);
        cx.p.text(&copyright, cr[0], height - 10.0, th.text, true);
        if hot {
            cx.p.fill(cr[0], height - 1.0, cw, 1.0, th.text);
        }
        if cx.ui.clicked(cr) {
            // Incari's credits, which lead on to the game's own (Credits & Attribution).
            let build = cx.host.build_info();
            let extras = cx.host.extras();
            cx.go = Some(Go::Open(Box::new(super::credits::Credits::new(crate::credits::all(&build), extras))));
        }
        let version = format!("Incari  ·  Minecraft {}", cx.host.game_version());
        let vy = if 6.0 + cx.p.width(&version) + GAP * 2.0 > cr[0] { height - 24.0 } else { height - 12.0 };
        cx.p.text(&version, 6.0, vy + 2.0, th.muted, false);
    }
}

impl Title {
    /// Incari's mark and INCARI, letter-spaced, and the line under them.
    fn wordmark(&self, cx: &mut Cx, x: f32, y: f32) {
        let th = cx.p.theme;
        if th.normal() {
            cx.p.shadow(
                x + 4.0,
                y + 4.0,
                WORDMARK_HEIGHT - 8.0,
                WORDMARK_HEIGHT - 8.0,
                (WORDMARK_HEIGHT - 8.0) / 2.0,
                8.0,
                th.accent & 0x55FF_FFFF,
            );
        }
        cx.p.logo(x, y, WORDMARK_HEIGHT, 1.0);
        let mut lx = x + WORDMARK_HEIGHT + 9.0;
        for letter in NAME.chars() {
            lx += cx.p.wordmark_letter(letter, lx, y + 3.0, WORDMARK_SIZE, th.text) + LETTER_SPACING;
        }
        let subtitle = format!("CLIENT  ·  {}", cx.host.build_info().version);
        cx.p.text(&subtitle, x + WORDMARK_HEIGHT + 10.0, y + WORDMARK_HEIGHT - 3.0, th.muted, false);
    }

    /// The player card on the right: their name, Edit Profile, Cosmetics and Incari Account.
    fn card(&self, cx: &mut Cx, width: f32, height: f32, column_x: f32, eased: f32) {
        let th = cx.p.theme;
        let card_width = (width * 0.2).round().clamp(104.0, 132.0);
        let model_height = (height * 0.38).round().min(112.0);
        let card_height = 12.0 + 9.0 + 8.0 + model_height + 8.0 + 9.0 + 8.0 + 3.0 * ROW + 2.0 * ROW_GAP + 12.0;
        let card_x = width - column_x - card_width;
        if card_x < column_x + 168.0 + 24.0 || card_height > height - 40.0 {
            // A narrow screen: the profile's links in the top left corner instead.
            let mut x = 6.0;
            for (label, go) in [("Edit Profile", "profile"), ("Cosmetics", ""), ("Incari Account", "")] {
                let w = cx.p.width(label) + 12.0;
                if ui::button(cx.p, cx.ui, [x, 6.0, w, 14.0], label, true) {
                    if label == "Incari Account" {
                        cx.open(crate::screens::account::Account::default());
                    } else if label == "Cosmetics" {
                        cx.open(crate::screens::wardrobe::Wardrobe::new());
                    } else if !go.is_empty() {
                        cx.go = Some(Go::Game("profile"));
                    }
                }
                x += w + 4.0;
            }
            return;
        }
        let card_y = ((height - card_height) / 2.0).floor();
        if th.normal() {
            // Normal's glass is light, made for a menu's dark tint: out here on the photo's
            // bright cloud its white text would wash out.
            cx.p.round(card_x, card_y, card_width, card_height, th.panel_radius, alpha(0x800C_131B, eased));
        }
        panels::panel(cx.p, card_x, card_y, card_width, card_height, 1.0);
        cx.p.centered("OFFLINE MODE", card_x + card_width / 2.0, card_y + 12.0, th.muted, true);
        let model_top = card_y + 12.0 + 9.0 + 8.0;
        let model_bottom = model_top + model_height;
        let model_width = (model_height * 11.0 / 20.0).floor();
        crate::cosmetics::preview::draw_title(
            cx,
            [card_x + ((card_width - model_width) / 2.0).floor(), model_top, model_width, model_height],
        );
        let name = cx.host.player_name();
        cx.p.centered(&name, card_x + card_width / 2.0, model_bottom + 8.0, th.text, true);
        let bx = card_x + 12.0;
        let bw = card_width - 24.0;
        let mut by = model_bottom + 8.0 + 9.0 + 8.0;
        if ui::button(cx.p, cx.ui, [bx, by, bw, ROW], "Edit Profile", true) {
            cx.go = Some(Go::Game("profile"));
        }
        by += ROW + ROW_GAP;
        if ui::button(cx.p, cx.ui, [bx, by, bw, ROW], "Cosmetics", true) {
            cx.open(crate::screens::wardrobe::Wardrobe::new());
        }
        by += ROW + ROW_GAP;
        let account = crate::online::account_button_label();
        if ui::button(cx.p, cx.ui, [bx, by, bw, ROW], &account, true) {
            cx.open(crate::screens::account::Account::default());
        }
    }
}

/// One of the game's small icon buttons (`SpriteIconButton`): its sprite on a button.
fn icon_button(cx: &mut Cx, r: [f32; 4], sprite: &str, tooltip_key: &str) -> bool {
    let hot = cx.ui.over(r);
    crate::skin::button(cx.p, r, false, hot, 1.0);
    cx.p.sprite(sprite, [0.0, 0.0, 15.0, 15.0], [r[0] + 3.0, r[1] + 3.0, 15.0, 15.0], 0xFFFF_FFFF);
    if hot {
        let tip = cx.host.translate(tooltip_key);
        cx.ui.tooltip(tip);
    }
    cx.ui.clicked(r)
}

/// Snow falling over the menus in Normal (Snowfall.java): soft flakes drifting down and swaying,
/// larger ones nearer and faster. Positions are fractions of the screen.
struct Snowfall {
    flakes: Vec<[f32; 5]>,
    last: f64,
    seed: u32,
}

const FLAKES: usize = 110;

impl Snowfall {
    fn new() -> Self {
        let mut s = Self {
            flakes: Vec::with_capacity(FLAKES),
            last: 0.0,
            seed: 0x2F6_E2B1,
        };
        for _ in 0..FLAKES {
            let (x, y, size) = (s.random(), s.random(), 0.25 + s.random() * 0.75);
            // A screen's height in 9 to 26 seconds; the nearest, largest flakes fastest.
            let speed = 0.038 + size * 0.07;
            let phase = s.random() * std::f32::consts::TAU;
            s.flakes.push([x, y, size, speed, phase]);
        }
        s
    }

    fn random(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    fn draw(&mut self, cx: &mut Cx, width: f32, height: f32) {
        let now = cx.p.motion.now;
        let dt = if self.last == 0.0 { 0.0 } else { ((now - self.last) / 1000.0).min(0.1) as f32 };
        self.last = now;
        let seconds = ((now % 600_000.0) / 1000.0) as f32;
        let scale = cx.p.scale;
        for i in 0..self.flakes.len() {
            let [x, y, size, speed, phase] = self.flakes[i];
            let mut y = y + speed * dt;
            let mut x = x;
            if y > 1.02 {
                y = -0.02;
                x = self.random();
            }
            self.flakes[i][0] = x;
            self.flakes[i][1] = y;
            let sway = (seconds * (0.5 + size * 0.4) + phase).sin() * 0.008;
            let a = (90.0 + size * 130.0).round() as u32;
            let px = ((1.5 + size * 3.5) * scale / 2.0).round().max(2.0);
            cx.p.dot((x + sway) * width, y * height, px / scale, a << 24 | 0xFF_FFFF);
        }
    }
}
