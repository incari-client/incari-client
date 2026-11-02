// Once a tick: the world's waypoints and remembered map opened or closed, deaths, due saves.
fn incari_waypoints_tick(game: &mut crate::game::Game<'_>) {
    crate::game::incari_waypoints_world::tick(game);
}
