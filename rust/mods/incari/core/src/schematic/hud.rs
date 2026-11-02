//! The module's HUD elements (StatusPanel, MaterialPanel): **Schematic**, the loaded schematic, how
//! far it's built and how it's placed in a few lines; **Material list**, the items still needed,
//! most first, each with its icon and how many, green when the player carries enough, amber when
//! not. Their lines are made again only when the session says something changed
//! (`hud_revision`), not each frame. Both show a sample in the HUD editor with nothing loaded.
use super::Mirror;
use super::session::{Layers, Look, Session};
use crate::hud::{HudCanvas, LINE, PAD_X, PAD_Y};
use crate::theme::Tone;
use std::cell::RefCell;

/// The widest a status line may be before it's cut short.
pub const STATUS_WIDTH: f32 = 170.0;
const ICON: f32 = 16.0;
const GAP: f32 = 3.0;
const NAME_WIDTH: f32 = 110.0;

/// What a frame's drawing needs of the game: an item's name in the player's language, and how many
/// of it the player carries (None outside a world).
pub struct Items<'a> {
    pub name: &'a dyn Fn(&str) -> String,
    pub carried: &'a dyn Fn(&str) -> Option<u32>,
}

/// The lines as last made, and what they were made from.
#[derive(Default)]
struct Cache {
    made: Option<(u64, bool, i32, Look, i32)>,
    status: Vec<(String, Tone)>,
    header: String,
    rows: Vec<Row>,
    count_width: f32,
}

struct Row {
    id: String,
    name: String,
    count: String,
    needed: i64,
}

thread_local! {
    static STATUS: RefCell<Cache> = RefCell::new(Cache::default());
    static MATERIALS: RefCell<Cache> = RefCell::new(Cache::default());
}

/// "Castle..." when `text` is wider than `width`.
fn fit(canvas: &HudCanvas, text: &str, width: f32) -> String {
    canvas.p.fit(text, width).into_owned()
}

/// The **Schematic** element: its size, None to hide it.
pub fn status(canvas: &mut HudCanvas, s: &Session, look: &Look) -> Option<[f32; 2]> {
    STATUS.with(|cell| {
        let mut c = cell.borrow_mut();
        let editing = canvas.editing && !s.is_loaded();
        let key = (s.hud_revision, editing, 0, *look, s.layer());
        if c.made != Some(key) {
            c.made = Some(key);
            c.status = status_lines(canvas, s, look, editing);
        }
        if c.status.is_empty() {
            return None;
        }
        let width = c.status.iter().map(|(l, _)| canvas.text_width(l)).fold(0.0, f32::max) + PAD_X * 2.0;
        let height = c.status.len() as f32 * LINE + PAD_Y * 2.0 - 1.0;
        canvas.fill(0.0, 0.0, width, height, Tone::Background);
        for (i, (line, tone)) in c.status.iter().enumerate() {
            canvas.text(line, PAD_X, PAD_Y + i as f32 * LINE, *tone);
        }
        Some([width, height])
    })
}

fn status_lines(canvas: &HudCanvas, s: &Session, look: &Look, editing: bool) -> Vec<(String, Tone)> {
    let Some(now) = s.loaded() else {
        if editing {
            return vec![
                ("Castle".into(), Tone::Accent),
                ("1200 of 4000 placed".into(), Tone::Text),
                ("3 wrong".into(), Tone::Warning),
            ];
        }
        return Vec::new();
    };
    let mut lines = vec![(fit(canvas, &now.schematic.name, STATUS_WIDTH), Tone::Accent)];
    let v = &now.verifier;
    let total = now.schematic.block_count();
    lines.push((format!("{} of {total} placed", v.right().min(total)), Tone::Text));
    let wrong = v.wrong_blocks() + v.wrong_states();
    if wrong > 0 || v.extra() > 0 {
        let problems = match (wrong, v.extra()) {
            (w, 0) => format!("{w} wrong"),
            (0, e) => format!("{e} extra"),
            (w, e) => format!("{w} wrong, {e} extra"),
        };
        lines.push((problems, Tone::Warning));
    } else if v.looked_at_all() && v.right() >= total {
        lines.push(("Finished".into(), Tone::Positive));
    }
    let p = &now.placement;
    let [x, y, z] = p.min();
    let mut placed = format!("{x} {y} {z}");
    if p.turns() != 0 {
        placed.push_str(&format!(", turned {}°", u32::from(p.turns()) * 90));
    }
    if p.mirror() != Mirror::None {
        placed.push_str(", mirrored");
    }
    lines.push((placed, Tone::Muted));
    match look.layers {
        Layers::All => {}
        Layers::One => lines.push((format!("Layer {}", s.layer()), Tone::Muted)),
        Layers::Below => lines.push((format!("Up to layer {}", s.layer()), Tone::Muted)),
    }
    lines
}

/// The **Material list** element: its size, None to hide it. `rows` is its own setting.
pub fn materials(canvas: &mut HudCanvas, s: &Session, items: &Items) -> Option<[f32; 2]> {
    MATERIALS.with(|cell| {
        let mut c = cell.borrow_mut();
        let editing = canvas.editing && !s.is_loaded();
        let rows = canvas.element.int("rows").clamp(3, 20);
        let key = (s.hud_revision, editing, rows, Look::default(), 0);
        if c.made != Some(key) {
            c.made = Some(key);
            build_rows(&mut c, canvas, s, items, editing, rows as usize);
        }
        if c.header.is_empty() {
            return None;
        }
        let mut width = canvas.text_width(&c.header);
        for row in &c.rows {
            width = width.max(ICON + GAP + canvas.text_width(&row.name) + GAP * 2.0 + c.count_width);
        }
        let width = width + PAD_X * 2.0;
        let height = PAD_Y * 2.0 + LINE + c.rows.len() as f32 * (ICON + 1.0);
        canvas.fill(0.0, 0.0, width, height, Tone::Background);
        canvas.text(&c.header, PAD_X, PAD_Y, Tone::Accent);
        let mut y = PAD_Y + LINE;
        let text_y = ((ICON - LINE) / 2.0).floor() + 1.0;
        for row in &c.rows {
            canvas.item(&row.id, PAD_X, y);
            canvas.text(&row.name, PAD_X + ICON + GAP, y + text_y, Tone::Text);
            let count_x = width - PAD_X - canvas.text_width(&row.count);
            let tone = match (items.carried)(&row.id) {
                None => Tone::Text,
                Some(n) if i64::from(n) >= row.needed => Tone::Positive,
                Some(_) => Tone::Warning,
            };
            canvas.text(&row.count, count_x, y + text_y, tone);
            y += ICON + 1.0;
        }
        Some([width, height])
    })
}

fn build_rows(c: &mut Cache, canvas: &HudCanvas, s: &Session, items: &Items, editing: bool, max: usize) {
    c.rows.clear();
    c.count_width = 0.0;
    let add = |c: &mut Cache, id: &str, needed: i64| {
        let name = (items.name)(id);
        let row = Row {
            id: id.to_owned(),
            name: fit(canvas, if name.is_empty() { id } else { &name }, NAME_WIDTH),
            count: needed.to_string(),
            needed,
        };
        c.count_width = c.count_width.max(canvas.text_width(&row.count));
        c.rows.push(row);
    };
    let Some(now) = s.loaded() else {
        c.header = if editing { "Materials: 2 left".into() } else { String::new() };
        if editing {
            add(c, "minecraft:stone_bricks", 320);
            add(c, "minecraft:oak_planks", 64);
        }
        return;
    };
    let m = &now.materials;
    c.header = match m.kinds_needed() {
        0 => "Materials: none left".into(),
        n => format!("Materials: {n} left"),
    };
    for rank in 0..m.kinds() {
        if c.rows.len() >= max {
            break;
        }
        let item = m.ranked(rank);
        let needed = m.needed(item);
        if needed <= 0 {
            break;
        }
        add(c, m.id(item), needed);
    }
}
