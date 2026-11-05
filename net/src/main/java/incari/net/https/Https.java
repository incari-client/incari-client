package incari.net.https;

import java.util.concurrent.CompletableFuture;

/** Sends HTTPS requests: {@link HttpsClient}, or a fake in tests. */
@FunctionalInterface
public interface Https {

    /** Sends {@code request}; the future fails with an {@link java.io.IOException} on any failure. */
    CompletableFuture<HttpResponse> send(HttpRequest request);
}
