// Room for the grid under a shulker box's tooltip, when it holds something.
fn incari_shulkertooltips_size(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    size: &mut Option<[f32; 2]>,
) {
    use crate::game::{incari_client as c, incari_shulkertooltips as m};
    let _ = game;
    if size.is_some() {
        return;
    }
    let stacks = stack.data().map_or(0, |d| d.container.len());
    if c::settings(m::ID, |s| m::grid(&stack.item, stacks, s.bool("only-with-shift"))) == Some(true) {
        *size = Some(m::SIZE);
    }
}
