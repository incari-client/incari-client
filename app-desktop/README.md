# app-desktop: the desktop dev runtime

Runs the game on a normal JVM through `platform-desktop`, so game logic can be debugged with a real
debugger and profiler before browser problems get in the way. `DesktopMain` installs the platform
and graphics backends, then starts the game's own `Main`.

```sh
./incari assets                              # once (~480 MB)
./gradlew :app-desktop:run                   # play offline in run/; -Pincari.username=<name> skips Edit Profile
```

## Smoke tests

Unattended sessions with injected input. Each needs a display and GPU, writes a report and
screenshots under `run/smoke-*/incari-smoke/`, and halts the JVM if the game hangs. **A task can end
`BUILD SUCCESSFUL` with failed checks: read `report.txt`.**

| Task | Checks |
| --- | --- |
| `smokeTest` | A world: movement, blocks, commands, inventory, chat, sound, LAN, a server ping, pause, quit |
| `smokeHost` + `smokeJoin` | A second client joins the first over LAN |
| `smokeServer -Pincari.server=<addr>` | Join a server, change dimension, reconnect |
| `smokeLegacy` | 1.8 sneaking, reach, cooldown, ladders and sword blocking on a 1.8 server |
| `smokeAuth` | Password login, then login by cookie, on an Eagler server |
| `smokeSkinA` + `smokeSkinB` | Eagler skins both ways |
| `smokeVoiceA` + `smokeVoiceB` | Voice signalling between two clients |
| `smokeOnlineA` + `smokeOnlineB` | Accounts, presence, friends, messages, blocking against an online service (`-Pincari.online`) |
| `smokeMesher` | Incari's mesher against the game's compiler, section by section |
| `smokeModules`, `smokeFeatures`, `smokeHudModules`, `smokeVisualModules`, `smokeScreenModules`, `smokeWaypoints`, `smokeCosmetics` | Modules turned on in a world, with screenshots |
| `smokeHudEditor` | Drag and resize in the HUD editor |
| `smokeScreens`, `smokeLook`, `smokeFit` | Screen edits and skins; the look in both GUI presets; text fitting at GUI scales 2–4 |
| `smokeLayout` | Every Incari screen at six GUI sizes from 320x240, in both fonts: no widget off the screen or over another, no text past its box. Problems in `layout.txt` |

`worldgenServer -Pworldgen=on|off` runs the dedicated server for ADR 0041's world generation check.

## Invariants

- **Wires one game to one platform.** Smoke code runs only when `incari.smoke` is set.
- **An unattended run never leaves a game behind.**

## How to test

```sh
./gradlew :app-desktop:check       # the layout rules' unit tests (LayoutRulesTest)
./gradlew :app-desktop:smokeLayout # after changing a screen's layout or Ui's widgets
```
