// Once a tick (ADR 0061, `Cosmetics.tick`): emotes start and end (the wheel's own turning the
// camera to face the player while it plays), and trails are left behind moving players.
fn incari_cosmetics_tick(game: &mut crate::game::Game<'_>) {
    use crate::game::incari_cosmetics as m;
    use incari_core::cosmetics::{Slot, catalog, wardrobe, world};
    let ticks = m::with(|s| {
        s.tick();
        s.ticks
    });
    if ticks % m::SETTINGS_TICKS == 1 {
        let path = horizon_os::env::current_dir().unwrap_or_default().join("incari").join("settings.txt");
        let text = horizon_os::fs::read(path).ok().and_then(|b| String::from_utf8(b).ok()).unwrap_or_default();
        m::with(|s| s.show_badges = m::show_badges(&text));
    }
    let played: Vec<(u128, String)> =
        incari_core::online::with(|o| o.take_emotes()).into_iter().map(|e| (e.player, e.emote)).collect();
    let feet = game.native_player().map(|p| [p.x, p.y, p.z]);
    let distance_sq = |a: [f64; 3], b: [f64; 3]| (0..3).map(|i| (a[i] - b[i]) * (a[i] - b[i])).sum::<f64>();
    let moved_sq = match (feet, m::with(|s| s.own)) {
        (Some(a), Some(b)) => distance_sq(a, b),
        _ => 0.0,
    };
    m::with(|s| s.own = feet);
    let camera = game.native_camera_type();
    let (show_others, own_trail) = wardrobe::with(|w| (w.show_others(), w.worn().in_slot(Slot::Trail)));
    let mut view = None;
    let mut spawn = Vec::new();
    world::with(|w| {
        // The wheel's emote, now the player and the camera are known. The player's own is kept
        // under 0, as `look` keeps it.
        if let Some(emote) = w.take_own()
            && feet.is_some()
        {
            view = w.play_own(0, emote, camera);
        }
        // Moving (the player's keys, a jump, being pushed) cuts the player's own emote short.
        if let Some(v) = w.tick(feet.is_some(), show_others, &played, moved_sq > 0.0004, camera) {
            view = Some(v);
        }
        let Some(me) = feet else { return };
        if let Some(trail) = own_trail
            && let Some(p) = w.trail(trail, 0, 0.0, moved_sq)
        {
            spawn.push((me, p));
        }
        m::with(|s| {
            let ticks = s.ticks;
            for seen in &mut s.seen {
                let moved = seen.ticked.map_or(0.0, |b| distance_sq(seen.position, b));
                seen.ticked = Some(seen.position);
                let Some(trail) = seen.trail.and_then(|i| catalog::all().get(i)) else { continue };
                if seen.tick + 1 < ticks {
                    continue;
                }
                if let Some(p) = w.trail(trail, seen.mob, distance_sq(seen.position, me), moved) {
                    spawn.push((seen.position, p));
                }
            }
        });
    });
    if let Some(v) = view {
        game.native_set_camera_type(v);
    }
    for (at, p) in spawn {
        let at = [at[0] + p.offset[0], at[1] + p.offset[1], at[2] + p.offset[2]];
        game.native_spawn_particle(p.kind, at, p.velocity);
    }
}
