package incari.features.schematics.format;

import java.util.List;

/**
 * Reads the game's own structure files ({@code .nbt}, a structure block's): a size, a palette (or
 * several, of which the first is used, as the game does for shipwrecks) and a list of the blocks
 * there are, each with its position and palette number. A position not listed is the structure's
 * void: nothing wanted there.
 */
final class StructureReader {

    private StructureReader() {}

    /** Whether {@code root} looks like one. */
    static boolean accepts(Nbt.Compound root) {
        return root.has("size") && root.has("blocks");
    }

    static Schematic read(Nbt.Compound root, String name) throws SchematicException {
        int[] size = root.ints("size");
        if (size == null || size.length != 3) {
            throw new SchematicException("It has no size");
        }
        List<Object> palette = root.list("palette");
        if (palette.isEmpty() && !root.list("palettes").isEmpty()) {
            palette = root.list("palettes").get(0) instanceof List<?> first ? copy(first) : List.of();
        }
        if (palette.isEmpty()) {
            throw new SchematicException("It has no palette");
        }
        Schematic.Builder builder = new Schematic.Builder(size[0], size[1], size[2]);
        builder.describe(name, root.string("author", ""), "Structure", root.integer("DataVersion", 0));
        int[] indices = new int[palette.size()];
        for (int i = 0; i < indices.length; i++) {
            indices[i] = palette.get(i) instanceof Nbt.Compound entry ? builder.index(LitematicReader.spec(entry)) : 0;
        }
        for (Object element : root.list("blocks")) {
            if (!(element instanceof Nbt.Compound block)) {
                continue;
            }
            int[] pos = block.ints("pos");
            int state = block.integer("state", -1);
            if (pos == null || pos.length != 3 || state < 0 || state >= indices.length) {
                throw new SchematicException("A block is broken");
            }
            if (!builder.contains(pos[0], pos[1], pos[2])) {
                throw new SchematicException("A block is outside its size");
            }
            builder.set(pos[0], pos[1], pos[2], indices[state]);
        }
        return builder.build();
    }

    private static List<Object> copy(List<?> list) {
        return List.copyOf(list);
    }
}
