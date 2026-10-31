# ADR 0067: Custom Sky: resource packs' OptiFine skies

- Status: proposed
- Date: 2026-10-30
- Builds on: ADR 0046 (modules), ADR 0007 (the renderer's backend API)

## Context

Most PvP resource packs ship a custom sky in OptiFine's format: `optifine/sky/world0/sky<N>.properties`
(MCPatcher's `mcpatcher/sky/world0` in older packs), each naming a texture of a cube's six faces in a
3 by 2 grid, with fade times, a blend and a rotation. The game ignores them. Players asked for them.

## Decision

- **A module, Custom Sky** (`customsky`), off by default, with a brightness. It parses the files
  (`SkyLayer`, no game types) and asks for them through `Game.showPackSkies`.
- **Two hooks in `SkyRenderer`**: `extractRenderState` reads the time and the weather as shown
  (so Weather Changer's held time and weather apply), and `render` draws the layers after the
  sunrise, before the sun, moon and stars, as OptiFine does. A new `SkyRenderer` (made after every
  resource reload) means the packs may have changed: no reload listener.
- **The end sky's shader, nine pipelines.** Each OptiFine blend is a pipeline over
  `core/position_tex_color` with OptiFine's blend factors, so the browser needs no new shader.
- **Loaded only while on.** Sky textures are large; they're loaded the first frame the module is on
  and released when it's off.

## Limits

- Overworld only. `biomes`, `heights`, `days`, `transition`, `sun.properties` and
  `moon_phases.properties` aren't read yet: a layer shows everywhere, every day.
- The face layout, rotation axis and blend factors follow OptiFine's; check them against a pack in
  OptiFine before calling it exact.
