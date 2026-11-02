// Death and leaving the world let go of the latches, as the game lets go of its own on respawn.
fn incari_togglesprint_tick(game: &mut crate::game::Game<'_>) {
    use crate::game::incari_togglesprint as m;
    if game.native_window() && (game.native_player().is_none() || game.native_ui_screen() == "dead") {
        m::update(m::Keys::unlatch);
    }
}
