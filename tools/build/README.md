# tools/build: the `incari` CLI

Turns a fresh clone into a patched, editable Minecraft source tree, and edits to that tree back
into patch files. This is how the repo stays patch-only. Run it through `./incari` at the repo root.

| Command | Does |
| --- | --- |
| `./incari setup` | `fetch` + `decompile` + `apply`: a fresh clone to `work/client`; also clones the pinned Horizon into `work/horizon` (ADR 0068), warning instead of failing |
| `./incari fetch` | Download the pinned version's jar and libraries, checked against Mojang's SHA-1s |
| `./incari decompile` | Vineflower into `work/decompiled/<version>` (skipped if unchanged) |
| `./incari apply [--force]` | Rebuild `work/client` from the decompiled source, `classes.txt` and `patches/`. Refuses to drop unsaved work without `--force` |
| `./incari rebuild [--check]` | Regenerate `patches/` from `work/client`'s commits. `--check` fails if stale |
| `./incari assets` | Download the asset index and sounds for the desktop and dev server |
| `./incari build [--out <dir>] [--only fast\|compatible\|rust] [--no-sounds] [--html]` | Release builds as one folder (`dist/web/`, ADR 0028) or one HTML file (`--html`, ADR 0057). The folder has the experimental Rust build in `rust/` when this machine can build it (ADR 0068; skipped, saying why, otherwise). Never share either ([`docs/self-hosting.md`](../../docs/self-hosting.md)) |
| `./incari survey` | The TeaVM compile survey ([`tools/survey`](../survey/README.md)) |
| `./incari copycheck` | Fail on code copied from the game ([`tools/copycheck`](../copycheck/README.md)) |

Code: `Main`; `GameFiles` fetches, `Decompiler` decompiles, `ClientTree` applies and rebuilds,
`WebBuild` runs `build`, `Horizon` fetches and builds the Rust build ([`rust/`](../../rust/README.md)).

## `work/` (git-ignored)

```
work/
  cache/                 manifest, version JSON, client.jar, libraries, jar lists for Gradle
  decompiled/<version>/  Vineflower's output and a stamp of its inputs
  client/                git repo: tag vendor -> classes.txt applied (tag base) -> one commit per patch
  survey/<version>/      survey output
  horizon/               the pinned Horizon commit (Rust build); horizon-stage/ with rust/mods applied,
                         horizon-resources/<version>/, horizon-target/, horizon-out/; horizon-override/,
                         an INCARI_HORIZON_DIR checkout's files without its ignored builds
```

## Invariants

- **Nothing of Mojang's leaves `work/`.** Only patch files and `classes.txt` are tracked.
- **Deterministic:** pinned jar and Vineflower, fixed options, git run without user config, so the
  same commits give byte-identical patches.
- **Patches carry as little Mojang text as possible:** one line of context, no function names in
  hunk headers, no file deletions. `rebuild` rejects a commit that deletes a file or edits a class
  `classes.txt` owns.
- **Unsaved work is never silently dropped.**
- **Commit subjects are `<subsystem>: <summary>`;** the subsystem is the folder under `patches/`.

## How to test

```sh
./gradlew :tools:build:test     # a synthetic vendor tree: no network, no Mojang jar
./incari setup && ./incari rebuild --check     # what CI runs end to end
```
