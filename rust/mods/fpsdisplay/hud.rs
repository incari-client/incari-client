// The frame rate on the glass, where the player placed it.
fn incari_fpsdisplay_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    use crate::game::{incari_client as c, incari_fpsdisplay as m};
    use incari_core::theme::Tone;
    let _ = corners;
    let (fps, _, _) = game.native_frame_rate();
    c::element(game, batch, m::ID, m::ELEMENT, |canvas| {
        let format = canvas.module.choice("format");
        let tone = match m::tone(canvas.module.bool("speed-colors"), fps) {
            m::Tone::Text => Tone::Text,
            m::Tone::Positive => Tone::Positive,
            m::Tone::Warning => Tone::Warning,
            m::Tone::Negative => Tone::Negative,
        };
        m::with_text(format, fps, |text| {
            Some(canvas.text_element(text, Some(m::widest(format)), tone))
        })
    });
}
