package incari.platform.desktop.net;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import com.sun.net.httpserver.HttpServer;
import incari.platform.api.PlatformLog;
import incari.platform.api.net.NetConnection;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import org.jspecify.annotations.Nullable;
import org.junit.jupiter.api.Test;

class DesktopNetworkTest {

    private final DesktopNetwork network = new DesktopNetwork(PlatformLog.NONE);

    @Test
    void fetchesABodyAndFailsOnAnErrorStatusOrOneTooBig() throws Exception {
        HttpServer server = HttpServer.create(new InetSocketAddress(InetAddress.getLoopbackAddress(), 0), 0);
        server.createContext("/skin", exchange -> {
            byte[] body = {(byte) 0x89, 'P', 'N', 'G'};
            exchange.sendResponseHeaders(200, body.length);
            exchange.getResponseBody().write(body);
            exchange.close();
        });
        server.createContext("/chunked", exchange -> {
            // Length 0: chunked, so the size is only known by reading.
            exchange.sendResponseHeaders(200, 0);
            exchange.getResponseBody().write(new byte[10]);
            exchange.close();
        });
        server.createContext("/missing", exchange -> {
            exchange.sendResponseHeaders(404, -1);
            exchange.close();
        });
        server.start();
        try {
            String base = "http://127.0.0.1:" + server.getAddress().getPort();
            assertThat(network.fetch(base + "/skin", 4)).containsExactly((byte) 0x89, 'P', 'N', 'G');
            assertThatThrownBy(() -> network.fetch(base + "/skin", 3))
                    .isInstanceOf(IOException.class)
                    .hasMessageContaining("over the limit");
            assertThat(network.fetch(base + "/chunked", 10)).hasSize(10);
            assertThatThrownBy(() -> network.fetch(base + "/chunked", 9))
                    .isInstanceOf(IOException.class)
                    .hasMessageContaining("over the limit");
            assertThatThrownBy(() -> network.fetch(base + "/missing", 4))
                    .isInstanceOf(IOException.class)
                    .hasMessageContaining("404");
        } finally {
            server.stop(0);
        }
    }

    @Test
    void sendsQueuedBytesOnceConnectedAndReceivesTheReply() throws Exception {
        try (ServerSocket server = new ServerSocket(0, 1, InetAddress.getLoopbackAddress())) {
            CompletableFuture<String> serverSaw = new CompletableFuture<>();
            Thread.ofVirtual().start(() -> {
                try (Socket client = server.accept()) {
                    serverSaw.complete(readExactly(client.getInputStream(), 5));
                    OutputStream out = client.getOutputStream();
                    out.write("world".getBytes(StandardCharsets.UTF_8));
                    out.flush();
                } catch (IOException e) {
                    serverSaw.completeExceptionally(e);
                }
            });

            Recorder recorder = new Recorder();
            NetConnection connection = network.connect("127.0.0.1", server.getLocalPort(), recorder);
            // Sent before the connection opens: must be queued, then written in order.
            connection.send(ByteBuffer.wrap("hel".getBytes(StandardCharsets.UTF_8)));
            connection.send(ByteBuffer.wrap("lo".getBytes(StandardCharsets.UTF_8)));

            assertThat(serverSaw.get(5, TimeUnit.SECONDS)).isEqualTo("hello");
            assertThat(recorder.connected.await(5, TimeUnit.SECONDS)).isTrue();
            assertThat(recorder.closed.await(5, TimeUnit.SECONDS)).isTrue();
            assertThat(recorder.received()).isEqualTo("world");
            assertThat(recorder.cause).isNull();
            assertThat(connection.isOpen()).isFalse();
        }
    }

    @Test
    void reportsAFailedConnectOnceWithTheCause() throws Exception {
        int unusedPort;
        try (ServerSocket probe = new ServerSocket(0, 1, InetAddress.getLoopbackAddress())) {
            unusedPort = probe.getLocalPort();
        }
        Recorder recorder = new Recorder();

        network.connect("127.0.0.1", unusedPort, recorder);

        assertThat(recorder.closed.await(10, TimeUnit.SECONDS)).isTrue();
        assertThat(recorder.connected.getCount()).isEqualTo(1);
        assertThat(recorder.cause).isInstanceOf(IOException.class);
        assertThat(recorder.closedCalls.get()).isEqualTo(1);
    }

    @Test
    void closingFromOurSideIsAnOrderlyClose() throws Exception {
        try (ServerSocket server = new ServerSocket(0, 1, InetAddress.getLoopbackAddress())) {
            CountDownLatch accepted = new CountDownLatch(1);
            Thread.ofVirtual().start(() -> {
                try (Socket client = server.accept()) {
                    accepted.countDown();
                    client.getInputStream().read();
                } catch (IOException e) {
                    // The test only cares about the client side.
                }
            });
            Recorder recorder = new Recorder();
            NetConnection connection = network.connect("127.0.0.1", server.getLocalPort(), recorder);
            assertThat(recorder.connected.await(5, TimeUnit.SECONDS)).isTrue();
            assertThat(accepted.await(5, TimeUnit.SECONDS)).isTrue();

            connection.close();

            assertThat(recorder.closed.await(5, TimeUnit.SECONDS)).isTrue();
            assertThat(recorder.cause).isNull();
            assertThat(recorder.closedCalls.get()).isEqualTo(1);
        }
    }

    private static String readExactly(InputStream in, int count) throws IOException {
        return new String(in.readNBytes(count), StandardCharsets.UTF_8);
    }

    /** Records what a connection reports. */
    private static final class Recorder implements NetConnection.Listener {
        final CountDownLatch connected = new CountDownLatch(1);
        final CountDownLatch closed = new CountDownLatch(1);
        final ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        volatile @Nullable Throwable cause;
        final java.util.concurrent.atomic.AtomicInteger closedCalls = new java.util.concurrent.atomic.AtomicInteger();

        @Override
        public void connected() {
            connected.countDown();
        }

        @Override
        public synchronized void received(ByteBuffer data) {
            byte[] chunk = new byte[data.remaining()];
            data.get(chunk);
            bytes.writeBytes(chunk);
        }

        @Override
        public void closed(@Nullable Throwable why) {
            cause = why;
            closedCalls.incrementAndGet();
            closed.countDown();
        }

        synchronized String received() {
            return bytes.toString(StandardCharsets.UTF_8);
        }
    }
}
