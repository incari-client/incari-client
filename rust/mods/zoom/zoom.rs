// The zoom, while it's on: Horizon asks while playing, and the spyglass's own wins.
fn incari_zoom_view(
    game: &crate::game::Game<'_>,
    zoom: &mut Option<crate::game::native_hooks::NativeZoom>,
) {
    use crate::game::{incari_client as c, incari_zoom as m};
    let _ = game;
    let wanted = c::settings(m::ID, |s| {
        (
            s.choice("mode"),
            m::zoom(s.float("amount"), s.choice("turning"), s.bool("ease")),
        )
    });
    let active = m::with(|k| k.active(wanted.map(|w| w.0)));
    if zoom.is_some() || !active {
        return;
    }
    if let Some((_, (factor, smooth, turn))) = wanted {
        *zoom = Some(crate::game::native_hooks::NativeZoom {
            factor,
            smooth,
            turn,
        });
    }
}
