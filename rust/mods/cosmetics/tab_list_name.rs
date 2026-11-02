// A name in the tab list (ADR 0062; `Cosmetics.tabListName`): Incari's badge before it, as the
// player's name tag has it. The player's own has it too while they show others they use Incari.
fn incari_cosmetics_tab_list_name(
    game: &crate::game::Game<'_>,
    uuid: u128,
    local: bool,
    name: &mut horizon_client::text::Text,
) {
    use horizon_client::text::{Style, Text};
    use incari_core::cosmetics::badges;
    use incari_core::online::Status;
    let _ = game;
    let show_badges = crate::game::incari_cosmetics::with(|s| s.show_badges);
    let shows = incari_core::online::with(|o| {
        if o.status() != Status::SignedIn {
            return false;
        }
        if local { o.shows_badge() } else { o.player(uuid).is_some_and(|p| p.badge) && show_badges }
    });
    if shows {
        let mut badged = Text::styled(
            badges::GLYPH,
            Style { font: Some(badges::FONT.into()), color: Some(badges::COLOR), ..Style::default() },
        );
        badged.push(Text::literal(" "));
        badged.push(std::mem::take(name));
        *name = badged;
    }
}
