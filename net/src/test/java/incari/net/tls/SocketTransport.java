package incari.net.tls;

import incari.platform.api.net.NetConnection;
import java.io.IOException;
import java.io.InputStream;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.nio.ByteBuffer;
import java.util.Arrays;
import org.jspecify.annotations.Nullable;

/** A plain TCP socket as a {@link Transport}, with a reader thread, for live tests on a JVM. */
public final class SocketTransport implements Transport {

    @Override
    public NetConnection connect(String host, int port, NetConnection.Listener listener) {
        Socket socket = new Socket();
        Thread reader = new Thread(
                () -> {
                    @Nullable Throwable failure = null;
                    try {
                        socket.connect(new InetSocketAddress(host, port), 10_000);
                        listener.connected();
                        InputStream in = socket.getInputStream();
                        byte[] buffer = new byte[16384];
                        int n;
                        while ((n = in.read(buffer)) >= 0) {
                            listener.received(ByteBuffer.wrap(Arrays.copyOf(buffer, n)));
                        }
                    } catch (IOException e) {
                        failure = socket.isClosed() ? null : e;
                    }
                    listener.closed(failure);
                },
                "socket " + host);
        reader.setDaemon(true);
        reader.start();
        return new NetConnection() {
            @Override
            public void send(ByteBuffer data) {
                try {
                    byte[] bytes = new byte[data.remaining()];
                    data.get(bytes);
                    socket.getOutputStream().write(bytes);
                } catch (IOException e) {
                    close();
                }
            }

            @Override
            public void close() {
                try {
                    socket.close();
                } catch (IOException ignored) {
                    // Closing.
                }
            }

            @Override
            public boolean isOpen() {
                return socket.isConnected() && !socket.isClosed();
            }

            @Override
            public String remoteDescription() {
                return host + ":" + port;
            }
        };
    }
}
