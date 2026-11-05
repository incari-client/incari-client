# armorpotionview: Armor & Potion View

Two HUD elements: your armor and held item with durability (green, amber below half, red below a fifth), and your active effects with level and time left. The HUD editor shows a sample when there's nothing to show. ([ADR 0050](../../docs/adr/0050-first-modules-game-view.md))

| Setting | Kind | Default |
| --- | --- | --- |
| Show armor | on/off | on |
| Show held item | on/off | on |
| Durability | Percent, Uses left, Uses left of max, Bar only | Percent |
| Color by wear | on/off | on |
| Armor layout | Vertical, Horizontal | Vertical |
| Show effects | on/off | on |
| Effect names | on/off (off: icons and times) | on |
| Color by kind | on/off: helpful green, harmful red | off |
| Effect icons on the | Left, Right | Left |
| HUD: Armor | position and scale | bottom right, 1x |
| HUD: Effects | position and scale | top left, below Toggle Sprint's line and the FPS, CPS and reach readouts (y 62), 1x |

```sh
./gradlew :features:armorpotionview:check
```
