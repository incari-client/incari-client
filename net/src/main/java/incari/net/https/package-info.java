/**
 * A small HTTPS/1.1 client over {@link incari.net.tls.TlsConnection}, for sign-in's handful of
 * JSON requests (ADR 0070): one request per connection, an allow-list of hosts, capped sizes and
 * a time limit. No game types, no blocking.
 */
@NullMarked
package incari.net.https;

import org.jspecify.annotations.NullMarked;
