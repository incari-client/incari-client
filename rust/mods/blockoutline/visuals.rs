// The mod's outline on the block the game would have outlined.
fn incari_blockoutline_rim(
    game: &crate::game::Game<'_>,
    rims: &mut Vec<crate::game::native_hooks::NativeBlockRim>,
) {
    use crate::game::{incari_blockoutline as m, incari_client as c};
    let _ = game;
    let Some(pos) = m::take() else {
        return;
    };
    let Some((argb, width)) = c::settings(m::ID, |s| {
        (
            m::argb(s.color("color"), s.int("opacity")),
            m::width(s.float("width")),
        )
    }) else {
        return;
    };
    rims.push(crate::game::native_hooks::NativeBlockRim { pos, argb, width });
}
