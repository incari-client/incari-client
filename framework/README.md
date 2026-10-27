# framework: the module framework

What every client feature is built on (ADR 0046). A `Module` declares its settings, HUD elements
and event listeners; the framework subscribes them while it's on, reads its keybinds, draws its HUD
where the player put it, and saves everything in the active profile. The client generates each
module's settings screen from its settings alone.

No game types: features see the game only through this module, and `client`'s
`incari.client.modules` fills it in.

## Code map

- `Module`: the base class. Settings in field initializers (`settings().bool(...)`), then `hud(...)`
  and `on(Events.X, listener)` in the constructor; `onEnable`/`onDisable` hooks.
- `ModuleSystem`: holds the modules; ticks them with the game's `KeyInput`, draws the HUD on a
  `HudCanvas`, manages profiles.
- `event`: `Events` (the channels), `EventBus`, `MovementInput`.
- `setting`: `BooleanSetting`, `IntSetting`, `FloatSetting`, `ChoiceSetting`, `ColorSetting`,
  `KeySetting` (with `Key`), `Settings`, and `SettingVisitor` over them all.
- `hud`: `HudElement`, `HudCanvas`, `Anchor`, `Tone`; `WorldCanvas` for the world itself (beams,
  `BlockGhosts`' ghost blocks, boxes).
- `game`: `Game`, the game as modules see it; `Blocks`, its block states and the world's blocks.
- `config`: `Profiles`, `ProfileCodec` (the JSON), `ConfigStorage`.
- Test fixtures: `incari.framework.testing.TestGame`, the game's side faked for feature tests;
  `FakeBlocks`, a world of blocks.

## Invariants

- **No game, platform or TeaVM types** (`BoundaryTest`): the JDK's basics, JSpecify, and Gson's tree
  API in `config`.
- **A disabled module costs nothing**: its listeners aren't subscribed and its HUD elements aren't in
  the drawn list.
- **Firing an event allocates nothing**; nor do the listener arrays until someone (un)subscribes.
- **Game thread only.** Nothing here is thread-safe.
- **Ids are config keys**: a module's, a setting's or a HUD element's id never changes once shipped.

## How to test

```sh
./gradlew :framework:check
./gradlew :app-desktop:smokeModules   # a display: the menu, Toggle Sprint in a world, the profile saved
```
