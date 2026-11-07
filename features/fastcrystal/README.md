# fastcrystal: Fast Crystal

An end crystal you hit, and a charged respawn anchor you set off, are gone on your screen at once,
not a ping later, so the next one can go in its place sooner. Prediction only
([ADR 0073](../../docs/adr/0073-combat-prediction-not-automation.md)): your click sends the one
packet it always did, nothing is clicked for you, and one the server keeps comes back. Off by
default.

| Setting | Kind | Default |
| --- | --- | --- |
| Crystals | On/off | On |
| Anchors | On/off: anchors outside the Nether (where they explode) | On |

The module holds `Game.predictBreaks`; the client's `ModuleBreaks` acts on it:

| Hook | What |
| --- | --- |
| `MultiPlayerGameMode.attack`, after the attack packet | Takes the crystal away, unless the hit does no damage or the crystal is invulnerable |
| `ClientPacketListener.handleRemoveEntities` | The server removed it: nothing to put back |
| `RespawnAnchorBlock.useWithoutItem`, client side | Sets the anchor to air in the game's block prediction; the server's acknowledgement restores it if it stays |

A crystal the server hasn't removed after twice the ping plus 0.3 s (0.6 to 3 s) comes back.

The Rust build's mod ([`rust/mods/fastcrystal`](../../rust/mods/fastcrystal/)) does the same on
Horizon's prediction hooks: `object_attacked` takes the crystal off (`native_take_crystal`), `tick`
puts back one the server kept (`native_return_crystal`), and `block_used` shows the anchor as air
under the use's sequence (`native_predict_air`).

```sh
./gradlew :features:fastcrystal:check
```
