package incari.net.https;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import org.junit.jupiter.api.Test;

class ResponseParserTest {

    @Test
    void readsABodyByLength() throws IOException {
        HttpResponse response =
                whole("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}");
        assertThat(response.status()).isEqualTo(200);
        assertThat(response.header("content-type")).isEqualTo("application/json");
        assertThat(response.text()).isEqualTo("{}");
    }

    @Test
    void readsABodyByteByByte() throws IOException {
        byte[] bytes = "HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello".getBytes(StandardCharsets.US_ASCII);
        ResponseParser parser = new ResponseParser(false);
        HttpResponse response = null;
        for (int i = 0; i < bytes.length; i++) {
            assertThat(response).isNull();
            response = parser.feed(bytes, i, 1);
        }
        assertThat(response).isNotNull();
        assertThat(response.text()).isEqualTo("hello");
    }

    @Test
    void readsChunks() throws IOException {
        HttpResponse response = whole(String.join(
                "\r\n",
                "HTTP/1.1 200 OK",
                "Transfer-Encoding: chunked",
                "",
                "4",
                "Wiki",
                "5;ext=1",
                "pedia",
                "0",
                "Trailer: x",
                "",
                ""));
        assertThat(response.text()).isEqualTo("Wikipedia");
    }

    @Test
    void skipsInterimResponses() throws IOException {
        HttpResponse response = whole("HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 204 No Content\r\n\r\n");
        assertThat(response.status()).isEqualTo(204);
        assertThat(response.body()).isEmpty();
    }

    @Test
    void readsUntilACleanCloseOnly() throws IOException {
        ResponseParser parser = new ResponseParser(false);
        byte[] bytes = "HTTP/1.1 200 OK\r\n\r\npartial".getBytes(StandardCharsets.US_ASCII);
        assertThat(parser.feed(bytes, 0, bytes.length)).isNull();
        assertThat(parser.finish(true).text()).isEqualTo("partial");

        ResponseParser truncated = new ResponseParser(false);
        truncated.feed(bytes, 0, bytes.length);
        assertThatThrownBy(() -> truncated.finish(false)).hasMessageContaining("cut off");
    }

    @Test
    void refusesAShortBody() throws IOException {
        ResponseParser parser = new ResponseParser(false);
        byte[] bytes = "HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nabc".getBytes(StandardCharsets.US_ASCII);
        parser.feed(bytes, 0, bytes.length);
        assertThatThrownBy(() -> parser.finish(true)).hasMessageContaining("cut off");
    }

    @Test
    void refusesHugeAndMalformedResponses() {
        assertThatThrownBy(() -> whole("HTTP/1.1 200 OK\r\nContent-Length: 99999999\r\n\r\n"))
                .hasMessageContaining("too big");
        assertThatThrownBy(() -> whole("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nFFFFFFF\r\n"))
                .hasMessageContaining("too big");
        assertThatThrownBy(() -> whole("SSH-2.0-OpenSSH\r\n\r\n")).hasMessageContaining("Not an HTTP");
        assertThatThrownBy(() -> whole("HTTP/1.1 200 OK\r\nContent-Length: 1\r\nContent-Length: 2\r\n\r\n"))
                .hasMessageContaining("Two different");
        assertThatThrownBy(() -> whole("HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip\r\n\r\n"))
                .hasMessageContaining("unsupported transfer");
        assertThatThrownBy(() -> whole("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabcd\r\n"))
                .hasMessageContaining("runs over");
        String longHeader = "HTTP/1.1 200 OK\r\nX: " + "a".repeat(ResponseParser.MAX_HEAD) + "\r\n\r\n";
        assertThatThrownBy(() -> whole(longHeader)).hasMessageContaining("too long");
    }

    private static HttpResponse whole(String text) throws IOException {
        byte[] bytes = text.getBytes(StandardCharsets.US_ASCII);
        HttpResponse response = new ResponseParser(false).feed(bytes, 0, bytes.length);
        if (response == null) {
            throw new IOException("incomplete");
        }
        return response;
    }
}
