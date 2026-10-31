package incari.features.shortitems;

import incari.framework.Category;
import incari.framework.Module;
import incari.framework.event.Events;
import incari.framework.event.HeldItem;
import incari.framework.setting.BooleanSetting;
import incari.framework.setting.IntSetting;

/**
 * Short Items: whatever is held in first person, in either hand, is drawn smaller, toward the
 * hand, as 1.8 PvP packs drew swords, so it hides less of the view. Only the drawing changes:
 * reach and hits are the game's.
 *
 * <p>Was Short Swords, for swords only: the ids stay {@code shortswords}'s, so a profile keeps it.
 */
public final class ShortItems extends Module {

    final IntSetting size = settings()
            .integer("size", "Size", 40, 100, 70)
            .formattedAs(percent -> percent + "%")
            .describedAs("How big held items are drawn, against the game's size.");
    final BooleanSetting whileUsing = settings()
            .bool("while-blocking", "Also while in use", true)
            .describedAs("Off: an item in use (blocking, eating, drawing a bow) is drawn at the game's size.");

    /** Short Items, off until the player turns it on. */
    public ShortItems() {
        super(
                "shortswords",
                "Short Items",
                "Draws the items in your hands smaller, so they hide less.",
                Category.VISUAL);
        on(Events.HELD_ITEM, this::onHeldItem);
    }

    private void onHeldItem(HeldItem item) {
        if (whileUsing.get() || !item.using()) {
            item.setScale(size.get() / 100.0F);
        }
    }
}
