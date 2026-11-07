package incari.client.resources;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import com.google.common.hash.HashCode;
import com.google.common.hash.HashFunction;
import com.google.common.hash.Hashing;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.OptionalLong;
import net.minecraft.util.HttpUtil;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

@SuppressWarnings("deprecation") // SHA-1 is what the protocol names packs by
class PackDownloadsTest {

    private static final HashFunction SHA1 = Hashing.sha1();
    private static final byte[] PACK = "a pack".getBytes(StandardCharsets.UTF_8);
    private static final HashCode PACK_HASH = SHA1.hashBytes(PACK);

    @TempDir
    Path dir;

    private final List<String> fetched = new ArrayList<>();
    private final Progress progress = new Progress();

    private PackDownloads.Fetch serving(String url, byte[] body) {
        return (asked, maxBytes) -> {
            fetched.add(asked);
            if (!asked.equals(url)) {
                throw new IOException("blocked");
            }
            return body;
        };
    }

    @Test
    void keepsTheFetchedPackUnderItsHash() throws IOException {
        Path file = PackDownloads.download(
                dir,
                "https://example.com/p.zip",
                SHA1,
                PACK_HASH,
                100,
                progress,
                serving("https://example.com/p.zip", PACK));

        assertThat(file).isEqualTo(dir.resolve(PACK_HASH.toString()));
        assertThat(Files.readAllBytes(file)).isEqualTo(PACK);
        assertThat(progress.events).containsExactly("start", "download", "bytes " + PACK.length, "finished true");
    }

    @Test
    void reusesAKeptPackWithoutFetching() throws IOException {
        Files.write(dir.resolve(PACK_HASH.toString()), PACK);

        Path file = PackDownloads.download(
                dir, "https://example.com/p.zip", SHA1, PACK_HASH, 100, progress, serving("nowhere", PACK));

        assertThat(file).isEqualTo(dir.resolve(PACK_HASH.toString()));
        assertThat(fetched).isEmpty();
        assertThat(progress.events).containsExactly("start", "finished true");
    }

    @Test
    void fetchesAgainOverAKeptFileThatIsntThePack() throws IOException {
        Files.write(dir.resolve(PACK_HASH.toString()), "damaged".getBytes(StandardCharsets.UTF_8));

        Path file = PackDownloads.download(
                dir,
                "https://example.com/p.zip",
                SHA1,
                PACK_HASH,
                100,
                progress,
                serving("https://example.com/p.zip", PACK));

        assertThat(Files.readAllBytes(file)).isEqualTo(PACK);
    }

    @Test
    void refusesAPackWhoseHashIsntTheOneTheServerNamed() {
        byte[] other = "another pack".getBytes(StandardCharsets.UTF_8);

        assertThatThrownBy(() -> PackDownloads.download(
                        dir,
                        "https://example.com/p.zip",
                        SHA1,
                        PACK_HASH,
                        100,
                        progress,
                        serving("https://example.com/p.zip", other)))
                .isInstanceOf(IllegalStateException.class)
                .hasRootCauseMessage(
                        "Its hash is " + SHA1.hashBytes(other) + ", not the " + PACK_HASH + " the server named");
        assertThat(progress.events).last().isEqualTo("finished false");
    }

    @Test
    void namesAPackWithNoHashGivenByItsOwn() throws IOException {
        Path file = PackDownloads.download(
                dir,
                "https://example.com/p.zip",
                SHA1,
                null,
                100,
                progress,
                serving("https://example.com/p.zip", PACK));

        assertThat(file).isEqualTo(dir.resolve(PACK_HASH.toString()));
    }

    @Test
    void triesAnHttpPackOverHttpsWhenHttpFails() throws IOException {
        byte[] body = PackDownloads.fetchEitherScheme(
                "http://example.com/p.zip", 100, serving("https://example.com/p.zip", PACK));

        assertThat(body).isEqualTo(PACK);
        assertThat(fetched).containsExactly("http://example.com/p.zip", "https://example.com/p.zip");
    }

    @Test
    void reportsTheFirstFailureWhenBothSchemesFail() {
        assertThatThrownBy(() ->
                        PackDownloads.fetchEitherScheme("http://example.com/p.zip", 100, serving("nowhere", PACK)))
                .isInstanceOf(IOException.class)
                .hasMessage("blocked")
                .satisfies(e -> assertThat(e.getSuppressed()).hasSize(1));
        assertThat(fetched).hasSize(2);
    }

    @Test
    void triesAnHttpsPackOnce() {
        assertThatThrownBy(() ->
                        PackDownloads.fetchEitherScheme("https://example.com/p.zip", 100, serving("nowhere", PACK)))
                .isInstanceOf(IOException.class);
        assertThat(fetched).containsExactly("https://example.com/p.zip");
    }

    private static final class Progress implements HttpUtil.DownloadProgressListener {
        final List<String> events = new ArrayList<>();

        @Override
        public void requestStart() {
            events.add("start");
        }

        @Override
        public void downloadStart(OptionalLong sizeBytes) {
            events.add("download");
        }

        @Override
        public void downloadedBytes(long bytesSoFar) {
            events.add("bytes " + bytesSoFar);
        }

        @Override
        public void requestFinished(boolean success) {
            events.add("finished " + success);
        }
    }
}
