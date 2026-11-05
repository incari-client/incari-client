# minimap: Minimap

A live map of the loaded world around you, round or square, turning with you or north-up, with waypoints, compass letters and coordinates. In the Nether it shows a slice at your height.

**M** opens the world map: drag, zoom, see waypoints (click to edit, right-click to add). With **Remember explored map** on, land you've seen is kept per world and dimension (`incari/map/`).

[ADR 0051](../../docs/adr/0051-minimap-and-waypoints.md).

| Setting | Kind | Default |
| --- | --- | --- |
| Shape | Round, Square | Round |
| Turn with you | on/off (off: north up, the arrow turns) | on |
| Zoom | 256, 192, 128, 96, 64, 48 or 32 blocks across | 96 |
| Zoom in key, Zoom out key | keybinds | =, - |
| World map key | keybind | M |
| Remember explored map | on/off | on |
| Show waypoints | on/off | on |
| Compass letters | on/off | on |
| Coordinates | on/off | on |
| HUD: Map | position and scale | top right, 1x |

The game's effect icons, also top right, move down below the map (and anything else of Incari's in their place) rather than over it (`ModuleSystem.effectIconsShift`, hooked in `Hud.extractEffects`).

```sh
./gradlew :features:minimap:check
```
