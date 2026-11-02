# rust

- **Hook by name** where Horizon has stamped the hook: each injection names `hook` and
  `hook_version` from Horizon's `native-hooks.json`, so a mod keeps building when Horizon moves
  (ADR 0068). A pending hook (a fork fragment) is targeted by function until it's stamped. A hook Horizon lacks
  goes into the fork (`native_hooks.rs` and `native-hooks.json`), offered upstream; then bump
  `horizon.revision`.
- **Never edit `work/horizon`** or carry patches to it: a change no mod can make is a commit to the
  fork and a pin bump.
- **It's the same client as the Java one** (ADR 0069): a screen of Incari's is a port of the Java
  screen, a mod a port of the Java module, with its README as the spec: same names, settings, text
  and layout. Mod ids are `incari_<module>`.
- **The client is `mods/incari`**: `core/` (`incari_core`) has no game types and is tested here;
  `glue.rs` (module `incari_client`) is its game side. Features read their settings with
  `incari_client::settings(id, ...)` and draw HUD elements with `incari_client::element(...)`
  (`PORTING.md`); every module, setting and HUD element is declared in `core/src/modules.rs` with
  the Java ids, so profiles read the same in both builds.
- **`runtime.rs` uses no game types**, so `rust/lib.rs` tests it alone. Helpers are a line or two
  over `incari_client`, Horizon's `native_*` helpers and the mod's runtime.
- **Hot paths don't allocate**: the `hud`, `frame` and `tick` hooks run every frame or tick. Cache
  in thread-locals, never in locks: in the browser the window's thread is the page's, where a lock
  that waits traps.
- **Write it, don't copy it.** Horizon's own mods (horizon-rust/horizon-mods) are MIT; read them
  for the API, write Incari's own.
- Add a mod to `rust/lib.rs`, the table in `README.md`, and its module's row in
  `features/README.md`; its module to `core/src/modules.rs` if it's new.
- Test: `cargo test` and `cargo clippy --all-targets -- -D warnings` in `rust/`; the build with
  `./incari build --only rust`.
