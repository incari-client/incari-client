// The keys on the glass, where the player placed them, lit while held (the Java module's Keys).
fn incari_keystrokes_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    use crate::game::{incari_client as c, incari_keystrokes as m};
    use incari_core::theme::Tone;
    let _ = corners;
    let Some(shown) = c::settings(m::ID, |s| m::Shown {
        size: s.int("key-size"),
        mouse: s.bool("show-mouse"),
        clicks: s.bool("show-cps"),
        sneak_sprint: s.bool("show-sneak-sprint"),
        space: s.bool("show-space"),
    }) else {
        return;
    };
    let layout = m::layout(shown);
    let (left, right) = game.native_clicks();
    // Each shown key's state and name, read before drawing: native_mapping needs the game mutably.
    // It makes the name each call (Horizon's helper), a few short strings a frame.
    let mut keys: [(bool, String); m::MAX_CELLS] = Default::default();
    for cell in layout.cells() {
        if let Some((down, name)) = game.native_mapping(m::MAPPINGS[cell.key]) {
            keys[cell.key] = (down, m::short_name(&name).unwrap_or(name));
        }
    }
    c::element(game, batch, m::ID, m::ELEMENT, |canvas| {
        let s = canvas.module;
        let custom = s.bool("custom-color").then(|| 0xFF00_0000 | s.color("pressed-color"));
        for cell in layout.cells() {
            let (down, name) = (keys[cell.key].0, keys[cell.key].1.as_str());
            let [x, y, w, h] = cell.rect.map(|v| v as f32);
            match (down, custom) {
                (true, Some(argb)) => canvas.fill_argb(x, y, w, h, argb),
                (true, None) => canvas.fill(x, y, w, h, Tone::Accent),
                (false, _) => canvas.fill(x, y, w, h, Tone::Background),
            }
            let tone = |muted: bool| match (down, muted) {
                (true, _) => Tone::OnAccent,
                (false, true) => Tone::Muted,
                (false, false) => Tone::Text,
            };
            // Centred, and shrunk if it's still too wide: text never spills out of its key.
            let label = |canvas: &mut incari_core::hud::HudCanvas<'_, '_>, text: &str, ty: f32, muted: bool| {
                let width = canvas.text_width(text);
                let scale = m::fit_scale(width, w);
                if scale >= 1.0 {
                    let tx = x + ((w - width) / 2.0).trunc();
                    canvas.text(text, tx, ty, tone(muted));
                } else {
                    let argb = canvas.color(tone(muted));
                    let ly = ty + m::LINE as f32 * (1.0 - scale) / 2.0;
                    canvas.text_scaled(text, x + (w - width * scale) / 2.0, ly, argb, scale);
                }
            };
            match cell.face {
                m::Face::Name => label(&mut *canvas, name, m::label_y(cell.rect, false) as f32, false),
                m::Face::Clicks(button) => {
                    let ty = m::label_y(cell.rect, true) as f32;
                    label(&mut *canvas, name, ty, false);
                    let count = if button == 0 { left } else { right };
                    m::with_clicks(button, count, |text| {
                        // "12 CPS" too wide for the button: "12".
                        let text = if canvas.text_width(text) > w { m::count_only(text) } else { text };
                        label(&mut *canvas, text, ty + m::LINE as f32, true);
                    });
                }
                m::Face::Bar => {
                    let [bx, by, bw, bh] = m::bar(cell.rect).map(|v| v as f32);
                    canvas.fill(bx, by, bw, bh, if down { Tone::OnAccent } else { Tone::Text });
                }
            }
        }
        Some([layout.size[0] as f32, layout.size[1] as f32])
    });
}
