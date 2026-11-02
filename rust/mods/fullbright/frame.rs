// Each frame: the world lit fully while on, and Horizon's own Fullbright extra's choice once off.
fn incari_fullbright_frame(game: &mut crate::game::Game<'_>) {
    use crate::game::{incari_client as c, incari_fullbright as m};
    let on = c::settings(m::ID, |s| m::lit(s.int("level"))).unwrap_or(false);
    game.native_set_fullbright(on);
}
