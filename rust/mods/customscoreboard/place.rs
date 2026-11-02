// The sidebar moved and scaled to the element.
fn incari_customscoreboard_place(
    game: &crate::game::Game<'_>,
    screen: [f32; 2],
    bounds: [f32; 4],
    offset: &mut [f32; 2],
    scale: &mut f32,
) {
    use crate::game::incari_customscoreboard as m;
    let _ = (game, screen);
    if let Some((o, s)) = m::drawing(bounds) {
        *offset = o;
        *scale = s;
    }
}
