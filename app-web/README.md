# app-web: the browser build

The game in a browser tab. `WebMain` installs `platform-web` and the WebGL2 backend, then starts the
game's own `Main`. `tools/webbuild` compiles it with TeaVM into two builds (ADR 0036):

| Build | Output | Threads | Runs in |
| --- | --- | --- | --- |
| Fast (`-Pteavm.fast`) | `build/teavm-fast/` | JSPI | Chrome/Edge 137+, Firefox 153+ |
| Compatible (default) | `build/teavm/` | TeaVM coroutines: slower, twice the size | anything with WebAssembly GC, Safari too |
| Rust (experimental) | `build/rust/` | Web Workers (Horizon's own) | WebGPU: Chrome and Edge 113+ ([`rust/`](../rust/README.md), ADR 0068) |

`-Pteavm.fast` also makes `browserCheck`, `screenshotDiff` and `benchmark` use Fast. The page offers
the builds that are compiled; delete a build's folder to leave it out. Rust isn't a TeaVM build:
`./incari build --only rust` writes it, and the page opens its own page, `rust/index.html`, with its
own files and service worker.

## Tasks

```sh
./gradlew :app-web:teavm                   # compile (~10 minutes); -Pteavm.release for an optimized build
./gradlew :app-web:serve                   # serve on http://localhost:8080/
./gradlew :app-web:browserCheck            # run headless until the game loop ticks
./gradlew :app-web:shaders                 # translate shaders only (seconds)
./gradlew :app-web:buildManifest           # page scripts, asset pack, build.json (seconds)
./gradlew :app-web:checkDownloadSize       # hold the download to download-budget.properties
./gradlew :app-web:screenshotDiff          # desktop vs browser renders of fixed scenes (ADR 0012)
./gradlew :app-web:benchmark               # profiler figures against a baseline (docs/benchmark.md)
```

Editing the page or its scripts needs no recompile: `serve` and `browserCheck` rebuild the manifest
and serve the page's files directly.

## Page options

| Option | Effect |
| --- | --- |
| `?build=fast` / `compatible` / `rust` | Pick a build, skipping the loading screen's choice. Rust's page gets the other options (Horizon reads `?args=`) |
| `?server=ws://host:port` | Join that server once the game starts |
| `?name=` | Offline player name; skips Edit Profile |
| `?world=&seed=` | Create or open a singleplayer world |
| `?options=<file>` | Start with that options file, fetched from beside the page |
| `?online=<url>`, `?updates=on` | Another online service; show the update notice in a dev build |
| `?profile`, `?overlay` | Profiler collecting from the start; its overlay (F9 in game) |
| `?tier=`, `?particles=` | Force a quality tier |
| `?sw=off` | No service worker (and unregister it) |
| `?renderer=headless` | Run the game loop without drawing |
| `?shaders=eager` | Link every WebGL program at start-up |
| `?multidraw=off`, `?drawcensus`, `?gpucensus` | Compare terrain drawing; count draws |
| `?<switch>=game` (or `off`, `all`) | Turn one optimization off to compare it. `WebMain` lists every switch |

## Driving the page in checks

`browserCheck` can act like a player over the DevTools protocol, from inline steps or a script:

```sh
./gradlew :app-web:browserCheck -Pbrowser.steps="shot:build/title.png;click:0.5,0.45;sleep:2000"
./gradlew :app-web:browserCheck -Pbrowser.steps=@browser-checks/input.steps
```

Steps cover screenshots, mouse, keys (`key:F3+KeyB`), typing and input methods, clipboard, viewport
size, page assertions (`holds:`, `until:`), reloads, file choosers, downloads, dropped files, test
resource packs and RCON commands on a test server. `BrowserCheck.runStep` lists them all. The
scripts and the checklist for what needs a real browser are in [`browser-checks/`](browser-checks/README.md).

## Code map

| Where | What |
| --- | --- |
| `WebMain` | Entry point and page options |
| `WebStorage` | The game's files under `/incari`, kept by `platform-web` (ADR 0011) |
| `ClientJar` | Mounts the client jar's resources read-only; the storage worker downloads it once |
| `WebAssets` | Sound list and pack metadata from the asset index; sounds fetched when first played (ADR 0013, 0037) |
| `WebShaders`, `AssetPack` | The build's translated shaders and asset pack (ADR 0014) |
| `LoaderPage`, `Fetch` | Loading-screen steps; downloads before the game starts |
| `headless/` | A graphics device that draws nothing |
| `src/main/web/index.html` | The loader page |
| `src/main/ts/` | `boot.ts` (loader), `loader.ts` (its decisions), `jspi.ts`, `features.ts` (feature detection), `sw.ts` and `cache-policy.ts` (service worker), `offline.ts` (single-file shim, ADR 0057) |

## Packaging

- `build.json` lists every file with size and SHA-256, so the service worker keeps each file until
  it changes. `incari.pack` holds build-time data (shaders, sprites, Via mappings). See ADR 0014.
- `./incari build` writes a folder for a static web server (ADR 0028); `--html` writes one HTML
  file (ADR 0057). Neither may be shared ([`docs/self-hosting.md`](../docs/self-hosting.md)).

## Download budget

`download-budget.properties` limits what a player downloads at start (gzip level 9, plus raw module
size; brotli reported). Every file is classed `download`, `debug`, `report` or `vendor` (Mojang's
jar, measured but not ours). An unclassed file fails the check. Raise a limit in the PR that needs
it, and say why. CI doesn't compile the browser build, so it only unit-tests the budget logic.

```sh
./gradlew :app-web:checkDownloadSize -PdownloadSize.skipBuild   # measure the existing build
```

## Screenshot diffs

Renders `screenshot-scenes/scenes.txt` on the desktop (OpenGL, the reference) and in headless
Chromium (WebGL2), on a fresh local server, and compares each pair by SSIM. Fails under a scene's
threshold, on any WebGL error, or on a skipped pipeline. Needs a display and GPU, so it runs
locally, not in CI. Output goes to `build/screenshots/` and is never committed.

```sh
./gradlew :app-web:screenshotDiff -Pscreenshots.skipBuild -Pscreenshots.only=sky,gui
./gradlew :app-web:screenshotDiff -Pscreenshots.reuseDesktop     # browser side only
./gradlew :app-web:screenshotDiff -Pscreenshots.gpu=swiftshader  # software WebGL, lower scores
```

Ports: `-Pscreenshots.port` (18210) and the five above it. The benchmark uses 18310 and four above.

## Probes

A full compile takes ten minutes. To test one behaviour of TeaVM or the class library, compile a
small program instead (seconds):

```sh
app-web/probes/run.sh <Name> [-Pteavm.fast]
```

[`probes/README.md`](probes/README.md) lists them. A one-off program runs the same way:
`./gradlew :app-web:browserCheck -Pteavm.main=Probe -Pteavm.classpath=<dir> "-Pbrowser.expect=probe done"`.

## Invariants

- **Builds are never committed or published.** The dev server binds to localhost only.
- **Mojang's assets and client jar are never copied into the build.** The page fetches them, and
  the storage worker keeps the jar outside the game's heap.
- **TypeScript is strict**, lint-clean, and uses only syntax Node's type stripping can erase, so
  tests run straight from source.

## How to test

```sh
npm ci && npm run check
./gradlew :app-web:browserCheck   # needs Chromium (Playwright's, or INCARI_CHROME)
```
