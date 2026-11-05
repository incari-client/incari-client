# reachdisplay: Reach Display

How far your last hit reached, for a few seconds after it. ([ADR 0052](../../docs/adr/0052-hud-modules.md))

| Setting | Kind | Default |
| --- | --- | --- |
| Decimals | 1-3 | 2 |
| Hide after | Never, 1-10 s | 3 s |
| Mark long hits | on/off (past 3 blocks in amber) | off |
| HUD: Reach | position and scale | top left, under CPS (y 46), 1x |

```sh
./gradlew :features:reachdisplay:check
```
