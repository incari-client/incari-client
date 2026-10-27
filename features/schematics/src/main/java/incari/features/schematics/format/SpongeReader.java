package incari.features.schematics.format;

import org.jspecify.annotations.Nullable;

/**
 * Reads a Sponge schematic ({@code .schem}, WorldEdit's and Axiom's), versions 1 to 3: a palette of
 * the game's state text ({@code minecraft:oak_stairs[facing=north]}) to numbers, and the blocks as
 * those numbers in variable-length bytes, in the order {@code (y * length + z) * width + x}. Version
 * 3 moves both into a {@code Blocks} compound and the whole under a {@code Schematic} one.
 */
final class SpongeReader {

    /** What version 1 files were written at, as they don't say: 1.13.2. */
    private static final int VERSION_1_DATA = 1631;

    private SpongeReader() {}

    /** The compound holding the schematic: the root, or version 3's {@code Schematic} in it; null if neither. */
    static Nbt.@Nullable Compound body(Nbt.Compound root) {
        Nbt.Compound inner = root.compound("Schematic");
        if (inner != null && inner.has("Width")) {
            return inner;
        }
        return root.has("Width") && root.has("Palette") ? root : null;
    }

    static Schematic read(Nbt.Compound body, String fallbackName) throws SchematicException {
        int width = unsigned(body.integer("Width", 0));
        int height = unsigned(body.integer("Height", 0));
        int length = unsigned(body.integer("Length", 0));
        Nbt.Compound blocks = body.compound("Blocks");
        Nbt.Compound palette = blocks != null ? blocks.compound("Palette") : body.compound("Palette");
        byte[] data = blocks != null ? blocks.bytes("Data") : body.bytes("BlockData");
        if (palette == null || data == null) {
            throw new SchematicException("It has no palette or blocks");
        }
        Schematic.Builder builder = new Schematic.Builder(width, height, length);
        Nbt.Compound metadata = body.compound("Metadata");
        String name = metadata == null ? "" : metadata.string("Name", "").strip();
        int version = body.integer("Version", 1);
        if (version < 1 || version > 3) {
            throw new SchematicException("Sponge version " + version + " isn't known");
        }
        builder.describe(
                name.isEmpty() ? fallbackName : name,
                metadata == null ? "" : metadata.string("Author", ""),
                "Sponge",
                body.integer("DataVersion", VERSION_1_DATA));

        int highest = -1;
        for (String key : palette.names()) {
            highest = Math.max(highest, palette.integer(key, -1));
        }
        if (highest < 0 || highest > 1 << 20) {
            throw new SchematicException("Its palette is empty or broken");
        }
        int[] indices = new int[highest + 1];
        for (String key : palette.names()) {
            int number = palette.integer(key, -1);
            if (number >= 0) {
                indices[number] = builder.index(BlockSpec.parse(key));
            }
        }

        int at = 0;
        for (int y = 0; y < height; y++) {
            for (int z = 0; z < length; z++) {
                for (int x = 0; x < width; x++) {
                    // A variable-length number: seven bits a byte, low first, the top bit for "more".
                    int value = 0;
                    int shift = 0;
                    while (true) {
                        if (at >= data.length) {
                            throw new SchematicException("Its blocks are cut short");
                        }
                        int b = data[at++];
                        value |= (b & 0x7F) << shift;
                        if ((b & 0x80) == 0) {
                            break;
                        }
                        shift += 7;
                        if (shift > 28) {
                            throw new SchematicException("Its blocks are broken");
                        }
                    }
                    if (value > highest) {
                        throw new SchematicException("Its blocks are past its palette");
                    }
                    builder.set(x, y, z, indices[value]);
                }
            }
        }
        return builder.build();
    }

    /** Sizes are unsigned shorts: a 40000-long schematic reads as negative. */
    private static int unsigned(int size) {
        return size < 0 ? size & 0xFFFF : size;
    }
}
