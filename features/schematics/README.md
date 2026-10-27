# schematics: Schematics

A structure from a file, placed in the world as see-through ghost blocks to build by. Move, turn and
mirror it where it stands; what's still missing shows as ghosts, wrong blocks as boxes, and a list
says what's left to place. Written clean-room ([ADR 0066](../../docs/adr/0066-schematics-clean-room.md)):
reads Litematica's files, but none of its code.

- **Files:** `.litematic` (every region, merged into one box), Sponge `.schem` versions 1 to 3
  (WorldEdit, Axiom), and the game's structure `.nbt`. Pre-1.13 MCEdit `.schematic` files are
  refused with a message. Files live in the game directory's `schematics` folder; the chooser opens
  it (in the browser: the file picker, or drop files on the window).
- **Old versions:** a block state this version can't read is upgraded with the game's data fixer
  (built only then, in the browser). A block the game doesn't have is left out, and the load says
  how many.
- **Placing:** it's loaded with its lowest corner where you stand. Turning and mirroring keep that
  corner; the blocks turn with it.
- **Checking:** the world is compared a 16-block section at a time, 8192 blocks a tick at most:
  where you build every few ticks, what's in range every two seconds, the rest in turn. Chunks the
  client hasn't loaded count as nothing yet.
- **Not in this version:** easy-place, editing or saving schematics, area selection, several
  schematics at once, keeping the placement across restarts.

| Setting | Kind | Default |
| --- | --- | --- |
| Schematic | button: Choose... | |
| Place / Turn / Mirror / Unload | buttons, once one is loaded | |
| Ghosts | Missing blocks, Every block, None | Missing blocks |
| Ghost opacity | 10-90% | 45% |
| Range | 16-128 blocks | 48 |
| Layers | All, Up to a layer, One layer | All |
| Show its box / Box color | on/off, color | on, `#55AAFF` |
| Mark wrong blocks | on/off | on |
| Wrong block / Wrong way / Extra block | colors | `#FF3333`, `#FFAA00`, `#FF55FF` |
| Place here, Turn, Mirror, Nudge, Up, Layer keys | keybinds (sneak reverses Turn, Nudge, Up, Layer) | unbound |

HUD: **Schematic** (name, blocks placed, problems, where it is) and **Material list** (items still
needed, most first; green when you carry enough).

```sh
./gradlew :features:schematics:check
```
