package incari.features.saturation;

import static org.assertj.core.api.Assertions.assertThat;

import incari.framework.testing.TestGame;
import org.junit.jupiter.api.Test;

class SaturationTest {

    private final Saturation module = new Saturation();
    private final TestGame game = new TestGame(module);

    @Test
    void tintsTheHungerBarWhileOn() {
        assertThat(game.saturationColor).isEqualTo(-1);
        module.setEnabled(true);
        assertThat(game.saturationColor).isEqualTo(0xFFCC00);
        module.color.set(0x00FF00);
        game.tick();
        assertThat(game.saturationColor).isEqualTo(0x00FF00);
        // Black is a color like any other, not "off".
        module.color.set(0);
        game.tick();
        assertThat(game.saturationColor).isZero();
        module.setEnabled(false);
        assertThat(game.saturationColor).isEqualTo(-1);
    }
}
