package incari.net.https;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.HashMap;
import java.util.Locale;
import java.util.Map;
import org.jspecify.annotations.Nullable;

/**
 * Reads one HTTP/1.1 response as its bytes arrive: the status line and headers, then a body framed
 * by {@code Content-Length}, by chunks, or (with neither) by the connection's end. Sizes are capped,
 * since a hostile proxy can't forge a response but a hostile or broken server could send any.
 *
 * <p>Interim 1xx responses are skipped. Not thread-safe; its connection feeds it in order.
 */
final class ResponseParser {

    static final int MAX_HEAD = 32 * 1024;
    static final int MAX_BODY = 1024 * 1024;

    /** CR LF CR LF as four bytes of an int. */
    private static final int BLANK_LINE = 0x0D0A0D0A;

    private enum State {
        HEAD,
        BODY_LENGTH,
        CHUNK_SIZE,
        CHUNK_DATA,
        CHUNK_END,
        TRAILERS,
        UNTIL_CLOSE,
        DONE
    }

    private final boolean head;
    private State state = State.HEAD;
    private final ByteArrayOutputStream line = new ByteArrayOutputStream();
    private final ByteArrayOutputStream headBytes = new ByteArrayOutputStream();
    private final ByteArrayOutputStream body = new ByteArrayOutputStream();
    private int status;
    private Map<String, String> headers = new HashMap<>();
    private long remaining;
    /** The last four bytes of the head, to spot the blank line that ends it. */
    private int headTail;
    /** The byte before the current one in a line, to spot its CRLF. */
    private byte lastLineByte;

    /** @param head whether the request was a HEAD (or otherwise expects no body) */
    ResponseParser(boolean head) {
        this.head = head;
    }

    /**
     * Takes more of the stream.
     *
     * @return the whole response once it's complete, else null
     * @throws IOException if the response is malformed or too big
     */
    @Nullable
    HttpResponse feed(byte[] data, int offset, int length) throws IOException {
        int i = offset;
        int end = offset + length;
        while (i < end && state != State.DONE) {
            switch (state) {
                case HEAD -> {
                    byte b = data[i++];
                    headBytes.write(b);
                    if (headBytes.size() > MAX_HEAD) {
                        throw new IOException("The response's headers are too long");
                    }
                    headTail = (headTail << 8) | (b & 0xFF);
                    if (headTail == BLANK_LINE) {
                        headTail = 0;
                        parseHead();
                    }
                }
                case BODY_LENGTH -> {
                    int n = (int) Math.min(remaining, end - i);
                    writeBody(data, i, n);
                    i += n;
                    remaining -= n;
                    if (remaining == 0) {
                        state = State.DONE;
                    }
                }
                case CHUNK_SIZE -> {
                    if (readLine(data[i++])) {
                        remaining = chunkSize(takeLine());
                        state = remaining == 0 ? State.TRAILERS : State.CHUNK_DATA;
                    }
                }
                case CHUNK_DATA -> {
                    int n = (int) Math.min(remaining, end - i);
                    writeBody(data, i, n);
                    i += n;
                    remaining -= n;
                    if (remaining == 0) {
                        state = State.CHUNK_END;
                    }
                }
                case CHUNK_END -> {
                    if (readLine(data[i++])) {
                        if (!takeLine().isEmpty()) {
                            throw new IOException("A chunk of the response runs over its size");
                        }
                        state = State.CHUNK_SIZE;
                    }
                }
                case TRAILERS -> {
                    if (readLine(data[i++]) && takeLine().isEmpty()) {
                        state = State.DONE;
                    }
                }
                case UNTIL_CLOSE -> {
                    writeBody(data, i, end - i);
                    i = end;
                }
                case DONE -> throw new IllegalStateException();
            }
        }
        return state == State.DONE ? response() : null;
    }

    /**
     * The connection ended. A body framed by the connection's end is whole only if the end was a
     * proper TLS close ({@code cleanClose}); anything else still waiting is truncated.
     */
    HttpResponse finish(boolean cleanClose) throws IOException {
        if (state == State.DONE || (state == State.UNTIL_CLOSE && cleanClose)) {
            return response();
        }
        throw new IOException("The response was cut off");
    }

    private HttpResponse response() {
        state = State.DONE;
        return new HttpResponse(status, headers, body.toByteArray());
    }

    private void parseHead() throws IOException {
        String text = headBytes.toString(StandardCharsets.ISO_8859_1);
        headBytes.reset();
        String[] lines = text.split("\r\n", -1);
        String[] statusLine = lines[0].split(" ", 3);
        if (statusLine.length < 2 || !statusLine[0].startsWith("HTTP/1.")) {
            throw new IOException("Not an HTTP/1.x response");
        }
        try {
            status = Integer.parseInt(statusLine[1]);
        } catch (NumberFormatException e) {
            throw new IOException("Not an HTTP status: " + statusLine[1], e);
        }
        if (status < 100 || status > 999) {
            throw new IOException("Not an HTTP status: " + status);
        }
        Map<String, String> parsed = new HashMap<>();
        for (int i = 1; i < lines.length; i++) {
            String header = lines[i];
            if (header.isEmpty()) {
                continue;
            }
            int colon = header.indexOf(':');
            if (colon <= 0) {
                throw new IOException("A malformed response header");
            }
            String name = header.substring(0, colon).strip().toLowerCase(Locale.ROOT);
            String value = header.substring(colon + 1).strip();
            if (name.equals("content-length")
                    && parsed.containsKey(name)
                    && !parsed.get(name).equals(value)) {
                throw new IOException("Two different response lengths");
            }
            parsed.put(name, value);
        }
        if (status < 200) {
            // An interim response (100 Continue): the real one follows.
            return;
        }
        headers = parsed;
        if (head || status == 204 || status == 304) {
            state = State.DONE;
            return;
        }
        String transfer = parsed.get("transfer-encoding");
        if (transfer != null) {
            if (!transfer.equalsIgnoreCase("chunked")) {
                throw new IOException("An unsupported transfer encoding: " + transfer);
            }
            state = State.CHUNK_SIZE;
            return;
        }
        String length = parsed.get("content-length");
        if (length != null) {
            try {
                remaining = Long.parseLong(length);
            } catch (NumberFormatException e) {
                throw new IOException("Not a response length: " + length, e);
            }
            if (remaining < 0 || remaining > MAX_BODY) {
                throw new IOException("The response is too big");
            }
            state = remaining == 0 ? State.DONE : State.BODY_LENGTH;
            return;
        }
        state = State.UNTIL_CLOSE;
    }

    private void writeBody(byte[] data, int offset, int length) throws IOException {
        if (body.size() + (long) length > MAX_BODY) {
            throw new IOException("The response is too big");
        }
        body.write(data, offset, length);
    }

    /** Adds a byte to the current line; true once the line has ended (CRLF). */
    private boolean readLine(byte b) throws IOException {
        line.write(b);
        if (line.size() > 1024) {
            throw new IOException("A line of the response is too long");
        }
        boolean ended = lastLineByte == '\r' && b == '\n';
        lastLineByte = ended ? 0 : b;
        return ended;
    }

    private String takeLine() {
        String text = line.toString(StandardCharsets.ISO_8859_1);
        line.reset();
        return text.substring(0, text.length() - 2);
    }

    private long chunkSize(String text) throws IOException {
        int semicolon = text.indexOf(';');
        String hex = (semicolon < 0 ? text : text.substring(0, semicolon)).strip();
        if (hex.isEmpty() || hex.length() > 8) {
            throw new IOException("Not a chunk size: " + hex);
        }
        long size;
        try {
            size = Long.parseLong(hex, 16);
        } catch (NumberFormatException e) {
            throw new IOException("Not a chunk size: " + hex, e);
        }
        if (size < 0 || body.size() + size > MAX_BODY) {
            throw new IOException("The response is too big");
        }
        return size;
    }
}
