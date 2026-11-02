// The hurt flash in the mod's colour.
fn incari_hitcolor_color(game: &crate::game::Game<'_>, color: &mut [f32; 4]) {
    use crate::game::{incari_client as c, incari_hitcolor as m};
    let _ = game;
    if let Some(t) = c::settings(m::ID, |s| m::tint(s.color("color"), s.int("strength"))) {
        *color = t;
    }
}
