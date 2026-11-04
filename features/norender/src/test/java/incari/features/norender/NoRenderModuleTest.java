package incari.features.norender;

import static org.assertj.core.api.Assertions.assertThat;

import incari.framework.game.NoRender;
import incari.framework.game.NoRender.Items;
import incari.framework.testing.TestGame;
import org.junit.jupiter.api.Test;

class NoRenderModuleTest {

    private final NoRenderModule module = new NoRenderModule();
    private final TestGame game = new TestGame(module);

    @Test
    void hidesItemsAndSparklesWhileOnAndNothingAfter() {
        assertThat(game.noRender).isNull();

        module.setEnabled(true);
        assertThat(game.noRender)
                .isEqualTo(new NoRender(Items.HIDDEN, true, false, false, false, true, false, false, false));

        module.setEnabled(false);
        assertThat(game.noRender).isNull();
    }

    @Test
    void aChangedSettingReachesTheGameAtOnceButOnlyWhileOn() {
        module.setEnabled(true);
        module.items.set(Items.STILL);
        module.orbs.set(true);
        module.allParticles.set(true);
        assertThat(game.noRender).isNotNull();
        assertThat(game.noRender.items()).isEqualTo(Items.STILL);
        assertThat(game.noRender.orbs()).isTrue();
        assertThat(game.noRender.allParticles()).isTrue();

        module.setEnabled(false);
        module.fireworks.set(true);
        assertThat(game.noRender).isNull();
    }

    @Test
    void onePerStackIsOnlyOfferedWhileItemsAreDrawn() {
        assertThat(module.oneCopy.isVisible()).isFalse();
        module.items.set(Items.STILL);
        assertThat(module.oneCopy.isVisible()).isTrue();
    }

    @Test
    void choicesAreKeptByTheirNames() {
        module.items.set(Items.STILL);
        assertThat(module.items.key()).isEqualTo("still");
    }
}
