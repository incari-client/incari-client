// Once a tick: settled changes saved.
fn incari_client_tick(game: &mut crate::game::Game<'_>) {
    crate::game::incari_client::tick(game);
}
