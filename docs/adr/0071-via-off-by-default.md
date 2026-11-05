# ADR 0071: Via is off unless the player turns it on

- Status: accepted
- Date: 2026-11-04
- Amends: ADR 0017 (Via in the client), ADR 0018 ("Auto" finds the version by joining)

## Context

"Auto" (ADR 0018) joined every server through version detection: pings, quick probes, retries on a
refusal, guesses after a silent Eagler drop, learned versions. It caused more trouble than it fixed:
wrong guesses, extra logins, and wrong versions remembered for multi-version servers.

## Decision

- **Each server's Via choice is Off, Auto or a release; Off is the default**, like Wispcraft. The
  edit-server screen's button reads "Via: Off" and opens the list, with Off first.
- **Off joins as the game's own version**: no ping, probe, retry or learned version. The server list
  marks other versions incompatible, as the game does.
- **Saved as `incariVia`**, not `incariVersion`. Older builds saved "Auto" as their default, so their
  choices are left unread and every existing server starts with Via off.
- **A hint on refusal**: with Via off, a join refused for its version ("Outdated server!",
  "Incompatible client!") or dropped during an Eagler login shows the server's reason with a line
  below it: edit the server and set Via to Auto (`ViaHint`, one hook in `DisconnectedScreen`).
- **Auto for the smoke tests' servers**, and for a quick play join whose page asks with `?via=auto`
  (the browser checks of other versions). A server an Eagler server redirects to gets Auto if Via
  was on for the one that sent the player. Featured servers keep the default: they run the game's version.

## Consequences

- Players of 1.8 servers turn Via on once per server.
- The Rust build has no Via, so it has nothing to change.
