package incari.features.schematics.format;

import static incari.features.schematics.format.NbtWriter.compound;
import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import java.io.IOException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.Random;
import org.junit.jupiter.api.Test;

class FormatTest {

    @Test
    void packedEntriesRunOnIntoTheNextLong() {
        Random random = new Random(66);
        for (int bits = 1; bits <= 32; bits++) {
            int[] values = new int[300];
            for (int i = 0; i < values.length; i++) {
                values[i] = (int) (random.nextLong() & ((1L << bits) - 1));
            }
            PackedArray read = new PackedArray(bits, values.length, NbtWriter.pack(values, bits));
            PackedArray written = new PackedArray(bits, values.length);
            for (int i = 0; i < values.length; i++) {
                assertThat(read.get(i)).as("bits %d, entry %d", bits, i).isEqualTo(values[i]);
                written.set(i, values[i]);
            }
            for (int i = 0; i < values.length; i++) {
                assertThat(written.get(i)).isEqualTo(values[i]);
            }
        }
    }

    @Test
    void bitsAreAtLeastTwoAndEnoughForThePalette() {
        assertThat(PackedArray.bitsFor(1, 2)).isEqualTo(2);
        assertThat(PackedArray.bitsFor(4, 2)).isEqualTo(2);
        assertThat(PackedArray.bitsFor(5, 2)).isEqualTo(3);
        assertThat(PackedArray.bitsFor(17, 2)).isEqualTo(5);
        assertThatThrownBy(() -> new PackedArray(5, 100, new long[2])).isInstanceOf(IllegalArgumentException.class);
    }

    @Test
    void statesParseAsTheGameWritesThem() {
        BlockSpec stairs = BlockSpec.parse("minecraft:oak_stairs[half=bottom,facing=north]");
        assertThat(stairs.name()).isEqualTo("minecraft:oak_stairs");
        assertThat(stairs.properties()).containsExactly(Map.entry("facing", "north"), Map.entry("half", "bottom"));
        assertThat(stairs.toString()).isEqualTo("minecraft:oak_stairs[facing=north,half=bottom]");
        assertThat(BlockSpec.parse("stone")).isEqualTo(new BlockSpec("minecraft:stone", Map.of()));
        assertThat(BlockSpec.parse("minecraft:cave_air").isAir()).isTrue();
    }

    @Test
    void nbtReadsPackedOrNotAndRefusesBrokenFiles() throws IOException {
        Map<String, Object> root = compound("a", 1, "b", "text", "c", List.of(1, 2, 3), "d", new long[] {5L});
        for (byte[] file : List.of(NbtWriter.plain(root), NbtWriter.gzipped(root))) {
            Nbt.Compound read = Nbt.read(file);
            assertThat(read.integer("a", 0)).isEqualTo(1);
            assertThat(read.string("b", "")).isEqualTo("text");
            assertThat(read.ints("c")).containsExactly(1, 2, 3);
            assertThat(read.longs("d")).containsExactly(5L);
        }
        byte[] whole = NbtWriter.plain(root);
        assertThatThrownBy(() -> Nbt.read(Arrays.copyOf(whole, whole.length - 3)))
                .isInstanceOf(IOException.class);
        // A long array claiming two billion entries, in a file of a few bytes.
        byte[] lying = NbtWriter.plain(compound("d", new long[] {1L}));
        lying[lying.length - 13] = 0x7F;
        assertThatThrownBy(() -> Nbt.read(lying)).isInstanceOf(IOException.class);
    }

    /** A region of {@code size} filled with palette indices {@code 0, 1, 2...} in turn. */
    private static Map<String, Object> region(int[] position, int[] size, List<String> palette) {
        int volume = Math.abs(size[0] * size[1] * size[2]);
        int[] values = new int[volume];
        for (int i = 0; i < volume; i++) {
            values[i] = i % palette.size();
        }
        List<Object> entries = new ArrayList<>();
        for (String name : palette) {
            BlockSpec spec = BlockSpec.parse(name);
            entries.add(
                    spec.properties().isEmpty()
                            ? compound("Name", spec.name())
                            : compound("Name", spec.name(), "Properties", Map.copyOf(spec.properties())));
        }
        return compound(
                "Position", compound("x", position[0], "y", position[1], "z", position[2]),
                "Size", compound("x", size[0], "y", size[1], "z", size[2]),
                "BlockStatePalette", entries,
                "BlockStates", NbtWriter.pack(values, PackedArray.bitsFor(palette.size(), 2)),
                "TileEntities", List.of(),
                "Entities", List.of());
    }

    @Test
    void litematicRegionsMergeIntoOneBoxWithNothingBetween() throws SchematicException {
        List<String> five = List.of(
                "minecraft:air",
                "minecraft:stone",
                "minecraft:oak_planks",
                "minecraft:glass",
                "minecraft:oak_log[axis=x]");
        Map<String, Object> root = compound(
                "Version",
                6,
                "MinecraftDataVersion",
                3700,
                "Metadata",
                compound("Name", "Hut", "Author", "Sam", "EnclosingSize", compound("x", 5, "y", 2, "z", 3)),
                "Regions",
                compound(
                        "main", region(new int[] {0, 0, 0}, new int[] {3, 2, 3}, five),
                        // Negative size: it runs from x 4 down to x 3.
                        "porch",
                                region(
                                        new int[] {4, 0, 0},
                                        new int[] {-2, 1, 1},
                                        List.of("minecraft:air", "minecraft:dirt"))));
        Schematic schematic = SchematicFiles.read("hut.litematic", NbtWriter.gzipped(root));
        assertThat(schematic.name()).isEqualTo("Hut");
        assertThat(schematic.author()).isEqualTo("Sam");
        assertThat(schematic.format()).isEqualTo("Litematic");
        assertThat(schematic.dataVersion()).isEqualTo(3700);
        assertThat(new int[] {schematic.sizeX(), schematic.sizeY(), schematic.sizeZ()})
                .containsExactly(5, 2, 3);
        // Main's blocks: index (y * 3 + z) * 3 + x into its five-entry palette, 3 bits each.
        for (int y = 0; y < 2; y++) {
            for (int z = 0; z < 3; z++) {
                for (int x = 0; x < 3; x++) {
                    String want = five.get(((y * 3 + z) * 3 + x) % 5);
                    assertThat(schematic.palette().get(schematic.at(x, y, z))).isEqualTo(BlockSpec.parse(want));
                }
            }
        }
        // The porch: air at x 3, dirt at x 4; nothing wanted where no region reaches.
        assertThat(schematic.palette().get(schematic.at(3, 0, 0)).isAir()).isTrue();
        assertThat(schematic.palette().get(schematic.at(4, 0, 0)).name()).isEqualTo("minecraft:dirt");
        assertThat(schematic.at(4, 1, 0)).isZero();
        assertThat(schematic.at(4, 0, 2)).isZero();
    }

    @Test
    void litematicWithTooFewLongsIsRefused() {
        Map<String, Object> region =
                region(new int[] {0, 0, 0}, new int[] {4, 4, 4}, List.of("minecraft:air", "minecraft:stone"));
        region.put("BlockStates", new long[1]);
        Map<String, Object> root = compound("Version", 6, "Regions", compound("r", region));
        assertThatThrownBy(() -> SchematicFiles.read("x.litematic", NbtWriter.gzipped(root)))
                .isInstanceOf(SchematicException.class)
                .hasMessageContaining("cut short");
    }

    private static int[] spongeBlocks(int width, int height, int length, int paletteSize) {
        int[] values = new int[width * height * length];
        for (int i = 0; i < values.length; i++) {
            values[i] = i % paletteSize;
        }
        return values;
    }

    /** A palette of air and 199 others, so numbers past 127 take two bytes. */
    private static Map<String, Object> spongePalette() {
        Map<String, Object> palette = compound("minecraft:air", 0);
        for (int i = 1; i < 200; i++) {
            palette.put("minecraft:wool_" + i + "[lit=" + (i % 2 == 0) + "]", i);
        }
        return palette;
    }

    @Test
    void spongeTwoAndThreeReadAlike() throws SchematicException {
        int[] values = spongeBlocks(9, 4, 7, 200);
        Map<String, Object> two = compound(
                "Version",
                2,
                "DataVersion",
                4440,
                "Width",
                (short) 9,
                "Height",
                (short) 4,
                "Length",
                (short) 7,
                "PaletteMax",
                200,
                "Palette",
                spongePalette(),
                "BlockData",
                NbtWriter.varints(values),
                "BlockEntities",
                List.of());
        Map<String, Object> three = compound(
                "Schematic",
                compound(
                        "Version",
                        3,
                        "DataVersion",
                        4556,
                        "Width",
                        (short) 9,
                        "Height",
                        (short) 4,
                        "Length",
                        (short) 7,
                        "Metadata",
                        compound("Name", "Arena"),
                        "Blocks",
                        compound(
                                "Palette",
                                spongePalette(),
                                "Data",
                                NbtWriter.varints(values),
                                "BlockEntities",
                                List.of())));
        Schematic a = SchematicFiles.read("arena.schem", NbtWriter.gzipped(two));
        Schematic b = SchematicFiles.read("whatever.schem", NbtWriter.gzipped(three));
        assertThat(a.name()).isEqualTo("arena");
        assertThat(b.name()).isEqualTo("Arena");
        assertThat(a.dataVersion()).isEqualTo(4440);
        for (int y = 0; y < 4; y++) {
            for (int z = 0; z < 7; z++) {
                for (int x = 0; x < 9; x++) {
                    int i = (y * 7 + z) * 9 + x;
                    String want = values[i] == 0
                            ? "minecraft:air"
                            : "minecraft:wool_" + values[i] + "[lit=" + (values[i] % 2 == 0) + "]";
                    assertThat(a.palette().get(a.at(x, y, z))).isEqualTo(BlockSpec.parse(want));
                    assertThat(b.palette().get(b.at(x, y, z))).isEqualTo(BlockSpec.parse(want));
                }
            }
        }
        assertThat(a.blockCount()).isEqualTo(values.length - (values.length + 199) / 200);
    }

    @Test
    void spongeCutShortIsRefused() {
        Map<String, Object> root = compound(
                "Version",
                2,
                "Width",
                (short) 4,
                "Height",
                (short) 4,
                "Length",
                (short) 4,
                "Palette",
                compound("minecraft:air", 0),
                "BlockData",
                new byte[10]);
        assertThatThrownBy(() -> SchematicFiles.read("x.schem", NbtWriter.gzipped(root)))
                .isInstanceOf(SchematicException.class)
                .hasMessageContaining("cut short");
    }

    @Test
    void structuresLeaveUnlistedBlocksAsVoidAndTakeTheNewNames() throws SchematicException {
        Map<String, Object> root = compound(
                "DataVersion", 5023,
                "size", List.of(2, 2, 2),
                "palette",
                        List.of(
                                compound("id", "minecraft:air"),
                                compound(
                                        "id",
                                        "minecraft:furnace",
                                        "properties",
                                        compound("facing", "east", "lit", "false"))),
                "blocks",
                        List.of(
                                compound("pos", List.of(0, 0, 0), "state", 1),
                                compound("pos", List.of(1, 1, 1), "state", 0)),
                "entities", List.of());
        Schematic schematic = SchematicFiles.read("camp.nbt", NbtWriter.gzipped(root));
        assertThat(schematic.format()).isEqualTo("Structure");
        assertThat(schematic.palette().get(schematic.at(0, 0, 0)))
                .isEqualTo(BlockSpec.parse("minecraft:furnace[facing=east,lit=false]"));
        assertThat(schematic.palette().get(schematic.at(1, 1, 1)).isAir()).isTrue();
        assertThat(schematic.at(1, 0, 0)).isZero();
        assertThat(schematic.blockCount()).isEqualTo(1);
    }

    @Test
    void oldAndUnknownFilesSayWhy() {
        Map<String, Object> mcedit = compound(
                "Width",
                (short) 1,
                "Height",
                (short) 1,
                "Length",
                (short) 1,
                "Blocks",
                new byte[1],
                "Materials",
                "Alpha");
        assertThatThrownBy(() -> SchematicFiles.read("old.schematic", NbtWriter.gzipped(mcedit)))
                .hasMessageContaining("before 1.13");
        assertThatThrownBy(() -> SchematicFiles.read("x.schem", new byte[] {1, 2, 3}))
                .isInstanceOf(SchematicException.class);
        Map<String, Object> huge = compound(
                "size",
                List.of(4096, 4096, 4096),
                "palette",
                List.of(compound("id", "minecraft:air")),
                "blocks",
                List.of());
        assertThatThrownBy(() -> SchematicFiles.read("big.nbt", NbtWriter.gzipped(huge)))
                .hasMessageContaining("Too big");
    }
}
