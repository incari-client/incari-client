// What the world leaves undrawn this frame.
fn incari_norender_hide(
    game: &crate::game::Game<'_>,
    hide: &mut crate::game::native_hooks::NativeNoRender,
) {
    use crate::game::{incari_client as c, incari_norender as m};
    let _ = game;
    let Some(h) = c::settings(m::ID, |s| {
        let (items_hidden, items_still, one_copy) = m::items(s.choice("items"), s.bool("one-copy"));
        m::Hide {
            items_hidden,
            items_still,
            one_copy,
            orbs: s.bool("orbs"),
            falling_blocks: s.bool("falling-blocks"),
            fireworks: s.bool("fireworks"),
            growth: s.bool("growth"),
            breaking: s.bool("breaking"),
            explosions: s.bool("explosions"),
            all_particles: s.bool("all-particles"),
        }
    }) else {
        return;
    };
    hide.items_hidden = h.items_hidden;
    hide.items_still = h.items_still;
    hide.one_copy = h.one_copy;
    hide.orbs = h.orbs;
    hide.falling_blocks = h.falling_blocks;
    hide.fireworks = h.fireworks;
    hide.particles = horizon_client::particles::ParticleHide {
        all: h.all_particles,
        growth: h.growth,
        breaking: h.breaking,
        explosions: h.explosions,
        fireworks: h.fireworks,
    };
}
