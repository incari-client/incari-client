# keystrokes: Keystrokes

Your movement keys, mouse buttons (with clicks per second) and space bar, as you bound them, lit while held. ([ADR 0052](../../docs/adr/0052-hud-modules.md))

| Setting | Kind | Default |
| --- | --- | --- |
| Key size | 14-30 | 20 |
| Mouse buttons | on/off | on |
| Clicks a second | on/off | on |
| Space bar | on/off | on |
| Sneak and sprint | on/off | off |
| Custom pressed color | on/off, and a color | off |
| HUD: Keys | position and scale | top right, below the minimap and its coordinates (y 120), 1x |

Text always fits its key: a mouse button too narrow for "12 CPS" shows "12", and a name or count still too wide is drawn smaller.

```sh
./gradlew :features:keystrokes:check
```
