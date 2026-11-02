# Incari architecture

Incari compiles Minecraft's Java client to WebAssembly with TeaVM. The game's code runs almost
unchanged. Everything underneath it is different. The reasons behind each decision are in
[`docs/adr/`](docs/adr/README.md), and terms are defined in [`docs/glossary.md`](docs/glossary.md).

## Layers

| Layer | What | Where |
| --- | --- | --- |
| 5. Incari features | Module framework, modules, UI kit and look | `framework`, `features/*`, `online`, `client`'s `modules` and `ui` |
| 4. Performance rewrites | Worker mesher, batched terrain, culling, memory diet, start-up | `mesher`, `render-webgl2`, `client`, patches |
| 3. Minecraft client | The decompiled 26.3 client, compiled nearly as-is | `work/client` (generated), built by `client` |
| 2. Java libraries | Mojang's pinned libraries (fastutil, Guava, Gson, JOML, Netty…), ViaVersion | Mojang's jars, Maven |
| 1. Platform | Window, input, audio, graphics surface, networking, storage, threads | `platform-api`, `platform-desktop`, `platform-web` |

The game reaches the machine only through layer 1. On the desktop that's LWJGL, so the game can be
debugged on a normal JVM. In the browser it's canvas, DOM events, Web Audio, WebSocket and OPFS.

## Modules

`gradle/module-boundaries.txt` lists which module may depend on which, and `./gradlew check` fails
on anything else. ArchUnit tests add package rules (no LWJGL in shared code, no game types in the
platform layer).

```
            app-desktop                         app-web
            /        \                        /   |    \
   platform-desktop   client ─── render-webgl2   |   platform-web
            \      /  / |  \                      |       /
             \ mesher via |  net                  |      /
              \         |  /                      |     /
              └──── platform-api ─────────────────┴────┘
```

| Module | Owns | May depend on |
| --- | --- | --- |
| `platform-api` | The `Platform*` interfaces: window, input, audio, files, threads, worker pools, WebRTC, profiling | nothing |
| `platform-desktop` | The LWJGL implementation, for development | `platform-api` |
| `platform-web` | The browser implementation (Java over TeaVM JSO, plus TypeScript): file system and storage worker, WebRTC, Web Workers | `platform-api` |
| `net` | WebSocket transport under the game's Netty pipeline; Eagler's handshake, messages, skins and relays | `platform-api` |
| `via` | ViaVersion and ViaBackwards as a pipeline stage, to join servers of other versions | nothing |
| `mesher` | Terrain mesher: section snapshot in, vertices out, byte-identical to the game's. Also compiled alone for workers | nothing |
| `client` | The patched game plus the adapter code its patches call. Package map in [`client/README.md`](client/README.md) | `platform-api`, `net`, `via`, `mesher`, `framework`, `online`, `features/*` |
| `framework` | Module framework: `Module`, events, settings, keybinds, HUD elements, profiles, waypoints, the game as modules see it. No game types | nothing |
| `online` | Incari accounts, friends, chat, cosmetics, client side. No game types | `platform-api`, `framework` |
| `features/<name>` | One module each, with its own README | `framework` only |
| `render-webgl2` | WebGL2 backend under the game's renderer | `client` |
| `web-classlib` | JDK classes TeaVM lacks, and LWJGL stand-ins, for the browser build | nothing |
| `app-desktop` | Desktop runtime: installs `platform-desktop`, starts the game | `client`, `platform-desktop` |
| `app-web` | Browser build: entry point, mesher worker, loader page | `client`, `platform-web`, `web-classlib`, `render-webgl2` |
| `tools/*` | Build CLI, TeaVM compile, shader and sprite translation, dev servers ([`tools/README.md`](tools/README.md)) | see `module-boundaries.txt` |
| `rust/` | Not a Gradle module: Incari on Horizon, the experimental Rust build: the client (`incari_core`, its look, screens and framework) and its feature mods ([`rust/README.md`](rust/README.md), ADR 0068, 0069) | Horizon's hook points only |

**Why `client` is one compilation:** patches call Incari code that uses game types, so the two can't
be compiled separately. The vendor half is compiled without lint, and `strictCompile` rechecks
Incari's half with the same gates as every other module. Put logic in the strict modules and keep
`client`'s own code a thin adapter.

## The vendor tree

The repo never contains Mojang code ([ADR 0001](docs/adr/0001-patch-only-repo.md)). `./incari setup`
downloads the pinned jar, decompiles it with Vineflower, and builds `work/client`: a git repo of the
vendor commit, then `patches/classes.txt`, then one commit per patch.

- **Decompile fixes** (`patches/decompile/`) make the tree compile and change no behaviour (ADR 0004).
- **Incari patches** are one- or two-line hooks, wrapped in `// Incari start: <reason>` and `// Incari end`.
- **Whole-class replacements** go in `patches/classes.txt`, so no Mojang file is republished in a diff.

## Platform layer

`platform-api` describes what the game needs in Incari's own terms, with no game or library types.
The game reaches it through replacement classes (Mojang's window, event pump and audio, rewritten)
and small hooks (ADR 0005).

Graphics plugs in lower: Mojang's renderer already has a backend API (`GpuDevice` in
`com.mojang.renderpearl`). The desktop keeps Mojang's OpenGL backend; the browser uses
`render-webgl2`, with shaders translated at build time (ADR 0007, 0012).

## Browser constraints that shape the code

- **Threads are cooperative.** Java threads take turns on one core (JSPI fibers in the Fast build,
  coroutines in Compatible). Anything that blocks has to become async or move to a worker. The background
  pool runs in the time a frame leaves over ([`docs/threading.md`](docs/threading.md), ADR 0015).
- **Workers share no objects.** Each worker is its own WebAssembly instance. Work crosses as flat
  data in a `SharedArrayBuffer` or a transferred `ArrayBuffer` (ADR 0023).
- **Off-heap memory is linear memory.** The game's native buffers live in the module's linear
  memory, with real addresses and no copy on upload (ADR 0008).
- **Closed world.** TeaVM sees the whole program at compile time, so reflection has to be declared or removed.
- **Files are in memory.** The game's file calls are synchronous and browser storage isn't, so files
  live in memory and a storage worker writes changes behind (ADR 0011).
- **Memory is budgeted.** A 4 GB Chromebook kills the tab when it runs out of memory. The benchmark holds
  heap, linear memory and GC pauses to budgets (ADR 0025).
- **Start-up only pays for the title screen.** Models load after it shows, and sprites are decoded at
  build time (ADR 0026).

## Where to start

| To | Read |
| --- | --- |
| Set up and edit game code | [`CONTRIBUTING.md`](CONTRIBUTING.md), [`tools/build/README.md`](tools/build/README.md) |
| See why things are built as they are | [`docs/adr/README.md`](docs/adr/README.md) |
| Measure performance | [`docs/benchmark.md`](docs/benchmark.md) |
| Work as an agent | [`AGENTS.md`](AGENTS.md) and the module's own `AGENTS.md` |
| Change a module boundary | `gradle/module-boundaries.txt`, then an ADR |
