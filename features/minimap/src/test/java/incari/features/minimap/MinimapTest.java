package incari.features.minimap;

import static org.assertj.core.api.Assertions.assertThat;

import incari.features.minimap.Minimap.MapShape;
import incari.framework.game.PlayerView;
import incari.framework.hud.Anchor;
import incari.framework.hud.HudElement;
import incari.framework.hud.Shape;
import incari.framework.testing.TestGame;
import incari.framework.waypoint.Waypoint;
import java.util.List;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

class MinimapTest {

    private final Minimap module = new Minimap();
    private final TestGame game = new TestGame(module);
    private final HudElement map = module.hudElements().get(0);

    @BeforeEach
    void atTheTopLeft() {
        module.setEnabled(true);
        map.anchor().set(Anchor.TOP_LEFT);
        map.offsetX().set(0);
        map.offsetY().set(0);
    }

    private List<String> drawn() {
        return game.drawLive().drawn;
    }

    @Test
    void liveAndOnlyInAWorld() {
        assertThat(map.isLive()).isTrue();
        assertThat(drawn()).isEmpty();
        game.join();
        assertThat(game.drawHud().drawn).isEmpty();
        assertThat(drawn()).isNotEmpty();
    }

    @Test
    void theGamesEffectIconsMoveBelowItInTheTopRight() {
        map.resetPlacement();
        game.join();
        game.drawLive();
        // At 4, 4 with the coordinates below: 111 tall, so the icons start a gap below 115.
        assertThat(game.system().effectIconsShift(320, 240)).isEqualTo(116);

        // Elsewhere, or once it's off, they keep their place.
        map.anchor().set(Anchor.TOP_LEFT);
        game.drawLive();
        assertThat(game.system().effectIconsShift(320, 240)).isZero();
        map.anchor().set(Anchor.TOP_RIGHT);
        game.drawLive();
        module.setEnabled(false);
        assertThat(game.system().effectIconsShift(320, 240)).isZero();
    }

    @Test
    void theTerrainTurnsWithThePlayer() {
        game.join().at(100.5, 70, -30.25).facing(90);
        assertThat(drawn())
                .contains("terrain round 92 at 2,2 around 101,-30 96 blocks facing 90", "shape ARROW 10 at 48,48 TEXT");
        module.turn.set(false);
        assertThat(drawn())
                .contains(
                        "terrain round 92 at 2,2 around 101,-30 96 blocks facing 0",
                        "shape ARROW 10 at 48,48 turned 90 TEXT");
    }

    @Test
    void zoomKeysStepThroughTheZooms() {
        game.join();
        game.press(module.zoomIn.get());
        game.tick();
        assertThat(module.blocksAcross()).isEqualTo(64);
        for (int i = 0; i < 10; i++) {
            game.press(module.zoomOut.get());
            game.tick();
        }
        assertThat(module.blocksAcross()).isEqualTo(256);
        assertThat(module.zoom.format(module.zoom.get())).isEqualTo("256 blocks across");
    }

    @Test
    void waypointsSitWhereTheyAreAndOnTheEdgeBeyondIt() {
        module.turn.set(false);
        module.zoom.set(5); // 64 blocks across 92 units
        game.join().at(0.5, 64, 0.5);
        Waypoints waypoints = new Waypoints(game);
        waypoints.add("East", 16, 0, 0xFF0000, Shape.STAR);
        waypoints.add("Far north", 0, -500, 0x00FF00, Shape.HEART);
        waypoints.add("Hidden", 5, 5, 0, Shape.CIRCLE).setVisible(false);
        List<String> drawn = drawn();
        // 16 blocks east at 92/64 units a block: 23 units right of the middle.
        assertThat(drawn).contains("shape STAR 8 at 71,48 #ffff0000");
        // Beyond the edge: on it, straight up, smaller.
        assertThat(drawn).contains("shape HEART 6 at 48,6 #ff00ff00");
        assertThat(drawn).noneMatch(line -> line.startsWith("shape CIRCLE"));

        module.showWaypoints.set(false);
        assertThat(drawn()).noneMatch(line -> line.startsWith("shape STAR"));
    }

    @Test
    void turnedWithThePlayerWhatTheyFaceIsUp() {
        game.join().at(0.5, 64, 0.5).facing(90);
        new Waypoints(game).add("East", 16, 0, 0xFF0000, Shape.STAR);
        // Facing east: east is up.
        assertThat(drawn()).anyMatch(line -> line.startsWith("shape STAR 8 at 48,"));
    }

    @Test
    void compassLettersAndCoordinates() {
        game.join().at(-12.5, 70.9, 300.1);
        List<String> drawn = drawn();
        assertThat(drawn).anyMatch(line -> line.startsWith("text \"N\"") && line.endsWith("ACCENT"));
        assertThat(drawn).anyMatch(line -> line.startsWith("text \"S\"") && line.endsWith("TEXT"));
        assertThat(drawn).anyMatch(line -> line.startsWith("text \"-13, 70, 300\""));
        module.compass.set(false);
        module.coordinates.set(false);
        assertThat(drawn()).noneMatch(line -> line.startsWith("text"));
        assertThat(map.height(game.drawLive())).isEqualTo(MapElement.SIZE);
    }

    @Test
    void squareMapsHaveNoRing() {
        module.shape.set(MapShape.SQUARE);
        game.join();
        assertThat(drawn())
                .noneMatch(line -> line.startsWith("ring"))
                .anyMatch(line -> line.startsWith("terrain square"));
    }

    /** Waypoints added to the test's world. */
    private record Waypoints(TestGame game) {

        Waypoint add(String name, int x, int z, int rgb, Shape icon) {
            Waypoint waypoint = new Waypoint(name, x, 64, z, PlayerView.OVERWORLD, rgb, icon);
            game.system().waypoints().add(waypoint);
            return waypoint;
        }
    }

    @Test
    void theWorldMapKeyOpensItAndTheMapIsRememberedAsSet() {
        game.join();
        assertThat(game.rememberMap).isTrue();
        game.press(module.worldMap.get());
        game.tick();
        assertThat(game.screens).containsExactly("worldmap");
        module.remember.set(false);
        assertThat(game.rememberMap).isFalse();
    }
}
