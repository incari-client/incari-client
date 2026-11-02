// The shield and the totems on the glass, each where the player placed it (the Java module's
// Shield and Totems).
fn incari_shieldtotem_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    use crate::game::{incari_client as c, incari_shieldtotem as m};
    use incari_core::hud::LINE;
    use incari_core::theme::Tone;
    let _ = corners;
    let game = &*game;
    let tone = |t: m::Tone| match t {
        m::Tone::Text => Tone::Text,
        m::Tone::Positive => Tone::Positive,
        m::Tone::Warning => Tone::Warning,
        m::Tone::Negative => Tone::Negative,
    };
    let in_world = game.native_player().is_some();
    let text_y = ((m::ICON - LINE) / 2.0).floor() + 1.0;

    c::element(game, batch, m::ID, m::SHIELD_ELEMENT, |canvas| {
        let s = canvas.module;
        if !s.bool("show-shield") {
            return None;
        }
        // The off hand first, where it's usually carried.
        let held = ["offhand", "mainhand"]
            .into_iter()
            .filter(|_| in_world)
            .filter_map(|slot| game.native_equipped(slot))
            .find(|st| &*st.item == m::SHIELD)
            .map(|st| m::Shield {
                damage: st.damage,
                max: game.native_max_damage(&st.item),
                cooldown: game.native_item_cooldown(st),
            });
        // The HUD editor's: a new shield.
        let sample = || m::Shield {
            damage: 0,
            max: game.native_max_damage(m::SHIELD),
            cooldown: 0.0,
        };
        let shield = held.or_else(|| canvas.editing.then(sample))?;
        canvas.item(m::SHIELD, 0.0, 0.0);
        if let Some((width, rgb)) = m::bar(shield) {
            canvas.fill_argb(2.0, 13.0, 13.0, 2.0, 0xFF00_0000);
            canvas.fill_argb(2.0, 13.0, width, 1.0, 0xFF00_0000 | rgb);
        }
        let text_tone = tone(m::shield_tone(shield, s.bool("wear-colors")));
        let width = m::with_shield_text(shield, |text| {
            canvas.text(text, m::ICON + m::TEXT_GAP, text_y, text_tone);
            canvas.text_width(text)
        });
        if shield.cooldown > 0.0 {
            // What's left of the cooldown, as a bar under the word.
            let bar = (canvas.text_width(m::DISABLED) * shield.cooldown).round();
            canvas.fill(m::ICON + m::TEXT_GAP, m::ICON - 1.0, bar, 1.0, Tone::Negative);
        }
        Some([m::ICON + m::TEXT_GAP + width, m::ICON])
    });

    c::element(game, batch, m::ID, m::TOTEMS_ELEMENT, |canvas| {
        let s = canvas.module;
        if !s.bool("show-totems") {
            return None;
        }
        let count = if in_world {
            let count = game.native_item_count(m::TOTEM);
            // None carried and not warned of: hidden, but placeable in the HUD editor.
            if count == 0 && !s.bool("show-no-totems") && !canvas.editing {
                return None;
            }
            count
        } else if canvas.editing {
            m::SAMPLE_TOTEMS
        } else {
            return None;
        };
        canvas.item(m::TOTEM, 0.0, 0.0);
        let text_tone = if count == 0 { Tone::Negative } else { Tone::Text };
        let width = m::with_count_text(count, |text| {
            canvas.text(text, m::ICON + m::TEXT_GAP, text_y, text_tone);
            canvas.text_width(text)
        });
        Some([m::ICON + m::TEXT_GAP + width, m::ICON])
    });
}
