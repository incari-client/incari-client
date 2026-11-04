# norender: No Render

Leaves what costs frames out of the world drawn, on this client only. Made for bone meal farms:
hundreds of items and sparkles can take a Chromebook to no frames at all. What's hidden is still
there: items are still picked up, falling blocks still land.

| Setting | Kind | Default |
| --- | --- | --- |
| Items on the ground | Shown, Still (no bobbing or spinning), Hidden | Hidden |
| One item per stack | On/off, while items are drawn: a stack as one item, not a pile of up to five | On |
| Hide XP orbs | On/off | Off |
| Hide falling blocks | On/off: sand, gravel, anvils until they land | Off |
| Hide fireworks | On/off: rockets and their bursts | Off |
| Hide bone meal sparkles | On/off: the green sparkles of bone meal and villagers | On |
| Hide block bits | On/off: blocks breaking, the dust of running and landing | Off |
| Hide explosions | On/off: explosions' smoke clouds | Off |
| Hide all particles | On/off | Off |

The module holds a `NoRender` (`Game.overrideNoRender`); the client's `ModuleNoRender` checks it
where the game draws items and entities and makes particles. Hidden entities aren't extracted at
all, and hidden particles aren't made, so both cost nothing.

```sh
./gradlew :features:norender:check
```
