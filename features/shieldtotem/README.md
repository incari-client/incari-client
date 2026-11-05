# shieldtotem: Shield & Totem HUD

Right of the hotbar, a slot's height up, the totems on the shield: your shield's durability (and "Disabled" during an axe cooldown), and how many totems you carry. ([ADR 0052](../../docs/adr/0052-hud-modules.md))

| Setting | Kind | Default |
| --- | --- | --- |
| Show shield | on/off | on |
| Color by wear | on/off | on |
| Show totems | on/off | on |
| Warn with none | on/off (a red x0; off hides it) | on |
| HUD: Shield, Totems | position and scale | 122 right of the screen's middle, 25 and 43 up: clear of the hotbar and a left-handed off hand's slot, 1x |

Both are 60 wide whatever they say, so the icons hold still as the text changes, and stay one column on narrow screens.

```sh
./gradlew :features:shieldtotem:check
```
