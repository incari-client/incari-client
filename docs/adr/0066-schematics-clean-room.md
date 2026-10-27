# ADR 0066: Schematics, written clean-room

- Status: accepted
- Date: 2026-10-25
- Builds on: ADR 0002 (the license)

## Context

Players asked for Litematica. Litematica and malilib are LGPL-3.0. Incari's Minecraft exception
(ADR 0002) can't be added to their code, so a port would have to stay LGPL in its own modules, carry
rebuild duties for anyone hosting a build, and risks LGPL §2 (a modified library that depends on the
application may have to go plain GPL). The author can't be expected to answer. A port is close to a
rewrite anyway: mixins become patches, malilib duplicates `framework`, the renderer must move to
`GpuDevice`.

## Decision

- **Clean-room implementation.** No Litematica or malilib code. Nobody writing it reads their source.
  Work from the file format and observed behaviour only.
- **Compatible, not branded.** Reads `.litematic` files. Not called Litematica.
- **Formats:** `.litematic` first; vanilla structure `.nbt` and Sponge `.schem` after. All are NBT.
- **Where code goes:** `features/schematics`: format readers, placement, material list, verifier,
  no game types. `client` adapter: block-state lookup, world reads, ghost rendering through
  `GpuDevice`. Hooks are patches, per AGENTS.md.
- **First version:** load, place, move, rotate, mirror, ghost render, material list, highlight
  missing and wrong blocks. Not in scope: easy-place, schematic editing and saving, area tools.

## `.litematic` format

Gzipped NBT. Enough to implement without reading Litematica:

- Root: `Version` (int), `MinecraftDataVersion` (int), `Metadata` (compound: `Name`, `Author`,
  `EnclosingSize`, `TotalBlocks`, `TimeCreated`, ...), `Regions` (compound, one entry per region).
- Region: `Position` and `Size` (`x y z` ints; size components can be negative, meaning the region
  extends the other way from `Position`), `BlockStatePalette` (list of `{Name, Properties}`),
  `BlockStates` (long array), `TileEntities`, `Entities`, `PendingBlockTicks`,
  `PendingFluidTicks`.
- `BlockStates`: palette indices, `max(2, ceil(log2(paletteSize)))` bits each, packed tightly so
  an entry can span two longs (unlike vanilla's padded chunks). Index order is
  `y * sizeX * sizeZ + z * sizeX + x` over absolute sizes.
- Upgrade block states older than the target's `MinecraftDataVersion` with the game's DataFixer.

Check each point against real files before relying on it.

## Consequences

- No LGPL modules, no extra notices, no rebuild duties on hosted builds.
- Reviewers reject code that mirrors Litematica's structure or names. `./incari copycheck` covers
  only game code, so this is review-enforced.
- AI-written code: the model may have seen Litematica in training. Implement from this ADR and real
  files, and keep the design Incari's own.
