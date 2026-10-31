package incari.client.net;

import static org.assertj.core.api.Assertions.assertThat;

import java.nio.file.Path;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class SavedWispProxyTest {

    @TempDir
    Path folder;

    @Test
    void remembersAcrossLoads() {
        assertThat(SavedWispProxy.load(folder)).isNull();
        SavedWispProxy.save(folder, " wss://wisp.example/ ");
        assertThat(SavedWispProxy.load(folder)).isEqualTo("wss://wisp.example/");
    }

    @Test
    void startsAServerNotYetSetUpFromTheSavedProxy() {
        assertThat(SavedWispProxy.initial(WispChoice.OFF, "wss://wisp.example/"))
                .isEqualTo(new WispChoice(false, "wss://wisp.example/"));
        assertThat(SavedWispProxy.initial(WispChoice.OFF, null)).isEqualTo(WispChoice.OFF);
    }

    @Test
    void keepsAServerThatHasItsOwnChoice() {
        WispChoice own = new WispChoice(false, "wss://other.example/");
        assertThat(SavedWispProxy.initial(own, "wss://wisp.example/")).isEqualTo(own);
        WispChoice onWithDefault = new WispChoice(true, WispChoice.DEFAULT_PROXY);
        assertThat(SavedWispProxy.initial(onWithDefault, "wss://wisp.example/")).isEqualTo(onWithDefault);
    }
}
