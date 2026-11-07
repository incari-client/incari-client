//! Every Incari module, declared as the Java modules declare themselves (`features/*`): the same
//! ids, names, settings, defaults and HUD elements, so a profile the Java client wrote reads and
//! writes the same here. What each module does lives with the game side's hooks.
use crate::framework::hud::{Anchor, HudElement};
use crate::framework::module::{Category, Module};
use crate::framework::setting::{self as s, Shown};
use crate::input::Key;

/// A module with its toggle key described as the Java one's: "Turns <name> on and off."
macro_rules! module {
    ($id:literal, $name:literal, $description:literal, $category:expr) => {{
        let mut m = Module::new($id, $name, $description, $category);
        if let Some(toggle) = m.settings.get_mut("toggle") {
            toggle.description = concat!("Turns ", $name, " on and off.");
        }
        m
    }};
}

/// Hold or Toggle: Toggle Sprint's sneak key, and Freelook's and Zoom's keys.
pub const HOLD_TOGGLE: &[(&str, &str)] = &[("hold", "Hold"), ("toggle", "Toggle")];

/// Toggle Sprint's sprint key.
pub const SPRINT_MODES: &[(&str, &str)] =
    &[("hold", "Hold"), ("toggle", "Toggle"), ("always", "Always")];

/// CPS's buttons.
pub const CPS_BUTTONS: &[(&str, &str)] = &[
    ("both", "Left and right"),
    ("left", "Left"),
    ("right", "Right"),
];

/// FPS Display's formats.
pub const FPS_FORMATS: &[(&str, &str)] = &[
    ("after", "120 FPS"),
    ("before", "FPS: 120"),
    ("number", "120"),
];

/// Coordinates' layouts.
pub const COORDINATE_LAYOUTS: &[(&str, &str)] = &[("vertical", "Lines"), ("line", "One line")];

/// Armor & Potion View's durability.
pub const DURABILITY: &[(&str, &str)] = &[
    ("percent", "Percent"),
    ("remaining", "Uses left"),
    ("remaining_of_max", "Uses left of max"),
    ("bar", "Bar only"),
];

/// Armor & Potion View's armor layouts.
pub const ARMOR_LAYOUTS: &[(&str, &str)] =
    &[("vertical", "Vertical"), ("horizontal", "Horizontal")];

/// Armor & Potion View's side for effect icons.
pub const SIDES: &[(&str, &str)] = &[("left", "Left"), ("right", "Right")];

/// The minimap's frames.
pub const MAP_SHAPES: &[(&str, &str)] = &[("round", "Round"), ("square", "Square")];

/// Blocks across the minimap at each zoom, out to in.
const ZOOMS: [i32; 7] = [256, 192, 128, 96, 64, 48, 32];

/// Cooldown Indicator's styles.
pub const COOLDOWN_STYLES: &[(&str, &str)] =
    &[("bar", "Bar"), ("ring", "Ring"), ("number", "Percent")];

/// The particle bursts of Hit & Kill Effects (framework's `Effect`).
pub const EFFECTS: &[(&str, &str)] = &[
    ("crits", "Crits"),
    ("magic", "Magic"),
    ("sparks", "Sparks"),
    ("hearts", "Hearts"),
    ("flames", "Flames"),
    ("snow", "Snow"),
    ("firework", "Firework"),
    ("souls", "Souls"),
    ("smoke", "Smoke"),
    ("totem", "Totem"),
];

/// Which entities get a hitbox (framework's `Hitboxes.Show`).
/// How No Render draws items on the ground.
pub const NO_RENDER_ITEMS: &[(&str, &str)] =
    &[("shown", "Shown"), ("still", "Still"), ("hidden", "Hidden")];

pub const HITBOX_SHOW: &[(&str, &str)] = &[
    ("players", "Players"),
    ("living", "Players and mobs"),
    ("all", "Everything"),
];

/// Where Freelook's camera goes.
pub const FREELOOK_VIEWS: &[(&str, &str)] = &[
    ("back", "Behind"),
    ("front", "In front"),
    ("first", "First person"),
];

/// How the view turns while zoomed.
pub const ZOOM_TURNING: &[(&str, &str)] = &[
    ("smooth", "Smooth"),
    ("slower", "Slower"),
    ("usual", "As usual"),
];

/// The crosshair's ready-made designs.
pub const CROSSHAIR_PRESETS: &[(&str, &str)] = &[
    ("custom", "Custom"),
    ("classic", "Classic"),
    ("competitive", "Competitive"),
    ("small", "Small"),
    ("dot", "Dot"),
    ("circle", "Circle"),
    ("t_shape", "T"),
];

/// What the crosshair draws.
pub const CROSSHAIR_STYLES: &[(&str, &str)] = &[
    ("cross", "Cross"),
    ("t", "T"),
    ("circle", "Circle"),
    ("dot", "Dot"),
    ("grid", "Pixel grid"),
];

/// The crosshair grid's first drawing: a plus five cells a side with a clear middle, 15 across.
const CROSSHAIR_GRID: &[u64] = &[
    0, 0x80, 0x80, 0x80, 0x80, 0x80, 0, 0x3E3E, 0, 0x80, 0x80, 0x80, 0x80, 0x80, 0,
];

/// Weather Changer's skies.
pub const SKIES: &[(&str, &str)] = &[
    ("world", "World's own"),
    ("clear", "Clear"),
    ("rain", "Rain"),
    ("thunder", "Thunder"),
    ("snow", "Snow"),
];

/// The Theme Editor's presets.
pub const THEME_PRESETS: &[(&str, &str)] = &[
    ("custom", "Custom"),
    ("frost", "Frost"),
    ("midnight", "Midnight"),
    ("ember", "Ember"),
    ("mint", "Mint"),
    ("rose", "Rose"),
    ("mono", "Mono"),
];

/// The client's fonts (framework's `UiFont`).
pub const FONTS: &[(&str, &str)] = &[
    ("red_hat_display", "Red Hat Display"),
    ("minecraft", "Minecraft"),
    ("inter", "Inter"),
    ("nunito", "Nunito"),
    ("poppins", "Poppins"),
    ("jetbrains_mono", "JetBrains Mono"),
    ("michroma", "Michroma"),
];

/// When a waypoint's name shows.
pub const WAYPOINT_LABELS: &[(&str, &str)] = &[
    ("always", "Always"),
    ("looked_at", "When looked at"),
    ("never", "Never"),
];

/// What Schematics shows as ghosts.
pub const GHOSTS: &[(&str, &str)] = &[
    ("missing", "Missing blocks"),
    ("all", "Every block"),
    ("none", "None"),
];

/// Which of a schematic's layers are drawn.
pub const LAYERS: &[(&str, &str)] = &[
    ("all", "All"),
    ("below", "Up to a layer"),
    ("one", "One layer"),
];

fn percent(v: f64) -> String {
    format!("{}%", v.round() as i64)
}

fn pixels(v: f64) -> String {
    format!("{} px", v.round() as i64)
}

fn blocks(v: f64) -> String {
    format!("{} blocks", v.round() as i64)
}

/// Reach Display's "Hide after": "Never" or "3 s".
fn seconds_or_never(v: f64) -> String {
    match v.round() as i64 {
        0 => "Never".into(),
        n => format!("{n} s"),
    }
}

/// "06:00".
fn clock(v: f64) -> String {
    format!("{:02}:00", v.round() as i64)
}

/// The minimap's zoom as "96 blocks across".
fn blocks_across(v: f64) -> String {
    let level = (v.round() as usize).clamp(1, ZOOMS.len());
    format!("{} blocks across", ZOOMS[level - 1])
}

/// Zoom's amount: "4x", "2.5x" (Java's `Double.toString` for a fraction).
fn times(v: f64) -> String {
    if v == v.floor() {
        format!("{}x", v as i64)
    } else {
        format!("{v}x")
    }
}

/// The two hidden settings a held game option keeps (OptionHold): the player's value before the
/// module took it, and whether it holds it.
fn hold_option(m: Module, before: s::Setting, held: s::Setting) -> Module {
    m.setting(before.hidden()).setting(held.hidden())
}

/// After module `module`'s setting `id` changed in the menu: what the Java module's `onChange`
/// listeners do.
pub fn changed(system: &mut crate::framework::system::ModuleSystem, module: &str, id: &str) {
    if module == "crosshair"
        && let Some(m) = system.module_mut(module)
    {
        crate::crosshair::changed(&mut m.settings, id);
    }
}

/// Every module, in the order the Java client registers them (`Features.all`).
pub fn all() -> Vec<Module> {
    vec![
        toggle_sprint(),
        keystrokes(),
        cps(),
        fps_display(),
        reach_display(),
        coordinates(),
        armor_potion_view(),
        shield_totem(),
        saturation(),
        custom_scoreboard(),
        minimap(),
        cooldown_indicator(),
        hit_effects(),
        hitboxes(),
        fast_crystal(),
        old_animations(),
        freelook(),
        zoom(),
        crosshair(),
        low_shield(),
        short_items(),
        weather_changer(),
        fullbright(),
        custom_sky(),
        block_outline(),
        hit_color(),
        enchant_glint(),
        old_textures(),
        theme(),
        clear_chat(),
        low_fire(),
        no_hurt_cam(),
        no_render(),
        waypoints(),
        schematics(),
        shulker_tooltips(),
    ]
}

fn toggle_sprint() -> Module {
    module!(
        "togglesprint",
        "Toggle Sprint",
        "Sprint and sneak without holding the keys.",
        Category::Movement
    )
    .setting(
        s::choice("sprint", "Sprint key", SPRINT_MODES, "toggle").described(
            "Toggle: press once to sprint, again to stop. Always: sprint whenever you can.",
        ),
    )
    .setting(
        s::choice("sneak", "Sneak key", HOLD_TOGGLE, "hold")
            .described("Toggle: press once to sneak, again to stop."),
    )
    .setting(
        s::bool("show-status", "Show status", true)
            .described("Shows on the HUD when sprint or sneak is toggled or held."),
    )
    .setting(
        s::bool("custom-color", "Custom text color", false)
            .described("Off: the status follows the client's theme.")
            .shown(Shown::When("show-status", true)),
    )
    .setting(
        s::color("color", "Text color", 0xFFFFFF, false)
            .shown(Shown::Both("show-status", "custom-color")),
    )
    .element(HudElement::new("status", "Status", Anchor::TopLeft, 2, 2))
}

fn keystrokes() -> Module {
    module!(
        "keystrokes",
        "Keystrokes",
        "Shows your movement keys and mouse buttons, lit while you hold them.",
        Category::Hud
    )
    .setting(
        s::int("key-size", "Key size", 14, 30, 20)
            .described("Each key's width and height, in GUI units."),
    )
    .setting(
        s::bool("show-mouse", "Mouse buttons", true)
            .described("Shows the attack and use buttons below the movement keys."),
    )
    .setting(
        s::bool("show-cps", "Clicks a second", true)
            .described("Shows clicks per second under each mouse button.")
            .shown(Shown::When("show-mouse", true)),
    )
    .setting(
        s::bool("show-space", "Space bar", true)
            .described("Shows the jump key as a bar at the bottom."),
    )
    .setting(
        s::bool("show-sneak-sprint", "Sneak and sprint", false)
            .described("Adds the sneak and sprint keys as a row of their own."),
    )
    .setting(
        s::bool("custom-color", "Custom pressed color", false)
            .described("Off: held keys light up in the theme's accent."),
    )
    .setting(
        s::color("pressed-color", "Pressed color", 0xA9DCFF, false)
            .shown(Shown::When("custom-color", true)),
    )
    .element(HudElement::new("keys", "Keys", Anchor::TopRight, 4, 120))
}

fn cps() -> Module {
    module!(
        "cps",
        "CPS",
        "Shows how many times a second you click.",
        Category::Hud
    )
    .setting(
        s::choice("buttons", "Buttons", CPS_BUTTONS, "both")
            .described("Which mouse buttons' clicks are counted."),
    )
    .element(HudElement::text("cps", "CPS", Anchor::TopLeft, 2, 30))
}

fn fps_display() -> Module {
    module!(
        "fpsdisplay",
        "FPS Display",
        "Shows how many frames a second the game draws.",
        Category::Hud
    )
    .setting(s::choice("format", "Format", FPS_FORMATS, "after").described("How the FPS is shown."))
    .setting(
        s::bool("speed-colors", "Color by speed", false)
            .described("Green from 60 frames a second, amber from 30, red below."),
    )
    .element(HudElement::text("fps", "FPS", Anchor::TopLeft, 2, 14))
}

fn reach_display() -> Module {
    module!(
        "reachdisplay",
        "Reach Display",
        "Shows how far your last hit reached.",
        Category::Combat
    )
    .setting(s::int("decimals", "Decimals", 1, 3, 2).described("How many decimal places to show."))
    .setting(
        s::int("hide-after", "Hide after", 0, 10, 3)
            .formatted(seconds_or_never)
            .described("How long the last hit's reach shows; Never keeps it until the next."),
    )
    .setting(
        s::bool("far-color", "Mark long hits", false)
            .described("Hits past 3 blocks show in amber."),
    )
    .element(HudElement::text("reach", "Reach", Anchor::TopLeft, 2, 46))
}

fn coordinates() -> Module {
    module!(
        "coordinates",
        "Coordinates",
        "Shows where you are, which way you face and the biome.",
        Category::Hud
    )
    .setting(
        s::choice("layout", "Layout", COORDINATE_LAYOUTS, "vertical")
            .described("X, Y and Z on lines of their own, or on one."),
    )
    .setting(
        s::bool("show-facing", "Facing", true)
            .described("Which way you face, with the axis it runs along."),
    )
    .setting(s::bool("show-biome", "Biome", true).described("The biome you stand in."))
    .setting(s::bool("show-other", "Other dimension", true).described(
        "In the Nether, where you'd be in the Overworld; in the Overworld, in the Nether.",
    ))
    .setting(s::bool("background", "Background", true).described("Draws a panel behind the text."))
    .element(HudElement::new(
        "position",
        "Coordinates",
        Anchor::TopLeft,
        2,
        62,
    ))
}

fn armor_potion_view() -> Module {
    module!(
        "armorpotionview",
        "Armor & Potion View",
        "Shows your armor's durability and your active effects on the HUD.",
        Category::Hud
    )
    .setting(
        s::bool("show-armor", "Show armor", true)
            .described("Shows the armor you wear and its durability."),
    )
    .setting(
        s::bool("show-held", "Show held item", true)
            .described("Adds the item in your main hand below the armor.")
            .shown(Shown::When("show-armor", true)),
    )
    .setting(
        s::choice("durability", "Durability", DURABILITY, "percent")
            .described("How durability is shown beside each piece.")
            .shown(Shown::When("show-armor", true)),
    )
    .setting(
        s::bool("wear-colors", "Color by wear", true)
            .described("Durability turns amber below half and red below a fifth.")
            .shown(Shown::WhenNot("show-armor", "durability", "bar")),
    )
    .setting(
        s::choice("layout", "Armor layout", ARMOR_LAYOUTS, "vertical")
            .shown(Shown::When("show-armor", true)),
    )
    .setting(
        s::bool("show-effects", "Show effects", true)
            .described("Lists your active effects with the time they have left."),
    )
    .setting(
        s::bool("effect-names", "Effect names", true)
            .described("Off: icons and times only.")
            .shown(Shown::When("show-effects", true)),
    )
    .setting(
        s::bool("kind-colors", "Color by kind", false)
            .described("Helpful effects' names in green, harmful ones' in red.")
            .shown(Shown::Both("show-effects", "effect-names")),
    )
    .setting(
        s::choice("side", "Effect icons on the", SIDES, "left")
            .shown(Shown::When("show-effects", true)),
    )
    .element(HudElement::new("armor", "Armor", Anchor::BottomRight, 2, 2))
    .element(HudElement::new(
        "effects",
        "Effects",
        Anchor::TopLeft,
        2,
        62,
    ))
}

fn shield_totem() -> Module {
    module!(
        "shieldtotem",
        "Shield & Totem HUD",
        "Shows your shield's durability and cooldown, and the totems you carry.",
        Category::Hud
    )
    .setting(
        s::bool("show-shield", "Show shield", true)
            .described("Shows the shield you hold, its durability and when it's disabled."),
    )
    .setting(
        s::bool("wear-colors", "Color by wear", true)
            .described("Durability turns amber below half and red below a fifth.")
            .shown(Shown::When("show-shield", true)),
    )
    .setting(
        s::bool("show-totems", "Show totems", true)
            .described("Shows how many totems of undying you carry."),
    )
    .setting(
        s::bool("show-no-totems", "Warn with none", true)
            .described("Shows a red 0 when you carry none; off hides the count instead.")
            .shown(Shown::When("show-totems", true)),
    )
    // One 60-wide column right of the hotbar and a left-handed off hand's slot (its left edge 122
    // right of the middle), a slot's height up; the totems on the shield.
    .element(HudElement::new("shield", "Shield", Anchor::Bottom, 152, 25))
    .element(HudElement::new("totems", "Totems", Anchor::Bottom, 152, 43))
}

fn saturation() -> Module {
    module!(
        "saturation",
        "Saturation",
        "Shows your saturation on the hunger bar.",
        Category::Hud
    )
    .setting(
        s::color("color", "Color", 0xFFCC00, false)
            .described("The color saturated shanks are outlined in."),
    )
}

fn custom_scoreboard() -> Module {
    module!(
        "customscoreboard",
        "Custom Scoreboard",
        "Move and resize the scoreboard a server shows, with the HUD editor.",
        Category::Hud
    )
    // A GamePartElement: named after the part it takes over.
    .element(HudElement::new(
        "sidebar",
        "Scoreboard",
        Anchor::Right,
        1,
        0,
    ))
}

fn minimap() -> Module {
    module!("minimap", "Minimap", "A map of the world around you, with your waypoints on it.", Category::Hud)
        .setting(s::choice("shape", "Shape", MAP_SHAPES, "round"))
        .setting(
            s::bool("turn", "Turn with you", true)
                .described("On: the way you face is up. Off: north is up and your arrow turns."),
        )
        .setting(
            s::int("zoom", "Zoom", 1, ZOOMS.len() as i32, 4)
                .formatted(blocks_across)
                .described("How much of the world the map shows."),
        )
        .setting(s::key("zoom-in", "Zoom in key", Key::Keyboard(46)))
        .setting(s::key("zoom-out", "Zoom out key", Key::Keyboard(45)))
        .setting(
            s::key("world-map", "World map key", Key::Keyboard(16))
                .described("Opens the world map. Drag to move, scroll to zoom."),
        )
        .setting(
            s::bool("remember", "Remember explored map", true)
                .described("Keeps the land the map has seen, for each world, so the world map shows where you've been."),
        )
        .setting(s::bool("waypoints", "Show waypoints", true).described("Marks this world's waypoints on the map."))
        .setting(s::bool("compass", "Compass letters", true).described("N, E, S and W around the map's edge."))
        .setting(s::bool("coordinates", "Coordinates", true).described("Where you stand, below the map."))
        .element(HudElement::new("map", "Map", Anchor::TopRight, 4, 4))
}

fn cooldown_indicator() -> Module {
    let m = module!(
        "cooldownindicator",
        "Cooldown Indicator",
        "Replaces the attack indicator with a bar, ring or percentage.",
        Category::Combat
    )
    .setting(
        s::choice("style", "Style", COOLDOWN_STYLES, "bar")
            .described("A bar, a ring around the crosshair, or a percentage."),
    )
    .setting(s::int("bar-width", "Bar width", 8, 80, 24).shown(Shown::Is("style", "bar")))
    .setting(s::int("bar-height", "Bar height", 1, 6, 2).shown(Shown::Is("style", "bar")))
    .setting(
        s::int("ring-size", "Ring size", 6, 24, 10)
            .described("The ring's radius, in GUI units.")
            .shown(Shown::Is("style", "ring")),
    )
    .setting(
        s::bool("hide-when-ready", "Hide when charged", true)
            .described("Shows the indicator only while the attack charges."),
    )
    .setting(
        s::bool("ready-on-target", "Show charged on target", true)
            .described("Shows it full when the crosshair is on a mob or player in reach.")
            .shown(Shown::When("hide-when-ready", true)),
    )
    .setting(
        s::bool("custom-colors", "Custom colors", false)
            .described("Off: charging in the accent, charged in green."),
    )
    .setting(
        s::color("charging-color", "Charging color", 0xA9DCFF, false)
            .shown(Shown::When("custom-colors", true)),
    )
    .setting(
        s::color("ready-color", "Charged color", 0x8FE3B0, false)
            .shown(Shown::When("custom-colors", true)),
    );
    hold_option(
        m,
        s::float(
            "before-attack-indicator",
            "Attack Indicator before",
            0.0,
            2.0,
            0.01,
            1.0,
        ),
        s::bool("held-attack-indicator", "Attack Indicator held", false),
    )
    .element(HudElement::new("bar", "Cooldown", Anchor::Center, 0, 12))
    .element(HudElement::new(
        "ring",
        "Cooldown ring",
        Anchor::Center,
        0,
        0,
    ))
}

fn hit_effects() -> Module {
    module!(
        "hiteffects",
        "Hit & Kill Effects",
        "Shows particles on what you hit and a burst when it dies.",
        Category::Combat
    )
    .setting(s::bool("on-hit", "On hit", true).described("Particles on whatever you hit."))
    .setting(
        s::choice("hit", "Hit effect", EFFECTS, "magic")
            .described("What shows when you hit.")
            .shown(Shown::When("on-hit", true)),
    )
    .setting(
        s::int("hit-count", "Hit particles", 1, 20, 6)
            .described("How many particles a hit makes.")
            .shown(Shown::When("on-hit", true)),
    )
    .setting(s::bool("on-kill", "On kill", true).described("A burst when something you hit dies."))
    .setting(
        s::choice("kill", "Kill effect", EFFECTS, "firework")
            .described("What shows when something you hit dies.")
            .shown(Shown::When("on-kill", true)),
    )
    .setting(
        s::int("kill-count", "Kill particles", 5, 60, 30)
            .described("How many particles a kill makes.")
            .shown(Shown::When("on-kill", true)),
    )
    .setting(
        s::bool("kill-sound", "Kill sound", true)
            .described("Plays the effect's sound on a kill.")
            .shown(Shown::When("on-kill", true)),
    )
}

fn hitboxes() -> Module {
    module!(
        "hitboxes",
        "Hitboxes",
        "Shows the box each player or mob is hit by.",
        Category::Combat
    )
    .setting(s::color("color", "Color", 0xFFFFFF, false).described("The boxes' color."))
    .setting(
        s::int("opacity", "Opacity", 10, 100, 100)
            .formatted(percent)
            .described("How solid the boxes are."),
    )
    .setting(
        s::float("width", "Width", 1.0, 6.0, 0.5, 2.0)
            .described("How thick the lines are, in pixels."),
    )
    .setting(
        s::bool("target-colored", "Target color", true)
            .described("Colors the box your crosshair is on differently."),
    )
    .setting(
        s::color("target-color", "Target's color", 0xFF5555, false)
            .described("The color of the box your crosshair is on.")
            .shown(Shown::When("target-colored", true)),
    )
    .setting(
        s::choice("show", "Show", HITBOX_SHOW, "living").described("Which entities get a box."),
    )
    .setting(
        s::bool("eye-line", "Eye height", true).described("A red line where each one's eyes are."),
    )
    .setting(
        s::bool("look-line", "Look direction", true)
            .described("A blue arrow the way each one looks."),
    )
    .setting(s::bool("own", "Your own", false).described("Your own box too, in third person."))
}

fn old_animations() -> Module {
    module!(
        "oldanimations",
        "1.7 Animations",
        "Draws your hands and items in first person as 1.7 did.",
        Category::Visual
    )
    .setting(
        s::bool("block-hit", "Block-hit swing", true)
            .described("Attacking while an item's in use swings your arm, as 1.7 drew it."),
    )
    .setting(
        s::bool("items", "1.7 item positions", true)
            .described("Swords, tools and other flat items held as in 1.7."),
    )
    .setting(
        s::bool("rod", "1.7 fishing rod", true)
            .described("The fishing rod points the way it did in 1.7."),
    )
    .setting(
        s::bool("sword-block", "1.7 sword blocking", true)
            .described("A blocking sword takes 1.7's pose (on servers where swords block)."),
    )
}

fn freelook() -> Module {
    module!(
        "freelook",
        "Freelook",
        "Hold a key to look around without turning.",
        Category::Visual
    )
    .setting(
        s::key("look", "Freelook key", Key::Keyboard(226))
            .described("Hold (or press) to look around without turning."),
    )
    .setting(
        s::choice("mode", "Key", HOLD_TOGGLE, "hold")
            .described("Hold the key to look around, or press it once to start and again to stop."),
    )
    .setting(
        s::choice("view", "Camera", FREELOOK_VIEWS, "back")
            .described("Where the camera goes while you look around."),
    )
    .setting(
        s::bool("invert", "Invert up and down", false)
            .described("Moving the mouse up looks down while looking around."),
    )
}

fn zoom() -> Module {
    module!("zoom", "Zoom", "Hold a key to zoom in.", Category::Visual)
        .setting(
            s::key("zoom", "Zoom key", Key::Keyboard(6)).described("Hold (or press) to zoom in."),
        )
        .setting(
            s::choice("mode", "Key", HOLD_TOGGLE, "hold").described(
                "Hold the key to zoom, or press it once to zoom in and again to zoom out.",
            ),
        )
        .setting(
            s::float("amount", "Zoom", 2.0, 10.0, 0.5, 4.0)
                .formatted(times)
                .described("How many times closer things look. OptiFine's is 4x."),
        )
        .setting(
            s::choice("turning", "Turning", ZOOM_TURNING, "smooth").described(
                "How the view turns while zoomed: smoothly as OptiFine's, slower, or as usual.",
            ),
        )
        .setting(
            s::bool("ease", "Ease in and out", false)
                .described("Zooms in and out gradually instead of at once."),
        )
}

fn crosshair() -> Module {
    module!(
        "crosshair",
        "Crosshair Editor",
        "Draws a crosshair of your own design.",
        Category::Visual
    )
    .setting(
        s::choice("preset", "Preset", CROSSHAIR_PRESETS, "classic")
            .described("Loads a preset crosshair into the sliders below."),
    )
    .setting(
        s::choice("style", "Style", CROSSHAIR_STYLES, "cross")
            .described("A cross, a T, a circle, a dot, or the pixel grid you draw."),
    )
    .setting(
        s::grid("grid", "Pixel grid", 15, CROSSHAIR_GRID)
            .described("Draw your own crosshair, a cell a GUI pixel.")
            .shown(Shown::Is("style", "grid")),
    )
    .setting(
        s::int("length", "Length", 1, 30, 9)
            .formatted(pixels)
            .described("How long each arm is, in screen pixels.")
            .shown(Shown::IsAny("style", &["cross", "t"])),
    )
    .setting(
        s::int("gap", "Gap", 0, 20, 0)
            .formatted(pixels)
            .described("Space between the middle and each arm; a circle's radius.")
            .shown(Shown::NotAny("style", &["dot", "grid"])),
    )
    .setting(
        s::int("thickness", "Thickness", 1, 10, 2)
            .formatted(pixels)
            .described("How thick the lines are, and the dot's size.")
            .shown(Shown::Not("style", "grid")),
    )
    .setting(
        s::bool("dot", "Center dot", false)
            .described("Adds a dot in the middle.")
            .shown(Shown::NotAny("style", &["dot", "grid"])),
    )
    .setting(
        s::bool("outline", "Outline", false)
            .described("A dark outline that keeps the crosshair visible on snow and sky."),
    )
    .setting(s::color("color", "Color", 0xFFFFFF, false))
    .setting(s::int("opacity", "Opacity", 10, 100, 100).formatted(percent))
    .setting(
        s::bool("target-color", "Color on target", false)
            .described("Changes color while the crosshair is on a mob or player in reach."),
    )
    .setting(
        s::color("on-target-color", "Target color", 0xFF5555, false)
            .shown(Shown::When("target-color", true)),
    )
}

fn low_shield() -> Module {
    module!(
        "lowshield",
        "Low Shield",
        "Draws the shield in your hand lower, so it hides less.",
        Category::Visual
    )
    .setting(
        s::int("lower", "Lower by", 0, 90, 35)
            .formatted(percent)
            .described("How far down the shield is drawn, as a share of its height."),
    )
    .setting(
        s::bool("while-blocking", "Also while blocking", true)
            .described("Off: a raised shield is drawn where the game draws it."),
    )
}

fn short_items() -> Module {
    // Was Short Swords: the id stays, so profiles keep it.
    module!(
        "shortswords",
        "Short Items",
        "Draws the items in your hands smaller, so they hide less.",
        Category::Visual
    )
    .setting(
        s::int("size", "Size", 40, 100, 70)
            .formatted(percent)
            .described("How big held items are drawn, against the game's size."),
    )
    .setting(
        s::bool("while-blocking", "Also while in use", true).described(
            "Off: an item in use (blocking, eating, drawing a bow) is drawn at the game's size.",
        ),
    )
}

fn weather_changer() -> Module {
    module!(
        "weatherchanger",
        "Weather Changer",
        "Changes the weather and time of day you see, on your screen only.",
        Category::Visual
    )
    .setting(
        s::choice("weather", "Weather", SKIES, "clear")
            .described("Changes the weather on your screen only."),
    )
    .setting(
        s::bool("hold-time", "Hold the time", false)
            .described("Freezes the time of day on your screen. The world's time keeps going."),
    )
    .setting(
        s::int("hour", "Time", 0, 23, 12)
            .formatted(clock)
            .described("The time of day you see.")
            .shown(Shown::When("hold-time", true)),
    )
}

fn fullbright() -> Module {
    module!(
        "fullbright",
        "Fullbright",
        "Lights up caves and nights as if it were day, on your screen only.",
        Category::Visual
    )
    .setting(
        s::int("level", "Brightness", 10, 100, 100)
            .formatted(percent)
            .described("100%: everything fully lit, as with night vision."),
    )
}

fn custom_sky() -> Module {
    module!(
        "customsky",
        "Custom Sky",
        "Shows the sky your resource packs bring, as OptiFine does.",
        Category::Visual
    )
    .setting(
        s::int("brightness", "Brightness", 10, 100, 100)
            .formatted(percent)
            .described("100%: the sky as the pack made it."),
    )
}

fn block_outline() -> Module {
    module!(
        "blockoutline",
        "Block Outline",
        "Draws the outline of the block you look at in your own color and width.",
        Category::Visual
    )
    .setting(s::color("color", "Color", 0xA9DCFF, false).described("The outline's color."))
    .setting(
        s::int("opacity", "Opacity", 10, 100, 90)
            .formatted(percent)
            .described("How solid the outline is."),
    )
    .setting(
        s::float("width", "Width", 1.0, 6.0, 0.5, 2.5)
            .described("How thick the outline is, in pixels."),
    )
    .setting(s::bool("fill", "Fill", false).described("Shades the block's faces too."))
    .setting(
        s::color("fill-color", "Fill color", 0xA9DCFF, false)
            .described("The shade's color.")
            .shown(Shown::When("fill", true)),
    )
    .setting(
        s::int("fill-opacity", "Fill opacity", 5, 60, 20)
            .formatted(percent)
            .described("How strong the shade is.")
            .shown(Shown::When("fill", true)),
    )
}

fn hit_color() -> Module {
    module!(
        "hitcolor",
        "Hit Color",
        "Changes the color hurt mobs and players flash.",
        Category::Visual
    )
    .setting(
        s::color("color", "Color", 0x5AB8FF, false)
            .described("The color hurt mobs and players flash."),
    )
    .setting(
        s::int("strength", "Strength", 10, 100, 30)
            .formatted(percent)
            .described("How strongly they're tinted. The game's red is 30%."),
    )
}

fn enchant_glint() -> Module {
    module!(
        "glintcolor",
        "Enchant Glint",
        "Colors the shimmer on enchanted items and armor.",
        Category::Visual
    )
    .setting(s::color("color", "Color", 0x7FC8FF, false).described("The glint's color."))
}

fn old_textures() -> Module {
    module!(
        "oldtextures",
        "Old Textures",
        "The textures from before 1.14 (the game's Programmer Art).",
        Category::Visual
    )
    .follows_pack("programmer_art")
}

fn theme() -> Module {
    // Frost's colors are the defaults (ThemeColors.FROST).
    module!(
        "theme",
        "Theme Editor",
        "Changes the client's colors and font: presets, or your own.",
        Category::Visual
    )
    .always_on()
    .setting(
        s::choice("preset", "Preset", THEME_PRESETS, "frost")
            .described("Loads a preset theme into the colors below."),
    )
    .setting(
        s::color("accent", "Accent", 0xA9DCFF, false).described("What's on, chosen or focused."),
    )
    .setting(s::color("text", "Text", 0xFFFFFF, false))
    .setting(
        s::color("muted", "Secondary text", 0xB4BDC6, false)
            .described("Descriptions, labels, what's off."),
    )
    .setting(
        s::color("panel", "Panels", 0x242A32, false)
            .described("Menu panels and the HUD's background."),
    )
    .setting(
        s::int("panel-opacity", "Panel opacity", 40, 100, 86)
            .formatted(percent)
            .described("How solid the menus' panels are (Potato GUI; Normal blurs behind them)."),
    )
    .setting(
        s::int("hud-opacity", "HUD background opacity", 0, 100, 42)
            .formatted(percent)
            .described("How solid the background behind HUD text is."),
    )
    .setting(
        s::int("radius", "Corner rounding", 0, 8, 3).described("How round buttons and panels are."),
    )
    .setting(
        s::choice("font", "Font", FONTS, "red_hat_display")
            .described("The text's font, in the menus, the HUD and the game."),
    )
    .setting(
        s::action("copy-code", "Export theme", "Copy theme code")
            .described("Copies this theme as a code other players can paste."),
    )
    .setting(
        s::action("paste-code", "Import theme", "Paste theme code")
            .described("Loads a theme code another player shared."),
    )
}

fn clear_chat() -> Module {
    let m = module!(
        "clearchat",
        "Clear Chat",
        "Removes the dark box behind chat (the game's Text Background Opacity, set to 0%).",
        Category::Hud
    );
    hold_option(
        m,
        s::float(
            "before-text-background-opacity",
            "Text Background Opacity before",
            0.0,
            1.0,
            0.01,
            0.5,
        ),
        s::bool(
            "held-text-background-opacity",
            "Text Background Opacity held",
            false,
        ),
    )
}

fn low_fire() -> Module {
    module!("lowfire", "Low Fire", "Draws the fire over your view lower, so it hides less.", Category::Visual).setting(
        s::int("lower", "Lower by", 0, 25, 10)
            .formatted(percent)
            .described("How far the flames move down: at 10% they keep to the bottom, at 20% only their tips show."),
    )
}

fn fast_crystal() -> Module {
    module!(
        "fastcrystal",
        "Fast Crystal",
        "Crystals and anchors go as you break them, not a ping later.",
        Category::Combat
    )
    .setting(
        s::bool("crystals", "Crystals", true)
            .described("A crystal you hit is gone at once, so the next can go in its place."),
    )
    .setting(
        s::bool("anchors", "Anchors", true)
            .described("A charged anchor you set off is gone at once, outside the Nether."),
    )
}

fn no_render() -> Module {
    module!(
        "norender",
        "No Render",
        "Hides items on the ground and other things that cost frames.",
        Category::Visual
    )
    .setting(
        s::choice("items", "Items on the ground", NO_RENDER_ITEMS, "hidden")
            .described("Still: no bobbing or spinning. Hidden: not drawn, but still picked up."),
    )
    .setting(
        s::bool("one-copy", "One item per stack", true)
            .described("Draws a stack on the ground as one item, not a pile of up to five.")
            .shown(Shown::Not("items", "hidden")),
    )
    .setting(s::bool("orbs", "Hide XP orbs", false).described("Experience orbs aren't drawn."))
    .setting(
        s::bool("falling-blocks", "Hide falling blocks", false)
            .described("Falling sand, gravel and anvils aren't drawn until they land."),
    )
    .setting(
        s::bool("fireworks", "Hide fireworks", false)
            .described("Firework rockets and their bursts aren't drawn."),
    )
    .setting(
        s::bool("growth", "Hide bone meal sparkles", true)
            .described("The green sparkles of bone meal and villagers aren't drawn."),
    )
    .setting(
        s::bool("breaking", "Hide block bits", false)
            .described("The bits of blocks breaking, and the dust of running and landing."),
    )
    .setting(
        s::bool("explosions", "Hide explosions", false)
            .described("Explosions' smoke clouds aren't drawn."),
    )
    .setting(
        s::bool("all-particles", "Hide all particles", false)
            .described("No particles at all: more frames, but no rain splashes, hits or potions."),
    )
}

fn no_hurt_cam() -> Module {
    let m = module!(
        "nohurtcam",
        "No Hurt Cam",
        "Stops the camera shaking when you take damage (the game's Damage Tilt).",
        Category::Visual
    )
    .setting(
        s::int("shake", "Shake amount", 0, 100, 0)
            .formatted(percent)
            .described("0%: the camera doesn't move at all when you're hurt."),
    );
    hold_option(
        m,
        s::float(
            "before-damage-tilt",
            "Damage Tilt before",
            0.0,
            1.0,
            0.01,
            1.0,
        ),
        s::bool("held-damage-tilt", "Damage Tilt held", false),
    )
}

fn waypoints() -> Module {
    module!(
        "waypoints",
        "Waypoints",
        "Marks places in the world, shown with their distance and a beacon beam.",
        Category::Utility
    )
    .setting(
        s::action("manage", "This world's waypoints", "Manage...")
            .described("Lists this world's waypoints to add, change, hide or remove them."),
    )
    .setting(
        s::key("new", "New waypoint key", Key::Keyboard(17))
            .described("Marks where you stand: opens a new waypoint to name."),
    )
    .setting(
        s::key("list", "Waypoint list key", Key::Keyboard(24))
            .described("Opens this world's waypoints."),
    )
    .setting(
        s::choice("labels", "Names", WAYPOINT_LABELS, "always")
            .described("When a waypoint's name shows above its icon."),
    )
    .setting(
        s::bool("distance", "Show distance", true)
            .described("Shows the distance beside each name."),
    )
    .setting(
        s::bool("beams", "Beams", true)
            .described("Shows a beacon beam on waypoints that have one turned on."),
    )
    .setting(
        s::bool("arrows", "Arrows at the edge", true)
            .described("Points to waypoints off the screen from its edge."),
    )
    .setting(
        s::float("size", "Size", 0.5, 2.0, 0.25, 1.0)
            .described("How big icons and names are drawn over the world."),
    )
    .setting(s::bool("deaths", "Mark deaths", true).described(
        "Adds a waypoint where you die, replacing the last one; on a server, until you leave.",
    ))
    .setting(
        s::bool("clear-deaths", "Clear deaths on arrival", true)
            .described("Removes a death's waypoint once you get back to it.")
            .shown(Shown::When("deaths", true)),
    )
}

fn schematics() -> Module {
    // Java shows Place, Turn, Mirror and Unload only while a schematic is loaded, which no setting
    // says: they show always here, and the game side ignores them with none loaded.
    module!(
        "schematics",
        "Schematics",
        "Places a schematic in the world as ghost blocks to build by.",
        Category::Utility
    )
    .setting(
        s::action("choose", "Schematic", "Choose...")
            .described("Picks a schematic file to build: .litematic, .schem or .nbt."),
    )
    .setting(
        s::action("here", "Place", "Here")
            .described("Moves the schematic's corner to where you stand."),
    )
    .setting(
        s::action("turn", "Turn", "90° right")
            .described("Turns it a quarter clockwise where it stands."),
    )
    .setting(
        s::action("mirror", "Mirror", "Next")
            .described("Mirrors it east-west, then north-south, then not."),
    )
    .setting(
        s::action("unload", "Unload", "Unload").described("Takes the schematic out of the world."),
    )
    .setting(
        s::choice("ghosts", "Ghosts", GHOSTS, "missing")
            .described("Which of the schematic's blocks show as see-through ghosts."),
    )
    .setting(
        s::int("opacity", "Ghost opacity", 10, 90, 45)
            .formatted(percent)
            .described("How solid the ghosts are."),
    )
    .setting(
        s::int("range", "Range", 16, 128, 48)
            .formatted(blocks)
            .described("How far from you ghosts and boxes show; nearer is faster."),
    )
    .setting(
        s::choice("layers", "Layers", LAYERS, "all")
            .described("Shows the whole schematic, or one layer to build by."),
    )
    .setting(
        s::bool("outline", "Show its box", true).described("Outlines the schematic's whole box."),
    )
    .setting(
        s::color("outline-color", "Box color", 0x55AAFF, false)
            .described("The outline's color.")
            .shown(Shown::When("outline", true)),
    )
    .setting(
        s::bool("highlight", "Mark wrong blocks", true)
            .described("Boxes blocks that differ from the schematic."),
    )
    .setting(
        s::color("wrong-color", "Wrong block", 0xFF3333, false)
            .described("A different block from the schematic's.")
            .shown(Shown::When("highlight", true)),
    )
    .setting(
        s::color("state-color", "Wrong way", 0xFFAA00, false)
            .described("The right block, but facing or set another way.")
            .shown(Shown::When("highlight", true)),
    )
    .setting(
        s::color("extra-color", "Extra block", 0xFF55FF, false)
            .described("A block where the schematic has air.")
            .shown(Shown::When("highlight", true)),
    )
    .setting(
        s::key("here-key", "Place here key", Key::None)
            .described("Moves the schematic's corner to where you stand."),
    )
    .setting(
        s::key("turn-key", "Turn key", Key::None)
            .described("Turns it a quarter clockwise; anticlockwise while sneaking."),
    )
    .setting(
        s::key("mirror-key", "Mirror key", Key::None)
            .described("Mirrors it east-west, then north-south, then not."),
    )
    .setting(
        s::key("nudge-key", "Nudge key", Key::None)
            .described("Moves it a block the way you face; back while sneaking."),
    )
    .setting(
        s::key("raise-key", "Up key", Key::None)
            .described("Moves it a block up; down while sneaking."),
    )
    .setting(
        s::key("layer-key", "Layer key", Key::None)
            .described("Shows the next layer up; down while sneaking."),
    )
    .element(HudElement::new(
        "status",
        "Schematic",
        Anchor::TopLeft,
        4,
        60,
    ))
    .element(
        HudElement::new("materials", "Material list", Anchor::TopRight, 4, 60)
            .with(s::int("rows", "Rows", 3, 20, 8).described("How many items it lists at most.")),
    )
}

fn shulker_tooltips() -> Module {
    module!(
        "shulkertooltips",
        "Shulker Tooltips",
        "Shows a shulker box's contents as a grid in its tooltip.",
        Category::Utility
    )
    .setting(s::bool("tinted", "Box color", true).described("The grid takes the box's dye color."))
    .setting(
        s::bool("only-with-shift", "Only with Shift", false)
            .described("Shows the grid only while Shift is held; otherwise the game's lines."),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::profiles;
    use crate::framework::setting::Settings;
    use serde_json::Value;

    fn unique_settings(settings: &Settings, owner: &str) {
        for (i, s) in settings.all.iter().enumerate() {
            assert!(
                settings.all[..i].iter().all(|o| o.id != s.id),
                "{owner}: two settings {}",
                s.id
            );
        }
    }

    #[test]
    fn ids_are_unique() {
        let modules = all();
        for (i, m) in modules.iter().enumerate() {
            assert!(
                modules[..i].iter().all(|o| o.id != m.id),
                "two modules {}",
                m.id
            );
            unique_settings(&m.settings, m.id);
            assert_eq!(
                m.settings.all[0].id, "toggle",
                "{}: the toggle key first",
                m.id
            );
            for (j, e) in m.hud.iter().enumerate() {
                assert!(
                    m.hud[..j].iter().all(|o| o.id != e.id),
                    "{}: two elements {}",
                    m.id,
                    e.id
                );
                unique_settings(&e.settings, e.id);
            }
        }
    }

    #[test]
    fn has_every_java_module() {
        assert_eq!(all().len(), 36, "Features.all registers 36");
        let theme = all().into_iter().find(|m| m.id == "theme").unwrap();
        assert!(theme.always_on);
        let fps = all().into_iter().find(|m| m.id == "fpsdisplay").unwrap();
        assert_eq!(
            fps.settings.get("toggle").unwrap().description,
            "Turns FPS Display on and off."
        );
    }

    #[test]
    fn writes_formats_as_java_does() {
        assert_eq!(percent(35.0), "35%");
        assert_eq!(clock(6.0), "06:00");
        assert_eq!(seconds_or_never(0.0), "Never");
        assert_eq!(seconds_or_never(3.0), "3 s");
        assert_eq!(blocks_across(4.0), "96 blocks across");
        assert_eq!(times(4.0), "4x");
        assert_eq!(times(2.5), "2.5x");
    }

    /// Two stored values the same: numbers as f64, the rest exactly.
    fn same(a: &Value, b: &Value) -> bool {
        match (a.as_f64(), b.as_f64()) {
            (Some(x), Some(y)) => (x - y).abs() < 1e-6,
            _ => a == b,
        }
    }

    fn same_settings(module: &str, original: &Value, written: &Value) {
        let Some(original) = original.as_object() else {
            return;
        };
        for (id, v) in original {
            let w = written.get(id).unwrap_or(&Value::Null);
            assert!(same(v, w), "{module}: {id} was {v}, written {w}");
        }
    }

    #[test]
    fn reads_and_writes_the_java_default_profile() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../run/incari/modules/profiles/Default.json"
        );
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        let original: Value = serde_json::from_str(&text).unwrap();
        let mut modules = all();
        let unknown = profiles::read(&original, &mut modules);
        assert!(
            unknown.is_empty(),
            "modules this build lacks: {:?}",
            unknown.keys().collect::<Vec<_>>()
        );
        let written = profiles::write(&modules, &unknown);
        for (id, entry) in original["modules"].as_object().unwrap() {
            let out = &written["modules"][id];
            assert_eq!(entry["enabled"], out["enabled"], "{id}: enabled");
            same_settings(id, &entry["settings"], &out["settings"]);
            if let Some(hud) = entry.get("hud").and_then(Value::as_object) {
                for (element, settings) in hud {
                    same_settings(&format!("{id}/{element}"), settings, &out["hud"][element]);
                }
            }
        }
    }
}
