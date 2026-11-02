//! The screenshot gallery (ScreenshotsScreen.java; ADR 0054, 0062): the game folder's
//! `screenshots/` pictures as a grid of thumbnails, newest first, oldest first or by name. A click
//! opens one large, with its name to change, buttons to delete, download (in the browser: a page
//! can't show its files) or copy it, and arrows to the next. Select mode picks several to download
//! as one zip.
//!
//! Thumbnails are made from the files as they scroll into view, one a frame: a Chromebook decodes
//! a full-size PNG in tens of milliseconds, and a frame mustn't take more than one. Each is kept
//! small (192 pixels wide) and painted into the canvas at the size it shows at.
use crate::canvas::{Image, Key};
use crate::draw::{Rect, contains};
use crate::host_extras::HostExtras;
use crate::input::{Key as InputKey, keys};
use crate::paint::Painter;
use crate::panels;
use crate::screen::{Cx, Go, Screen};
use crate::theme::{CONTROL_HEIGHT, GAP as THEME_GAP};
use crate::ui;
use std::sync::atomic::{AtomicU32, Ordering};

/// The folder under the game folder that Horizon's F2 writes to.
pub const FOLDER: &str = "screenshots";

/// A thumbnail's width in pixels: enough for the grid's cells at GUI scale 2, and the canvas holds
/// a screenful of them.
const THUMB_PIXELS: u32 = 192;
/// The largest the open picture is kept and painted, in pixels: its share of the 2048 by 1024
/// canvas, leaving room for the buttons' corners and the text.
const FULL_MAX: [u32; 2] = [1280, 720];

const CELL_WIDTH: f32 = 96.0;
const CELL_HEIGHT: f32 = 54.0;
const LABEL: f32 = 11.0;
const GAP: f32 = 8.0;
const TOP: f32 = 40.0;
const BOTTOM: f32 = 34.0;
/// The grid's buttons at their widest.
const FOOTER_BUTTON: f32 = 90.0;
/// The narrowest the viewer's name field gets before its row goes on two lines.
const MIN_NAME_FIELD: f32 = 80.0;
const NAME_FIELD: f32 = 140.0;
const RENAME: f32 = 60.0;

/// Ids for the pictures painted into the canvas, unique for the run, so a picture of an earlier
/// gallery (or one renamed or deleted) is never shown for another.
static NEXT_ID: AtomicU32 = AtomicU32::new(1);

fn next_id() -> u32 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// How the grid is ordered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    Newest,
    Oldest,
    Name,
}

impl Sort {
    fn label(self) -> &'static str {
        match self {
            Sort::Newest => "Newest first",
            Sort::Oldest => "Oldest first",
            Sort::Name => "By name",
        }
    }

    fn next(self) -> Self {
        match self {
            Sort::Newest => Sort::Oldest,
            Sort::Oldest => Sort::Name,
            Sort::Name => Sort::Newest,
        }
    }
}

/// One picture: its file and, once made, its thumbnail.
struct Shot {
    name: String,
    modified: f64,
    id: u32,
    thumb: Option<Image>,
    failed: bool,
    selected: bool,
}

impl Shot {
    fn base(&self) -> &str {
        &self.name[..self.name.len() - ".png".len()]
    }

    fn path(&self) -> String {
        format!("{FOLDER}/{}", self.name)
    }
}

/// The picture open large, at the size it's kept.
struct Full {
    id: u32,
    image: Image,
}

pub struct Screenshots {
    extras: Box<dyn HostExtras>,
    shots: Vec<Shot>,
    sort: Sort,
    selecting: bool,
    scroll: f32,
    /// The id of the picture open large, or None for the grid.
    viewing: Option<u32>,
    full: Option<Full>,
    /// The viewer's name field.
    name: String,
    status: Option<String>,
    /// Asking whether to delete the open picture (ConfirmScreen's question).
    confirm_delete: bool,
    /// The height the buttons take at the bottom: `BOTTOM`, more when they're on two rows.
    bottom: f32,
    /// Thumbnails made this frame.
    made: u32,
    /// Every picture painted into the canvas, forgotten when the gallery closes.
    painted: Vec<Key>,
}

const NAME_ID: &str = "screenshot-name";

impl Screenshots {
    pub fn new(extras: Box<dyn HostExtras>) -> Self {
        let mut s = Self {
            extras,
            shots: Vec::new(),
            sort: Sort::Newest,
            selecting: false,
            scroll: 0.0,
            viewing: None,
            full: None,
            name: String::new(),
            status: None,
            confirm_delete: false,
            bottom: BOTTOM,
            made: 0,
            painted: Vec::new(),
        };
        s.reload();
        s
    }

    fn reload(&mut self) {
        self.shots = self
            .extras
            .list_files(FOLDER)
            .into_iter()
            .filter(|f| f.name.to_lowercase().ends_with(".png") && f.name.len() > ".png".len())
            .map(|f| Shot {
                name: f.name,
                modified: f.modified_ms,
                id: next_id(),
                thumb: None,
                failed: false,
                selected: false,
            })
            .collect();
        self.order();
    }

    fn order(&mut self) {
        let sort = self.sort;
        self.shots.sort_by(|a, b| {
            let by_name = || a.name.to_lowercase().cmp(&b.name.to_lowercase());
            let by_time = || a.modified.total_cmp(&b.modified).then_with(by_name);
            match sort {
                Sort::Newest => by_time().reverse(),
                Sort::Oldest => by_time(),
                Sort::Name => by_name(),
            }
        });
    }

    fn index_of(&self, id: u32) -> Option<usize> {
        self.shots.iter().position(|s| s.id == id)
    }

    fn selected_count(&self) -> usize {
        self.shots.iter().filter(|s| s.selected).count()
    }

    fn open(&mut self, index: usize) {
        let Some(shot) = self.shots.get(index) else {
            return;
        };
        self.viewing = Some(shot.id);
        self.name = shot.base().to_owned();
        self.status = None;
        self.full = None;
    }

    fn close_viewer(&mut self) {
        self.viewing = None;
        self.status = None;
        self.full = None;
        self.confirm_delete = false;
    }

    fn step(&mut self, by: i32) {
        let Some(i) = self.viewing.and_then(|id| self.index_of(id)) else {
            return;
        };
        let n = self.shots.len() as i32;
        self.open((i as i32 + by).rem_euclid(n) as usize);
    }

    /// Makes `index`'s thumbnail if it has none, one a frame.
    fn thumb(&mut self, index: usize) {
        if self.made > 0 {
            return;
        }
        let path = {
            let shot = &self.shots[index];
            if shot.thumb.is_some() || shot.failed {
                return;
            }
            shot.path()
        };
        self.made += 1;
        let image = self
            .extras
            .read_file(&path)
            .and_then(|b| Image::decode_png(&b));
        let shot = &mut self.shots[index];
        match image {
            Some(image) => {
                let [w, h] = shrink([image.width, image.height], [THUMB_PIXELS, u32::MAX]);
                shot.thumb = Some(if [w, h] == [image.width, image.height] {
                    image
                } else {
                    image.scaled(w, h)
                });
            }
            None => shot.failed = true,
        }
    }

    fn load_full(&mut self, index: usize) {
        let path = self.shots[index].path();
        match self
            .extras
            .read_file(&path)
            .and_then(|b| Image::decode_png(&b))
        {
            Some(image) => {
                let [w, h] = shrink([image.width, image.height], FULL_MAX);
                let image = if [w, h] == [image.width, image.height] {
                    image
                } else {
                    image.scaled(w, h)
                };
                self.full = Some(Full {
                    id: next_id(),
                    image,
                });
            }
            None => self.shots[index].failed = true,
        }
    }

    fn rename(&mut self) {
        let Some(i) = self.viewing.and_then(|id| self.index_of(id)) else {
            return;
        };
        let wanted = clean_name(&self.name);
        if wanted.is_empty() {
            self.status = Some("A name needs a letter or a number".into());
            return;
        }
        let file = format!("{wanted}.png");
        if file == self.shots[i].name {
            return;
        }
        let target = format!("{FOLDER}/{file}");
        if self.extras.file_exists(&target) {
            self.status = Some(format!("There's already a screenshot called {wanted}"));
            return;
        }
        if self.extras.rename_file(&self.shots[i].path(), &target) {
            self.shots[i].name = file;
            self.name = wanted;
            self.status = Some("Renamed".into());
            self.order();
        } else {
            self.status = Some("Couldn't rename it".into());
        }
    }

    fn delete(&mut self, cx: &mut Cx) {
        let Some(i) = self.viewing.and_then(|id| self.index_of(id)) else {
            return;
        };
        if !self.extras.delete_file(&self.shots[i].path()) {
            self.status = Some("Couldn't delete it".into());
            return;
        }
        let shot = self.shots.remove(i);
        self.forget(cx, shot.id);
        if let Some(full) = self.full.take() {
            self.forget(cx, full.id);
        }
        self.status = None;
        if self.shots.is_empty() {
            self.close_viewer();
        } else {
            self.open(i.min(self.shots.len() - 1));
        }
    }

    fn download(&mut self, index: usize) {
        let shot = &self.shots[index];
        if self.extras.shows_folders() {
            let path = shot.path();
            self.extras.open_path(&path);
            return;
        }
        let name = shot.name.clone();
        self.status = Some(match self.extras.read_file(&shot.path()) {
            Some(bytes) => {
                if self.extras.download(&name, bytes) {
                    format!("Downloading {name}")
                } else {
                    "This platform can't download files".into()
                }
            }
            None => "Couldn't read it".into(),
        });
    }

    fn copy(&mut self, index: usize) {
        let copied = self
            .extras
            .read_file(&self.shots[index].path())
            .is_some_and(|png| self.extras.set_clipboard_image(&png));
        self.status = Some(
            if copied {
                "Copied to the clipboard"
            } else {
                "This platform can't copy pictures"
            }
            .into(),
        );
    }

    fn download_selected(&mut self) {
        let chosen: Vec<usize> = (0..self.shots.len())
            .filter(|&i| self.shots[i].selected)
            .collect();
        if chosen.is_empty() {
            return;
        }
        if chosen.len() == 1 || self.extras.shows_folders() {
            for i in chosen {
                self.download(i);
            }
            return;
        }
        let mut files = Vec::with_capacity(chosen.len());
        for &i in &chosen {
            let Some(bytes) = self.extras.read_file(&self.shots[i].path()) else {
                self.status = Some("Couldn't zip them".into());
                return;
            };
            files.push((self.shots[i].name.clone(), bytes));
        }
        let zip = stored_zip(&files);
        self.status = Some(if self.extras.download("screenshots.zip", zip) {
            format!("Downloading {} screenshots", chosen.len())
        } else {
            "This platform can't download files".into()
        });
    }

    fn forget(&mut self, cx: &mut Cx, id: u32) {
        self.painted.retain(|k| {
            let mine = key_id(*k) == Some(id % 0xFFFE);
            if mine {
                cx.p.canvas.forget(*k);
            }
            !mine
        });
    }

    fn columns(width: f32) -> usize {
        (((width - GAP * 2.0) / (CELL_WIDTH + GAP)).floor() as usize).max(1)
    }

    fn draw_grid(&mut self, cx: &mut Cx) {
        let [w, h] = cx.size;
        let th = cx.p.theme;
        self.bottom = BOTTOM;
        cx.p.centered("Screenshots", w / 2.0, 12.0, th.text, true);
        let n = self.shots.len();
        let caption = if n == 0 {
            "No screenshots yet. Press F2 in a world to take one.".to_owned()
        } else {
            format!(
                "{n}{}{}",
                if n == 1 {
                    " screenshot"
                } else {
                    " screenshots"
                },
                if self.selecting {
                    " - click to pick, then download"
                } else {
                    ""
                }
            )
        };
        let shown = cx.p.fit(&caption, w - 20.0).into_owned();
        cx.p.centered(&shown, w / 2.0, 24.0, th.muted, true);

        let columns = Self::columns(w);
        let grid_left = ((w - (columns as f32 * (CELL_WIDTH + GAP) - GAP)) / 2.0).floor();
        let bottom = h - self.bottom;
        let area = [0.0, TOP - 4.0, w, (bottom - (TOP - 4.0)).max(0.0)];
        // The wheel: rows of cells, 24 a step.
        let rows = n.div_ceil(columns) as f32;
        let content = rows * (CELL_HEIGHT + LABEL + GAP);
        let room = h - TOP - self.bottom;
        let steps = cx.ui.scrolled([0.0, 0.0, w, h]);
        self.scroll = (self.scroll - steps * 24.0).clamp(0.0, (content - room).max(0.0));

        cx.p.clip_start(area);
        let mut clicked = None;
        for i in 0..n {
            let x = grid_left + (i % columns) as f32 * (CELL_WIDTH + GAP);
            let y = TOP + (i / columns) as f32 * (CELL_HEIGHT + LABEL + GAP) - self.scroll.floor();
            if y + CELL_HEIGHT + LABEL < TOP - 4.0 || y > bottom {
                continue;
            }
            let cell = [x, y, CELL_WIDTH, CELL_HEIGHT];
            let hovered = cx.ui.over(cell) && contains(area, cx.ui.mouse);
            let frame = [
                x - 2.0,
                y - 2.0,
                CELL_WIDTH + 4.0,
                CELL_HEIGHT + LABEL + 4.0,
            ];
            panels::dark_panel(cx.p, frame[0], frame[1], frame[2], frame[3], 1.0);
            self.thumb(i);
            let shot = &self.shots[i];
            if let Some(thumb) = &shot.thumb {
                picture(cx.p, &mut self.painted, shot.id, thumb, cell);
            } else {
                let text = if shot.failed { "Can't read" } else { "..." };
                cx.p.centered(
                    text,
                    x + CELL_WIDTH / 2.0,
                    y + CELL_HEIGHT / 2.0 - 4.0,
                    th.muted,
                    true,
                );
            }
            if shot.selected {
                cx.p.outline(frame[0], frame[1], frame[2], frame[3], th.radius, th.accent);
                cx.p.round(x + CELL_WIDTH - 12.0, y + 2.0, 10.0, 10.0, 5.0, th.accent);
            } else if hovered {
                cx.p.outline(frame[0], frame[1], frame[2], frame[3], th.radius, th.text);
            }
            let label = cx.p.fit(shot.base(), CELL_WIDTH).into_owned();
            cx.p.text(
                &label,
                x,
                y + CELL_HEIGHT + 2.0,
                if hovered { th.text } else { th.muted },
                false,
            );
            // A press in the cell or its label, inside the list's area.
            let hit = crate::draw::intersect([x, y, CELL_WIDTH, CELL_HEIGHT + LABEL], area);
            if cx.ui.clicked(hit) {
                clicked = Some(i);
            }
        }
        cx.p.clip_end();
        if let Some(i) = clicked {
            if self.selecting {
                self.shots[i].selected = !self.shots[i].selected;
            } else {
                self.open(i);
            }
        }

        // The footer: sort, select, download or open the folder, done.
        let selected = self.selected_count();
        let download = format!("Download {selected}");
        let select = if self.selecting {
            "Done selecting"
        } else {
            "Select"
        };
        let done = cx.host.translate("gui.done");
        let mut labels: Vec<(&str, bool)> = vec![(self.sort.label(), true), (select, true)];
        if self.selecting {
            labels.push((&download, selected > 0));
        } else if self.extras.shows_folders() {
            labels.push(("Open folder", true));
        }
        labels.push((&done, true));
        let count = labels.len() as f32;
        // Narrower buttons where four at their widest don't fit (a small window at a large scale).
        let each = FOOTER_BUTTON.min(((w - GAP * 2.0 - (count - 1.0) * THEME_GAP) / count).floor());
        let rects = row(&vec![each; labels.len()], w, h - BOTTOM + 7.0);
        let mut pressed = None;
        for (i, ((label, active), r)) in labels.iter().zip(&rects).enumerate() {
            if ui::button(cx.p, cx.ui, *r, label, *active) {
                pressed = Some(i);
            }
        }
        match pressed {
            Some(0) => {
                self.sort = self.sort.next();
                self.order();
            }
            Some(1) => {
                self.selecting = !self.selecting;
                if !self.selecting {
                    self.shots.iter_mut().for_each(|s| s.selected = false);
                }
            }
            Some(2) if self.selecting => self.download_selected(),
            Some(2) if labels.len() == 4 => self.extras.open_path(FOLDER),
            Some(_) => cx.go(Go::Back),
            None => {}
        }
    }

    fn draw_viewer(&mut self, cx: &mut Cx, index: usize) {
        let [w, h] = cx.size;
        let th = cx.p.theme;
        let typing = cx.ui.focus == Some(ui::id(NAME_ID, 0));
        // Keys first, so the field doesn't take them: Enter renames, the arrows step.
        let enter =
            typing
                && cx.ui.keys.iter().any(|(k, _)| {
                    matches!(k, InputKey::Keyboard(keys::ENTER | keys::KEYPAD_ENTER))
                });
        if !typing && cx.ui.key(keys::LEFT) {
            self.step(-1);
            return;
        }
        if !typing && cx.ui.key(keys::RIGHT) {
            self.step(1);
            return;
        }
        let steps = cx.ui.scrolled([0.0, 0.0, w, h]);
        if steps != 0.0 {
            self.step(if steps > 0.0 { -1 } else { 1 });
            return;
        }

        // The buttons: the name and Rename, then Show or Download, Copy, Delete and Back; on two
        // rows when one is too narrow for the name.
        let show = if self.extras.shows_folders() {
            "Show"
        } else {
            "Download"
        };
        let back = cx.host.translate("gui.back");
        let actions = [
            (show, 70.0),
            ("Copy", 50.0),
            ("Delete", 55.0),
            (back.as_str(), 55.0),
        ];
        let room = w - GAP * 2.0;
        let others = RENAME + THEME_GAP + actions.iter().map(|a| a.1 + THEME_GAP).sum::<f32>();
        let y = h - BOTTOM + 7.0;
        let (field, rename, action_rects) = if room - others >= MIN_NAME_FIELD {
            self.bottom = BOTTOM;
            let mut widths = vec![NAME_FIELD.min(room - others), RENAME];
            widths.extend(actions.iter().map(|a| a.1));
            let r = row(&widths, w, y);
            (r[0], r[1], r[2..].to_vec())
        } else {
            self.bottom = BOTTOM + CONTROL_HEIGHT + THEME_GAP;
            let top = row(
                &[NAME_FIELD.min(room - RENAME - THEME_GAP).max(0.0), RENAME],
                w,
                y - CONTROL_HEIGHT - THEME_GAP,
            );
            let widths: Vec<f32> = actions.iter().map(|a| a.1).collect();
            (top[0], top[1], row(&widths, w, y))
        };

        let shot = &self.shots[index];
        let name = cx.p.fit(&shot.name, w - 20.0).into_owned();
        cx.p.centered(&name, w / 2.0, 12.0, th.text, true);
        let caption = format!("{} of {}", index + 1, self.shots.len());
        cx.p.centered(&caption, w / 2.0, 24.0, th.muted, true);
        if self.full.is_none() && !shot.failed {
            // Once: a picture that can't be read isn't read again every frame.
            self.load_full(index);
        }
        let area = [32.0, TOP, w - 64.0, h - TOP - self.bottom - 6.0];
        if let Some(full) = &self.full {
            picture(cx.p, &mut self.painted, full.id, &full.image, area);
        } else {
            cx.p.centered("Can't read this picture", w / 2.0, h / 2.0, th.muted, true);
        }

        ui::text_field(
            cx.p,
            cx.ui,
            ui::id(NAME_ID, 0),
            field,
            &mut self.name,
            "Name",
            120,
        );
        let mut do_rename = enter;
        if ui::button(cx.p, cx.ui, rename, "Rename", true) {
            do_rename = true;
        }
        let mut pressed = None;
        for (i, ((label, _), r)) in actions.iter().zip(&action_rects).enumerate() {
            if ui::button(cx.p, cx.ui, *r, label, true) {
                pressed = Some(i);
            }
        }
        let arrow_y = (TOP + (h - TOP - self.bottom) / 2.0 - 10.0).floor();
        if ui::button(cx.p, cx.ui, [6.0, arrow_y, 20.0, CONTROL_HEIGHT], "<", true) {
            self.step(-1);
            return;
        }
        if ui::button(
            cx.p,
            cx.ui,
            [w - 26.0, arrow_y, 20.0, CONTROL_HEIGHT],
            ">",
            true,
        ) {
            self.step(1);
            return;
        }
        if do_rename {
            self.rename();
        }
        match pressed {
            Some(0) => self.download(index),
            Some(1) => self.copy(index),
            Some(2) => self.confirm_delete = true,
            Some(3) => self.close_viewer(),
            _ => {}
        }
    }

    /// ConfirmScreen's question before a delete: the title at 70, the message under it, Yes and
    /// No side by side.
    fn draw_confirm(&mut self, cx: &mut Cx) {
        let [w, h] = cx.size;
        let th = cx.p.theme;
        let Some(i) = self.viewing.and_then(|id| self.index_of(id)) else {
            self.confirm_delete = false;
            return;
        };
        let title = format!("Delete {}?", self.shots[i].name);
        let title = cx.p.fit(&title, w - 20.0).into_owned();
        cx.p.centered(&title, w / 2.0, 70.0, th.text, true);
        let lines = cx.p.wrap("This can't be undone.", w - 50.0, 6);
        for (n, line) in lines.iter().enumerate() {
            cx.p.centered(line, w / 2.0, 90.0 + n as f32 * 9.0, th.text, true);
        }
        let y = (90.0 + lines.len() as f32 * 9.0 + 12.0)
            .clamp(h / 6.0 + 96.0, (h - 24.0).max(h / 6.0 + 96.0));
        let (yes, no) = (cx.host.translate("gui.yes"), cx.host.translate("gui.no"));
        if ui::button(
            cx.p,
            cx.ui,
            [w / 2.0 - 155.0, y, 150.0, CONTROL_HEIGHT],
            &yes,
            true,
        ) {
            self.confirm_delete = false;
            self.delete(cx);
        } else if ui::button(
            cx.p,
            cx.ui,
            [w / 2.0 + 5.0, y, 150.0, CONTROL_HEIGHT],
            &no,
            true,
        ) {
            self.confirm_delete = false;
        }
    }
}

impl Screen for Screenshots {
    fn draw(&mut self, cx: &mut Cx) {
        self.made = 0;
        if self.confirm_delete {
            self.draw_confirm(cx);
            return;
        }
        match self.viewing.and_then(|id| self.index_of(id)) {
            Some(i) => self.draw_viewer(cx, i),
            None => {
                self.viewing = None;
                self.draw_grid(cx);
            }
        }
        if let Some(note) = &self.status {
            let [w, h] = cx.size;
            let muted = cx.p.theme.muted;
            let note = cx.p.fit(note, w - 20.0).into_owned();
            cx.p.centered(&note, w / 2.0, h - self.bottom - 10.0, muted, true);
        }
    }

    fn escape(&mut self, cx: &mut Cx) {
        if self.confirm_delete {
            self.confirm_delete = false;
        } else if self.viewing.is_some() {
            self.close_viewer();
        } else {
            cx.go(Go::Back);
        }
    }

    fn closed(&mut self, cx: &mut Cx) {
        for key in self.painted.drain(..) {
            cx.p.canvas.forget(key);
        }
    }
}

/// `widths` side by side, `THEME_GAP` apart, centred across `width` at `y`.
fn row(widths: &[f32], width: f32, y: f32) -> Vec<Rect> {
    let total = widths.iter().sum::<f32>() + THEME_GAP * (widths.len().saturating_sub(1)) as f32;
    let mut x = ((width - total) / 2.0).floor();
    widths
        .iter()
        .map(|&w| {
            let r = [x, y, w, CONTROL_HEIGHT];
            x += w + THEME_GAP;
            r
        })
        .collect()
}

/// `size` made no larger than `max` either way, its shape kept (never larger than it is).
fn shrink([w, h]: [u32; 2], [mw, mh]: [u32; 2]) -> [u32; 2] {
    let scale = (mw as f64 / w as f64).min(mh as f64 / h as f64).min(1.0);
    [
        ((w as f64 * scale).round() as u32).max(1),
        ((h as f64 * scale).round() as u32).max(1),
    ]
}

/// Where a picture `size` shows as large as fits `area`, its shape kept, centred (`fit` in Java).
fn fit_rect(size: [u32; 2], area: Rect) -> Rect {
    let scale = (area[2] / size[0] as f32).min(area[3] / size[1] as f32);
    let (dw, dh) = (
        (size[0] as f32 * scale).round().max(1.0),
        (size[1] as f32 * scale).round().max(1.0),
    );
    [
        area[0] + ((area[2] - dw) / 2.0).floor(),
        area[1] + ((area[3] - dh) / 2.0).floor(),
        dw,
        dh,
    ]
}

/// The canvas key for picture `id` painted `width` pixels wide.
fn picture_key(id: u32, width: u32) -> Key {
    Key::Picture(id, width.min(u16::MAX as u32) as u16)
}

/// The picture id a key painted by `picture_key` is for.
fn key_id(key: Key) -> Option<u32> {
    match key {
        Key::Picture(id, _) => Some(id),
        _ => None,
    }
}

/// Draws `image` as large as fits `area`, painted into the canvas at the size it shows at in
/// screen pixels (no larger than it's kept).
fn picture(p: &mut Painter, painted: &mut Vec<Key>, id: u32, image: &Image, area: Rect) {
    let [x, y, dw, dh] = fit_rect([image.width, image.height], area);
    let px = ((dw * p.scale * p.zoom).round() as u32).clamp(1, image.width);
    let ph = ((image.height as f32 * px as f32 / image.width as f32).round() as u32).max(1);
    let key = picture_key(id, px);
    if !p.canvas.has(key) {
        painted.push(key);
    }
    let Some(region) = p.canvas.get(key, || {
        if px == image.width {
            image.clone()
        } else {
            image.scaled(px, ph)
        }
    }) else {
        return;
    };
    let dst = p.place(x, y, dw, dh);
    let argb = p.faded(0xFFFF_FFFF);
    p.list.image(region.src(), dst, argb);
}

/// What's left of `name` for a file: letters, digits, spaces, dots, dashes and underscores, no
/// `.png` at the end or dot at the start, at most 100 characters.
pub fn clean_name(name: &str) -> String {
    let clean: String = name
        .trim()
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'))
        .collect();
    let mut result = clean.trim();
    while result.to_lowercase().ends_with(".png") {
        result = &result[..result.len() - 4];
    }
    let result = result.trim_start_matches('.');
    result.chars().take(100).collect()
}

/// A zip of `files` (name, bytes), stored, not deflated: PNGs are compressed already.
pub fn stored_zip(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::with_capacity(
        files
            .iter()
            .map(|f| f.1.len() + f.0.len() * 2 + 100)
            .sum::<usize>()
            + 22,
    );
    let mut central = Vec::new();
    // 1980-01-01 00:00, the earliest a zip says.
    let (time, date) = (0u16, 0x21u16);
    for (name, data) in files {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let (size, name_len) = (data.len() as u32, name.len() as u16);
        // Bit 11: the name is UTF-8.
        let common = |v: &mut Vec<u8>| {
            v.extend_from_slice(&20u16.to_le_bytes());
            v.extend_from_slice(&0x0800u16.to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes());
            v.extend_from_slice(&time.to_le_bytes());
            v.extend_from_slice(&date.to_le_bytes());
            v.extend_from_slice(&crc.to_le_bytes());
            v.extend_from_slice(&size.to_le_bytes());
            v.extend_from_slice(&size.to_le_bytes());
            v.extend_from_slice(&name_len.to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes());
        };
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        common(&mut out);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        common(&mut central);
        central.extend_from_slice(&[0; 8]); // comment length, disk, internal and external attributes
        central.extend_from_slice(&[0; 2]);
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let (start, len) = (out.len() as u32, central.len() as u32);
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&start.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

const CRC_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
};

fn crc32(data: &[u8]) -> u32 {
    !data.iter().fold(!0u32, |c, &b| {
        CRC_TABLE[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_extras::FileEntry;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::rc::Rc;

    /// Path to bytes and modified time.
    type Files = BTreeMap<String, (Vec<u8>, f64)>;

    #[derive(Clone, Default)]
    struct Fake {
        files: Rc<RefCell<Files>>,
        downloads: Rc<RefCell<Vec<(String, usize)>>>,
    }

    impl HostExtras for Fake {
        fn list_files(&self, folder: &str) -> Vec<FileEntry> {
            let prefix = format!("{folder}/");
            self.files
                .borrow()
                .iter()
                .filter_map(|(k, v)| {
                    k.strip_prefix(&prefix).map(|n| FileEntry {
                        name: n.into(),
                        modified_ms: v.1,
                    })
                })
                .collect()
        }
        fn read_file(&self, path: &str) -> Option<Vec<u8>> {
            self.files.borrow().get(path).map(|v| v.0.clone())
        }
        fn file_exists(&self, path: &str) -> bool {
            self.files.borrow().contains_key(path)
        }
        fn rename_file(&mut self, from: &str, to: &str) -> bool {
            let mut f = self.files.borrow_mut();
            let Some(v) = f.remove(from) else {
                return false;
            };
            f.insert(to.into(), v);
            true
        }
        fn delete_file(&mut self, path: &str) -> bool {
            self.files.borrow_mut().remove(path);
            true
        }
        fn shows_folders(&self) -> bool {
            false
        }
        fn open_path(&mut self, _path: &str) {}
        fn download(&mut self, name: &str, data: Vec<u8>) -> bool {
            self.downloads.borrow_mut().push((name.into(), data.len()));
            true
        }
        fn set_clipboard_image(&mut self, _png: &[u8]) -> bool {
            false
        }
        fn open_link(&mut self, _url: &str) {}
    }

    fn gallery() -> (Screenshots, Fake) {
        let fake = Fake::default();
        for (name, t) in [
            ("b.png", 2.0),
            ("a.png", 3.0),
            ("c.PNG", 1.0),
            ("notes.txt", 9.0),
        ] {
            fake.files
                .borrow_mut()
                .insert(format!("screenshots/{name}"), (vec![1, 2, 3], t));
        }
        (Screenshots::new(Box::new(fake.clone())), fake)
    }

    fn names(s: &Screenshots) -> Vec<&str> {
        s.shots.iter().map(|s| s.name.as_str()).collect()
    }

    #[test]
    fn lists_pictures_newest_first_then_oldest_then_by_name() {
        let (mut s, _) = gallery();
        assert_eq!(names(&s), ["a.png", "b.png", "c.PNG"]);
        s.sort = s.sort.next();
        s.order();
        assert_eq!(names(&s), ["c.PNG", "b.png", "a.png"]);
        s.sort = s.sort.next();
        s.order();
        assert_eq!(names(&s), ["a.png", "b.png", "c.PNG"]);
        assert_eq!(s.sort.next(), Sort::Newest);
    }

    #[test]
    fn renames_with_a_clean_name_and_refuses_a_taken_one() {
        let (mut s, fake) = gallery();
        s.open(0);
        s.name = "b".into();
        s.rename();
        assert_eq!(
            s.status.as_deref(),
            Some("There's already a screenshot called b")
        );
        s.name = "  My  fight!.png ".into();
        s.rename();
        assert_eq!(s.status.as_deref(), Some("Renamed"));
        assert!(
            fake.files
                .borrow()
                .contains_key("screenshots/My  fight.png")
        );
        assert!(!fake.files.borrow().contains_key("screenshots/a.png"));
        s.name = "!!".into();
        s.rename();
        assert_eq!(
            s.status.as_deref(),
            Some("A name needs a letter or a number")
        );
    }

    #[test]
    fn steps_around_the_list() {
        let (mut s, _) = gallery();
        s.open(0);
        s.step(-1);
        assert_eq!(s.viewing.and_then(|id| s.index_of(id)), Some(2));
        s.step(1);
        assert_eq!(s.viewing.and_then(|id| s.index_of(id)), Some(0));
    }

    #[test]
    fn downloads_several_as_one_zip() {
        let (mut s, fake) = gallery();
        s.shots[0].selected = true;
        s.shots[2].selected = true;
        s.download_selected();
        assert_eq!(fake.downloads.borrow()[0].0, "screenshots.zip");
        assert_eq!(s.status.as_deref(), Some("Downloading 2 screenshots"));
    }

    #[test]
    fn cleans_names_as_java_does() {
        assert_eq!(
            clean_name("  2026-11-01_12.00.00.png "),
            "2026-11-01_12.00.00"
        );
        assert_eq!(clean_name("..hidden.PNG.png"), "hidden");
        assert_eq!(clean_name("a/b\\c:d"), "abcd");
        assert_eq!(clean_name(&"x".repeat(150)).len(), 100);
    }

    #[test]
    fn writes_a_stored_zip() {
        assert_eq!(crc32(b"hello"), 0x3610_A686);
        let zip = stored_zip(&[
            ("a.png".into(), b"hello".to_vec()),
            ("b.png".into(), vec![]),
        ]);
        assert_eq!(&zip[..4], &[0x50, 0x4B, 0x03, 0x04]);
        let end = zip.len() - 22;
        assert_eq!(&zip[end..end + 4], &[0x50, 0x4B, 0x05, 0x06]);
        assert_eq!(u16::from_le_bytes([zip[end + 10], zip[end + 11]]), 2);
        let start = u32::from_le_bytes(zip[end + 16..end + 20].try_into().unwrap()) as usize;
        assert_eq!(&zip[start..start + 4], &[0x50, 0x4B, 0x01, 0x02]);
        // The first entry's data sits after its 30-byte header and name.
        assert_eq!(&zip[35..40], b"hello");
    }

    #[test]
    fn fits_pictures_keeping_their_shape() {
        assert_eq!(shrink([1920, 1080], [192, u32::MAX]), [192, 108]);
        assert_eq!(shrink([1920, 1080], FULL_MAX), [1280, 720]);
        assert_eq!(shrink([100, 50], FULL_MAX), [100, 50]);
        assert_eq!(
            fit_rect([192, 108], [0.0, 0.0, 96.0, 96.0]),
            [0.0, 21.0, 96.0, 54.0]
        );
        assert_eq!(key_id(picture_key(7, 96)), Some(7));
    }
}
