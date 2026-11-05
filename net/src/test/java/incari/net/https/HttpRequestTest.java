package incari.net.https;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import java.nio.charset.StandardCharsets;
import java.util.Map;
import org.junit.jupiter.api.Test;

class HttpRequestTest {

    @Test
    void encodesAPost() {
        HttpRequest request = HttpRequest.post(
                "https://Login.Live.com/oauth20_token.srf",
                "application/json",
                "{}",
                Map.of("Accept", "application/json"));
        assertThat(request.host()).isEqualTo("login.live.com");
        String wire = new String(request.encode(), StandardCharsets.UTF_8);
        assertThat(wire)
                .startsWith("POST /oauth20_token.srf HTTP/1.1\r\nHost: login.live.com\r\nConnection: close\r\n")
                .contains("Content-Length: 2\r\n", "Content-Type: application/json\r\n")
                .endsWith("\r\n\r\n{}");
    }

    @Test
    void refusesAnythingButPlainHttps() {
        assertThatThrownBy(() -> HttpRequest.get("http://login.live.com/", Map.of()))
                .hasMessageContaining("https");
        assertThatThrownBy(() -> HttpRequest.get("https://login.live.com:8443/", Map.of()))
                .hasMessageContaining("443");
        assertThatThrownBy(() -> HttpRequest.get("https://user@login.live.com/", Map.of()))
                .hasMessageContaining("443");
    }

    @Test
    void refusesHeaderInjection() {
        assertThatThrownBy(() -> HttpRequest.get("https://login.live.com/", Map.of("X", "a\r\nEvil: 1")))
                .hasMessageContaining("control character");
        assertThatThrownBy(() -> HttpRequest.get("https://login.live.com/", Map.of("X: y", "a")))
                .hasMessageContaining("Not an HTTP token");
        assertThatThrownBy(() -> HttpRequest.get("https://login.live.com/a b", Map.of()))
                .hasMessageContaining("Not a request path");
        assertThatThrownBy(() -> HttpRequest.get("https://login.live.com/\r\nX", Map.of()))
                .hasMessageContaining("Not a request path");
    }

    @Test
    void neverPrintsTheQueryOrBody() {
        HttpRequest request = HttpRequest.post("https://login.live.com/x?secret=1", "text/plain", "token", Map.of());
        assertThat(request.toString()).isEqualTo("POST https://login.live.com/x");
    }
}
