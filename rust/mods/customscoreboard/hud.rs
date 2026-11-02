// The sidebar's element: placed by the sidebar's size, which Horizon then draws there
// (`place.rs`). In the HUD editor with no sidebar, a sample (GameParts.sample).
fn incari_customscoreboard_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    use crate::game::{incari_client as c, incari_customscoreboard as m};
    use incari_core::framework::hud::Anchor;
    use incari_core::theme::Tone;
    let _ = corners;
    let drawn = m::frame();
    c::element(game, batch, m::ID, m::ELEMENT, |canvas| {
        let size = match drawn {
            Some(size) => size,
            None if canvas.editing => {
                let [w, h] = m::SAMPLE;
                canvas.fill(0.0, 0.0, w, h, Tone::Background);
                let tw = canvas.text_width(m::SAMPLE_TITLE);
                canvas.text(m::SAMPLE_TITLE, ((w - tw) / 2.0).floor(), 1.0, Tone::Text);
                for (i, (name, score)) in m::SAMPLE_LINES.iter().enumerate() {
                    let y = 10.0 + 9.0 * i as f32;
                    canvas.text(name, 2.0, y, Tone::Muted);
                    let sw = canvas.text_width(score);
                    canvas.text_argb(score, w - 2.0 - sw, y, 0xFFFF_5555);
                }
                return Some(m::SAMPLE);
            }
            None => return None,
        };
        // Where the client puts it (HudElement::bounds), for the game to draw it there.
        let e = canvas.element;
        let s = e.float("scale");
        let anchor = Anchor::from_index(e.get("anchor").map_or(0, |a| a.as_choice()));
        let (w, h) = ((size[0] * s).round() as i32, (size[1] * s).round() as i32);
        let (sw, sh) = (canvas.screen[0] as i32, canvas.screen[1] as i32);
        let at = [anchor.x(e.int("x"), w, sw) as f32, anchor.y(e.int("y"), h, sh) as f32];
        m::place(at, s);
        Some(size)
    });
}
