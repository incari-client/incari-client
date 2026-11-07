// A block used on a server: a charged anchor it sets off is shown as air until the server answers.
fn incari_fastcrystal_used(game: &mut crate::game::Game<'_>, pos: [i32; 3], main_hand: bool, sequence: i32) {
    use crate::game::{incari_client as c, incari_fastcrystal as m};
    if !main_hand
        || c::settings(m::ID, |s| s.bool("anchors")) != Some(true)
        || !game.native_anchor_set_off(pos)
    {
        return;
    }
    game.native_predict_air(pos, sequence);
}
