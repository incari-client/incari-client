// A section's ghost blocks, while Horizon meshes it.
fn incari_schematics_ghost_section(
    game: &crate::game::Game<'_>,
    section: [i32; 3],
    ghosts: &mut Vec<crate::game::native_hooks::NativeGhost>,
) {
    crate::game::incari_schematics_world::ghost_section(game, section, ghosts);
}
