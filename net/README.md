# net: the transport under the game's pipeline

How the game's Netty pipeline reaches a server (ADR 0009): TCP on the desktop, or WebSocket, the
only way out of a browser. The game's own protocol code runs unchanged on top. Every WebSocket
server is treated as an Eagler server, so Eagler's client side lives here too (ADR 0016, 0019).
Version translation is `:via`'s job.

## Code map

| Where | What |
| --- | --- |
| `NetConnectionChannel` | A Netty channel over a platform connection |
| `WebSocketUrl`, `WebSocketAddress` | `ws://`/`wss://` addresses, parsed as a browser reads them |
| `MessageFraming` | One packet per WebSocket message, replacing length-prefix framing under the same handler names |
| `eagler.EaglerHandshake` | The Eagler stage: protocol V5, then answers the game's login in the server's place, then passes packets |
| `eagler.EaglerLogin`, `EaglerMessages`, `EaglerTextures` | Login contents; Eagler's own messages; skin and cape formats |
| `eagler.EaglerQuery` | The MOTD query the server list uses |
| `eagler.EaglerPassword` | Auth plugins' password methods |
| `relay.*` | Joining a world shared through an Eagler relay, over a WebRTC data channel |
| `wisp.WispConnection`, `WispAddress` | A Java server's TCP port through a Wisp proxy (Wispcraft, ADR 0065), with its SRV lookup |
| `tls.TlsConnection`, `Transport` | TLS 1.2/1.3 (Bouncy Castle, non-blocking) over a socket or a Wisp stream, for sign-in (ADR 0070) |
| `tls.CertificateChain`, `TrustAnchors` | The server's chain against four pinned roots: host name, dates, CA rules, signatures |
| `https.HttpsClient`, `HttpRequest`, `ResponseParser` | One HTTPS/1.1 request per connection, to an allow-list of hosts, sizes capped |
| `msa.MicrosoftSignIn` | Microsoft sign-in as the launcher: device code, Xbox Live, XSTS, Minecraft token and profile; session joins; chat keys. A `SignInException` is `temporary()` when the request didn't get through or the service answered 5xx/429: the screen keeps polling, and a failed refresh keeps the account |
| `msa.MicrosoftAccount`, `MicrosoftEndpoints` | The signed-in account (tokens never printed); the client id, URLs and allowed hosts |

## Invariants

- **No game types**, only `platform-api` and Netty (`BoundaryTest`). `client`'s `incari.client.net`
  is the game-facing half.
- **The game's handler names survive**, so everything added relative to "splitter" and
  "prepender" works on both transports.
- **Bytes reach the pipeline only on its event loop.**
- **The Eagler stage speaks the server's version.** It sits below framing and Via.
- **Nothing of EaglerXServer's code** (it's All Rights Reserved). The client side is written from
  the wire protocol.
- **Sign-in never logs a token** and reaches only `MicrosoftEndpoints.HOSTS`, over pinned TLS.

## How to test

```sh
./gradlew :net:check
INCARI_LIVE=1 ./gradlew :net:test --tests '*LiveEndpointsTest'   # the real hosts' TLS; signs nobody in
```

`TlsConnectionTest` runs real handshakes against the JDK's TLS server on localhost. A whole
sign-in needs a Microsoft account that owns the game: Edit Profile → Microsoft Sign-In, then an
online-mode server (`online-mode=true`) directly or through `wisp-js-server` (below).

End to end against a local vanilla server behind `tools/wsbridge` (a stand-in Eagler proxy):

```sh
java -jar server.jar nogui        # server-port=25581, online-mode=false, player opped
./gradlew :tools:wsbridge:run --args="--listen 8816 --target 127.0.0.1:25581"
./gradlew :app-desktop:smokeServer -Pincari.server=ws://127.0.0.1:8816
```

The report (`run/smoke-server/incari-smoke/report.txt`) covers ping, join, chat, the Nether and
back, and reconnecting. Against a real EaglerXServer, use its address; `smokeSkinA` with
`smokeSkinB` checks skins both ways (setup in ADR 0016). In the browser: `server-join.steps`.

The same server through a Wisp proxy (ADR 0065), with `@mercuryworkshop/wisp-js` from npm:

```sh
wisp-js-server --port 6001 -O '{"allow_private_ips":true,"allow_loopback_ips":true}'
./gradlew :app-desktop:smokeServer -Pincari.server=127.0.0.1:25581 -Pincari.wisp=ws://127.0.0.1:6001/
```
