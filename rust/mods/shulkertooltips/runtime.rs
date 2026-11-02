// Incari's Shulker Tooltips for the Rust build (ADR 0068, 0069), as features/shulkertooltips does
// it: a shulker box's tooltip shows its contents as a grid, nine across, on the box's colour, in
// place of the game's first five lines of names. Its settings are the module's in
// `incari_core::modules` (the Java ids). No game types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "shulkertooltips";

/// A shulker box's 27 slots, 9 a row, a slot 18 GUI pixels.
pub const COLUMNS: usize = 9;
pub const ROWS: usize = 3;
pub const CELL: f32 = 18.0;
pub const SIZE: [f32; 2] = [COLUMNS as f32 * CELL, ROWS as f32 * CELL];
/// An empty slot's square, as a container screen's.
pub const SLOT: u32 = 0x60_8B8B8B;

/// The game's own lines' formats when the language has none (`item.container.item_count`,
/// `item.container.more_items`).
pub const COUNT_FORMAT: &str = "%s x %s";
pub const MORE_FORMAT: &str = "and %s more...";
/// How many stacks the game names before "and 3 more...".
pub const NAMED: usize = 5;

/// Whether the grid shows for a box holding something: always, or with Shift held when
/// `only_with_shift`.
pub fn shows(only_with_shift: bool, shift: bool) -> bool {
    !only_with_shift || shift
}

/// `format` split at its two `%s`: what goes before the first, between them and after. None for a
/// format this can't read (positional `%1$s`), whose lines are then left alone.
fn parts(format: &str) -> Option<(&str, &str, &str)> {
    if format.contains("$s") {
        return None;
    }
    let (a, rest) = format.split_once("%s")?;
    match rest.split_once("%s") {
        Some((b, c)) => Some((a, b, c)),
        None => Some((a, rest, "")),
    }
}

/// `line` starts with `count` where `format` has its first `%s`, followed by what comes next.
/// Doesn't allocate: it's asked for each line, each frame a tooltip shows.
fn starts_with_count(a: &str, b: &str, count: u32, line: &str) -> bool {
    let Some(rest) = line.strip_prefix(a) else {
        return false;
    };
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    rest[..digits].parse::<u32>().ok() == Some(count) && rest[digits..].starts_with(b)
}

/// Whether `line` is the game's "3 x Diamond" for a stack of `count` (`format`: the language's
/// `item.container.item_count`).
pub fn is_count_line(format: &str, count: u32, line: &str) -> bool {
    parts(format).is_some_and(|(a, b, _)| starts_with_count(a, b, count, line))
}

/// Whether `line` is the game's "and 3 more..." for `more` stacks.
pub fn is_more_line(format: &str, more: u32, line: &str) -> bool {
    parts(format).is_some_and(|(a, b, _)| {
        starts_with_count(a, b, more, line) && line.len() == a.len() + digits(more) + b.len()
    })
}

/// How many digits `n` is written with.
fn digits(n: u32) -> usize {
    n.checked_ilog10().map_or(1, |d| d as usize + 1)
}

/// Where the game's contents lines are in `lines` (the name first), for stacks of `counts` in slot
/// order: the range to take out, or none when they aren't there as expected.
pub fn contents_lines(
    lines: &[impl AsRef<str>],
    counts: &[u32],
    count_format: &str,
    more_format: &str,
) -> Option<std::ops::Range<usize>> {
    let named = counts.len().min(NAMED);
    let more = counts.len() - named;
    let len = named + usize::from(more > 0);
    if named == 0 {
        return None;
    }
    (1..lines.len().saturating_sub(len - 1)).find_map(|start| {
        let all_named = (0..named).all(|j| is_count_line(count_format, counts[j], lines[start + j].as_ref()));
        let ends = more == 0 || is_more_line(more_format, more as u32, lines[start + named].as_ref());
        (all_named && ends).then_some(start..start + len)
    })
}

/// Whether the grid shows for `item` holding `stacks` stacks, Shift held or not, with the module's
/// Only with Shift.
pub fn grid(item: &str, stacks: usize, only_with_shift: bool) -> bool {
    stacks > 0 && is_shulker_box(item) && shows(only_with_shift, shift())
}

pub fn is_shulker_box(item: &str) -> bool {
    item.ends_with("shulker_box")
}

/// Where slot `slot`'s icon goes from the grid's top left.
pub fn cell(slot: u8) -> [f32; 2] {
    let slot = usize::from(slot);
    [
        (slot % COLUMNS) as f32 * CELL + 1.0,
        (slot / COLUMNS) as f32 * CELL + 1.0,
    ]
}

/// The box's colour behind the grid, translucent: its dye's, or the undyed box's purple.
pub fn box_color(item: &str) -> u32 {
    const DYES: [(&str, u32); 16] = [
        ("white", 0xF9FFFE),
        ("orange", 0xF9801D),
        ("magenta", 0xC74EBD),
        ("light_blue", 0x3AB3DA),
        ("yellow", 0xFED83D),
        ("lime", 0x80C71F),
        ("pink", 0xF38BAA),
        ("gray", 0x474F52),
        ("light_gray", 0x9D9D97),
        ("cyan", 0x169C9C),
        ("purple", 0x8932B8),
        ("blue", 0x3C44AA),
        ("brown", 0x835432),
        ("green", 0x5E7C16),
        ("red", 0xB02E26),
        ("black", 0x1D1D21),
    ];
    let name = item.rsplit(':').next().unwrap_or(item);
    let dye = name.strip_suffix("_shulker_box");
    let rgb = dye
        .and_then(|d| DYES.iter().find(|e| e.0 == d))
        .map_or(0x97_6997, |e| e.1);
    0xA0_000000 | rgb
}

// Shift, as the key hook last saw it: the tooltip hooks can't ask.
thread_local! {
    static SHIFT: std::cell::Cell<[bool; 2]> = const { std::cell::Cell::new([false; 2]) };
}

pub fn shift_key(key: &str, pressed: bool) {
    let side = match key {
        "key.keyboard.left.shift" => 0,
        "key.keyboard.right.shift" => 1,
        _ => return,
    };
    SHIFT.with(|s| {
        let mut held = s.get();
        held[side] = pressed;
        s.set(held);
    });
}

pub fn shift() -> bool {
    SHIFT.with(|s| s.get().contains(&true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lays_out_a_box() {
        assert!(
            is_shulker_box("minecraft:red_shulker_box") && is_shulker_box("minecraft:shulker_box")
        );
        assert!(!is_shulker_box("minecraft:chest"));
        assert_eq!(cell(10), [19.0, 19.0]);
        assert_eq!(SIZE, [162.0, 54.0]);
        assert_eq!(box_color("minecraft:light_blue_shulker_box"), 0xA03A_B3DA);
        assert_eq!(box_color("minecraft:shulker_box"), 0xA097_6997);
    }

    #[test]
    fn finds_the_games_contents_lines() {
        let lines = ["Shulker Box", "3 x Diamond", "64 x Stone", "Lore"];
        assert_eq!(contents_lines(&lines, &[3, 64], COUNT_FORMAT, MORE_FORMAT), Some(1..3));
        assert_eq!(contents_lines(&lines, &[3, 63], COUNT_FORMAT, MORE_FORMAT), None);
        let many = ["Box", "1 x A", "2 x B", "3 x C", "4 x D", "5 x E", "and 2 more...", "Lore"];
        assert_eq!(
            contents_lines(&many, &[1, 2, 3, 4, 5, 6, 7], COUNT_FORMAT, MORE_FORMAT),
            Some(1..7)
        );
        assert_eq!(contents_lines(&many[..6], &[1, 2, 3, 4, 5, 6, 7], COUNT_FORMAT, MORE_FORMAT), None);
        assert!(!is_count_line(COUNT_FORMAT, 3, "33 x Dirt"));
        assert!(!is_count_line("%2$s ×%1$s", 3, "Dirt ×3"), "positional: left alone");
        assert!(is_more_line(MORE_FORMAT, 12, "and 12 more..."));
        assert!(!is_more_line(MORE_FORMAT, 12, "and 12 more... and"));
        assert!(shows(false, false) && shows(true, true) && !shows(true, false));
    }

    #[test]
    fn hears_either_shift() {
        shift_key("key.keyboard.right.shift", true);
        assert!(shift());
        shift_key("key.keyboard.right.shift", false);
        shift_key("key.keyboard.a", true);
        assert!(!shift());
    }
}
