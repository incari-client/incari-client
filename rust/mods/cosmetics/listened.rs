// Players are posed for the mods only while there's something to show: the player wears
// something, others' cosmetics show and Incari players are about, or an emote plays.
fn incari_cosmetics_listened(listened: &mut bool) {
    use incari_core::cosmetics::{wardrobe, world};
    let (own, others) = wardrobe::with(|w| (!w.worn().is_empty(), w.show_others()));
    if own
        || world::with(|w| w.any_playing())
        || (others && incari_core::online::with(|o| !o.world_players().is_empty()))
    {
        *listened = true;
    }
}
