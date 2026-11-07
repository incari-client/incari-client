package incari.client.resources;

import com.google.common.hash.HashCode;
import com.google.common.hash.HashFunction;
import com.google.common.hash.Hasher;
import incari.client.platform.ClientPlatform;
import incari.platform.api.net.PlatformNetwork;
import java.io.IOException;
import java.io.InputStream;
import java.net.Proxy;
import java.net.URL;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.FileTime;
import java.time.Instant;
import java.util.Map;
import java.util.OptionalLong;
import net.minecraft.util.HttpUtil;
import org.jspecify.annotations.Nullable;

/**
 * Server resource packs, for the game's {@code DownloadQueue}. The game downloads them with a
 * {@code URLConnection}, which the browser build doesn't have, so every server's pack failed. In a
 * page they come through the platform's {@code fetch}; where the platform has sockets (the
 * desktop) the game's own download runs as before.
 *
 * <p>A page can download only packs whose host allows web pages to (CORS), and sends none of the
 * game's {@code X-Minecraft-*} headers, which would make the browser ask the host first and fail
 * on most hosts. The body arrives whole, so the toast shows its size only at the end.
 *
 * <p>Runs on the download queue's thread: the fetch blocks.
 */
public final class PackDownloads {

    /** Fetches a URL's body, at most {@code maxBytes}: the platform's {@code fetch}, or a test's. */
    @FunctionalInterface
    interface Fetch {
        byte[] get(String url, int maxBytes) throws IOException;
    }

    private PackDownloads() {}

    /** Hook ({@code DownloadQueue}): {@code HttpUtil.downloadFile}'s arguments and contract. */
    public static Path download(
            Path targetDir,
            URL url,
            Map<String, String> headers,
            HashFunction hashing,
            @Nullable HashCode wanted,
            int maxSize,
            Proxy proxy,
            HttpUtil.DownloadProgressListener listener) {
        PlatformNetwork network = ClientPlatform.get().network();
        if (network.canOpenSockets()) {
            return HttpUtil.downloadFile(targetDir, url, headers, hashing, wanted, maxSize, proxy, listener);
        }
        return download(targetDir, url.toString(), hashing, wanted, maxSize, listener, network::fetch);
    }

    /**
     * The pack's file in {@code dir}, named by its hash: the one kept from an earlier download if
     * its hash is {@code wanted}, else fetched.
     *
     * @throws IllegalStateException if it can't be fetched or isn't the pack the server named
     */
    static Path download(
            Path dir,
            String url,
            HashFunction hashing,
            @Nullable HashCode wanted,
            int maxSize,
            HttpUtil.DownloadProgressListener listener,
            Fetch fetch) {
        listener.requestStart();
        boolean done = false;
        try {
            Path file = keptOrFetched(dir, url, hashing, wanted, maxSize, listener, fetch);
            done = true;
            return file;
        } catch (IOException e) {
            throw new IllegalStateException("Couldn't download the server's resource pack from " + url, e);
        } finally {
            listener.requestFinished(done);
        }
    }

    private static Path keptOrFetched(
            Path dir,
            String url,
            HashFunction hashing,
            @Nullable HashCode wanted,
            int maxSize,
            HttpUtil.DownloadProgressListener listener,
            Fetch fetch)
            throws IOException {
        if (wanted != null) {
            Path kept = dir.resolve(wanted.toString());
            if (Files.isRegularFile(kept) && hashOf(kept, hashing).equals(wanted)) {
                // The game clears out packs unused the longest; this one was just used.
                Files.setLastModifiedTime(kept, FileTime.from(Instant.now()));
                return kept;
            }
        }
        listener.downloadStart(OptionalLong.empty());
        byte[] pack = fetchEitherScheme(url, maxSize, fetch);
        listener.downloadedBytes(pack.length);
        HashCode hash = hashing.hashBytes(pack);
        if (wanted != null && !hash.equals(wanted)) {
            throw new IOException("Its hash is " + hash + ", not the " + wanted + " the server named");
        }
        Files.createDirectories(dir);
        Path file = dir.resolve(hash.toString());
        Files.write(file, pack);
        return file;
    }

    /**
     * {@code url}'s body; for an {@code http} URL that fails, the same over https. An https page
     * may not fetch http at all, and many hosts serve both.
     */
    static byte[] fetchEitherScheme(String url, int maxSize, Fetch fetch) throws IOException {
        try {
            return fetch.get(url, maxSize);
        } catch (IOException e) {
            if (!url.regionMatches(true, 0, "http://", 0, 7)) {
                throw e;
            }
            try {
                return fetch.get("https://" + url.substring(7), maxSize);
            } catch (IOException overHttps) {
                e.addSuppressed(overHttps);
                throw e;
            }
        }
    }

    /** The hash of a kept pack, read a piece at a time: packs run to hundreds of megabytes. */
    private static HashCode hashOf(Path file, HashFunction hashing) throws IOException {
        Hasher hasher = hashing.newHasher();
        byte[] piece = new byte[64 * 1024];
        try (InputStream in = Files.newInputStream(file)) {
            for (int n; (n = in.read(piece)) > 0; ) {
                hasher.putBytes(piece, 0, n);
            }
        }
        return hasher.hash();
    }
}
