# platform-web: the browser platform

`platform-api` in the browser, reached from Java through TeaVM's JavaScript interop (JSO): the
canvas as the window, DOM input, WebSocket networking, an in-memory file system kept in OPFS, Web
Audio and WebRTC. Designs: input ADR 0010, networking ADR 0009, storage ADR 0011, sound ADR 0013.

## Code map

| Where | What |
| --- | --- |
| `WebPlatform` | The platform. `endFrame` waits for the next animation frame and logs a heartbeat |
| `Dom` | Every JavaScript snippet for input, fullscreen, Pointer Lock, the clipboard, dropped files |
| `DomEventQueue`, `DomKeys`, `CanvasSize`, `DroppedFiles` | Plain Java (JVM-tested): DOM events become SDL-style events between frames; `KeyboardEvent.code` → scancodes; F then a digit is that function key (F+3 = F3, F+0 = F10) for Chromebooks, not while typing text |
| `CanvasWindow`, `WebDisplays` | The window and displays |
| `audio/` | One `AudioContext` with the unlock prompt; decoding through the browser; `Spatializer` matches OpenAL Soft's panning |
| `WebSocketNetwork` | Server connections; callbacks queue events, `pollEvents` delivers them on the game's thread |
| `rtc/WebRtc` | Peer connections, data channels and the microphone, for voice and shared worlds (ADR 0019) |
| `WebProfiling` | Memory figures and per-frame rows in `window.__incariFrames` for the benchmark (ADR 0021) |
| `storage/` | `WebFileSystem` (flushes once a frame), `TreeFileSystem` (the in-memory tree, JVM-tested), `BrowserStore` and `src/main/js/storage-worker.js` (the worker that writes OPFS or IndexedDB) |

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

## How to test

```sh
./gradlew :platform-web:check
npm run check                       # includes the storage worker's tests
./gradlew :app-web:browserCheck -Pbrowser.steps=@browser-checks/input.steps
```

What headless Chromium can't show (real Pointer Lock, input method windows, the leave-page prompt)
is on the manual checklist in [`app-web/browser-checks/README.md`](../app-web/browser-checks/README.md).
