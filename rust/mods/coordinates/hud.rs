// The position rows on the glass, where the player placed them (the Java module's Position).
fn incari_coordinates_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    use crate::game::{incari_client as c, incari_coordinates as m};
    use incari_core::hud::{LINE, PAD_X, PAD_Y};
    use incari_core::theme::Tone;
    let _ = corners;
    let spot = game.native_player().map(|p| m::Spot {
        at: [m::block(p.x), m::block(p.y), m::block(p.z)],
        sector: m::sector(p.yaw),
        nether: p.dimension == m::NETHER,
        overworld: p.dimension == m::OVERWORLD,
    });
    c::element(game, batch, m::ID, m::ELEMENT, |canvas| {
        let spot = spot.or(canvas.editing.then_some(m::SAMPLE))?;
        let s = canvas.module;
        let shown = m::Shown {
            one_line: s.choice("layout") == "line",
            facing: s.bool("show-facing"),
            other: s.bool("show-other"),
        };
        let background = s.bool("background");
        m::with_rows(spot, shown, |rows| {
            if rows.is_empty() {
                return None;
            }
            let (pad, pad_y) = if background { (PAD_X, PAD_Y) } else { (0.0, 0.0) };
            let label_width = |canvas: &incari_core::hud::HudCanvas<'_, '_>, label: &str| {
                if label.is_empty() { 0.0 } else { canvas.text_width(label) + m::LABEL_GAP }
            };
            let width = rows
                .iter()
                .map(|(label, value)| label_width(&*canvas, label) + canvas.text_width(value))
                .fold(0.0, f32::max);
            let w = width + pad * 2.0;
            let h = rows.len() as f32 * LINE + if background { pad_y * 2.0 - 1.0 } else { -1.0 };
            if background {
                canvas.fill(0.0, 0.0, w, h, Tone::Background);
            }
            let mut y = pad_y;
            for (i, (label, value)) in rows.iter().enumerate() {
                let mut x = pad;
                if !label.is_empty() {
                    canvas.text(label, x, y, Tone::Accent);
                    x += label_width(&*canvas, label);
                }
                let tone = if m::muted(i, label) { Tone::Muted } else { Tone::Text };
                canvas.text(value, x, y, tone);
                y += LINE;
            }
            Some([w, h])
        })
    });
}
