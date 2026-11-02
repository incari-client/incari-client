// A chat line (ADR 0062; `Cosmetics.chatMessage`, `Badges.badgedInChat`): Incari's badge before
// the first mention of each Incari player in it who shows it. Players are found by their names,
// since most servers send chat as plain text with no sender. Once a line.
fn incari_cosmetics_chat_message(game: &crate::game::Game<'_>, message: &mut horizon_client::text::Text) {
    use horizon_client::text::{Style, Text};
    use incari_core::cosmetics::badges::{self, Piece};
    use incari_core::online::Status;
    let own = game.native_player_name();
    let show_badges = crate::game::incari_cosmetics::with(|s| s.show_badges);
    let positions = incari_core::online::with(|o| {
        if o.status() != Status::SignedIn {
            return Vec::new();
        }
        let text: String = message.spans.iter().map(|s| s.text.as_str()).collect();
        badges::chat_positions(&text, |name| {
            if name == own {
                o.shows_badge()
            } else {
                show_badges && o.world_players().get(name).is_some_and(|p| p.badge)
            }
        })
    });
    if positions.is_empty() {
        return;
    }
    let texts: Vec<&str> = message.spans.iter().map(|s| s.text.as_str()).collect();
    let pieces = badges::badged_pieces(&texts, &positions);
    let mut out = Text::empty();
    for piece in pieces {
        match piece {
            Piece::Text(i, text) => out.push(Text::styled(text, message.spans[i].style.clone())),
            Piece::Badge => {
                out.push(Text::styled(
                    badges::GLYPH,
                    Style { font: Some(badges::FONT.into()), color: Some(badges::COLOR), ..Style::default() },
                ));
                out.push(Text::literal(" "));
            }
        }
    }
    *message = out;
}
