package incari.features.schematics.format;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import org.jspecify.annotations.Nullable;

/**
 * Reads a {@code .litematic} (ADR 0066): gzipped NBT with one or more regions, each its own box
 * with its own palette, merged here into one box around them all. A region's size may be negative
 * on an axis: it then runs the other way from its position. Its blocks are palette indices packed
 * tightly into longs ({@link PackedArray}), {@code max(2, ceil(log2(paletteSize)))} bits each, in
 * the order {@code (y * sizeZ + z) * sizeX + x}. Where the regions don't reach, nothing is wanted.
 */
final class LitematicReader {

    private LitematicReader() {}

    /** Whether {@code root} looks like one. */
    static boolean accepts(Nbt.Compound root) {
        return root.compound("Regions") != null;
    }

    private record Region(int minX, int minY, int minZ, int sizeX, int sizeY, int sizeZ, Nbt.Compound tag) {}

    static Schematic read(Nbt.Compound root, String fallbackName) throws SchematicException {
        Nbt.Compound regions = root.compound("Regions");
        if (regions == null) {
            throw new SchematicException("Not a litematic: no regions");
        }
        List<Region> boxes = new ArrayList<>();
        long minX = Long.MAX_VALUE, minY = Long.MAX_VALUE, minZ = Long.MAX_VALUE;
        long maxX = Long.MIN_VALUE, maxY = Long.MIN_VALUE, maxZ = Long.MIN_VALUE;
        for (String name : regions.names()) {
            Nbt.Compound region = regions.compound(name);
            if (region == null) {
                continue;
            }
            int[] position = xyz(region.compound("Position"));
            int[] size = xyz(region.compound("Size"));
            if (position == null || size == null) {
                throw new SchematicException("Region " + name + " has no position or size");
            }
            if (size[0] == 0 || size[1] == 0 || size[2] == 0) {
                continue;
            }
            Region box = new Region(
                    low(position[0], size[0]),
                    low(position[1], size[1]),
                    low(position[2], size[2]),
                    Math.abs(size[0]),
                    Math.abs(size[1]),
                    Math.abs(size[2]),
                    region);
            if ((long) box.sizeX * box.sizeY * box.sizeZ > Schematic.MAX_VOLUME) {
                throw new SchematicException("Region " + name + " is too big to load");
            }
            boxes.add(box);
            minX = Math.min(minX, box.minX);
            minY = Math.min(minY, box.minY);
            minZ = Math.min(minZ, box.minZ);
            maxX = Math.max(maxX, (long) box.minX + box.sizeX);
            maxY = Math.max(maxY, (long) box.minY + box.sizeY);
            maxZ = Math.max(maxZ, (long) box.minZ + box.sizeZ);
        }
        if (boxes.isEmpty()) {
            throw new SchematicException("It has no blocks");
        }
        if (maxX - minX > Integer.MAX_VALUE || maxY - minY > Integer.MAX_VALUE || maxZ - minZ > Integer.MAX_VALUE) {
            throw new SchematicException("Too big to load");
        }
        Schematic.Builder builder =
                new Schematic.Builder((int) (maxX - minX), (int) (maxY - minY), (int) (maxZ - minZ));
        Nbt.Compound metadata = root.compound("Metadata");
        String name = metadata == null ? "" : metadata.string("Name", "").strip();
        builder.describe(
                name.isEmpty() ? fallbackName : name,
                metadata == null ? "" : metadata.string("Author", ""),
                "Litematic",
                root.integer("MinecraftDataVersion", 0));
        for (Region box : boxes) {
            region(builder, box, (int) (box.minX - minX), (int) (box.minY - minY), (int) (box.minZ - minZ));
        }
        return builder.build();
    }

    /** The lower end of a region's span on one axis: its position, or past it if its size is negative. */
    private static int low(int position, int size) {
        return size < 0 ? position + size + 1 : position;
    }

    private static void region(Schematic.Builder builder, Region box, int offsetX, int offsetY, int offsetZ)
            throws SchematicException {
        List<Object> entries = box.tag.list("BlockStatePalette");
        if (entries.isEmpty()) {
            throw new SchematicException("A region has no palette");
        }
        int[] indices = new int[entries.size()];
        for (int i = 0; i < indices.length; i++) {
            indices[i] = entries.get(i) instanceof Nbt.Compound entry ? builder.index(spec(entry)) : 0;
        }
        long[] states = box.tag.longs("BlockStates");
        int volume = box.sizeX * box.sizeY * box.sizeZ;
        int bits = PackedArray.bitsFor(indices.length, 2);
        if (states == null || states.length < PackedArray.longsFor(bits, volume)) {
            throw new SchematicException("A region's blocks are cut short");
        }
        PackedArray packed = new PackedArray(bits, volume, states);
        int i = 0;
        for (int y = 0; y < box.sizeY; y++) {
            for (int z = 0; z < box.sizeZ; z++) {
                for (int x = 0; x < box.sizeX; x++) {
                    int index = packed.get(i++);
                    if (index >= indices.length) {
                        throw new SchematicException("A region's blocks are past its palette");
                    }
                    builder.set(offsetX + x, offsetY + y, offsetZ + z, indices[index]);
                }
            }
        }
    }

    /** A palette entry: {@code Name} and {@code Properties}, or the newer {@code id} and {@code properties}. */
    static BlockSpec spec(Nbt.Compound entry) {
        String name = entry.string("Name", entry.string("id", "minecraft:air"));
        Nbt.Compound properties = entry.compound("Properties");
        if (properties == null) {
            properties = entry.compound("properties");
        }
        return BlockSpec.of(name, properties == null ? Map.of() : strings(properties));
    }

    private static Map<String, String> strings(Nbt.Compound properties) {
        Map<String, String> out = new TreeMap<>();
        for (String key : properties.names()) {
            Object value = properties.get(key);
            if (value != null) {
                out.put(key, value.toString());
            }
        }
        return out;
    }

    private static int @Nullable [] xyz(Nbt.@Nullable Compound tag) {
        if (tag == null || !tag.has("x") || !tag.has("y") || !tag.has("z")) {
            return null;
        }
        return new int[] {tag.integer("x", 0), tag.integer("y", 0), tag.integer("z", 0)};
    }
}
