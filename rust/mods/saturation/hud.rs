// Saturation's outlines over the food row, in the module's colour.
fn incari_saturation_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    use crate::game::{incari_client as c, incari_saturation as m};
    let _ = corners;
    let Some((saturation, width, height)) = m::take() else {
        return;
    };
    let Some(rgb) = c::settings(m::ID, |s| s.color("color")) else {
        return;
    };
    let argb = 0xFF00_0000 | (rgb & 0xFF_FFFF);
    for i in 0..10 {
        let Some(outline) = m::outline(i, saturation) else {
            break;
        };
        for rect in m::edges(m::shank(i, width, height), outline) {
            game.native_fill(batch, rect, argb);
        }
    }
}
