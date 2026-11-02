// The camera draws from the Freelook rotation while it's on.
fn incari_freelook_camera(yaw: &mut f32, pitch: &mut f32) {
    crate::game::incari_freelook::with(|f| f.camera(yaw, pitch));
}
