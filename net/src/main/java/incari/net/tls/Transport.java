package incari.net.tls;

import incari.net.WebSocketUrl;
import incari.net.wisp.WispAddress;
import incari.net.wisp.WispConnection;
import incari.platform.api.net.NetConnection;
import incari.platform.api.net.PlatformNetwork;

/**
 * Where TLS's bytes go: a TCP connection to a host and port, however this platform makes one. The
 * desktop opens a socket; a page, which has none, opens a stream through a Wisp proxy, which sees
 * only the encrypted bytes.
 */
@FunctionalInterface
public interface Transport {

    /** Starts a TCP connection and returns at once, as {@link PlatformNetwork#connect} does. */
    NetConnection connect(String host, int port, NetConnection.Listener listener);

    /** A socket of this machine's own (the desktop). */
    static Transport direct(PlatformNetwork network) {
        return network::connect;
    }

    /** A stream through the Wisp proxy at {@code proxy} (the browser). */
    static Transport wisp(PlatformNetwork network, WebSocketUrl proxy) {
        return (host, port, listener) -> WispConnection.open(network, new WispAddress(proxy, host, port), listener);
    }
}
