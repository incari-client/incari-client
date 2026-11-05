# coordinates: Coordinates

Your block position, facing, biome, and the matching spot in the other dimension (Nether ↔ Overworld). ([ADR 0052](../../docs/adr/0052-hud-modules.md))

| Setting | Kind | Default |
| --- | --- | --- |
| Layout | Lines, One line | Lines |
| Facing | on/off | on |
| Biome | on/off | on |
| Other dimension | on/off | on |
| Background | on/off | on |
| HUD: Coordinates | position and scale | top left, under the readouts (y 62), above the chat, 1x |

In the Nether, the Overworld spot is worked out from where you stand, not your block: x 100.9 is 807, not 800.

```sh
./gradlew :features:coordinates:check
```
