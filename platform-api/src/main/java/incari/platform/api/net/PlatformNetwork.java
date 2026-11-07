package incari.platform.api.net;

import java.io.IOException;

/**
 * Connections to game servers. Two kinds: a TCP byte stream (the desktop only), and a WebSocket
 * carrying one packet per binary message (every platform; the only kind a browser has). The game's
 * own protocol code (compression, packets) runs on top of either. Small files come by HTTP GET
 * ({@link #fetch}).
 */
public interface PlatformNetwork {

    /**
     * Starts a TCP connection and returns at once. The listener hears {@link
     * NetConnection.Listener#connected} or {@link NetConnection.Listener#closed} with the reason it
     * failed. Bytes arrive as the stream delivers them, with no message boundaries.
     *
     * <p>A platform without raw sockets (the browser) fails the connection through the listener.
     *
     * @param host a host name or IP address literal
     * @param port the TCP port
     */
    NetConnection connect(String host, int port, NetConnection.Listener listener);

    /**
     * Starts a WebSocket connection and returns at once. Messages are binary: each {@link
     * NetConnection#send} is one whole message, and each {@link NetConnection.Listener#received}
     * buffer is one whole message as the server sent it. Text messages go both ways too ({@link
     * NetConnection#sendText}, {@link NetConnection.Listener#receivedText}), for Eagler's queries.
     *
     * @param url a {@code ws://} or {@code wss://} URL, already checked for syntax by the caller
     */
    NetConnection openWebSocket(String url, NetConnection.Listener listener);

    /**
     * The body of an HTTP GET of {@code url}: public files such as player skins and server
     * resource packs. Blocks the calling thread until the whole body has arrived, so call it from a
     * background thread, never the render thread. The bytes are the caller's. No headers beyond
     * the platform's own are sent.
     *
     * <p>In a browser the server must allow the page's origin (CORS), and an {@code https} page
     * can't fetch {@code http} URLs.
     *
     * @param url an absolute {@code http} or {@code https} URL
     * @param maxBytes the largest body accepted; a bigger one fails, before it's all downloaded
     *     when the server gives its length
     * @throws IOException if it can't be fetched, the answer isn't a 2xx, or the body is too big
     */
    default byte[] fetch(String url, int maxBytes) throws IOException {
        throw new IOException("This platform can't fetch " + url);
    }

    /**
     * Whether other machines can connect to this one: the game's "Open to LAN" listens on a TCP
     * port. A page can't listen for connections; its worlds are shared through an Eagler relay
     * instead.
     */
    default boolean canAcceptConnections() {
        return true;
    }

    /**
     * Whether {@link #connect} opens real TCP connections. A page can't: what needs TCP there goes
     * through a Wisp proxy instead (Java servers, ADR 0065; Microsoft sign-in, ADR 0070).
     */
    default boolean canOpenSockets() {
        return true;
    }
}
