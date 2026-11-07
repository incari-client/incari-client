# platform-desktop: the LWJGL platform

`platform-api` on a normal JVM, over LWJGL (SDL3, OpenAL, STB): the desktop dev runtime, where
the game is debugged with a real debugger and profiler.

## Code map

`DesktopPlatform` is the whole platform; `app-desktop` installs it.

- `SdlWindow`, `SdlDisplays`, `SdlEventPump`, `SdlInput`, `SdlShell`, `SdlLogBridge`: SDL3.
  Windowing policy lives here (borderless-window fullscreen on Windows, exclusive-mode fallback,
  mouse confinement in exclusive fullscreen).
- `audio`: `OpenAlAudio` and its device, sources and buffers. It's also the reference the
  browser's sound is held to: `openAlReference` (below) measures how it pans and fades.
- `net`: `DesktopNetwork`: TCP sockets with a virtual thread per connection, and WebSockets
  and HTTP GETs (`fetch`) through the JDK's HTTP client (`java.net.http`).

## Invariants

- **Depends only on `platform-api` and LWJGL.** No game types (`BoundaryTest`).
- **Compiles against the LWJGL version the game ships** (`gradle/libs.versions.toml`); at run
  time the game's own jars are used.
- **Written fresh against LWJGL's API.** It replaces what Mojang's classes did; never copy them.

## How to test

```sh
./gradlew :platform-desktop:check
./gradlew :platform-desktop:openAlReference   # the desktop's panning and fading, measured
```

`openAlReference` plays a tone at known positions with OpenAL Soft writing to a WAV file (no sound
card needed), and checks the levels `platform-web`'s `Spatializer` copies (ADR 0013). Run it after
an LWJGL update.
