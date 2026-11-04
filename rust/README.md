# rust: the experimental Rust build

Incari on [Horizon](https://github.com/horizon-rust/horizon), the game rewritten in Rust, through
the browser port in [horizon-web](https://github.com/nobuildersnotools/horizon-web) (ADR 0068). It
draws with WebGPU, and the loading screen offers it as **Rust** beside Fast and Compatible where
WebGPU works.

None of Incari's Java runs in it, but it is the same client (ADR 0069): the Frost look over every
screen, Incari's own title, pause menu, module menu, module settings, HUD editor, Incari Settings
and profiles, every module with the Java ids, and the profiles in the Java client's files. The
client is the `incari_core` crate; Incari's features are Horizon **native mods** that read its
settings and draw their HUD through it: horizon-mixins bundles that hook Horizon's named hook
points (`native-hooks.json`), applied to Horizon's source before it's compiled. Horizon itself is
never committed: `incari.properties` pins a commit, and `./incari setup` clones it into
`work/horizon`.

| Folder | What |
| --- | --- |
| `mods/incari/` | The client: crate `core/` (`incari_core`, no game types: the look, the screens, the framework, `modules.rs`), and its game side `glue.rs` (module `incari_client`) with the hook helpers that draw its screens over the game's |
| `mods/<name>/` | One bundle a feature: `horizon-mixin.toml`, `runtime.rs` (its logic, no game types) and one helper `.rs` a hook |
| `lib.rs`, `Cargo.toml` | Every `runtime.rs` as one crate, and `incari_core` as a workspace member, so their tests run without Horizon |
| `PORTING.md` | How a feature mod reads its settings and draws its HUD through the client |

## Mods

Each mod's settings and HUD elements are its module's in `incari_core::modules` (the Java ids):
the module menu shows them, the HUD editor places them by anchor, the profiles keep them. Crosshair
Editor and Theme Editor have no mod: the client draws the crosshair in the skin and takes its
colours from Theme's settings.

| Mod | Incari module | Hooks | Not here yet |
| --- | --- | --- | --- |
| [`armorpotionview`](mods/armorpotionview/) | [Armor & Potion View](../features/armorpotionview/README.md) | hud | Neutral effects count as harmful; effects the game hides are listed, in Horizon's order; unbreakable items show durability |
| [`blockoutline`](mods/blockoutline/) | [Block Outline](../features/blockoutline/README.md) | block highlight, visuals | Fill; widths past 4; the game's own colour under High Contrast |
| [`clearchat`](mods/clearchat/) | [Clear Chat](../features/clearchat/README.md) | frame | Holds the chat box clear itself, not the game's slider, so moving Text Background Opacity doesn't turn it off |
| [`cooldownindicator`](mods/cooldownindicator/) | [Cooldown Indicator](../features/cooldownindicator/README.md) | hud | Holding the game's Attack Indicator off (turn it off in Options) |
| [`coordinates`](mods/coordinates/) | [Coordinates](../features/coordinates/README.md) | hud | Biome |
| [`cosmetics`](mods/cosmetics/) | Cosmetics (ADR 0061) | player look\*, mob layer, name tag\*, tab list name\*, chat message\*, frame, tick, key | Preview drawn by the client, not the game's renderer; others' trails only in view; wheel key fixed to G |
| [`cps`](mods/cps/) | [CPS](../features/cps/README.md) | hud | |
| [`customsky`](mods/customsky/) | [Custom Sky](../features/customsky/README.md) | frame, resources reloaded, sky layers\* | Nearest-sampled textures; not in Horizon's own packs' water reflection; a shader pack's own sky shows under the layers; biomes, heights, days, transition, sun and moon_phases not read (as Java) |
| [`customscoreboard`](mods/customscoreboard/) | [Custom Scoreboard](../features/customscoreboard/README.md) | sidebar place\*, hud | Placed by the size it had the frame before; the editor's sample is drawn by the mod, not the game |
| [`fpsdisplay`](mods/fpsdisplay/) | [FPS Display](../features/fpsdisplay/README.md) | hud | |
| [`freelook`](mods/freelook/) | [Freelook](../features/freelook/README.md) | key, tick, turn\*, camera rotation\* | Billboards and the in-water fog check follow the player's facing |
| [`fullbright`](mods/fullbright/) | [Fullbright](../features/fullbright/README.md) | frame | Brightness below 100%: Horizon lights fully or not at all |
| [`glintcolor`](mods/glintcolor/) | [Enchant Glint](../features/glintcolor/README.md) | glint color\* | Repainting the glint in the colour: Horizon tints the game's purple glint by it |
| [`hitboxes`](mods/hitboxes/) | [Hitboxes](../features/hitboxes/README.md) | lines\* | Everything (items, arrows); your own box; widths past 4; giving way to F3+B |
| [`hitcolor`](mods/hitcolor/) | [Hit Color](../features/hitcolor/README.md) | hit color\* | Shader packs' entities keep vanilla's red |
| [`hiteffects`](mods/hiteffects/) | [Hit & Kill Effects](../features/hiteffects/README.md) | player attacked\*, mob died\*, frame | |
| [`keystrokes`](mods/keystrokes/) | [Keystrokes](../features/keystrokes/README.md) | hud | |
| [`lowfire`](mods/lowfire/) | [Low Fire](../features/lowfire/README.md) | fire overlay\* | |
| [`lowshield`](mods/lowshield/) | [Low Shield](../features/lowshield/README.md) | hand pose\* | Lowered after the swing's turn, not before it (mid-swing it moves a little differently) |
| [`minimap`](mods/minimap/) | [Minimap](../features/minimap/README.md) | minimap, frame, key, hud | The world map's land (Horizon's map colours); round at 48 and 32 blocks across shows 64; north up, a dot shows the way you face (no turned arrow) |
| [`nohurtcam`](mods/nohurtcam/) | [No Hurt Cam](../features/nohurtcam/README.md) | hurt tilt\* | Holding the game's Damage Tilt option: the hook sets the tilt a frame at a time, so the game's slider doesn't show it or hand control back |
| [`norender`](mods/norender/) | [No Render](../features/norender/README.md) | no render\* | Items being picked up still bob and spin when Still; already-made particles finish their life when a kind is turned off |
| [`oldanimations`](mods/oldanimations/) | [1.7 Animations](../features/oldanimations/README.md) | tick, block highlight, hand pose\* | 1.7 positions for flat items other than tools, weapons, sticks and the bow (the hook doesn't say which are blocks or which model an item has); a left-handed main arm (the main hand is taken as the right); the block-hit with the off hand in use |
| [`oldtextures`](mods/oldtextures/) | [Old Textures](../features/oldtextures/README.md) | frame (Programmer Art helpers\*) | Switching the pack in Resource Packs doesn't turn the module with it |
| [`reachdisplay`](mods/reachdisplay/) | [Reach Display](../features/reachdisplay/README.md) | attack reach\*, frame, hud | Hits on LAN players |
| [`saturation`](mods/saturation/) | [Saturation](../features/saturation/README.md) | health hud, hud | Outlines are the shank's box, not traced from its sprite |
| [`schematics`](mods/schematics/) | [Schematics](../features/schematics/README.md) | tick, key, hud, lines\*, ghosts\*, ghost revision\*, ghost section\* | Old block names aren't upgraded (Horizon has no data fixer: such blocks are left out); no dropping files on the window; the chooser lists no sizes; a problem box's edges only while Horizon's 1024 lines a frame hold out (its fill always shows); messages go to chat, not a toast |
| [`shieldtotem`](mods/shieldtotem/) | [Shield & Totem HUD](../features/shieldtotem/README.md) | hud | |
| [`shortitems`](mods/shortitems/) | [Short Items](../features/shortitems/README.md) | hand pose\* | Next to 1.7 item positions, scaled before or after them as Horizon orders the two mods' hooks |
| [`shulkertooltips`](mods/shulkertooltips/) | [Shulker Tooltips](../features/shulkertooltips/README.md) | tooltip, tooltip component, key | Shulker boxes only, not other items holding items |
| [`togglesprint`](mods/togglesprint/) | [Toggle Sprint](../features/togglesprint/README.md) | movement keys\*, tick, hud | Leaving the game's own Toggle options to the game; two presses within a tick count as one |
| [`waypoints`](mods/waypoints/) | [Waypoints](../features/waypoints/README.md) | tick, frame, key, hud | Beams drawn on the HUD over terrain (no beacon-beam hook); the world map's land (Horizon doesn't hand its map colours over yet); right-click adds at your height |
| [`weatherchanger`](mods/weatherchanger/) | [Weather Changer](../features/weatherchanger/README.md) | seen weather\*, seen day time\* | Snow: rain's levels, which Horizon snows only where the biome is cold |
| [`zoom`](mods/zoom/) | [Zoom](../features/zoom/README.md) | zoom, key | Smooth (cinematic) turning: Horizon has no cinematic camera for a mod, so Smooth turns slower, as Slower does; easing is Horizon's spyglass easing |

\* A hook point Horizon hasn't stamped into `native-hooks.json` yet (a fragment in its
`tools/native-hook-fragments/`): the bundle targets its function by path, and its manifest says the
one-line change for when it's stamped. Block highlight is Horizon's own; the rest, and the player
state helpers (`native_attack_strength`, `native_effect`, `native_equipped`...), are Incari's, in
the fork's `incari_client_hooks.json`, to offer upstream.

An Eaglercraft server's own pause menu (its `pause_menu.yml`: server info and Discord buttons, and
pictures beside the menu's buttons and title) shows in Incari's pause menu in place of the game's
Server Links button, as in the Java build. Horizon reads it (the fork's `eagler::pause_menu`,
`native_eagler_pause_menu`); a server info button that shows a web view isn't shown.

F then a digit types that function key (F+3 is F3, F+0 is F10) for Chromebooks, as in the Java build. It's in the Horizon fork (`game/f_chord.rs`), not a mod: a mod's `key` hook can't swallow or remake a key. Needs the pin bumped to the fork commit that has it.

Everything of the Java client's is here. What runs differently: the screen recorder records in the
browser build only (the page's `MediaRecorder`), as Java's; Incari's online service works only in
the threaded browser build (the others have no WebSocket client); the world map remembers land
while the minimap shows; the Wardrobe's and the title card's player is drawn by the client, not the
game's renderer; the Theme Editor's font choice stays Red Hat Display.

## Build

Needs, beside Incari's own: Rust's nightly that Horizon's `web/build.sh` names, with `rust-src` and
the `wasm32-unknown-unknown` target; the `wasm-bindgen-cli` of Horizon's `Cargo.lock`; binaryen's
`wasm-opt`; and Python 3. `./incari build` says which are missing.

```sh
./incari build --only rust     # Horizon's resources, the mods, both wasm builds -> app-web/build/rust/
./gradlew :app-web:serve       # serves it at /rust/; the page offers it
INCARI_HORIZON_DIR=~/horizon-web ./incari build --only rust   # another Horizon checkout
```

The first build compiles `horizon-prepare` and the game: long.
Under `work/`:

| Folder | What |
| --- | --- |
| `horizon/` | The pinned Horizon commit, never edited here |
| `horizon-override/` | With `INCARI_HORIZON_DIR`: that checkout's files as git sees them (its ignored builds left out) |
| `horizon-resources/<version>/` | Horizon's resources, prepared from the jars and assets `./incari setup` and `./incari assets` fetched |
| `horizon-stage/` | Horizon with `rust/mods` applied (horizon-mixins keeps unchanged files' times, so rebuilds are incremental) |
| `horizon-target/`, `horizon-out/` | Cargo's output, and `web/package.py`'s folder before it's copied to `app-web/build/rust/` |
| `cache/horizon-mixins/<commit>/` | horizon-mixins, built once |

## How to test

```sh
cargo test --manifest-path rust/Cargo.toml      # each mod's logic
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
./incari build --only rust && ./gradlew :app-web:serve   # then open http://localhost:8080/?build=rust
```

The game itself needs WebGPU: a real GPU, or Chrome with `--enable-unsafe-webgpu`.
