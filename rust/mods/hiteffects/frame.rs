// Ages the remembered hit.
fn incari_hiteffects_frame(elapsed: std::time::Duration) {
    crate::game::incari_hiteffects::with(|s| s.advance(elapsed.as_secs_f32()));
}
