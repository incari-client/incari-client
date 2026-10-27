package incari.framework.game;

import java.util.Map;

/**
 * The game's block states and the world's blocks, as modules see them ({@link Game#blocks()}): a
 * state is an int the client hands out, the same for the same state while the game runs; never
 * saved. Game and render thread. Reads of the world must not block: a block in a chunk the client
 * hasn't loaded is {@link #UNKNOWN}.
 */
public interface Blocks {

    /** Air: no block. */
    int AIR = 0;

    /** No such state, or a block the client can't see (an unloaded chunk, out of the world's height). */
    int UNKNOWN = -1;

    /** No game: every state unknown, the world empty. */
    Blocks NONE = new Blocks() {
        @Override
        public int state(String name, Map<String, String> properties, int dataVersion) {
            return UNKNOWN;
        }

        @Override
        public int transform(int state, Mirror mirror, int clockwiseTurns) {
            return state;
        }

        @Override
        public int at(int x, int y, int z) {
            return UNKNOWN;
        }

        @Override
        public boolean isAir(int state) {
            return state == AIR;
        }

        @Override
        public int block(int state) {
            return state;
        }

        @Override
        public String itemId(int state) {
            return "";
        }

        @Override
        public String name(int state) {
            return "";
        }
    };

    /**
     * The state named {@code name} ({@code minecraft:oak_stairs}) with these {@code properties}
     * ({@code facing=north}), as saved by the game at {@code dataVersion} and upgraded to this one;
     * properties left out take the block's defaults. {@link #UNKNOWN} if the game has no such block.
     * Slow-ish: call it once per palette entry, not per block.
     */
    int state(String name, Map<String, String> properties, int dataVersion);

    /**
     * {@code state} mirrored, then turned {@code clockwiseTurns} quarter turns seen from above, as
     * the game turns a structure's blocks: stairs face the other way, logs lie along the other axis.
     */
    int transform(int state, Mirror mirror, int clockwiseTurns);

    /** The block at {@code x, y, z} in the player's world; {@link #UNKNOWN} where the client has none. */
    int at(int x, int y, int z);

    /** Whether {@code state} is air of any kind (cave air too). */
    boolean isAir(int state);

    /**
     * One number per block, whatever its state: two states are of the same block when these are
     * equal (oak stairs facing north and south).
     */
    int block(int state);

    /** The id of the item that places {@code state} ({@code minecraft:torch} for a wall torch); empty for none. */
    String itemId(int state);

    /** {@code state}'s block's name in the player's language ("Oak Stairs"). */
    String name(int state);
}
