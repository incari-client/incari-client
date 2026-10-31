package incari.client.net;

import com.mojang.logging.LogUtils;
import incari.client.ui.EditedScreen;
import incari.client.ui.Theme;
import incari.net.WebSocketUrl;
import incari.net.wisp.WispAddress;
import java.net.InetSocketAddress;
import java.nio.file.Path;
import java.util.Optional;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.ManageServerScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.multiplayer.ServerData;
import net.minecraft.client.multiplayer.resolver.AddressCheck;
import net.minecraft.client.multiplayer.resolver.ResolvedServerAddress;
import net.minecraft.client.multiplayer.resolver.ServerAddress;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.network.chat.Component;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;

/**
 * Wispcraft: joining a Java server through a Wisp proxy ({@code incari.net.wisp}), which opens the
 * server's TCP port for a page that only has WebSockets. Each server in the list has the choice,
 * saved with it and set beside its version on the edit-server screen.
 *
 * <p>The connect screen and the server list's pinger say which server they're about to reach
 * ({@link #route}) on their thread; the resolver hook, called next on the same thread, then gives
 * the server's address with the proxy instead of looking it up: the proxy resolves the host, and a
 * page couldn't. {@link ServerConnections} gives such an address the game's plain TCP pipeline.
 * A {@code ws://} address is an Eagler server and never goes through Wisp.
 */
public final class WispServers {

    private static final Logger LOGGER = LogUtils.getLogger();
    private static final String TAG_ON = "incariWisp";
    private static final String TAG_PROXY = "incariWispProxy";

    /** The choice of the server this thread reaches next; null for one joined directly. */
    private static final ThreadLocal<@Nullable WispChoice> ROUTE = new ThreadLocal<>();

    private WispServers() {}

    /** {@code ServerData.write}: saves the choice if it isn't the default. */
    public static void write(ServerData server, CompoundTag tag) {
        WispChoice choice = server.incariWisp;
        if (choice.on()) {
            tag.putBoolean(TAG_ON, true);
        }
        if (!choice.proxy().equals(WispChoice.DEFAULT_PROXY)) {
            tag.putString(TAG_PROXY, choice.proxy());
        }
    }

    /** {@code ServerData.read}. */
    public static void read(ServerData server, CompoundTag tag) {
        server.incariWisp =
                new WispChoice(tag.getBooleanOr(TAG_ON, false), tag.getStringOr(TAG_PROXY, WispChoice.DEFAULT_PROXY));
    }

    /**
     * {@code ConnectScreen} and {@code ServerStatusPinger}, just before they resolve {@code server}'s
     * address: whether this thread's lookups go through its proxy. Set or cleared every time, so a
     * pinger thread that reaches other servers next doesn't keep it.
     */
    public static void route(ServerData server) {
        if (server.incariWisp.on()) {
            ROUTE.set(server.incariWisp);
        } else {
            ROUTE.remove();
        }
    }

    /** Whether {@code ServerNameResolver.resolveAddress} should hand this address to {@link #resolve}. */
    public static boolean isRouted(ServerAddress address) {
        return ROUTE.get() != null && address.webSocketUrl() == null;
    }

    /**
     * The address through the proxy, if the game's server block list allows the host. No DNS or SRV
     * lookup here: the proxy resolves the host, and {@code WispConnection} asks for the SRV record.
     */
    public static Optional<ResolvedServerAddress> resolve(ServerAddress address, AddressCheck check) {
        WispChoice choice = ROUTE.get();
        if (choice == null) {
            throw new IllegalStateException("No Wisp proxy for " + address);
        }
        if (!check.isAllowed(address)) {
            return Optional.empty();
        }
        WebSocketUrl proxy = WebSocketUrl.tryParse(choice.proxy());
        if (proxy == null) {
            LOGGER.warn("Not a Wisp proxy URL: {}", choice.proxy());
            return Optional.empty();
        }
        WispAddress resolved = new WispAddress(proxy, address.getHost(), address.getPort());
        return Optional.of(new ResolvedServerAddress() {
            @Override
            public String getHostName() {
                return address.getHost();
            }

            @Override
            public String getHostIp() {
                // Unknown: the proxy resolves it. The block list checks the name.
                return address.getHost();
            }

            @Override
            public int getPort() {
                return address.getPort();
            }

            @Override
            public InetSocketAddress asInetSocketAddress() {
                return resolved;
            }
        });
    }

    /**
     * The edit-server screen's edit: halves the version button, and puts the Wispcraft button
     * beside it.
     */
    public static void editServerScreen(EditedScreen<ManageServerScreen> edited) {
        for (AbstractWidget widget : edited.widgets()) {
            if (widget instanceof ServerVersions.VersionButton version) {
                // From the full width, not the current one: a screen shown again is only laid out
                // again, and keeps the button this edit already halved.
                int full = ServerVersions.VersionButton.FULL_WIDTH;
                int half = (full - Theme.GAP) / 2;
                int x = version.getX() + full - half;
                version.setWidth(half);
                edited.add(button(version.server(), x, version.getY(), half));
                return;
            }
        }
    }

    /** Shows the choice, and opens {@link #screen}. */
    private static Button button(ServerData server, int x, int y, int width) {
        return Button.builder(label(server.incariWisp), pressed -> {
                    Minecraft minecraft = Minecraft.getInstance();
                    Screen editScreen = minecraft.gui.screen();
                    if (editScreen != null) {
                        minecraft.gui.setScreen(
                                screen(editScreen, server, () -> pressed.setMessage(label(server.incariWisp))));
                    }
                })
                .bounds(x, y, width, Theme.CONTROL_HEIGHT)
                .build();
    }

    /**
     * The screen that sets {@code server}'s choice, then runs {@code onDone} and goes back to {@code parent}.
     * The proxy entered is saved at once ({@link SavedWispProxy}), and a server not yet set up starts
     * from the last one.
     */
    public static Screen screen(Screen parent, ServerData server, Runnable onDone) {
        Path folder = Minecraft.getInstance().gameDirectory.toPath().resolve("incari");
        WispChoice initial = SavedWispProxy.initial(server.incariWisp, SavedWispProxy.load(folder));
        return new WispScreen(parent, initial, choice -> {
            server.incariWisp = choice;
            SavedWispProxy.save(folder, choice.proxy());
            onDone.run();
        });
    }

    // Literal English until Incari has its own language files.
    private static Component label(WispChoice choice) {
        return Component.literal("Wispcraft: " + (choice.on() ? "On" : "Off"));
    }
}
