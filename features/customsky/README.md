# customsky: Custom Sky

Draws the skies resource packs bring in OptiFine's format (`optifine/sky/world0/sky<N>.properties`,
or MCPatcher's `mcpatcher/sky/world0`), over the game's own sky, in the overworld. Without it the
game ignores them. A pack without one leaves the sky as it is. ([ADR 0067](../../docs/adr/0067-custom-sky.md))

| Setting | Kind | Default |
| --- | --- | --- |
| Brightness | 10-100% | 100% |

| Pack key | Read |
| --- | --- |
| `source`, `blend`, `rotate`, `speed`, `axis`, `weather` | yes |
| `startFadeIn`, `endFadeIn`, `startFadeOut`, `endFadeOut` | yes (`endFadeOut` defaults as OptiFine's) |
| `biomes`, `heights`, `days`, `daysLoop`, `transition` | not yet: the layer shows everywhere, every day |
| `sun.properties`, `moon_phases.properties`, the Nether and End | not yet |

The module parses the files (`SkyLayer`, no game types); the client's `ModuleSkyLayers` finds them,
loads the textures while the module is on, and draws them after the sunrise, before the sun.

```sh
./gradlew :features:customsky:check
```
