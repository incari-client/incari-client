// The count on the glass, where the player placed it.
fn incari_cps_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    use crate::game::{incari_client as c, incari_cps as m};
    let _ = corners;
    let (left, right) = game.native_clicks();
    c::element(game, batch, m::ID, "cps", |canvas| {
        let buttons = canvas.module.choice("buttons");
        m::with_text(buttons, left, right, |text| {
            Some(canvas.text_element(text, Some(m::widest(buttons)), incari_core::theme::Tone::Text))
        })
    });
}
