// A death soon after the player's hit: the kill burst and its sound.
fn incari_hiteffects_died(game: &mut crate::game::Game<'_>, entity: u32, pos: [f64; 3]) {
    use crate::game::{incari_client as c, incari_hiteffects as m};
    let Some((on, key, count, sound)) = c::settings(m::ID, |s| {
        (
            s.bool("on-kill"),
            s.choice("kill"),
            s.int("kill-count"),
            s.bool("kill-sound"),
        )
    }) else {
        return;
    };
    let Some(size) = m::with(|s| s.killed(entity).then(|| s.size())) else {
        return;
    };
    if !on {
        return;
    }
    let (particle, speed, event) = m::effect(key);
    for _ in 0..count.max(0) {
        let (at, velocity) = m::with(|s| s.spray(pos, size, speed));
        game.native_spawn_particle(particle, at, velocity);
    }
    if sound {
        let at = [pos[0], pos[1] + size[1] / 2.0, pos[2]];
        game.native_play_local_sound(event, at, m::VOLUME, m::PITCH, 0);
    }
}
