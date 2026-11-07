# Threading in the browser build

In the page, Java threads take turns on one core (JSPI fibers in Fast, coroutines in Compatible).
A blocking call suspends its thread and lets others run; nothing runs in parallel. So a wait is
harmless on a background thread, but on the render thread the frame waits as long as the call
does, and forever if what it waits for can only run on the render thread.

This is the audit of every blocking call (ADR 0015 has the decisions). Each build writes the list to
`app-web/build/teavm/blocking.txt` (`tools/webbuild`'s `BlockingCalls`).

## How the render thread is kept from waiting

| Rule | How |
| --- | --- |
| **The background pool runs between frames** | `IdleTimeExecutor` queues the game's pool tasks (meshes, occlusion graph, reloads) and runs them after each frame, until shortly before the next is due |
| **A task waiting for a worker steps aside** | Another pool runner (up to 16) takes the slot, so several meshes are in workers at once (ADR 0023) |
| **Long tasks pause at the slot's end** | `PlatformThreads.pauseIfLate()` between parts; section snapshots call it per layer |
| **Slots end before the next frame** | The deadline comes from the display's measured interval |
| **Slots get at least 4 ms** | Even after a late frame, while there's work (`?poolfloor=<ms>`, ADR 0035) |
| **A waiting render thread gets the pool at once** | Slots run immediately, 4 ms at a time, until the frame ends |
| **Woken threads skip the 4 ms timer clamp** | The server worker schedules through a `MessageChannel` (`wakeups.ts`, ADR 0040) |
| **The page paces frames, not sleeps** | The frame limiter's sleep is replaced by waiting for animation frames (`Platform.limitFrameRate`) |
| **A hidden page keeps time in a worker** | Hidden, its threads wake and its frames come from a worker's timer, not the page's throttled ones (`wakeups.ts`, ADR 0072) |
| **Stalls are counted** | The frame monitor (`window.__incariFrames`) counts frames where the render thread waited; `Waits` prints the stack of any wait of 50 ms or more |
| **The gate** | `browser-checks/soak.steps`: 30 minutes over fresh terrain with no render-thread wait of 50 ms |

## On the render thread

| Call | Waits for | In the browser |
| --- | --- | --- |
| `FramerateLimiter.limitDisplayFPS` | the rest of a limited frame | **Fixed:** the platform paces frames |
| `SectionOcclusionGraph.waitAndReset` | the occlusion graph, a pool task | On level or render-distance change; the pool runs at once |
| `RegistryDataCollector.loadNewElementsAndTags` | server registries on the pool | While joining, behind the loading screen |
| `SessionSearchTrees.*` | search trees built on world load | Recipe book or creative search opened early; the pool runs at once |
| `SoundEngine.play` | the sound thread to hand out a channel | Every sound; costs a thread switch, well under a frame |
| `SoundEngineExecutor.shutDown` | the sound thread to stop | On a resource reload |
| `Minecraft.getGameProfile` and friends | profile lookups from launch | Mojang's services are out of the build, so they fail at once |
| `Main.main` (DataFixer) | rule building on the pool | At start, before the first frame |
| Resource reload steps (`AtlasManager`, `ModelManager`, `TextureManager`, `FontManager`…) | earlier reload steps | Run as later steps, when the earlier ones are done |
| `ClientPacketListener.tick`, `ClientMannequin.tick`, `CommandSuggestions` | key pair, skin, suggestions | Only after `isDone()`: never waits |
| `PipelineCache.get` | a pipeline compile | WebGL2 compiles synchronously: never waits |
| `PlayerSocialManager.isBlocked` | block list refresh | Fails at once (no Mojang services) |
| `BlockTintCache`, `SectionRenderDispatcher` locks | a lock also taken on a worker | Holders never suspend while holding; `MonitorElision` makes these fail loudly otherwise |
| `ThreadingDetector.checkAndLock` | concurrent container use | A debug guard; contention would be a game bug |
| World loading (`doWorldLoad`, `loadWorldDataBlocking`…) | the integrated server | Singleplayer only (below) |
| `Minecraft.grabPanoramixScreenshot` | a debug command | Not reachable in play |

## On background threads (fine)

| Call | Thread |
| --- | --- |
| `SectionRenderDispatcher` compile and resort (`onSpinWait`) | pool; yields so the render thread can upload |
| `SoundEngineExecutor`, `SoundLoader` | sound thread, Incari's sound loader |
| Netty event loops, timers and promises | the connection's own threads |
| Guava futures | whoever calls them |
| The integrated server (`MinecraftServer`, `ChunkMap`, `IOWorker`, …) | its own thread (below) |
| `TracingExecutor.shutdownAndAwait`, `ClientShutdownWatchdog` | shutdown only |
| `Util$9.run` | sleeps forever by design (a Windows timer fix) |
| `LanServerPinger.run` | Open to LAN |
| `WebFetch.get` (skins, player heads, server packs) | the game's download pool, the pack download queue |

**Not in the browser's paths:** Realms, Mojang's services, online skin registration, the Chase
debug feature, Netty's NIO transport, and joins of already-finished futures in DataFixerUpper and
Brigadier.

## Singleplayer

The server runs in a Web Worker with its own threads, pool and files (ADR 0039).
`?singleplayer=page` runs it on the page instead (ADR 0038), which needed two more rules:

- **The game's own wait loops end their frame.** World load and save pump events from inside a
  loop; `SDLEventHandler.pumpEvents` ends the frame there, or the pool would never run.
- **A pool task that blocks gives up the pool** (`IdleTimeExecutor.beforeWait`), since world
  generation joins work it queued on the same pool.

Optimize World runs and waits on its own thread. `LevelStorageSource.deleteLevel` sleeps on the
render thread only between retries of a failed delete.
