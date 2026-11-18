# The online service's protocol

What the client sends to Incari's online service and expects back. The service is closed source;
it implements this file, so a change here is a change to both. Client code: `:online`'s
`OnlineService`. `FakeService` (test fixtures, also served by `tools/onlinedev`) answers as the
service should. Decisions: accounts ADR 0058, friends ADR 0059, releases ADR 0060, cosmetics
ADR 0061, counts/featured/badge ADR 0062.

Protocol version: **1**.

## Framing

- One WebSocket, `wss://<host>/v1/ws`. JSON text frames, one message each, at most 16 KiB.
- Request: `{"t": "<type>", "id": <n>, ...}`, `id` counting from 1 per connection.
- Answer: `{"t": "ok", "id": <n>, ...}` or `{"t": "err", "id": <n>, "code": "<code>"}`.
- Event: a message without `id`. The client ignores unknown event types.
- Timeouts: an answer after 20 s counts as failed. No answer to `hello` within 15 s, or to
  `signup`/`signin`/`recover`/`resume` in time, and the client reconnects (the service may have
  signed in a connection the client doesn't know about; a fresh one starts both signed out).

### Errors

| Code | Means | Client shows |
| --- | --- | --- |
| `unauthorized` | Wrong credentials, code or password; unknown token | The form's wrong-credentials message |
| `no_session` | Needs sign-in, connection isn't | "You were signed out", and signs out |
| `taken` | Username taken | "That username is taken" |
| `bad_request`, `weak_password` | Broke a rule (length, common password, stale challenge) | "The service turned that down" |
| `closed` | Recipient doesn't take messages from the sender (setting, block, or `pid` gone). Indistinguishable on purpose | "They aren't taking messages from you" |
| `rate_limited` | Too many tries. **Never sent for sign-in**, which answers `unauthorized` after the usual delay | "Too many tries" |
| `not_owned` | `equip`/`emote` named an unowned cosmetic | Nothing; re-fetches the wardrobe |
| `update` | This protocol version is retired; the service then closes | "Update the client"; stops reconnecting |
| `unavailable`, other | Service trouble, or (`signin`, `resume`) too many connections for the account | "The service is having trouble" |

## Before signing in

| `t` | Sent | `ok` answer |
| --- | --- | --- |
| `hello` | `v` (1), `client` (`incari/<game version>`) | `v`; optionally `limits`, `release`, `featured` |
| `count` | `install`, `version`, `kind` | — |
| `challenge` | — | `challenge` (16 bytes, base64url, no padding), `bits` (0–40) |
| `signup` | `username`, `password`, `challenge`, `nonce` | `token`, `recoveryCodes` (10), `account` |
| `signin` | `username`, `password` | `token`, `account` |
| `resume` | `token` | `account` |
| `recover` | `username`, `code`, `newPassword` | `token`, `account` (all other sessions end) |

- **`release`**: `{"version": "<latest release>"}`, 1–4 dot-separated numbers of up to 6 digits.
  A release build shows a title-screen notice, linking to the built-in repository URL, when it's
  newer than its own (missing parts count as 0). Malformed or absent: no notice. Sent signed in or not.
- **`featured`**: up to 4 `{"name": <1–32 chars>, "address": <1–128 chars>}` pinned atop the server
  list as Sponsored. Present (even `[]`) it replaces the built-in list for this run; absent, the
  built-in list stays. Out-of-bounds entries are dropped. Sent signed in or not.
- **`count`**: an install ran today. `install` is 16 random bytes (base64url, 22 chars), made once,
  never tied to an account; `version` as `release`; `kind` is `fast`, `compatible`, `html` or
  `desktop`. At most once a UTC day, after `hello`, never with Usage Counts off. Any answer but
  `rate_limited` uses the day (errors too); `rate_limited` retries after 30 minutes, no answer after
  a minute. The service keeps no IP or account: a keyed hash of `install` per day (35 days) and
  daily totals by `version` and `kind`. Repeats get `ok`; malformed is `bad_request`.
- **`account`**: `{"username", "settings": {"shareServer": <bool>, "messages": "everyone" |
  "friends" | "nobody", "friendRequests": <bool>, "badge": <bool>}}`. Older services may omit
  `messages`/`friendRequests` (client assumes `friends`/`true`) or `badge` (assumes `true`).
- **Resume:** sent right after `hello` if a token is saved. `unauthorized` or `no_session` forgets it.
- **Proof of work:** SHA-256 of the 16 challenge bytes then the nonce as 8 bytes big-endian must
  start with `bits` zero bits. Nonce is 0 to 2^53−1, a JSON number, tried from 0 up. A challenge
  lasts 2 minutes and one successful sign-up (a wrong nonce doesn't use it).
- **Recovery codes:** `XXXX-XXXX-XXXX`, Crockford base32 upper case. The client normalizes input
  (case, spaces, dashes; I and L → 1, O → 0).
- **Usernames:** 3–16 of `[A-Za-z0-9_]`, unique ignoring case. **Passwords:** 10–128 characters,
  not common, not containing the username.

## Signed in

| `t` | Sent | `ok` answer |
| --- | --- | --- |
| `presence` | `serverHash`, `name`, `server` (only while shared); or `serverHash: null` to clear | — |
| `lookup` | `names` (1–256 Minecraft names) | `players`: `[{"name", "pid", "friend", "cosmetics", "badge"}]` for Incari players on the asker's server |
| `settings` | any of `shareServer`, `messages`, `friendRequests`, `badge` | — |
| `password` | `old`, `new` | — (all other sessions end) |
| `codes` | `password` | `recoveryCodes` (10 new; old ones stop working) |
| `signout` | `everywhere` (bool) | — |
| `delete` | `password` | — |
| `block` | `pid` and `name` (as seen), or `username` | — |
| `blocks` | — | `blocks`: `[{"bid", "label"}]` |
| `unblock` | `bid` | — |
| `report` | `pid` or `username`, `reason`, `note` (≤500 chars) | — |

- **Server hash:** SHA-256 (64 lowercase hex) of `incari-server-v1\n` + the normalized address:
  - trimmed;
  - plain address: host lower case, no trailing dot, port only if not 25565 (`[v6]:port` for IPv6);
  - `ws://`/`wss://`: scheme and host lower case, port only if not 80/443, path as written without
    a trailing slash, query and fragment dropped;
  - `relay:<code>` for a relay-shared world, code as typed.

  Test vectors (made with Python's hashlib): `:online`'s `vectors.json`.
- **Presence:** singleplayer and the player's own LAN world are never reported. A `presence`
  replaces the last; the service clears it when the connection drops. The client sends one only
  when it changes what the service holds, one at a time. On leaving a world it waits 10 s before
  clearing (a proxy server switch rejoins the same address, which isn't a change; the service limits
  changes to about 30 an hour). Retries after `rate_limited`, `unavailable` or no answer: 1 minute,
  doubling to 15. After `bad_request`: not until the place changes. Resent after a reconnect.
- **Lookups:** the whole tab list on join, then names as they're added, at most one lookup per
  6.5 s; all again after a reconnect. The service sends `seen`/`gone` only for names this connection
  looked up, so reporting a server's hash without being there reveals nothing. It answers only for
  names an account has reported on the asker's `serverHash` right now; nothing for a name two
  accounts claim; nothing across a block either way.
- **`pid`:** the account's id on that server only, so it never follows an account between servers.
- **Block `label`:** the name the blocker saw (by `pid`) or typed (by `username`). Never reveals a
  blocked account's username. `bid` is opaque.
- **Report reasons:** `harassment`, `inappropriate_name`, `spam`, `impersonation`, `other`.
- **`badge`** ("Show others I use Incari", default `true`): carried by `lookup` and `seen`; the client
  draws the Incari icon only when `true`. Changes go out as a `badge` event like a wardrobe change.
  It hides the icon, not the account: lookups still answer.

## Friends and messages

The client sends `friends` after every sign-in. `bad_request` means the service has no friends
support; the client says so and stops asking.

| `t` | Sent | `ok` answer |
| --- | --- | --- |
| `friends` | — | `friends`: `[F + "unread"]`, `requests`: `[R]` |
| `befriend` | `pid` and `name`, or `username` | `request`: R, or `friend`: F (they'd already asked) |
| `accept` | `rid` (received) | `friend`: F |
| `decline` | `rid` (declines received, withdraws sent) | — |
| `unfriend` | `fid` | — |
| `send` | `fid` or `pid`, `text` (1–500 chars) | `mid`, `at` |
| `history` | `fid`, optional `before` (a `mid`) | `messages`: `[M]` (≤50, before `before` or newest), `more` |
| `read` | `fid`, optional `mid` (newest read) | — |

- **F:** `{"fid", "username", "online", "server"}`; `server` only while online and shared.
  `unread` (in `friends` only) counts unread messages.
- **R:** `{"rid", "incoming", "label"}`. Received: the sender's username. Sent: the `name` or
  `username` it went to. A request by `pid` never reveals the receiver's username.
- **M:** `{"mid", "from": "me" | "them", "text", "at"}`, `at` in ms since 1970 UTC.
- `fid`, `rid`, `mid` are opaque strings.
- **`befriend`:** unknown username or yourself → `bad_request`. To someone with `friendRequests`
  off, or across a block: `ok` with R, never delivered. Repeats return the first R. Crossing
  requests make a friendship.
- **`send`:** to a friend needs the friendship and their `messages` not `nobody`. By `pid` needs the
  recipient on the sender's server now with `messages: everyone`. Otherwise, or across a block:
  `closed`. Text is trimmed; empty is `bad_request`.
- **Blocking a friend** ends the friendship (`unfriended` both sides; pending requests →
  `requestgone`).
- **History** is kept for friends only (30 days). Messages by `pid` are live only, kept just for reports.
- **After a reconnect** the client takes the unread counts as given and re-fetches the newest
  `history` page for each held conversation when next shown. It sends `read` only after that page,
  so it never marks unseen messages read.

## Cosmetics

The client sends `wardrobe` after every sign-in and resume. `bad_request` means no cosmetics
support; worn cosmetics then stay on the device.

| `t` | Sent | `ok` answer |
| --- | --- | --- |
| `wardrobe` | — | `owned`: `[id]`, `equipped`: `[id]` |
| `equip` | `cosmetics`: `[id]` (everything worn now; `[]` for none) | `equipped`: `[id]` |
| `emote` | `emote`: an id | — |

- **Ids:** `<slot>.<name>`, lowercase letters, digits, `_`, one dot. The catalog is `:online`'s
  `Catalog`; the service keeps the same list and which are free.
- **`owned`** includes free ones. **`equipped`**: at most one per worn slot (`cape`, `hat`, `back`,
  `nametag`, `trail`). Emotes are owned, never equipped.
- **`equip`** replaces what's worn. Unknown id, an emote, two in a slot or more than five →
  `bad_request`; unowned → `not_owned`; nothing changes. On `ok`, a `cosmetics` event goes to every
  connection that looked this account up on a server it's on (as `seen`, blocks respected), and a
  `wardrobe` event to the account's other connections. Grants and revokes fan out the same way.
  The client sends at most one a second after changes stop; the service may limit (~30/minute).
- **`emote`:** an `emote` event to every connection that looked the player up on their current
  server (blocks excepted); `ok` even if nobody hears. Client: at most one per 2 s; service may limit
  (~20/minute). Never stored.
- `lookup`, `seen` and `cosmetics` carry the wearer's `equipped`. Clients draw only ids they know,
  so the service can add cosmetics first.
- **No uploads or one-of-a-kind cosmetics:** a unique cosmetic would follow an account between
  servers, past what `pid` hides.

| Slot | Ids (all free for now) |
| --- | --- |
| `cape` | `cape.frost`, `cape.aurora`, `cape.midnight`, `cape.ember`, `cape.sakura`, `cape.tide`, `cape.mono` |
| `hat` | `hat.crown`, `hat.halo`, `hat.top_hat`, `hat.beanie`, `hat.horns`, `hat.cat_ears` |
| `back` | `back.angel_wings`, `back.frost_wings`, `back.bat_wings`, `back.backpack` |
| `nametag` | `nametag.ice`, `nametag.gold`, `nametag.mint`, `nametag.rose`, `nametag.violet`, `nametag.frost`, `nametag.sunset`, `nametag.aurora` |
| `trail` | `trail.snow`, `trail.hearts`, `trail.sparkles`, `trail.notes` |
| `emote` | `emote.wave`, `emote.clap`, `emote.cheer`, `emote.salute`, `emote.facepalm`, `emote.shrug`, `emote.dab`, `emote.bow` |

## Events

| `t` | Fields | When |
| --- | --- | --- |
| `seen` | `name`, `pid`, `friend`, `cosmetics`, `badge` | A looked-up name arrived as an Incari player on the asker's server (no block either way) |
| `gone` | `pid` | They left, signed out, or a block came between (looked-up names only) |
| `cosmetics` | `pid`, `cosmetics` | A looked-up player on the asker's server changed what they wear |
| `wardrobe` | `owned`, `equipped` | The account's wardrobe changed elsewhere, or it was given a cosmetic |
| `badge` | `pid`, `badge` | A looked-up player on the asker's server changed `badge` |
| `emote` | `pid`, `emote` | A looked-up player on the asker's server played an emote |
| `request` | R | A friend request arrived (or one the asker sent from elsewhere, `incoming: false`) |
| `requestgone` | `rid` | A request was accepted, declined or withdrawn |
| `friend` | `friend`: F | A new friendship, or a friend went on/offline or changed server |
| `unfriended` | `fid` | Unfriended or blocked, either side |
| `message` | `mid`, `at`, `text`, and `fid` or `pid` + `name` | A message for the asker |
| `signedout` | — | The session ended elsewhere (sign out everywhere, new password, recovery, deletion, ban). The client forgets its token |

## The client never

- Sends or keeps a password beyond its form and the one message that carries it.
- Logs passwords, tokens or recovery codes.
- Waits on the service: nothing in the game blocks on an answer, and without a service the client
  behaves as if accounts didn't exist.


## Client state

A signed-in client MAY add a `clientState` string to the `settings` request. The service keeps it
against the account and hands it back inside `settings` on `signup`, `signin`, `resume` and
`recover`. It holds the client's settings bundle (the `incari-settings.json` content: client
switches, client keybinds, module profiles) minified, and the whole message must stay within the
message-size cap; a bundle that no longer fits is not pushed rather than truncated.

A service that does not keep client state answers `bad_request`; the client then stops pushing for
the session and everything else keeps working.
