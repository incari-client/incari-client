package incari.features.customsky;

import static org.assertj.core.api.Assertions.assertThat;

import incari.framework.testing.TestGame;
import org.junit.jupiter.api.Test;

class CustomSkyTest {

    private final CustomSky module = new CustomSky();
    private final TestGame game = new TestGame(module);

    @Test
    void showsThePackSkiesAtTheChosenBrightness() {
        module.setEnabled(true);
        assertThat(game.packSkies).isEqualTo(1);
        module.brightness.set(40);
        game.tick();
        assertThat(game.packSkies).isEqualTo(0.4F);
    }

    @Test
    void offGivesTheGameItsSkyBack() {
        module.setEnabled(true);
        module.setEnabled(false);
        assertThat(game.packSkies).isZero();
    }
}
