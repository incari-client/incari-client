package incari.client.singleplayer;

import com.mojang.blaze3d.platform.SDLEventHandler;
import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.logging.LogUtils;
import incari.client.net.PlatformEventLoopGroupHolder;
import incari.client.net.PlatformSocketChannel;
import incari.client.platform.ClientPlatform;
import incari.platform.api.storage.PlatformStorage;
import incari.platform.api.threads.PlatformChannel;
import io.netty.bootstrap.Bootstrap;
import io.netty.channel.Channel;
import io.netty.channel.ChannelInitializer;
import io.netty.channel.ChannelPipeline;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.time.Instant;
import java.util.List;
import java.util.Optional;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Gui;
import net.minecraft.client.gui.screens.AlertScreen;
import net.minecraft.client.gui.screens.GenericMessageScreen;
import net.minecraft.client.gui.screens.LevelLoadingScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.gui.screens.multiplayer.JoinMultiplayerScreen;
import net.minecraft.client.gui.screens.worldselection.CreateWorldScreen;
import net.minecraft.client.gui.screens.worldselection.SelectWorldScreen;
import net.minecraft.client.multiplayer.ClientHandshakePacketListenerImpl;
import net.minecraft.client.multiplayer.LevelLoadTracker;
import net.minecraft.network.Connection;
import net.minecraft.network.chat.Component;
import net.minecraft.network.protocol.PacketFlow;
import net.minecraft.network.protocol.login.ServerboundHelloPacket;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.WorldStem;
import net.minecraft.server.level.progress.LevelLoadListener;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.players.NameAndId;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.gamerules.GameRules;
import net.minecraft.world.level.storage.LevelResource;
import net.minecraft.world.level.storage.LevelStorageSource;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;

/**
 * The page's side of singleplayer's server in a worker (ADR 0039). Hooks in {@code Minecraft} hand a
 * world being opened to a worker instead of an integrated server ({@link #load}), tell it when the
 * game pauses ({@link #pauses}), and wait for it to save and stop when the player leaves
 * ({@link #stopAndWait}). The world's files go to the worker with it and come back when it stops;
 * the game connects to it as to a remote server, over the channel.
 */
public final class WorkerWorlds {

    private static final Logger LOGGER = LogUtils.getLogger();

    /**
     * Where the game writes a world's icon for a server in a worker: outside the stored folders, and
     * not in the world's, which is the worker's while it runs.
     */
    private static final Path ICON = Path.of("/tmp/incari-world-icon.png");

    private static boolean enabled = true;
    private static @Nullable Session session;
    private static @Nullable Connection connection;
    /**
     * A worker started early (ADR 0041), loading while the player picks or makes a world; null when
     * there's none.
     */
    private static @Nullable PlatformChannel prepared;

    private WorkerWorlds() {}

    /** The entry point's choice: false runs singleplayer's server on the page, as ADR 0038 did. */
    public static void setEnabled(boolean enabled) {
        WorkerWorlds.enabled = enabled;
    }

    /**
     * Hook ({@code Gui.setScreen}): the world list and the create-world screen start a worker early
     * ({@link #prepare}); the title and multiplayer screens drop it to free its heap.
     */
    public static void screenShown(@Nullable Screen screen) {
        if (screen instanceof SelectWorldScreen || screen instanceof CreateWorldScreen) {
            prepare();
        } else if (screen instanceof TitleScreen || screen instanceof JoinMultiplayerScreen) {
            discardPrepared();
        }
    }

    /**
     * Starts singleplayer's server worker now: loading, mounting the client jar and bootstrapping
     * take seconds on the reference device, and this overlaps them with the page opening the world.
     */
    public static void prepare() {
        if (!enabled || prepared != null || running()) {
            return;
        }
        prepared = ClientPlatform.get().threads().startGameWorker("server");
    }

    private static void discardPrepared() {
        PlatformChannel worker = prepared;
        prepared = null;
        if (worker != null) {
            worker.close();
        }
    }

    /** Whether a world's server runs in a worker now. */
    public static boolean running() {
        Session current = session;
        return current != null && current.ready && !current.ended;
    }

    /** The folder name of the world whose server runs in a worker now, or null: what waypoints are kept by. */
    public static @Nullable String worldFolder() {
        Session current = session;
        Path name = current == null || !current.ready || current.ended ? null : current.world.getFileName();
        return name == null ? null : name.toString();
    }

    /**
     * Hook ({@code Minecraft.doWorldLoad}, first): opens the world in a worker if the platform has
     * one. Returns false to let the game start its integrated server; true when it's handled here,
     * after which {@link #takeConnection} is the game's connection to it, or null if it failed (the
     * player is told).
     */
    public static boolean load(
            Minecraft minecraft,
            LevelStorageSource.LevelStorageAccess access,
            PackRepository packRepository,
            WorldStem worldStem,
            Optional<GameRules> gameRules,
            boolean newWorld) {
        if (!enabled) {
            return false;
        }
        PlatformChannel channel = prepared;
        prepared = null;
        if (channel == null) {
            channel = ClientPlatform.get().threads().startGameWorker("server");
        }
        if (channel == null) {
            return false;
        }
        minecraft.disconnectWithProgressScreen();
        Instant loadStart = Instant.now();
        LevelLoadTracker tracker = new LevelLoadTracker(newWorld ? 500L : 0L);
        LevelLoadingScreen screen = new LevelLoadingScreen(tracker, LevelLoadingScreen.Reason.OTHER);
        minecraft.gui.setScreen(screen);
        PlatformStorage storage = ClientPlatform.get().storage();
        Session started;
        try {
            // Write the world's data now, as the game does before starting its server, and a new
            // world's generation settings and game rules too (its server would save them first):
            // the worker reads the world from its files.
            access.saveDataTag(worldStem.worldDataAndGenSettings().data());
            Path world =
                    access.getLevelPath(LevelResource.ROOT).toAbsolutePath().normalize();
            if (newWorld) {
                LevelStorageSource.writeWorldGenSettings(
                        worldStem.registries().compositeAccess(),
                        world,
                        worldStem.worldDataAndGenSettings().genSettings());
                if (gameRules.isPresent()) {
                    LevelStorageSource.writeGameRules(
                            worldStem.worldDataAndGenSettings().data(), world, gameRules.get());
                }
            }
            String levelId = access.getLevelId();
            worldStem.close();
            // The worker takes the world's lock and files; the page lets go of both.
            access.close();
            byte[] files = storage.handOver(world.toString());
            started = new Session(channel, storage, world, tracker);
            // Kept until the worker is running: if it fails before it has the world, the page
            // takes these back, and the world list keeps the world.
            started.handedOver = files;
            session = started;
            started.startReading();
            byte[] start = new WorldMessages.Start(
                            levelId,
                            minecraft.getUser().getName(),
                            minecraft.getUser().getProfileId(),
                            minecraft.options.renderDistance().get(),
                            minecraft.options.simulationDistance().get(),
                            files == null ? new byte[0] : files)
                    .encode();
            channel.send(WorldMessages.START, start, 0, start.length);
            // A page closed or hidden mid-game saves the world: the page tells the worker itself.
            channel.sendWhenLeaving(WorldMessages.SAVE);
        } catch (IOException | RuntimeException e) {
            LOGGER.error("Couldn't hand the world to singleplayer's server in a worker", e);
            channel.close();
            session = null;
            showFailure(minecraft, e.toString());
            return true;
        }

        while (!started.ready && !started.ended) {
            frame(minecraft, screen);
        }
        if (!started.ready) {
            session = null;
            started.restoreIfLost();
            showFailure(minecraft, started.failure != null ? started.failure : "The world's server stopped");
            return true;
        }

        LOGGER.info("Connecting to singleplayer's server in a worker");
        ChannelLink clientEnd = new ChannelLink(channel, WorldMessages.PACKET_TO_SERVER, WorldMessages.CLIENT_CLOSED);
        started.link = clientEnd;
        Connection client = new Connection(PacketFlow.CLIENTBOUND);
        new Bootstrap()
                .group(PlatformEventLoopGroupHolder.INSTANCE.eventLoopGroup())
                .channelFactory(PlatformSocketChannel::new)
                .handler(new ChannelInitializer<Channel>() {
                    @Override
                    protected void initChannel(Channel netChannel) {
                        ChannelPipeline pipeline = netChannel.pipeline();
                        Connection.configureSerialization(pipeline, PacketFlow.CLIENTBOUND, false, null);
                        client.configurePacketHandler(pipeline);
                    }
                })
                .connect(clientEnd.address())
                .syncUninterruptibly();
        Duration loadTime = Duration.between(loadStart, Instant.now());
        client.initiateServerboundPlayConnection(
                "singleplayer",
                0,
                new ClientHandshakePacketListenerImpl(
                        client, minecraft, null, null, newWorld, loadTime, status -> {}, tracker, null));
        client.send(new ServerboundHelloPacket(
                minecraft.getUser().getName(), minecraft.getUser().getProfileId()));
        connection = client;
        return true;
    }

    /**
     * Hook ({@code GameRenderer.tryTakeScreenshotIfNeeded}): where to take a world's icon when its
     * server runs in a worker and the world has none yet. The page sends it to the worker once
     * the game has written it ({@link #pauses}, every frame).
     */
    public static Optional<Path> iconFile() {
        Session current = session;
        if (current == null || !current.ready || current.ended || !current.wantsIcon || Files.isRegularFile(ICON)) {
            return Optional.empty();
        }
        return Optional.of(ICON);
    }

    /**
     * World Options' host when the world's server runs in a worker (ADR 0039), or null if none
     * does: the host settings the worker last reported, changed by asking it.
     */
    static @Nullable SingleplayerHost host(Minecraft minecraft) {
        Session current = session;
        return current == null || !current.ready || current.ended ? null : new WorkerHost(minecraft, current);
    }

    /**
     * Hook ({@code WorldOptionsScreen.publish}): the chat's line for a world just shared, in place
     * of the LAN port's, when a worker's world is shared through a relay (ADR 0062); null else.
     */
    public static net.minecraft.network.chat.@Nullable Component publishedMessage() {
        Session current = session;
        return current != null && current.sharing != null
                ? net.minecraft.network.chat.Component.literal("Sharing the world...")
                : null;
    }

    /** A relay to share the next world on as soon as it's open (the page's {@code ?share=}); null for none. */
    private static volatile @Nullable String shareOn;

    /** The page's {@code ?share=<relay>} (ADR 0062's check): shares the next world on that relay once open. */
    public static void shareWhenOpen(String relay) {
        shareOn = relay;
    }

    /** The connection {@link #load} made, once; null if it made none. */
    public static @Nullable Connection takeConnection() {
        Connection made = connection;
        connection = null;
        return made;
    }

    /**
     * Hook ({@code Minecraft.runTick}, where the game decides whether it's paused): whether a world
     * in a worker pauses now, which it does as an integrated server's would. Tells the worker when
     * that changes, and when the player's view or simulation distance does. Every frame.
     */
    public static boolean pauses(Minecraft minecraft) {
        Session current = session;
        if (current == null || !current.ready || current.ended) {
            return false;
        }
        sendIconIfTaken(current);
        String relay = shareOn;
        if (relay != null && current.sharing == null && minecraft.player != null) {
            shareOn = null;
            current.sharing = WorldSharing.start(
                    current.channel, current.world.getFileName().toString(), List.of(relay));
        }
        WorldSharing shared = current.sharing;
        if (shared != null) {
            shared.tick();
        }
        // A shared world plays on under the owner's menus, as a LAN world does (ADR 0062).
        boolean paused = minecraft.gui.isPausing() && shared == null;
        if (paused != current.sentPaused) {
            current.sentPaused = paused;
            current.channel.send(WorldMessages.PAUSED, new byte[] {(byte) (paused ? 1 : 0)}, 0, 1);
        }
        int view = minecraft.options.renderDistance().get();
        int simulation = minecraft.options.simulationDistance().get();
        if (view != current.sentView || simulation != current.sentSimulation) {
            current.sentView = view;
            current.sentSimulation = simulation;
            byte[] distances = WorldMessages.ints(view, simulation);
            current.channel.send(WorldMessages.DISTANCES, distances, 0, distances.length);
        }
        return paused;
    }

    /**
     * Hook ({@code Minecraft.disconnect}, after the game's own wait for its integrated server): the
     * world's server in a worker saves and stops, and its files come back, while the "Saving world"
     * screen shows.
     */
    public static void stopAndWait(Minecraft minecraft) {
        Session current = session;
        if (current == null) {
            return;
        }
        session = null;
        if (!current.ended) {
            current.channel.send(WorldMessages.STOP);
            Screen saving = new GenericMessageScreen(Gui.SAVING_LEVEL);
            minecraft.gui.setScreen(saving);
            while (!current.ended) {
                frame(minecraft, saving);
            }
        }
        if (!current.handedBack) {
            LOGGER.warn(
                    "Singleplayer's server ended without handing {} back; its last saved state is in storage, "
                            + "and comes back when the page loads again",
                    current.world);
        }
    }

    /** The world's icon, once the game has written it, goes to the worker, which keeps it in the world. */
    private static void sendIconIfTaken(Session current) {
        if (!current.wantsIcon || !Files.isRegularFile(ICON)) {
            return;
        }
        try {
            byte[] png = Files.readAllBytes(ICON);
            Files.delete(ICON);
            if (png.length > 0) {
                current.wantsIcon = false;
                current.channel.send(WorldMessages.ICON, png, 0, png.length);
            }
        } catch (IOException e) {
            LOGGER.warn("Couldn't send the world's icon to its server", e);
            current.wantsIcon = false;
        }
    }

    /** One frame of a wait, as the game draws them while it waits for its integrated server. */
    private static void frame(Minecraft minecraft, Screen screen) {
        SDLEventHandler events = SDLEventHandler.current();
        if (events != null) {
            RenderSystem.pumpEvents(events);
        }
        screen.tick();
        if (minecraft.gui.overlay() != null) {
            minecraft.gui.overlay().tick();
        }
        minecraft.renderFrame(false);
        minecraft.managedBlock(() -> minecraft.getPendingTasksCount() == 0);
    }

    private static void showFailure(Minecraft minecraft, String message) {
        minecraft.gui.setScreen(new AlertScreen(
                () -> minecraft.gui.setScreen(new TitleScreen()),
                Component.literal("Couldn't open the world"),
                Component.literal(message)));
    }

    /** A world running in a worker, as the page sees it. */
    private static final class Session {
        final PlatformChannel channel;
        final PlatformStorage storage;
        final Path world;
        final LevelLoadTracker tracker;
        volatile boolean ready;
        volatile boolean ended;
        volatile boolean handedBack;
        volatile @Nullable String failure;
        volatile @Nullable ChannelLink link;
        /** The world shared through a relay (ADR 0062), while it is. */
        @Nullable
        WorldSharing sharing;

        byte @Nullable [] handedOver;
        /** The world has no icon yet: the page takes one for it. */
        volatile boolean wantsIcon;
        // The host settings as the worker last reported them (World Options).
        volatile GameType worldGameMode = GameType.SURVIVAL;
        volatile boolean allowCommands;
        volatile boolean forceGameMode = true;
        volatile boolean guestCommands;

        boolean sentPaused;
        int sentView = -1;
        int sentSimulation = -1;

        Session(PlatformChannel channel, PlatformStorage storage, Path world, LevelLoadTracker tracker) {
            this.channel = channel;
            this.storage = storage;
            this.world = world;
            this.tracker = tracker;
        }

        void startReading() {
            Thread reader = new Thread(this::read, "Singleplayer server reader");
            reader.setDaemon(true);
            reader.start();
        }

        /** Reads what the worker sends until it ends. */
        private void read() {
            PlatformChannel.Message message;
            while ((message = channel.receive()) != null) {
                byte[] bytes = message.bytes();
                switch (message.kind()) {
                    case WorldMessages.PACKET_TO_CLIENT -> {
                        ChannelLink clientEnd = link;
                        if (clientEnd != null) {
                            clientEnd.deliver(bytes);
                        }
                    }
                    case WorldMessages.SERVER_CLOSED -> {
                        ChannelLink clientEnd = link;
                        if (clientEnd != null) {
                            clientEnd.closedByOtherEnd();
                        }
                    }
                    case WorldMessages.LOAD_PROGRESS -> progress(WorldMessages.Progress.decode(bytes));
                    case WorldMessages.READY -> {
                        handedOver = null;
                        wantsIcon = bytes.length > 0 && bytes[0] == 0;
                        ready = true;
                    }
                    case WorldMessages.FILE_CHANGES -> storage.storeChanges(bytes);
                    case WorldMessages.HOST_STATE -> {
                        worldGameMode = GameType.byId(WorldMessages.intAt(bytes, 0));
                        allowCommands = WorldMessages.intAt(bytes, 1) != 0;
                        forceGameMode = WorldMessages.intAt(bytes, 2) != 0;
                        guestCommands = WorldMessages.intAt(bytes, 3) != 0;
                    }
                    case WorldMessages.STOPPED -> {
                        if (bytes.length > 0) {
                            storage.takeOver(bytes);
                            handedBack = true;
                        }
                    }
                    case WorldMessages.GUEST_SEND, WorldMessages.GUEST_END -> {
                        WorldSharing shared = sharing;
                        if (shared != null) {
                            shared.fromWorker(message.kind(), bytes);
                        }
                    }
                    case WorldMessages.FAILED -> {
                        failure = new String(bytes, StandardCharsets.UTF_8);
                        LOGGER.error("Singleplayer's server in a worker failed: {}", failure);
                    }
                    default -> LOGGER.warn("A message from the worker the page doesn't know: {}", message.kind());
                }
            }
            ended = true;
            WorldSharing shared = sharing;
            sharing = null;
            if (shared != null) {
                shared.end();
            }
            ChannelLink clientEnd = link;
            if (clientEnd != null) {
                clientEnd.closedByOtherEnd();
            }
        }

        /** The worker ended before it had the world and gave nothing back: the page's copy comes back. */
        void restoreIfLost() {
            byte[] files = handedOver;
            handedOver = null;
            if (!handedBack && files != null && files.length > 0) {
                storage.takeOver(files);
                handedBack = true;
            }
        }

        private void progress(WorldMessages.Progress progress) {
            LevelLoadListener.Stage stage;
            try {
                stage = LevelLoadListener.Stage.valueOf(progress.stage());
            } catch (IllegalArgumentException e) {
                return;
            }
            if (progress.done() < 0) {
                tracker.finish(stage);
            } else if (progress.done() == 0) {
                tracker.start(stage, progress.total());
            } else {
                tracker.update(stage, progress.done(), progress.total());
            }
        }
    }

    /** World Options' host for a world in a worker: settings as reported, changes sent there. */
    private static final class WorkerHost implements SingleplayerHost {
        private final Minecraft minecraft;
        private final Session session;

        WorkerHost(Minecraft minecraft, Session session) {
            this.minecraft = minecraft;
            this.session = session;
        }

        private void change(int setting, int value) {
            byte[] message = WorldMessages.ints(setting, value);
            session.channel.send(WorldMessages.HOST_CHANGE, message, 0, message.length);
        }

        @Override
        public WorldSettings getWorldData() {
            return new WorldSettings() {
                @Override
                public GameType getGameType() {
                    return session.worldGameMode;
                }

                @Override
                public boolean isAllowCommands() {
                    return session.allowCommands;
                }
            };
        }

        @Override
        public boolean isSingleplayerOwner(NameAndId nameAndId) {
            return nameAndId.name().equalsIgnoreCase(minecraft.getUser().getName());
        }

        @Override
        public @Nullable GameType getPersonalGameMode() {
            // The server keeps the client told of the player's game mode.
            return minecraft.gameMode == null ? null : minecraft.gameMode.getPlayerMode();
        }

        @Override
        public boolean isDemo() {
            return false;
        }

        @Override
        public boolean isHardcore() {
            return minecraft.level != null && minecraft.level.getLevelData().isHardcore();
        }

        @Override
        public boolean forceGameMode() {
            return session.forceGameMode;
        }

        @Override
        public void setForceGameMode(boolean forceGameMode) {
            session.forceGameMode = forceGameMode;
            change(WorldMessages.HOST_FORCE_GAME_MODE, forceGameMode ? 1 : 0);
        }

        @Override
        public boolean getGuestCommandAccess() {
            return session.guestCommands;
        }

        @Override
        public void setGuestCommandAccess(boolean commandAccess) {
            session.guestCommands = commandAccess;
            change(WorldMessages.HOST_GUEST_COMMANDS, commandAccess ? 1 : 0);
        }

        @Override
        public MinecraftServer.MultiplayerScope getMultiplayerScope() {
            // Shared through a relay (ADR 0062) shows as the LAN choice it was made with.
            return session.sharing != null
                    ? MinecraftServer.MultiplayerScope.LAN
                    : MinecraftServer.MultiplayerScope.OFF;
        }

        @Override
        public int getPort() {
            return -1;
        }

        @Override
        public boolean publishServer(MinecraftServer.MultiplayerScope scope, int port) {
            // A page can't listen for connections; with WebRTC it shares the world through a relay
            // instead (ADR 0062), else the screen says the LAN game couldn't be hosted.
            if (session.sharing != null) {
                return true;
            }
            WorldSharing started = WorldSharing.start(
                    session.channel, session.world.getFileName().toString(), WorldSharing.relays());
            session.sharing = started;
            return started != null;
        }

        @Override
        public boolean unpublishServer() {
            WorldSharing shared = session.sharing;
            session.sharing = null;
            if (shared == null) {
                return false;
            }
            shared.stop();
            return true;
        }

        @Override
        public void setWorldAllowCommands(boolean allowCommands) {
            session.allowCommands = allowCommands;
            change(WorldMessages.HOST_ALLOW_COMMANDS, allowCommands ? 1 : 0);
        }

        @Override
        public void setWorldGameType(GameType gameMode) {
            session.worldGameMode = gameMode;
            change(WorldMessages.HOST_WORLD_GAME_MODE, gameMode.getId());
        }

        @Override
        public void setPersonalGameType(GameType gameMode) {
            change(WorldMessages.HOST_PERSONAL_GAME_MODE, gameMode.getId());
        }
    }
}
