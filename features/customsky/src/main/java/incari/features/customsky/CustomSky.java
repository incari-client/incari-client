package incari.features.customsky;

import incari.framework.Category;
import incari.framework.Module;
import incari.framework.event.Events;
import incari.framework.setting.IntSetting;

/**
 * Custom Sky: the skies resource packs bring in OptiFine's format ({@link SkyLayer}), drawn over
 * the game's own in the overworld, on this client only. Most PvP packs have one; without OptiFine
 * the game ignores them. A pack without one leaves the game's sky as it is.
 */
public final class CustomSky extends Module {

    final IntSetting brightness = settings()
            .integer("brightness", "Brightness", 10, 100, 100)
            .formattedAs(percent -> percent + "%")
            .describedAs("100%: the sky as the pack made it.");

    /** Custom Sky, off until the player turns it on. */
    public CustomSky() {
        super("customsky", "Custom Sky", "Shows the sky your resource packs bring, as OptiFine does.", Category.VISUAL);
        on(Events.TICK, this::apply);
    }

    private void apply() {
        system().game().showPackSkies(brightness.get() / 100F);
    }

    @Override
    protected void onEnable() {
        apply();
    }

    @Override
    protected void onDisable() {
        system().game().showPackSkies(0);
    }
}
