/**
 * TLS for Microsoft sign-in (ADR 0070): Bouncy Castle's non-blocking TLS client over any
 * connection ({@link incari.net.tls.Transport}: a socket, or a Wisp stream in the browser), with
 * the server's chain checked against a few pinned roots. No game types, no blocking.
 */
@NullMarked
package incari.net.tls;

import org.jspecify.annotations.NullMarked;
