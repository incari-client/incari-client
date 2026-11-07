# features

One client module per folder, each a `Module` built on `:framework` (ADR 0046). Every module but
Theme (always on) also has a **toggle key** setting, unbound by default, not repeated in each
README. How to write and add one: [`AGENTS.md`](AGENTS.md). **Rust** says whether the experimental
Rust build has it (ADR 0068, 0069): its settings and HUD are the Rust client's own
([`rust/mods/incari`](../rust/README.md)), its behaviour a mod in `rust/mods`.

| Module | Category | What it does | Rust |
| --- | --- | --- | --- |
| [1.7 Animations](oldanimations/README.md) | Visual | 1.7's block-hit swing and first-person poses | Ported |
| [Armor & Potion View](armorpotionview/README.md) | HUD | Armor durability and active effects | Ported |
| [Block Outline](blockoutline/README.md) | Visual | Your own block outline colour and width | Ported |
| [Clear Chat](clearchat/README.md) | HUD | No dark box behind chat | Ported |
| [Cooldown Indicator](cooldownindicator/README.md) | Combat | Attack cooldown as a bar, ring or percentage | Ported |
| [Coordinates](coordinates/README.md) | HUD | Position, facing, biome, other-dimension spot | Ported |
| [CPS](cps/README.md) | HUD | Clicks per second | Ported |
| [Crosshair Editor](crosshair/README.md) | Visual | A crosshair of your own design | Ported |
| [Custom Scoreboard](customscoreboard/README.md) | HUD | Place and scale the server's sidebar | Ported |
| [Custom Sky](customsky/README.md) | Visual | Resource packs' skies, OptiFine's format | Ported |
| [Enchant Glint](glintcolor/README.md) | Visual | Enchantment shimmer in your colour | Ported |
| [Fast Crystal](fastcrystal/README.md) | Combat | Crystals and anchors gone as you break them, not a ping later | Ported |
| [FPS Display](fpsdisplay/README.md) | HUD | Frames per second | Ported |
| [Freelook](freelook/README.md) | Visual | Look around without turning | Ported |
| [Fullbright](fullbright/README.md) | Visual | Caves and nights as light as day | Ported |
| [Hitboxes](hitboxes/README.md) | Combat | Entities' hitboxes in your color | Ported |
| [Hit & Kill Effects](hiteffects/README.md) | Combat | Particles on hits and kills | Ported |
| [Hit Color](hitcolor/README.md) | Visual | Hurt flash in your colour | Ported |
| [Keystrokes](keystrokes/README.md) | HUD | Keys and mouse buttons, lit while held | Ported |
| [Low Fire](lowfire/README.md) | Visual | Fire overlay drawn lower | Ported |
| [Low Shield](lowshield/README.md) | Visual | Shield drawn lower | Ported |
| [Minimap](minimap/README.md) | HUD | Live map, world map, remembered exploration | Ported |
| [No Hurt Cam](nohurtcam/README.md) | Visual | No camera tilt when hurt | Ported |
| [No Render](norender/README.md) | Visual | Hide items on the ground, XP orbs, particles | Ported |
| [Old Textures](oldtextures/README.md) | Visual | Programmer Art on and off | Ported |
| [Reach Display](reachdisplay/README.md) | Combat | Your last hit's reach | Ported |
| [Saturation](saturation/README.md) | HUD | Saturation shown on the hunger bar | Ported |
| [Schematics](schematics/README.md) | Utility | Ghost blocks of a schematic to build by | Ported |
| [Shield & Totem HUD](shieldtotem/README.md) | HUD | Shield durability and totem count | Ported |
| [Short Items](shortitems/README.md) | Visual | Smaller first-person held items | Ported |
| [Shulker Tooltips](shulkertooltips/README.md) | Utility | Shulker contents as a grid | Ported |
| [Theme Editor](theme/README.md) | Visual | Client colours and font (always on) | Ported |
| [Toggle Sprint](togglesprint/README.md) | Movement | Latch sprint and sneak | Ported |
| [Waypoints](waypoints/README.md) | Utility | Marked places over the world | Ported |
| [Weather Changer](weatherchanger/README.md) | Visual | Your own weather and time of day | Ported |
| [Zoom](zoom/README.md) | Visual | Hold C to zoom in, as OptiFine's | Ported |

Test one module with `./gradlew :features:<name>:check`.
