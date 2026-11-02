// Incari's Weather Changer for the Rust build (ADR 0068, 0069), as features/weatherchanger does
// it: the weather and time of day on your screen only; the server's go on underneath. Its settings
// are the module's in `incari_core::modules` (the Java ids). Horizon snows only where the biome is
// cold, so Snow is rain's levels: snow there, rain elsewhere. No game types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "weatherchanger";

/// The rain and thunder levels for the `weather` setting's key, or none to keep the world's.
pub fn weather(weather: &str) -> Option<(f32, f32)> {
    match weather {
        "clear" => Some((0.0, 0.0)),
        "rain" | "snow" => Some((1.0, 0.0)),
        "thunder" => Some((1.0, 1.0)),
        _ => None,
    }
}

/// The day time showing `hour` o'clock on the day `day_time` is in, so the moon keeps its phase.
/// The game's day starts at 06:00 (Java's `ticks`).
pub fn held_time(day_time: i64, hour: i32) -> i64 {
    let ticks = i64::from(hour - 6).rem_euclid(24) * 1000;
    day_time - day_time.rem_euclid(24000) + ticks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_the_sky_by_choice() {
        assert_eq!(weather("clear"), Some((0.0, 0.0)));
        assert_eq!(weather("world"), None);
        assert_eq!(weather("thunder"), Some((1.0, 1.0)));
        assert_eq!(weather("snow"), Some((1.0, 0.0)));
    }

    #[test]
    fn holds_the_hour_on_the_same_day() {
        assert_eq!(held_time(0, 12), 6000);
        assert_eq!(held_time(24000 * 3 + 500, 6), 72000);
        assert_eq!(held_time(100, 0), 18000);
        assert_eq!(held_time(-100, 12), -24000 + 6000);
    }
}
