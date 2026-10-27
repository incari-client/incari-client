package incari.framework.testing;

import incari.framework.game.Blocks;
import incari.framework.game.Mirror;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

/**
 * The game's block states and a world of blocks, faked for feature tests ({@link TestGame#blocks}).
 * A state is its text ({@code minecraft:oak_stairs[facing=north]}, properties sorted) given a number
 * the first time it's seen; air is 0. Any block name is known but those in {@link #unknown}. Turning
 * and mirroring change a {@code facing} property as the game would, and nothing else.
 */
public final class FakeBlocks implements Blocks {

    private static final String[] TURNS = {"north", "east", "south", "west"};

    private final List<String> states = new ArrayList<>(List.of("minecraft:air"));
    private final Map<String, Integer> ids = new HashMap<>(Map.of("minecraft:air", AIR));

    /** The world's blocks by {@link #key}; anywhere else is air. */
    public final Map<Long, Integer> world = new HashMap<>();

    /** Block names the game doesn't have. */
    public final List<String> unknown = new ArrayList<>();

    /** Whether the world's chunks are loaded: off, every read is {@link #UNKNOWN}. */
    public boolean loaded = true;

    /** How many times {@link #at} was read. */
    public int reads;

    /** The state with this text, made if it's new. */
    public int state(String text) {
        Integer id = ids.get(text);
        if (id == null) {
            id = states.size();
            states.add(text);
            ids.put(text, id);
        }
        return id;
    }

    /** {@code state}'s text. */
    public String text(int state) {
        return state < 0 ? "unknown" : states.get(state);
    }

    /** Puts {@code text} at {@code x, y, z}. */
    public void set(int x, int y, int z, String text) {
        world.put(key(x, y, z), state(text));
    }

    /** One number per block position. */
    public static long key(int x, int y, int z) {
        return ((long) x & 0x3FFFFFF) << 38 | ((long) z & 0x3FFFFFF) << 12 | (y & 0xFFF);
    }

    @Override
    public int state(String name, Map<String, String> properties, int dataVersion) {
        if (unknown.contains(name)) {
            return UNKNOWN;
        }
        if (properties.isEmpty()) {
            return state(name);
        }
        StringBuilder text = new StringBuilder(name).append('[');
        boolean first = true;
        for (Map.Entry<String, String> property : new TreeMap<>(properties).entrySet()) {
            if (!first) {
                text.append(',');
            }
            first = false;
            text.append(property.getKey()).append('=').append(property.getValue());
        }
        return state(text.append(']').toString());
    }

    @Override
    public int transform(int state, Mirror mirror, int clockwiseTurns) {
        String text = text(state);
        int at = text.indexOf("facing=");
        if (at < 0) {
            return state;
        }
        int end = text.indexOf(',', at);
        if (end < 0) {
            end = text.indexOf(']', at);
        }
        String facing = text.substring(at + 7, end);
        int turn = List.of(TURNS).indexOf(facing);
        if (turn < 0) {
            return state;
        }
        if ((mirror == Mirror.X && turn % 2 == 1) || (mirror == Mirror.Z && turn % 2 == 0)) {
            turn += 2;
        }
        turn = Math.floorMod(turn + clockwiseTurns, 4);
        return state(text.substring(0, at + 7) + TURNS[turn] + text.substring(end));
    }

    @Override
    public int at(int x, int y, int z) {
        reads++;
        if (!loaded) {
            return UNKNOWN;
        }
        Integer state = world.get(key(x, y, z));
        return state == null ? AIR : state;
    }

    @Override
    public boolean isAir(int state) {
        return state == AIR;
    }

    @Override
    public int block(int state) {
        String text = text(state);
        int bracket = text.indexOf('[');
        return state(bracket < 0 ? text : text.substring(0, bracket));
    }

    @Override
    public String itemId(int state) {
        if (state <= AIR) {
            return "";
        }
        String text = text(state);
        int bracket = text.indexOf('[');
        return bracket < 0 ? text : text.substring(0, bracket);
    }

    @Override
    public String name(int state) {
        return itemId(state);
    }
}
