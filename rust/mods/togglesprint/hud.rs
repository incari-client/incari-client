// The status line, where the player placed it.
fn incari_togglesprint_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    use crate::game::{incari_client as c, incari_togglesprint as m};
    use incari_core::theme::Tone;
    let _ = corners;
    let keys = m::keys();
    c::element(game, batch, m::ID, "status", |canvas| {
        let s = canvas.module;
        if !s.bool("show-status") {
            return None;
        }
        let shown = keys.status(s.choice("sprint"), s.choice("sneak"));
        // In the HUD editor with nothing latched, the longest common line.
        let text = shown.or(canvas.editing.then_some(m::SPRINTING_TOGGLED))?;
        let argb = if s.bool("custom-color") {
            s.color("color") | 0xFF00_0000
        } else {
            canvas.p.theme.tone(Tone::Text)
        };
        canvas.text_argb(text, 0.0, 0.0, argb);
        Some([canvas.text_width(text), incari_core::hud::LINE])
    });
}
