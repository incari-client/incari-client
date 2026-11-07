# platform-web: the browser platform

`platform-api` in the browser, reached from Java through TeaVM's JavaScript interop (JSO): the
canvas as the window, DOM input, WebSocket networking, an in-memory file system kept in OPFS, Web
Audio and WebRTC. Designs: input ADR 0010, networking ADR 0009, storage ADR 0011, sound ADR 0013.

## Code map

| Where | What |
| --- | --- |
| `WebPlatform` | The platform. `endFrame` waits for the next animation frame and logs a heartbeat |
| `Dom` | Every JavaScript snippet for input, fullscreen, Pointer Lock, the clipboard, dropped files |
| `DomEventQueue`, `DomKeys`, `CanvasSize`, `DroppedFiles` | Plain Java (JVM-tested): DOM events become SDL-style events between frames; `KeyboardEvent.code` → scancodes; on ChromeOS only (`?fchord=on`/`off` overrides), F then a digit is that function key (F+3 = F3, F+0 = F10), not while typing text; elsewhere F is reported at once. A lost WebGL context (`"graphics"`, from render-webgl2) keeps the window minimized |
| `CanvasWindow`, `WebDisplays` | The window and displays. The leave-page prompt is on while the mouse is held or the player is in a world (`setLeaveWarning`) |
| `audio/` | One `AudioContext` with the unlock prompt; decoding through the browser; `Spatializer` matches OpenAL Soft's panning |
| `WebSocketNetwork` | Server connections; callbacks queue events, `pollEvents` delivers them on the game's thread |
| `WebFetch` | `PlatformNetwork.fetch` over the browser's `fetch`; the calling thread suspends until the body is in |
| `rtc/WebRtc` | Peer connections, data channels and the microphone, for voice and shared worlds (ADR 0019) |
| `WebProfiling` | Memory figures and per-frame rows in `window.__incariFrames` for the benchmark (ADR 0021) |
| `storage/` | `WebFileSystem` (flushes once a frame; a change the store couldn't keep is sent again whole 5 s later; read-only, no-storage and failed saves become `PlatformNotice`s), `TreeFileSystem` (the in-memory tree, JVM-tested), `BrowserStore` and `src/main/js/storage-worker.js` (the worker that writes OPFS or IndexedDB, and names the paths it couldn't) |
| `JsChannel` | A game worker's channel; `sendWhenLeaving` has the page's own listeners (hidden, `beforeunload`, `pagehide`) post a message, so singleplayer's server saves with no game frame to come |

## Invariants

- **Only `platform-api` and TeaVM JSO.** No game types (`BoundaryTest`).
- **Never block the game's thread.** Its only wait is `endFrame`'s. Async browser APIs report back
  as queued events. Exception: the audio decoder suspends, but only sound-loader threads call it.
- **File calls never wait.** Files live in memory; storage hears of changes after each frame.
- **DOM listeners never call Java** (except the pure `DomEventQueue.consumesKey`). They push onto
  an array the game drains. A coroutine can't start from a JavaScript callback.
- **Keys belong to the game only while it has focus.** The host page's own controls keep theirs.
- **The page sizes the canvas with CSS.** The platform sets the backing store to CSS size × device
  pixel ratio.
- **A hidden page keeps answering servers.** Its frames' quarter-second fallback, and the background
  pool's, use the page's worker timer (`window.incariTimer`, app-web's `wakeups.ts`) while hidden:
  the page's own timers are held to one wake-up a minute after five minutes hidden.

## How to test

```sh
./gradlew :platform-web:check
npm run check                       # includes the storage worker's tests
./gradlew :app-web:browserCheck -Pbrowser.steps=@browser-checks/input.steps
```

What headless Chromium can't show (real Pointer Lock, input method windows, the leave-page prompt)
is on the manual checklist in [`app-web/browser-checks/README.md`](../app-web/browser-checks/README.md).
