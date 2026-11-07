package incari.platform.desktop.net;

import incari.platform.api.PlatformLog;
import incari.platform.api.net.NetConnection;
import incari.platform.api.net.PlatformNetwork;
import java.io.IOException;
import java.io.InputStream;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import org.jspecify.annotations.Nullable;

/** Server connections as plain TCP sockets, or WebSockets and HTTP GETs through the JDK's HTTP client. */
public final class DesktopNetwork implements PlatformNetwork {

    private static final Duration CONNECT_TIMEOUT = Duration.ofSeconds(30);
    private static final Duration FETCH_TIMEOUT = Duration.ofSeconds(30);

    private final PlatformLog log;
    /** Shared by every WebSocket and fetch: it owns the selector thread and a connection pool. */
    private volatile @Nullable HttpClient http;

    public DesktopNetwork(PlatformLog log) {
        this.log = log;
    }

    @Override
    public NetConnection connect(String host, int port, NetConnection.Listener listener) {
        SocketConnection connection = new SocketConnection(host, port, listener, log);
        connection.start();
        return connection;
    }

    @Override
    public NetConnection openWebSocket(String url, NetConnection.Listener listener) {
        WebSocketConnection connection = new WebSocketConnection(url, listener, log);
        connection.start(httpClient());
        return connection;
    }

    @Override
    public byte[] fetch(String url, int maxBytes) throws IOException {
        HttpRequest request;
        try {
            request = HttpRequest.newBuilder(URI.create(url))
                    .timeout(FETCH_TIMEOUT)
                    .GET()
                    .build();
        } catch (IllegalArgumentException e) {
            throw new IOException("Can't fetch " + url + ": " + e.getMessage(), e);
        }
        HttpResponse<InputStream> response;
        try {
            response = httpClient().send(request, HttpResponse.BodyHandlers.ofInputStream());
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new IOException("Interrupted fetching " + url, e);
        }
        try (InputStream body = response.body()) {
            if (response.statusCode() / 100 != 2) {
                throw new IOException("Couldn't fetch " + url + ": HTTP " + response.statusCode());
            }
            long length = response.headers().firstValueAsLong("Content-Length").orElse(-1);
            if (length > maxBytes) {
                throw new IOException(
                        "Couldn't fetch " + url + ": the file is " + length + " bytes, over the limit of " + maxBytes);
            }
            // One byte past the limit is enough to know the body is too big.
            byte[] bytes = body.readNBytes(maxBytes + 1);
            if (bytes.length > maxBytes) {
                throw new IOException(
                        "Couldn't fetch " + url + ": the file is over the limit of " + maxBytes + " bytes");
            }
            return bytes;
        }
    }

    private HttpClient httpClient() {
        HttpClient client = http;
        if (client == null) {
            synchronized (this) {
                client = http;
                if (client == null) {
                    client = HttpClient.newBuilder()
                            .connectTimeout(CONNECT_TIMEOUT)
                            .followRedirects(HttpClient.Redirect.NEVER)
                            .build();
                    http = client;
                }
            }
        }
        return client;
    }
}
