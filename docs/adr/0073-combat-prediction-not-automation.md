# ADR 0073: Combat modules predict; they never play for the player

- Status: accepted
- Date: 2026-11-06

## Context

Players asked for Fast Crystal: an end crystal or respawn anchor gone as soon as they break it, not
a ping later, so the next one goes in its place sooner. The same name also covers cheats that place
and hit crystals by themselves (CrystalAura, AutoCrystal).

## Decision

- **A combat module may predict what the player's own click already did, and nothing more.** One
  click sends the one packet the game sends, at the time the game sends it. No packet is added,
  repeated, sent sooner or sent without a click. No cooldown or reach check is skipped.
- **What the server keeps comes back.** A crystal comes back after twice the ping (0.6 to 3 s)
  without the server's removal. An anchor is predicted inside the game's own block prediction, so
  the server's acknowledgement puts it back.
- **Only where the server agrees:** no crystal is taken away for a hit doing no damage (weakness
  with a bare hand) or for an invulnerable crystal; no anchor where it sets the spawn (the Nether).
- **Off by default**, with a toggle key like every module, since some servers ban even prediction.
- In code: `Game.predictBreaks` (`:framework`), `ModuleBreaks` (`:client`), three one-line hooks.

## Consequences

- Fast Crystal sends exactly what the vanilla client sends, so an anticheat sees nothing new.
- Auto-place, auto-hit and similar requests are refused under this ADR.
- The Rust build has it as a mod (`rust/mods/fastcrystal`) on the fork's prediction hooks
  (`object_attacked`, `block_used`, `native_take_crystal`, `native_predict_air`: fragment
  `incari_fast_crystal`), which keep to the same rule.
