// The food row is drawn this frame: its saturation and the HUD's size, for the HUD hook.
fn incari_saturation_health(inputs: &horizon_client::hud_layout::HealthInputs, wi: i32, hi: i32) {
    // A ridden mob's hearts take the food row's place.
    if inputs.vehicle_hearts == 0 {
        crate::game::incari_saturation::drawn(inputs.saturation, wi as f32, hi as f32);
    }
}
