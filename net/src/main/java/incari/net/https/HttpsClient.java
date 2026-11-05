package incari.net.https;

import incari.net.tls.CertificateChain;
import incari.net.tls.TlsConnection;
import incari.net.tls.Transport;
import incari.platform.api.net.NetConnection;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.util.Set;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;
import java.util.function.LongSupplier;
import org.jspecify.annotations.Nullable;

/**
 * HTTPS requests over {@link TlsConnection}: one connection per request, HTTP/1.1, closed after
 * the response. Only the hosts it's built with can be reached, so nothing that reaches it can
 * point it, and the tokens it carries, anywhere else.
 *
 * <p>Nothing blocks: {@link #send} returns a future completed on the transport's thread (the
 * browser's main thread, the desktop's reader thread). Callers that touch the game hop back to it.
 */
public final class HttpsClient implements Https {

    /** How long a request may take, from connecting to the response's last byte. */
    static final long TIMEOUT_SECONDS = 30;

    private static final int PORT = 443;

    private final Transport transport;
    private final Set<String> hosts;
    private final CertificateChain certificates;
    private final LongSupplier clock;

    /**
     * @param hosts the only hosts requests may go to
     * @param clock the time certificates are checked against, in epoch milliseconds
     */
    public HttpsClient(Transport transport, Set<String> hosts, CertificateChain certificates, LongSupplier clock) {
        this.transport = transport;
        this.hosts = Set.copyOf(hosts);
        this.certificates = certificates;
        this.clock = clock;
    }

    /** Pinned roots and the system clock. */
    public static HttpsClient pinned(Transport transport, Set<String> hosts) {
        return new HttpsClient(transport, hosts, CertificateChain.pinned(), System::currentTimeMillis);
    }

    /**
     * Sends {@code request}. The future fails with an {@link IOException} if the host isn't allowed,
     * the connection or TLS fails, the response is malformed, or it takes too long.
     */
    @Override
    public CompletableFuture<HttpResponse> send(HttpRequest request) {
        CompletableFuture<HttpResponse> result = new CompletableFuture<>();
        if (!hosts.contains(request.host())) {
            result.completeExceptionally(new IOException("Incari doesn't send sign-in requests to " + request.host()));
            return result;
        }
        Exchange exchange = new Exchange(request, result);
        NetConnection connection = TlsConnection.open(transport, request.host(), PORT, certificates, clock, exchange);
        exchange.attach(connection);
        // The connection closes once the response is in, fails or times out.
        return result.orTimeout(TIMEOUT_SECONDS, TimeUnit.SECONDS)
                .whenComplete((response, failure) -> connection.close());
    }

    /** One request and its response, as the connection's listener. */
    private static final class Exchange implements NetConnection.Listener {

        private final HttpRequest request;
        private final CompletableFuture<HttpResponse> result;
        private final ResponseParser parser;
        // Guarded by this: the connection can say it's connected before open() has returned it.
        private @Nullable NetConnection connection;
        private boolean connectedEarly;

        Exchange(HttpRequest request, CompletableFuture<HttpResponse> result) {
            this.request = request;
            this.result = result;
            this.parser = new ResponseParser(request.method().equals("HEAD"));
        }

        void attach(NetConnection opened) {
            boolean sendNow;
            synchronized (this) {
                connection = opened;
                sendNow = connectedEarly;
            }
            if (sendNow) {
                sendRequest(opened);
            }
        }

        @Override
        public void connected() {
            NetConnection current;
            synchronized (this) {
                current = connection;
                connectedEarly = current == null;
            }
            if (current != null) {
                sendRequest(current);
            }
        }

        private void sendRequest(NetConnection current) {
            current.send(ByteBuffer.wrap(request.encode()));
            current.flush();
        }

        @Override
        public void received(ByteBuffer data) {
            if (result.isDone()) {
                return;
            }
            byte[] bytes = new byte[data.remaining()];
            data.get(bytes);
            try {
                synchronized (parser) {
                    HttpResponse response = parser.feed(bytes, 0, bytes.length);
                    if (response != null) {
                        result.complete(response);
                    }
                }
            } catch (IOException | RuntimeException e) {
                result.completeExceptionally(e instanceof IOException ? e : new IOException(e));
            }
        }

        @Override
        public void closed(@Nullable Throwable cause) {
            if (result.isDone()) {
                return;
            }
            try {
                synchronized (parser) {
                    result.complete(parser.finish(cause == null));
                }
            } catch (IOException e) {
                IOException failure = cause != null ? new IOException(failureText(cause), cause) : e;
                result.completeExceptionally(failure);
            }
        }

        private String failureText(Throwable cause) {
            String message = cause.getMessage();
            return "Couldn't reach " + request.host() + (message != null ? ": " + message : "");
        }
    }
}
