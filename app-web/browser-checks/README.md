# Browser check scripts

Step scripts for `./gradlew :app-web:browserCheck`, one step per line (`#` comments). They play the
game in headless Chromium over the DevTools protocol and fail on the first `assert:` or `waitfor:`
that doesn't hold. Positions are fractions of the 1280×720 window. Screenshots go to
`build/browser-checks/`. `BrowserCheck.runStep` lists every step.

```sh
./gradlew :app-web:browserCheck -Pbrowser.steps=@browser-checks/input.steps
```

The page opens as `?name=Player` (straight to the title screen) unless the query names someone;
`-Pbrowser.query=name=` opens Edit Profile first.

## Scripts

| Script | Checks | Needs |
| --- | --- | --- |
| `input.steps` | Text fields, copy/paste, input methods, key names on a French layout, F11, resize, dropped files, Pointer Lock | — |
| `loader.steps` | Loading screen, manifest, service worker caching, reload from cache | — |
| `storage.steps` | An option, a server and an imported pack survive a reload; F2 stores a PNG | — |
| `assets.steps` | Japanese from the asset index: the Unicode font and the language file | assets |
| `audio.steps` | The sound unlock prompt, menu music after the first click, click sounds | assets |
| `audio-positional.steps` | Panning, rear attenuation and distance fade within 1% of the desktop's OpenAL | assets, server |
| `server-join.steps` | Join over `ws://`, chat, the Nether and back, a transfer, a kick and rejoin | server |
| `version-join.steps` | Join a 1.8.9, 1.12.2 or 1.20.4 server through Via; idle 20 s; kick and rejoin | old server |
| `screenshot.steps` | F2 in a world, then the chat link downloads the PNG | server |
| `community-pack.steps` | A real community resource pack loads | a pack |
| `soak.steps` | 30 minutes over fresh terrain; fails if the render thread ever waits 50 ms | server, GPU |
| `voice-talker.steps`, `voice-listener.steps` | Eagler voice chat between two browsers | EaglerXServer |
| `relay-join.steps` | Join a world shared through an Eagler relay | relay |
| `share-host.steps`, `share-guest.steps` | Host a singleplayer world on a relay; a second page joins it | relay |
| `singleplayer.steps` | Create, play, save and reopen a world; World Options; zip download and import; delete | — |
| `singleplayer-lan.steps` | Open to LAN with the server on the page fails cleanly | — |
| `modules.steps` | Module menu, Toggle Sprint, its settings, the profile saved | — |
| `hudeditor.steps` | HUD editor: drag an element, blurred backdrop, saved anchor | — |
| `waypoints.steps` | Minimap, a new waypoint, its tag and beam, the list, the world map, saved data | — |
| `newmodules.steps` | Each module from ADRs 0052–0054 turned on and seen working | — |
| `incari-settings.steps` | Block-break sounds, Incari Settings, the screen recorder (F8) | — |
| `look.steps`, `menus-look.steps` | The Frost look in Potato and Normal; menus checked for overlapping text | — |
| `old-textures.steps` | Built-in packs listed; Old Textures switches Programmer Art on and off | — |
| `sound-retry.steps` | Sounds still play when their first downloads fail | assets |
| `legacy-sounds.steps` | Block sounds on a 1.8.9 server through Via | old server |
| `html-sounds.steps` | The single HTML file plays sounds from disk (`-Pbrowser.html=<file>`) | an HTML build |

Not covered: typing in chat and most F3 keys (check by hand). `input.steps` stops on the Resource
Packs screen because leaving it reloads resources. Ctrl+W and Ctrl+T aren't sent: Chromium acts on
them outside fullscreen.

## Setting up what checks need

**Assets:** `./incari assets` once. The dev server serves them from `work/cache/assets`.

**Server** (`server-join`, `screenshot`, `soak`, `audio-positional`): a vanilla 26.3 server behind
`tools/wsbridge`, with RCON, played as `IncariWeb`. The jar is Mojang's: keep it in `run/` (ignored),
never commit it or its world.

```sh
mkdir -p run/server && cd run/server && echo eula=true > eula.txt
cat > server.properties <<'EOF'
server-ip=127.0.0.1
server-port=25582
online-mode=false
accepts-transfers=true
white-list=false
enable-rcon=true
rcon.port=25592
rcon.password=<a password>
EOF
java -jar server.jar nogui        # jar from work/cache/versions/26.3/26.3.json, downloads.server

./gradlew :tools:wsbridge:run --args="--listen 8817 --target 127.0.0.1:25582"   # another terminal

INCARI_RCON_PASSWORD=<the password> ./gradlew :app-web:browserCheck \
    "-Pbrowser.query=server=ws://127.0.0.1:8817&name=IncariWeb" \
    -Pbrowser.rcon=127.0.0.1:25592 -Pbrowser.seconds=300 \
    -Pbrowser.steps=@browser-checks/server-join.steps
```

- `rcon:<command>` runs a command; `rconwait:<command> => <regex>` repeats until the reply matches.
- RCON runs as the console, so `~` is the world spawn, not the player. Turn a player with
  `execute as IncariWeb at @s run tp @s ~ ~ ~ ~45 ~`.
- `soak.steps` also wants `-Pbrowser.gpu=gl -Pbrowser.seconds=2100`.

**Old servers** (`version-join`): vanilla 1.8.9, 1.12.2 or 1.20.4 (1.8.9 and 1.12.2 need Java 8;
use `difficulty=0`), each behind its own bridge. The suggested ports: 1.8.9 on 25601 (RCON 25611,
bridge 8801), 1.12.2 on 25602/25612/8802, 1.20.4 on 25603/25613/8803. Add `&via=auto` to
`browser.query`: Via is off unless the join asks for it (ADR 0071).

**EaglerXServer** (`version-join`, voice): BungeeCord with the EaglerXServer plugin (downloaded,
never committed) in front of the 1.8.9 server: `online_mode: false`, lobby `127.0.0.1:25601`,
`inject_address` equal to Bungee's listener host. Join `ws://127.0.0.1:25577`. For voice, set
`voice_service.enable_voice_service: true`, then run both scripts at once (Chromium gets a fake
microphone):

```sh
INCARI_RCON_PASSWORD=<pw> ./gradlew :app-web:browserCheck "-Pbrowser.query=server=ws://127.0.0.1:25577&via=auto&name=VoiceA" \
    -Pbrowser.rcon=127.0.0.1:25611 -Pbrowser.seconds=200 -Pbrowser.steps=@browser-checks/voice-talker.steps &
sleep 8
INCARI_RCON_PASSWORD=<pw> ./gradlew :app-web:browserCheck "-Pbrowser.query=server=ws://127.0.0.1:25577&via=auto&name=VoiceB" \
    -Pbrowser.rcon=127.0.0.1:25611 -Pbrowser.seconds=200 -Pbrowser.steps=@browser-checks/voice-listener.steps
```

The desktop checks voice signalling alone: `smokeVoiceA` and `smokeVoiceB` together.

**Relay** (`relay-join`, `share-*`): EaglerSPRelay on port 6699 (its jar ships in the EaglercraftX
workspace's `relay_download.zip`; keep it in `run/`).

```sh
# relay-join: relay-host.html is a stand-in host that prints a join code
java -jar run/relay/EaglerSPRelay.jar &
chrome --headless=new --enable-logging=stderr --user-data-dir=$(mktemp -d) \
    "file://$PWD/app-web/browser-checks/relay-host.html?relay=ws://127.0.0.1:6699/" 2> host.log &
sleep 5; CODE=$(grep -ao 'incari-relay-code: [a-z0-9]*' host.log | tail -1 | cut -d' ' -f2)
sed "s/CODE/$CODE/" app-web/browser-checks/relay-join.steps > build/relay.steps
./gradlew :app-web:browserCheck -Pbrowser.seconds=150 -Pbrowser.steps=@build/relay.steps

# share-host + share-guest: the guest runs BrowserCheck directly (Gradle runs one build at a time;
# the class path is printed by the first run with --info)
./gradlew :app-web:browserCheck -Pbrowser.seconds=300 \
    "-Pbrowser.query=name=Host&world=Shared&seed=2&share=ws://127.0.0.1:6699/" \
    -Pbrowser.steps=@browser-checks/share-host.steps | tee host.log &
java -cp <class path> incari.tools.webbuild.BrowserCheck app-web/build/teavm <same --file/--dir args> \
    --query "name=Guest&shared=ws://127.0.0.1:6699/|<code from host.log>" --seconds 240 \
    --steps-file app-web/browser-checks/share-guest.steps
```

**Community pack:** download one into the ignored `build/`, never the repo. Tested with Faithful 32x:

```sh
mkdir -p app-web/build/community-packs
curl -L -o app-web/build/community-packs/pack.zip \
    "https://cdn.modrinth.com/data/w0TnApzs/versions/lDYpMiqk/Faithful%2032x%20-%2026.3.zip"
```

**Audio notes:** Chromium runs muted with `--autoplay-policy=document-user-activation-required`, so
audio starts locked as for a player; the scripts measure the page's master bus. DevTools
evaluations count as a click, so steps before the first click use `holds:` and `until:`. Desktop
reference numbers come from `./gradlew :platform-desktop:openAlReference`.

`window.incariStorage.read(path)` in the page reads a stored file back (handy in the console too).

## By hand, in a real browser

Headless Chromium can't do everything. With `./gradlew :app-web:serve` and a world, check in Chrome
(and Firefox where noted):

- [ ] **Pointer Lock:** clicking into a world hides the cursor; turning is smooth, with no acceleration on Windows.
- [ ] **Escape:** opens the pause menu; Back to Game locks again (perhaps after one more click).
- [ ] **Focus:** Alt+Tab or a tab switch pauses the game; no key stays held on return.
- [ ] **Fullscreen:** F11 and the video setting both toggle it. Inside, Escape pauses, and Ctrl+W and Ctrl+1–9 don't reach the browser.
- [ ] **Ctrl+W while playing:** asks "Leave site?". Nothing asks on the title screen.
- [ ] **F3 and chat:** F3+B/G/H/T work without browser side effects; chat typing, editing, history and Tab completion work.
- [ ] **Clipboard:** paste into chat from another program; copy out of a game text field; F3+C copies the location.
- [ ] **Key names:** on AZERTY, Key Binds shows Z/Q/S/D; names follow a layout switch (Chrome) or are learned as you type (Firefox).
- [ ] **Input methods and dead keys:** Japanese/Chinese composition shows in the field; ´ then e types é.
- [ ] **Resize and density:** window resize, browser zoom and a move to another display redraw sharply; clicks land right.
- [ ] **Dropped files:** a pack .zip or folder dropped on Resource Packs is offered and survives a reload.
- [ ] **Sound by ear:** the unlock note, then music; positional sounds turn with you and fade with distance; each volume slider only moves its category; jukeboxes pause and loop; no gaps between tracks. Also in Firefox.
- [ ] **Chromebook tablet mode:** tapping a text field opens the on-screen keyboard (note if it takes a second tap).
- [ ] **Storage after closing the browser:** an option, a server and a pack are all still there. Over plain http from another machine the console says `indexeddb` and it still works.
- [ ] **A second tab:** warns that the other tab keeps the files.
- [ ] **Downloads:** F2, then the chat link saves the PNG (Chrome, Safari, Firefox).
- [ ] **Closing right after a change:** the change is kept (the same-frame window is accepted, ADR 0011).
