package incari.client.settings;

import com.mojang.logging.LogUtils;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
import java.util.TreeMap;
import net.minecraft.client.Minecraft;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;

/**
 * The client's own settings, the device's rather than a module profile's: what Incari Settings
 * switches (screenshots to the clipboard, usage counts, Incari icons, the screen recorder's
 * options) and the install's usage-count id. Kept as {@code key=value} lines in
 * {@code incari/settings.txt}, written at each change: the browser's files are in memory and
 * stored behind (ADR 0011), so a write never waits on the disk.
 *
 * <p>Game thread.
 */
public final class ClientSettings {

    private static final Logger LOGGER = LogUtils.getLogger();
    private static final String FILE = "settings.txt";

    /** F2's screenshot also goes on the clipboard. */
    public static final Flag COPY_SCREENSHOTS = new Flag("copy-screenshots", false);

    /** A daily anonymous count of this install (ADR 0062). */
    public static final Flag USAGE_COUNTS = new Flag("usage-counts", true);

    /** Other Incari players' badge beside their names. */
    public static final Flag SHOW_BADGES = new Flag("show-badges", true);

    /** The screen recorder takes the microphone too. */
    public static final Flag RECORD_MICROPHONE = new Flag("record-microphone", false);

    /** The disconnection screen joins the server again after a countdown ({@code Reconnect}). */
    public static final Flag AUTO_RECONNECT = new Flag("auto-reconnect", false);

    private static final Map<String, String> VALUES = new TreeMap<>();
    private static @Nullable Path file;

    private ClientSettings() {}

    /** A switch, on or off. */
    public record Flag(String key, boolean defaultValue) {

        public boolean get() {
            String value = value(key);
            return value == null ? defaultValue : value.equals("true");
        }

        public void set(boolean on) {
            put(key, on ? "true" : "false");
        }
    }

    /** A whole number. */
    public record Whole(String key, int min, int max, int defaultValue) {

        public int get() {
            String value = value(key);
            if (value == null) {
                return defaultValue;
            }
            try {
                return Math.clamp(Integer.parseInt(value), min, max);
            } catch (NumberFormatException e) {
                return defaultValue;
            }
        }

        public void set(int number) {
            put(key, Integer.toString(Math.clamp(number, min, max)));
        }
    }

    /** The text kept under {@code key}, or null. */
    public static @Nullable String value(String key) {
        load();
        return VALUES.get(key);
    }

    /** Keeps {@code text} (one line) under {@code key}, and saves. */
    public static void put(String key, String text) {
        load();
        if (text.equals(VALUES.put(key, text))) {
            return;
        }
        Path path = file;
        if (path == null) {
            return;
        }
        StringBuilder lines = new StringBuilder();
        for (Map.Entry<String, String> entry : VALUES.entrySet()) {
            lines.append(entry.getKey()).append('=').append(entry.getValue()).append('\n');
        }
        try {
            Files.createDirectories(path.getParent());
            Files.writeString(path, lines, StandardCharsets.UTF_8);
        } catch (IOException e) {
            LOGGER.warn("Couldn't save {}: {}", path, e.toString());
        }
    }

    private static void load() {
        if (file != null) {
            return;
        }
        Minecraft minecraft = Minecraft.getInstance();
        if (minecraft == null) {
            return;
        }
        Path path = minecraft.gameDirectory.toPath().resolve("incari").resolve(FILE);
        file = path;
        if (!Files.isRegularFile(path)) {
            return;
        }
        try {
            for (String line : Files.readAllLines(path, StandardCharsets.UTF_8)) {
                int split = line.indexOf('=');
                if (split > 0) {
                    VALUES.put(
                            line.substring(0, split).trim(),
                            line.substring(split + 1).trim());
                }
            }
        } catch (IOException e) {
            LOGGER.warn("Couldn't read {}: {}", path, e.toString());
        }
    }
}
