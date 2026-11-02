// The armor and the effects on the glass, each where the player placed it (the Java module's
// ArmorElement and EffectsElement).
fn incari_armorpotionview_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    use crate::game::{incari_armorpotionview as m, incari_client as c};
    use incari_core::hud::{HudCanvas, LINE};
    use incari_core::theme::Tone;
    let _ = corners;
    let game = &*game;
    let tone = |t: m::Tone| match t {
        m::Tone::Text => Tone::Text,
        m::Tone::Muted => Tone::Muted,
        m::Tone::Positive => Tone::Positive,
        m::Tone::Warning => Tone::Warning,
        m::Tone::Negative => Tone::Negative,
    };
    let in_world = game.native_player().is_some();

    c::element(game, batch, m::ID, m::ARMOR, |canvas| {
        let s = canvas.module;
        if !s.bool("show-armor") {
            return None;
        }
        let held = s.bool("show-held");
        let worn = |slot: usize| {
            let stack = game.native_equipped(m::SLOTS[slot]).filter(|st| in_world && st.count > 0)?;
            Some(m::Item {
                id: &stack.item,
                damage: stack.damage,
                max: game.native_max_damage(&stack.item),
            })
        };
        let mut items = [m::Item { id: "", damage: 0, max: 0 }; m::SLOTS.len()];
        let mut count = 0;
        for slot in (0..m::SLOTS.len()).filter(|&slot| slot != m::HELD || held) {
            if let Some(item) = worn(slot) {
                items[count] = item;
                count += 1;
            }
        }
        if count == 0 && canvas.editing {
            for slot in (0..m::SLOTS.len()).filter(|&slot| slot != m::HELD || held) {
                items[count] = m::SAMPLE_ITEMS[slot];
                count += 1;
            }
        }
        let items = &items[..count];
        if items.is_empty() {
            return None;
        }
        let durability = s.choice("durability");
        let colors = s.bool("wear-colors");
        let horizontal = s.choice("layout") == "horizontal";
        let cell = |canvas: &HudCanvas<'_, '_>, row: usize, item: m::Item<'_>| {
            m::with_item_text(row, item, durability, |text| {
                text.map_or(m::ITEM_ICON, |t| m::ITEM_ICON + m::ITEM_TEXT_GAP + canvas.text_width(t))
            })
        };
        let cells = items.iter().enumerate().map(|(row, item)| cell(&*canvas, row, *item));
        let size = if horizontal {
            [cells.sum::<f32>() + m::CELL_GAP * (count - 1) as f32, m::ITEM_ICON]
        } else {
            [cells.fold(0.0, f32::max), count as f32 * (m::ITEM_ICON + m::ROW_GAP) - m::ROW_GAP]
        };
        let text_y = ((m::ITEM_ICON - LINE) / 2.0).floor() + 1.0;
        let (mut x, mut y) = (0.0, 0.0);
        for (row, item) in items.iter().enumerate() {
            canvas.item(item.id, x, y);
            // The game's durability bar, as the item's decorations draw it.
            if let Some((width, rgb)) = m::bar(*item) {
                canvas.fill_argb(x + 2.0, y + 13.0, 13.0, 2.0, 0xFF00_0000);
                canvas.fill_argb(x + 2.0, y + 13.0, width, 1.0, 0xFF00_0000 | rgb);
            }
            m::with_item_text(row, *item, durability, |text| {
                if let Some(text) = text {
                    let at = x + m::ITEM_ICON + m::ITEM_TEXT_GAP;
                    canvas.text(text, at, y + text_y, tone(m::wear(*item, colors)));
                }
            });
            if horizontal {
                x += cell(&*canvas, row, *item) + m::CELL_GAP;
            } else {
                y += m::ITEM_ICON + m::ROW_GAP;
            }
        }
        Some(size)
    });

    c::element(game, batch, m::ID, m::EFFECTS, |canvas| {
        let s = canvas.module;
        if !s.bool("show-effects") {
            return None;
        }
        // The real effects, or in the HUD editor the sample when there are none.
        let real = in_world && game.native_effect(0).is_some();
        let sample = !real && canvas.editing;
        let effect = |i: usize| {
            if real {
                game.native_effect(i).map(|e| m::Effect {
                    id: e.id,
                    amplifier: e.amplifier,
                    duration: e.duration,
                    beneficial: e.beneficial,
                })
            } else if sample {
                m::SAMPLE_EFFECTS.get(i).copied()
            } else {
                None
            }
        };
        let count = (0..m::MAX_EFFECTS).take_while(|&i| effect(i).is_some()).count();
        if count == 0 {
            return None;
        }
        let names = s.bool("effect-names");
        let by_kind = s.bool("kind-colors");
        let right = s.choice("side") == "right";
        let mut text = 0.0_f32;
        for (i, e) in (0..count).filter_map(|i| effect(i).map(|e| (i, e))) {
            if names {
                let w = m::with_name(i, e, |k| game.native_translate(k), |t| canvas.text_width(t));
                text = text.max(w);
            }
            text = text.max(m::with_time(i, e.duration, |t| canvas.text_width(t)));
        }
        let width = m::EFFECT_ICON + m::EFFECT_TEXT_GAP + text;
        let height = count as f32 * (m::EFFECT_ICON + m::EFFECT_ROW_GAP) - m::EFFECT_ROW_GAP;
        let line = |canvas: &mut HudCanvas<'_, '_>, text: &str, y: f32, t: Tone| {
            let x = if right {
                width - m::EFFECT_ICON - m::EFFECT_TEXT_GAP - canvas.text_width(text)
            } else {
                m::EFFECT_ICON + m::EFFECT_TEXT_GAP
            };
            canvas.text(text, x, y, t);
        };
        for (i, e) in (0..count).filter_map(|i| effect(i).map(|e| (i, e))) {
            let y = i as f32 * (m::EFFECT_ICON + m::EFFECT_ROW_GAP);
            let icon_x = if right { width - m::EFFECT_ICON } else { 0.0 };
            m::with_icon(i, e.id, |sprite| {
                canvas.sprite(sprite, icon_x, y, m::EFFECT_ICON, m::EFFECT_ICON);
            });
            let time_tone = tone(m::time_tone(e.duration));
            if names {
                let name_tone = tone(m::name_tone(e, by_kind));
                m::with_name(i, e, |k| game.native_translate(k), |t| line(&mut *canvas, t, y, name_tone));
                m::with_time(i, e.duration, |t| line(&mut *canvas, t, y + LINE, time_tone));
            } else {
                let ty = y + ((m::EFFECT_ICON - LINE) / 2.0).floor() + 1.0;
                m::with_time(i, e.duration, |t| line(&mut *canvas, t, ty, time_tone));
            }
        }
        Some([width, height])
    });
}
