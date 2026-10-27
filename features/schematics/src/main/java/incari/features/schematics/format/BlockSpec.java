package incari.features.schematics.format;

import java.util.Collections;
import java.util.Map;
import java.util.TreeMap;

/**
 * A block state as a file names it: {@code minecraft:oak_stairs} and its properties ({@code
 * facing=north}), not yet looked up in the game. Properties are kept sorted, so the same state
 * is equal however a file ordered them.
 *
 * @param name the block's id, with its namespace
 * @param properties its properties, sorted by name; empty for the block's defaults
 */
public record BlockSpec(String name, Map<String, String> properties) {

    /** No block wanted: a structure's void, outside a schematic's regions. Never looked up. */
    public static final BlockSpec NOTHING = new BlockSpec("", Map.of());

    /** Air. */
    public static final BlockSpec AIR = new BlockSpec("minecraft:air", Map.of());

    public BlockSpec {
        properties = properties.isEmpty() ? Map.of() : Collections.unmodifiableMap(new TreeMap<>(properties));
    }

    /** {@code name} with the {@code minecraft:} namespace if it has none. */
    public static BlockSpec of(String name, Map<String, String> properties) {
        String id = name.strip();
        if (!id.isEmpty() && id.indexOf(':') < 0) {
            id = "minecraft:" + id;
        }
        return new BlockSpec(id, properties);
    }

    /**
     * Parses the game's text for a state, {@code minecraft:oak_stairs[facing=north,half=bottom]}, as
     * Sponge schematics keep their palette. Anything after the closing bracket is ignored.
     */
    public static BlockSpec parse(String text) {
        int open = text.indexOf('[');
        if (open < 0) {
            return of(text, Map.of());
        }
        int close = text.indexOf(']', open);
        String inner = text.substring(open + 1, close < 0 ? text.length() : close);
        Map<String, String> properties = new TreeMap<>();
        int start = 0;
        while (start <= inner.length()) {
            int comma = inner.indexOf(',', start);
            String pair = inner.substring(start, comma < 0 ? inner.length() : comma);
            int equals = pair.indexOf('=');
            if (equals > 0) {
                properties.put(
                        pair.substring(0, equals).strip(),
                        pair.substring(equals + 1).strip());
            }
            if (comma < 0) {
                break;
            }
            start = comma + 1;
        }
        return of(text.substring(0, open), properties);
    }

    /** Whether it's air of any kind. */
    public boolean isAir() {
        return name.equals("minecraft:air") || name.equals("minecraft:cave_air") || name.equals("minecraft:void_air");
    }

    /** Whether it stands for no block wanted ({@link #NOTHING}). */
    public boolean isNothing() {
        return name.isEmpty();
    }

    /** One property's value, or {@code fallback} if it has none. */
    public String property(String key, String fallback) {
        String value = properties.get(key);
        return value == null ? fallback : value;
    }

    /** The game's text for it. */
    @Override
    public String toString() {
        if (properties.isEmpty()) {
            return name;
        }
        StringBuilder text = new StringBuilder(name).append('[');
        boolean first = true;
        for (Map.Entry<String, String> property : properties.entrySet()) {
            if (!first) {
                text.append(',');
            }
            first = false;
            text.append(property.getKey()).append('=').append(property.getValue());
        }
        return text.append(']').toString();
    }
}
