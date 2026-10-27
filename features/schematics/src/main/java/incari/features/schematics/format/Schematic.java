package incari.features.schematics.format;

import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * A structure to build, whatever file it came from: a box of blocks, each an index into the palette
 * of block states. Index 0 is always {@link BlockSpec#NOTHING}, no block wanted (outside a
 * {@code .litematic}'s regions, a structure's void); air in the palette is air wanted. Positions are
 * from the box's lowest corner, {@code 0} to {@code size - 1}. Read once, never changed.
 */
public final class Schematic {

    /**
     * The most blocks a box may hold, so a schematic fits a Chromebook's memory with the game: a few
     * bits each, a few tens of MB at most.
     */
    public static final long MAX_VOLUME = 1L << 25;

    private final String name;
    private final String author;
    private final String format;
    private final int dataVersion;
    private final int sizeX;
    private final int sizeY;
    private final int sizeZ;
    private final List<BlockSpec> palette;
    private final PackedArray blocks;
    private final int[] counts;

    private Schematic(Builder builder) {
        this.name = builder.name;
        this.author = builder.author;
        this.format = builder.format;
        this.dataVersion = builder.dataVersion;
        this.sizeX = builder.sizeX;
        this.sizeY = builder.sizeY;
        this.sizeZ = builder.sizeZ;
        this.palette = Collections.unmodifiableList(new ArrayList<>(builder.palette));
        this.blocks = builder.blocks;
        this.counts = new int[palette.size()];
        for (int i = 0; i < blocks.length(); i++) {
            counts[blocks.get(i)]++;
        }
    }

    /** Its name, as the file gives it or the file's name. */
    public String name() {
        return name;
    }

    /** Who made it; empty if the file doesn't say. */
    public String author() {
        return author;
    }

    /** Which kind of file it came from ("Litematic", "Sponge", "Structure"). */
    public String format() {
        return format;
    }

    /** The game data version its blocks were saved at; states older than the game's are upgraded. */
    public int dataVersion() {
        return dataVersion;
    }

    /** Its size along x. */
    public int sizeX() {
        return sizeX;
    }

    /** Its size along y. */
    public int sizeY() {
        return sizeY;
    }

    /** Its size along z. */
    public int sizeZ() {
        return sizeZ;
    }

    /** The block states it uses; index 0 is {@link BlockSpec#NOTHING}. */
    public List<BlockSpec> palette() {
        return palette;
    }

    /** The palette index at {@code x, y, z}, which must be in the box. Hot path. */
    public int at(int x, int y, int z) {
        return blocks.get((y * sizeZ + z) * sizeX + x);
    }

    /** How many blocks have palette index {@code index}. */
    public int count(int index) {
        return counts[index];
    }

    /** How many blocks it wants, air left out. */
    public int blockCount() {
        int total = 0;
        for (int i = 1; i < counts.length; i++) {
            if (!palette.get(i).isAir()) {
                total += counts[i];
            }
        }
        return total;
    }

    /** Builds a schematic block by block; any block not set is {@link BlockSpec#NOTHING}. */
    public static final class Builder {

        private final int sizeX;
        private final int sizeY;
        private final int sizeZ;
        private final List<BlockSpec> palette = new ArrayList<>();
        private final Map<BlockSpec, Integer> indices = new HashMap<>();
        private PackedArray blocks;
        private String name = "";
        private String author = "";
        private String format = "";
        private int dataVersion;

        /**
         * A box of this size.
         *
         * @throws SchematicException if it's empty or bigger than {@link #MAX_VOLUME}
         */
        public Builder(int sizeX, int sizeY, int sizeZ) throws SchematicException {
            if (sizeX <= 0 || sizeY <= 0 || sizeZ <= 0) {
                throw new SchematicException("It's empty: " + sizeX + " x " + sizeY + " x " + sizeZ);
            }
            long volume = (long) sizeX * sizeY * sizeZ;
            if (volume > MAX_VOLUME) {
                throw new SchematicException("Too big to load: " + sizeX + " x " + sizeY + " x " + sizeZ);
            }
            this.sizeX = sizeX;
            this.sizeY = sizeY;
            this.sizeZ = sizeZ;
            palette.add(BlockSpec.NOTHING);
            indices.put(BlockSpec.NOTHING, 0);
            blocks = new PackedArray(2, (int) volume);
        }

        /** Its name, author, format and data version. */
        public Builder describe(String name, String author, String format, int dataVersion) {
            this.name = name;
            this.author = author;
            this.format = format;
            this.dataVersion = dataVersion;
            return this;
        }

        /** {@code spec}'s palette index, added if it's new. */
        public int index(BlockSpec spec) {
            Integer index = indices.get(spec);
            if (index == null) {
                index = palette.size();
                palette.add(spec);
                indices.put(spec, index);
                int needed = PackedArray.bitsFor(palette.size(), 2);
                if (needed > blocks.bits()) {
                    blocks = blocks.widened(needed);
                }
            }
            return index;
        }

        /** Sets {@code x, y, z}, in the box, to palette index {@code index}. */
        public void set(int x, int y, int z, int index) {
            blocks.set((y * sizeZ + z) * sizeX + x, index);
        }

        /** Whether {@code x, y, z} is in the box. */
        public boolean contains(int x, int y, int z) {
            return x >= 0 && y >= 0 && z >= 0 && x < sizeX && y < sizeY && z < sizeZ;
        }

        /** The schematic. */
        public Schematic build() {
            return new Schematic(this);
        }
    }
}
