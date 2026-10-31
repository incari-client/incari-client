# ADR 0065: Wispcraft: Java servers through a Wisp proxy

- Status: accepted
- Date: 2026-10-25
- Builds on: ADR 0009 (the transport), ADR 0017 (a version per server)

## Context

A page has only WebSockets, so the browser build joins only Eagler servers (ADR 0016).
[Wispcraft](https://github.com/MercuryWorkshop/wispcraft) lets Eaglercraft join plain Java servers
by tunnelling TCP over [Wisp](https://github.com/MercuryWorkshop/wisp-protocol), a protocol for TCP
streams over one WebSocket, through a public proxy. The maintainer asked for the same, switched per
server on the add and edit server screens, like the version.

## Decision

- **Transport, not a proxy in the page.** `incari.net.wisp.WispConnection` is a `NetConnection`: a
  WebSocket to the proxy, one Wisp v1 stream to the server, its bytes passed up as TCP's. The game's
  own TCP pipeline (framing, encryption, compression) and Via run on top unchanged. Wispcraft
  re-implements the protocol in JavaScript; Incari already has the game's.
- **v1 only.** No `Sec-WebSocket-Protocol` header, so v2 proxies speak v1. Flow control is honoured
  (CONTINUE); what's sent between flushes goes as one DATA packet.
- **SRV through the proxy.** A host with no port asks `1.1.1.1:53` over DNS-over-TCP on a second
  stream, as the game does over JNDI. No answer: the address as written.
- **Per server, saved with it.** `ServerData.incariWisp` (on/off and the proxy, default Wispcraft's
  `wss://anura.pro/`). The edit-server screen's version button is halved and a Wispcraft button sits
  beside it (a screen edit, no patch); it opens a screen to switch it and set the proxy.
- **The last proxy is remembered.** Done saves it to `incari/wisp-proxy.txt` at once; a server
  not yet set up (off, default proxy) starts from it, so it's typed once.
- **Routing by thread.** The connect screen and the pinger name the server before resolving
  (`WispServers.route`); the resolver hook then hands back a `WispAddress` with no DNS lookup.
  `ServerConnections` gives it the plain pipeline. `ws://` addresses never go through Wisp.

## Limits

- **Offline-mode servers only**, like any join from Incari: there's no Microsoft sign-in, which
  Wispcraft adds for online-mode servers.
- Everything the player sends passes through the proxy; the screen says so.
- A server's transfer packet to another host keeps the server's choice only when the game reuses its
  list entry.
