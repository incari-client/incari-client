package incari.features.schematics.format;

import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.zip.GZIPInputStream;
import org.jspecify.annotations.Nullable;

/**
 * Reads the game's binary NBT, gzipped or not, into plain Java values: a {@link Compound}, a
 * {@code List<Object>}, {@code byte[]}, {@code int[]}, {@code long[]}, {@code String}, and the boxed
 * numbers. Every schematic format is NBT. A file is untrusted: sizes are checked against what's left
 * before anything is made, so a bad file fails with an {@link IOException} instead of running the
 * tab out of memory.
 */
public final class Nbt {

    /** The most a file may unpack to: a 4 GB Chromebook has to hold it and the game. */
    static final int MAX_BYTES = 192 << 20;

    private static final int MAX_DEPTH = 512;

    private final byte[] data;
    private final int end;
    private int at;

    private Nbt(byte[] data, int end) {
        this.data = data;
        this.end = end;
    }

    /** A named compound: its fields in file order. */
    public static final class Compound {

        private final Map<String, Object> fields = new LinkedHashMap<>();

        /** The field's value, or null if there's none. */
        public @Nullable Object get(String name) {
            return fields.get(name);
        }

        /** Whether there's a field by this name. */
        public boolean has(String name) {
            return fields.containsKey(name);
        }

        /** The field names, in file order. */
        public Iterable<String> names() {
            return fields.keySet();
        }

        /** The field's compound, or null if it's missing or isn't one. */
        public @Nullable Compound compound(String name) {
            return fields.get(name) instanceof Compound c ? c : null;
        }

        /** The field's list, or an empty one. */
        public List<Object> list(String name) {
            return fields.get(name) instanceof List<?> l ? cast(l) : List.of();
        }

        @SuppressWarnings("unchecked")
        private static List<Object> cast(List<?> list) {
            return (List<Object>) list;
        }

        /** The field's text, or {@code fallback} if it isn't text. */
        public String string(String name, String fallback) {
            return fields.get(name) instanceof String s ? s : fallback;
        }

        /** The field as a whole number of any width, or {@code fallback} if it isn't one. */
        public long number(String name, long fallback) {
            Object value = fields.get(name);
            if (value instanceof Byte || value instanceof Short || value instanceof Integer || value instanceof Long) {
                return ((Number) value).longValue();
            }
            return fallback;
        }

        /** {@link #number} as an int. */
        public int integer(String name, int fallback) {
            return (int) number(name, fallback);
        }

        /** The field's int array (or a list of ints, as structures keep positions), or null. */
        public int @Nullable [] ints(String name) {
            Object value = fields.get(name);
            if (value instanceof int[] ints) {
                return ints;
            }
            if (value instanceof List<?> list) {
                int[] out = new int[list.size()];
                for (int i = 0; i < out.length; i++) {
                    if (!(list.get(i) instanceof Integer n)) {
                        return null;
                    }
                    out[i] = n;
                }
                return out;
            }
            return null;
        }

        /** The field's long array, or null. */
        public long @Nullable [] longs(String name) {
            return fields.get(name) instanceof long[] longs ? longs : null;
        }

        /** The field's byte array, or null. */
        public byte @Nullable [] bytes(String name) {
            return fields.get(name) instanceof byte[] bytes ? bytes : null;
        }

        void put(String name, Object value) {
            fields.put(name, value);
        }
    }

    /**
     * The root compound of a file's bytes, unpacking them first if they're gzipped.
     *
     * @throws IOException if they aren't NBT, or unpack to more than a schematic should
     */
    public static Compound read(byte[] file) throws IOException {
        byte[] data = file;
        int length = file.length;
        if (file.length >= 2 && (file[0] & 0xFF) == 0x1F && (file[1] & 0xFF) == 0x8B) {
            Unpacked unpacked = gunzip(file);
            data = unpacked.bytes;
            length = unpacked.length;
        }
        Nbt reader = new Nbt(data, length);
        int type = reader.u8();
        if (type != 10) {
            throw new IOException("Not NBT: the root isn't a compound");
        }
        reader.utf();
        return reader.compound(0);
    }

    /** Unpacked bytes: the array, and how much of it is used. */
    private static final class Unpacked {

        final byte[] bytes;
        final int length;

        Unpacked(byte[] bytes, int length) {
            this.bytes = bytes;
            this.length = length;
        }
    }

    private static Unpacked gunzip(byte[] file) throws IOException {
        try (InputStream in = new GZIPInputStream(new ByteArrayInputStream(file), 64 * 1024)) {
            byte[] out = new byte[Math.max(1024, Math.min(MAX_BYTES, file.length * 4))];
            int length = 0;
            while (true) {
                if (length == out.length) {
                    if (out.length >= MAX_BYTES) {
                        throw new IOException("Too big: over " + (MAX_BYTES >> 20) + " MB unpacked");
                    }
                    out = Arrays.copyOf(out, (int) Math.min(MAX_BYTES, out.length * 2L));
                }
                int read = in.read(out, length, out.length - length);
                if (read < 0) {
                    return new Unpacked(out, length);
                }
                length += read;
            }
        }
    }

    private Object value(int type, int depth) throws IOException {
        return switch (type) {
            case 1 -> (byte) u8();
            case 2 -> (short) u16();
            case 3 -> s32();
            case 4 -> s64();
            case 5 -> Float.intBitsToFloat(s32());
            case 6 -> Double.longBitsToDouble(s64());
            case 7 -> {
                int n = count(1);
                byte[] bytes = Arrays.copyOfRange(data, at, at + n);
                at += n;
                yield bytes;
            }
            case 8 -> utf();
            case 9 -> list(depth);
            case 10 -> compound(depth);
            case 11 -> {
                int n = count(4);
                int[] ints = new int[n];
                for (int i = 0; i < n; i++) {
                    ints[i] = s32();
                }
                yield ints;
            }
            case 12 -> {
                int n = count(8);
                long[] longs = new long[n];
                for (int i = 0; i < n; i++) {
                    longs[i] = s64();
                }
                yield longs;
            }
            default -> throw new IOException("Bad NBT: tag type " + type);
        };
    }

    private Compound compound(int depth) throws IOException {
        if (depth > MAX_DEPTH) {
            throw new IOException("Bad NBT: nested too deep");
        }
        Compound compound = new Compound();
        while (true) {
            int type = u8();
            if (type == 0) {
                return compound;
            }
            String name = utf();
            compound.put(name, value(type, depth + 1));
        }
    }

    private List<Object> list(int depth) throws IOException {
        if (depth > MAX_DEPTH) {
            throw new IOException("Bad NBT: nested too deep");
        }
        int type = u8();
        // The smallest any element can be: an empty compound is one byte, an empty list five.
        int n = count(type == 0 ? 0 : 1);
        if (type == 0 && n > 0) {
            throw new IOException("Bad NBT: a list of nothing");
        }
        List<Object> list = new ArrayList<>(Math.min(n, 1 << 16));
        for (int i = 0; i < n; i++) {
            list.add(value(type, depth + 1));
        }
        return list;
    }

    /** A length prefix for {@code size}-byte elements, checked against what's left. */
    private int count(int size) throws IOException {
        int n = s32();
        if (n < 0 || (long) n * size > end - at) {
            throw new IOException("Bad NBT: " + n + " elements past the end");
        }
        return n;
    }

    private void need(int bytes) throws IOException {
        if (end - at < bytes) {
            throw new IOException("Bad NBT: ends early");
        }
    }

    private int u8() throws IOException {
        need(1);
        return data[at++] & 0xFF;
    }

    private int u16() throws IOException {
        need(2);
        int value = (data[at] & 0xFF) << 8 | (data[at + 1] & 0xFF);
        at += 2;
        return value;
    }

    private int s32() throws IOException {
        need(4);
        int value = (data[at] & 0xFF) << 24
                | (data[at + 1] & 0xFF) << 16
                | (data[at + 2] & 0xFF) << 8
                | (data[at + 3] & 0xFF);
        at += 4;
        return value;
    }

    private long s64() throws IOException {
        long high = s32() & 0xFFFFFFFFL;
        return high << 32 | (s32() & 0xFFFFFFFFL);
    }

    /** Java's modified UTF-8, as the game writes names and text; read as UTF-8, which it is for real text. */
    private String utf() throws IOException {
        int n = u16();
        need(n);
        String text = new String(data, at, n, StandardCharsets.UTF_8);
        at += n;
        return text;
    }
}
