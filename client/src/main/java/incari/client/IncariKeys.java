package incari.client;

import net.minecraft.client.KeyMapping;

/**
 * Incari's key mappings, added to the game's list by one hook in {@code Options} so they show in
 * the controls screen and are saved with the rest.
 *
 * <p>The names are shown as they are: the game translates a key mapping's name, and Incari has no
 * language files yet, so an English name is its own fallback. A name is also the key's line in
 * {@code options.txt} ({@code key_<name>:<key>}), which the game splits at the first colon, so a
 * name must have none: with one, the bind was read back as an unknown option on every start and
 * went back to its default.
 */
public final class IncariKeys {

    /** Voice chat: held to talk. B, as in Eagler's clients. */
    public static final KeyMapping PUSH_TO_TALK =
            new KeyMapping("Voice - Push to Talk", 5, KeyMapping.Category.MULTIPLAYER);

    /**
     * The profiler overlay, on and off. F9: the game leaves it free, and browsers don't take
     * it for themselves.
     */
    public static final KeyMapping PROFILER_OVERLAY = new KeyMapping("Profiler Overlay", 66, KeyMapping.Category.DEBUG);

    /**
     * The module menu. Right Shift, as in other clients; the game leaves it free. Modules' own
     * binds aren't here: they're in each module's settings and its profile ({@code GameKeys}).
     */
    public static final KeyMapping MODULE_MENU = new KeyMapping("Incari - Modules", 229, KeyMapping.Category.MISC);

    /**
     * The emote wheel (ADR 0061), held open. G: the game leaves it free, and B is push to talk's.
     */
    public static final KeyMapping EMOTE_WHEEL = new KeyMapping("Incari - Emote Wheel", 10, KeyMapping.Category.MISC);

    /**
     * The screen recorder, on and off (ADR 0062; the browser only). F8: the game leaves it free, and
     * Chrome takes F6 and F7 for itself.
     */
    public static final KeyMapping RECORD_SCREEN =
            new KeyMapping("Incari - Record Screen", 65, KeyMapping.Category.MISC);

    /** Every Incari key mapping, as a new array for the game's list. */
    public static KeyMapping[] all() {
        return new KeyMapping[] {PUSH_TO_TALK, PROFILER_OVERLAY, MODULE_MENU, EMOTE_WHEEL, RECORD_SCREEN};
    }

    private IncariKeys() {}
}
