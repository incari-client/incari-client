package incari.features.schematics;

import static org.assertj.core.api.Assertions.assertThat;

import incari.features.schematics.format.BlockSpec;
import incari.features.schematics.format.Schematic;
import incari.features.schematics.format.SchematicException;
import incari.framework.game.Mirror;
import incari.framework.testing.FakeBlocks;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;
import org.junit.jupiter.api.Test;

class PlacementTest {

    /** A 4 x 2 x 3 box with a different block in every place. */
    private static Schematic numbered() throws SchematicException {
        Schematic.Builder builder = new Schematic.Builder(4, 2, 3);
        for (int y = 0; y < 2; y++) {
            for (int z = 0; z < 3; z++) {
                for (int x = 0; x < 4; x++) {
                    builder.set(x, y, z, builder.index(BlockSpec.of("test:b" + x + y + z, Map.of())));
                }
            }
        }
        return builder.build();
    }

    @Test
    void everyTurnAndMirrorMapsEachBlockToOnePlaceInTheBox() throws SchematicException {
        Schematic schematic = numbered();
        int[] out = new int[3];
        for (Mirror mirror : Mirror.values()) {
            for (int turns = 0; turns < 4; turns++) {
                Placement placement = new Placement(schematic, 100, 64, -50);
                placement.mirror(mirror);
                placement.turn(turns);
                assertThat(placement.sizeX()).isEqualTo(turns % 2 == 0 ? 4 : 3);
                assertThat(placement.sizeZ()).isEqualTo(turns % 2 == 0 ? 3 : 4);
                Set<String> places = new HashSet<>();
                for (int y = 0; y < 2; y++) {
                    for (int z = 0; z < 3; z++) {
                        for (int x = 0; x < 4; x++) {
                            placement.toWorld(x, y, z, out);
                            assertThat(placement.contains(out[0], out[1], out[2]))
                                    .isTrue();
                            assertThat(places.add(out[0] + "," + out[1] + "," + out[2]))
                                    .isTrue();
                            assertThat(placement.indexAt(out[0], out[1], out[2]))
                                    .isEqualTo(schematic.at(x, y, z));
                        }
                    }
                }
            }
        }
    }

    @Test
    void outsideTheBoxNothingIsWanted() throws SchematicException {
        Placement placement = new Placement(numbered(), 0, 0, 0);
        placement.turn(1);
        assertThat(placement.indexAt(-1, 0, 0)).isZero();
        assertThat(placement.indexAt(3, 0, 0)).isZero();
        assertThat(placement.indexAt(0, 2, 0)).isZero();
        assertThat(placement.indexAt(0, 0, 4)).isZero();
        assertThat(placement.indexAt(2, 1, 3)).isNotZero();
    }

    /**
     * A block that faces the box's west end, at the west end: turned or mirrored, it must still face
     * the end it's at, so the position's turn and the state's agree.
     */
    @Test
    void blocksTurnTheWayTheirPlacesDo() throws SchematicException {
        FakeBlocks blocks = new FakeBlocks();
        Schematic.Builder builder = new Schematic.Builder(3, 1, 1);
        builder.set(0, 0, 0, builder.index(BlockSpec.of("minecraft:stairs", Map.of("facing", "west"))));
        Schematic schematic = builder.build();
        int west = blocks.state("minecraft:stairs[facing=west]");
        int[] out = new int[3];
        for (Mirror mirror : Mirror.values()) {
            for (int turns = 0; turns < 4; turns++) {
                Placement placement = new Placement(schematic, 0, 0, 0);
                placement.mirror(mirror);
                placement.turn(turns);
                placement.toWorld(0, 0, 0, out);
                String facing = blocks.text(blocks.transform(west, mirror, turns));
                String end;
                if (placement.sizeX() > 1) {
                    end = out[0] == 0 ? "west" : "east";
                } else {
                    end = out[2] == 0 ? "north" : "south";
                }
                assertThat(facing).as("%s, %d turns", mirror, turns).isEqualTo("minecraft:stairs[facing=" + end + "]");
            }
        }
    }
}
