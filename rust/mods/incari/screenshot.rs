// An F2 screenshot saved: on the clipboard too, with Copy screenshots on.
fn incari_client_screenshot(game: &mut crate::game::Game<'_>, path: &std::path::Path) {
    crate::game::incari_client::screenshot_saved(game, path);
}
