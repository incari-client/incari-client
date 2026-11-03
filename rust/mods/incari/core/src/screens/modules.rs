//! The module menu (ModuleListScreen.java): one glass panel, categories down the left with their
//! counts, the theme, GUI preset and profile below them, a search field over a grid of module
//! cards, and Cosmetics and Edit HUD beside the search. A click on a card turns its module on or
//! off; a click on its gear opens its settings. Opened by Right Shift, Incari Settings or the
//! pause menu.
use crate::framework::module::{CATEGORIES, Category};
use crate::icons::Icon;
use crate::panels::{self, SWITCH_WIDTH};
use crate::screen::{Cx, Go, Screen};
use crate::theme::{CONTROL_HEIGHT, GAP, Preset};
use crate::ui;

const SIDEBAR: f32 = 98.0;
const PADDING: f32 = 10.0;
const ENTRY: f32 = 16.0;
const CARD_HEIGHT: f32 = 46.0;
const CARD_GAP: f32 = 6.0;
const SCROLL_STEP: f32 = 24.0;
/// The narrowest the search field gets before Edit HUD and Cosmetics lose their words.
const MIN_SEARCH: f32 = 80.0;
/// An icon-only menu button: the icon and its margins.
const ICON_BUTTON: f32 = 25.0;
const GEAR: f32 = 8.0;

#[derive(Default)]
pub struct ModuleList {
    category: Option<Category>,
    query: String,
    scroll: f32,
}

impl Screen for ModuleList {
    fn draw(&mut self, cx: &mut Cx) {
        let [width, height] = cx.size;
        let th = cx.p.theme;
        let panel_w = (width - 24.0).min(480.0);
        let panel_h = (height - 20.0).min(296.0);
        let px = ((width - panel_w) / 2.0).floor();
        let py = ((height - panel_h) / 2.0).floor();
        panels::panel(cx.p, px, py, panel_w, panel_h, 1.0);

        // The sidebar's mark, and the line between it and the grid.
        cx.p.logo(px + 11.0, py + 11.0, 12.0, 1.0);
        let mut lx = px + 29.0;
        for letter in ["I", "N", "C", "A", "R", "I"] {
            cx.p.text(letter, lx, py + 14.0, th.text, false);
            lx += cx.p.width(letter) + 2.0;
        }
        cx.p.fill(px + SIDEBAR, py + 8.0, 1.0, panel_h - 16.0, 0x1AFF_FFFF);

        // Categories with their counts.
        let listed: Vec<usize> = (0..cx.modules.modules.len())
            .filter(|&i| !cx.modules.modules[i].always_on)
            .collect();
        let count = |c: Category, cx: &Cx| {
            listed
                .iter()
                .filter(|&&i| cx.modules.modules[i].category == c)
                .count()
        };
        let x = px + 8.0;
        let entry_w = SIDEBAR - 16.0;
        let mut y = py + 34.0;
        let bottom = py + panel_h - 8.0 - CONTROL_HEIGHT;
        let used: Vec<Category> = CATEGORIES
            .into_iter()
            .filter(|c| count(*c, cx) > 0)
            .collect();
        // On a short screen the entries close up so they don't run into the lines below.
        let step = if y + used.len() as f32 * (ENTRY + 2.0) + ENTRY + 2.0
            <= bottom - 3.0 * (ENTRY + 2.0)
        {
            ENTRY + 2.0
        } else {
            ENTRY
        };
        let all = listed.len().to_string();
        if ui::entry(
            cx.p,
            cx.ui,
            [x, y, entry_w, ENTRY],
            "All",
            &all,
            self.category.is_none(),
        ) {
            self.choose(None);
        }
        for c in used {
            y += step;
            let n = count(c, cx).to_string();
            if ui::entry(
                cx.p,
                cx.ui,
                [x, y, entry_w, ENTRY],
                c.label(),
                &n,
                self.category == Some(c),
            ) {
                self.choose(Some(c));
            }
        }
        // The theme is the client's, not a module: a line here, like the GUI preset.
        if let Some(theme) = cx.modules.module("theme") {
            let preset = theme
                .settings
                .get("preset")
                .map(|s| s.label())
                .unwrap_or_default();
            if ui::entry(
                cx.p,
                cx.ui,
                [x, bottom - 3.0 * step, entry_w, ENTRY],
                "Theme",
                &preset,
                false,
            ) {
                cx.open(super::module_settings::ModuleSettings::new("theme"));
            }
        }
        let gui = cx.prefs.preset.label();
        if ui::entry(
            cx.p,
            cx.ui,
            [x, bottom - 2.0 * step, entry_w, ENTRY],
            "GUI",
            gui,
            false,
        ) {
            cx.prefs.preset = if cx.prefs.preset == Preset::Normal {
                Preset::Potato
            } else {
                Preset::Normal
            };
            cx.prefs.dirty = true;
        }
        let profile = cx.modules.active_profile().to_owned();
        if ui::entry(
            cx.p,
            cx.ui,
            [x, bottom - step, entry_w, ENTRY],
            "Profile",
            &profile,
            false,
        ) {
            cx.open(super::profiles::Profiles::default());
        }
        let done = cx.host.translate("gui.done");
        if ui::button(
            cx.p,
            cx.ui,
            [x, bottom, entry_w, CONTROL_HEIGHT],
            &done,
            true,
        ) {
            cx.go(Go::Back);
        }

        // Edit HUD and Cosmetics beside the search; only their icons where their words would
        // leave the search too narrow to type in.
        let grid_x = px + SIDEBAR + PADDING;
        let grid_w = px + panel_w - PADDING - grid_x;
        let mut edit_w = 24.0 + cx.p.width("Edit HUD") + 10.0;
        let mut cosmetics_w = 24.0 + cx.p.width("Cosmetics") + 10.0;
        let icons_only = grid_w - edit_w - cosmetics_w - 2.0 * GAP < MIN_SEARCH;
        if icons_only {
            edit_w = ICON_BUTTON;
            cosmetics_w = ICON_BUTTON;
        }
        let edit_r = [
            grid_x + grid_w - edit_w,
            py + PADDING,
            edit_w,
            CONTROL_HEIGHT,
        ];
        let label = if icons_only { "" } else { "Edit HUD" };
        if ui::menu_button(cx.p, cx.ui, edit_r, label, Some(Icon::Layout), "", true) {
            cx.open(super::hud_editor::HudEditor::default());
        }
        if icons_only && cx.ui.over(edit_r) {
            cx.ui.tooltip("Edit HUD");
        }
        let cosmetics_r = [
            edit_r[0] - GAP - cosmetics_w,
            py + PADDING,
            cosmetics_w,
            CONTROL_HEIGHT,
        ];
        let label = if icons_only { "" } else { "Cosmetics" };
        if ui::menu_button(
            cx.p,
            cx.ui,
            cosmetics_r,
            label,
            Some(Icon::Hanger),
            "",
            true,
        ) {
            cx.open(super::wardrobe::Wardrobe::new());
        }
        if icons_only && cx.ui.over(cosmetics_r) {
            cx.ui.tooltip("Cosmetics");
        }
        let search_r = [
            grid_x,
            py + PADDING,
            grid_w - edit_w - cosmetics_w - 2.0 * GAP,
            CONTROL_HEIGHT,
        ];
        if ui::text_field(
            cx.p,
            cx.ui,
            ui::id("search", 0),
            search_r,
            &mut self.query,
            "Search modules",
            64,
        ) {
            self.scroll = 0.0;
        }

        // The cards that match the category and the search, in rows, scrolled.
        let grid_y = py + PADDING + CONTROL_HEIGHT + PADDING;
        let grid_h = py + panel_h - PADDING - grid_y;
        let columns = if grid_w >= 420.0 {
            3.0
        } else if grid_w >= 260.0 {
            2.0
        } else {
            1.0
        };
        let card_w = ((grid_w - (columns - 1.0) * CARD_GAP) / columns).floor();
        let needle = self.query.trim().to_lowercase();
        let shown: Vec<usize> = listed
            .into_iter()
            .filter(|&i| {
                let m = &cx.modules.modules[i];
                self.category.is_none_or(|c| m.category == c)
                    && (needle.is_empty()
                        || m.name.to_lowercase().contains(&needle)
                        || m.description.to_lowercase().contains(&needle))
            })
            .collect();
        let rows = (shown.len() as f32 / columns).ceil();
        let content = (rows * (CARD_HEIGHT + CARD_GAP) - CARD_GAP).max(0.0);
        let area = [grid_x, grid_y, grid_w, grid_h];
        ui::scroll(cx.ui, area, &mut self.scroll, content, SCROLL_STEP);
        cx.p.clip_start([grid_x - 6.0, grid_y, grid_w + 12.0, grid_h]);
        for (n, &i) in shown.iter().enumerate() {
            let col = (n as f32 % columns).floor();
            let row = (n as f32 / columns).floor();
            let r = [
                grid_x + col * (card_w + CARD_GAP),
                grid_y + row * (CARD_HEIGHT + CARD_GAP) - self.scroll.floor(),
                card_w,
                CARD_HEIGHT,
            ];
            if r[1] + r[3] <= grid_y || r[1] >= grid_y + grid_h {
                continue;
            }
            self.card(cx, i, r, area);
        }
        cx.p.clip_end();
        if shown.is_empty() {
            let text = format!("No modules match \"{}\"", self.query.trim());
            cx.p.centered(&text, grid_x + grid_w / 2.0, grid_y + 24.0, th.muted, false);
        }
        ui::scrollbar(cx.p, area, content, self.scroll);
    }

    fn closed(&mut self, cx: &mut Cx) {
        let _ = cx;
    }
}

impl ModuleList {
    fn choose(&mut self, category: Option<Category>) {
        self.category = category;
        self.scroll = 0.0;
    }

    /// Module `i`'s card at `r`, inside the grid `area`.
    fn card(&mut self, cx: &mut Cx, i: usize, r: [f32; 4], area: [f32; 4]) {
        let th = cx.p.theme;
        let m = &cx.modules.modules[i];
        let (id, on, has_settings) = (m.id, m.enabled, m.has_settings());
        let (name, description) = (m.name, m.description);
        let visible = crate::draw::intersect(r, area);
        let hot = cx.ui.over(visible);
        panels::card(cx.p, r[0], r[1], r[2], r[3], hot, on, 1.0);
        // The name stops short of the switch; cut, it's shown whole under the pointer.
        let title =
            cx.p.fit(name, r[2] - 16.0 - SWITCH_WIDTH - 4.0)
                .into_owned();
        if hot && title != name {
            cx.ui.tooltip(name);
        }
        cx.p.text(&title, r[0] + 8.0, r[1] + 8.0, th.text, false);
        panels::toggle(cx.p, r[0] + r[2] - 8.0 - SWITCH_WIDTH, r[1] + 7.0, on, 1.0);
        let gear_room = if has_settings { GEAR + 4.0 } else { 0.0 };
        let lines = cx.p.wrap(description, r[2] - 16.0 - gear_room, 2);
        for (n, line) in lines.iter().enumerate() {
            cx.p.text(
                line,
                r[0] + 8.0,
                r[1] + 21.0 + n as f32 * 10.0,
                th.muted,
                false,
            );
        }
        let gear = [
            r[0] + r[2] - 8.0 - GEAR - 3.0,
            r[1] + r[3] - 8.0 - GEAR - 3.0,
            GEAR + 11.0,
            GEAR + 11.0,
        ];
        if has_settings {
            let over_gear = hot && cx.ui.over(gear);
            let color = if over_gear {
                th.accent
            } else {
                crate::color::alpha(th.muted, 0.8)
            };
            cx.p.icon(
                Icon::Gear,
                r[0] + r[2] - 8.0 - GEAR,
                r[1] + r[3] - 8.0 - GEAR,
                GEAR,
                color,
            );
            if cx.ui.clicked(crate::draw::intersect(gear, area)) {
                cx.open(super::module_settings::ModuleSettings::new(id));
                return;
            }
        }
        if cx.ui.clicked(visible) {
            cx.modules.toggle(id);
        }
    }
}
