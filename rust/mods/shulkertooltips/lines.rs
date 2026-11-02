// The game's "3 x Diamond" lines taken out while the grid shows them, as the Java module does.
fn incari_shulkertooltips_lines(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    lines: &mut Vec<(horizon_client::text::Text, u32)>,
) {
    use crate::game::{incari_client as c, incari_shulkertooltips as m};
    let Some(data) = stack.data() else {
        return;
    };
    let shown = c::settings(m::ID, |s| m::grid(&stack.item, data.container.len(), s.bool("only-with-shift")));
    if shown != Some(true) {
        return;
    }
    // The game's own words, or its fallbacks when the language has none.
    let format = |key: &str, fallback: &'static str| {
        let text = game.native_translate(key);
        if text == key { fallback.to_owned() } else { text }
    };
    let count = format("item.container.item_count", m::COUNT_FORMAT);
    let more = format("item.container.more_items", m::MORE_FORMAT);
    let mut slots: Vec<(u8, u32)> = data.container.iter().map(|(slot, s)| (*slot, s.count)).collect();
    slots.sort_unstable_by_key(|s| s.0);
    let counts: Vec<u32> = slots.into_iter().map(|s| s.1).collect();
    let plain: Vec<String> = lines.iter().map(|l| l.0.plain()).collect();
    if let Some(range) = m::contents_lines(&plain, &counts, &count, &more) {
        let _ = lines.drain(range);
    }
}
