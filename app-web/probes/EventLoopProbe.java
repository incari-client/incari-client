import io.netty.bootstrap.Bootstrap;
import io.netty.bootstrap.ServerBootstrap;
import io.netty.channel.Channel;
import io.netty.channel.ChannelInitializer;
import io.netty.channel.EventLoopGroup;
import io.netty.channel.MultiThreadIoEventLoopGroup;
import io.netty.channel.local.LocalAddress;
import io.netty.channel.local.LocalChannel;
import io.netty.channel.local.LocalIoHandler;
import io.netty.channel.local.LocalServerChannel;

/**
 * Whether Netty's event loop knows its own thread in the browser build: {@code inEventLoop()} from
 * a task on the loop, and a channel connected through a {@code ChannelInitializer}, as singleplayer's
 * server end does. Netty asserts both ({@code DefaultChannelPipeline.invokeHandlerAddedIfNeeded}),
 * so a false there fails every connection with an AssertionError.
 */
public final class EventLoopProbe {

    private EventLoopProbe() {}

    public static void main(String[] args) throws Exception {
        // As app-web's LibraryDefaults sets them.
        System.setProperty("io.netty.allocator.type", "unpooled");
        System.setProperty("io.netty.noUnsafe", "true");
        System.setProperty("io.netty.leakDetection.level", "disabled");
        System.setProperty("io.netty.machineId", "00:00:00:00:00:00:00:01");
        System.setProperty("io.netty.processId", "1");
        boolean assertions = false;
        assert assertions = true;
        System.out.println("assertions enabled: " + assertions);

        EventLoopGroup group = new MultiThreadIoEventLoopGroup(1, (java.util.concurrent.ThreadFactory) r -> new Thread(r, "Probe IO"), LocalIoHandler.newFactory());
        var loop = group.next();
        System.out.println("inEventLoop from main: " + loop.inEventLoop());
        boolean[] inside = new boolean[1];
        loop.submit(() -> inside[0] = loop.inEventLoop()).sync();
        System.out.println("inEventLoop from a task: " + inside[0]);

        LocalAddress address = new LocalAddress("probe");
        new ServerBootstrap()
                .group(group)
                .channel(LocalServerChannel.class)
                .childHandler(new ChannelInitializer<Channel>() {
                    @Override
                    protected void initChannel(Channel channel) {}
                })
                .bind(address)
                .sync();
        try {
            Channel channel = new Bootstrap()
                    .group(group)
                    .channel(LocalChannel.class)
                    .handler(new ChannelInitializer<Channel>() {
                        @Override
                        protected void initChannel(Channel channel) {
                            System.out.println("initChannel ran, inEventLoop " + channel.eventLoop().inEventLoop());
                        }
                    })
                    .connect(address)
                    .sync()
                    .channel();
            System.out.println("connected: " + channel.isActive());
            System.out.println("probe passed");
        } catch (Throwable e) {
            System.out.println("probe failed: " + e);
        }
    }
}
