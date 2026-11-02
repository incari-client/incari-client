// After Horizon composed the map this frame: the next time it asks is to draw it in a corner.
fn incari_minimap_frame(game: &mut crate::game::Game<'_>) {
    let _ = game;
    crate::game::incari_minimap::set_drawing(true);
}
