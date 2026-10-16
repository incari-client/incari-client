package incari.features.zoom;

import static org.assertj.core.api.Assertions.assertThat;

import incari.framework.event.Turn;
import incari.framework.setting.Key;
import incari.framework.testing.TestGame;
import org.junit.jupiter.api.Test;

class ZoomTest {

    private static final Key C = Key.keyboard(Zoom.KEY_C);

    private final Zoom module = new Zoom();
    private final TestGame game = new TestGame(module);

    @Test
    void holdingTheKeyZoomsInAsOptiFineDoes() {
        module.setEnabled(true);
        game.join();
        game.hold(C, true);
        game.tick();
        assertThat(game.fovScale).isEqualTo(0.25F);
        assertThat(game.smoothCamera).isTrue();
        // Smooth turning is the game's own: the turn itself is left alone.
        Turn turn = game.turn(8, 4);
        assertThat(turn.scaled()).isFalse();
        assertThat(turn.consumed()).isFalse();

        game.hold(C, false);
        game.tick();
        assertThat(game.fovScale).isEqualTo(1);
        assertThat(game.smoothCamera).isFalse();
    }

    @Test
    void slowerTurningScalesTheTurnByTheZoom() {
        module.setEnabled(true);
        module.turning.set(Zoom.Turning.SLOWER);
        module.amount.set(2);
        game.join();
        game.hold(C, true);
        game.tick();
        assertThat(game.smoothCamera).isFalse();
        Turn turn = game.turn(8, 4);
        assertThat(turn.scaled()).isTrue();
        assertThat(turn.yaw()).isEqualTo(4);
        assertThat(turn.pitch()).isEqualTo(2);
    }

    @Test
    void theToggleStaysOnUntilPressedAgain() {
        module.setEnabled(true);
        module.mode.set(Zoom.Mode.TOGGLE);
        game.join();
        game.press(C);
        game.tick();
        game.tick();
        assertThat(module.zooming()).isTrue();
        game.press(C);
        game.tick();
        assertThat(module.zooming()).isFalse();
        assertThat(game.fovScale).isEqualTo(1);
    }

    @Test
    void easingGetsThereOverAFewFrames() {
        module.setEnabled(true);
        module.ease.set(true);
        game.join();
        game.hold(C, true);
        game.tick();
        assertThat(game.fovScale).isEqualTo(1);
        game.camera(0, 0);
        game.millis += 16;
        game.camera(0, 0);
        assertThat(game.fovScale).isBetween(0.25F, 1F).isNotIn(0.25F, 1F);
        for (int i = 0; i < 60; i++) {
            game.millis += 16;
            game.camera(0, 0);
        }
        assertThat(game.fovScale).isEqualTo(0.25F);
    }

    @Test
    void outsideAWorldNothingZooms() {
        module.setEnabled(true);
        game.hold(C, true);
        game.tick();
        assertThat(module.zooming()).isFalse();
        assertThat(game.fovScale).isEqualTo(1);
    }

    @Test
    void turningOffGivesTheViewBack() {
        module.setEnabled(true);
        game.join();
        game.hold(C, true);
        game.tick();
        module.setEnabled(false);
        assertThat(game.fovScale).isEqualTo(1);
        assertThat(game.smoothCamera).isFalse();
    }
}
