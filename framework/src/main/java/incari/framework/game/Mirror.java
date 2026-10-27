package incari.framework.game;

/** How a structure is mirrored before it's turned ({@link Blocks#transform}). */
public enum Mirror {
    /** As built. */
    NONE("None"),
    /** East and west swap: x runs the other way. */
    X("East-west"),
    /** North and south swap: z runs the other way. */
    Z("North-south");

    private final String label;

    Mirror(String label) {
        this.label = label;
    }

    /** What the player is shown. */
    public String label() {
        return label;
    }
}
