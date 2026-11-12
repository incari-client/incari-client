package incari.framework;

/** Where a module is listed in the module menu, in this order. */
public enum Category {
    /** Information drawn on the HUD: keystrokes, FPS, coordinates, armor. */
    HUD("HUD"),
    /** Combat: displays and the feel of fighting. */
    COMBAT("战斗"),
    /** How the player moves: toggle sprint, toggle sneak. */
    MOVEMENT("移动"),
    /** How the world looks: zoom, freelook, the crosshair, presets. */
    VISUAL("视觉"),
    /** Everything else: chat, screenshots, waypoints. */
    UTILITY("实用");

    private final String label;

    Category(String label) {
        this.label = label;
    }

    /** What the module menu calls it. */
    public String label() {
        return label;
    }
}
