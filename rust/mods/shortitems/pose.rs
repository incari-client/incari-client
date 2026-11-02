// The held item scaled in its own space, toward the hand.
fn incari_shortitems_pose(game: &crate::game::Game<'_>, using: bool, pose: &mut glam::Mat4) {
    use crate::game::{incari_client as c, incari_shortitems as m};
    let _ = game;
    let Some(scale) = c::settings(m::ID, |s| {
        m::scale(s.int("size"), using, s.bool("while-blocking"))
    }) else {
        return;
    };
    if scale != 1.0 {
        *pose *= glam::Mat4::from_scale(glam::Vec3::splat(scale));
    }
}
