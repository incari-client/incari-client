//! Incari's badge beside an Incari player's name tag, their name in the tab list and their name in
//! chat (ADR 0062; `Badges.java`), so Incari players know each other without asking. The badge is
//! one glyph of Incari's pack's `incari:badge` font (the mark, white so it keeps its own colours),
//! so the game's own text drawing carries it.

/// The badge's character in `incari:badge` (the pack's `font/badge.json`).
pub const GLYPH: &str = "\u{E100}";
/// Its font.
pub const FONT: &str = "incari:badge";
/// Its colour: white, so the glyph keeps its own.
pub const COLOR: u32 = 0xFFFFFF;

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Where a badge goes in chat's `text` (character offsets): before the first mention of each name
/// `badged` takes. Names are runs of name characters up to 16 long; legacy `§` codes (1.8
/// servers' chat) are skipped over.
pub fn chat_positions(text: &str, badged: impl Fn(&str) -> bool) -> Vec<usize> {
    let chars: Vec<char> = text.chars().collect();
    let mut at = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '§' {
            i += 2;
        } else if !is_name_char(c) {
            i += 1;
        } else {
            let start = i;
            while i < chars.len() && is_name_char(chars[i]) {
                i += 1;
            }
            let name: String = chars[start..i].iter().collect();
            if i - start <= 16 && seen.insert(name.clone()) && badged(&name) {
                at.push(start);
            }
        }
    }
    at
}

/// The `§` codes in `text`, in order: what a later cut of the same text must repeat.
pub fn legacy_codes(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i + 1 < chars.len() {
        if chars[i] == '§' {
            out.push(chars[i]);
            out.push(chars[i + 1]);
            i += 2;
        } else {
            i += 1;
        }
    }
    out
}

/// A piece of a badged chat line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Piece {
    /// Text in the style of the message's span `span`.
    Text(usize, String),
    /// The badge, then a space.
    Badge,
}

/// The message whose spans' texts are `spans`, rebuilt piece by piece with a badge at each of
/// `positions` (from `chat_positions` on the spans' text joined). A span cut by a badge repeats
/// the `§` codes before the cut after it, so a coloured name keeps its colour.
pub fn badged_pieces(spans: &[&str], positions: &[usize]) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut offset = 0;
    let mut next = 0;
    for (index, piece) in spans.iter().enumerate() {
        let chars: Vec<char> = piece.chars().collect();
        let end = offset + chars.len();
        let mut from = 0;
        let mut carried = String::new();
        while next < positions.len() && positions[next] < end {
            let cut = positions[next].saturating_sub(offset).max(from);
            next += 1;
            let before: String = chars[from..cut].iter().collect();
            if !before.is_empty() {
                out.push(Piece::Text(index, format!("{carried}{before}")));
            }
            carried.push_str(&legacy_codes(&before));
            out.push(Piece::Badge);
            from = cut;
        }
        let rest: String = chars[from..].iter().collect();
        if !rest.is_empty() || !carried.is_empty() {
            out.push(Piece::Text(index, format!("{carried}{rest}")));
        }
        offset = end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badges_go_before_each_names_first_mention() {
        let text = "<Steve> hi Alex, Steve here";
        let at = chat_positions(text, |n| n == "Steve" || n == "Alex");
        assert_eq!(at, [1, 11]);
        let pieces = badged_pieces(&["<", "Steve> hi Alex, Steve here"], &at);
        assert_eq!(
            pieces,
            [
                Piece::Text(0, "<".into()),
                Piece::Badge,
                Piece::Text(1, "Steve> hi ".into()),
                Piece::Badge,
                Piece::Text(1, "Alex, Steve here".into()),
            ]
        );
    }

    #[test]
    fn legacy_codes_are_skipped_and_carried() {
        let text = "§6[VIP] §eSteve: yo";
        let at = chat_positions(text, |n| n == "Steve");
        assert_eq!(at, [10]);
        let pieces = badged_pieces(&[text], &at);
        assert_eq!(
            pieces,
            [
                Piece::Text(0, "§6[VIP] §e".into()),
                Piece::Badge,
                Piece::Text(0, "§6§eSteve: yo".into())
            ]
        );
        assert!(chat_positions("a_name_far_too_long_for_minecraft", |_| true).is_empty());
    }
}
