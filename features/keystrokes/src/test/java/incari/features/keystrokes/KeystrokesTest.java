package incari.features.keystrokes;

import static org.assertj.core.api.Assertions.assertThat;

import incari.framework.event.ClickListener;
import incari.framework.game.GameKey;
import incari.framework.setting.BooleanSetting;
import incari.framework.testing.TestGame;
import java.util.List;
import org.junit.jupiter.api.Test;

class KeystrokesTest {

    private final Keystrokes module = new Keystrokes();
    private final TestGame game = new TestGame(module);

    @Test
    void drawsEveryKeyWithItsBinding() {
        module.setEnabled(true);
        List<String> drawn = game.drawHud().drawn;
        for (String name : new String[] {"W", "A", "S", "D", "LMB", "RMB"}) {
            assertThat(drawn).anyMatch(line -> line.startsWith("text \"" + name + "\"") && line.endsWith("TEXT"));
        }
        assertThat(drawn).anyMatch(line -> line.startsWith("text \"0 CPS\""));
    }

    @Test
    void lightsHeldKeys() {
        module.setEnabled(true);
        game.keysDown.add(GameKey.FORWARD);
        game.keysDown.add(GameKey.JUMP);
        List<String> drawn = game.drawHud().drawn;
        assertThat(drawn).anyMatch(line -> line.startsWith("text \"W\"") && line.endsWith("ON_ACCENT"));
        assertThat(drawn).anyMatch(line -> line.startsWith("text \"A\"") && line.endsWith(" TEXT"));
        assertThat(drawn.stream().filter(line -> line.startsWith("fill") && line.endsWith(" ACCENT")))
                .hasSize(2);
    }

    @Test
    void countsClicksUnderTheMouseButtons() {
        module.setEnabled(true);
        game.click(ClickListener.LEFT);
        game.click(ClickListener.LEFT);
        game.click(ClickListener.RIGHT);
        List<String> drawn = game.drawHud().drawn;
        assertThat(drawn).anyMatch(line -> line.startsWith("text \"2 CPS\""));
        assertThat(drawn).anyMatch(line -> line.startsWith("text \"1 CPS\""));
    }

    @Test
    void clicksTooWideForTheButtonDropTheirUnit() {
        module.setEnabled(true);
        for (int i = 0; i < 12; i++) {
            game.click(ClickListener.LEFT);
        }
        // "12 CPS" is 36 wide on the test canvas; a mouse button at the default size is 31.
        List<String> drawn = game.drawHud().drawn;
        assertThat(drawn).anyMatch(line -> line.startsWith("text \"12\""));
        assertThat(drawn).noneMatch(line -> line.startsWith("text \"12 CPS\""));
    }

    @Test
    void sizeFollowsTheRowsShown() {
        module.setEnabled(true);
        ((BooleanSetting) module.settings().get("show-mouse")).set(false);
        ((BooleanSetting) module.settings().get("show-space")).set(false);
        // Two rows of 20 with a gap of 2, three keys across.
        assertThat(game.drawHud().drawn).noneMatch(line -> line.contains("LMB"));
        assertThat(module.hudElements().get(0).height(game.drawHud())).isEqualTo(42);
        assertThat(module.hudElements().get(0).width(game.drawHud())).isEqualTo(64);
    }
}
