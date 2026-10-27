package incari.features.schematics;

import static incari.features.schematics.format.NbtWriter.compound;
import static org.assertj.core.api.Assertions.assertThat;

import incari.features.schematics.format.NbtWriter;
import incari.framework.testing.FakePlayer;
import incari.framework.testing.TestGame;
import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

class SchematicsModuleTest {

    private final SchematicsModule module = new SchematicsModule();
    private final TestGame game = new TestGame(module);
    private FakePlayer player = new FakePlayer();

    /**
     * A Sponge schematic 3 x 2 x 1: stone, stone and stairs facing north along the bottom; air, air
     * and glass on top.
     */
    static byte[] tower() {
        Map<String, Object> palette = compound(
                "minecraft:air", 0,
                "minecraft:stone", 1,
                "minecraft:oak_stairs[facing=north]", 2,
                "minecraft:glass", 3);
        return NbtWriter.gzipped(compound(
                "Version",
                2,
                "DataVersion",
                4440,
                "Width",
                (short) 3,
                "Height",
                (short) 2,
                "Length",
                (short) 1,
                "Palette",
                palette,
                "BlockData",
                NbtWriter.varints(new int[] {1, 1, 2, 0, 0, 3}),
                "BlockEntities",
                List.of()));
    }

    @BeforeEach
    void join() {
        player = game.join().at(100.5, 64, 200.5);
        module.setEnabled(true);
    }

    private void load() {
        game.chosenName = "tower.schem";
        game.chosenBytes = tower();
        module.choose.run();
    }

    @Test
    void choosingAFileLoadsItWhereThePlayerStands() {
        assertThat(module.here.isVisible()).isFalse();
        load();
        assertThat(game.screens).containsExactly("choose:schematics .litematic .schem .nbt");
        assertThat(game.messages).containsExactly("Schematics: tower, 3 x 2 x 1, placed where you stand.");
        assertThat(module.isLoaded()).isTrue();
        assertThat(module.here.isVisible()).isTrue();

        List<String> drawn = game.drawWorld();
        assertThat(drawn)
                .contains(
                        "ghost minecraft:stone at 100,64,200",
                        "ghost minecraft:stone at 101,64,200",
                        "ghost minecraft:oak_stairs[facing=north] at 102,64,200",
                        "ghost minecraft:glass at 102,65,200",
                        "box 100,64,200 to 103,66,201 #ff55aaff");
    }

    @Test
    void builtBlocksStopBeingGhostsAndWrongOnesAreBoxed() {
        load();
        game.blocks.set(100, 64, 200, "minecraft:stone");
        game.blocks.set(101, 64, 200, "minecraft:dirt");
        game.blocks.set(102, 64, 200, "minecraft:oak_stairs[facing=south]");
        game.blocks.set(100, 65, 200, "minecraft:torch");
        game.ticks(Verifier.NEAR_TICKS + 1);

        List<String> drawn = game.drawWorld();
        assertThat(drawn).noneMatch(line -> line.startsWith("ghost minecraft:stone"));
        assertThat(drawn)
                .contains(
                        "ghost minecraft:glass at 102,65,200",
                        "box 101,64,200 to 102,65,201 #ffff3333",
                        "box 102,64,200 to 103,65,201 #ffffaa00",
                        "box 100,65,200 to 101,66,201 #ffff55ff");

        module.highlight.set(false);
        assertThat(game.drawWorld()).noneMatch(line -> line.startsWith("box 101,64,200"));
    }

    @Test
    void theMaterialListCountsWhatsLeftAndWhatThePlayerCarries() {
        load();
        player.carry("minecraft:stone", 5);
        game.ticks(SchematicsModule.MATERIAL_TICKS);
        TestGame.Canvas hud = game.drawHud();
        String all = String.join("\n", hud.drawn);
        assertThat(all).contains("Materials: 3 left", "\"2\"", "POSITIVE");

        game.blocks.set(100, 64, 200, "minecraft:stone");
        game.blocks.set(101, 64, 200, "minecraft:stone");
        game.ticks(SchematicsModule.MATERIAL_TICKS);
        assertThat(String.join("\n", game.drawHud().drawn)).contains("Materials: 2 left", "2 of 4 placed");
    }

    @Test
    void turningMovesTheGhostsRoundItsCorner() {
        load();
        module.turn.run();
        List<String> drawn = game.drawWorld();
        // Turned clockwise, the row that ran east now runs south, and the stairs face east.
        assertThat(drawn)
                .contains(
                        "ghost minecraft:stone at 100,64,200",
                        "ghost minecraft:stone at 100,64,201",
                        "ghost minecraft:oak_stairs[facing=east] at 100,64,202",
                        "box 100,64,200 to 101,66,203 #ff55aaff");

        // Mirrored east-west first, the stairs swap ends; facing north, they still face north before the turn.
        module.mirror.run();
        assertThat(game.drawWorld())
                .contains(
                        "ghost minecraft:oak_stairs[facing=east] at 100,64,200", "ghost minecraft:stone at 100,64,202");
    }

    @Test
    void oneLayerAtATime() {
        load();
        module.layers.set(SchematicsModule.Layers.ONE);
        assertThat(module.layer()).isEqualTo(64);
        assertThat(game.drawWorld()).noneMatch(line -> line.contains(",65,"));
        player.at(100.5, 65, 200.5);
        module.layers.set(SchematicsModule.Layers.ALL);
        module.layers.set(SchematicsModule.Layers.ONE);
        assertThat(game.drawWorld()).noneMatch(line -> line.startsWith("ghost") && line.contains(",64,"));
    }

    @Test
    void aBrokenFileOrUnknownBlocksAreSaid() {
        game.chosenName = "broken.schem";
        game.chosenBytes = new byte[] {0x1F, (byte) 0x8B, 1, 2};
        module.choose.run();
        assertThat(game.messages).singleElement().asString().startsWith("Schematics: Couldn't load broken.schem: ");
        assertThat(module.isLoaded()).isFalse();

        game.blocks.unknown.add("minecraft:glass");
        load();
        assertThat(game.messages.get(1)).endsWith("1 block isn't in this version: left out.");
        assertThat(game.drawWorld()).noneMatch(line -> line.contains("glass"));
    }

    @Test
    void unloadingTakesItAway() {
        load();
        module.unload.run();
        assertThat(module.isLoaded()).isFalse();
        assertThat(game.drawWorld()).isEmpty();
        assertThat(game.drawHud().drawn).isEmpty();
    }
}
