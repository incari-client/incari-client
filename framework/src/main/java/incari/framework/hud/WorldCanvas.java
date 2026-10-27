package incari.framework.hud;

/**
 * What {@link incari.framework.event.Events#WORLD}'s listeners draw into the world itself, with
 * its blocks in front of and behind it. Render thread, while the frame's world is put together.
 */
public interface WorldCanvas {

    /**
     * A beacon's beam in {@code rgb} up from {@code x, y, z} (the block's corner, as a beacon's
     * block position) into the sky. A beam farther than the world is drawn is moved in along the
     * line of sight to where it's drawn, looking the same: the player sees it from any distance.
     */
    void beam(int x, int y, int z, int rgb);

    /**
     * {@code ghosts}' blocks as the game draws them, see-through at {@code alpha} (0 to 255) and
     * fully lit, within {@code range} blocks of the camera. The client keeps each section's mesh
     * between frames ({@link BlockGhosts#revision}) and meshes a few changed sections a frame, nearest
     * first; hand it the same object each frame.
     */
    void ghosts(BlockGhosts ghosts, int alpha, int range);

    /**
     * A box's edges in {@code argb}, its faces filled in {@code fillArgb} (0 for none), from block
     * corner {@code minX, minY, minZ} to {@code maxX, maxY, maxZ} (one past the last block). Behind
     * blocks it's hidden, as the world's other things.
     */
    void box(int minX, int minY, int minZ, int maxX, int maxY, int maxZ, int argb, int fillArgb);

    /** The camera's position in the world this frame: x. */
    double cameraX();

    /** The camera's y. */
    double cameraY();

    /** The camera's z. */
    double cameraZ();
}
