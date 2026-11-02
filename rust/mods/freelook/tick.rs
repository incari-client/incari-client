// Once a tick: Freelook starts or stops as the key says, and the view changes with it.
fn incari_freelook_tick(game: &mut crate::game::Game<'_>) {
    use crate::game::{incari_client as c, incari_freelook as m};
    if !game.native_window() {
        return;
    }
    let mode = c::settings(m::ID, |s| {
        (s.choice("mode"), m::camera_type(s.choice("view")))
    });
    let (playing, player, now) = (
        game.native_playing(),
        game.native_player().is_some(),
        game.native_camera_type(),
    );
    let change = m::with(|f| {
        f.tick(
            mode.map(|x| x.0),
            playing,
            player,
            mode.map_or(now, |x| x.1),
            now,
        )
    });
    match change {
        m::Change::Start(view) | m::Change::Stop(view) => game.native_set_camera_type(view),
        m::Change::None => {}
    }
}
