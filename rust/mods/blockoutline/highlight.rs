// The game's own outline skipped, and the block kept for the mod's.
fn incari_blockoutline_highlight(game: &crate::game::Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    use crate::game::{incari_blockoutline as m, incari_client as c};
    let _ = game;
    if c::on(m::ID) {
        m::aim(pos);
        *canceled = true;
    }
}
