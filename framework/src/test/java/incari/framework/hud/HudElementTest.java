package incari.framework.hud;

import static org.assertj.core.api.Assertions.assertThat;

import incari.framework.testing.TestGame;
import org.junit.jupiter.api.Test;

class HudElementTest {

    /** Fifty units by ten. */
    private static final class Box extends HudElement {

        Box() {
            super("box", "Box", Anchor.TOP_LEFT, 2, 2);
        }

        @Override
        public int width(HudCanvas canvas) {
            return 50;
        }

        @Override
        public int height(HudCanvas canvas) {
            return 10;
        }

        @Override
        public void draw(HudCanvas canvas) {
            canvas.fill(0, 0, 50, 10, Tone.BACKGROUND);
        }
    }

    private final TestGame.Canvas canvas = new TestGame.Canvas(320, 240);
    private final int[] bounds = new int[4];

    @Test
    void boundsAreWhereItsDrawnScaled() {
        Box box = new Box();
        box.scale().set(1.5F);
        box.anchor().set(Anchor.BOTTOM_RIGHT);
        box.bounds(canvas, bounds);
        assertThat(bounds).containsExactly(243, 223, 75, 15);
        box.render(canvas);
        assertThat(canvas.drawn).containsExactly("fill 50x10 at 243,223 BACKGROUND");
    }

    @Test
    void placingPicksTheNearestAnchorAndKeepsThePlace() {
        Box box = new Box();
        box.place(260, 200, 50, 10, 320, 240);
        assertThat(box.anchor().get()).isEqualTo(Anchor.BOTTOM_RIGHT);
        assertThat(box.offsetX().get()).isEqualTo(10);
        assertThat(box.offsetY().get()).isEqualTo(30);
        box.bounds(canvas, bounds);
        assertThat(bounds).containsExactly(260, 200, 50, 10);

        // On a bigger screen it keeps its distance from the corner.
        TestGame.Canvas wide = new TestGame.Canvas(640, 480);
        box.bounds(wide, bounds);
        assertThat(bounds).containsExactly(580, 440, 50, 10);
    }

    @Test
    void remembersWhereTheGameDrewItButNotTheEditor() {
        Box box = new Box();
        assertThat(box.lastDrawn(bounds)).isFalse();
        box.render(canvas);
        assertThat(box.lastDrawn(bounds)).isTrue();
        assertThat(bounds).containsExactly(2, 2, 50, 10);

        box.offsetY().set(40);
        TestGame.Canvas editor = new TestGame.Canvas(320, 240);
        editor.editing = true;
        box.render(editor);
        box.lastDrawn(bounds);
        assertThat(bounds).containsExactly(2, 2, 50, 10);
    }

    @Test
    void resetPutsItBack() {
        Box box = new Box();
        box.place(135, 115, 50, 10, 320, 240);
        box.scale().set(2.0F);
        box.resetPlacement();
        box.bounds(canvas, bounds);
        assertThat(bounds).containsExactly(2, 2, 50, 10);
    }
}
