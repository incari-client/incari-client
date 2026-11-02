// The mouse turns the Freelook camera, not the player, while it's on.
fn incari_freelook_turn(game: &mut crate::game::Game<'_>, dx: f64, dy: f64, claimed: &mut bool) {
    use crate::game::{incari_client as c, incari_freelook as m};
    let _ = game;
    let invert = c::settings(m::ID, |s| s.bool("invert")).unwrap_or(false);
    if m::with(|f| f.turn(dx, dy, invert)) {
        *claimed = true;
    }
}
