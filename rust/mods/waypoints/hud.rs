// The waypoints over the world: icons and tags, edge arrows, beams.
fn incari_waypoints_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    let _ = corners;
    crate::game::incari_waypoints_world::hud(game, batch);
}
