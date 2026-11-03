// Every shown box's lines, near the player.
fn incari_hitboxes_lines(
    game: &crate::game::Game<'_>,
    partial: f32,
    lines: &mut Vec<crate::game::native_hooks::NativeLine>,
) {
    use crate::game::native_hooks::NativeLine;
    use crate::game::{incari_client as c, incari_hitboxes as m};
    let Some((color, target_color, width, show, eye, look)) = c::settings(m::ID, |s| {
        let color = m::argb(s.color("color"), s.int("opacity"));
        let target = if s.bool("target-colored") {
            m::argb(s.color("target-color"), s.int("opacity"))
        } else {
            color
        };
        (
            color,
            target,
            m::width(s.float("width")),
            s.choice("show"),
            s.bool("eye-line"),
            s.bool("look-line"),
        )
    }) else {
        return;
    };
    let Some(p) = game.native_player() else {
        return;
    };
    let target = (target_color != color)
        .then(|| game.native_targeted_entity())
        .flatten();
    let (eye_argb, look_argb) = (
        m::with_alpha_of(color, m::EYE),
        m::with_alpha_of(color, m::LOOK),
    );
    let line = |(start, end): ([f64; 3], [f64; 3]), argb| NativeLine {
        start,
        end,
        argb,
        width,
    };
    for n in 0..game.native_entity_count() {
        let Some(e) = game.native_entity_at(n, partial) else {
            continue;
        };
        let near = (e.min[0] - p.x).powi(2) + (e.min[1] - p.y).powi(2) + (e.min[2] - p.z).powi(2);
        if !m::shown(show, e.player) || near > m::RANGE * m::RANGE {
            continue;
        }
        let argb = if target == Some(e.id) {
            target_color
        } else {
            color
        };
        lines.extend(
            m::edges(e.min, e.max)
                .into_iter()
                .map(|edge| line(edge, argb)),
        );
        if eye {
            lines.extend(
                m::eye_edges(e.min, e.max, e.eye_y)
                    .into_iter()
                    .map(|edge| line(edge, eye_argb)),
            );
        }
        if look {
            let from = [
                (e.min[0] + e.max[0]) / 2.0,
                e.eye_y,
                (e.min[2] + e.max[2]) / 2.0,
            ];
            lines.extend(
                m::look_arrow(from, e.yaw, e.pitch)
                    .into_iter()
                    .map(|edge| line(edge, look_argb)),
            );
        }
    }
}
