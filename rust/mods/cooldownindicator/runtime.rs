// Incari's Cooldown Indicator for the Rust build (ADR 0068, 0069), as features/cooldownindicator
// does it: the attack cooldown as a bar, a ring round the crosshair or a percentage, hidden once
// charged unless the crosshair is on a target. Its settings and HUD elements ("bar", "ring") are
// the module's in `incari_core::modules`. Horizon can't hold the game's own Attack Indicator off,
// so the player turns it off in Options. No game types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "cooldownindicator";

/// The ring's thickness, in GUI units.
pub const RING_WIDTH: f32 = 1.5;

/// The charge the HUD editor shows.
pub const SAMPLE: f32 = 0.6;

/// The widest the percentage says.
pub const WIDEST: &str = "100%";

/// The charge to draw, or None to draw nothing: `strength` the game's (0 to 1), `hide` the
/// "hide-when-ready" setting, `on_target` "ready-on-target" with the crosshair on a mob or player.
pub fn charge(strength: f32, hide: bool, on_target: bool, editing: bool) -> Option<f32> {
    let strength = strength.min(1.0);
    if strength >= 1.0 && hide && !on_target {
        return editing.then_some(SAMPLE);
    }
    Some(strength)
}

/// Whether the charge counts as charged (the Positive tone, or the "ready-color").
pub fn ready(charge: f32) -> bool {
    charge >= 1.0
}

/// The bar's filled width.
pub fn filled(width: f32, charge: f32) -> f32 {
    (width * charge.clamp(0.0, 1.0)).round()
}

// Hot path, don't allocate: "73%" made again only when the percentage changes.
thread_local! {
    static TEXT: std::cell::RefCell<(i32, String)> = const { std::cell::RefCell::new((-1, String::new())) };
}

/// Calls `f` with "60%" for `charge`.
pub fn with_percent<R>(charge: f32, f: impl FnOnce(&str) -> R) -> R {
    let percent = (charge * 100.0).round() as i32;
    TEXT.with(|cell| {
        let mut cached = cell.borrow_mut();
        if cached.0 != percent {
            *cached = (percent, format!("{percent}%"));
        }
        f(&cached.1)
    })
}

/// The arc of a ring `radius` to its outer edge and `width` thick, centred on (`radius`,
/// `radius`), from straight up `fraction` of the way round clockwise: as rectangles `[x, y, w, h]`
/// of `step` (a screen pixel) across, for `fill`. Allocation-free.
pub fn arc(radius: f32, width: f32, fraction: f32, step: f32, mut fill: impl FnMut([f32; 4])) {
    use std::f32::consts::FRAC_PI_2;
    let fraction = fraction.clamp(0.0, 1.0);
    if radius <= 0.0 || width <= 0.0 || step <= 0.0 || fraction <= 0.0 {
        return;
    }
    let (outer, inner) = (radius, (radius - width).max(0.0));
    // Clockwise from up, on screen (y down): up, right, down, left.
    const DIRS: [[f32; 2]; 4] = [[0.0, -1.0], [1.0, 0.0], [0.0, 1.0], [-1.0, 0.0]];
    let angle = fraction * 4.0 * FRAC_PI_2;
    let strips = (outer / step).ceil() as usize;
    let span = |at: f32| {
        (
            (inner * inner - at * at).max(0.0).sqrt(),
            (outer * outer - at * at).max(0.0).sqrt(),
        )
    };
    for q in 0..4 {
        let phi = angle - q as f32 * FRAC_PI_2;
        if phi <= 0.0 {
            break;
        }
        let (s, t) = (DIRS[q], DIRS[(q + 1) % 4]);
        // A rectangle `a0..a1` along `s` and `b0..b1` along `t`, from the centre.
        let mut rect = |a0: f32, a1: f32, b0: f32, b1: f32| {
            if a1 <= a0 || b1 <= b0 {
                return;
            }
            let p = |a: f32, b: f32| [radius + a * s[0] + b * t[0], radius + a * s[1] + b * t[1]];
            let (p0, p1) = (p(a0, b0), p(a1, b1));
            let (x, y) = (p0[0].min(p1[0]), p0[1].min(p1[1]));
            fill([x, y, (p0[0] - p1[0]).abs(), (p0[1] - p1[1]).abs()]);
        };
        let full = phi >= FRAC_PI_2;
        // Below 45 degrees the edge is steep across `s`: strips along `s`, else along `t`, so a
        // strip's error stays under a step.
        let along_s = full || phi <= FRAC_PI_2 / 2.0;
        for n in 0..strips {
            let (c0, c1) = (n as f32 * step, ((n + 1) as f32 * step).min(outer));
            let mid = (c0 + c1) / 2.0;
            let (lo, hi) = span(mid);
            if along_s {
                let limit = if full { hi } else { mid * phi.tan() };
                rect(c0, c1, lo, hi.min(limit));
            } else {
                let start = mid / phi.tan();
                rect(lo.max(start), hi, c0, c1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_when_charged_as_the_java_module_does() {
        assert_eq!(charge(1.0, true, false, false), None);
        assert_eq!(charge(1.0, true, false, true), Some(SAMPLE));
        assert_eq!(charge(1.2, true, true, false), Some(1.0));
        assert_eq!(charge(1.0, false, false, false), Some(1.0));
        assert_eq!(charge(0.4, true, false, false), Some(0.4));
        assert!(ready(1.0) && !ready(0.99));
        assert_eq!(filled(24.0, 0.5), 12.0);
        assert_eq!(with_percent(0.734, str::to_owned), "73%");
        assert_eq!(with_percent(0.6, str::to_owned), "60%");
    }

    fn area(fraction: f32) -> (f32, [f32; 4]) {
        let (mut total, mut bounds) = (0.0, [f32::MAX, f32::MAX, f32::MIN, f32::MIN]);
        arc(10.0, 1.5, fraction, 0.5, |[x, y, w, h]| {
            total += w * h;
            bounds = [
                bounds[0].min(x),
                bounds[1].min(y),
                bounds[2].max(x + w),
                bounds[3].max(y + h),
            ];
        });
        (total, bounds)
    }

    #[test]
    fn fills_the_ring_as_far_round_as_charged() {
        let ring = std::f32::consts::PI * (100.0 - 8.5f32.powi(2));
        let (full, bounds) = area(1.0);
        assert!((full - ring).abs() / ring < 0.05, "{full} {ring}");
        assert_eq!(bounds, [0.0, 0.0, 20.0, 20.0]);
        let (quarter, bounds) = area(0.25);
        assert!((quarter - ring / 4.0).abs() / ring < 0.02);
        // The top right quarter: right of the centre, above it.
        assert!(bounds[0] >= 10.0 && bounds[3] <= 10.0);
        for f in [0.1, 0.3, 0.6, 0.9] {
            let (part, _) = area(f);
            assert!(
                (part - ring * f).abs() / ring < 0.03,
                "{f}: {part} of {ring}"
            );
        }
        assert_eq!(area(0.0).0, 0.0);
    }
}
