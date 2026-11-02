// A new atlas: the entity canvas is blank again.
fn incari_cosmetics_reloaded() {
    crate::game::incari_cosmetics::with(|s| s.painted = false);
}
