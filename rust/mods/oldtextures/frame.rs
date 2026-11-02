// Each frame: Old Textures and Programmer Art follow each other, as the Java module follows its pack.
fn incari_oldtextures_frame(game: &mut crate::game::Game<'_>) {
    use crate::game::{incari_client as c, incari_oldtextures as m};
    if !c::ready() {
        return;
    }
    match m::follow(c::on(m::ID), game.native_programmer_art()) {
        Some(m::Change::Pack(on)) => game.native_set_programmer_art(on),
        Some(m::Change::Module(on)) => c::set_enabled(m::ID, on),
        None => {}
    }
}
