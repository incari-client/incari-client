package incari.framework.event;

/**
 * An item held in first person, about to be drawn, as {@link Events#HELD_ITEM} hands it to
 * listeners: which hand, whether it's a shield or a sword, whether it's in use. Listeners say how it's
 * drawn: lower (Low Shield), smaller (Short Items), or in 1.7's poses (1.7 Animations). One object, reused for each hand each
 * frame.
 */
public final class HeldItem {

    private boolean mainHand;
    private boolean shield;
    private boolean sword;
    private boolean using;
    private float lower;
    private float scale;
    private boolean legacyItems;
    private boolean legacyRod;
    private boolean legacyBlock;
    private boolean swingWhileUsing;

    /** The game's side: this hand's item, not a sword, before the listeners run. */
    public void reset(boolean mainHand, boolean shield, boolean using) {
        reset(mainHand, shield, false, using);
    }

    /** The game's side: this hand's item, before the listeners run. */
    public void reset(boolean mainHand, boolean shield, boolean sword, boolean using) {
        this.mainHand = mainHand;
        this.shield = shield;
        this.sword = sword;
        this.using = using;
        this.lower = 0;
        this.scale = 1;
        this.legacyItems = false;
        this.legacyRod = false;
        this.legacyBlock = false;
        this.swingWhileUsing = false;
    }

    /** Whether it's in the main hand. */
    public boolean mainHand() {
        return mainHand;
    }

    /** Whether it's a shield. */
    public boolean shield() {
        return shield;
    }

    /** Whether it's a sword (the game's swords tag). */
    public boolean sword() {
        return sword;
    }

    /** Whether the player is using it now: blocking, eating, drawing a bow. */
    public boolean using() {
        return using;
    }

    /** How far down it's moved, in the view's units (about a screen's height is 1). */
    public float lower() {
        return lower;
    }

    /** Moves it down by {@code amount} of the view's units. */
    public void setLower(float amount) {
        this.lower = amount;
    }

    /** How big it's drawn against the game's size: 1 as the game draws it. */
    public float scale() {
        return scale;
    }

    /** Draws it {@code factor} times the game's size, toward the hand that holds it. */
    public void setScale(float factor) {
        this.scale = factor;
    }

    /** Whether flat items (swords, tools, most items) are held as 1.7 held them. */
    public boolean legacyItems() {
        return legacyItems;
    }

    public void setLegacyItems(boolean on) {
        this.legacyItems = on;
    }

    /** Whether a fishing rod points the way 1.7's did. */
    public boolean legacyRod() {
        return legacyRod;
    }

    public void setLegacyRod(boolean on) {
        this.legacyRod = on;
    }

    /** Whether a blocking sword takes 1.7's pose. */
    public boolean legacyBlock() {
        return legacyBlock;
    }

    public void setLegacyBlock(boolean on) {
        this.legacyBlock = on;
    }

    /**
     * Whether the arm swings while the item's in use (1.7's block-hit): attacking while blocking,
     * eating or drawing a bow swings it, as 1.7 drew it. Only the drawing: nothing is sent.
     */
    public boolean swingWhileUsing() {
        return swingWhileUsing;
    }

    public void setSwingWhileUsing(boolean on) {
        this.swingWhileUsing = on;
    }
}
