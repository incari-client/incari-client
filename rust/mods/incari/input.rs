// Input reaches Incari's screens and keys before the game.
fn incari_client_input(
    game: &mut crate::game::Game<'_>,
    input: &crate::game::native_hooks::NativeInput<'_>,
    consumed: &mut bool,
) {
    if crate::game::incari_client::input(game, input) {
        *consumed = true;
    }
}
