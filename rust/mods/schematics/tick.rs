// Once a tick: a chosen file loaded, buttons done, the world compared, messages shown.
fn incari_schematics_tick(game: &mut crate::game::Game<'_>) {
    crate::game::incari_schematics_world::tick(game);
}
