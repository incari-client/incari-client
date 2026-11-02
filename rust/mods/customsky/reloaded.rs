// A resource reload: the packs, and so the sky, may have changed (Java: a new SkyRenderer).
fn incari_customsky_reloaded(game: &mut crate::game::Game<'_>) {
    let _ = game;
    crate::game::incari_customsky::mark_stale();
}
