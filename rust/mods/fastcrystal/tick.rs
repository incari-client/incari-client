// Crystals the server kept past their wait come back.
fn incari_fastcrystal_tick(game: &mut crate::game::Game<'_>) {
    use crate::game::incari_fastcrystal as m;
    m::tick(|id| {
        game.native_return_crystal(id);
    });
}
