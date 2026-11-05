# ADR 0070: Microsoft sign-in for online-mode servers

- Status: proposed
- Date: 2026-11-04 (security review the same day: key-size limits, server-auth usage, file mode)
- Builds on: ADR 0065 (Wispcraft), ADR 0006 (the link step), ADR 0009 (the transport)

## Context

Through Wispcraft (ADR 0065) Incari joins plain Java servers, but only offline-mode ones: an
online-mode server sends an encryption request, and the client has to tell Mojang's session server
it's joining as a real account. Players asked for it; Wispcraft and other Eagler clients have it.

A page can't call Microsoft's, Xbox's or Mojang's APIs directly: they send no CORS headers.
Wispcraft gets past that by running TLS in the page over a Wisp stream (epoxy-tls, AGPL-3.0, wasm).

## Decision

- **The launcher's client id.** Sign-in asks `login.live.com` for tokens as the official Minecraft
  launcher (`00000000402b5328`, scope `service::user.auth.xboxlive.com::MBI_SSL`), as other Eagler
  clients do. An app of Incari's own would need Mojang's approval for `login_with_xbox`. It's one
  constant, `MicrosoftEndpoints.CLIENT_ID`.
- **Device code flow.** The game shows a code; the player types it at `microsoft.com/link` on any
  device. The password never reaches Incari.
- **TLS in Java, over any connection** (`incari.net.tls`). Bouncy Castle's lightweight TLS client
  (MIT) in its non-blocking mode, over a `NetConnection`: a socket on the desktop, a Wisp stream in
  the browser, through the player's last proxy (or Wispcraft's default). End to end: the proxy sees
  only ciphertext. Chosen over epoxy-tls for its licence, and because it runs and is tested on the
  JVM like the rest of `net`.
- **Pinned roots, strict checks** (`CertificateChain`). Only the four roots Microsoft's hosts chain to
  (DigiCert Global Root G2 and G3, Microsoft RSA and ECC Root 2017). The leaf must name the host
  (subject alternative names, one-label wildcards) and server auth; a CA that names purposes must
  include it. Every link must be in date, a CA's, and signed with RSA 2048 to 4096 bits (exponent
  ≤ 32 bits) or ECDSA P-256/P-384 over SHA-2. Sizes are checked from the raw numbers before any key
  is read, so a hostile proxy can't stall the client with huge keys. TLS 1.2 and 1.3 only.
- **A small HTTPS client** (`incari.net.https`): one request per connection, `Connection: close`,
  capped sizes, 30 s limit, and an allow-list of the five hosts sign-in uses. Nothing else can be
  reached through it, so a token can't be sent elsewhere.
- **The token chain** (`incari.net.msa`): Microsoft → Xbox Live (`RpsTicket "t="`, retried once as
  `"d="`) → XSTS → `login_with_xbox` → profile. Microsoft's refresh token is kept, so the account
  lasts past the Minecraft token's 24 hours. A token about to expire is renewed before a join.
- **The game's own handshake.** On the web, `web-classlib` now supplies the JCA classes the
  handshake and chat signing call (`Cipher` for RSA/PKCS#1 and AES/CFB8, `KeyFactory`,
  `Signature` for SHA256withRSA, `KeyGenerator`), over Bouncy Castle's engines, tested against the
  JDK. They replace removals the survey made only because they were missing. `KeyFactory` refuses
  RSA keys over 4096 bits or with exponents over 64 bits, as a server's key arrives unchecked. `KeyPairGenerator`
  stays missing, so the worker's integrated server still makes no key pair.
- **Two hooks.** `authenticateServer` joins through `ServerJoins` (and, offline, says how to sign in
  instead of "invalid session"); `getProfileKeyPairManager` gives the account's chat key, fetched
  from `/player/certificates`. A launcher's own account (desktop, real token) keeps the game's path.
- **Stored on the device.** `incari/microsoft-account.json` in the game directory (OPFS in a page)
  holds the name, id, Minecraft token and refresh token. Never logged; Sign Out deletes it. On the
  desktop it's owner-only (0600) and replaced atomically.
- **Screen**: Edit Profile's "Microsoft Sign-In" link opens `MicrosoftScreen`. Signed in, the name
  is the account's and can't be edited. Accounts can't change while in a world.

## Consequences

- Online-mode Java servers work through Wisp and on the desktop, with secure chat.
- Bouncy Castle is in both builds (credits and `NOTICE.md`). Only the classes reached are linked.
- If a host moves to a root that isn't pinned, sign-in fails ("not issued by a trusted root") until
  `TrustAnchors` is updated. No revocation checks, as browsers soft-fail them.
- Using the launcher's id borrows its identity: Microsoft's page names it, and Microsoft could block
  it for everyone at once.
- The refresh token on disk is as safe as the device's storage for the page. Anyone with the
  player's browser profile can use it, and so can any page on the same origin: host Incari on an
  origin of its own (not a shared `user.github.io`); `docs/self-hosting.md` says so.
- On the desktop, a signed-in token reaches vanilla code too (Realms, over the JDK's trust store),
  as a launcher's would.
- CFB8 runs one AES block per byte: the cost of every byte on an online-mode server. Measure it on
  a Chromebook before calling it free.
- Not in the Rust build: Horizon has no Wisp and no online mode (ADR 0068).
- Networking and licensing reviewers (`.github/CODEOWNERS`) review it.
