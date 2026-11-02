// The fire overlay, lower.
fn incari_lowfire_fire(game: &crate::game::Game<'_>, lower: &mut f32) {
    use crate::game::{incari_client as c, incari_lowfire as m};
    let _ = game;
    if let Some(percent) = c::settings(m::ID, |s| s.int("lower")) {
        *lower = m::lower(percent);
    }
}
