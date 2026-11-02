// Once a tick: the block-hit swings the arm while the attack key's held on a block with an item
// in use. Drawn only: nothing is sent.
fn incari_oldanimations_tick(game: &mut crate::game::Game<'_>) {
    use crate::game::{incari_client as c, incari_oldanimations as m};
    if !game.native_window() {
        return;
    }
    let on = c::settings(m::ID, |s| s.bool("block-hit")).unwrap_or(false);
    let playing = game.native_playing();
    let attack = on
        && playing
        && game
            .native_mapping("key.attack")
            .is_some_and(|(down, _)| down);
    m::with(|b| b.tick(on, playing, attack));
}
