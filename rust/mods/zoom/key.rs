// The Zoom key (the module's `zoom` setting), held or pressed.
fn incari_zoom_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    use crate::game::{incari_client as c, incari_zoom as m};
    let bound = c::settings(m::ID, |s| s.key("zoom")).filter(|k| k.is_bound());
    if bound != Some(incari_core::input::Key::from_name(key)) {
        return;
    }
    let playing = game.native_playing();
    m::with(|k| k.key(pressed, playing));
}
