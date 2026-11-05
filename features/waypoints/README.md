# waypoints: Waypoints

Named, colored places in each world, shown over the world with their icon, name, distance and an optional beam, with arrows to those off screen. **N** marks where you stand, **U** lists them, and deaths leave one. The minimap shows them too.

Each waypoint has a name, color (12 presets or any hex), icon (circle, square, diamond, triangle, star, heart, hexagon, cross), position, and its own shown and beam switches.

[ADR 0051](../../docs/adr/0051-minimap-and-waypoints.md).

| Setting | Kind | Default |
| --- | --- | --- |
| This world's waypoints | button: Manage... (the list) | |
| New waypoint key | keybind | N |
| Waypoint list key | keybind | U |
| Names | Always, When looked at, Never | Always |
| Show distance | on/off | on |
| Beams | on/off (each waypoint has its own switch too) | on |
| Arrows at the edge | on/off | on |
| Size | 0.5x to 2x | 1x |
| Mark deaths | on/off (on a server: for the visit only, never saved, as one address can be many maps) | on |
| Clear deaths on arrival (within 4 blocks, after being 8 away) | on/off | on |

```sh
./gradlew :features:waypoints:check
```
