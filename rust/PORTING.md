# Porting a feature mod onto the Incari client (ADR 0069)

You're moving some of the Rust build's feature mods (`rust/mods/<name>/`) off Horizon's own
Options > Mods pages and onto Incari's own framework, so the Rust build looks and works exactly
like the Java client: settings in Incari's module menu with the Java ids, HUD elements placed by
anchor in Incari's HUD editor and drawn in Incari's Frost look.

## What exists

- `rust/mods/incari/core/` — crate `incari_core` (no game types): the framework, the look, the
  screens. Read `src/modules.rs` (every module's id, settings and HUD elements, exactly as the
  Java ones declare them), `src/framework/setting.rs` (`Settings::bool/int/float/choice/color/key`
  by Java setting id; `choice` returns the stored key, e.g. `"left"`), `src/hud.rs` (`HudCanvas`:
  `text`, `text_argb`, `fill(.., Tone)`, `fill_argb`, `round`, `item`, `sprite`, `shape`,
  `color(tone)` (honours the element's custom colour), `text_element(text, widest, tone) ->
  [w, h]` = Java's TextElement), `src/theme.rs` (`Tone`), `src/input.rs` (`Key`, `Key::from_name`).
- `rust/mods/incari/glue.rs` — module `crate::game::incari_client` in the game, the helpers your
  hook files call:
  - `settings(module_id, |s: &Settings| ...) -> Option<R>`: None while the module is off.
  - `on(module_id) -> bool`.
  - `element(game: &Game, batch, module_id, element_id, |canvas| -> Option<[f32; 2]>)`: draws a
    HUD element at its own origin and scale (return its unscaled size, None to hide it) and puts
    it where the player placed it. Call it from the `hud` hook. `canvas.module` is the module's
    settings, `canvas.element` the element's, `canvas.editing` true in the HUD editor (show a
    sample when there's nothing to say, as Java's elements do).
- The template, already ported: `rust/mods/cps/` (`runtime.rs`, `hud.rs`, `horizon-mixin.toml`).
  Compare it with `git diff HEAD -- rust/mods/cps` / the old version in git (`git show HEAD:rust/mods/cps/...`
  may not exist: the old files were `list.rs`, `settings.rs`, `click.rs`, which `incari_glue` used).

## For each of your mods

1. Read the Java module: `features/<name>/src/main/java/**` and `features/<name>/README.md`.
   That is the spec: same behaviour, same text, same defaults. Read the mod's current Rust files.
2. Delete `list.rs`, `settings.rs`, `click.rs` and their `[[injection]]`s (`mods`, `settings`,
   `settings_click` hooks) from `horizon-mixin.toml`. Keep every other hook the mod needs; add
   ones it needs to do what the Java module does, if Horizon has them (`native-hooks.json` in
   `/home/bedrock/Documents/horizon-web-development`, and the fork's pending fragments in
   `tools/native-hook-fragments/`, targeted by `target = "..."` path as `rust/mods/nohurtcam`
   does). Don't edit the fork.
3. `runtime.rs` (no game types, tested by `rust/lib.rs`): the module's logic, keyed by the Java
   setting ids/choice keys instead of the old `Def`/`Values`/`SETTINGS` tables. Remove its use of
   `super::incari` (the old common module) unless something there is still truly needed. Keep it
   allocation-free on hot paths (thread-local caches like cps's `with_text`).
4. Hook files: read settings with `crate::game::incari_client::settings(ID, |s| ...)`, draw HUD
   with `crate::game::incari_client::element(...)` using the Java element ids and the Java
   drawing (text elements via `text_element`; custom elements drawn with the canvas as the Java
   `draw(HudCanvas)` does). The `corners` binding of the `hud` hook is no longer used (`let _ =
   corners;`). Remove uses of `crate::game::incari_glue` and `crate::game::incari`.
5. Keys a module binds itself (Zoom's key, Freelook's key...) are `key` settings: compare in the
   `key` hook with `incari_core::input::Key::from_name(key)`. Toggle keys are handled by the client.
6. Update the mod's row in `rust/README.md`'s table ("Not here yet" column: what still differs
   from Java) — only your rows.

## Checks

From `/home/bedrock/Documents/incari-client/rust`: `cargo test` and
`cargo clippy --all-targets -- -D warnings` must pass for the whole workspace (other agents are
editing other mods at the same time: if an error is in a mod that isn't yours, ignore it and say
so). You can't compile the game helpers (`hud.rs` etc.) here — only `runtime.rs` is compiled by
`rust/lib.rs` — so write them carefully against the signatures in `glue.rs`, the existing hook
signatures in each mod's `horizon-mixin.toml`, and the fork's `native_hooks.rs`. Don't run
`./incari build` or anything in `work/` (another build may be running). Don't touch
`rust/lib.rs`, `rust/mods/incari/`, `rust/mods/common/` or other mods. Don't commit.

Report: per mod, what now matches Java, what still differs and why, any hook point Horizon lacks
that the Java behaviour needs, and anything in `glue.rs`/`incari_core` you needed but wasn't there.
