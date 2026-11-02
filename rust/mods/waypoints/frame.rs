// Once a frame: where the player is, for the overlay and the screens.
fn incari_waypoints_frame(game: &mut crate::game::Game<'_>) {
    crate::game::incari_waypoints_world::frame(game);
}
