// Incari draws the title screen, the pause menu and its own screens in a world itself.
fn incari_client_claim(game: &crate::game::Game<'_>, screen: &str, claimed: &mut bool) {
    let _ = game;
    if crate::game::incari_client::claims(screen) {
        *claimed = true;
    }
}
