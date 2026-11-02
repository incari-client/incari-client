// The Freelook key (the module's `look` setting), held or pressed.
fn incari_freelook_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    use crate::game::{incari_client as c, incari_freelook as m};
    let bound = c::settings(m::ID, |s| s.key("look")).filter(|k| k.is_bound());
    if bound != Some(incari_core::input::Key::from_name(key)) {
        return;
    }
    let playing = game.native_playing();
    m::with(|f| f.key(pressed, playing));
}
