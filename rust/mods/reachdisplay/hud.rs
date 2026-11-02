// The last hit's reach on the glass, where the player placed it, until it's hidden.
fn incari_reachdisplay_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    use crate::game::{incari_client as c, incari_reachdisplay as m};
    use incari_core::theme::Tone;
    let _ = corners;
    c::element(game, batch, m::ID, m::ELEMENT, |canvas| {
        let s = canvas.module;
        let reach = m::showing(s.int("hide-after"));
        let shown = reach.or(canvas.editing.then_some(m::SURVIVAL_REACH))?;
        let tone = if m::far(reach, s.bool("far-color")) { Tone::Warning } else { Tone::Text };
        m::with_text(shown, s.int("decimals"), |text| Some(canvas.text_element(text, None, tone)))
    });
}
