package incari.client.singleplayer;

import com.mojang.authlib.GameProfile;
import com.mojang.authlib.services.MinecraftServicesDiscoveryService;
import com.mojang.logging.LogUtils;
import com.mojang.serialization.Dynamic;
import incari.client.net.PlatformEventLoopGroupHolder;
import incari.client.net.PlatformSocketChannel;
import incari.client.platform.ClientPlatform;
import incari.platform.api.storage.PlatformStorage;
import incari.platform.api.threads.PlatformChannel;
import io.netty.bootstrap.Bootstrap;
import io.netty.channel.Channel;
import io.netty.channel.ChannelInitializer;
import io.netty.channel.ChannelPipeline;
import java.net.Proxy;
import java.nio.file.Path;
import java.util.Optional;
import java.util.concurrent.atomic.AtomicReference;
import net.minecraft.CrashReport;
import net.minecraft.SharedConstants;
import net.minecraft.commands.Commands;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.Registries;
import net.minecraft.nbt.NbtUtils;
import net.minecraft.network.Connection;
import net.minecraft.network.protocol.PacketFlow;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.Services;
import net.minecraft.server.WorldLoader;
import net.minecraft.server.WorldStem;
import net.minecraft.server.level.progress.LevelLoadListener;
import net.minecraft.server.level.progress.LoggingLevelLoadListener;
import net.minecraft.server.network.ServerHandshakePacketListenerImpl;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.permissions.LevelBasedPermissionSet;
import net.minecraft.util.Util;
import net.minecraft.util.datafix.DataFixTypes;
import net.minecraft.util.datafix.DataFixers;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.dimension.LevelStem;
import net.minecraft.world.level.storage.LevelDataAndDimensions;
import net.minecraft.world.level.storage.LevelResource;
import net.minecraft.world.level.storage.LevelStorageSource;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;

/**
 * Singleplayer's server, in a worker started by the page (ADR 0039): takes the world the page hands
 * it, loads it as the game does, runs a {@link WorkerServer} on it, and answers the page until told
 * to stop, when it saves and hands the world back. The game's connection runs over the channel as
 * packets ({@link ChannelLink}); the world's file changes go to the page's store as they're made.
 *
 * <p>Worker-side: runs in a copy of the game with no client, no window and no {@code Minecraft}.
 */
public final class WorldWorker {

    private static final Logger LOGGER = LogUtils.getLogger();
    /** How often the world's file changes go to the page's store. */
    private static final long CHANGES_MILLIS = 250;
    /** How often the worker looks at whether the server has started or stopped. */
    private static final long POLL_MILLIS = 20;

    private final PlatformChannel page;
    private final PlatformStorage storage;
    private final AtomicReference<@Nullable WorkerServer> server = new AtomicReference<>();
    private volatile @Nullable ChannelLink link;

    /** A shared world's guests (ADR 0062), by the page's number for each. */
    private final java.util.Map<Integer, GuestLink> guests = new java.util.concurrent.ConcurrentHashMap<>();

    private volatile boolean stopRequested;

    private WorldWorker(PlatformChannel page, PlatformStorage storage) {
        this.page = page;
        this.storage = storage;
    }

    /** The worker's entry point, once the platform is installed; returns when the world is handed back. */
    public static void run() {
        PlatformChannel page = ClientPlatform.get().threads().parentChannel();
        if (page == null) {
            throw new IllegalStateException("Not a worker started by the page: no channel to it");
        }
        new WorldWorker(page, ClientPlatform.get().storage()).serve();
    }

    private void serve() {
        warmUp();
        // Its terrain helpers start the game too, meanwhile (ADR 0042).
        incari.client.world.RemoteTerrain.prepare();
        PlatformChannel.Message first = page.receive();
        if (first == null || first.kind() != WorldMessages.START) {
            LOGGER.error("The page didn't start a world; stopping");
            page.close();
            return;
        }
        WorldMessages.Start start = WorldMessages.Start.decode(first.bytes());
        try {
            storage.takeOver(start.files());
            WorkerServer started = startServer(start);
            server.set(started);
            Thread reader = new Thread(this::readPage, "Page reader");
            reader.setDaemon(true);
            reader.start();
            awaitReady(started);
            if (started.isRunning()) {
                connect(started);
                LOGGER.info("Singleplayer's server is running; the page may connect");
                boolean hasIcon = started.getWorldScreenshotFile()
                        .map(java.nio.file.Files::isRegularFile)
                        .orElse(false);
                page.send(WorldMessages.READY, new byte[] {(byte) (hasIcon ? 1 : 0)}, 0, 1);
                started.execute(() -> sendHostState(started));
            }
            while (!started.isShutdown()) {
                sendChanges();
                sleep(stopRequested ? POLL_MILLIS : CHANGES_MILLIS);
            }
            stop(started);
        } catch (Exception | LinkageError e) {
            LOGGER.error("Singleplayer's server couldn't run", e);
            fail(e.toString());
            // The world goes back as it is, so the page's world list keeps it.
            handBack(Path.of(storage.gameDirectory(), "saves", start.levelId()));
        }
    }

    /**
     * What any world needs, done before the page sends one (ADR 0041): the page may start this
     * worker ahead of need ({@code WorkerWorlds.prepare}), while the player picks or makes a world.
     */
    private static void warmUp() {
        SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        net.minecraft.server.Bootstrap.validate();
        DataFixers.getDataFixer();
    }

    /** Loads the world's data and packs as the game does for a world it opens, and starts the server on it. */
    private WorkerServer startServer(WorldMessages.Start start) throws Exception {
        Path gameDirectory = Path.of(storage.gameDirectory());
        LevelStorageSource levels = LevelStorageSource.createDefault(gameDirectory.resolve("saves"));
        LevelStorageSource.LevelStorageAccess access = levels.validateAndCreateAccess(start.levelId());
        Dynamic<?> unfixed = access.getUnfixedDataTag(false);
        Dynamic<?> levelData = DataFixTypes.LEVEL.updateToCurrentVersion(
                DataFixers.getDataFixer(), unfixed, NbtUtils.getDataVersion(unfixed));
        PackRepository packs = ServerPacksSource.createPackRepository(access);
        WorldLoader.PackConfig packConfig = LevelStorageSource.getPackConfig(levelData, packs, false);
        // The terrain helpers load the same packs while the server loads its own (ADR 0042).
        incari.client.world.RemoteTerrain.loadData(packConfig.initialDataConfig());
        WorldLoader.InitConfig config = new WorldLoader.InitConfig(
                packConfig, Commands.CommandSelection.INTEGRATED, LevelBasedPermissionSet.GAMEMASTER);
        WorldStem stem = WorldLoader.load(
                        config,
                        context -> {
                            Registry<LevelStem> dimensions =
                                    context.datapackDimensions().lookupOrThrow(Registries.LEVEL_STEM);
                            LevelDataAndDimensions data = LevelStorageSource.getLevelDataAndDimensions(
                                    access,
                                    levelData,
                                    context.dataConfiguration(),
                                    dimensions,
                                    context.datapackWorldRegistries());
                            return new WorldLoader.DataLoadOutput<>(
                                    data.worldDataAndGenSettings(),
                                    data.dimensions().dimensionsRegistryAccess());
                        },
                        WorldStem::new,
                        Util.backgroundExecutor(),
                        Runnable::run)
                .get();
        LevelLoadListener progress =
                LevelLoadListener.compose(new ProgressToPage(), LoggingLevelLoadListener.forSingleplayer());
        GameProfile owner = new GameProfile(start.hostId(), start.hostName());
        Services services = Services.create(
                MinecraftServicesDiscoveryService.create(Proxy.NO_PROXY, false), gameDirectory.toFile());
        return MinecraftServer.spin(thread -> new WorkerServer(
                thread,
                access,
                packs,
                stem,
                Optional.empty(),
                DataFixers.getDataFixer(),
                services,
                progress,
                owner,
                gameDirectory,
                start.viewDistance(),
                start.simulationDistance(),
                this::crashed));
    }

    private void awaitReady(WorkerServer started) {
        long since = System.currentTimeMillis();
        long told = since;
        while (!started.isReady() && started.isRunning()) {
            sendChanges();
            sleep(POLL_MILLIS);
            long now = System.currentTimeMillis();
            if (now - told >= 10_000) {
                told = now;
                LOGGER.info("Singleplayer's server is still starting ({} s)", (now - since) / 1000);
            }
        }
    }

    /**
     * The server's end of the game's connection: the server-side pipeline the game builds for a
     * TCP client, framing included, over the channel, a flush per message (ADR 0041).
     */
    @SuppressWarnings("FutureReturnValueIgnored") // A failure is logged by the listener.
    private void connect(WorkerServer started) {
        ChannelLink serverEnd = new ChannelLink(page, WorldMessages.PACKET_TO_CLIENT, WorldMessages.SERVER_CLOSED);
        Connection connection = new Connection(PacketFlow.SERVERBOUND);
        connection.setListenerForServerboundHandshake(new ServerHandshakePacketListenerImpl(started, connection));
        started.getConnection().getConnections().add(connection);
        link = serverEnd;
        new Bootstrap()
                .group(PlatformEventLoopGroupHolder.INSTANCE.eventLoopGroup())
                .channelFactory(PlatformSocketChannel::new)
                .handler(new ChannelInitializer<Channel>() {
                    @Override
                    protected void initChannel(Channel channel) {
                        ChannelPipeline pipeline = channel.pipeline();
                        Connection.configureSerialization(pipeline, PacketFlow.SERVERBOUND, false, null);
                        connection.configurePacketHandler(pipeline);
                    }
                })
                .connect(serverEnd.address())
                .addListener(future -> {
                    if (!future.isSuccess()) {
                        LOGGER.error("The server's end of the connection didn't open", future.cause());
                    }
                })
                // Open before the page is told to connect: what it sends goes straight to the server.
                .syncUninterruptibly();
    }

    /**
     * A shared world's guest (ADR 0062): the server-side pipeline a TCP client gets, with the
     * data channel's framing in place of the stream's ({@code LanFraming}, one packet a message).
     */
    @SuppressWarnings("FutureReturnValueIgnored") // A failure is logged by the listener.
    private void connectGuest(WorkerServer started, int number) {
        GuestLink guestEnd = new GuestLink(page, number);
        guests.put(number, guestEnd);
        Connection connection = new Connection(PacketFlow.SERVERBOUND);
        connection.setListenerForServerboundHandshake(new ServerHandshakePacketListenerImpl(started, connection));
        started.getConnection().getConnections().add(connection);
        new Bootstrap()
                .group(PlatformEventLoopGroupHolder.INSTANCE.eventLoopGroup())
                .channelFactory(PlatformSocketChannel::new)
                .handler(new ChannelInitializer<Channel>() {
                    @Override
                    protected void initChannel(Channel channel) {
                        ChannelPipeline pipeline = channel.pipeline();
                        Connection.configureSerialization(pipeline, PacketFlow.SERVERBOUND, false, null);
                        incari.net.MessageFraming.install(pipeline, "splitter", "prepender", null);
                        incari.net.relay.LanFraming.install(pipeline);
                        connection.configurePacketHandler(pipeline);
                    }
                })
                .connect(guestEnd.address())
                .addListener(future -> {
                    if (!future.isSuccess()) {
                        LOGGER.error("A guest's connection didn't open", future.cause());
                        guests.remove(number);
                    }
                });
        LOGGER.info("A guest joined the shared world (guest {})", number);
    }

    /** Reads what the page sends until it closes the channel. */
    private void readPage() {
        PlatformChannel.Message message;
        while ((message = page.receive()) != null) {
            WorkerServer running = server.get();
            switch (message.kind()) {
                case WorldMessages.PACKET_TO_SERVER -> {
                    ChannelLink serverEnd = link;
                    if (serverEnd != null) {
                        serverEnd.deliver(message.bytes());
                    }
                }
                case WorldMessages.PAUSED -> {
                    if (running != null) {
                        running.setClientPaused(message.bytes().length > 0 && message.bytes()[0] != 0);
                    }
                }
                case WorldMessages.SAVE -> {
                    if (running != null) {
                        // As the integrated server does when the game pauses; the files go to the
                        // storage worker straight from here (ADR 0041), so the page needn't run.
                        running.execute(() -> running.saveEverything(true, false, false));
                    }
                }
                case WorldMessages.DISTANCES -> {
                    if (running != null) {
                        running.setDistances(
                                WorldMessages.intAt(message.bytes(), 0), WorldMessages.intAt(message.bytes(), 1));
                    }
                }
                case WorldMessages.CLIENT_CLOSED -> {
                    ChannelLink serverEnd = link;
                    if (serverEnd != null) {
                        serverEnd.closedByOtherEnd();
                    }
                }
                case WorldMessages.HOST_CHANGE -> {
                    if (running != null) {
                        int setting = WorldMessages.intAt(message.bytes(), 0);
                        int value = WorldMessages.intAt(message.bytes(), 1);
                        running.execute(() -> {
                            changeHost(running, setting, value);
                            sendHostState(running);
                        });
                    }
                }
                case WorldMessages.SHARED -> {
                    if (running != null) {
                        boolean shared = message.bytes().length > 0 && message.bytes()[0] != 0;
                        running.execute(() -> running.setShared(shared));
                    }
                }
                case WorldMessages.GUEST_OPEN -> {
                    if (running != null) {
                        connectGuest(running, WorldMessages.intAt(message.bytes(), 0));
                    }
                }
                case WorldMessages.GUEST_MESSAGE -> {
                    GuestLink guest = guests.get(WorldMessages.intAt(message.bytes(), 0));
                    if (guest != null) {
                        guest.deliver(java.util.Arrays.copyOfRange(message.bytes(), 4, message.bytes().length));
                    }
                }
                case WorldMessages.GUEST_CLOSED -> {
                    GuestLink guest = guests.remove(WorldMessages.intAt(message.bytes(), 0));
                    if (guest != null) {
                        guest.closedByOtherEnd();
                    }
                }
                case WorldMessages.ICON -> {
                    if (running != null) {
                        writeIcon(running, message.bytes());
                    }
                }
                case WorldMessages.STOP -> {
                    stopRequested = true;
                    if (running != null) {
                        running.halt(false);
                    }
                }
                default -> LOGGER.warn("A message from the page this worker doesn't know: {}", message.kind());
            }
        }
        // The page went away: stop without anyone to hand the world to (its changes are stored).
        WorkerServer running = server.get();
        if (running != null && running.isRunning()) {
            stopRequested = true;
            running.halt(false);
        }
    }

    /** A World Options change, on the server's thread. */
    private static void changeHost(WorkerServer running, int setting, int value) {
        switch (setting) {
            case WorldMessages.HOST_ALLOW_COMMANDS -> running.setWorldAllowCommands(value != 0);
            case WorldMessages.HOST_WORLD_GAME_MODE -> running.setWorldGameType(GameType.byId(value));
            case WorldMessages.HOST_PERSONAL_GAME_MODE -> running.setOwnerGameType(GameType.byId(value));
            case WorldMessages.HOST_FORCE_GAME_MODE -> running.setForceGameMode(value != 0);
            case WorldMessages.HOST_GUEST_COMMANDS -> running.setGuestCommandAccess(value != 0);
            default -> LOGGER.warn("A host setting this worker doesn't know: {}", setting);
        }
    }

    /** The host settings as World Options shows them, for the page. */
    private void sendHostState(WorkerServer running) {
        byte[] state = WorldMessages.ints(
                running.getWorldData().getGameType().getId(),
                running.getWorldData().isAllowCommands() ? 1 : 0,
                running.forceGameMode() ? 1 : 0,
                running.guestCommandAccess() ? 1 : 0);
        page.send(WorldMessages.HOST_STATE, state, 0, state.length);
    }

    /** Keeps the icon the page took in the world, where the world list shows it. */
    private static void writeIcon(WorkerServer running, byte[] png) {
        running.getWorldScreenshotFile().ifPresent(path -> {
            try {
                java.nio.file.Files.write(path, png);
            } catch (java.io.IOException e) {
                LOGGER.warn("Couldn't keep the world's icon", e);
            }
        });
    }

    /** Sends the world's file changes so far to the page's store. */
    private void sendChanges() {
        byte[] changes = storage.takeChanges();
        if (changes != null) {
            page.send(WorldMessages.FILE_CHANGES, changes, 0, changes.length);
        }
    }

    /** The server has stopped and saved: the last changes, then the world itself, go back to the page. */
    private void stop(WorkerServer stopped) {
        ChannelLink serverEnd = link;
        if (serverEnd != null) {
            serverEnd.close();
        }
        handBack(stopped.getWorldPath(LevelResource.ROOT));
    }

    /** The world's last changes, then its files, go back to the page, and the channel closes. */
    private void handBack(Path world) {
        sendChanges();
        byte[] files = storage.handOver(world.toAbsolutePath().normalize().toString());
        if (files == null) {
            files = new byte[0];
        }
        page.send(WorldMessages.STOPPED, files, 0, files.length);
        page.close();
    }

    private void crashed(CrashReport report) {
        LOGGER.error("Singleplayer's server crashed:\n{}", report.getFriendlyReport(net.minecraft.ReportType.CRASH));
        fail(report.getTitle() + ": " + report.getException());
    }

    private void fail(String message) {
        byte[] bytes = message.getBytes(java.nio.charset.StandardCharsets.UTF_8);
        page.send(WorldMessages.FAILED, bytes, 0, bytes.length);
    }

    private static void sleep(long millis) {
        try {
            Thread.sleep(millis);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }

    /** The world's load as the page's loading screen shows it. */
    private final class ProgressToPage implements LevelLoadListener {

        @Override
        public void start(LevelLoadListener.Stage stage, int totalChunks) {
            send(stage, 0, totalChunks);
        }

        @Override
        public void update(LevelLoadListener.Stage stage, int currentChunks, int totalChunks) {
            send(stage, currentChunks, totalChunks);
        }

        @Override
        public void finish(LevelLoadListener.Stage stage) {
            send(stage, -1, -1);
        }

        @Override
        public void updateFocus(ResourceKey<Level> dimension, ChunkPos chunkPos) {}

        private void send(LevelLoadListener.Stage stage, int done, int total) {
            byte[] message = new WorldMessages.Progress(stage.name(), done, total).encode();
            page.send(WorldMessages.LOAD_PROGRESS, message, 0, message.length);
        }
    }
}
