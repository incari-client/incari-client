package incari.client.net;

import com.mojang.logging.LogUtils;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.Locale;
import java.util.Map;
import org.slf4j.Logger;

/**
 * The versions servers turned out to speak when "Auto" had to find out by joining (ADR 0018): an
 * Eagler server's MOTD doesn't say, and a proxy's status ping reports the client's own version
 * whatever its backend runs. Kept by address in {@code incari/learned-versions.txt}, one {@code
 * address=protocol} per line, so the next join goes straight to the right one.
 *
 * <p>Builds up to 0.1.2 kept them in {@code server-versions.txt}, where a silent drop after a kick
 * could teach a multi-version server 1.8 for good. That file is dropped unread: each server is
 * learned again on its next join.
 */
final class LearnedVersions {

    private static final Logger LOGGER = LogUtils.getLogger();
    private static final String FILE = "learned-versions.txt";
    private static final String OLD_FILE = "server-versions.txt";

    private final Path file;
    private final Map<String, Integer> versions = new LinkedHashMap<>();

    private LearnedVersions(Path file) {
        this.file = file;
    }

    /** The store in {@code incariFolder}, read now; a missing or unreadable file is an empty store. */
    static LearnedVersions load(Path incariFolder) {
        LearnedVersions store = new LearnedVersions(incariFolder.resolve(FILE));
        try {
            Files.deleteIfExists(incariFolder.resolve(OLD_FILE));
        } catch (IOException e) {
            LOGGER.warn("Couldn't delete {}: {}", incariFolder.resolve(OLD_FILE), e.toString());
        }
        if (!Files.isRegularFile(store.file)) {
            return store;
        }
        try {
            for (String line : Files.readAllLines(store.file, StandardCharsets.UTF_8)) {
                int split = line.lastIndexOf('=');
                if (split <= 0) {
                    continue;
                }
                try {
                    store.versions.put(
                            line.substring(0, split),
                            Integer.parseInt(line.substring(split + 1).trim()));
                } catch (NumberFormatException e) {
                    // A hand-edited line; the rest still count.
                }
            }
        } catch (IOException e) {
            LOGGER.warn("Couldn't read {}: {}", store.file, e.toString());
        }
        return store;
    }

    /** The protocol {@code address} was found to speak, or -1. */
    synchronized int get(String address) {
        return versions.getOrDefault(key(address), -1);
    }

    /** Remembers that {@code address} speaks {@code protocol}, and saves. */
    synchronized void put(String address, int protocol) {
        Integer previous = versions.put(key(address), protocol);
        if (previous == null || previous != protocol) {
            save();
        }
    }

    private void save() {
        StringBuilder text = new StringBuilder();
        versions.forEach((address, protocol) ->
                text.append(address).append('=').append(protocol).append('\n'));
        try {
            Files.createDirectories(file.getParent());
            Files.writeString(file, text, StandardCharsets.UTF_8);
        } catch (IOException e) {
            LOGGER.warn("Couldn't save {}: {}", file, e.toString());
        }
    }

    private static String key(String address) {
        // The address as typed; '=' would split the line, and case never matters in a host name.
        return address.trim().toLowerCase(Locale.ROOT).replace('=', '_');
    }
}
