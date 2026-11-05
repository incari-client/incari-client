# Building Incari for a web server

`./incari build` turns a set-up checkout into one folder any static web server can serve, or one
HTML file that plays from disk.

## Don't share builds

**Never share, upload, mirror or publish a build**, any part of one, or a link to a page serving one.

Every build contains Minecraft's code and files, which Mojang doesn't allow redistributing. So the
project publishes no builds anywhere, gives nobody permission to distribute one, and isn't
responsible for builds others make or pass around. A build also bundles ViaVersion and
ViaBackwards, whose GPL raises its own question when combined with Minecraft (ADR 0002).

If you put a build on a server, keep it to yourself or people you know (behind a login or on a
private network), never a public address, and take it down when done. The folder's `README.txt`
repeats this.

Incari isn't affiliated with Mojang or Microsoft. Everyone who plays needs to own Minecraft: Java Edition.

## Making the folder

Needs `./incari setup` (JDK 25, git, Node 24) and about 20 GB of free memory.

```sh
./incari build                    # Fast and Compatible into dist/web/ (git-ignored)
./incari build --out ~/incari-web # anywhere outside the checkout
./incari build --only fast        # one compile; Safari and older browsers can't play
./incari build --no-sounds        # no asset index (~460 MB less): silent, English only
```

| Build | Needs | Notes |
| --- | --- | --- |
| **Fast** (recommended) | JSPI: Chrome/Edge 137+, Firefox 153+ | Quicker, smaller download |
| **Compatible** | WebAssembly GC: Chrome 119+, Firefox 120+, Safari 18.2+ | Noticeably slower |

Where both run, the loading screen offers a choice and remembers it; `?build=fast|compatible` picks
directly.

It fetches the sounds if missing, compiles both release builds (~10 minutes each when code
changed), and copies everything into the folder. It refuses a folder git would track inside the
checkout, and only replaces a folder it wrote (marked `.incari-web-folder`). The release builds
replace the development ones in `app-web/build/`.

## What's in it

| File | What |
| --- | --- |
| `index.html`, `boot.js`, `loader.js`, `features.js`, `jspi.js`, `cache-policy.js`, `sw.js` | Loader page and service worker (ADR 0014) |
| `build.json` | Every file with size and hash |
| `incari.wasm`, `incari-fast.wasm`, `incari.wasm-runtime.js` | Compatible and Fast builds, TeaVM's loader |
| `mesher.wasm`, `mesher.wasm-runtime.js` | The terrain mesher's workers |
| `incari.pack` | Translated shaders, decoded sprites, Via's mappings |
| `client.jar` | Mojang's jar, resources only (12 MB) |
| `asset-index.json`, `assets/objects/` | Sounds, the Unicode font and languages, fetched as needed |
| `.htaccess`, `README.txt` | Apache settings; the notice and server configs |

## Defaults to check

A build talks to a few services by default. Change them in `incari.properties` (or with the Gradle
property named) before `./incari build` if they aren't what you want:

| Default | Setting | What it does |
| --- | --- | --- |
| Incari's online service, `wss://backend.incariclient.org/v1/ws` | `incari.online.url` (`-Pincari.online.url=`) | Accounts, friends, chat, cosmetics, the update notice and usage counts (ADR 0058 to 0062). Empty turns all of them off |
| Carrotcraft, a sponsored server pinned atop the server list | `incari.featured` | `name\|address` entries joined by `;`. Empty for none. The online service can replace the list |
| Eaglercraft's public relays (`relay.deev.is`, `relay.lax1dude.net`, `relay.shhnowisnottheti.me`) | Built into `WorldSharing` and `RelayScreen`; each player's list is `relays` in `incari/settings.txt` | Where shared worlds meet (ADR 0019, 0062) |

Usage counts are anonymous: a random install id, the version and the build kind, once a day. The
service keeps a keyed hash of the id for 35 days and totals by version and build. Players can turn
them off in Incari Settings, and nothing is sent when the online service is off.

## Serving it

Serve the folder as static files; every URL is relative, so a sub-path works. `README.txt` has nginx
and Caddy configs; `.htaccess` covers Apache.

- **HTTPS** or `localhost`. Over plain HTTP from another machine it still runs, but without the
  service worker or OPFS (saves go to IndexedDB).
- **`.wasm` as `application/wasm`**, so it compiles while downloading.
- **Compression** for `.wasm`, `.js` and `.json`.
- **Optional:** `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy:
  require-corp` let workers share memory instead of copying (a little faster).
- **An origin of its own.** Players who sign in with Microsoft keep their tokens in the page's
  storage, which every page on the same origin can read. Don't serve Incari beside other people's
  pages (a shared `user.github.io`, say): give it its own host or subdomain (ADR 0070).

Players join over WebSocket: EaglerXServer, or a vanilla server behind a bridge. A page on HTTPS
can only open `wss://` addresses.

## One HTML file

```sh
./incari build --html                         # both builds: dist/incari.html, 133 MB
./incari build --html --only fast             # Fast only, smaller
./incari build --html --out ~/incari.html
```

Plays when opened from disk, no server needed (ADR 0057). Same rules: never share it; the file says
so at the top.

- **Sounds:** every sound but music, re-encoded at 22 kHz (~40 MB; needs **ffmpeg** at build time).
  Without ffmpeg, only common sounds (~10 MB). `--no-sounds` drops them and the languages.
- **Storage:** IndexedDB (Chrome gives `file://` pages no OPFS), shared by every file opened from
  disk and kept in the browser profile.
- **No service worker**; nothing is fetched.
- Runs in Chrome, Edge and Firefox. It also works from a web server, but the folder is better there.
