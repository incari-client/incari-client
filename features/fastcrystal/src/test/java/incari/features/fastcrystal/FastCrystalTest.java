package incari.features.fastcrystal;

import static org.assertj.core.api.Assertions.assertThat;

import incari.framework.testing.TestGame;
import org.junit.jupiter.api.Test;

class FastCrystalTest {

    private final FastCrystal module = new FastCrystal();
    private final TestGame game = new TestGame(module);

    @Test
    void offUntilTurnedOnAndOffAgainAfter() {
        assertThat(module.isEnabled()).isFalse();
        assertThat(game.predictCrystals).isFalse();
        assertThat(game.predictAnchors).isFalse();

        module.setEnabled(true);
        assertThat(game.predictCrystals).isTrue();
        assertThat(game.predictAnchors).isTrue();

        module.setEnabled(false);
        assertThat(game.predictCrystals).isFalse();
        assertThat(game.predictAnchors).isFalse();
    }

    @Test
    void eachKindCanBeLeftToTheGame() {
        module.setEnabled(true);
        module.anchors.set(false);
        assertThat(game.predictCrystals).isTrue();
        assertThat(game.predictAnchors).isFalse();

        module.crystals.set(false);
        module.anchors.set(true);
        assertThat(game.predictCrystals).isFalse();
        assertThat(game.predictAnchors).isTrue();
    }

    @Test
    void aSettingChangedWhileOffWaitsForTheModule() {
        module.anchors.set(false);
        assertThat(game.predictAnchors).isFalse();
        assertThat(game.predictCrystals).isFalse();

        module.setEnabled(true);
        assertThat(game.predictCrystals).isTrue();
        assertThat(game.predictAnchors).isFalse();
    }
}
