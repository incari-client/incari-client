// The module's keys (Place here, Turn, Mirror, Nudge, Up, Layer), while playing.
fn incari_schematics_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::incari_schematics_world::key(game, key, pressed);
}
