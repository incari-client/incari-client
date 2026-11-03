//! The Wardrobe (ADR 0061; `WardrobeScreen.java`): the player's cosmetics, laid out as the module
//! menu is. Kinds down the left (Capes, Hats, Wings & Back, Name Tags, Trails, Emotes, Outfits), a
//! grid of cards in the middle, and the player on the right wearing what's picked. Pointing at a
//! card tries it on; a click puts it on, and a click on what's worn (or on None) takes it off.
//! Emotes play on the preview. Outfits are what's worn saved under a name, put on again in a click.
//!
//! Under the preview: whether other Incari players see what's worn (signed in) or only the player
//! does (with Sign In), and the switch that hides other players' cosmetics.
//!
//! Opened from the title card's Cosmetics button, Incari Settings and the module menu. Changes
//! are made at once and reach the service a moment later (`cosmetics::wardrobe`).
use crate::cosmetics::catalog::{self, Cosmetic, Slot};
use crate::cosmetics::loadout::Loadout;
use crate::cosmetics::preview::PlayerPreview;
use crate::cosmetics::wardrobe::{self as wardrobe_file, MAX_OUTFIT_NAME};
use crate::cosmetics::{nametags, textures};
use crate::icons::Icon;
use crate::online::{self, Status};
use crate::panels::{self, SWITCH_WIDTH};
use crate::screen::{Cx, Go, Screen};
use crate::theme::{CONTROL_HEIGHT, GAP};
use crate::ui;
use std::cell::Cell;

const SIDEBAR: f32 = 104.0;
const PREVIEW: f32 = 128.0;
const PADDING: f32 = 10.0;
const ENTRY: f32 = 16.0;
const CARD_WIDTH: f32 = 64.0;
const CARD_HEIGHT: f32 = 74.0;
const CARD_GAP: f32 = 6.0;
const SCROLL_STEP: f32 = 24.0;
const TRASH: f32 = 9.0;
const LETTERS: [&str; 8] = ["W", "A", "R", "D", "R", "O", "B", "E"];

/// A kind of cosmetic down the left, and Outfits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Kind(Slot),
    Outfits,
}

const TABS: [Tab; 7] = [
    Tab::Kind(Slot::Cape),
    Tab::Kind(Slot::Hat),
    Tab::Kind(Slot::Back),
    Tab::Kind(Slot::Nametag),
    Tab::Kind(Slot::Trail),
    Tab::Kind(Slot::Emote),
    Tab::Outfits,
];

impl Tab {
    fn label(self) -> &'static str {
        match self {
            Tab::Kind(slot) => slot.label(),
            Tab::Outfits => "Outfits",
        }
    }

    fn index(self) -> usize {
        TABS.iter().position(|t| *t == self).unwrap_or(0)
    }
}

thread_local! {
    /// The tab shown last, shown again next time.
    static LAST_TAB: Cell<usize> = const { Cell::new(0) };
}

/// A card: a cosmetic, None for a kind (takes it off), an outfit, or, on the Outfits tab with
/// neither, Take Off All.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Card {
    Cosmetic(&'static Cosmetic),
    None(Slot),
    Outfit(usize),
    TakeOffAll,
}

pub struct Wardrobe {
    tab: Tab,
    scroll: f32,
    preview: PlayerPreview,
    outfit_name: String,
    notice: Option<(String, bool)>,
    /// The kind the preview was last turned for.
    faced: Option<Tab>,
}

impl Default for Wardrobe {
    fn default() -> Self {
        Self::new()
    }
}

impl Wardrobe {
    /// The Wardrobe open on the tab shown last.
    pub fn new() -> Self {
        let tab = TABS[LAST_TAB.with(Cell::get).min(TABS.len() - 1)];
        Self {
            tab,
            scroll: 0.0,
            preview: PlayerPreview::default(),
            outfit_name: String::new(),
            notice: None,
            faced: None,
        }
    }

    /// The Wardrobe open on `slot`'s kind, or on Outfits for None.
    pub fn showing(slot: Option<Slot>) -> Self {
        let mut w = Self::new();
        w.tab = slot.map_or(Tab::Outfits, Tab::Kind);
        w
    }

    fn choose(&mut self, tab: Tab) {
        self.tab = tab;
        LAST_TAB.with(|t| t.set(tab.index()));
        self.scroll = 0.0;
        self.notice = None;
        if let Tab::Kind(slot) = tab {
            self.preview.show(slot);
        }
    }

    fn hint(&self) -> String {
        match self.tab {
            Tab::Kind(Slot::Emote) => format!("In a world, hold {}", super::emote_wheel::KEY_LABEL),
            Tab::Kind(Slot::Nametag) => "Unless the server colors names".into(),
            Tab::Kind(Slot::Hat) => "Hidden under a helmet".into(),
            Tab::Kind(Slot::Back) => "Hidden under an elytra".into(),
            _ => String::new(),
        }
    }

    fn cards(&self, outfits: usize) -> Vec<Card> {
        match self.tab {
            Tab::Outfits => std::iter::once(Card::TakeOffAll)
                .chain((0..outfits).map(Card::Outfit))
                .collect(),
            Tab::Kind(slot) => {
                let none = slot.worn().then_some(Card::None(slot));
                none.into_iter()
                    .chain(catalog::in_slot(slot).map(Card::Cosmetic))
                    .collect()
            }
        }
    }
}

/// What the wardrobe shows now, read once a frame.
struct Seen {
    worn: Loadout,
    outfits: Vec<(String, Loadout)>,
    shared: bool,
    show_others: bool,
}

fn label(card: Card, seen: &Seen) -> String {
    match card {
        Card::Cosmetic(c) => c.name.to_owned(),
        Card::None(_) => "None".into(),
        Card::Outfit(i) => seen
            .outfits
            .get(i)
            .map_or_else(String::new, |o| o.0.clone()),
        Card::TakeOffAll => "Take Off All".into(),
    }
}

fn owns(c: &Cosmetic) -> bool {
    wardrobe_file::with(|w| w.owns(c))
}

/// What the preview wears while the pointer's on `card`.
fn try_on(card: Card, seen: &Seen) -> Option<Loadout> {
    match card {
        Card::Cosmetic(c) => (c.slot.worn() && owns(c)).then(|| seen.worn.with(c)),
        Card::Outfit(i) => seen.outfits.get(i).map(|o| o.1),
        Card::None(slot) => Some(seen.worn.without(slot)),
        Card::TakeOffAll => Some(Loadout::EMPTY),
    }
}

/// Whether `card` is what's worn.
fn worn(card: Card, seen: &Seen) -> bool {
    match card {
        Card::Cosmetic(c) => seen.worn.wears(c),
        Card::Outfit(i) => seen.outfits.get(i).is_some_and(|o| o.1 == seen.worn),
        Card::None(slot) => slot.worn() && seen.worn.in_slot(slot).is_none(),
        Card::TakeOffAll => false,
    }
}

impl Screen for Wardrobe {
    fn draw(&mut self, cx: &mut Cx) {
        wardrobe_file::load_and_save(cx.host);
        let now = cx.host.now_ms();
        wardrobe_file::with(|w| online::with(|o| w.sync(o, now)));
        let status = online::with(|o| o.status());
        let seen = wardrobe_file::with(|w| Seen {
            worn: w.worn(),
            outfits: w
                .outfits()
                .iter()
                .map(|o| (o.name.clone(), o.loadout))
                .collect(),
            shared: w.shared(),
            show_others: w.show_others(),
        });
        let [width, height] = cx.size;
        let th = cx.p.theme;
        let panel_w = (width - 24.0).min(560.0);
        let panel_h = (height - 20.0).min(300.0);
        let px = ((width - panel_w) / 2.0).floor();
        let py = ((height - panel_h) / 2.0).floor();
        panels::panel(cx.p, px, py, panel_w, panel_h, 1.0);

        // The sidebar's mark, and the lines between the columns.
        cx.p.icon(Icon::Hanger, px + 10.0, py + 10.0, 13.0, th.text);
        let mut lx = px + 28.0;
        for letter in LETTERS {
            cx.p.text(letter, lx, py + 14.0, th.text, false);
            lx += cx.p.width(letter) + 2.0;
        }
        let preview_w =
            (panel_w - SIDEBAR - 2.0 * PADDING - 2.0 * CARD_WIDTH - CARD_GAP).clamp(88.0, PREVIEW);
        let preview_x = px + panel_w - PADDING - preview_w;
        cx.p.fill(px + SIDEBAR, py + 8.0, 1.0, panel_h - 16.0, 0x1AFF_FFFF);
        cx.p.fill(preview_x - 5.0, py + 8.0, 1.0, panel_h - 16.0, 0x1AFF_FFFF);

        // The kinds, down the left, with how many the player has.
        let x = px + 8.0;
        let entry_w = SIDEBAR - 16.0;
        let mut y = py + 34.0;
        for tab in TABS {
            let count = match tab {
                Tab::Outfits => seen.outfits.len(),
                Tab::Kind(slot) => catalog::in_slot(slot).filter(|c| owns(c)).count(),
            };
            if ui::entry(
                cx.p,
                cx.ui,
                [x, y, entry_w, ENTRY],
                tab.label(),
                &count.to_string(),
                self.tab == tab,
            ) {
                self.choose(tab);
            }
            y += ENTRY + 2.0;
        }
        let done = cx.host.translate("gui.done");
        if ui::button(
            cx.p,
            cx.ui,
            [
                x,
                py + panel_h - 8.0 - CONTROL_HEIGHT,
                entry_w,
                CONTROL_HEIGHT,
            ],
            &done,
            true,
        ) {
            cx.go(Go::Back);
            return;
        }
        if self.faced != Some(self.tab) {
            self.faced = Some(self.tab);
            if let Tab::Kind(slot) = self.tab {
                self.preview.face(slot);
            }
        }

        // The grid: the Outfits tab has a name and Save over its cards.
        let grid_x = px + SIDEBAR + PADDING;
        let grid_w = preview_x - PADDING - grid_x;
        let mut grid_y = py + PADDING + 14.0;
        if self.tab == Tab::Outfits {
            let save_w = 70.0;
            // A grid too narrow for both side by side puts Save under the name.
            let stacked = grid_w < save_w + GAP + 80.0;
            let name_w = if stacked {
                grid_w
            } else {
                grid_w - save_w - GAP
            };
            let hint = cx.p.fit("Name this outfit", name_w - 8.0).into_owned();
            ui::text_field(
                cx.p,
                cx.ui,
                ui::id("outfit-name", 0),
                [grid_x, grid_y, name_w, CONTROL_HEIGHT],
                &mut self.outfit_name,
                &hint,
                MAX_OUTFIT_NAME,
            );
            if stacked {
                grid_y += CONTROL_HEIGHT + GAP;
            }
            let save_r = [
                if stacked {
                    grid_x
                } else {
                    grid_x + grid_w - save_w
                },
                grid_y,
                if stacked { grid_w } else { save_w },
                CONTROL_HEIGHT,
            ];
            if cx.ui.over(save_r) {
                cx.ui.tooltip("Save what you're wearing as an outfit");
            }
            if ui::button(cx.p, cx.ui, save_r, "Save", true) {
                let saved = wardrobe_file::with(|w| w.save_outfit(&self.outfit_name));
                self.notice = Some(if saved {
                    ("Outfit saved.".into(), false)
                } else {
                    ("No room for more outfits. Delete one first.".into(), true)
                });
            }
            grid_y += CONTROL_HEIGHT + PADDING;
        } else {
            // The grid's heading: the kind, and a hint for the ones that need one.
            let label = self.tab.label();
            cx.p.text(label, grid_x, py + PADDING, th.text, false);
            // Cut on a narrow grid rather than run into the label.
            let hint = self.hint();
            let hint =
                cx.p.fit(&hint, grid_w - cx.p.width(label) - 8.0)
                    .into_owned();
            let hw = cx.p.width(&hint);
            cx.p.text(&hint, grid_x + grid_w - hw, py + PADDING, th.muted, false);
        }
        let grid_h = py + panel_h - PADDING - grid_y;
        let area = [grid_x, grid_y, grid_w, grid_h];

        // Lay the cards out, and try on what the pointer's on.
        let cards = self.cards(seen.outfits.len());
        let columns = ((grid_w + CARD_GAP) / (CARD_WIDTH + CARD_GAP))
            .floor()
            .max(1.0);
        let card_w = ((grid_w - (columns - 1.0) * CARD_GAP) / columns).floor();
        let rows = (cards.len() as f32 / columns).ceil();
        let content = (rows * (CARD_HEIGHT + CARD_GAP) - CARD_GAP).max(0.0);
        ui::scroll(cx.ui, area, &mut self.scroll, content, SCROLL_STEP);
        let rect_of = |n: usize, scroll: f32| {
            let (col, row) = ((n as f32 % columns).floor(), (n as f32 / columns).floor());
            [
                grid_x + col * (card_w + CARD_GAP),
                grid_y + row * (CARD_HEIGHT + CARD_GAP) - scroll.floor(),
                card_w,
                CARD_HEIGHT,
            ]
        };
        let mut tried = None;
        for (n, card) in cards.iter().enumerate() {
            let visible = crate::draw::intersect(rect_of(n, self.scroll), area);
            if visible[2] > 0.0 && visible[3] > 0.0 && cx.ui.over(visible) {
                tried = try_on(*card, &seen);
            }
        }
        let shown = tried.unwrap_or(seen.worn);

        // The name over the preview, in the name tag cosmetic if one's shown.
        let name = cx.host.player_name();
        let name = cx.p.fit(&name, preview_w - 6.0).into_owned();
        let label_w = cx.p.width(&name);
        let label_x = (preview_x + (preview_w - label_w) / 2.0).floor();
        let label_y = py + PADDING;
        cx.p.fill(
            label_x - 3.0,
            label_y - 2.0,
            label_w + 5.0,
            11.0,
            0x4000_0000,
        );
        draw_name(cx, &name, shown.in_slot(Slot::Nametag), label_x, label_y);

        // The player, wearing what's tried on or worn.
        let preview_top = py + PADDING + 14.0;
        let preview_h = panel_h - PADDING - 14.0 - 64.0;
        self.preview.draw(
            cx.p,
            cx.ui,
            [preview_x + 8.0, preview_top, preview_w - 16.0, preview_h],
            shown,
        );

        // Who sees what's worn, and Sign In when only the player does.
        let status_y = preview_top + preview_h + 4.0;
        let text = if seen.shared {
            "Incari players see these"
        } else {
            "Only you see these"
        };
        let text = cx.p.fit(text, preview_w - 12.0).into_owned();
        let status_w = cx.p.width(&text) + 12.0;
        let status_x = (preview_x + (preview_w - status_w) / 2.0).floor();
        let color = if seen.shared { th.positive } else { th.muted };
        cx.p.icon(
            if seen.shared { Icon::Check } else { Icon::Eye },
            status_x,
            status_y,
            9.0,
            color,
        );
        cx.p.text(&text, status_x + 12.0, status_y + 1.0, color, false);
        if !seen.shared && status == Status::SignedOut {
            let r = [preview_x, status_y + 12.0, preview_w, CONTROL_HEIGHT];
            if cx.ui.over(r) {
                cx.ui
                    .tooltip("Sign in so other Incari players can see your cosmetics");
            }
            if ui::button(cx.p, cx.ui, r, "Sign In", true) {
                cx.open(super::account::Account::default());
                return;
            }
        }

        // The switch for other players' cosmetics.
        let switch_r = [preview_x, py + panel_h - 8.0 - 14.0, preview_w, 12.0];
        let hot = cx.ui.over(switch_r);
        if hot {
            cx.ui.tooltip(
                "Off hides other players' capes, hats, wings, trails and emotes. Yours still show.",
            );
        }
        // Cut short of the switch beside a narrow preview; the tooltip says what it does.
        let switch_label =
            cx.p.fit("Show others' cosmetics", preview_w - SWITCH_WIDTH - 4.0)
                .into_owned();
        cx.p.text(
            &switch_label,
            switch_r[0],
            switch_r[1] + 2.0,
            if hot { th.text } else { th.muted },
            false,
        );
        panels::toggle(
            cx.p,
            switch_r[0] + switch_r[2] - SWITCH_WIDTH,
            switch_r[1] + 1.0,
            seen.show_others,
            1.0,
        );
        if cx.ui.clicked(switch_r) {
            wardrobe_file::with(|w| w.set_show_others(!seen.show_others));
        }

        // The cards, inside the grid's clip.
        cx.p.clip_start([grid_x - 6.0, grid_y, grid_w + 12.0, grid_h]);
        for (n, card) in cards.iter().enumerate() {
            let r = rect_of(n, self.scroll);
            if r[1] + r[3] <= grid_y || r[1] >= grid_y + grid_h {
                continue;
            }
            if self.card(cx, *card, r, area, &seen) {
                break;
            }
        }
        cx.p.clip_end();
        if self.tab == Tab::Outfits && cards.len() <= 1 {
            let mut line_y = grid_y + CARD_HEIGHT + 16.0;
            for line in cx.p.wrap(
                "Put on what you like, name it above and press Save.",
                grid_w,
                3,
            ) {
                cx.p.centered(&line, grid_x + grid_w / 2.0, line_y, th.muted, false);
                line_y += 10.0;
            }
        }
        if let Some((text, warning)) = &self.notice {
            let text = cx.p.fit(text, grid_w).into_owned();
            cx.p.text(
                &text,
                grid_x,
                py + panel_h - PADDING + 1.0,
                if *warning { th.warning } else { th.muted },
                false,
            );
        }
        ui::scrollbar(cx.p, area, content, self.scroll);
        wardrobe_file::load_and_save(cx.host);
    }
}

/// The player's name at (`x`, `y`) in the name tag cosmetic `style`, or plain white.
fn draw_name(cx: &mut Cx, name: &str, style: Option<&'static Cosmetic>, x: f32, y: f32) {
    let Some(style) = style else {
        cx.p.text(name, x, y, 0xFFFF_FFFF, false);
        return;
    };
    let mut at = x;
    for (piece, rgb) in nametags::styled(name, style) {
        cx.p.text(&piece, at, y, 0xFF00_0000 | rgb, false);
        at += cx.p.width(&piece);
    }
}

impl Wardrobe {
    /// Card `card` at `r` inside the grid `area`. True when the cards changed under it (an outfit
    /// deleted): the rest wait for the next frame.
    fn card(&mut self, cx: &mut Cx, card: Card, r: [f32; 4], area: [f32; 4], seen: &Seen) -> bool {
        let th = cx.p.theme;
        let visible = crate::draw::intersect(r, area);
        let hot = cx.ui.over(visible);
        let on = worn(card, seen);
        let owned = match card {
            Card::Cosmetic(c) => owns(c),
            _ => true,
        };
        panels::card(
            cx.p,
            r[0],
            r[1],
            r[2],
            r[3],
            hot,
            on,
            if owned { 1.0 } else { 0.5 },
        );
        self.picture(cx, card, r, seen);
        // An outfit's name can be 24 characters: cut to the card, not into the next one.
        let text = label(card, seen);
        let text = cx.p.fit(&text, r[2] - 8.0).into_owned();
        let tw = cx.p.width(&text);
        let tx = r[0] + (((r[2] - tw) / 2.0).floor()).max(4.0);
        cx.p.text(
            &text,
            tx,
            r[1] + r[3] - 14.0,
            if owned { th.text } else { th.muted },
            false,
        );
        if on {
            cx.p.icon(Icon::Check, r[0] + r[2] - 13.0, r[1] + 5.0, 8.0, th.accent);
        }
        let trash = [
            r[0] + r[2] - 5.0 - TRASH - 2.0,
            r[1],
            TRASH + 7.0,
            5.0 + TRASH + 2.0,
        ];
        let over_trash = matches!(card, Card::Outfit(_)) && hot && cx.ui.over(trash);
        if matches!(card, Card::Outfit(_)) && hot {
            cx.p.icon(
                Icon::Trash,
                r[0] + r[2] - 5.0 - TRASH,
                r[1] + 5.0,
                TRASH,
                if over_trash { th.negative } else { th.muted },
            );
        }
        if !owned {
            cx.p.text("Locked", r[0] + 5.0, r[1] + 5.0, th.muted, false);
        }
        if !cx.ui.clicked(visible) {
            return false;
        }
        match card {
            Card::Outfit(i) => {
                if over_trash {
                    let name = seen
                        .outfits
                        .get(i)
                        .map_or_else(String::new, |o| o.0.clone());
                    wardrobe_file::with(|w| w.delete_outfit(i));
                    self.notice = Some((format!("Deleted {name}."), false));
                    return true;
                }
                if let Some(o) = seen.outfits.get(i) {
                    wardrobe_file::with(|w| w.wear_all(o.1));
                }
            }
            Card::None(slot) => wardrobe_file::with(|w| w.take_off(slot)),
            Card::TakeOffAll => wardrobe_file::with(|w| w.wear_all(Loadout::EMPTY)),
            Card::Cosmetic(c) if owned => {
                if c.slot == Slot::Emote {
                    self.preview.play(c, cx.p.motion.now);
                } else if seen.worn.wears(c) {
                    wardrobe_file::with(|w| w.take_off(c.slot));
                } else {
                    wardrobe_file::with(|w| w.wear(c));
                    self.preview.show(c.slot);
                }
            }
            Card::Cosmetic(_) => {}
        }
        false
    }

    /// The card's picture, 44 tall, centred across.
    fn picture(&mut self, cx: &mut Cx, card: Card, r: [f32; 4], seen: &Seen) {
        let th = cx.p.theme;
        let (x, y) = ((r[0] + r[2] / 2.0).floor(), r[1] + 6.0);
        let shown = match card {
            Card::Cosmetic(c) => c,
            Card::Outfit(i) => {
                let loadout = seen.outfits.get(i).map_or(Loadout::EMPTY, |o| o.1);
                outfit_picture(cx, x, y, loadout);
                return;
            }
            Card::None(_) | Card::TakeOffAll => {
                cx.p.icon(Icon::Close, x - 8.0, y + 14.0, 16.0, th.muted);
                return;
            }
        };
        match shown.slot {
            // The cape's outer side, as it hangs: 10x16 pixels at 2.6 times the size.
            Slot::Cape => textures::draw_cape(cx.p, shown, [x - 13.0, y + 1.0, 26.0, 42.0]),
            Slot::Hat | Slot::Back => textures::draw_icon(cx.p, shown, x - 18.0, y + 4.0, 36.0),
            Slot::Nametag => {
                let name = cx.host.player_name();
                let name_w = (r[2] - 8.0).min(cx.p.width(&name));
                cx.p.fill(
                    x - (name_w / 2.0).floor() - 3.0,
                    y + 15.0,
                    name_w + 6.0,
                    12.0,
                    0x6000_0000,
                );
                cx.p.clip_start([r[0] + 3.0, y, r[2] - 6.0, 40.0]);
                let nx = x - (cx.p.width(&name) / 2.0).floor();
                draw_name(cx, &name, Some(shown), nx, y + 17.0);
                cx.p.clip_end();
            }
            Slot::Trail => {
                let colors: &[u32] = match shown.id {
                    "trail.hearts" => &[0xFFFF_5A7A, 0xFFFF_8FA6],
                    "trail.sparkles" => &[0xFFFF_F4C2, 0xFFFF_FFFF],
                    "trail.notes" => &[0xFF6A_F2B0, 0xFFA0_6CFF, 0xFFFF_D36A],
                    _ => &[0xFFFF_FFFF, 0xFFA9_DCFF],
                };
                // Dots drifting up from the bottom left, bigger toward the start.
                for i in 0..7 {
                    let dot_x = x - 16.0 + i as f32 * 5.0;
                    let dot_y = y + 34.0 - i as f32 * 4.0 - (i % 2) as f32 * 3.0;
                    let size = (6 - i / 2).max(2) as f32;
                    cx.p.round(
                        dot_x,
                        dot_y,
                        size,
                        size,
                        (size / 2.0).floor(),
                        colors[i % colors.len()],
                    );
                }
            }
            Slot::Emote => cx.p.icon(Icon::Person, x - 12.0, y + 8.0, 24.0, th.text),
        }
    }
}

/// Up to four of what an outfit wears, small, in a row.
fn outfit_picture(cx: &mut Cx, x: f32, y: f32, loadout: Loadout) {
    let worn: Vec<&'static Cosmetic> = loadout.worn().take(4).collect();
    if worn.is_empty() {
        cx.p.icon(Icon::Hanger, x - 10.0, y + 12.0, 20.0, cx.p.theme.muted);
        return;
    }
    let each = 14.0;
    let start_x = x - ((worn.len() as f32 * (each + 2.0)) / 2.0).floor();
    for (i, c) in worn.iter().enumerate() {
        let cx0 = start_x + i as f32 * (each + 2.0);
        match c.slot {
            Slot::Cape => textures::draw_cape(cx.p, c, [cx0 + 2.0, y + 10.0, 10.0, 16.0]),
            Slot::Hat | Slot::Back => textures::draw_icon(cx.p, c, cx0, y + 12.0, each),
            _ => {
                let color = if c.slot == Slot::Nametag {
                    0xFF00_0000 | nametags::colors(c)[0]
                } else {
                    cx.p.theme.muted | 0xFF00_0000
                };
                cx.p.round(cx0 + 3.0, y + 15.0, 8.0, 8.0, 4.0, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_has_none_first_and_outfits_take_off_all() {
        let w = Wardrobe::showing(Some(Slot::Hat));
        let cards = w.cards(0);
        assert_eq!(cards[0], Card::None(Slot::Hat));
        assert_eq!(cards.len(), 7);
        let emotes = Wardrobe::showing(Some(Slot::Emote)).cards(0);
        assert!(
            matches!(emotes[0], Card::Cosmetic(_)),
            "emotes aren't worn: no None"
        );
        let outfits = Wardrobe::showing(None).cards(2);
        assert_eq!(
            outfits,
            [Card::TakeOffAll, Card::Outfit(0), Card::Outfit(1)]
        );
    }

    #[test]
    fn trying_on_a_card_changes_only_its_slot() {
        let on = Loadout::of(&["cape.frost", "hat.halo"]);
        let seen = Seen {
            worn: on,
            outfits: vec![("A".into(), Loadout::EMPTY)],
            shared: false,
            show_others: true,
        };
        let crown = catalog::by_id("hat.crown").unwrap();
        assert_eq!(
            try_on(Card::Cosmetic(crown), &seen).unwrap().ids(),
            ["cape.frost", "hat.crown"]
        );
        assert_eq!(
            try_on(Card::None(Slot::Cape), &seen).unwrap().ids(),
            ["hat.halo"]
        );
        assert_eq!(try_on(Card::TakeOffAll, &seen), Some(Loadout::EMPTY));
        assert!(try_on(Card::Cosmetic(catalog::by_id("emote.wave").unwrap()), &seen).is_none());
        assert!(worn(Card::None(Slot::Back), &seen) && !worn(Card::None(Slot::Hat), &seen));
    }
}
