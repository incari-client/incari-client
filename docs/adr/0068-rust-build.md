# ADR 0068: An experimental Rust build on Horizon

- Status: proposed
- Date: 2026-11-01
- Builds on: ADR 0001 (patch-only repo), ADR 0036 (Fast and Compatible builds), ADR 0046 (modules)

## Context

[Horizon](https://github.com/horizon-rust/horizon) (MIT) is Minecraft: Java Edition 26.3 rewritten
in Rust and checked against the Java game by differential harnesses. A fork,
[horizon-web](https://github.com/nobuildersnotools/horizon-web) (branch `web`), compiles its client
to `wasm32-unknown-unknown` with threads as Web Workers and draws through WebGPU. It opens
single-player worlds, joins Eagler servers and plays sound, and runs well in early tests.

Incari's two builds compile the same Java to WebAssembly two ways. Horizon shares none of that code,
so it can't be a third way to compile Incari: Incari's features have to be written again for it.
The aim is to ship it beside Fast and Compatible without a second project to maintain.

## Decision

- **A third build, Rust, marked experimental.** It's listed in `build.json` beside `fast` and
  `compatible` as `"rust": {"page": "rust/index.html"}` (format 2 still: older pages ignore it). The
  loading screen offers it only where `navigator.gpu` gives an adapter, never recommends it, and
  `?build=rust` picks it. Browsers without WebGPU are out of scope.
- **Its own page.** Picking Rust opens `rust/index.html`: Horizon's page, files and service worker,
  untouched. Incari's worker leaves `rust/` alone, and its file list doesn't name Horizon's files.
- **Horizon is pinned, fetched and never committed**, as the game is (ADR 0001).
  `incari.properties` names `horizon.repository` and `horizon.revision` (a commit on the fork's
  `web` branch). `./incari setup` clones it into `work/horizon` (a warning, not a failure, when
  it can't). `INCARI_HORIZON_DIR=<path>` builds a local checkout instead, for work on Horizon itself (its files
  as git sees them, copied into `work/horizon-override`: horizon-mixins refuses the symlink a
  checkout's own web build leaves).
- **One game download.** `horizon-prepare` reads the pinned 26.3 jars, libraries and assets
  `./incari setup` and `./incari assets` already fetched (`--client-jar`, `--libraries-dir`,
  `--assets-dir`, `--server-jar`). The prepared resources stay in `work/horizon-resources/` and are
  never committed or published.
- **Incari's changes are Horizon native mods**, in `rust/mods/<name>/`: horizon-mixins bundles that
  use only Horizon's native hook points (`"hooks": N`, never a pinned commit), so they keep building
  as Horizon moves. A missing hook point is added to the fork, and offered upstream. A change no
  mod can make is committed to the fork and the pin bumped; there's no second patch system. What
  every mod shares (settings pages, HUD boxes by corner) is a bundle of its own, `rust/mods/common`,
  with no hooks.
- **One build command.** `./incari build` builds Rust before Fast and Compatible: horizon-mixins
  (pinned too) applies `rust/mods` into `work/horizon-stage`, then Horizon's `web/package.py folder`
  builds its threaded and single-threaded modules into `app-web/build/rust/`. `--only rust` builds
  it alone. The web folder has it in `rust/`; the single file doesn't (`--html` leaves it out:
  Horizon's offline file is a page of its own).
- **Its tools are optional.** It needs Rust `nightly-2026-10-14` with `rust-src` and the
  `wasm32-unknown-unknown` target, `wasm-bindgen-cli` 0.2.129 and binaryen. Without them
  `./incari build` skips Rust and says why; Fast and Compatible still build.
- **Contracts are shared, code isn't.** Java and Rust can't share modules, so both read one source:

  | Contract | Source |
  | --- | --- |
  | Version, featured servers, online URL | `incari.properties`, turned into Rust constants at build time |
  | Accounts, friends, cosmetics | `docs/online-protocol.md` |
  | Settings and profiles | One file format in browser storage, so switching builds keeps them |
  | What a feature does | The feature's `README.md`, the spec for both ports |

- **Tracked per feature.** `features/README.md` has a Rust column (Ported, Not yet, Won't port), and
  `tools/ci/check-rust-status.sh` fails when a feature has none.
- **Releases don't wait for it.** When Horizon is behind (Incari on a newer game version, or a
  broken pin), `build.json` marks `rust` unavailable with a reason and the release goes out.
  Bumping `horizon.revision` is its own pull request.

## Consequences

- Every feature is written twice. Rust ports start with the PvP HUD and visual modules.
- No ViaVersion in the client: Horizon speaks only 26.3, so it joins servers whose proxy translates
  (Eagler servers with ViaVersion, Horizon servers). Servers that cap the version refuse it.
- Horizon's web port is experimental: its README lists page-side locks that can stop the game, no
  screenshots or world icons, and no online mode.
- CI runs the mods' own tests (`rust/`'s `cargo test`, `clippy`, `fmt`), not the Rust build: that
  needs the game's files. Whether headless Chromium (SwiftShader) can run WebGPU for `browserCheck`
  is unchecked.
- Renderer, networking and licensing reviewers (`.github/CODEOWNERS`) review the Rust build.
  Horizon is MIT, which GPL-3.0 Incari may include.
- `rust/` isn't a Gradle module, so `gradle/module-boundaries.txt` doesn't list it; its rules are in
  `rust/AGENTS.md`.

## Open questions

- Upstreaming the web port to horizon-rust/horizon, which would leave only the mods to maintain.
- Measurements: Rust against Fast on the same machine (`docs/benchmark.md` scenarios) before the
  loading screen describes it as faster.
