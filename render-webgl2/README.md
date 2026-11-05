# render-webgl2: the WebGL2 graphics backend

Mojang's `GpuDeviceBackend` API over WebGL2, under the game's own frontend device, which validates
every call first (ADR 0007). Every vanilla pipeline runs (ADR 0012). The browser build installs
`WebGl2Backend` with the canvas and the shaders `tools/shaders` translated.

## Code map

| Where | What |
| --- | --- |
| `WebGl2Backend` | Makes the WebGL2 context and the frontend device |
| `WebGl2Device` | Buffers, textures, samplers, surface, encoder; reported limits; draw counts and GPU time for the profiler |
| `PipelineCompiler`, `WebGl2Pipeline` | Pipelines from translated shaders, linked on first use |
| `WebGl2CommandEncoder`, `WebGl2RenderPass` | Commands, run as recorded. Terrain's indirect draws become a few `multiDrawElementsWEBGL` calls (ADR 0022) |
| `QuadIndices`, `DrawRows`, `DrawIdRewrite` | Shared quad indices; per-draw data read by `gl_DrawID` |
| `TexelBufferTexture` | A texel buffer emulated as a 2D texture |
| `shaders` | The translated-shader manifest format, shared with `tools/shaders` |
| `WebGl`, `Staging` | Every JavaScript snippet; how bytes reach WebGL |

## Invariants

- **The game's copy of a buffer is the truth.** WebGL2 can't map buffers, so writes are uploaded.
- **Bytes reach WebGL through linear memory.** The game's memory is viewed in place; anything else
  is copied into one staging buffer first.
- **Depth runs 0 to 1 where the browser allows** (EXT_clip_control), as on the desktop.
- **Extensions are detected, each with a fallback:** clip control, float blending, anisotropy,
  polygon mode (wireframe).
- **State is cached.** `GlState` and `Bindings` skip calls that change nothing; on a Chromebook
  call overhead, not the GPU, limits the draw rate.
- **Uploads bind on the last texture unit**, so draw bindings stay cached.
- **Only terrain draws indirectly, one instance per draw.** The multi-draw path relies on it.
- **Depends only on the game's render API, `incari.client.render` and TeaVM JSO** (`BoundaryTest`).
- **A lost context isn't rebuilt.** The game keeps no copies of what it put on the GPU. `WebGl`
  cancels `webglcontextlost`, shows a "reload the page" box, and tells platform-web through the
  canvas's event sink, which reports the window minimized (no drawing, 10 frames a second, the world
  saved). The surface reports the loss to the game once, not every frame.

## Debugging

- `window.__incariWebGl` (updated each frame): frames, draws, skipped draws.
- `browserCheck`'s `init:` step runs a script before the page loads, to wrap WebGL calls.
- The profiler overlay (F9 or `?overlay`): draw calls, terrain sections, GPU time.
- `?multidraw=off` draws terrain a section at a time.

## How to test

```sh
./gradlew :render-webgl2:check
./gradlew :app-web:screenshotDiff          # against the desktop (needs GPU and display)
./gradlew :app-web:browserCheck -Pbrowser.query=shaders=eager   # link every program at start
./gradlew :app-web:benchmark               # docs/benchmark.md
```
