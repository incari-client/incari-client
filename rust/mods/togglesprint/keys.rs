// Sprint and sneak as held, from the module's latches.
fn incari_togglesprint_keys(game: &crate::game::Game<'_>, sprint: &mut bool, sneak: &mut bool) {
    use crate::game::{incari_client as c, incari_togglesprint as m};
    let Some((sprint_mode, sneak_mode)) = c::settings(m::ID, |s| (s.choice("sprint"), s.choice("sneak"))) else {
        m::update(m::Keys::unlatch);
        return;
    };
    if game.native_ui_screen() == "dead" {
        m::update(m::Keys::unlatch);
    }
    (*sprint, *sneak) = m::update(|k| k.tick(*sprint, *sneak, sprint_mode, sneak_mode));
}
