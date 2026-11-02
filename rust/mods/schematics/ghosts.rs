// The ghosts' box, opacity and range this frame, and the wrong blocks' fills.
fn incari_schematics_ghosts(
    game: &crate::game::Game<'_>,
    ghosts: &mut crate::game::native_hooks::NativeGhosts,
) {
    crate::game::incari_schematics_world::ghosts(game, ghosts);
}
