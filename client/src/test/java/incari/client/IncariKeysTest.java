package incari.client;

import static org.assertj.core.api.Assertions.assertThat;

import net.minecraft.client.KeyMapping;
import org.junit.jupiter.api.Test;

/** {@link IncariKeys}' binds survive a restart: {@code options.txt} reads each line back to its key. */
class IncariKeysTest {

    @Test
    void namesHaveNoColon() {
        // options.txt is "key_<name>:<key>", split at the first colon.
        for (KeyMapping key : IncariKeys.all()) {
            assertThat(key.getName()).doesNotContain(":");
        }
    }
}
