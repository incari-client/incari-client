# ADR 0069: Incari Client itself on Horizon

- Status: proposed
- Date: 2026-11-01
- Changes: ADR 0068 (its mods' settings pages and HUD corners)

## Context

ADR 0068's Rust build ported Incari's features as Horizon mods: each listed in Horizon's
Options > Mods, settings as cycling buttons, HUD boxes in four corners, on Horizon's vanilla title
and pause screens. It played like Horizon with some extras, not like Incari: a player had to learn
it anew, and their profiles didn't carry over.

## Decision

The Rust build is the same client: at first glance it looks and works as the Java build does.

- **The client is one crate**, `incari_core` (`rust/mods/incari/core`, a horizon-mixins bundle
  crate), with no game types: the Frost look (theme, rounded glass, icons, the mark, the wordmark),
  Incari's screens (title, pause, module menu, module settings, HUD editor, Incari Settings,
  profiles), the module framework with every module declared with the Java ids
  (`modules.rs`), and profiles in the Java JSON format. It draws by listing fills, canvas images
  and text; `rust/mods/incari/glue.rs` replays the list into Horizon's HUD batch and does what the
  client asks of the game through `native_*` helpers.
- **Feature mods keep their hooks, not their pages.** Each reads its module's settings
  (`incari_client::settings`) and draws its HUD elements where the player placed them
  (`incari_client::element`). Nothing is listed in Horizon's Options > Mods.
- **The fork gains generic hook points** (pending fragment `incari_shell_hooks.json`), offered
  upstream: a mod draws a screen itself (`native_screen_claim`), draws over every screen and hears
  input first (`native_overlay`, `native_overlay_input`), draws its own backdrop in place of the
  panorama (`native_backdrop_claim`, `native_backdrop`), and paints images at run time into a
  2048x1024 atlas slot (`native_canvas_upload`). A GUI skin (`hud::set_skin`) is asked first for
  every sprite drawn by name, so Frost covers Horizon's own screens and HUD, as `FrostSkin` covers
  the game's in Java. Helpers: `native_open`, `native_connect`, `native_translate`,
  `native_own_skin`, `native_clipboard`, `native_atlas` and others in the fragment.
- **Incari's pack is always on**, as in Java: `./incari build` copies it over Horizon's prepared
  resources (Red Hat Display as the game's font) and the bundle's data (the mark, the backdrop,
  Michroma) from `client/src/main/resources/incari-pack`. One copy in the repo.
- **Settings are shared with the Java build's files**: `incari/modules/profiles/*.json`,
  `incari/look.txt` and `incari/settings.txt`, same keys.

## Consequences

- A screen of Incari's is written twice (Java and Rust), from the same layout constants. The
  Java one is the spec.
- Ported besides the modules: Incari accounts, chat and friends (`incari_core::online`, the
  socket only in the threaded browser build), screenshots, the screen recorder (the page's
  `MediaRecorder`, browser only), credits, featured servers (the fork's
  `multiplayer::set_featured`), waypoints and the world map (its land from the fork's
  `native_map_tile`), Custom Sky (the fork's `native_sky_layers`, a render pass of sky layers), Schematics (the fork's
  `native_ghosts`, ghost blocks meshed from the game's models).
- Cosmetics and the Wardrobe (the fork's `native_player_look`, name tag, tab list and chat hooks);
  their previews are drawn by the client, not the game's renderer.
- Still open: linear sampling of the backdrop (Horizon's HUD samples nearest), and the Theme
  Editor's font choice.
- Renderer reviewers review the fork's skin and canvas; they touch every GUI draw.
