// A player's name tag (ADR 0061, 0062; `NameTags.style`, `Badges.badged`): in their name tag
// cosmetic's colours unless the server coloured it (a team's colour means something), with
// Incari's badge before an Incari player's name while they show it and the player wants badges.
fn incari_cosmetics_name_tag(
    game: &crate::game::Game<'_>,
    who: &crate::game::native_mob_draw::NativeNameTag<'_>,
    tag: &mut horizon_client::text::Text,
) {
    use crate::game::incari_cosmetics as m;
    use horizon_client::text::{Style, Text};
    use incari_core::cosmetics::{Slot, badges, nametags, wardrobe, world};
    let _ = game;
    let key = if who.local { 0 } else { match who.uuid { Some(u) => u, None => return } };
    let (own, show_others) = wardrobe::with(|w| (w.worn(), w.show_others()));
    let show_badges = m::with(|s| s.show_badges);
    let (style, badge) = incari_core::online::with(|o| {
        let incari = if who.local { None } else { o.player(key) };
        let ids = incari.map(|p| p.cosmetics.as_slice());
        let worn = world::with(|w| w.worn_by(key, who.local, own, ids, show_others));
        (worn.loadout.in_slot(Slot::Nametag), incari.is_some_and(|p| p.badge) && show_badges)
    });
    if let Some(style) = style
        && !tag.spans.iter().any(|s| s.style.color.is_some() && !s.text.is_empty())
    {
        let plain = tag.plain();
        let mut styled = Text::empty();
        for (piece, rgb) in nametags::styled(&plain, style) {
            styled.push(Text::colored(piece, rgb));
        }
        *tag = styled;
    }
    if badge {
        let mut badged = Text::styled(
            badges::GLYPH,
            Style { font: Some(badges::FONT.into()), color: Some(badges::COLOR), ..Style::default() },
        );
        badged.push(Text::literal(" "));
        badged.push(std::mem::take(tag));
        *tag = badged;
    }
}
