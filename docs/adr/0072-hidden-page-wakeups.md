# ADR 0072: A hidden page keeps time in a worker

- Status: accepted
- Date: 2026-11-04

## Context

The page runs the game on timers: TeaVM wakes every thread with `setTimeout`, and a hidden tab's
frames come from a quarter-second `setTimeout` (no animation frames when hidden). Chrome holds a
hidden page's chained timers to one wake-up a second, and after five minutes hidden and silent to
one a minute. Packets reach the game once a frame and Netty's threads wake on timers, so the
server's keep-alive went unanswered and players came back to "Timed out".

## Decision

- While the page is hidden, every TeaVM wakeup and both quarter-second fallbacks (frames,
  background pool) go to a small Blob worker's `setTimeout`, which answers with a message
  (`wakeups.ts`: `createPageTimer`, `createPageAsyncImports`; `window.incariTimer`).
- While visible, nothing changes: TeaVM's own `setTimeout`, and animation frames.
- Not the `MessageChannel` wakeups of ADR 0040: on the page they once hung singleplayer at spawn
  (ADR 0043), and a spinning wait would busy a hidden tab's core.
- `?wakeups=timers` turns it off.

## Consequences

- A hidden tab runs the game at about four frames a second instead of
  one a minute: more battery used in the background.
- Not measured in a real browser yet: whether Chrome exempts a dedicated worker's timers from its
  intensive throttling is from its documentation, not a test. `browserCheck` can't hide its page
  for five minutes.
