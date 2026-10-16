package incari.client.online;

import static org.assertj.core.api.Assertions.assertThat;

import incari.client.IncariBuild;
import incari.framework.config.ConfigStorage;
import incari.online.FakeService;
import incari.online.OnlineService;
import java.util.HashMap;
import java.util.Map;
import org.jspecify.annotations.Nullable;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

class UpdateNoticeTest {

    private final FakeService fake = new FakeService();

    @AfterEach
    void unforce() {
        UpdateNotice.force(false);
    }

    /** A client that has said hello to a service naming {@code release} as the latest. */
    private OnlineService greeted(String release, String... notes) {
        fake.announceRelease(release, notes);
        Map<String, String> files = new HashMap<>();
        OnlineService online = new OnlineService(
                "wss://online.test/v1/ws",
                "test",
                fake,
                new ConfigStorage() {
                    @Override
                    public @Nullable String read(String path) {
                        return files.get(path);
                    }

                    @Override
                    public void write(String path, String text) {
                        files.put(path, text);
                    }

                    @Override
                    public void delete(String path) {
                        files.remove(path);
                    }
                },
                Runnable::run,
                () -> {},
                () -> 0,
                () -> 0.5,
                (message, error) -> {});
        online.start();
        for (int i = 0; i < 6; i++) {
            online.tick();
        }
        assertThat(online.latestRelease()).isEqualTo(release);
        return online;
    }

    @Test
    void aDevBuildIsNeverTold() {
        assertThat(IncariBuild.RELEASE).isFalse();
        assertThat(UpdateNotice.newer(greeted("999.0.0"))).isNull();
    }

    @Test
    void aNewerReleaseIsShown() {
        UpdateNotice.force(true);
        assertThat(UpdateNotice.newer(greeted("999.0.0"))).isEqualTo("999.0.0");
    }

    @Test
    void thisReleaseOrAnOlderOneIsNot() {
        UpdateNotice.force(true);
        assertThat(UpdateNotice.newer(greeted(IncariBuild.VERSION))).isNull();
        assertThat(UpdateNotice.newer(greeted("0.0.1"))).isNull();
    }

    @Test
    void theNoticeCarriesTheNewerReleasesChangelog() {
        UpdateNotice.force(true);
        assertThat(UpdateNotice.notes(greeted("999.0.0", "Faster chunk loading", "A changelog")))
                .containsExactly("Faster chunk loading", "A changelog");
        assertThat(UpdateNotice.notes(greeted("999.0.0"))).isEmpty();
    }

    @Test
    void noChangelogWithoutANotice() {
        assertThat(UpdateNotice.notes(greeted("999.0.0", "Faster chunk loading")))
                .as("dev build")
                .isEmpty();
        UpdateNotice.force(true);
        assertThat(UpdateNotice.notes(greeted("0.0.1", "Old news"))).isEmpty();
    }
}
