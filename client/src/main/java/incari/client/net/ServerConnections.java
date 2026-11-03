package incari.client.net;

import com.mojang.logging.LogUtils;
import incari.client.eagler.EaglerAuth;
import incari.client.eagler.EaglerCookies;
import incari.client.eagler.EaglerJoin;
import incari.client.eagler.EaglerVoice;
import incari.client.skins.OwnSkin;
import incari.net.MessageFraming;
import incari.net.WebSocketAddress;
import incari.net.WebSocketUrl;
import incari.net.eagler.EaglerHandshake;
import incari.net.eagler.EaglerLogin;
import incari.net.eagler.EaglerSession;
import incari.net.eagler.EaglerTextures;
import incari.net.relay.LanFraming;
import incari.net.relay.LanLogin;
import incari.net.relay.RelayWorldAddress;
import incari.net.wisp.WispAddress;
import incari.via.IncariVia;
import incari.via.ViaPipeline;
import io.netty.bootstrap.Bootstrap;
import io.netty.channel.Channel;
import io.netty.channel.ChannelFuture;
import io.netty.channel.ChannelHandlerContext;
import io.netty.channel.ChannelInboundHandlerAdapter;
import io.netty.channel.ChannelInitializer;
import io.netty.channel.ChannelPipeline;
import io.netty.handler.timeout.ReadTimeoutHandler;
import java.io.File;
import java.net.InetSocketAddress;
import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;
import net.minecraft.SharedConstants;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ServerData;
import net.minecraft.client.multiplayer.resolver.ResolvedServerAddress;
import net.minecraft.client.multiplayer.resolver.ServerAddress;
import net.minecraft.client.multiplayer.resolver.ServerNameResolver;
import net.minecraft.network.BandwidthDebugMonitor;
import net.minecraft.network.Connection;
import net.minecraft.network.DisconnectionDetails;
import net.minecraft.network.chat.Component;
import net.minecraft.network.protocol.PacketFlow;
import net.minecraft.network.protocol.game.ClientboundLoginPacket;
import net.minecraft.network.protocol.ping.ClientboundPongResponsePacket;
import net.minecraft.network.protocol.status.ClientStatusPacketListener;
import net.minecraft.network.protocol.status.ClientboundStatusResponsePacket;
import net.minecraft.network.protocol.status.ServerStatus;
import net.minecraft.network.protocol.status.ServerboundStatusRequestPacket;
import net.minecraft.server.network.EventLoopGroupHolder;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;

/**
 * How the game's connection to a server is built (ADR 0016, ADR 0017). The connect screen tells
 * {@link #prepare} which server it's about to join, on its connector thread; the hook in {@code
 * Connection.connect}, called next on the same thread, then builds the pipeline with what that
 * server needs:
 *
 * <ul>
 *   <li>a WebSocket address: one packet per message, and the Eagler handshake at the head;
 *   <li>a server joined through a Wisp proxy ({@link WispServers}): the game's TCP pipeline as it is;
 *   <li>a server of another version: Via after the framing, translating both ways.
 * </ul>
 *
 * Connections nobody prepared (the server list's pings) get the plain pipeline for their address.
 */
public final class ServerConnections {

    private static final Logger LOGGER = LogUtils.getLogger();

    /** The game's names for its framing handlers ({@code Connection.configureSerialization}). */
    private static final String SPLITTER = "splitter";

    private static final String PREPENDER = "prepender";
    /** Shared worlds are Eagler's 1.8 integrated servers. */
    private static final int LAN_PROTOCOL = 47;
    /** The brand UUID shared-world hosts are told: Incari's own. */
    private static final UUID BRAND = UUID.fromString("5b9a0f4c-3a4e-4d0b-9a8f-2f6a1c7e0d11");
    /** Seconds without a packet before the game gives up on a server, as over TCP. */
    private static final int READ_TIMEOUT_SECONDS = 30;
    /** How long a version check before joining waits for a TCP server's status. */
    private static final long PROBE_TIMEOUT_SECONDS = 5;

    /** What the connect screen worked out for a connection it's about to open. */
    private record Plan(int serverProtocol) {}

    /** A join the connect screen prepared: the list's entry and its address. */
    record Join(ServerData server, ServerAddress address) {}

    /**
     * The plan for the connection this thread opens next. The connect screen prepares before it
     * takes its lock (preparing may wait for a ping), then opens the connection under it.
     */
    private static final ThreadLocal<@Nullable Plan> NEXT = new ThreadLocal<>();

    /** The protocol of the server the connect screen last joined, for tests and diagnostics. */
    private static volatile int lastJoined = -1;
    /** The server the connect screen last prepared a join to. */
    private static volatile @Nullable Join lastJoin;
    /** Whether the last connection opened was to a shared world, which {@link #lastJoin} isn't. */
    private static volatile boolean lastWasRelay;
    /** The Eagler stage of the connect screen's last connection, if it was an Eagler one. */
    private static volatile @Nullable EaglerHandshake lastEagler;

    private ServerConnections() {}

    /**
     * {@code ConnectScreen}, before {@code Connection.connect}: works out which version {@code
     * server} speaks (its saved choice, the last ping, or a quick one now) and starts Via if it's
     * not the game's. On the connector thread: it may wait for a ping and for Via's data.
     */
    public static void prepare(ServerData server, ServerAddress address) {
        NEXT.remove();
        int retry = VersionRetry.retryProtocol(server);
        int protocol = retry >= 0 ? retry : targetProtocol(server, address);
        VersionRetry.joining(server, address, protocol);
        lastJoin = new Join(server, address);
        lastWasRelay = false;
        if (protocol != gameProtocol()) {
            LOGGER.info("Joining {} as {} through Via", address, ServerVersions.name(protocol));
            IncariVia.configure(new File(Minecraft.getInstance().gameDirectory, "incari/via"), gameProtocol());
            IncariVia.load(protocol);
        }
        NEXT.set(new Plan(protocol));
    }

    /** Whether {@code Connection.connect} should hand this connection to {@link #connect}. */
    public static boolean accepts(InetSocketAddress address) {
        if (address instanceof WebSocketAddress || address instanceof WispAddress) {
            // Neither is a socket the game's own channel classes can open.
            return true;
        }
        Plan plan = NEXT.get();
        if (plan == null) {
            return false;
        }
        if (plan.serverProtocol() == gameProtocol()) {
            // The game's own pipeline will do: the plan is spent here.
            lastJoined = plan.serverProtocol();
            NEXT.remove();
            return false;
        }
        return true;
    }

    /**
     * Opens the connection with the game's client pipeline, plus what {@link #prepare} found it
     * needs: message framing and the Eagler handshake for a WebSocket, Via for another version.
     */
    public static ChannelFuture connect(
            final InetSocketAddress address,
            final EventLoopGroupHolder eventLoops,
            final Connection connection,
            final @Nullable BandwidthDebugMonitor bandwidth) {
        Plan plan = NEXT.get();
        NEXT.remove();
        int protocol = plan != null ? plan.serverProtocol() : gameProtocol();
        if (plan != null) {
            lastJoined = protocol;
        }
        boolean webSocket = address instanceof WebSocketAddress;
        return new Bootstrap()
                .group(eventLoops.eventLoopGroup())
                // A factory, not the channel class: no reflection, which the browser build can't do.
                .channelFactory(PlatformSocketChannel::new)
                .handler(new ChannelInitializer<Channel>() {
                    @Override
                    protected void initChannel(final Channel channel) {
                        ChannelPipeline pipeline =
                                channel.pipeline().addLast("timeout", new ReadTimeoutHandler(READ_TIMEOUT_SECONDS));
                        Connection.configureSerialization(pipeline, PacketFlow.CLIENTBOUND, false, bandwidth);
                        if (webSocket) {
                            MessageFraming.install(
                                    pipeline, SPLITTER, PREPENDER, bandwidth == null ? null : bandwidth::onReceive);
                        }
                        if (protocol != gameProtocol()) {
                            ViaPipeline.install(channel, protocol, SPLITTER, PREPENDER);
                        }
                        EaglerHandshake eagler = null;
                        if (webSocket) {
                            String server = ((WebSocketAddress) address).url().toString();
                            eagler = EaglerHandshake.install(
                                    pipeline,
                                    eaglerLogin(server, protocol, plan != null),
                                    eaglerSession(server, plan != null));
                        }
                        if (plan != null) {
                            lastEagler = eagler;
                        }
                        // The game logs a connection's failure only as its message; this keeps the stack.
                        pipeline.addLast("incari-errors", new JoinWatch(address, plan != null ? protocol : -1));
                        connection.configurePacketHandler(pipeline);
                    }
                })
                .connect(address);
    }

    /**
     * Connects to a world shared through a relay (ADR 0019) over the host's open data channel, which
     * speaks its variant of 1.8: Via translates the game to 1.8, then the host's login and framing.
     * Not on the render thread: it may wait for Via's data.
     */
    public static ChannelFuture connectRelay(
            final RelayWorldAddress world, final EventLoopGroupHolder eventLoops, final Connection connection) {
        lastWasRelay = true;
        if (world.incariProtocol() >= 0) {
            return connectIncariHost(world, eventLoops, connection);
        }
        int protocol = LAN_PROTOCOL;
        IncariVia.configure(new File(Minecraft.getInstance().gameDirectory, "incari/via"), gameProtocol());
        IncariVia.load(protocol);
        lastJoined = protocol;
        OwnSkin own = OwnSkin.load(Minecraft.getInstance().gameDirectory.toPath());
        LanLogin login =
                new LanLogin(EaglerTextures.legacySkinPacket(own.skin()), EaglerTextures.capePacket(own.cape()), BRAND);
        return new Bootstrap()
                .group(eventLoops.eventLoopGroup())
                .channelFactory(PlatformSocketChannel::new)
                .handler(new ChannelInitializer<Channel>() {
                    @Override
                    protected void initChannel(final Channel channel) {
                        ChannelPipeline pipeline =
                                channel.pipeline().addLast("timeout", new ReadTimeoutHandler(READ_TIMEOUT_SECONDS));
                        Connection.configureSerialization(pipeline, PacketFlow.CLIENTBOUND, false, null);
                        MessageFraming.install(pipeline, SPLITTER, PREPENDER, null);
                        ViaPipeline.install(channel, protocol, SPLITTER, PREPENDER);
                        LanFraming.install(pipeline);
                        LanLogin.install(pipeline, login);
                        pipeline.addLast("incari-errors", new JoinWatch(world, -1));
                        connection.configurePacketHandler(pipeline);
                    }
                })
                .connect(world);
    }

    /**
     * A world an Incari player shares (ADR 0062): its host says so first on the channel, and plays
     * today's game, so the client joins it as it is, with nothing between but the data channel's
     * framing.
     */
    private static ChannelFuture connectIncariHost(
            final RelayWorldAddress world, final EventLoopGroupHolder eventLoops, final Connection connection) {
        // A host of another version was turned away before this ({@code RelayConnectScreen}).
        lastJoined = gameProtocol();
        return new Bootstrap()
                .group(eventLoops.eventLoopGroup())
                .channelFactory(PlatformSocketChannel::new)
                .handler(new ChannelInitializer<Channel>() {
                    @Override
                    protected void initChannel(final Channel channel) {
                        ChannelPipeline pipeline =
                                channel.pipeline().addLast("timeout", new ReadTimeoutHandler(READ_TIMEOUT_SECONDS));
                        Connection.configureSerialization(pipeline, PacketFlow.CLIENTBOUND, false, null);
                        MessageFraming.install(pipeline, SPLITTER, PREPENDER, null);
                        LanFraming.install(pipeline);
                        pipeline.addLast("incari-errors", new JoinWatch(world, -1));
                        connection.configurePacketHandler(pipeline);
                    }
                })
                .connect(world);
    }

    /** The join the connect screen last prepared, or null before the first. */
    static @Nullable Join lastJoin() {
        return lastJoin;
    }

    /** {@link Reconnect}: the server to join again, or null when the last connection wasn't to one. */
    static @Nullable Join rejoinable() {
        return lastWasRelay ? null : lastJoin;
    }

    /**
     * Whether the connect screen's last connection was an Eagler one the server dropped during its
     * login without a word ({@link EaglerHandshake#droppedDuringLogin}).
     */
    static boolean lastJoinDroppedDuringLogin() {
        EaglerHandshake stage = lastEagler;
        return stage != null && stage.droppedDuringLogin();
    }

    /** The protocol the connect screen last joined a server with, or -1 before the first join. */
    public static int lastJoinedProtocol() {
        return lastJoined;
    }

    private static int gameProtocol() {
        return SharedConstants.getCurrentVersion().protocolVersion();
    }

    // --- The server's version ----------------------------------------------------------------

    /**
     * The version to join {@code server} with (ADR 0017, ADR 0018): the player's choice; else the
     * version an earlier "Auto" join found by being refused; else what the server says of itself
     * (the server list's ping this session, or a quick one now); else the game's own. A wrong
     * guess costs a refused login, which {@link VersionRetry} reads and joins again from.
     */
    private static int targetProtocol(ServerData server, ServerAddress address) {
        if (server.incariVersion != ServerVersions.AUTO && ServerVersions.isCompatible(server.incariVersion)) {
            return server.incariVersion;
        }
        int learned = VersionRetry.learned(server.ip);
        if (learned >= 0 && ServerVersions.isCompatible(learned)) {
            return learned;
        }
        WebSocketUrl url = address.webSocketUrl();
        ServerData.State state = server.state();
        if ((state == ServerData.State.SUCCESSFUL || state == ServerData.State.INCOMPATIBLE)
                && server.protocol != ServerVersions.UNKNOWN
                && ServerVersions.isCompatible(server.protocol)) {
            // The server list pinged it this session, and it named its version.
            return server.protocol;
        }
        int reported = url != null ? EaglerPings.detectProtocol(url) : probe(address);
        LOGGER.info("{} says it speaks protocol {}", address, reported);
        return reported != ServerVersions.UNKNOWN && ServerVersions.isCompatible(reported) ? reported : gameProtocol();
    }

    /** The protocol a TCP server's status reports, or -1 if it doesn't answer in time. */
    private static int probe(ServerAddress address) {
        Optional<InetSocketAddress> resolved =
                ServerNameResolver.DEFAULT.resolveAddress(address).map(ResolvedServerAddress::asInetSocketAddress);
        if (resolved.isEmpty()) {
            return -1;
        }
        CompletableFuture<Integer> result = new CompletableFuture<>();
        Connection connection;
        try {
            connection = Connection.connectToServer(resolved.get(), EventLoopGroupHolder.remote(false), null);
        } catch (RuntimeException e) {
            LOGGER.warn("Couldn't ask {} for its version: {}", address, e.toString());
            return -1;
        }
        // The connect future can finish before the connection sees its channel go active; until it
        // does, what's sent waits for a tick of the connection, which nobody gives this one.
        long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(PROBE_TIMEOUT_SECONDS);
        while (!connection.isConnected() && System.nanoTime() < deadline) {
            try {
                Thread.sleep(5);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                return -1;
            }
        }
        connection.initiateServerboundStatusConnection(
                address.getHost(), address.getPort(), new ClientStatusPacketListener() {
                    @Override
                    public void handleStatusResponse(ClientboundStatusResponsePacket packet) {
                        result.complete(packet.status()
                                .version()
                                .map(ServerStatus.Version::protocol)
                                .orElse(-1));
                        connection.disconnect(Component.translatable("multiplayer.status.finished"));
                    }

                    @Override
                    public void handlePongResponse(ClientboundPongResponsePacket packet) {}

                    @Override
                    public void onDisconnect(DisconnectionDetails details) {
                        result.complete(-1);
                    }

                    @Override
                    public boolean isAcceptingMessages() {
                        return connection.isConnected();
                    }
                });
        connection.send(ServerboundStatusRequestPacket.INSTANCE);
        try {
            return result.get(PROBE_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return -1;
        } catch (Exception e) {
            LOGGER.warn("{} didn't say its version: {}", address, e.toString());
            return -1;
        } finally {
            if (connection.isConnected()) {
                connection.disconnect(Component.translatable("multiplayer.status.cancelled"));
            }
        }
    }

    // --- Eagler ------------------------------------------------------------------------------

    private static EaglerLogin eaglerLogin(String server, int protocol, boolean joining) {
        Minecraft minecraft = Minecraft.getInstance();
        int capabilities = EaglerLogin.COOKIE | EaglerLogin.REDIRECT | EaglerLogin.PAUSE_MENU;
        if (joining && EaglerVoice.supported()) {
            capabilities |= EaglerLogin.VOICE;
        }
        byte[] cookie =
                joining ? EaglerCookies.get(minecraft.gameDirectory.toPath()).cookie(server) : null;
        return new EaglerLogin(
                minecraft.getUser().getName(),
                protocol,
                "Incari",
                SharedConstants.getCurrentVersion().name(),
                "",
                OwnSkin.load(minecraft.gameDirectory.toPath()).profileData(),
                joining ? EaglerAuth.password(server) : null,
                cookie == null ? null : new EaglerLogin.Cookie(cookie),
                capabilities);
    }

    /** A joining connection's session: skins, voice, cookies, passwords; a ping-like one needs none. */
    private static EaglerSession eaglerSession(String server, boolean joining) {
        if (!joining) {
            return new EaglerSession() {};
        }
        return EaglerJoin.start(server);
    }

    /**
     * The last handler before the game's: logs what fails in the pipeline with its stack (the game
     * logs only the message), and tells {@link VersionRetry} when a join the connect screen
     * prepared reaches the world, so the version it used is remembered.
     */
    private static final class JoinWatch extends ChannelInboundHandlerAdapter {

        private final InetSocketAddress address;
        /** The protocol of a prepared join, or -1 for a connection nobody prepared. */
        private final int protocol;

        JoinWatch(InetSocketAddress address, int protocol) {
            this.address = address;
            this.protocol = protocol;
        }

        @Override
        public void channelRead(ChannelHandlerContext ctx, Object message) {
            if (message instanceof ClientboundLoginPacket && protocol >= 0) {
                VersionRetry.joined(protocol);
            }
            ctx.fireChannelRead(message);
        }

        @Override
        public void exceptionCaught(ChannelHandlerContext ctx, Throwable cause) {
            LOGGER.warn("The connection to {} failed", address, cause);
            ctx.fireExceptionCaught(cause);
        }
    }
}
