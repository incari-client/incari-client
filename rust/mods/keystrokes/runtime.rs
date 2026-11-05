// Incari's Keystrokes for the Rust build (ADR 0068, 0069), as features/keystrokes does it: the
// movement keys, the mouse buttons with their clicks a second, sneak and sprint, and the space bar,
// as the player bound them, lit while held. Its settings and HUD element are the module's in
// `incari_core::modules` (the Java ids). No game types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "keystrokes";
/// Its HUD element's id.
pub const ELEMENT: &str = "keys";

/// The space between keys.
pub const GAP: i32 = 2;
/// A row of sneak and sprint, and the space bar, are this much of a key's height.
const SHORT_ROW_PERCENT: i32 = 60;
/// A line of text's height (`incari_core::hud::LINE`).
pub const LINE: i32 = 9;

/// The vanilla mappings a key can show, in `Cell::key`'s order.
pub const MAPPINGS: [&str; 9] = [
    "key.forward",
    "key.left",
    "key.back",
    "key.right",
    "key.attack",
    "key.use",
    "key.sneak",
    "key.sprint",
    "key.jump",
];

/// What a key shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    /// Its key's name, as Controls shows it.
    Name,
    /// Its name over its clicks a second: 0 attack, 1 use.
    Clicks(usize),
    /// A bar, not the key's name: "Space" doesn't fit a short row at small sizes.
    Bar,
}

/// A key: which of `MAPPINGS` it shows, where (from the element's top left) and what it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub key: usize,
    pub rect: [i32; 4],
    pub face: Face,
}

pub const MAX_CELLS: usize = MAPPINGS.len();

/// What the settings ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shown {
    pub size: i32,
    pub mouse: bool,
    pub clicks: bool,
    pub sneak_sprint: bool,
    pub space: bool,
}

/// The keys and the element's size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub cells: [Cell; MAX_CELLS],
    pub len: usize,
    pub size: [i32; 2],
}

impl Layout {
    pub fn cells(&self) -> &[Cell] {
        &self.cells[..self.len]
    }
}

/// The short rows' height.
pub fn short_row(size: i32) -> i32 {
    (size * SHORT_ROW_PERCENT / 100).max(8)
}

/// The keys, as the Java module lays them out: W over A S D, the mouse buttons (tall enough for
/// two lines when they write clicks a second), sneak and sprint, then the space bar.
pub fn layout(shown: Shown) -> Layout {
    let s = shown.size;
    let width = s * 3 + GAP * 2;
    let half = (width - GAP) / 2;
    let short = short_row(s);
    let mut cells = [Cell {
        key: 0,
        rect: [0; 4],
        face: Face::Name,
    }; MAX_CELLS];
    let mut len = 0;
    let mut push = |key, rect, face| {
        cells[len] = Cell { key, rect, face };
        len += 1;
    };
    push(0, [s + GAP, 0, s, s], Face::Name);
    let mut y = s + GAP;
    push(1, [0, y, s, s], Face::Name);
    push(2, [s + GAP, y, s, s], Face::Name);
    push(3, [(s + GAP) * 2, y, s, s], Face::Name);
    y += s + GAP;
    if shown.mouse {
        let (h, left, right) = if shown.clicks {
            (s.max(LINE * 2 + 3), Face::Clicks(0), Face::Clicks(1))
        } else {
            (s, Face::Name, Face::Name)
        };
        push(4, [0, y, half, h], left);
        push(5, [half + GAP, y, width - half - GAP, h], right);
        y += h + GAP;
    }
    if shown.sneak_sprint {
        push(6, [0, y, half, short], Face::Name);
        push(7, [half + GAP, y, width - half - GAP, short], Face::Name);
        y += short + GAP;
    }
    if shown.space {
        push(8, [0, y, width, short], Face::Bar);
        y += short + GAP;
    }
    Layout {
        cells,
        len,
        size: [width, y - GAP],
    }
}

/// Where a key's label (and the line below it, when it has one) starts down: centred in the key.
pub fn label_y(rect: [i32; 4], two_lines: bool) -> i32 {
    let text = if two_lines { LINE * 2 - 1 } else { LINE - 1 };
    rect[1] + (rect[3] - text + 1) / 2
}

/// The space bar's line: a third of the bar wide, centred, half way down.
pub fn bar(rect: [i32; 4]) -> [i32; 4] {
    let [x, y, w, h] = rect;
    let width = w / 3;
    [x + (w - width) / 2, y + h / 2, width, 1]
}

// Hot path, don't allocate: each button's "12 CPS", made again only when its count changes.
thread_local! {
    static CLICKS: std::cell::RefCell<[(u32, String); 2]> =
        const { std::cell::RefCell::new([(u32::MAX, String::new()), (u32::MAX, String::new())]) };
}

/// Calls `f` with "12 CPS" for button `button` (0 attack, 1 use) at `count`.
pub fn with_clicks<R>(button: usize, count: u32, f: impl FnOnce(&str) -> R) -> R {
    CLICKS.with(|cell| {
        let mut cached = cell.borrow_mut();
        let entry = &mut cached[button.min(1)];
        if entry.0 != count {
            *entry = (count, format!("{count} CPS"));
        }
        f(&entry.1)
    })
}

/// The count alone of a "12 CPS": what a button too narrow for the whole shows.
pub fn count_only(clicks: &str) -> &str {
    clicks.split(' ').next().unwrap_or(clicks)
}

/// How much to shrink text `width` wide to fit a key `room` wide: 1 if it fits (never grows it).
pub fn fit_scale(width: f32, room: f32) -> f32 {
    if width <= room || room <= 0.0 {
        1.0
    } else {
        room / width
    }
}

/// "LMB" for "Left Button": the mouse's names are too long for a key's box (ModuleGame.shortName).
pub fn short_name(name: &str) -> Option<String> {
    Some(
        match name {
            "Left Button" => "LMB",
            "Right Button" => "RMB",
            "Middle Button" => "MMB",
            "Left Shift" => "Shift",
            "Left Control" => "Ctrl",
            "Left Alt" => "Alt",
            _ => return None,
        }
        .to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: Shown = Shown {
        size: 20,
        mouse: true,
        clicks: true,
        sneak_sprint: true,
        space: true,
    };

    #[test]
    fn lays_the_keys_out_as_the_java_module_does() {
        let all = layout(ALL);
        assert_eq!(all.len, 9);
        assert_eq!(all.size[0], 64);
        assert_eq!(all.cells[0].rect, [22, 0, 20, 20]);
        assert_eq!(all.cells[4].face, Face::Clicks(0));
        assert_eq!(all.cells[4].rect, [0, 44, 31, 21]);
        assert_eq!(all.cells[5].rect, [33, 44, 31, 21]);
        // Movement, mouse (two lines and 3 high), sneak and sprint, space.
        assert_eq!(all.size[1], 20 + 2 + 20 + 2 + 21 + 2 + 12 + 2 + 12);
        let keys = layout(Shown {
            size: 14,
            mouse: false,
            sneak_sprint: false,
            space: false,
            ..ALL
        });
        assert_eq!(keys.cells().len(), 4);
        assert_eq!(keys.size, [46, 30]);
        let mouse = layout(Shown {
            size: 30,
            clicks: false,
            sneak_sprint: false,
            ..ALL
        });
        assert_eq!(mouse.cells()[4].rect[3], 30);
        assert_eq!(mouse.cells()[6].face, Face::Bar);
        assert_eq!(MAPPINGS[mouse.cells()[6].key], "key.jump");
        assert_eq!(short_row(14), 8);
    }

    #[test]
    fn centres_labels_as_the_java_module_does() {
        assert_eq!(label_y([0, 0, 20, 20], false), 6);
        assert_eq!(label_y([0, 44, 31, 21], true), 46);
        assert_eq!(bar([0, 90, 64, 12]), [21, 96, 21, 1]);
    }

    #[test]
    fn text_too_wide_drops_its_unit_then_shrinks() {
        assert_eq!(count_only("12 CPS"), "12");
        assert_eq!(fit_scale(30.0, 31.0), 1.0);
        assert_eq!(fit_scale(40.0, 20.0), 0.5);
    }

    #[test]
    fn counts_each_button_apart() {
        assert_eq!(with_clicks(0, 12, str::to_owned), "12 CPS");
        assert_eq!(with_clicks(1, 3, str::to_owned), "3 CPS");
        let first = with_clicks(0, 12, |t| t.as_ptr());
        assert_eq!(with_clicks(0, 12, |t| t.as_ptr()), first);
    }
}
