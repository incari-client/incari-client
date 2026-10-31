package incari.client.net;

import com.mojang.logging.LogUtils;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;

/**
 * The Wisp proxy the player last entered, in {@code incari/wisp-proxy.txt}: saved the moment its
 * screen's Done is pressed, so it isn't typed again for the next server, or after a server's own
 * edit was cancelled or the page reloaded. A server that has Wispcraft off and the default proxy
 * starts from it ({@link WispServers#screen}).
 */
final class SavedWispProxy {

    private static final Logger LOGGER = LogUtils.getLogger();
    private static final String FILE = "wisp-proxy.txt";

    private SavedWispProxy() {}

    /** The proxy saved in {@code incariFolder}, or null if there's none or it can't be read. */
    static @Nullable String load(Path incariFolder) {
        Path file = incariFolder.resolve(FILE);
        if (!Files.isRegularFile(file)) {
            return null;
        }
        try {
            String proxy = Files.readString(file, StandardCharsets.UTF_8).strip();
            return proxy.isEmpty() ? null : proxy;
        } catch (IOException e) {
            LOGGER.warn("Couldn't read {}: {}", file, e.toString());
            return null;
        }
    }

    /** Saves {@code proxy} in {@code incariFolder}. */
    static void save(Path incariFolder, String proxy) {
        Path file = incariFolder.resolve(FILE);
        try {
            Files.createDirectories(incariFolder);
            Files.writeString(file, proxy.strip() + "\n", StandardCharsets.UTF_8);
        } catch (IOException e) {
            LOGGER.warn("Couldn't save {}: {}", file, e.toString());
        }
    }

    /**
     * What the screen starts from: {@code server}'s choice, or the saved proxy for a server that
     * hasn't been set up (off, with the default proxy).
     */
    static WispChoice initial(WispChoice server, @Nullable String saved) {
        if (saved == null || server.on() || !server.proxy().equals(WispChoice.DEFAULT_PROXY)) {
            return server;
        }
        return new WispChoice(false, saved);
    }
}
