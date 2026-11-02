// Incari's Shield & Totem HUD for the Rust build (ADR 0068, 0069), as features/shieldtotem does
// it: the shield you hold with its durability ("Disabled" while an axe has it on cooldown), and how
// many totems of undying you carry, each a HUD element of its own. Its settings and HUD elements are
// the module's in `incari_core::modules` (the Java ids). No game types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "shieldtotem";
/// Its HUD elements' ids.
pub const SHIELD_ELEMENT: &str = "shield";
pub const TOTEMS_ELEMENT: &str = "totems";

pub const SHIELD: &str = "minecraft:shield";
pub const TOTEM: &str = "minecraft:totem_of_undying";

pub const ICON: f32 = 16.0;
pub const TEXT_GAP: f32 = 2.0;
pub const DISABLED: &str = "Disabled";

/// The HUD editor's totem count with no world.
pub const SAMPLE_TOTEMS: u32 = 2;

/// The text's tone (`incari_core::theme::Tone`'s of the same names).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Text,
    Positive,
    Warning,
    Negative,
}

/// A held shield: its wear, the most it takes and what's left of its cooldown (0 to 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shield {
    pub damage: u32,
    pub max: u32,
    pub cooldown: f32,
}

/// Whole percent left, rounded down, but 1 while anything's left; 100 for one that doesn't wear.
pub fn percent(shield: Shield) -> u32 {
    if shield.max == 0 {
        return 100;
    }
    let remaining = shield.max.saturating_sub(shield.damage);
    if remaining == 0 {
        return 0;
    }
    (u64::from(remaining) * 100 / u64::from(shield.max)).max(1) as u32
}

/// The shield's tone: red while disabled, else by wear (green, amber below half, red below a
/// fifth) when coloured.
pub fn shield_tone(shield: Shield, colors: bool) -> Tone {
    if shield.cooldown > 0.0 {
        return Tone::Negative;
    }
    if !colors || shield.max == 0 {
        return Tone::Text;
    }
    let (remaining, max) = (u64::from(shield.max.saturating_sub(shield.damage)), u64::from(shield.max));
    if remaining * 5 < max {
        Tone::Negative
    } else if remaining * 2 < max {
        Tone::Warning
    } else {
        Tone::Positive
    }
}

/// The game's durability bar on the shield's icon: its width of 13 and its RGB, None while unworn.
pub fn bar(shield: Shield) -> Option<(f32, u32)> {
    if shield.max == 0 || shield.damage == 0 {
        return None;
    }
    let (damage, max) = (shield.damage as f32, shield.max as f32);
    let width = (13.0 - damage * 13.0 / max + 0.5).floor().clamp(0.0, 13.0);
    let hue = ((max - damage) / max).max(0.0) / 3.0;
    // `Mth.hsvToRgb(hue, 1, 1)` for a hue from red (0) to green (1/3).
    let f = hue * 6.0;
    let byte = |v: f32| ((v * 255.0) as u32).min(255);
    let rgb = if f < 1.0 {
        0xFF_0000 | byte(f) << 8
    } else if f < 2.0 {
        byte(2.0 - f) << 16 | 0x00_FF00
    } else {
        0x00_FF00
    };
    Some((width, rgb))
}

// Hot path, don't allocate: each text made again only when its number changes.
thread_local! {
    static PERCENT: std::cell::RefCell<(Option<u32>, String)> = const { std::cell::RefCell::new((None, String::new())) };
    static COUNT: std::cell::RefCell<(Option<u32>, String)> = const { std::cell::RefCell::new((None, String::new())) };
}

/// Calls `f` with the shield's text: "Disabled" on cooldown, else "86%".
pub fn with_shield_text<R>(shield: Shield, f: impl FnOnce(&str) -> R) -> R {
    if shield.cooldown > 0.0 {
        return f(DISABLED);
    }
    let percent = percent(shield);
    PERCENT.with(|cell| {
        let mut cached = cell.borrow_mut();
        if cached.0 != Some(percent) {
            *cached = (Some(percent), format!("{percent}%"));
        }
        f(&cached.1)
    })
}

/// Calls `f` with "x3".
pub fn with_count_text<R>(count: u32, f: impl FnOnce(&str) -> R) -> R {
    COUNT.with(|cell| {
        let mut cached = cell.borrow_mut();
        if cached.0 != Some(count) {
            *cached = (Some(count), format!("x{count}"));
        }
        f(&cached.1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shield(damage: u32, cooldown: f32) -> Shield {
        Shield { damage, max: 336, cooldown }
    }

    #[test]
    fn says_what_the_java_module_says() {
        assert_eq!(with_shield_text(shield(84, 0.0), str::to_owned), "75%");
        assert_eq!(with_shield_text(shield(0, 0.5), str::to_owned), "Disabled");
        assert_eq!(with_shield_text(shield(335, 0.0), str::to_owned), "1%");
        assert_eq!(percent(Shield { damage: 0, max: 0, cooldown: 0.0 }), 100);
        assert_eq!(with_count_text(0, str::to_owned), "x0");
    }

    #[test]
    fn colours_by_wear_and_cooldown() {
        assert_eq!(shield_tone(shield(0, 0.0), true), Tone::Positive);
        assert_eq!(shield_tone(shield(200, 0.0), true), Tone::Warning);
        assert_eq!(shield_tone(shield(300, 0.0), true), Tone::Negative);
        assert_eq!(shield_tone(shield(300, 0.0), false), Tone::Text);
        assert_eq!(shield_tone(shield(0, 0.1), false), Tone::Negative);
    }

    #[test]
    fn draws_the_games_bar() {
        assert_eq!(bar(shield(0, 0.0)), None);
        assert_eq!(bar(shield(168, 0.0)), Some((7.0, 0xFF_FF00)));
        assert_eq!(bar(Shield { damage: 1, max: 1000, cooldown: 0.0 }), Some((13.0, 0x00_FF00)));
    }
}
