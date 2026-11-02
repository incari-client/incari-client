// The cooldown as a bar, a percentage or a ring, where the player placed it.
fn incari_cooldownindicator_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    use crate::game::{incari_client as c, incari_cooldownindicator as m};
    use incari_core::theme::Tone;
    let _ = corners;
    if game.native_player().is_none() {
        return;
    }
    let strength = game.native_attack_strength();
    let Some(targets) = c::settings(m::ID, |s| {
        s.bool("hide-when-ready") && s.bool("ready-on-target")
    }) else {
        return;
    };
    // The crosshair's target only matters once charged.
    let on_target = targets && strength >= 1.0 && game.native_targeted_entity().is_some();
    // The charge, and its colour: the custom one, else the tone's.
    let look = |s: &incari_core::framework::setting::Settings, editing: bool| {
        let charge = m::charge(strength, s.bool("hide-when-ready"), on_target, editing)?;
        let custom = s.bool("custom-colors");
        let color = if m::ready(charge) {
            s.color("ready-color")
        } else {
            s.color("charging-color")
        };
        let tone = if m::ready(charge) {
            Tone::Positive
        } else {
            Tone::Accent
        };
        Some((charge, custom.then_some(color), tone))
    };
    c::element(game, batch, m::ID, "bar", |canvas| {
        let style = canvas.module.choice("style");
        if style == "ring" {
            return None;
        }
        let (charge, color, tone) = look(canvas.module, canvas.editing)?;
        let argb = color.unwrap_or_else(|| canvas.p.theme.tone(tone));
        if style == "number" {
            let width = canvas.text_width(m::WIDEST);
            return m::with_percent(charge, |text| {
                let x = ((width - canvas.text_width(text)) / 2.0).floor();
                canvas.text_argb(text, x, 0.0, argb);
                Some([width, incari_core::hud::LINE])
            });
        }
        let (w, h) = (
            canvas.module.int("bar-width") as f32,
            canvas.module.int("bar-height") as f32,
        );
        canvas.fill(0.0, 0.0, w, h, Tone::Background);
        canvas.fill_argb(0.0, 0.0, m::filled(w, charge), h, argb);
        Some([w, h])
    });
    c::element(game, batch, m::ID, "ring", |canvas| {
        if canvas.module.choice("style") != "ring" {
            return None;
        }
        let (charge, color, tone) = look(canvas.module, canvas.editing)?;
        let argb = color.unwrap_or_else(|| canvas.p.theme.tone(tone));
        let r = canvas.module.int("ring-size") as f32;
        let background = canvas.p.theme.tone(Tone::Background);
        canvas.p.ring(r, r, r, m::RING_WIDTH, background);
        // A screen pixel a strip.
        let step = 1.0 / (canvas.p.scale * canvas.p.zoom).max(1.0);
        m::arc(r, m::RING_WIDTH, charge, step, |[x, y, w, h]| {
            canvas.fill_argb(x, y, w, h, argb)
        });
        Some([r * 2.0, r * 2.0])
    });
}
