// Each frame: the chat box cleared while on, and the game's own back once off.
fn incari_clearchat_frame(game: &mut crate::game::Game<'_>) {
    use crate::game::{incari_clearchat as m, incari_client as c};
    game.native_set_chat_background(m::opacity(c::on(m::ID)));
}
