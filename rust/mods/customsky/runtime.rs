// Incari's Custom Sky for the Rust build (ADR 0067, 0068, 0069), as features/customsky does it:
// the skies resource packs bring in OptiFine's format (`optifine/sky/world0/sky<N>.properties`,
// or MCPatcher's `mcpatcher/sky/world0`), drawn over the game's own sky in the Overworld. The
// parser is Java's `SkyLayer`; the loading and drawing are `ModuleSkyLayers`', with Horizon's
// renderer drawing the cubes (`native_sky_layers`). Its setting is the module's in
// `incari_core::modules` (the Java ids). No game types: rust/lib.rs tests it.

use std::cell::RefCell;

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "customsky";

/// Where the layers are looked for, in the resource root, the first folder with any winning.
pub const FOLDERS: [&str; 2] = [
    "assets/minecraft/optifine/sky/world0",
    "assets/minecraft/mcpatcher/sky/world0",
];

/// Ticks in a day; 0 is 6:00, as the game counts.
const DAY: i64 = 24000;

/// How a layer is mixed with what's behind it, OptiFine's names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blend {
    Alpha,
    Add,
    Subtract,
    Multiply,
    Dodge,
    Burn,
    Screen,
    Overlay,
    Replace,
}

/// One layer of a resource pack's custom sky, in OptiFine's format: a `sky<N>.properties` naming
/// a texture that holds a cube's six faces in a 3 by 2 grid.
///
/// Understood: `source`, the four fade times, `blend`, `rotate`, `speed`, `axis` and `weather`.
/// Not yet: `biomes`, `heights`, `days` and `transition`; a layer that has them shows
/// everywhere, every day.
#[derive(Clone, Debug, PartialEq)]
pub struct SkyLayer {
    source: String,
    /// The fades in ticks from dawn; `start_fade_in` -1 for a layer always shown.
    start_fade_in: i32,
    end_fade_in: i32,
    start_fade_out: i32,
    end_fade_out: i32,
    blend: Blend,
    rotate: bool,
    speed: f32,
    axis: [f32; 3],
    clear: bool,
    rain: bool,
    thunder: bool,
}

impl SkyLayer {
    /// The layer `text` describes, or none if it can't be drawn (fade times missing or
    /// unreadable, a source that isn't a path). `file` is where it was found,
    /// `optifine/sky/world0/sky1.properties`: a source is relative to its folder, and its own
    /// name with `.png` is the default source.
    pub fn parse(text: &str, file: &str) -> Option<SkyLayer> {
        let folder = file.rfind('/').map_or("", |i| &file[..i]);
        let base = file.strip_suffix(".properties").unwrap_or(file);
        let mut source_text: Option<&str> = None;
        let mut fade_text: [Option<&str>; 4] = [None; 4];
        let mut blend = Blend::Add;
        let mut rotate = true;
        let mut speed = 1.0;
        let mut axis = [0.0, 0.0, 1.0];
        let (mut clear, mut rain, mut thunder) = (true, false, false);
        for raw in text.split('\n') {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
                continue;
            }
            let Some(split) = line.find('=').or_else(|| line.find(':')) else {
                continue;
            };
            if split == 0 {
                continue;
            }
            let key = line[..split].trim();
            let value = line[split + 1..].trim();
            match key {
                "source" => source_text = Some(value),
                "startFadeIn" => fade_text[0] = Some(value),
                "endFadeIn" => fade_text[1] = Some(value),
                "startFadeOut" => fade_text[2] = Some(value),
                "endFadeOut" => fade_text[3] = Some(value),
                "blend" => blend = parse_blend(value),
                "rotate" => rotate = !value.eq_ignore_ascii_case("false"),
                "speed" => speed = number(value).unwrap_or(1.0),
                "axis" => axis = parse_axis(value),
                "weather" => {
                    let has = |w: &str| value.split_whitespace().any(|v| v.eq_ignore_ascii_case(w));
                    (clear, rain, thunder) = (has("clear"), has("rain"), has("thunder"));
                }
                // Biomes, heights, days, transition: not yet; the layer shows regardless.
                _ => {}
            }
        }
        let fades = fades(fade_text)?;
        let default_source = format!("{base}.png");
        let source = resolve(source_text.unwrap_or(&default_source), folder)?;
        Some(SkyLayer {
            source,
            start_fade_in: fades[0],
            end_fade_in: fades[1],
            start_fade_out: fades[2],
            end_fade_out: fades[3],
            blend,
            rotate,
            speed,
            axis,
            clear,
            rain,
            thunder,
        })
    }

    /// The texture, `namespace:path`.
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn blend(&self) -> Blend {
        self.blend
    }

    /// The axis the layer turns on, in the world's frame (x east, y up, z south), of length 1.
    pub fn axis(&self) -> [f32; 3] {
        self.axis
    }

    /// How far the layer is turned, in radians, with the sun at `sun_angle`.
    pub fn angle(&self, sun_angle: f32) -> f32 {
        if self.rotate {
            sun_angle * self.speed
        } else {
            0.0
        }
    }

    /// How strongly it shows, 0 to 1, at `time_of_day` ticks from dawn, with the rain and
    /// thunder as shown (0 to 1 each).
    pub fn brightness(&self, time_of_day: i64, rain: f32, thunder: f32) -> f32 {
        let mut weather = 0.0;
        if self.clear {
            weather += 1.0 - rain;
        }
        if self.rain {
            weather += (rain - thunder).max(0.0);
        }
        if self.thunder {
            weather += thunder;
        }
        f32::clamp(weather, 0.0, 1.0) * self.fade(time_of_day)
    }

    fn fade(&self, time_of_day: i64) -> f32 {
        if self.start_fade_in < 0 {
            return 1.0;
        }
        // Every time from the start of the fade in, so a layer that shows over midnight wraps.
        let start = i64::from(self.start_fade_in);
        let full_in = wrap(i64::from(self.end_fade_in) - start);
        let out = wrap(i64::from(self.start_fade_out) - start);
        let gone = wrap(i64::from(self.end_fade_out) - start);
        let now = wrap(time_of_day - start);
        if now < full_in {
            now as f32 / full_in as f32
        } else if now < out {
            1.0
        } else if now < gone {
            1.0 - (now - out) as f32 / (gone - out) as f32
        } else {
            0.0
        }
    }
}

/// The fades in ticks; -1 each for a layer always shown, none if they can't be read.
fn fades(text: [Option<&str>; 4]) -> Option<[i32; 4]> {
    if text.iter().all(Option::is_none) {
        return Some([-1; 4]);
    }
    let mut fades = [0; 4];
    for i in 0..3 {
        fades[i] = time(text[i]?);
        if fades[i] < 0 {
            return None;
        }
    }
    // OptiFine's default: the fade out takes as long as the fade in.
    fades[3] = match text[3] {
        None => wrap(i64::from(fades[2]) + i64::from(fades[1]) - i64::from(fades[0])),
        Some(t) => time(t),
    };
    (fades[3] >= 0).then_some(fades)
}

/// `hh:mm` as ticks from dawn (6:00 is 0), or -1.
pub fn time(text: &str) -> i32 {
    let Some((hours, minutes)) = text.split_once(':') else {
        return -1;
    };
    let (Ok(hours), Ok(minutes)) = (hours.trim().parse::<i32>(), minutes.trim().parse::<i32>())
    else {
        return -1;
    };
    if !(0..=24).contains(&hours) || !(0..=59).contains(&minutes) {
        return -1;
    }
    wrap(i64::from(hours * 1000 + minutes * 1000 / 60 - 6000))
}

/// A source as OptiFine resolves it: `./` from the file's folder, `~/` from `optifine`,
/// `assets/<namespace>/` or a namespace for that namespace, a bare name in the file's folder,
/// and any other path from `assets/minecraft`. None if it isn't a path the game can name.
pub fn resolve(source: &str, folder: &str) -> Option<String> {
    let mut path = source.trim().to_owned();
    let mut namespace = "minecraft".to_owned();
    if let Some(rest) = path.strip_prefix("./") {
        path = format!("{folder}/{rest}");
    } else if let Some(rest) = path.strip_prefix("~/") {
        path = format!("optifine/{rest}");
    } else {
        if let Some(rest) = path.strip_prefix('/') {
            path = rest.to_owned();
        }
        let assets = path
            .strip_prefix("assets/")
            .and_then(|rest| rest.split_once('/'))
            .filter(|(ns, _)| !ns.is_empty())
            .map(|(ns, p)| (ns.to_owned(), p.to_owned()));
        match path.find(':') {
            Some(colon) if colon > 0 => {
                namespace = path[..colon].to_owned();
                path = path[colon + 1..].to_owned();
            }
            _ => {
                if let Some((ns, p)) = assets {
                    (namespace, path) = (ns, p);
                } else if !path.contains('/') && !folder.is_empty() {
                    path = format!("{folder}/{path}");
                }
            }
        }
    }
    valid_path(&namespace, &path).then(|| format!("{namespace}:{path}"))
}

/// What the game takes for a resource's namespace and path.
fn valid_path(namespace: &str, path: &str) -> bool {
    let name = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || "_-.".contains(c);
    !namespace.is_empty()
        && !path.is_empty()
        && !path.contains("..")
        && namespace.chars().all(name)
        && path.chars().all(|c| c == '/' || name(c))
}

/// Where a resource `namespace:path` is in the resource root.
pub fn resource_path(id: &str) -> Option<String> {
    let (namespace, path) = id.split_once(':')?;
    valid_path(namespace, path).then(|| format!("assets/{namespace}/{path}"))
}

/// Where a file comes in its folder's order: the number in `sky12.properties`; a file without
/// one goes last.
pub fn order(file: &str) -> i32 {
    let name = file.rfind('/').map_or(file, |i| &file[i + 1..]);
    let end = name.find('.').unwrap_or(name.len());
    let digits = name[..end]
        .bytes()
        .rev()
        .take_while(u8::is_ascii_digit)
        .count();
    if digits == 0 || digits > 6 {
        return i32::MAX;
    }
    name[end - digits..end].parse().unwrap_or(i32::MAX)
}

/// The layers' files among a folder's `names`, as their parser names them
/// (`optifine/sky/world0/sky1.properties`), in their order: `sun.properties` and
/// `moon_phases.properties` aren't layers.
pub fn layer_files(folder: &str, names: &[String]) -> Vec<String> {
    let relative = folder.strip_prefix("assets/minecraft/").unwrap_or(folder);
    let mut files: Vec<String> = names
        .iter()
        .filter(|n| n.ends_with(".properties") && order(n) != i32::MAX)
        .map(|n| format!("{relative}/{n}"))
        .collect();
    files.sort_by(|a, b| order(a).cmp(&order(b)).then_with(|| a.cmp(b)));
    files
}

fn parse_blend(text: &str) -> Blend {
    match text.trim().to_ascii_lowercase().as_str() {
        "alpha" => Blend::Alpha,
        "subtract" => Blend::Subtract,
        "multiply" => Blend::Multiply,
        "dodge" => Blend::Dodge,
        "burn" => Blend::Burn,
        "screen" => Blend::Screen,
        "overlay" => Blend::Overlay,
        "replace" => Blend::Replace,
        _ => Blend::Add,
    }
}

fn number(text: &str) -> Option<f32> {
    text.trim().parse::<f32>().ok().filter(|v| v.is_finite())
}

/// Three numbers, made length 1; the sun's axis if they aren't.
fn parse_axis(text: &str) -> [f32; 3] {
    let parts: Vec<&str> = text.split_whitespace().collect();
    if let [x, y, z] = parts[..]
        && let (Some(x), Some(y), Some(z)) = (number(x), number(y), number(z))
    {
        let length = (x * x + y * y + z * z).sqrt();
        if length > 1.0e-4 {
            return [x / length, y / length, z / length];
        }
    }
    [0.0, 0.0, 1.0]
}

fn wrap(ticks: i64) -> i32 {
    ticks.rem_euclid(DAY) as i32
}

/// The brightness setting (10 to 100%) as the layers' strength.
pub fn level(percent: i32) -> f32 {
    (percent as f32 / 100.0).clamp(0.0, 1.0)
}

/// What the game side does this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Nothing: the layers are as they should be.
    Keep,
    /// Off with textures kept: free them ([`take_textures`]).
    Release,
    /// On with the layers due (never read, a resource reload, a texture the renderer lost):
    /// free the old ones ([`take_textures`]), read them, then [`finish_load`].
    Load,
}

/// The layers read and their textures, the brightness asked for (0 while off), and whether the
/// layers must be read again. Window's thread only.
struct State {
    layers: Vec<(SkyLayer, u64)>,
    brightness: f32,
    stale: bool,
}

thread_local! {
    static STATE: RefCell<State> = const {
        RefCell::new(State { layers: Vec::new(), brightness: 0.0, stale: true })
    };
}

/// From the frame: the brightness asked for (0 while off), and which textures the renderer
/// still has. Allocation-free.
pub fn step(brightness: f32, alive: impl Fn(u64) -> bool) -> Step {
    STATE.with_borrow_mut(|s| {
        s.brightness = brightness.max(0.0);
        if s.brightness <= 0.0 {
            if s.layers.is_empty() {
                return Step::Keep;
            }
            // Read again when next wanted: the packs may change while it's off.
            s.stale = true;
            return Step::Release;
        }
        if s.stale || s.layers.iter().any(|(_, id)| !alive(*id)) {
            Step::Load
        } else {
            Step::Keep
        }
    })
}

/// A resource reload: the packs may have changed.
pub fn mark_stale() {
    STATE.with_borrow_mut(|s| s.stale = true);
}

/// The kept textures, to free: the layers are gone until the next [`finish_load`].
pub fn take_textures() -> Vec<u64> {
    STATE.with_borrow_mut(|s| {
        s.stale = true;
        s.layers.drain(..).map(|(_, id)| id).collect()
    })
}

/// The layers just read, in order, with their textures.
pub fn finish_load(layers: Vec<(SkyLayer, u64)>) {
    STATE.with_borrow_mut(|s| {
        s.layers = layers;
        s.stale = false;
    });
}

/// Each layer that shows now, in order: its texture, blend, strength (the fades, the weather and
/// the brightness), angle and axis. `time_of_day`, `rain` and `thunder` as the player sees them,
/// `sun_angle` in radians. Allocation-free.
pub fn each_layer(
    time_of_day: i64,
    rain: f32,
    thunder: f32,
    sun_angle: f32,
    mut f: impl FnMut(u64, Blend, f32, f32, [f32; 3]),
) {
    STATE.with_borrow(|s| {
        if s.brightness <= 0.0 {
            return;
        }
        for (layer, texture) in &s.layers {
            let alpha = layer.brightness(time_of_day, rain, thunder) * s.brightness;
            if alpha > 1.0e-4 {
                f(
                    *texture,
                    layer.blend(),
                    alpha,
                    layer.angle(sun_angle),
                    layer.axis(),
                );
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "optifine/sky/world0/sky1.properties";

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    fn at(text: &str) -> i64 {
        i64::from(time(text))
    }

    #[test]
    fn a_bare_file_is_its_own_texture_always_shown_added() {
        let layer = SkyLayer::parse("", FILE).unwrap();
        assert_eq!(layer.source(), "minecraft:optifine/sky/world0/sky1.png");
        assert_eq!(layer.blend(), Blend::Add);
        assert_eq!(layer.brightness(18000, 0.0, 0.0), 1.0);
        assert_eq!(layer.angle(1.5), 1.5);
        assert_eq!(layer.axis()[2], 1.0);
    }

    #[test]
    fn reads_optifines_keys() {
        let layer = SkyLayer::parse(
            "# a night sky\nsource=./stars.png\nblend=alpha\nrotate=true\nspeed=2\naxis=0.0 2.0 0.0\n",
            FILE,
        )
        .unwrap();
        assert_eq!(layer.source(), "minecraft:optifine/sky/world0/stars.png");
        assert_eq!(layer.blend(), Blend::Alpha);
        assert_eq!(layer.angle(1.0), 2.0);
        assert_eq!(layer.axis()[1], 1.0);
        assert_eq!(
            SkyLayer::parse("rotate=false", FILE).unwrap().angle(1.0),
            0.0
        );
        // Windows line ends and a colon for the equals sign.
        let layer = SkyLayer::parse("blend: screen\r\nspeed=x\r\n", FILE).unwrap();
        assert_eq!(layer.blend(), Blend::Screen);
        assert_eq!(layer.angle(1.0), 1.0);
    }

    #[test]
    fn resolves_sources_as_optifine_does() {
        let folder = "optifine/sky/world0";
        let r = |s| resolve(s, folder);
        assert_eq!(
            r("sky2.png").as_deref(),
            Some("minecraft:optifine/sky/world0/sky2.png")
        );
        assert_eq!(
            r("~/sky/clouds.png").as_deref(),
            Some("minecraft:optifine/sky/clouds.png")
        );
        assert_eq!(
            r("/assets/minecraft/mcpatcher/sky/a.png").as_deref(),
            Some("minecraft:mcpatcher/sky/a.png")
        );
        assert_eq!(
            r("textures/sky/a.png").as_deref(),
            Some("minecraft:textures/sky/a.png")
        );
        assert_eq!(r("pack:sky/a.png").as_deref(), Some("pack:sky/a.png"));
        assert_eq!(r("Sky Night.png"), None);
        assert_eq!(r("./../../secret.png"), None);
    }

    #[test]
    fn fades_in_and_out_over_midnight() {
        // Night stars: in from 18:00 to 19:00, out from 05:00 to 06:00.
        let layer = SkyLayer::parse(
            "startFadeIn=18:00\nendFadeIn=19:00\nstartFadeOut=5:00\nendFadeOut=6:00",
            FILE,
        )
        .unwrap();
        assert_eq!(layer.brightness(at("12:00"), 0.0, 0.0), 0.0);
        assert!(close(layer.brightness(at("18:30"), 0.0, 0.0), 0.5));
        assert_eq!(layer.brightness(at("0:00"), 0.0, 0.0), 1.0);
        assert!(close(layer.brightness(at("5:30"), 0.0, 0.0), 0.5));
        // A later day, and the game's total ticks.
        assert_eq!(layer.brightness(at("0:00") + 24000 * 7, 0.0, 0.0), 1.0);
    }

    #[test]
    fn an_end_fade_out_left_out_takes_as_long_as_the_fade_in() {
        let layer = SkyLayer::parse(
            "startFadeIn=18:00\nendFadeIn=20:00\nstartFadeOut=4:00",
            FILE,
        )
        .unwrap();
        assert!(close(layer.brightness(at("5:00"), 0.0, 0.0), 0.5));
        assert_eq!(layer.brightness(at("6:30"), 0.0, 0.0), 0.0);
    }

    #[test]
    fn half_the_fade_times_is_no_layer() {
        assert_eq!(SkyLayer::parse("startFadeIn=18:00", FILE), None);
        assert_eq!(
            SkyLayer::parse("startFadeIn=dusk\nendFadeIn=19:00\nstartFadeOut=5:00", FILE),
            None
        );
    }

    #[test]
    fn clear_skies_fade_with_the_rain() {
        let clear = SkyLayer::parse("", FILE).unwrap();
        assert_eq!(clear.brightness(0, 1.0, 0.0), 0.0);
        assert_eq!(clear.brightness(0, 0.25, 0.0), 0.75);
        let storm = SkyLayer::parse("weather=thunder", FILE).unwrap();
        assert_eq!(storm.brightness(0, 0.0, 0.0), 0.0);
        assert_eq!(storm.brightness(0, 1.0, 1.0), 1.0);
        let wet = SkyLayer::parse("weather=rain thunder", FILE).unwrap();
        assert_eq!(wet.brightness(0, 1.0, 0.0), 1.0);
        assert_eq!(wet.brightness(0, 0.0, 0.0), 0.0);
    }

    #[test]
    fn times_count_from_dawn() {
        assert_eq!(time("6:00"), 0);
        assert_eq!(time("12:00"), 6000);
        assert_eq!(time("0:00"), 18000);
        assert_eq!(time("6:30"), 500);
        assert_eq!(time("noon"), -1);
        assert_eq!(time("25:00"), -1);
    }

    #[test]
    fn orders_files_by_their_number() {
        assert!(
            order("optifine/sky/world0/sky2.properties")
                < order("optifine/sky/world0/sky10.properties")
        );
        assert_eq!(order("optifine/sky/world0/sun.properties"), i32::MAX);
        let names = [
            "sky10.properties",
            "sun.properties",
            "sky2.properties",
            "sky2.png",
        ]
        .map(String::from);
        assert_eq!(
            layer_files(FOLDERS[0], &names),
            [
                "optifine/sky/world0/sky2.properties",
                "optifine/sky/world0/sky10.properties"
            ]
        );
    }

    #[test]
    fn sources_are_found_under_assets() {
        assert_eq!(
            resource_path("minecraft:optifine/sky/world0/sky1.png").as_deref(),
            Some("assets/minecraft/optifine/sky/world0/sky1.png")
        );
        assert_eq!(resource_path("no-colon"), None);
    }

    #[test]
    fn loads_when_on_releases_when_off() {
        let alive = |_| true;
        // Off with nothing kept.
        assert_eq!(step(0.0, alive), Step::Keep);
        // On: read them, then keep them.
        assert_eq!(step(1.0, alive), Step::Load);
        assert!(take_textures().is_empty());
        finish_load(vec![(SkyLayer::parse("", FILE).unwrap(), 7)]);
        assert_eq!(step(0.5, alive), Step::Keep);
        let mut shown = Vec::new();
        each_layer(0, 0.0, 0.0, 1.0, |t, b, a, angle, _| {
            shown.push((t, b, a, angle))
        });
        assert_eq!(shown, [(7, Blend::Add, 0.5, 1.0)]);
        // The renderer lost the texture (a new one): read again.
        assert_eq!(step(0.5, |id| id != 7), Step::Load);
        // A resource reload.
        finish_load(vec![(SkyLayer::parse("", FILE).unwrap(), 8)]);
        mark_stale();
        assert_eq!(step(0.5, alive), Step::Load);
        finish_load(vec![(SkyLayer::parse("", FILE).unwrap(), 9)]);
        // Off: free them; nothing drawn.
        assert_eq!(step(0.0, alive), Step::Release);
        assert_eq!(take_textures(), [9]);
        let mut any = false;
        each_layer(0, 0.0, 0.0, 0.0, |_, _, _, _, _| any = true);
        assert!(!any);
        assert_eq!(step(0.0, alive), Step::Keep);
        assert_eq!(step(1.0, alive), Step::Load);
    }

    #[test]
    fn the_brightness_setting_is_a_share() {
        assert_eq!(level(100), 1.0);
        assert_eq!(level(40), 0.4);
    }
}
