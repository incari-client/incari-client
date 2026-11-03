// A hit: remembered for its kill, and its particles.
fn incari_hiteffects_attacked(game: &mut crate::game::Game<'_>, entity: u32, pos: [f64; 3]) {
    use crate::game::{incari_client as c, incari_hiteffects as m};
    let Some((on, key, count)) = c::settings(m::ID, |s| {
        (s.bool("on-hit"), s.choice("hit"), s.int("hit-count"))
    }) else {
        return;
    };
    // Its size, from Horizon's list (a hit is rare: walking it is fine).
    let size = (0..game.native_entity_count())
        .filter_map(|n| game.native_entity_at(n, 1.0))
        .find(|e| e.id == entity)
        .map_or(m::SIZE, |e| [e.max[0] - e.min[0], e.max[1] - e.min[1]]);
    m::with(|s| s.hit(entity, size));
    if !on {
        return;
    }
    let (particle, speed, _) = m::effect(key);
    for _ in 0..count.max(0) {
        let (at, velocity) = m::with(|s| s.spray(pos, size, speed));
        game.native_spawn_particle(particle, at, velocity);
    }
}
