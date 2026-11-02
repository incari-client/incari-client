// The schematic's box and its wrong blocks' edges.
fn incari_schematics_lines(
    game: &crate::game::Game<'_>,
    partial: f32,
    lines: &mut Vec<crate::game::native_hooks::NativeLine>,
) {
    let _ = partial;
    crate::game::incari_schematics_world::lines(game, lines);
}
