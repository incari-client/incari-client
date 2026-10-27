package incari.framework.hud;

/**
 * Blocks a module wants drawn in the world as see-through ghosts ({@link WorldCanvas#ghosts}): what
 * a schematic still needs built. The client meshes them 16 blocks a side at a time (a section, as
 * the world's own: {@code x >> 4}) and keeps each mesh until {@link #revision} says it changed, so
 * a frame costs only drawing. Render thread.
 */
public interface BlockGhosts {

    /** The box the ghosts are in, in world blocks: the lowest corner, x. */
    int minX();

    /** The lowest corner's y. */
    int minY();

    /** The lowest corner's z. */
    int minZ();

    /** The highest corner, one past the last block: x. */
    int maxX();

    /** One past the last block's y. */
    int maxY();

    /** One past the last block's z. */
    int maxZ();

    /**
     * The {@link incari.framework.game.Blocks} state to draw a ghost of at {@code x, y, z};
     * {@link incari.framework.game.Blocks#AIR} for none. Called while a section is meshed.
     */
    int ghostAt(int x, int y, int z);

    /**
     * A number that changes whenever what {@link #ghostAt} gives in the section {@code sectionX,
     * sectionY, sectionZ} might have: the section is meshed again when it does.
     */
    long revision(int sectionX, int sectionY, int sectionZ);
}
