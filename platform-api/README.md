# platform-api: the platform interfaces

Everything the game needs from the machine it runs on, as Java interfaces with no game or library
types: the window and its events, input, audio, the graphics surface, files, clipboard, threads.
`platform-desktop` and `platform-web` implement it; the game reaches it through `client`'s hooks.

## Code map

- `Platform` (package `incari.platform.api`): lifecycle, the clock, event polling, and access to
  everything below. `Platforms` holds the one installed platform.
- `window`: `PlatformWindow`, `PlatformDisplays`, `PlatformEventListener` (window and input
  events), display modes, cursors, icons.
- `input`: keyboard state and key names; `Scancodes` and `KeyModifiers` define the values, which
  are SDL3's (USB HID usage ids, `SDL_Keymod` bits), the format the game's key handling reads.
- `audio`: output devices, sources, PCM buffers, the listener.
- `net`: `PlatformNetwork`, a byte stream (TCP) or a message stream (WebSocket) to a server, and a
  blocking, size-capped HTTP GET (`fetch`) for public files such as skins and server packs.
- `rtc`: `PlatformRtc`, WebRTC peer connections with voice and data channels, and the
  microphone, for Eagler voice chat and shared worlds; only the browser has it (ADR 0019).
- `storage`, `threads`, and `PlatformShell` (clipboard, links, message boxes).

[ADR 0005](../docs/adr/0005-platform-layer.md) explains the design.

## Invariants

- **Depends on nothing** but the JDK subset TeaVM supports and JSpecify
  (`gradle/module-boundaries.txt`, `BoundaryTest`).
- **Every public type has Javadoc**, including its threading and ownership rules.
- **Null-marked**: unannotated types are non-null; `@Nullable` marks the exceptions.

## How to test

```sh
./gradlew :platform-api:check
```
