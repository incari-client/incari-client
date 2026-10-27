package incari.features.schematics.format;

import java.io.ByteArrayOutputStream;
import java.io.DataOutputStream;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.zip.GZIPOutputStream;

/**
 * Writes NBT for the tests, to make schematic files of known content: a {@code Map} is a compound,
 * a {@code List} a list, {@code byte[]}/{@code int[]}/{@code long[]} arrays, boxed numbers and text
 * as themselves.
 */
public final class NbtWriter {

    private NbtWriter() {}

    /** A compound from key, value pairs, in order. */
    public static Map<String, Object> compound(Object... pairs) {
        Map<String, Object> map = new LinkedHashMap<>();
        for (int i = 0; i < pairs.length; i += 2) {
            map.put((String) pairs[i], pairs[i + 1]);
        }
        return map;
    }

    /** {@code root}, gzipped as the game writes files. */
    public static byte[] gzipped(Map<String, Object> root) {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        try (DataOutputStream out = new DataOutputStream(new GZIPOutputStream(bytes))) {
            write(out, root);
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
        return bytes.toByteArray();
    }

    /** {@code root}, not packed. */
    public static byte[] plain(Map<String, Object> root) {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        try (DataOutputStream out = new DataOutputStream(bytes)) {
            write(out, root);
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
        return bytes.toByteArray();
    }

    private static void write(DataOutputStream out, Map<String, Object> root) throws IOException {
        out.writeByte(10);
        out.writeUTF("");
        body(out, root);
    }

    private static int type(Object value) {
        return switch (value) {
            case Byte b -> 1;
            case Short s -> 2;
            case Integer i -> 3;
            case Long l -> 4;
            case Float f -> 5;
            case Double d -> 6;
            case byte[] b -> 7;
            case String s -> 8;
            case List<?> l -> 9;
            case Map<?, ?> m -> 10;
            case int[] i -> 11;
            case long[] l -> 12;
            default -> throw new IllegalArgumentException("Not NBT: " + value);
        };
    }

    @SuppressWarnings("unchecked")
    private static void body(DataOutputStream out, Object value) throws IOException {
        switch (value) {
            case Byte b -> out.writeByte(b);
            case Short s -> out.writeShort(s);
            case Integer i -> out.writeInt(i);
            case Long l -> out.writeLong(l);
            case Float f -> out.writeFloat(f);
            case Double d -> out.writeDouble(d);
            case byte[] bytes -> {
                out.writeInt(bytes.length);
                out.write(bytes);
            }
            case String s -> out.writeUTF(s);
            case List<?> list -> {
                out.writeByte(list.isEmpty() ? 0 : type(list.get(0)));
                out.writeInt(list.size());
                for (Object element : list) {
                    body(out, element);
                }
            }
            case Map<?, ?> map -> {
                for (Map.Entry<String, Object> entry : ((Map<String, Object>) map).entrySet()) {
                    out.writeByte(type(entry.getValue()));
                    out.writeUTF(entry.getKey());
                    body(out, entry.getValue());
                }
                out.writeByte(0);
            }
            case int[] ints -> {
                out.writeInt(ints.length);
                for (int i : ints) {
                    out.writeInt(i);
                }
            }
            case long[] longs -> {
                out.writeInt(longs.length);
                for (long l : longs) {
                    out.writeLong(l);
                }
            }
            default -> throw new IllegalArgumentException("Not NBT: " + value);
        }
    }

    /**
     * Packs {@code values} tightly, {@code bits} each, low bits first, by single bits: an oracle
     * written apart from {@link PackedArray}.
     */
    public static long[] pack(int[] values, int bits) {
        long[] out = new long[(int) (((long) values.length * bits + 63) / 64)];
        for (int i = 0; i < values.length; i++) {
            for (int b = 0; b < bits; b++) {
                if ((values[i] >>> b & 1) != 0) {
                    long bit = (long) i * bits + b;
                    out[(int) (bit / 64)] |= 1L << (bit % 64);
                }
            }
        }
        return out;
    }

    /** A schematic's blocks as Sponge's variable-length bytes. */
    public static byte[] varints(int[] values) {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        for (int value : values) {
            int v = value;
            while ((v & ~0x7F) != 0) {
                out.write((v & 0x7F) | 0x80);
                v >>>= 7;
            }
            out.write(v);
        }
        return out.toByteArray();
    }
}
