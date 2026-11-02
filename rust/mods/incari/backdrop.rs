// The menus' photo, under the title screen and the screens over it.
fn incari_client_backdrop(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    crate::game::incari_client::backdrop(game, batch);
}
