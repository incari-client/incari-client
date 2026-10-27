package incari.features.schematics.format;

/**
 * Small non-negative numbers packed tightly into longs, {@code bits} each, low bits first; an entry
 * may run on into the next long. This is how a {@code .litematic} keeps a region's blocks, so its
 * array is read as it is, and every format's blocks are kept this way: a few bits a block, not an
 * int.
 */
public final class PackedArray {

    private final int bits;
    private final long mask;
    private final int length;
    private final long[] data;

    /** {@code length} zeros of {@code bits} bits each (1 to 32). */
    public PackedArray(int bits, int length) {
        this(bits, length, new long[longsFor(bits, length)]);
    }

    /**
     * Over {@code data} as it is.
     *
     * @throws IllegalArgumentException if {@code data} is too short for {@code length} entries
     */
    public PackedArray(int bits, int length, long[] data) {
        if (bits < 1 || bits > 32 || length < 0) {
            throw new IllegalArgumentException("Bad packing: " + bits + " bits, " + length + " entries");
        }
        if (data.length < longsFor(bits, length)) {
            throw new IllegalArgumentException(
                    "Too few longs: " + data.length + " for " + length + " entries of " + bits + " bits");
        }
        this.bits = bits;
        this.mask = (1L << bits) - 1;
        this.length = length;
        this.data = data;
    }

    /** How many longs {@code length} entries of {@code bits} take. */
    public static int longsFor(int bits, int length) {
        return (int) (((long) bits * length + 63) / 64);
    }

    /** The fewest bits that hold {@code 0} to {@code count - 1}; at least {@code min}. */
    public static int bitsFor(int count, int min) {
        int bits = 32 - Integer.numberOfLeadingZeros(Math.max(0, count - 1));
        return Math.max(min, Math.max(1, bits));
    }

    /** How many entries. */
    public int length() {
        return length;
    }

    /** Bits per entry. */
    public int bits() {
        return bits;
    }

    /** Entry {@code index}. Hot path: no allocation, no bounds check past the array's own. */
    public int get(int index) {
        long bit = (long) index * bits;
        int word = (int) (bit >>> 6);
        int offset = (int) (bit & 63);
        long value = data[word] >>> offset;
        if (offset + bits > 64) {
            value |= data[word + 1] << (64 - offset);
        }
        return (int) (value & mask);
    }

    /** Sets entry {@code index} to {@code value}, which must fit in {@link #bits}. */
    public void set(int index, int value) {
        long bit = (long) index * bits;
        int word = (int) (bit >>> 6);
        int offset = (int) (bit & 63);
        long v = value & mask;
        data[word] = (data[word] & ~(mask << offset)) | (v << offset);
        if (offset + bits > 64) {
            int spill = 64 - offset;
            data[word + 1] = (data[word + 1] & ~(mask >>> spill)) | (v >>> spill);
        }
    }

    /** A copy with {@code newBits} bits an entry, for values that outgrew these. */
    public PackedArray widened(int newBits) {
        PackedArray wider = new PackedArray(newBits, length);
        for (int i = 0; i < length; i++) {
            wider.set(i, get(i));
        }
        return wider;
    }
}
