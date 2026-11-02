// The glint in the mod's colour.
fn incari_glintcolor_color(game: &crate::game::Game<'_>, tint: &mut [f32; 3]) {
    use crate::game::{incari_client as c, incari_glintcolor as m};
    let _ = game;
    if let Some(t) = c::settings(m::ID, |s| m::tint(s.color("color"))) {
        *tint = t;
    }
}
