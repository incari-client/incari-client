// After a player's model: their hat on the head and their back cosmetic on the body (ADR 0061,
// `CosmeticsLayer`), as `look` found them this frame, from the entity canvas.
fn incari_cosmetics_layer(
    game: &crate::game::Game<'_>,
    mob: &crate::game::native_mob_draw::NativeMobRender<'_>,
    who: Option<&crate::game::native_mob_draw::NativePlayerRender<'_>>,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) {
    use crate::game::incari_cosmetics as m;
    use crate::game::native_block_entity_draw::HumanoidPart;
    use incari_core::cosmetics::{catalog, models};
    if who.is_none() {
        return;
    }
    let Some(job) = m::with(|s| s.job(mob.id)) else {
        return;
    };
    let state = models::State {
        age: job.age,
        crouching: job.crouching,
        fall_flying: job.fall_flying,
        chest_armor: job.chest_armor,
        cape_swing: job.cape_swing,
    };
    for (index, part) in [(job.hat, HumanoidPart::Head), (job.back, HumanoidPart::Body)] {
        let Some(cosmetic) = index.and_then(|i| catalog::all().get(i)) else {
            continue;
        };
        let Some(slot) = models::number(cosmetic).map(m::model_slot) else {
            continue;
        };
        draw.push();
        if !draw.attach(part) {
            draw.pop();
            return;
        }
        let light = draw.light;
        models::with_mesh(cosmetic, &state, |spec, quads| {
            if spec.look == models::Look::Glow {
                // Lit by itself, at full brightness.
                draw.light = 0xF0 | 0xF0 << 8;
            }
            for q in quads {
                let (corners, uv) = models::quad_for_rect(q);
                let corners = corners.map(|c| c.map(|v| v / 16.0));
                game.native_draw_quad(draw, horizon_client::hud::MOD_ENTITY_CANVAS_SPRITE, corners, m::uv16(slot, uv), 0xFF_FFFF);
            }
        });
        draw.light = light;
        draw.pop();
    }
}
