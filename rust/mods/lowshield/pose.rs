// The shield moved down in its own space.
fn incari_lowshield_pose(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    using: bool,
    pose: &mut glam::Mat4,
) {
    use crate::game::{incari_client as c, incari_lowshield as m};
    let _ = game;
    let Some((lower, while_blocking)) =
        c::settings(m::ID, |s| (s.int("lower"), s.bool("while-blocking")))
    else {
        return;
    };
    let drop = m::drop(&stack.item, using, lower, while_blocking);
    if drop > 0.0 {
        *pose *= glam::Mat4::from_translation(glam::Vec3::new(0.0, -drop, 0.0));
    }
}
