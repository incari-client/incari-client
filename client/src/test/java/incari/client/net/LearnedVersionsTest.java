package incari.client.net;

import static org.assertj.core.api.Assertions.assertThat;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class LearnedVersionsTest {

    @TempDir
    Path folder;

    @Test
    void remembersAcrossLoads() {
        LearnedVersions.load(folder).put("Play.Example.net", 47);
        assertThat(LearnedVersions.load(folder).get("play.example.net")).isEqualTo(47);
    }

    @Test
    void dropsTheOldFileUnread() throws IOException {
        // Its entries may be 1.8 taught by a silent drop after a kick; each server is learned again.
        Files.writeString(folder.resolve("server-versions.txt"), "wss://carrot.example=47\n");
        assertThat(LearnedVersions.load(folder).get("wss://carrot.example")).isEqualTo(-1);
        assertThat(folder.resolve("server-versions.txt")).doesNotExist();
    }

    @Test
    void aSilentDropOfAVersionThatWorkedIsNoHint() {
        // A rejoin after a kick, turned away while the old session lingers: not a version problem.
        assertThat(VersionRetry.dropIsAHint(775, 775)).isFalse();
        // A first join, or a learned version the server has since stopped speaking.
        assertThat(VersionRetry.dropIsAHint(775, -1)).isTrue();
        assertThat(VersionRetry.dropIsAHint(775, 47)).isTrue();
    }
}
