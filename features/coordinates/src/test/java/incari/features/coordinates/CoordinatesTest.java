package incari.features.coordinates;

import static org.assertj.core.api.Assertions.assertThat;

import incari.framework.game.PlayerView;
import incari.framework.setting.BooleanSetting;
import incari.framework.setting.ChoiceSetting;
import incari.framework.testing.TestGame;
import java.util.List;
import org.junit.jupiter.api.Test;

class CoordinatesTest {

    private final Coordinates module = new Coordinates();
    private final TestGame game = new TestGame(module);

    @Test
    void drawsNothingOutsideAWorld() {
        module.setEnabled(true);
        assertThat(game.drawHud().drawn).isEmpty();
    }

    @Test
    void writesThePositionFacingBiomeAndTheNethersPlace() {
        module.setEnabled(true);
        game.join().at(100.5, 64, -41.2).facing(91).biome("Desert");
        List<String> drawn = game.drawHud().drawn;
        assertThat(drawn.get(0)).startsWith("fill").endsWith("BACKGROUND");
        assertThat(drawn).anyMatch(line -> line.startsWith("text \"100\""));
        assertThat(drawn).anyMatch(line -> line.startsWith("text \"-42\""));
        assertThat(drawn).anyMatch(line -> line.startsWith("text \"East (+X)\""));
        assertThat(drawn).anyMatch(line -> line.startsWith("text \"Desert\""));
        assertThat(drawn).anyMatch(line -> line.startsWith("text \"12, -6\""));
    }

    @Test
    void inTheNetherGivesTheOverworldsPlace() {
        module.setEnabled(true);
        game.join().at(10, 70, -3).in(PlayerView.NETHER);
        assertThat(game.drawHud().drawn).anyMatch(line -> line.startsWith("text \"80, -24\""));
    }

    @Test
    void theOverworldsPlaceIsFromTheExactPosition() {
        module.setEnabled(true);
        // Block 100 and -1 in the Nether, but 807 and -4 in the Overworld: not 800 and -8.
        game.join().at(100.9, 70, -0.5).in(PlayerView.NETHER);
        assertThat(game.drawHud().drawn).anyMatch(line -> line.startsWith("text \"807, -4\""));
    }

    @Test
    @SuppressWarnings("unchecked")
    void oneLineWhenAsked() {
        module.setEnabled(true);
        ((ChoiceSetting<Coordinates.Layout>) module.settings().get("layout")).set(Coordinates.Layout.LINE);
        ((BooleanSetting) module.settings().get("show-facing")).set(false);
        ((BooleanSetting) module.settings().get("show-biome")).set(false);
        ((BooleanSetting) module.settings().get("show-other")).set(false);
        game.join().at(1, 2, 3);
        assertThat(game.drawHud().drawn).contains("text \"X 1  Y 2  Z 3\" at 6,65 TEXT");
    }

    @Test
    void facingRoundsToTheNearestPoint() {
        assertThat(Coordinates.facing(0)).isEqualTo("North (-Z)");
        assertThat(Coordinates.facing(359)).isEqualTo("North (-Z)");
        assertThat(Coordinates.facing(200)).isEqualTo("South (+Z)");
        assertThat(Coordinates.facing(230)).isEqualTo("Southwest");
        assertThat(Coordinates.block(-0.5)).isEqualTo(-1);
    }
}
