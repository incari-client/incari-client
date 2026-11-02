// The weather the player sees.
fn incari_weatherchanger_weather(game: &crate::game::Game<'_>, rain: &mut f32, thunder: &mut f32) {
    use crate::game::{incari_client as c, incari_weatherchanger as m};
    let _ = game;
    if let Some((r, t)) = c::settings(m::ID, |s| m::weather(s.choice("weather"))).flatten() {
        (*rain, *thunder) = (r, t);
    }
}
