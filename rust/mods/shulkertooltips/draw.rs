// The grid: the box's colour, a square a slot, and each stack with its count and wear.
fn incari_shulkertooltips_draw(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    batch: &mut horizon_client::hud::HudBatch,
    origin: [f32; 2],
    size: [f32; 2],
) {
    use crate::game::{incari_client as c, incari_shulkertooltips as m};
    let Some(tinted) = c::settings(m::ID, |s| s.bool("tinted")) else {
        return;
    };
    if size != m::SIZE || !m::is_shulker_box(&stack.item) {
        return;
    }
    let [x, y] = origin;
    if tinted {
        game.native_fill(batch, [x, y, size[0], size[1]], m::box_color(&stack.item));
    }
    for slot in 0..(m::COLUMNS * m::ROWS) as u8 {
        let [cx, cy] = m::cell(slot);
        game.native_fill(batch, [x + cx, y + cy, 16.0, 16.0], m::SLOT);
    }
    let Some(data) = stack.data() else {
        return;
    };
    for (slot, item) in &data.container {
        if usize::from(*slot) < m::COLUMNS * m::ROWS {
            let [cx, cy] = m::cell(*slot);
            game.native_stack(batch, item, [x + cx, y + cy], true);
        }
    }
}
