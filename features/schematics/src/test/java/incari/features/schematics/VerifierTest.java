package incari.features.schematics;

import static org.assertj.core.api.Assertions.assertThat;

import incari.features.schematics.format.BlockSpec;
import incari.features.schematics.format.Schematic;
import incari.features.schematics.format.SchematicException;
import incari.framework.game.Blocks;
import incari.framework.testing.FakeBlocks;
import java.util.Map;
import org.junit.jupiter.api.Test;

class VerifierTest {

    private final FakeBlocks blocks = new FakeBlocks();

    /** A row of five along x at y 64: stone, stone, oak stairs facing north, air, planks. */
    private Placement row() throws SchematicException {
        Schematic.Builder builder = new Schematic.Builder(5, 1, 1);
        int stone = builder.index(BlockSpec.of("minecraft:stone", Map.of()));
        builder.set(0, 0, 0, stone);
        builder.set(1, 0, 0, stone);
        builder.set(2, 0, 0, builder.index(BlockSpec.of("minecraft:oak_stairs", Map.of("facing", "north"))));
        builder.set(3, 0, 0, builder.index(BlockSpec.AIR));
        builder.set(4, 0, 0, builder.index(BlockSpec.of("minecraft:oak_planks", Map.of())));
        return new Placement(builder.build(), 10, 64, 20);
    }

    private int[] states(Schematic schematic) {
        int[] states = new int[schematic.palette().size()];
        for (int i = 0; i < states.length; i++) {
            BlockSpec spec = schematic.palette().get(i);
            states[i] = spec.isNothing() ? Blocks.UNKNOWN : blocks.state(spec.name(), spec.properties(), 0);
        }
        return states;
    }

    @Test
    void eachKindOfProblemIsCountedAndListed() throws SchematicException {
        Placement placement = row();
        Verifier verifier = new Verifier(placement, states(placement.schematic), blocks);
        blocks.set(10, 64, 20, "minecraft:stone");
        // 11: missing. 12: stairs the wrong way. 13: dirt where air is wanted. 14: glass, not planks.
        blocks.set(12, 64, 20, "minecraft:oak_stairs[facing=south]");
        blocks.set(13, 64, 20, "minecraft:dirt");
        blocks.set(14, 64, 20, "minecraft:glass");
        verifier.tick(12, 64, 20, 48);

        assertThat(verifier.right()).isEqualTo(1);
        assertThat(verifier.missing()).isEqualTo(1);
        assertThat(verifier.wrongStates()).isEqualTo(1);
        assertThat(verifier.extra()).isEqualTo(1);
        assertThat(verifier.wrongBlocks()).isEqualTo(1);
        assertThat(verifier.lookedAtAll()).isTrue();

        Verifier.Section section = verifier.section(0, 4, 1);
        assertThat(section).isNotNull();
        assertThat(section.issueCount).isEqualTo(3);
        int stairs = (0 << 8 | 4 << 4 | 12) << 3 | Verifier.WRONG_STATE;
        int dirt = (0 << 8 | 4 << 4 | 13) << 3 | Verifier.EXTRA;
        int glass = (0 << 8 | 4 << 4 | 14) << 3 | Verifier.WRONG_BLOCK;
        assertThat(new int[] {section.issue(0), section.issue(1), section.issue(2)})
                .containsExactly(stairs, dirt, glass);
    }

    @Test
    void buildingNearbyShowsWithinAFewTicksAndChangesTheRevision() throws SchematicException {
        Placement placement = row();
        Verifier verifier = new Verifier(placement, states(placement.schematic), blocks);
        verifier.tick(12, 64, 20, 48);
        assertThat(verifier.missing()).isEqualTo(4);
        Verifier.Section section = verifier.section(0, 4, 1);
        assertThat(section).isNotNull();
        long before = section.revision;

        blocks.set(11, 64, 20, "minecraft:stone");
        for (int i = 0; i < Verifier.NEAR_TICKS; i++) {
            verifier.tick(12, 64, 20, 48);
        }
        assertThat(verifier.missing()).isEqualTo(3);
        assertThat(verifier.right()).isEqualTo(1);
        assertThat(verifier.rightOf(placement.schematic.at(1, 0, 0))).isEqualTo(1);
        assertThat(section.revision).isNotEqualTo(before);
    }

    @Test
    void anUnloadedChunkCountsAsNothingYet() throws SchematicException {
        Placement placement = row();
        Verifier verifier = new Verifier(placement, states(placement.schematic), blocks);
        blocks.loaded = false;
        verifier.tick(12, 64, 20, 48);
        assertThat(verifier.missing() + verifier.right() + verifier.extra()).isZero();
        blocks.loaded = true;
        for (int i = 0; i < Verifier.NEAR_TICKS; i++) {
            verifier.tick(12, 64, 20, 48);
        }
        assertThat(verifier.missing()).isEqualTo(4);
    }

    @Test
    void aTickLooksAtNoMoreThanItsBudget() throws SchematicException {
        // 96 x 32 x 96: 300,000 blocks, far past a tick's looking.
        Schematic.Builder builder = new Schematic.Builder(96, 32, 96);
        int stone = builder.index(BlockSpec.of("minecraft:stone", Map.of()));
        for (int y = 0; y < 32; y++) {
            for (int z = 0; z < 96; z++) {
                for (int x = 0; x < 96; x++) {
                    builder.set(x, y, z, stone);
                }
            }
        }
        Placement placement = new Placement(builder.build(), 0, 0, 0);
        Verifier verifier = new Verifier(placement, states(placement.schematic), blocks);
        blocks.reads = 0;
        verifier.tick(48, 16, 48, 32);
        assertThat(blocks.reads).isLessThanOrEqualTo(Verifier.BUDGET + 4096);
        for (int i = 0; i < 200; i++) {
            verifier.tick(48, 16, 48, 32);
        }
        assertThat(verifier.lookedAtAll()).isTrue();
        assertThat(verifier.missing()).isEqualTo(96 * 32 * 96);
    }
}
