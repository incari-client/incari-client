package incari.features.fpsdisplay;

import static org.assertj.core.api.Assertions.assertThat;

import incari.framework.setting.BooleanSetting;
import incari.framework.testing.TestGame;
import org.junit.jupiter.api.Test;

class FpsDisplayTest {

    private final FpsDisplay module = new FpsDisplay();
    private final TestGame game = new TestGame(module);

    @Test
    void drawsNothingWhileOff() {
        assertThat(game.drawHud().drawn).isEmpty();
    }

    @Test
    void writesTheFrameRateOnTheGlass() {
        module.setEnabled(true);
        game.fps = 144;
        assertThat(game.drawHud().drawn)
                .contains("fill 50x14 at 2,14 BACKGROUND")
                .anyMatch(line -> line.startsWith("text \"144 FPS\""));
    }

    @Test
    void followsTheFormat() {
        assertThat(FpsDisplay.text(FpsDisplay.Format.BEFORE, 60)).isEqualTo("FPS: 60");
        assertThat(FpsDisplay.text(FpsDisplay.Format.NUMBER, 7)).isEqualTo("7");
    }

    @Test
    void colorsBySpeedWhenAsked() {
        module.setEnabled(true);
        ((BooleanSetting) module.settings().get("speed-colors")).set(true);
        game.fps = 24;
        assertThat(game.drawHud().drawn)
                .anyMatch(line -> line.startsWith("text \"24 FPS\"") && line.endsWith("NEGATIVE"));
        game.fps = 75;
        assertThat(game.drawHud().drawn)
                .anyMatch(line -> line.startsWith("text \"75 FPS\"") && line.endsWith("POSITIVE"));
    }
}
