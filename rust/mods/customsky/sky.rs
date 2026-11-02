// This frame's layers for Horizon to draw after the sunrise, before the sun, moon and stars.
fn incari_customsky_sky_layers(
    game: &crate::game::Game<'_>,
    day_time: i64,
    rain: f32,
    thunder: f32,
    sun_angle: f32,
    layers: &mut Vec<crate::game::native_hooks::NativeSkyLayer>,
) {
    use crate::game::incari_customsky::{self as m, Blend};
    use crate::game::native_hooks::{NativeSkyBlend as B, NativeSkyLayer};
    let _ = game;
    m::each_layer(
        day_time,
        rain,
        thunder,
        sun_angle,
        |texture, blend, alpha, angle, axis| {
            let blend = match blend {
                Blend::Alpha => B::Alpha,
                Blend::Add => B::Add,
                Blend::Subtract => B::Subtract,
                Blend::Multiply => B::Multiply,
                Blend::Dodge => B::Dodge,
                Blend::Burn => B::Burn,
                Blend::Screen => B::Screen,
                Blend::Overlay => B::Overlay,
                Blend::Replace => B::Replace,
            };
            layers.push(NativeSkyLayer {
                texture,
                blend,
                alpha,
                angle,
                axis,
            });
        },
    );
}
