// Incari's screens over the game's, every frame.
fn incari_client_overlay(game: &mut crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    crate::game::incari_client::frame(game, batch);
}
