//! Name tag cosmetics (ADR 0061; `NameTags.java`): the name over a player's head in a colour, or
//! fading from one colour to another across it. Only where the server leaves the name uncoloured:
//! a team's or a rank's colour means something in the game, and wins.
use super::art;
use super::catalog::Cosmetic;

/// The colours (RGB) of `style`: one for a plain colour, two for a fade.
pub fn colors(style: &Cosmetic) -> &'static [u32] {
    match style.id {
        "nametag.ice" => &[0xA9DCFF],
        "nametag.gold" => &[0xFFD36A],
        "nametag.mint" => &[0x8FE3B0],
        "nametag.rose" => &[0xFF9BB8],
        "nametag.violet" => &[0xC6A2FF],
        "nametag.frost" => &[0xFFFFFF, 0x7FC8FF],
        "nametag.sunset" => &[0xFFB36A, 0xFF6FA8],
        "nametag.aurora" => &[0x6AF2B0, 0xA06CFF],
        _ => &[0xFFFFFF],
    }
}

/// The colour (RGB) of each character of `name` in `style`, in order. One run for a plain colour.
pub fn styled(name: &str, style: &Cosmetic) -> Vec<(String, u32)> {
    let colors = colors(style);
    let count = name.chars().count();
    if colors.len() == 1 || count < 2 {
        return vec![(name.to_owned(), colors[0])];
    }
    let last = (count - 1) as f32;
    name.chars()
        .enumerate()
        .map(|(i, c)| (c.to_string(), art::mix(colors[0], colors[1], i as f32 / last)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cosmetics::catalog::by_id;

    #[test]
    fn fades_run_from_the_first_colour_to_the_second() {
        let fade = styled("Steve", by_id("nametag.sunset").unwrap());
        assert_eq!(fade.len(), 5);
        assert_eq!(fade[0], ("S".into(), 0xFFB36A));
        assert_eq!(fade[4], ("e".into(), 0xFF6FA8));
        assert_eq!(styled("Steve", by_id("nametag.gold").unwrap()), [("Steve".to_owned(), 0xFFD36A)]);
        assert_eq!(styled("A", by_id("nametag.aurora").unwrap()).len(), 1);
    }
}
