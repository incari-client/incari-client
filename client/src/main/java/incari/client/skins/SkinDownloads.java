package incari.client.skins;

import com.mojang.blaze3d.platform.NativeImage;
import com.mojang.logging.LogUtils;
import incari.client.platform.ClientPlatform;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import org.slf4j.Logger;

/**
 * Skins, capes and custom player heads from a profile's {@code textures}, for the game's {@code
 * SkinTextureDownloader}. The game downloads them with a {@code URLConnection}, which the browser
 * build doesn't have, so every head a server textured (Slimefun's items, for one) stayed Steve. They
 * come through the platform's {@code fetch} instead, and a copy is kept where the game keeps it.
 *
 * <p>Runs on the game's download pool: the fetch blocks, so never on the render thread.
 */
public final class SkinDownloads {

    private static final Logger LOGGER = LogUtils.getLogger();
    /** Far past any 64x64 skin; a server could name any file as a head's texture. */
    private static final int MAX_BYTES = 1024 * 1024;

    private SkinDownloads() {}

    /** The image at {@code url}, from {@code localCopy} if an earlier download kept one. */
    public static NativeImage read(Path localCopy, String url) throws IOException {
        if (Files.isRegularFile(localCopy)) {
            return NativeImage.read(Files.readAllBytes(localCopy));
        }
        byte[] png = ClientPlatform.get().network().fetch(secure(url), MAX_BYTES);
        try {
            Files.createDirectories(localCopy.getParent());
            Files.write(localCopy, png);
        } catch (IOException e) {
            // Only the copy is lost: the next session downloads it again.
            LOGGER.warn("Couldn't keep a copy of {} in {}: {}", url, localCopy, e.toString());
        }
        return NativeImage.read(png);
    }

    /**
     * {@code url} over https. Profiles name Mojang's texture server with {@code http://}, which an
     * https page may not fetch; the server answers the same over https.
     */
    static String secure(String url) {
        return url.regionMatches(true, 0, "http://", 0, 7) ? "https://" + url.substring(7) : url;
    }
}
