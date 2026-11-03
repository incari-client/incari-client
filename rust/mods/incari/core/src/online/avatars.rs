//! The chat menu's pictures of people (Avatars.java): a disc in a colour of their name with its
//! first letter. (The Java client shows a player on this server their skin's face; that needs the
//! game side's tab-list skins, so here everyone has a disc.)
use crate::paint::Painter;

/// Discs' colours: calm, readable under white text, apart from the accent and the warnings.
const COLORS: [u32; 8] = [
    0xFF4E6E9E, 0xFF5A8F7B, 0xFF8A6BB0, 0xFFB0735A, 0xFF4F8FA8, 0xFF9E5A78, 0xFF6F8A4E, 0xFF7A6E5A,
];

pub const ONLINE: u32 = 0xFF5ED38A;
pub const OFFLINE: u32 = 0xFF7D8590;

/// Java's `String.hashCode`, so a name has the same colour in both clients.
fn java_hash(text: &str) -> i32 {
    text.encode_utf16()
        .fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(i32::from(c)))
}

/// `label`'s colour.
pub fn color(label: &str) -> u32 {
    COLORS[java_hash(&label.to_lowercase()).rem_euclid(COLORS.len() as i32) as usize]
}

/// `label`'s colour, lightened for their name over the dark glass.
pub fn lighter(label: &str) -> u32 {
    let c = color(label);
    let lift = |shift: u32| {
        let v = c >> shift & 0xFF;
        (v + (255 - v) / 2) << shift
    };
    0xFF00_0000 | lift(16) | lift(8) | lift(0)
}

/// `label`'s picture, `size` square at (`x`, `y`).
pub fn draw(p: &mut Painter, x: f32, y: f32, size: f32, label: &str) {
    let radius = size / 2.0;
    p.round(x, y, size, size, radius, color(label));
    let letter: String = label
        .chars()
        .next()
        .map_or("?".into(), |c| c.to_uppercase().collect());
    let w = p.width(&letter);
    let th = p.theme;
    p.text(
        &letter,
        x + ((size - w) / 2.0).floor() + 1.0,
        y + ((size - 7.0) / 2.0).floor(),
        th.text,
        false,
    );
}

/// A presence dot on an avatar's lower right corner, ringed in `behind` to stand off it.
pub fn status(p: &mut Painter, x: f32, y: f32, size: f32, online: bool, behind: u32) {
    let (cx, cy) = (x + size - 2.5, y + size - 2.5);
    p.round(cx - 3.5, cy - 3.5, 7.0, 7.0, 3.5, behind);
    p.round(
        cx - 2.5,
        cy - 2.5,
        5.0,
        5.0,
        2.5,
        if online { ONLINE } else { OFFLINE },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_follow_javas_hash() {
        // "steve".hashCode() is 109761491, which is 3 mod 8.
        assert_eq!(java_hash("steve"), 109_761_491);
        assert_eq!(color("Steve"), COLORS[3]);
        assert_eq!(color("Steve"), color("STEVE"));
        assert_eq!(lighter("Steve") >> 24, 0xFF);
    }
}
