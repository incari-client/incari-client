// The emote wheel's key (G): held, the wheel shows over the game; let go, the emote pointed at
// plays (`incari_core::screens::emote_wheel`).
fn incari_cosmetics_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    use incari_core::screens::emote_wheel as wheel;
    if key != wheel::KEY_NAME {
        return;
    }
    wheel::key_held(pressed);
    // The wheel opens with its key over the game, not over another screen.
    if pressed && game.native_playing() && game.native_player().is_some() {
        crate::game::incari_client::open(game, Box::new(wheel::EmoteWheel::new()));
    }
}
