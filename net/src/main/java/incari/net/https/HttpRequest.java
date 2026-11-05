package incari.net.https;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * One HTTPS request: method, host ({@code host}, lower case), path and query, headers and body. Built from an {@code https://} URL;
 * other schemes and ports are refused, since sign-in only ever talks to HTTPS hosts on 443.
 * {@code Host}, {@code Content-Length} and {@code Connection: close} are added on the wire.
 */
public final class HttpRequest {

    private final String method;
    private final String host;
    private final String path;
    private final Map<String, String> headers;
    private final byte[] body;

    public HttpRequest(String method, String host, String path, Map<String, String> headers, byte[] body) {
        this.method = method;
        this.host = host;
        this.path = path;
        this.headers = Map.copyOf(headers);
        this.body = body.clone();
        checkToken(method);
        checkHost(host);
        if (!path.startsWith("/") || hasControl(path) || path.indexOf(' ') >= 0) {
            throw new IllegalArgumentException("Not a request path: " + path);
        }
        for (Map.Entry<String, String> header : this.headers.entrySet()) {
            checkToken(header.getKey());
            if (hasControl(header.getValue())) {
                throw new IllegalArgumentException("Header " + header.getKey() + " has a control character");
            }
        }
    }

    public String method() {
        return method;
    }

    public String host() {
        return host;
    }

    public String path() {
        return path;
    }

    public Map<String, String> headers() {
        return headers;
    }

    public byte[] body() {
        return body.clone();
    }

    /** A GET of {@code url}. */
    public static HttpRequest get(String url, Map<String, String> headers) {
        Target target = Target.parse(url);
        return new HttpRequest("GET", target.host(), target.path(), headers, new byte[0]);
    }

    /** A POST of {@code body} as {@code contentType} to {@code url}. */
    public static HttpRequest post(String url, String contentType, String body, Map<String, String> headers) {
        Target target = Target.parse(url);
        Map<String, String> all = new LinkedHashMap<>(headers);
        all.put("Content-Type", contentType);
        return new HttpRequest("POST", target.host(), target.path(), all, body.getBytes(StandardCharsets.UTF_8));
    }

    /** The request as it goes on the wire: HTTP/1.1, one request, then the connection closes. */
    byte[] encode() {
        StringBuilder head = new StringBuilder();
        head.append(method).append(' ').append(path).append(" HTTP/1.1\r\n");
        head.append("Host: ").append(host).append("\r\n");
        head.append("Connection: close\r\n");
        head.append("Accept-Encoding: identity\r\n");
        if (body.length > 0 || method.equals("POST")) {
            head.append("Content-Length: ").append(body.length).append("\r\n");
        }
        List<String> names = new ArrayList<>(headers.keySet());
        names.sort(String.CASE_INSENSITIVE_ORDER);
        for (String name : names) {
            head.append(name).append(": ").append(headers.get(name)).append("\r\n");
        }
        head.append("\r\n");
        byte[] start = head.toString().getBytes(StandardCharsets.UTF_8);
        byte[] all = new byte[start.length + body.length];
        System.arraycopy(start, 0, all, 0, start.length);
        System.arraycopy(body, 0, all, start.length, body.length);
        return all;
    }

    @Override
    public String toString() {
        // Never the body or headers: they carry tokens.
        return method + " https://" + host + pathWithoutQuery();
    }

    private String pathWithoutQuery() {
        int query = path.indexOf('?');
        return query < 0 ? path : path.substring(0, query);
    }

    private static void checkToken(String token) {
        if (token.isEmpty()) {
            throw new IllegalArgumentException("Empty HTTP token");
        }
        for (int i = 0; i < token.length(); i++) {
            char c = token.charAt(i);
            boolean ok =
                    (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || "-_.".indexOf(c) >= 0;
            if (!ok) {
                throw new IllegalArgumentException("Not an HTTP token: " + token);
            }
        }
    }

    private static void checkHost(String host) {
        if (host.isEmpty() || !host.equals(host.toLowerCase(java.util.Locale.ROOT))) {
            throw new IllegalArgumentException("Not a lower-case host: " + host);
        }
        for (int i = 0; i < host.length(); i++) {
            char c = host.charAt(i);
            if (!((c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '-' || c == '.')) {
                throw new IllegalArgumentException("Not a host name: " + host);
            }
        }
    }

    private static boolean hasControl(String text) {
        for (int i = 0; i < text.length(); i++) {
            char c = text.charAt(i);
            if (c < 0x20 || c == 0x7F) {
                return true;
            }
        }
        return false;
    }

    /** An {@code https://host/path} URL, split by hand (the browser build has no {@code java.net.URI}). */
    record Target(String host, String path) {

        static Target parse(String url) {
            String prefix = "https://";
            if (!url.startsWith(prefix)) {
                throw new IllegalArgumentException("Not an https URL: " + url);
            }
            String rest = url.substring(prefix.length());
            int slash = rest.indexOf('/');
            String authority = slash < 0 ? rest : rest.substring(0, slash);
            String path = slash < 0 ? "/" : rest.substring(slash);
            if (authority.indexOf('@') >= 0 || authority.indexOf(':') >= 0) {
                throw new IllegalArgumentException("Only a plain host on port 443: " + url);
            }
            return new Target(authority.toLowerCase(java.util.Locale.ROOT), path);
        }
    }
}
