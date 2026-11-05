package incari.framework.waypoint;

import incari.framework.hud.Shape;
import java.util.List;
import org.jspecify.annotations.Nullable;

/**
 * A named mark at a block in one dimension of a world: its color and icon, whether it's shown,
 * and whether a beacon's beam rises from it. Changing one that's in a {@link Waypoints} saves it.
 * Game thread.
 */
public final class Waypoint {

    /** The colors the editor offers, {@code 0xRRGGBB}, in order: each new waypoint takes the next. */
    public static final List<Integer> COLORS = List.of(
            0xFF5A5A, 0xFF9F43, 0xFFD93D, 0x8BE04E, 0x3DDC97, 0x3DD6F5, 0x5B8CFF, 0x9D7BFF, 0xF06FD8, 0xFFFFFF,
            0xA9B4BF, 0x8B5E3C);

    /** The longest name, in characters. */
    public static final int MAX_NAME = 32;

    private String name;
    private int x;
    private int y;
    private int z;
    private String dimension;
    private int rgb;
    private Shape icon;
    private boolean visible = true;
    private boolean beam = true;
    private boolean death;
    private boolean temporary;
    private @Nullable Waypoints owner;

    /**
     * @param name what it's called (cut to {@link #MAX_NAME})
     * @param x the block's x
     * @param y the block's y
     * @param z the block's z
     * @param dimension the dimension's id, as {@code PlayerView.dimension()} gives it
     * @param rgb its color, {@code 0xRRGGBB}
     * @param icon its shape
     */
    public Waypoint(String name, int x, int y, int z, String dimension, int rgb, Shape icon) {
        this.name = clip(name);
        this.x = x;
        this.y = y;
        this.z = z;
        this.dimension = dimension;
        this.rgb = rgb & 0xFFFFFF;
        this.icon = icon;
    }

    /** A copy outside any list, for an editor to change before the player saves or cancels. */
    public Waypoint copy() {
        Waypoint copy = new Waypoint(name, x, y, z, dimension, rgb, icon);
        copy.visible = visible;
        copy.beam = beam;
        copy.death = death;
        copy.temporary = temporary;
        return copy;
    }

    /** Takes every value of {@code other} (an edited {@link #copy}), as one change. */
    public void set(Waypoint other) {
        name = other.name;
        x = other.x;
        y = other.y;
        z = other.z;
        dimension = other.dimension;
        rgb = other.rgb;
        icon = other.icon;
        visible = other.visible;
        beam = other.beam;
        death = other.death;
        temporary = other.temporary;
        changed();
    }

    /** What it's called. */
    public String name() {
        return name;
    }

    /** Renames it (cut to {@link #MAX_NAME}). */
    public void setName(String name) {
        this.name = clip(name);
        changed();
    }

    /** The block's x. */
    public int x() {
        return x;
    }

    /** The block's y. */
    public int y() {
        return y;
    }

    /** The block's z. */
    public int z() {
        return z;
    }

    /** Moves it to another block in its dimension. */
    public void setPosition(int x, int y, int z) {
        this.x = x;
        this.y = y;
        this.z = z;
        changed();
    }

    /** The dimension it's in. */
    public String dimension() {
        return dimension;
    }

    /** Its color, {@code 0xRRGGBB}. */
    public int rgb() {
        return rgb;
    }

    /** Its color with full alpha, {@code 0xFFRRGGBB}, for drawing. */
    public int argb() {
        return 0xFF000000 | rgb;
    }

    /** Recolors it. */
    public void setRgb(int rgb) {
        this.rgb = rgb & 0xFFFFFF;
        changed();
    }

    /** Its shape on the minimap and over the world. */
    public Shape icon() {
        return icon;
    }

    /** Gives it another shape. */
    public void setIcon(Shape icon) {
        this.icon = icon;
        changed();
    }

    /** Whether it's shown, on the minimap and over the world. */
    public boolean isVisible() {
        return visible;
    }

    /** Shows or hides it. */
    public void setVisible(boolean visible) {
        this.visible = visible;
        changed();
    }

    /** Whether a beacon's beam rises from it, where beams are on. */
    public boolean hasBeam() {
        return beam;
    }

    /** Turns its beam on or off. */
    public void setBeam(boolean beam) {
        this.beam = beam;
        changed();
    }

    /** Whether it marks where the player last died (made by the game, replaced at the next death). */
    public boolean isDeath() {
        return death;
    }

    /** Marks it as the last death's, or not. */
    public void setDeath(boolean death) {
        this.death = death;
        changed();
    }

    /**
     * Whether it lasts only while the player stays in the world: shown, but never saved to the
     * world's file (a death's on a server, where one address can be many maps).
     */
    public boolean isTemporary() {
        return temporary;
    }

    /** Makes it last only for this visit to the world, or be saved as usual. */
    public void setTemporary(boolean temporary) {
        this.temporary = temporary;
        // Saved either way: the file gains it, or loses it.
        Waypoints list = owner;
        if (list != null) {
            list.changed(false);
        }
    }

    /** The squared distance from {@code px, py, pz} to the middle of its block. */
    public double distanceSquared(double px, double py, double pz) {
        double dx = x + 0.5 - px;
        double dy = y + 0.5 - py;
        double dz = z + 0.5 - pz;
        return dx * dx + dy * dy + dz * dz;
    }

    void attach(@Nullable Waypoints list) {
        owner = list;
    }

    @Nullable
    Waypoints owner() {
        return owner;
    }

    private void changed() {
        Waypoints list = owner;
        if (list != null) {
            list.changed(temporary);
        }
    }

    private static String clip(String name) {
        String stripped = name.strip();
        return stripped.length() > MAX_NAME ? stripped.substring(0, MAX_NAME) : stripped;
    }

    @Override
    public String toString() {
        return name + " (" + x + ", " + y + ", " + z + " in " + dimension + ")";
    }
}
