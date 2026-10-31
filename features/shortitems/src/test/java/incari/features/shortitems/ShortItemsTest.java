package incari.features.shortitems;

import static org.assertj.core.api.Assertions.assertThat;

import incari.framework.testing.TestGame;
import org.junit.jupiter.api.Test;

class ShortItemsTest {

    private final ShortItems module = new ShortItems();
    private final TestGame game = new TestGame(module);

    @Test
    void shrinksEveryItemInEitherHand() {
        module.setEnabled(true);
        assertThat(game.heldSword(true, false).scale()).isEqualTo(0.7F);
        assertThat(game.heldItem(true, false, false).scale()).isEqualTo(0.7F);
        assertThat(game.heldItem(false, false, false).scale()).isEqualTo(0.7F);
        assertThat(game.heldItem(false, true, false).scale()).isEqualTo(0.7F);
    }

    @Test
    void followsTheSizeAndCanLeaveAnItemInUseAlone() {
        module.setEnabled(true);
        module.size.set(50);
        assertThat(game.heldItem(false, false, false).scale()).isEqualTo(0.5F);
        module.whileUsing.set(false);
        assertThat(game.heldSword(true, true).scale()).isEqualTo(1);
        assertThat(game.heldItem(true, false, true).scale()).isEqualTo(1);
        assertThat(game.heldItem(true, false, false).scale()).isEqualTo(0.5F);
    }

    @Test
    void keepsShortSwordsIdSoProfilesKeepIt() {
        assertThat(module.id()).isEqualTo("shortswords");
    }

    @Test
    void offLeavesItemsAsTheGameDrawsThem() {
        assertThat(game.heldSword(true, false).scale()).isEqualTo(1);
        assertThat(game.heldItem(true, false, false).scale()).isEqualTo(1);
    }
}
