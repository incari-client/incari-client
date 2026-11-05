package incari.features.waypoints;

import static org.assertj.core.api.Assertions.assertThat;

import incari.features.waypoints.WaypointsModule.Labels;
import incari.framework.game.PlayerView;
import incari.framework.hud.Shape;
import incari.framework.testing.FakePlayer;
import incari.framework.testing.TestGame;
import incari.framework.waypoint.Waypoint;
import incari.framework.waypoint.Waypoints;
import java.util.List;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

class WaypointsModuleTest {

    private final WaypointsModule module = new WaypointsModule();
    private final TestGame game = new TestGame(module);
    private final Waypoints store = game.system().waypoints();
    private FakePlayer player = new FakePlayer();

    @BeforeEach
    void inAWorld() {
        module.setEnabled(true);
        player = game.join().at(10.5, 64, 20.5);
    }

    private Waypoint add(String name, int x, int y, int z) {
        Waypoint waypoint = new Waypoint(name, x, y, z, PlayerView.OVERWORLD, 0x5B8CFF, Shape.DIAMOND);
        store.add(waypoint);
        return waypoint;
    }

    @Test
    void theNewKeyOpensANewWaypointWhereThePlayerStands() {
        game.press(module.newKey.get());
        game.tick();
        assertThat(game.screens).containsExactly("new Waypoint 1");
        // Not added until the player saves it.
        assertThat(store.all()).isEmpty();
    }

    @Test
    void newNamesSkipThoseTaken() {
        add("Waypoint 2", 0, 64, 0);
        assertThat(WaypointsModule.nextName(store.all())).isEqualTo("Waypoint 3");
        add("Waypoint 3", 0, 64, 0);
        assertThat(WaypointsModule.nextName(store.all())).isEqualTo("Waypoint 4");
    }

    @Test
    void theListKeyAndTheSettingsButtonOpenTheList() {
        game.press(module.listKey.get());
        game.tick();
        module.manage.run();
        assertThat(game.screens).containsExactly("waypoints", "waypoints");
    }

    @Test
    void aDeathIsMarkedAndTheNextReplacesIt() {
        player.at(-3.2, 70, 8.9).dead(true);
        game.tick();
        game.tick();
        player.dead(false);
        game.tick();
        assertThat(store.all()).singleElement().satisfies(death -> {
            assertThat(death.isDeath()).isTrue();
            assertThat(death.name()).isEqualTo("Death");
            assertThat(death.icon()).isEqualTo(Shape.CROSS);
            assertThat(death.x()).isEqualTo(-4);
            assertThat(death.y()).isEqualTo(70);
            assertThat(death.z()).isEqualTo(8);
        });
        player.at(100, 12, 100).dead(true);
        game.tick();
        assertThat(store.all())
                .singleElement()
                .satisfies(death -> assertThat(death.x()).isEqualTo(100));
    }

    @Test
    void onAServerADeathIsMarkedForTheVisitOnly() {
        // One address, many maps: the death mustn't be kept to show on the next one.
        store.close();
        store.open("server/play.example.net");
        add("Base", 0, 64, 0);
        player.at(-3.2, 70, 8.9).dead(true);
        game.tick();
        assertThat(store.death())
                .isNotNull()
                .satisfies(death -> assertThat(death.isTemporary()).isTrue());
        store.flush();
        String saved = String.join("\n", game.files.values());
        assertThat(saved).contains("Base").doesNotContain("Death");

        store.close();
        store.open("server/play.example.net");
        assertThat(store.all())
                .singleElement()
                .satisfies(kept -> assertThat(kept.name()).isEqualTo("Base"));
    }

    @Test
    void inSingleplayerADeathIsKept() {
        player.at(-3.2, 70, 8.9).dead(true);
        game.tick();
        assertThat(store.death())
                .isNotNull()
                .satisfies(death -> assertThat(death.isTemporary()).isFalse());
    }

    @Test
    void aDeathIsClearedOnceThePlayerGetsBackToIt() {
        player.at(0, 70, 0).dead(true);
        game.tick();
        // Respawned beside it: not an arrival.
        player.dead(false);
        game.tick();
        assertThat(store.all()).hasSize(1);
        player.at(50, 70, 50);
        game.tick();
        assertThat(store.all()).hasSize(1);
        player.at(2, 71, -1);
        game.tick();
        assertThat(store.all()).isEmpty();
    }

    @Test
    void aDeathStaysWithClearingOff() {
        module.clearDeaths.set(false);
        player.at(0, 70, 0).dead(true);
        game.tick();
        player.dead(false);
        player.at(50, 70, 50);
        game.tick();
        player.at(0, 70, 0);
        game.tick();
        assertThat(store.all()).hasSize(1);
    }

    @Test
    void noDeathMarkedWithMarkDeathsOff() {
        module.deaths.set(false);
        player.dead(true);
        game.tick();
        assertThat(store.all()).isEmpty();
    }

    @Test
    void aWaypointAheadShowsItsIconNameAndDistance() {
        // 30 blocks north, at eye height: straight ahead.
        add("Base", 10, 65, -10);
        List<String> drawn = game.drawLive().drawn;
        assertThat(drawn).hasSize(4);
        assertThat(drawn.get(0)).startsWith("shape DIAMOND 9 at 160,").endsWith("#ff5b8cff");
        assertThat(drawn.get(1)).startsWith("fill").endsWith("BACKGROUND");
        assertThat(drawn.get(2)).startsWith("text \"Base\"").endsWith("TEXT");
        assertThat(drawn.get(3)).startsWith("text \"30 m\"").endsWith("MUTED");
    }

    @Test
    void namesOnlyNearTheCrosshairWhenSoSet() {
        module.labels.set(Labels.LOOKED_AT);
        add("Ahead", 10, 65, -10);
        // 4.5 blocks to the side: on screen, 45 units from the crosshair.
        add("Aside", 15, 65, -10);
        List<String> drawn = game.drawLive().drawn;
        assertThat(drawn)
                .filteredOn(line -> line.startsWith("text"))
                .containsExactly(
                        drawn.stream()
                                .filter(line -> line.startsWith("text \"Ahead\""))
                                .findFirst()
                                .orElseThrow(),
                        drawn.stream()
                                .filter(line -> line.startsWith("text \"30 m\""))
                                .findFirst()
                                .orElseThrow());
    }

    @Test
    void fartherWaypointsAreDrawnFirst() {
        module.labels.set(Labels.NEVER);
        add("Near", 12, 65, 10);
        add("Far", 8, 65, -50);
        // The far one, 70 blocks off and left of the middle, before the near one, 10 off and right.
        assertThat(game.drawLive().drawn)
                .extracting(line -> line.substring(0, line.indexOf(",")))
                .containsExactly("shape DIAMOND 9 at 140", "shape DIAMOND 9 at 180");
    }

    @Test
    void oneBehindIsPointedToFromTheEdge() {
        add("Behind", 10, 65, 60);
        List<String> drawn = game.drawLive().drawn;
        assertThat(drawn.get(0)).startsWith("shape ARROW");
        assertThat(drawn.get(1)).startsWith("shape DIAMOND");
        assertThat(drawn.get(2)).startsWith("text \"40 m\"");
        module.arrows.set(false);
        assertThat(game.drawLive().drawn).isEmpty();
    }

    @Test
    void arrowsThatWouldSitTogetherAreSpreadAlongTheEdge() {
        // Three behind, nearly in a line: their arrows would all sit at the bottom's middle.
        add("One", 10, 65, 60);
        add("Two", 11, 65, 80);
        add("Three", 10, 65, 100);
        assertThat(game.drawLive().drawn)
                .filteredOn(line -> line.startsWith("shape ARROW"))
                .extracting(line -> line.substring(line.indexOf(" at ") + 4, line.indexOf(" turned")))
                .hasSize(3)
                .doesNotHaveDuplicates();
    }

    @Test
    void hiddenOthersDimensionsAndTheOneUnderfootArentDrawn() {
        add("Hidden", 10, 65, -10).setVisible(false);
        store.add(new Waypoint("Nether", 10, 65, -10, PlayerView.NETHER, 0, Shape.CIRCLE));
        add("Here", 10, 64, 20);
        assertThat(game.drawLive().drawn).isEmpty();
        // The beam of the one underfoot still rises round the player.
        assertThat(game.drawWorld()).containsExactly("beam at 10,64,20 #5b8cff");
    }

    @Test
    void beamsRiseFromWaypointsWithTheirBeamOn() {
        add("Beam", 10, 65, -10);
        add("No beam", 40, 70, -10).setBeam(false);
        assertThat(game.drawWorld()).containsExactly("beam at 10,65,-10 #5b8cff");
        module.beams.set(false);
        assertThat(game.drawWorld()).isEmpty();
    }

    @Test
    void distancesReadInMetersThenKilometers() {
        WaypointOverlay.DistanceText text = new WaypointOverlay.DistanceText();
        assertThat(text.of(12.7F)).isEqualTo("12 m");
        assertThat(text.of(999.9F)).isEqualTo("999 m");
        assertThat(text.of(1234)).isEqualTo("1.2 km");
        String same = text.of(1250);
        assertThat(text.of(1299)).isSameAs(same);
        assertThat(text.of(34_560)).isEqualTo("34.5 km");
    }

    @Test
    void nothingDrawnWhileOff() {
        add("Base", 10, 65, -10);
        module.setEnabled(false);
        assertThat(game.drawLive().drawn).isEmpty();
        assertThat(game.drawWorld()).isEmpty();
    }
}
