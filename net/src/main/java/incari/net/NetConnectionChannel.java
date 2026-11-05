package incari.net;

import incari.net.wisp.WispAddress;
import incari.net.wisp.WispConnection;
import incari.platform.api.net.NetConnection;
import incari.platform.api.net.PlatformNetwork;
import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;
import io.netty.channel.AbstractChannel;
import io.netty.channel.ChannelConfig;
import io.netty.channel.ChannelId;
import io.netty.channel.ChannelMetadata;
import io.netty.channel.ChannelOutboundBuffer;
import io.netty.channel.ChannelPromise;
import io.netty.channel.ConnectTimeoutException;
import io.netty.channel.DefaultChannelConfig;
import io.netty.channel.EventLoop;
import java.net.ConnectException;
import java.net.InetSocketAddress;
import java.net.SocketAddress;
import java.nio.ByteBuffer;
import java.util.ArrayDeque;
import java.util.Queue;
import java.util.concurrent.ScheduledFuture;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicLong;
import org.jspecify.annotations.Nullable;

/**
 * A Netty client channel over one of the platform's connections ({@code PlatformNetwork}), so the
 * game's own pipeline (compression, packets) runs unchanged over a TCP socket on the desktop or a
 * WebSocket anywhere. Connecting to a {@link WebSocketAddress} opens a WebSocket; each buffer the
 * pipeline writes is then one message, and each message arrives as one buffer.
 *
 * <p>It registers no I/O with its event loop: the platform delivers bytes on its own thread, and
 * the channel hops onto the event loop before touching the pipeline. So it runs on any event loop.
 */
public abstract class NetConnectionChannel extends AbstractChannel {

    private static final ChannelMetadata METADATA = new ChannelMetadata(false);
    private static final AtomicLong NEXT_ID = new AtomicLong();

    private enum State {
        OPEN,
        CONNECTING,
        ACTIVE,
        CLOSED
    }

    private final ChannelConfig config = new DefaultChannelConfig(this);
    /** Bytes that arrived while the pipeline wasn't reading (auto-read off). Event loop only. */
    private final Queue<ByteBuf> unread = new ArrayDeque<>();

    private volatile State state = State.OPEN;
    private volatile @Nullable NetConnection connection;
    private volatile @Nullable InetSocketAddress remoteAddress;
    private @Nullable ChannelPromise connectPromise;
    /**
     * Gives up on a connect that neither opens nor fails ({@code CONNECT_TIMEOUT_MILLIS}, 30 s by
     * default, as Netty's socket channels do): a server behind a firewall that drops packets would
     * otherwise leave "Connecting to the server" up for as long as the browser waits. Event loop only.
     */
    private @Nullable ScheduledFuture<?> connectTimeout;
    /** Whether the pipeline asked for data (Netty's read()); event loop only. */
    private boolean readRequested;

    // The config only keeps a reference to the channel; it reads nothing from it while constructed.
    @SuppressWarnings("this-escape")
    protected NetConnectionChannel() {
        super(null);
    }

    /**
     * Opens the platform connection to {@code remote}, usually with {@link #openOn}. Called once, on
     * the event loop; must return at once.
     */
    protected abstract NetConnection open(InetSocketAddress remote, NetConnection.Listener listener);

    /**
     * {@link #open} on a platform: a WebSocket for a {@link WebSocketAddress}, a stream through a Wisp
     * proxy for a {@link WispAddress}, a TCP socket otherwise.
     */
    protected static NetConnection openOn(
            final PlatformNetwork network, final InetSocketAddress remote, final NetConnection.Listener listener) {
        if (remote instanceof WebSocketAddress webSocket) {
            return network.openWebSocket(webSocket.url().toString(), listener);
        }
        if (remote instanceof WispAddress wisp) {
            return WispConnection.open(network, wisp, listener);
        }
        return network.connect(remote.getHostString(), remote.getPort(), listener);
    }

    /**
     * A plain counter: Netty's default id is built from the machine's MAC address and process id,
     * which a page can't read (its lookup would fail in the browser build).
     */
    @Override
    protected ChannelId newId() {
        return new ConnectionId(NEXT_ID.incrementAndGet());
    }

    @Override
    public ChannelConfig config() {
        return this.config;
    }

    @Override
    public boolean isOpen() {
        return this.state != State.CLOSED;
    }

    @Override
    public boolean isActive() {
        return this.state == State.ACTIVE;
    }

    @Override
    public ChannelMetadata metadata() {
        return METADATA;
    }

    @Override
    protected AbstractUnsafe newUnsafe() {
        return new ConnectionUnsafe();
    }

    @Override
    protected boolean isCompatible(final EventLoop loop) {
        return true;
    }

    @Override
    protected @Nullable SocketAddress localAddress0() {
        return null;
    }

    @Override
    protected @Nullable SocketAddress remoteAddress0() {
        return this.remoteAddress;
    }

    @Override
    protected void doBind(final SocketAddress localAddress) {
        throw new UnsupportedOperationException("Platform connections can't choose a local address");
    }

    @Override
    protected void doDisconnect() {
        this.doClose();
    }

    /** The connect took too long: fails it and closes the connection still opening. Event loop only. */
    private void onConnectTimeout(final ChannelPromise promise, final int millis) {
        this.connectTimeout = null;
        // A channel connects once: if this connect is still pending, the promise is this one.
        if (this.connectPromise == null || this.state != State.CONNECTING) {
            return;
        }
        this.connectPromise = null;
        promise.tryFailure(new ConnectTimeoutException(
                "Connection timed out after " + (millis / 1000) + " seconds: " + this.remoteAddress));
        if (this.isOpen()) {
            this.unsafe().close(this.unsafe().voidPromise());
        }
    }

    private void cancelConnectTimeout() {
        ScheduledFuture<?> timeout = this.connectTimeout;
        this.connectTimeout = null;
        if (timeout != null) {
            timeout.cancel(false);
        }
    }

    @Override
    protected void doClose() {
        this.cancelConnectTimeout();
        this.state = State.CLOSED;
        NetConnection current = this.connection;
        if (current != null) {
            current.close();
        }
        ByteBuf pending;
        while ((pending = this.unread.poll()) != null) {
            pending.release();
        }
    }

    @Override
    protected void doBeginRead() {
        this.readRequested = true;
        this.deliverUnread();
    }

    @Override
    protected void doWrite(final ChannelOutboundBuffer outbound) {
        NetConnection current = this.connection;
        while (true) {
            Object message = outbound.current();
            if (message == null) {
                if (current != null) {
                    current.flush();
                }
                return;
            }
            if (message instanceof ByteBuf buffer && current != null) {
                // The platform keeps the bytes it's given; copy them out of Netty's pooled buffer.
                // One buffer is one WebSocket message: the framing handlers write whole packets.
                ByteBuffer copy = ByteBuffer.allocate(buffer.readableBytes());
                buffer.getBytes(buffer.readerIndex(), copy);
                copy.flip();
                current.send(copy);
                outbound.remove();
            } else {
                outbound.remove(
                        new UnsupportedOperationException("Can't write " + message + " to a platform connection"));
            }
        }
    }

    private void deliverUnread() {
        boolean delivered = false;
        while (this.readRequested && !this.unread.isEmpty()) {
            ByteBuf data = this.unread.remove();
            this.pipeline().fireChannelRead(data);
            delivered = true;
            if (!this.config.isAutoRead()) {
                // Without auto-read, each read() asks for one batch.
                this.readRequested = false;
            }
        }
        if (delivered) {
            this.pipeline().fireChannelReadComplete();
        }
    }

    private final class ConnectionUnsafe extends AbstractUnsafe {

        @Override
        public void connect(
                final SocketAddress remote, final @Nullable SocketAddress local, final ChannelPromise promise) {
            if (!promise.setUncancellable() || !this.ensureOpen(promise)) {
                return;
            }
            if (NetConnectionChannel.this.state != State.OPEN) {
                promise.setFailure(new IllegalStateException("Already connecting or connected"));
                return;
            }
            if (!(remote instanceof InetSocketAddress address)) {
                promise.setFailure(new UnsupportedOperationException("Can't connect to " + remote));
                return;
            }
            NetConnectionChannel.this.state = State.CONNECTING;
            NetConnectionChannel.this.remoteAddress = address;
            NetConnectionChannel.this.connectPromise = promise;
            int millis = NetConnectionChannel.this.config.getConnectTimeoutMillis();
            if (millis > 0) {
                NetConnectionChannel.this.connectTimeout = NetConnectionChannel.this
                        .eventLoop()
                        .schedule(
                                () -> NetConnectionChannel.this.onConnectTimeout(promise, millis),
                                millis,
                                TimeUnit.MILLISECONDS);
            }
            NetConnectionChannel.this.connection = NetConnectionChannel.this.open(address, new EventLoopListener());
        }
    }

    /** A channel's id: its number among this process's platform connections. */
    private record ConnectionId(long number) implements ChannelId {

        private static final long serialVersionUID = 1L;

        @Override
        public String asShortText() {
            return Long.toHexString(number);
        }

        @Override
        public String asLongText() {
            return "incari-" + asShortText();
        }

        @Override
        public int compareTo(final ChannelId other) {
            if (other instanceof ConnectionId that) {
                return Long.compare(number, that.number);
            }
            return asLongText().compareTo(other.asLongText());
        }

        @Override
        public String toString() {
            return asShortText();
        }
    }

    /** Moves each platform callback onto the channel's event loop, where Netty expects it. */
    private final class EventLoopListener implements NetConnection.Listener {

        @Override
        public void connected() {
            NetConnectionChannel.this.eventLoop().execute(NetConnectionChannel.this::onConnected);
        }

        @Override
        public void received(final ByteBuffer data) {
            ByteBuf buffer = Unpooled.wrappedBuffer(data);
            NetConnectionChannel.this.eventLoop().execute(() -> NetConnectionChannel.this.onReceived(buffer));
        }

        @Override
        public void closed(final @Nullable Throwable cause) {
            NetConnectionChannel.this.eventLoop().execute(() -> NetConnectionChannel.this.onClosed(cause));
        }
    }

    private void onConnected() {
        this.cancelConnectTimeout();
        ChannelPromise promise = this.connectPromise;
        this.connectPromise = null;
        if (this.state != State.CONNECTING || promise == null) {
            return;
        }
        this.state = State.ACTIVE;
        if (promise.trySuccess()) {
            this.pipeline().fireChannelActive();
        } else {
            // The connect was given up on (timed out, cancelled) while it was opening.
            this.unsafe().close(this.unsafe().voidPromise());
        }
    }

    private void onReceived(final ByteBuf data) {
        if (this.state != State.ACTIVE) {
            data.release();
            return;
        }
        this.unread.add(data);
        if (this.config.isAutoRead()) {
            this.readRequested = true;
        }
        this.deliverUnread();
    }

    private void onClosed(final @Nullable Throwable cause) {
        this.cancelConnectTimeout();
        ChannelPromise promise = this.connectPromise;
        this.connectPromise = null;
        if (promise != null) {
            InetSocketAddress address = this.remoteAddress;
            ConnectException failure = new ConnectException("Couldn't connect to " + address);
            if (cause != null) {
                failure.initCause(cause);
            }
            promise.tryFailure(failure);
        } else if (cause != null && this.state == State.ACTIVE) {
            this.pipeline().fireExceptionCaught(cause);
        }
        if (this.isOpen()) {
            this.unsafe().close(this.unsafe().voidPromise());
        }
    }
}
