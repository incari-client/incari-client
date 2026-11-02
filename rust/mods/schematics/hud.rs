// The Schematic and Material list elements, where the player placed them.
fn incari_schematics_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    let _ = corners;
    crate::game::incari_schematics_world::hud(game, batch);
}
