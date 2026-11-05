package incari.net.https;

import java.nio.charset.StandardCharsets;
import java.util.Map;
import org.jspecify.annotations.Nullable;

/**
 * A whole HTTP response: the status, the headers (names in lower case; a repeated header keeps its
 * last value) and the body (decoded from chunks if it was chunked).
 */
public final class HttpResponse {

    private final int status;
    private final Map<String, String> headers;
    private final byte[] body;

    public HttpResponse(int status, Map<String, String> headers, byte[] body) {
        this.status = status;
        this.headers = Map.copyOf(headers);
        this.body = body.clone();
    }

    public int status() {
        return status;
    }

    public Map<String, String> headers() {
        return headers;
    }

    public byte[] body() {
        return body.clone();
    }

    /** The body as UTF-8 text. */
    public String text() {
        return new String(body, StandardCharsets.UTF_8);
    }

    /** A header's value, by name in any case. */
    public @Nullable String header(String name) {
        return headers.get(name.toLowerCase(java.util.Locale.ROOT));
    }

    /** Whether the status is 2xx. */
    public boolean ok() {
        return status >= 200 && status < 300;
    }

    @Override
    public String toString() {
        // Never the body: it can carry tokens.
        return "HTTP " + status + " (" + body.length + " bytes)";
    }
}
