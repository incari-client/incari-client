// A hit's reach, kept while the module is on (the Java module hears attacks only then).
fn incari_reachdisplay_reach(distance: f64) {
    use crate::game::{incari_client as c, incari_reachdisplay as m};
    if c::on(m::ID) {
        m::hit(distance);
    }
}
