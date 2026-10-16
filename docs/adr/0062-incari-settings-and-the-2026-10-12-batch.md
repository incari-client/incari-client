# ADR 0062: Incari Settings, and the 2026-10-12 batch

- Status: accepted
- Date: 2026-10-12
- Builds on: ADRs 0019, 0039, 0046, 0048, 0054, 0058–0061

## Context

One batch of seventeen features and two fixes, with these decisions made up front:

- **Short Swords:** first-person swords smaller, with a slider; other items unchanged.
- **An Incari icon**, drawn in code (there was only the wordmark).
- **Theme fonts:** a few open fonts in Incari's pack, loaded only when picked.
- **Usage counts:** on by default with a switch; anonymous, once a day.
- **Featured server:** Carrotcraft, pinned, clearly Sponsored, not deletable; replaceable by the service.
- **Incari's badge** beside other Incari players' names, with an opt-out.
- **Presets:** 1.8 PvP, Modern PvP, QoL (module lists in `Presets`).
- **Feedback** to GitHub Discussions, **bugs** to Issues.
- **Shared worlds** through Eaglercraft's public relays; **the recorder** as Eaglercraft 1.8's.
- **The pause menu** keeps one Incari button.

## Decision

| Item | What was built |
| --- | --- |
| Incari Settings | One pause-menu button with the new icon (`Icons.INCARI`, a hexagon with an I). `IncariSettingsScreen` opens the module menu, HUD editor, Wardrobe, screenshot gallery, recorder, chat and account, and holds the device's own switches (`ClientSettings`, `incari/settings.txt`) |
| Screenshots | No longer a module: the gallery is the client's, from Incari Settings. "Copy screenshots" is a switch there |
| Always-on modules | `Module.alwaysOn`: on from load, hidden from cards and presets, no toggle key. Theme is one: **Theme** in the module menu's sidebar |
| Theme fonts | `UiFont`: Red Hat Display, Minecraft (pixel), Inter, Nunito, Poppins, JetBrains Mono, Michroma. The four new ones are OFL, subset to 406 characters (72 KB gzip together). `SmoothFont.useFont` swaps the font in place, scaled to Red Hat Display's metrics. Share codes gain an eighth field; old codes still paste |
| Short Swords | `HeldItem.sword()` and a scale; 40–100%, default 70%, optionally not while blocking |
| Presets | `ModuleSystem.applyPreset`: switch to a profile of the preset's name (copied the first time) with exactly its modules on; settings kept |
| Featured server | `incari.featured` in `incari.properties`; `hello` may replace it. `FeaturedServers` adds them on each load, never saves them, and protects them from edit and delete; a **Sponsored** label |
| Usage counts | `count`: random install id, version and build kind, once a UTC day; off in Incari Settings. The service keeps a keyed hash per day (35 days) |
| Badge | Account setting "Show others I use Incari" (default on). `Badges` puts one glyph of a bitmap font (`incari:badge`) before the name tag, the tab list name and the first mention of the player's name in each chat line (matched by tab list name, since most servers send chat as plain text): no new drawing in the world. Only signed-in players see badges |
| Screen recorder | `PlatformRecorder` (browser only): canvas + game sound (+ mic) into a `MediaRecorder` (MP4 or WebM), downloaded on stop. F8 or Incari Settings; blinking REC. ~19 MB a minute in memory |
| Shared worlds | World Options' LAN shares a worker world through a relay (`RelayHost`): a join code in chat, up to 8 players. WebRTC lives on the page (`WorldSharing`) and relays to the worker (`GuestLink`). Incari hosts send a `HostHello`, so Incari guests join natively; Eaglercraft 1.8 clients can't join an Incari host (would need ViaRewind) |
| Repository and links | `github.com/incari-client/incari-client`; Report Bugs → issue template; Give Feedback → Discussions |
| Mojang's social features | Player Reporting removed; every Friends button opens Incari Chat |
| Tab title | Stays "Incari Client" |
| Death waypoints | Removed when the player returns within 4 blocks after being 8 away |

### Fixes

- **Text drawn over itself:** the game's shadow and bold redraw text a whole GUI unit away, which
  reads as a second copy with anti-aliased fonts. `SmoothFont` uses half a unit.
- **Missing sounds:** a failed first download left a sound silent all session and held its channel.
  Downloads now retry three times (`Fetch.bytesRetrying`); a sound that still fails plays a moment
  of silence so the channel returns, and retries five seconds later (`sound-retry.steps`).
- **The HTML file's sounds:** it had none. Now it carries every sound but music, re-encoded by ffmpeg
  as 22 kHz Ogg Vorbis (84 → 40 MB; file 87 → 133 MB). Without ffmpeg, only common sounds (~10 MB).

## Alternatives rejected

- **Fonts from Google Fonts at run time:** a third-party request from school devices, and nothing offline.
- **A drawn quad or entity for the badge:** a font glyph rides the name tag's existing drawing and culling.
- **Data channels moved into the worker:** Chrome can't hand them over reliably.
- **ViaRewind for Eaglercraft 1.8 guests:** another GPL component, and a 26.3 world shown to 1.8 loses most blocks.

## Verified

- Desktop: `smokeScreenModules`, `smokeLook`; unit tests across the framework, Theme, Short Swords,
  Waypoints, `:online` (`Adr0062Test`), `:net` (`RelayHostTest`), `FeaturedServersTest`.
- Chrome: `incari-settings.steps` (recording, sounds), `share-host.steps` + `share-guest.steps` (a
  guest joins, sees the world, chats). A pattern `switch` broke TeaVM's linking; `RelayHost` uses `instanceof`.
