// A hit on a server's object: a crystal it breaks is taken off this client at once.
fn incari_fastcrystal_attacked(game: &mut crate::game::Game<'_>, entity: u32) {
    use crate::game::{incari_client as c, incari_fastcrystal as m};
    // The server breaks it only if the hit does damage: weakness with a bare hand doesn't.
    if c::settings(m::ID, |s| s.bool("crystals")) != Some(true) || !game.native_attack_hurts() {
        return;
    }
    if game.native_take_crystal(entity) {
        m::taken(entity, game.native_latency());
    }
}
