// The map where the player placed it (MapElement.draw): the frame, Horizon's terrain, the arrow,
// the compass letters and the coordinates' tag.
fn incari_minimap_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    use crate::game::{incari_client as c, incari_minimap as m};
    use incari_core::paint::Shape;
    use incari_core::theme::Tone;
    let _ = corners;
    // Horizon has drawn (or not) its own corner map: asking again is to compose.
    m::set_drawing(false);
    let player = game.native_player().map(|p| ([p.x, p.y, p.z], p.yaw));
    c::element(game, batch, m::ID, m::ELEMENT, |canvas| {
        if player.is_none() && !canvas.editing {
            return None;
        }
        let s = canvas.module;
        let round = s.choice("shape") == "round";
        let turn = s.bool("turn");
        let coordinates = s.bool("coordinates");
        let th = canvas.p.theme;
        let (size, frame) = (m::SIZE, m::FRAME);
        let center = size / 2.0;
        let inner = size - 2.0 * frame;
        let radius = inner / 2.0;

        // The frame, and the backdrop where nothing's loaded.
        if round {
            canvas.round(0.0, 0.0, size, size, center, th.background);
        } else {
            canvas.fill(0.0, 0.0, size, size, Tone::Background);
        }
        let heading = player.map_or(0.0, |p| m::heading(p.1));
        let rotation = if turn { heading } else { 0.0 };
        if player.is_some() {
            let (_, src) = m::view(m::blocks_across(m::zoom_level(s.int("zoom"))), round);
            canvas.p.sprite(horizon_client::hud::MINIMAP_SPRITE, src, [frame, frame, inner, inner], 0xFFFF_FFFF);
        }
        if round {
            canvas.p.ring(center, center, center - 0.5, 1.0, th.tone(Tone::Edge));
        }

        // Waypoints on the map (MapElement): those beyond its rim sit on its edge.
        if s.bool("waypoints") {
            let bpu = m::blocks_across(m::zoom_level(s.int("zoom"))) as f32 / inner;
            let (sin, cos) = rotation.to_radians().sin_cos();
            incari_core::waypoint::with(|store| {
                incari_core::waypoint::with_player(|pl| {
                    let Some(pl) = pl else { return };
                    for w in store.all().iter().filter(|w| pl.sees(w)) {
                        let dx = (w.x as f64 + 0.5 - pl.x) as f32 / bpu;
                        let dz = (w.z as f64 + 0.5 - pl.z) as f32 / bpu;
                        let (mut x, mut y) = (dx * cos + dz * sin, -dx * sin + dz * cos);
                        let r = radius - 4.0;
                        if round {
                            let d = (x * x + y * y).sqrt();
                            if d > r {
                                x *= r / d;
                                y *= r / d;
                            }
                        } else {
                            x = x.clamp(-r, r);
                            y = y.clamp(-r, r);
                        }
                        canvas.shape(w.icon, center + x, center + y, 7.0, w.argb());
                    }
                })
            });
        }

        // The player: an arrow pointing up while the map turns; north up, the arrow can't turn
        // (the canvas draws no rotated shapes), so a dot marks the way they face.
        let ink = th.tone(Tone::Text);
        if turn {
            canvas.shape(Shape::Arrow, center, center, m::ARROW, ink);
        } else {
            let [fx, fy] = m::facing(heading);
            canvas.shape(Shape::Circle, center, center, 6.0, ink);
            canvas.shape(Shape::Circle, center + fx * 5.0, center + fy * 5.0, 3.0, ink);
        }

        // Every badge, then every letter: one layer each.
        if s.bool("compass") {
            for i in 0..4 {
                let [x, y] = m::compass_point(i, rotation, radius - 1.0, round);
                canvas.round(center + x - m::BADGE, center + y - m::BADGE, 2.0 * m::BADGE, 2.0 * m::BADGE, m::BADGE, th.background);
            }
            for (i, letter) in m::LETTERS.iter().enumerate() {
                let [x, y] = m::compass_point(i, rotation, radius - 1.0, round);
                let argb = th.tone(if i == 0 { Tone::Accent } else { Tone::Text });
                let w = canvas.text_width(letter);
                canvas.p.push(center + x, center + y, 0.75, |p| {
                    p.text(letter, -(w / 2.0).round(), -(m::LINE / 2.0).floor() + 1.0, argb, true);
                });
            }
        }

        // "x, y, z" on a tag below; in the HUD editor without a world, a sample.
        if coordinates {
            let at = player.map_or([0, 64, 0], |p| p.0.map(|v| v.floor() as i32));
            m::with_text(at, |text| {
                let w = canvas.text_width(text) + 10.0;
                let h = m::LINE + 3.0;
                let left = ((size - w) / 2.0).floor();
                let top = size + m::COORDINATES_GAP;
                canvas.fill(left, top, w, h, Tone::Background);
                canvas.text(text, left + 5.0, top + 2.0, Tone::Text);
            });
        }
        Some([size, m::height(coordinates)])
    });
}
