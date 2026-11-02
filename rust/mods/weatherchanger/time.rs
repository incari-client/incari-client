// The time of day the player sees, while held.
fn incari_weatherchanger_time(game: &crate::game::Game<'_>, day_time: &mut i64) {
    use crate::game::{incari_client as c, incari_weatherchanger as m};
    let _ = game;
    if let Some(hour) = c::settings(m::ID, |s| s.bool("hold-time").then(|| s.int("hour"))).flatten() {
        *day_time = m::held_time(*day_time, hour);
    }
}
