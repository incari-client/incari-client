// The hurt tilt, held at the module's Shake amount for this frame.
fn incari_nohurtcam_tilt(game: &crate::game::Game<'_>, strength: &mut f64) {
    use crate::game::{incari_client as c, incari_nohurtcam as m};
    let _ = game;
    if let Some(shake) = c::settings(m::ID, |s| s.int("shake")) {
        *strength = m::tilt(shake);
    }
}
