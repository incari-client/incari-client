// The map Horizon composes, as the settings ask; none while it would draw it in a corner, since the
// HUD element draws it where the player placed it.
fn incari_minimap_map(
    game: &crate::game::Game<'_>,
    minimap: &mut Option<crate::game::native_hooks::NativeMinimap>,
) {
    use crate::game::{incari_client as c, incari_minimap as m};
    let _ = game;
    if minimap.is_some() || m::drawing() {
        return;
    }
    let Some((round, turn, zoom)) = c::settings(m::ID, |s| (s.choice("shape") == "round", s.bool("turn"), s.int("zoom"))) else {
        return;
    };
    let (blocks_per_pixel, _) = m::view(m::blocks_across(m::zoom_level(zoom)), round);
    *minimap = Some(crate::game::native_hooks::NativeMinimap {
        corner: horizon_client::extras::Corner::TopRight,
        size: m::SIZE - 2.0 * m::FRAME,
        blocks_per_pixel,
        rotate: turn,
        round,
    });
}
