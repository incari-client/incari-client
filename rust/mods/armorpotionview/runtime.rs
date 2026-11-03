// Incari's Armor & Potion View for the Rust build (ADR 0068, 0069), as features/armorpotionview
// does it: two HUD elements, placed apart. One shows the armor you wear (and what you hold), each
// icon with the game's durability bar and its durability beside it; the other lists your active
// effects with their level and the time left. Its settings and HUD elements are the module's in
// `incari_core::modules` (the Java ids). No game types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "armorpotionview";
/// Its HUD elements' ids.
pub const ARMOR: &str = "armor";
pub const EFFECTS: &str = "effects";

/// Helmet first, then the held item.
pub const SLOTS: [&str; 5] = ["head", "chest", "legs", "feet", "mainhand"];
/// The held item's place in `SLOTS`.
pub const HELD: usize = 4;

/// An item's icon.
pub const ITEM_ICON: f32 = 16.0;
/// Between an item's icon and its text.
pub const ITEM_TEXT_GAP: f32 = 2.0;
/// Between armor rows (vertical).
pub const ROW_GAP: f32 = 1.0;
/// Between armor cells (horizontal).
pub const CELL_GAP: f32 = 4.0;

/// An effect's icon.
pub const EFFECT_ICON: f32 = 18.0;
pub const EFFECT_TEXT_GAP: f32 = 3.0;
pub const EFFECT_ROW_GAP: f32 = 2.0;
/// Ticks left from which the time turns amber: ten seconds.
pub const ENDING: i32 = 200;
/// The most effects listed.
pub const MAX_EFFECTS: usize = 32;

/// The text's tone (`incari_core::theme::Tone`'s of the same names).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Text,
    Muted,
    Positive,
    Warning,
    Negative,
}

/// An item as the armor element shows it: its id, its wear and the most it takes (0 for one that
/// doesn't wear).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item<'a> {
    pub id: &'a str,
    pub damage: u32,
    pub max: u32,
}

/// The Java sample player's: diamond armor at a few levels of wear and a sword, for the HUD editor
/// when you wear nothing.
pub const SAMPLE_ITEMS: [Item<'static>; 5] = [
    Item {
        id: "minecraft:diamond_helmet",
        damage: 36,
        max: 363,
    },
    Item {
        id: "minecraft:diamond_chestplate",
        damage: 185,
        max: 528,
    },
    Item {
        id: "minecraft:diamond_leggings",
        damage: 297,
        max: 495,
    },
    Item {
        id: "minecraft:diamond_boots",
        damage: 365,
        max: 429,
    },
    Item {
        id: "minecraft:diamond_sword",
        damage: 312,
        max: 1561,
    },
];

/// An effect as the effects element shows it: its id (vanilla's without its namespace), its
/// amplifier (0 is level I), its ticks left (-1 for none) and whether it helps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Effect {
    pub id: &'static str,
    pub amplifier: i32,
    pub duration: i32,
    pub beneficial: bool,
}

/// The Java sample player's effects, for the HUD editor when you have none.
pub const SAMPLE_EFFECTS: [Effect; 3] = [
    Effect {
        id: "speed",
        amplifier: 1,
        duration: 1660,
        beneficial: true,
    },
    Effect {
        id: "strength",
        amplifier: 0,
        duration: 3600,
        beneficial: true,
    },
    Effect {
        id: "fire_resistance",
        amplifier: 0,
        duration: 160,
        beneficial: true,
    },
];

/// Whole percent left, rounded down, but 1 while anything's left: 0% is broken.
pub fn percent(remaining: u32, max: u32) -> u32 {
    if max == 0 || remaining == 0 {
        return 0;
    }
    (u64::from(remaining) * 100 / u64::from(max)).max(1) as u32
}

/// "86%", "312", "312/363", or nothing for the bar alone, by the `durability` setting's key.
pub fn format(durability: &str, remaining: u32, max: u32) -> String {
    match durability {
        "remaining" => remaining.to_string(),
        "remaining_of_max" => format!("{remaining}/{max}"),
        "bar" => String::new(),
        _ => format!("{}%", percent(remaining, max)),
    }
}

/// Durability's tone: green, amber below half, red below a fifth; the theme's text when not
/// coloured or for an item that doesn't wear.
pub fn wear(item: Item<'_>, colors: bool) -> Tone {
    if !colors || item.max == 0 {
        return Tone::Text;
    }
    let (remaining, max) = (
        u64::from(item.max.saturating_sub(item.damage)),
        u64::from(item.max),
    );
    if remaining * 5 < max {
        Tone::Negative
    } else if remaining * 2 < max {
        Tone::Warning
    } else {
        Tone::Positive
    }
}

/// The game's durability bar on an item's icon (`ItemStack.getBarWidth`/`getBarColor`): its width
/// of 13 and its RGB, None while the item is unworn.
pub fn bar(item: Item<'_>) -> Option<(f32, u32)> {
    if item.max == 0 || item.damage == 0 {
        return None;
    }
    let (damage, max) = (item.damage as f32, item.max as f32);
    let width = (13.0 - damage * 13.0 / max + 0.5).floor().clamp(0.0, 13.0);
    let left = ((max - damage) / max).max(0.0);
    Some((width, hsv(left / 3.0)))
}

/// `Mth.hsvToRgb(hue, 1, 1)`.
fn hsv(hue: f32) -> u32 {
    let h = (hue * 6.0) as i32 % 6;
    let f = hue * 6.0 - (hue * 6.0).floor();
    let (q, t) = (1.0 - f, f);
    let (r, g, b) = match h {
        0 => (1.0, t, 0.0),
        1 => (q, 1.0, 0.0),
        2 => (0.0, 1.0, t),
        3 => (0.0, q, 1.0),
        4 => (t, 0.0, 1.0),
        _ => (1.0, 0.0, q),
    };
    let byte = |v: f32| ((v * 255.0) as u32).min(255);
    byte(r) << 16 | byte(g) << 8 | byte(b)
}

/// " II" for amplifier 1; nothing for level I; numbers past X.
pub fn level(amplifier: i32) -> String {
    const NUMERALS: [&str; 10] = [
        "", " II", " III", " IV", " V", " VI", " VII", " VIII", " IX", " X",
    ];
    usize::try_from(amplifier)
        .ok()
        .and_then(|a| NUMERALS.get(a))
        .map_or_else(|| format!(" {}", amplifier + 1), |n| (*n).to_owned())
}

/// "0:08", "1:23", "1:02:03".
pub fn clock(seconds: i32) -> String {
    let (hours, minutes, secs) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes}:{secs:02}")
    }
}

/// An effect's time left: "1:23", "∞" for one without end.
pub fn time(duration: i32) -> String {
    if duration < 0 {
        "∞".to_owned()
    } else {
        clock(duration / 20)
    }
}

/// The time's tone: amber in its last ten seconds.
pub fn time_tone(duration: i32) -> Tone {
    if (0..ENDING).contains(&duration) {
        Tone::Warning
    } else {
        Tone::Muted
    }
}

/// The name's tone: helpful in green, harmful in red, when coloured by kind.
pub fn name_tone(effect: Effect, by_kind: bool) -> Tone {
    match (by_kind, effect.beneficial) {
        (false, _) => Tone::Text,
        (true, true) => Tone::Positive,
        (true, false) => Tone::Negative,
    }
}

/// Effect `id`'s translation key: `effect.minecraft.speed`, a mod's `effect.<ns>.<path>`.
pub fn name_key(id: &str) -> String {
    match id.split_once(':') {
        Some((ns, path)) => format!("effect.{ns}.{path}"),
        None => format!("effect.minecraft.{id}"),
    }
}

/// "Jump Boost" for `jump_boost`: an effect's name when the language has none.
pub fn fallback_name(id: &str) -> String {
    let path = id.rsplit(':').next().unwrap_or(id);
    let mut name = String::with_capacity(path.len());
    for word in path.split('_').filter(|w| !w.is_empty()) {
        if !name.is_empty() {
            name.push(' ');
        }
        let mut c = word.chars();
        if let Some(first) = c.next() {
            name.extend(first.to_uppercase());
            name.push_str(c.as_str());
        }
    }
    name
}

/// What a row's text was made from, and the text.
type Cached<K> = (Option<K>, String);
/// An armor row's text, by the wear, the most and the format it was made for.
type ItemText = Cached<(u32, u32, &'static str)>;

// Hot path, don't allocate: each row's text is made again only when what it says changes.
thread_local! {
    static ITEM_TEXT: std::cell::RefCell<[ItemText; 5]> =
        std::cell::RefCell::new(Default::default());
    static NAMES: std::cell::RefCell<[Cached<(usize, i32)>; MAX_EFFECTS]> =
        std::cell::RefCell::new(std::array::from_fn(|_| (None, String::new())));
    static TIMES: std::cell::RefCell<[Cached<i32>; MAX_EFFECTS]> =
        std::cell::RefCell::new(std::array::from_fn(|_| (None, String::new())));
    static ICONS: std::cell::RefCell<[Cached<usize>; MAX_EFFECTS]> =
        std::cell::RefCell::new(std::array::from_fn(|_| (None, String::new())));
}

/// Calls `f` with armor row `row`'s durability text in `durability`'s format, None when nothing is
/// written beside it (an item that doesn't wear, or the bar alone).
pub fn with_item_text<R>(
    row: usize,
    item: Item<'_>,
    durability: &'static str,
    f: impl FnOnce(Option<&str>) -> R,
) -> R {
    if item.max == 0 || durability == "bar" {
        return f(None);
    }
    ITEM_TEXT.with(|cell| {
        let mut rows = cell.borrow_mut();
        let slot = &mut rows[row.min(4)];
        let key = (item.damage, item.max, durability);
        if slot.0 != Some(key) {
            *slot = (
                Some(key),
                format(durability, item.max.saturating_sub(item.damage), item.max),
            );
        }
        f(Some(&slot.1))
    })
}

/// Calls `f` with effect row `row`'s "Speed II", its name from `translate` (the key to the
/// language's name), made again only when the row's effect changes. Ids are the game's static
/// strings: the address names one.
pub fn with_name<R>(
    row: usize,
    effect: Effect,
    translate: impl FnOnce(&str) -> String,
    f: impl FnOnce(&str) -> R,
) -> R {
    NAMES.with(|cell| {
        let mut rows = cell.borrow_mut();
        let slot = &mut rows[row.min(MAX_EFFECTS - 1)];
        let key = (effect.id.as_ptr() as usize, effect.amplifier);
        if slot.0 != Some(key) {
            let lang = name_key(effect.id);
            let mut name = translate(&lang);
            if name.is_empty() || name == lang {
                name = fallback_name(effect.id);
            }
            name.push_str(&level(effect.amplifier));
            *slot = (Some(key), name);
        }
        f(&slot.1)
    })
}

/// Calls `f` with effect row `row`'s time left, made again only when the second changes.
pub fn with_time<R>(row: usize, duration: i32, f: impl FnOnce(&str) -> R) -> R {
    let seconds = if duration < 0 { -1 } else { duration / 20 };
    TIMES.with(|cell| {
        let mut rows = cell.borrow_mut();
        let slot = &mut rows[row.min(MAX_EFFECTS - 1)];
        if slot.0 != Some(seconds) {
            *slot = (Some(seconds), time(duration));
        }
        f(&slot.1)
    })
}

/// Calls `f` with effect `id`'s icon sprite, `mob_effect/<id>`, for row `row`.
pub fn with_icon<R>(row: usize, id: &'static str, f: impl FnOnce(&str) -> R) -> R {
    ICONS.with(|cell| {
        let mut rows = cell.borrow_mut();
        let slot = &mut rows[row.min(MAX_EFFECTS - 1)];
        let key = id.as_ptr() as usize;
        if slot.0 != Some(key) {
            *slot = (Some(key), format!("mob_effect/{id}"));
        }
        f(&slot.1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(damage: u32, max: u32) -> Item<'static> {
        Item {
            id: "minecraft:diamond_helmet",
            damage,
            max,
        }
    }

    #[test]
    fn writes_durability_as_the_java_module_does() {
        assert_eq!(format("percent", 217, 250), "86%");
        assert_eq!(format("remaining", 217, 250), "217");
        assert_eq!(format("remaining_of_max", 217, 250), "217/250");
        assert_eq!(format("bar", 217, 250), "");
        assert_eq!(percent(1, 1000), 1);
        assert_eq!(percent(0, 1000), 0);
        assert_eq!(
            with_item_text(0, item(1, 4), "percent", |t| t.map(str::to_owned)),
            Some("75%".into())
        );
        assert!(with_item_text(0, item(1, 4), "bar", |t| t.is_none()));
        assert!(with_item_text(1, item(0, 0), "percent", |t| t.is_none()));
    }

    #[test]
    fn colours_by_wear() {
        assert_eq!(wear(item(0, 100), true), Tone::Positive);
        assert_eq!(wear(item(51, 100), true), Tone::Warning);
        assert_eq!(wear(item(81, 100), true), Tone::Negative);
        assert_eq!(wear(item(81, 100), false), Tone::Text);
        assert_eq!(wear(item(0, 0), true), Tone::Text);
    }

    #[test]
    fn draws_the_games_bar() {
        assert_eq!(bar(item(0, 100)), None);
        assert_eq!(bar(item(50, 100)).map(|b| b.0), Some(7.0));
        assert_eq!(bar(item(1, 1000)), Some((13.0, 0x00FF00)));
        assert_eq!(bar(item(999, 1000)).map(|b| b.1 >> 16), Some(0xFF));
    }

    #[test]
    fn names_effects_as_the_java_module_does() {
        assert_eq!(level(0), "");
        assert_eq!(level(1), " II");
        assert_eq!(level(11), " 12");
        assert_eq!(clock(8), "0:08");
        assert_eq!(clock(83), "1:23");
        assert_eq!(clock(3723), "1:02:03");
        assert_eq!(time(-1), "∞");
        assert_eq!(time_tone(199), Tone::Warning);
        assert_eq!(time_tone(-1), Tone::Muted);
        assert_eq!(name_key("speed"), "effect.minecraft.speed");
        assert_eq!(name_key("mymod:frost_bite"), "effect.mymod.frost_bite");
        assert_eq!(fallback_name("mymod:frost_bite"), "Frost Bite");
        let speed = SAMPLE_EFFECTS[0];
        assert_eq!(
            with_name(0, speed, |_| "Speed".into(), str::to_owned),
            "Speed II"
        );
        let strength = SAMPLE_EFFECTS[1];
        assert_eq!(
            with_name(1, strength, str::to_owned, str::to_owned),
            "Strength"
        );
        assert_eq!(with_time(0, 1660, str::to_owned), "1:23");
        assert_eq!(with_icon(0, "speed", str::to_owned), "mob_effect/speed");
        assert_eq!(name_tone(speed, true), Tone::Positive);
    }
}
