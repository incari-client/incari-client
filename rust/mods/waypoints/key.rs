// The New waypoint (N) and Waypoint list (U) keys, while playing.
fn incari_waypoints_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::incari_waypoints_world::key(game, key, pressed);
}
