# Porting a Java screen or subsystem to the Rust client (ADR 0069)

The Rust build (Horizon, `rust/`) runs Incari's own client, `incari_core`
(`rust/mods/incari/core/`, no game types), with its game side in `rust/mods/incari/glue.rs`. The
Java client is the spec: same layout, text, behaviour and files. Read first:

- `docs/adr/0069-incari-client-on-horizon.md`, `rust/README.md`, `rust/AGENTS.md`.
- `incari_core`: `src/screen.rs` (`Screen`, `Cx`, `Go`, the `Host` trait the game side
  implements), `src/client.rs` (the screen stack, `Client::frame`/`input`/`hud_element`),
  `src/ui.rs` (buttons, menu buttons, entries, switch, slider, text field, scrolling, tooltips),
  `src/paint.rs` (`Painter`: round boxes, icons, text that fits/wraps, shapes, images on the
  canvas), `src/panels.rs`, `src/screens/` (ported screens: `list.rs` is ListScreen,
  `module_settings.rs`, `hud_editor.rs`, `incari_settings.rs`, `title.rs`, `grid.rs`),
  `src/framework/` (modules, settings, profiles), `src/modules.rs` (every module, Java ids).
- The Java source of what you port (paths in your task), its ADR, and the matching
  `run/smoke-*/incari-smoke/screenshots/*.png` if there is one.

## Rules

- **Only new files, plus one-line registrations**: you may create files under
  `rust/mods/incari/core/src/` (new modules, `screens/<name>.rs`) and new mod folders
  `rust/mods/<name>/`, and add their `pub mod` lines to `core/src/lib.rs`,
  `core/src/screens/mod.rs` and `rust/lib.rs`. Other agents are doing the same right now, so
  re-read a file just before editing it and keep edits to those single lines.
- **Don't edit** `client.rs`, `screen.rs`, `glue.rs`, `modules.rs`, other screens, other mods, or
  the Horizon fork (`/home/bedrock/Documents/horizon-web-development`). When you need something
  there (a `Host` method, a glue helper, a fork hook point, a button wired to your screen, a new
  setting), write it down precisely in your report: the signature, what it does, and the exact
  code to add, so it can be applied as is. Write your screens against those planned APIs (use
  `todo!()`-free code: e.g. take the data as constructor arguments or a trait you define in your
  own module, so your code compiles now).
- No game types in `incari_core`; no `std::fs`, `std::time`, `std::thread` there either (the
  browser has none): files and time come through `Host`/your own trait.
- Hot paths don't allocate; thread-locals, never locks, on the window's thread.
- Tests: pure logic gets unit tests (codecs, layout maths, state machines). From
  `/home/bedrock/Documents/incari-client/rust`: `cargo test --workspace` and
  `cargo clippy --workspace --all-targets -- -D warnings` must pass. Errors in files that aren't
  yours: ignore and mention. Don't run `./incari build` or anything in `work/`. Don't commit.

## Report

What you ported (files), what matches Java and what differs, then the integration list: every
change to a file you couldn't edit, as exact code, and every fork hook point you need with its
signature and what it must do in Horizon.
