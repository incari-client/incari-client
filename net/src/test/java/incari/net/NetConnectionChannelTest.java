package incari.net;

import static org.assertj.core.api.Assertions.assertThat;

import incari.platform.api.net.NetConnection;
import incari.platform.api.net.PlatformNetwork;
import io.netty.bootstrap.Bootstrap;
import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;
import io.netty.channel.Channel;
import io.netty.channel.ChannelFuture;
import io.netty.channel.ChannelHandlerContext;
import io.netty.channel.ChannelInboundHandlerAdapter;
import io.netty.channel.ChannelInitializer;
import io.netty.channel.ChannelOption;
import io.netty.channel.ConnectTimeoutException;
import io.netty.channel.EventLoopGroup;
import io.netty.channel.MultiThreadIoEventLoopGroup;
import io.netty.channel.local.LocalIoHandler;
import java.net.ConnectException;
import java.net.InetSocketAddress;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.TimeUnit;
import org.jspecify.annotations.Nullable;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

class NetConnectionChannelTest {

    private final EchoNetwork network = new EchoNetwork();
    private final EventLoopGroup loop = new MultiThreadIoEventLoopGroup(1, LocalIoHandler.newFactory());

    @AfterEach
    void cleanUp() throws InterruptedException {
        loop.shutdownGracefully(0, 1, TimeUnit.SECONDS).await(5, TimeUnit.SECONDS);
    }

    @Test
    void carriesBytesBothWaysOverTcp() throws Exception {
        CompletableFuture<String> reply = new CompletableFuture<>();
        ChannelFuture connect = bootstrap(reply).connect(InetSocketAddress.createUnresolved("127.0.0.1", 25565));

        assertThat(connect.await(5, TimeUnit.SECONDS)).isTrue();
        assertThat(connect.isSuccess()).isTrue();
        Channel channel = connect.channel();
        assertThat(channel.isActive()).isTrue();
        assertThat(network.opened).containsExactly("tcp 127.0.0.1:25565");

        channel.writeAndFlush(Unpooled.copiedBuffer("ping", StandardCharsets.UTF_8))
                .sync();

        assertThat(reply.get(5, TimeUnit.SECONDS)).isEqualTo("echo:ping");
    }

    @Test
    void aWebSocketAddressOpensAWebSocket() throws Exception {
        CompletableFuture<String> reply = new CompletableFuture<>();
        WebSocketAddress address = new WebSocketAddress(WebSocketUrl.parse("wss://mc.example.com/play"));

        ChannelFuture connect = bootstrap(reply).connect(address);

        assertThat(connect.await(5, TimeUnit.SECONDS)).isTrue();
        assertThat(connect.isSuccess()).isTrue();
        assertThat(network.opened).containsExactly("ws wss://mc.example.com/play");
        assertThat(connect.channel().remoteAddress()).isSameAs(address);
        connect.channel()
                .writeAndFlush(Unpooled.copiedBuffer("hi", StandardCharsets.UTF_8))
                .sync();
        assertThat(reply.get(5, TimeUnit.SECONDS)).isEqualTo("echo:hi");
    }

    @Test
    void aClosedConnectionClosesTheChannel() throws Exception {
        ChannelFuture connect =
                bootstrap(new CompletableFuture<>()).connect(InetSocketAddress.createUnresolved("127.0.0.1", 1));
        assertThat(connect.await(5, TimeUnit.SECONDS)).isTrue();

        network.remoteClose();

        assertThat(connect.channel().closeFuture().await(5, TimeUnit.SECONDS)).isTrue();
        assertThat(connect.channel().isOpen()).isFalse();
    }

    @Test
    void closingTheChannelClosesTheConnection() throws Exception {
        ChannelFuture connect =
                bootstrap(new CompletableFuture<>()).connect(InetSocketAddress.createUnresolved("127.0.0.1", 1));
        assertThat(connect.await(5, TimeUnit.SECONDS)).isTrue();

        connect.channel().close().sync();

        assertThat(network.closedByUs).isTrue();
    }

    @Test
    void aFailedConnectFailsTheConnectFuture() throws Exception {
        network.refuse = true;

        ChannelFuture connect =
                bootstrap(new CompletableFuture<>()).connect(InetSocketAddress.createUnresolved("127.0.0.1", 1));

        assertThat(connect.await(5, TimeUnit.SECONDS)).isTrue();
        assertThat(connect.cause()).isInstanceOf(ConnectException.class);
        assertThat(connect.cause().getCause()).hasMessage("refused");
        assertThat(connect.channel().closeFuture().await(5, TimeUnit.SECONDS)).isTrue();
    }

    @Test
    void aConnectThatNeverAnswersTimesOut() throws Exception {
        network.silent = true;

        ChannelFuture connect = bootstrap(new CompletableFuture<>())
                .option(ChannelOption.CONNECT_TIMEOUT_MILLIS, 200)
                .connect(InetSocketAddress.createUnresolved("127.0.0.1", 1));

        assertThat(connect.await(5, TimeUnit.SECONDS)).isTrue();
        assertThat(connect.cause()).isInstanceOf(ConnectTimeoutException.class);
        assertThat(connect.channel().closeFuture().await(5, TimeUnit.SECONDS)).isTrue();
        assertThat(network.closedByUs).isTrue();
    }

    @Test
    void aConnectThatOpensInTimeStaysOpen() throws Exception {
        ChannelFuture connect = bootstrap(new CompletableFuture<>())
                .option(ChannelOption.CONNECT_TIMEOUT_MILLIS, 200)
                .connect(InetSocketAddress.createUnresolved("127.0.0.1", 1));
        assertThat(connect.await(5, TimeUnit.SECONDS)).isTrue();
        assertThat(connect.isSuccess()).isTrue();

        Thread.sleep(400);

        assertThat(connect.channel().isActive()).isTrue();
    }

    private Bootstrap bootstrap(CompletableFuture<String> reply) {
        return new Bootstrap()
                .group(loop)
                .channelFactory(() -> new TestChannel(network))
                .handler(new ChannelInitializer<Channel>() {
                    @Override
                    protected void initChannel(Channel channel) {
                        channel.pipeline().addLast(new ChannelInboundHandlerAdapter() {
                            @Override
                            public void channelRead(ChannelHandlerContext ctx, Object message) {
                                ByteBuf data = (ByteBuf) message;
                                reply.complete(data.toString(StandardCharsets.UTF_8));
                                data.release();
                            }
                        });
                    }
                });
    }

    private static final class TestChannel extends NetConnectionChannel {
        private final PlatformNetwork network;

        TestChannel(PlatformNetwork network) {
            this.network = network;
        }

        @Override
        protected NetConnection open(InetSocketAddress remote, NetConnection.Listener listener) {
            return openOn(network, remote, listener);
        }
    }

    /** Answers every message with "echo:" and the message, from another thread, like a real transport. */
    private static final class EchoNetwork implements PlatformNetwork {
        final List<String> opened = new CopyOnWriteArrayList<>();
        volatile boolean refuse;
        /** Neither opens nor fails, like a server behind a firewall that drops packets. */
        volatile boolean silent;

        volatile boolean closedByUs;
        private volatile NetConnection.@Nullable Listener listener;

        @Override
        public NetConnection connect(String host, int port, NetConnection.Listener listener) {
            opened.add("tcp " + host + ":" + port);
            return start(listener);
        }

        @Override
        public NetConnection openWebSocket(String url, NetConnection.Listener listener) {
            opened.add("ws " + url);
            return start(listener);
        }

        private NetConnection start(NetConnection.Listener listener) {
            this.listener = listener;
            Thread.ofVirtual().start(() -> {
                if (silent) {
                    return;
                }
                if (refuse) {
                    listener.closed(new ConnectException("refused"));
                } else {
                    listener.connected();
                }
            });
            return new NetConnection() {
                @Override
                public void send(ByteBuffer data) {
                    String text = StandardCharsets.UTF_8.decode(data).toString();
                    Thread.ofVirtual()
                            .start(() -> listener.received(
                                    ByteBuffer.wrap(("echo:" + text).getBytes(StandardCharsets.UTF_8))));
                }

                @Override
                public void close() {
                    closedByUs = true;
                }

                @Override
                public boolean isOpen() {
                    return true;
                }

                @Override
                public String remoteDescription() {
                    return "test";
                }
            };
        }

        void remoteClose() {
            NetConnection.Listener current = listener;
            if (current != null) {
                current.closed(null);
            }
        }
    }
}
