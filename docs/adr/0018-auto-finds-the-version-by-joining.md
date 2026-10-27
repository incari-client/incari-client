# ADR 0018: "Auto" finds a server's version by joining

- Status: accepted
- Date: 2026-09-29

## Context

ADR 0017's "Auto" took any Eagler server that doesn't name its version as 1.8. Nothing an Eagler
server says before login names it (the MOTD has none; the handshake accepts anything), so every
non-1.8 Eagler server was joined as 1.8 and refused. BungeeCord over TCP has the same problem.

What servers do say, reliably, is why they refuse, in words: "Outdated server! I'm still on 1.8.8",
"Incompatible client! Please use 1.20.4", BungeeCord's "Kicked whilst connecting to lobby: …".

## Decision

- **"Auto" joins as**, in order: the player's choice; the version that last reached the world at
  this address (`incari/learned-versions.txt`); the version the server reports; else the game's own.
- **Refusals are read** (`VersionRetry`, one hook in `DisconnectedScreen.init`): release numbers in
  the text and its translation arguments map to protocols through Via. The newest version below the
  refused one is tried next (older only if nothing older is named), never one already tried. Only
  within a minute of connecting, only for "Auto", at most four joins.
- **A silent drop is a hint:** EaglerXServer on BungeeCord closes without a word when the proxy
  can't carry the protocol. A close during the Eagler login (`droppedDuringLogin`) makes "Auto" try
  1.8 next, then 1.12.2; a newer backend refuses 1.8 by name, which the next retry reads. Not
  when the dropped version already reached the world at this address: a rejoin right after a kick
  is often dropped while the old session lingers, and a multi-version server would accept the 1.8
  guess and teach it for good (Carrotcraft). Only a refusal in words corrects a learned version.
- **Only a join that reaches the world is remembered.** A stale one is refused and corrected,
  except on a server that accepts every version: a wrong one sticks there, so the drop rule above
  guards it. `server-versions.txt`, which builds up to 0.1.2 could fill that way, is deleted once.

## Alternatives rejected

- **Keeping 1.8 as the Eagler default:** wrong for every non-1.8 server.
- **Probing versions in parallel:** each probe is a full login, visible to the server, and looks like an attack.
- **Trying every release newest-first:** up to fifty logins, where a refusal names the version in one.

## Consequences

- **Verified** against EaglerXServer 1.1.1 on BungeeCord: with a 1.8.9 backend, 26.3 is dropped,
  "Auto" tries 1.8, joins, and remembers it. With a 1.20.4 backend, 26.3 dropped, 1.8 refused naming
  1.20.4, then 1.20.4 tried.
- That last join fails outside the client: EaglerXServer 1.1.1 on BungeeCord carries protocols only
  up to 1.18.2 here. Modern Eagler networks need Velocity.
- The first join to such a 1.8 server costs one dropped login (~10 s); later joins use the learned version.
- A kick message that mentions a version and a refusal word triggers a retry; the minute window
  and four-join cap stop loops.
