package incari.tools.onlinedev;

import incari.online.FakeService;
import incari.platform.api.net.NetConnection;
import io.netty.bootstrap.ServerBootstrap;
import io.netty.channel.Channel;
import io.netty.channel.ChannelHandlerContext;
import io.netty.channel.ChannelInitializer;
import io.netty.channel.EventLoopGroup;
import io.netty.channel.MultiThreadIoEventLoopGroup;
import io.netty.channel.SimpleChannelInboundHandler;
import io.netty.channel.nio.NioIoHandler;
import io.netty.channel.socket.SocketChannel;
import io.netty.channel.socket.nio.NioServerSocketChannel;
import io.netty.handler.codec.http.HttpObjectAggregator;
import io.netty.handler.codec.http.HttpServerCodec;
import io.netty.handler.codec.http.websocketx.TextWebSocketFrame;
import io.netty.handler.codec.http.websocketx.WebSocketServerProtocolHandler;
import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.List;
import org.jspecify.annotations.Nullable;

/**
 * Serves {@code :online}'s fake service on {@code ws://localhost:<port>/v1/ws}, one Netty thread for
 * every connection (the fake is single-threaded). Accounts, sessions and presence live in memory and
 * are gone when it stops. Sign-up's proof of work asks for {@link FakeService#BITS} bits.
 * {@code --release <version>} names a latest release in every {@code hello} answer, and each
 * {@code --note <change>} a line of its changelog.
 */
// Writes are fire-and-forget: a failed one closes its channel.
@SuppressWarnings("FutureReturnValueIgnored")
public final class Main {

    private Main() {}

    public static void main(String[] args) throws InterruptedException {
        int port = 8090;
        String release = null;
        List<String> notes = new ArrayList<>();
        for (int i = 0; i + 1 < args.length; i++) {
            if (args[i].equals("--listen")) {
                port = Integer.parseInt(args[i + 1]);
            } else if (args[i].equals("--release")) {
                // The latest release hello names, for the title screen's update notice.
                release = args[i + 1];
            } else if (args[i].equals("--note")) {
                notes.add(args[i + 1]);
            }
        }
        FakeService service = new FakeService();
        service.announceRelease(release, notes.toArray(String[]::new));
        // One thread: the fake is used by one thread at a time.
        EventLoopGroup group = new MultiThreadIoEventLoopGroup(1, NioIoHandler.newFactory());
        Channel server = new ServerBootstrap()
                .group(group)
                .channel(NioServerSocketChannel.class)
                .childHandler(new ChannelInitializer<SocketChannel>() {
                    @Override
                    protected void initChannel(SocketChannel channel) {
                        channel.pipeline()
                                .addLast(new HttpServerCodec())
                                .addLast(new HttpObjectAggregator(16 * 1024))
                                .addLast(new WebSocketServerProtocolHandler("/v1/ws", null, false, 16 * 1024))
                                .addLast(new Client(service));
                    }
                })
                .bind("127.0.0.1", port)
                .sync()
                .channel();
        System.out.println(
                "Incari online dev service on ws://127.0.0.1:" + port + "/v1/ws (memory only; Ctrl+C stops)");
        server.closeFuture().sync();
    }

    /** One client's WebSocket, handed to the fake as if it had opened it. */
    private static final class Client extends SimpleChannelInboundHandler<TextWebSocketFrame> {

        private final FakeService service;
        private @Nullable NetConnection socket;

        Client(FakeService service) {
            this.service = service;
        }

        @Override
        public void userEventTriggered(ChannelHandlerContext ctx, Object event) {
            if (!(event instanceof WebSocketServerProtocolHandler.HandshakeComplete)) {
                ctx.fireUserEventTriggered(event);
                return;
            }
            Channel channel = ctx.channel();
            System.out.println("Connected: " + channel.remoteAddress());
            socket = service.openWebSocket("ws://local", new NetConnection.Listener() {
                @Override
                public void connected() {}

                @Override
                public void received(ByteBuffer data) {}

                @Override
                public void receivedText(String text) {
                    channel.writeAndFlush(new TextWebSocketFrame(text));
                }

                @Override
                public void closed(@Nullable Throwable cause) {
                    channel.close();
                }
            });
        }

        @Override
        protected void channelRead0(ChannelHandlerContext ctx, TextWebSocketFrame frame) {
            NetConnection open = socket;
            if (open != null) {
                String text = frame.text();
                // The type only: never the message, which may hold a password.
                System.out.println("<- " + text.replaceAll("^\\{\"t\":\"([a-z]+)\".*", "$1"));
                open.sendText(text);
            }
        }

        @Override
        public void channelInactive(ChannelHandlerContext ctx) {
            NetConnection open = socket;
            if (open != null) {
                service.disconnected(open);
            }
            System.out.println("Disconnected: " + ctx.channel().remoteAddress());
        }
    }
}
