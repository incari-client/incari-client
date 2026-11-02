// The crosshair is on a block, for the block-hit.
fn incari_oldanimations_highlight(pos: [i32; 3]) {
    let _ = pos;
    crate::game::incari_oldanimations::with(|b| b.seen_block());
}
