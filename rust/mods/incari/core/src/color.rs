//! Colour arithmetic the look's parts share (`Panels.alpha`, `Panels.mix`, `ARGB.multiplyAlpha`):
//! colours are `0xAARRGGBB`, as the Java client's are.

/// `argb` with its alpha scaled by `fade` (0 to 1).
pub fn alpha(argb: u32, fade: f32) -> u32 {
    if fade >= 1.0 {
        return argb;
    }
    let a = ((argb >> 24) as f32 * fade.max(0.0)).round() as u32;
    a << 24 | (argb & 0xFF_FFFF)
}

/// The colour `t` of the way from `from` to `to`, each channel.
pub fn mix(from: u32, to: u32, t: f32) -> u32 {
    if t <= 0.0 {
        return from;
    }
    if t >= 1.0 {
        return to;
    }
    let channel = |shift: u32| {
        let (a, b) = ((from >> shift & 0xFF) as f32, (to >> shift & 0xFF) as f32);
        ((a + (b - a) * t).round() as u32) << shift
    };
    channel(24) | channel(16) | channel(8) | channel(0)
}

/// `rgb` with each channel scaled by `keep` (Theme.darken).
pub fn darken(rgb: u32, keep: f32) -> u32 {
    let c = |shift: u32| (((rgb >> shift & 0xFF) as f32 * keep).round() as u32).min(255) << shift;
    c(16) | c(8) | c(0)
}

/// `rgb` moved `by` (0 to 1) of the way to white (Theme.lighten).
pub fn lighten(rgb: u32, by: f32) -> u32 {
    let c = |shift: u32| {
        let v = rgb >> shift & 0xFF;
        (v + ((255 - v) as f32 * by).round() as u32) << shift
    };
    c(16) | c(8) | c(0)
}

/// How light `rgb` looks, 0 to 1 (Theme.luminance).
pub fn luminance(rgb: u32) -> f32 {
    (0.2126 * (rgb >> 16 & 0xFF) as f32
        + 0.7152 * (rgb >> 8 & 0xFF) as f32
        + 0.0722 * (rgb & 0xFF) as f32)
        / 255.0
}

/// An alpha byte from a 0 to 100 opacity, in place for `<< 24`.
pub fn opacity(percent: i32) -> u32 {
    ((percent.clamp(0, 100) as f32 * 2.55).round() as u32) << 24
}

/// `#AARRGGBB` (or `#RRGGBB`, opaque) as the Java client writes colours.
pub fn parse_hex(text: &str) -> Option<u32> {
    let hex = text.strip_prefix('#')?;
    let value = u32::from_str_radix(hex, 16).ok()?;
    match hex.len() {
        6 => Some(0xFF00_0000 | value),
        8 => Some(value),
        _ => None,
    }
}

/// `argb` as `#AARRGGBB`.
pub fn hex(argb: u32) -> String {
    format!("#{argb:08X}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fades_and_mixes_as_the_java_panels_do() {
        assert_eq!(alpha(0xFF12_3456, 0.5), 0x8012_3456);
        assert_eq!(alpha(0x2EFF_FFFF, 1.0), 0x2EFF_FFFF);
        assert_eq!(mix(0x0000_0000, 0xFFFF_FFFF, 0.5), 0x8080_8080);
        assert_eq!(mix(0x1F00_0000, 0x4700_0000, 1.0), 0x4700_0000);
    }

    #[test]
    fn reads_and_writes_the_profiles_colours() {
        assert_eq!(parse_hex("#FFA9DCFF"), Some(0xFFA9_DCFF));
        assert_eq!(parse_hex("#A9DCFF"), Some(0xFFA9_DCFF));
        assert_eq!(parse_hex("A9DCFF"), None);
        assert_eq!(hex(0xFFA9_DCFF), "#FFA9DCFF");
    }
}
