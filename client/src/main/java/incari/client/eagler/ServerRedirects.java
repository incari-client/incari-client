package incari.client.eagler;

import com.mojang.logging.LogUtils;
import incari.client.net.ServerVersions;
import incari.net.WebSocketUrl;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.ConnectScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.gui.screens.multiplayer.JoinMultiplayerScreen;
import net.minecraft.client.multiplayer.ServerData;
import net.minecraft.client.multiplayer.resolver.ServerAddress;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;

/**
 * An Eagler server sending the player to another server (ADR 0019): during login, in place
 * of letting them in, or during play. Only WebSocket addresses are followed; the player joins the
 * new server as a server of their list would be, with "Auto" for its version.
 */
public final class ServerRedirects {

    private static final Logger LOGGER = LogUtils.getLogger();
    private static final Object LOCK = new Object();
    private static @Nullable String pending;

    private ServerRedirects() {}

    /** The login ended in a redirect; the disconnection that follows joins {@code address}. */
    static void pending(String address) {
        synchronized (LOCK) {
            pending = address;
        }
    }

    /** The redirect the last failed login left, once. */
    static @Nullable String takePending() {
        synchronized (LOCK) {
            String address = pending;
            pending = null;
            return address;
        }
    }

    /** During play: leaves this server for {@code address}. Any thread. */
    static void redirect(String address) {
        Minecraft minecraft = Minecraft.getInstance();
        minecraft.execute(() -> {
            if (minecraft.level != null) {
                minecraft.disconnectFromWorld(net.minecraft.network.chat.Component.literal("Redirected"));
            }
            join(minecraft, new JoinMultiplayerScreen(new TitleScreen()), address);
        });
    }

    /** Joins {@code address} from {@code parent}, if it's a WebSocket address. Render thread. */
    static boolean join(Minecraft minecraft, Screen parent, String address) {
        if (!WebSocketUrl.isWebSocketUrl(address)) {
            LOGGER.warn("Not following a redirect to {}: only ws:// and wss:// addresses", address);
            return false;
        }
        LOGGER.info("Redirected to {}", address);
        ServerData server = new ServerData(address, address, ServerData.Type.OTHER);
        // Via as the player set it for the server that sent them on.
        server.incariVersion = ServerVersions.redirectChoice();
        ConnectScreen.startConnecting(parent, minecraft, ServerAddress.parseString(address), server, false, null);
        return true;
    }
}
