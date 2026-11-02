// Ages the last hit; forgets it while the module is off.
fn incari_reachdisplay_frame(elapsed: std::time::Duration) {
    use crate::game::{incari_client as c, incari_reachdisplay as m};
    if c::on(m::ID) {
        m::advance(elapsed.as_secs_f32());
    } else {
        m::clear();
    }
}
