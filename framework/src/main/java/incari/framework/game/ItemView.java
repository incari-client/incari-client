package incari.framework.game;

/**
 * An item stack as modules see it: enough to show it and how worn it is. Drawn with {@code
 * HudCanvas.item}. Reused by the client: read it during the call that handed it out.
 */
public interface ItemView {

    /** Whether there's nothing there. */
    boolean isEmpty();

    /** How many are in the stack. */
    int count();

    /** Whether it wears out with use (armor, tools, weapons). */
    boolean isDamageable();

    /** How much wear it has taken; 0 for new. */
    int damage();

    /** The wear it can take before it breaks; 0 if it isn't damageable. */
    int maxDamage();

    /** The item's name in the player's language ("Oak Planks"); its id where there's no game. */
    default String name() {
        return id();
    }

    /** The item's id ({@code minecraft:shield}); empty for an empty stack. */
    default String id() {
        return "";
    }

    /**
     * How much of the item's cooldown is left, 1 just begun to 0 ready: a shield an axe disabled,
     * an ender pearl just thrown. 0 if it has none.
     */
    default float cooldown() {
        return 0;
    }

    /** Uses left before it breaks: {@code maxDamage() - damage()}. */
    default int remaining() {
        return maxDamage() - damage();
    }
}
