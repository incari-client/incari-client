package incari.features.fastcrystal;

import incari.framework.Category;
import incari.framework.Module;
import incari.framework.setting.BooleanSetting;

/**
 * Fast Crystal: an end crystal the player hits, and a charged respawn anchor they set off, go from
 * this client at once instead of a round trip later, so the next one can go in its place without
 * waiting out the ping. Prediction only (as the game predicts a broken block): the player's own
 * click still sends the one packet it always did, nothing is clicked for them, and one the server
 * keeps comes back. The choice is handed to the game once per change; the client checks it on a
 * hit or a use.
 */
public final class FastCrystal extends Module {

    final BooleanSetting crystals = settings()
            .bool("crystals", "Crystals", true)
            .describedAs("A crystal you hit is gone at once, so the next can go in its place.");
    final BooleanSetting anchors = settings()
            .bool("anchors", "Anchors", true)
            .describedAs("A charged anchor you set off is gone at once, outside the Nether.");

    /** Fast Crystal, off until the player turns it on. */
    public FastCrystal() {
        super(
                "fastcrystal",
                "Fast Crystal",
                "Crystals and anchors go as you break them, not a ping later.",
                Category.COMBAT);
        crystals.onChange(this::changed);
        anchors.onChange(this::changed);
    }

    private void changed() {
        if (isEnabled()) {
            system().game().predictBreaks(crystals.get(), anchors.get());
        }
    }

    @Override
    protected void onEnable() {
        system().game().predictBreaks(crystals.get(), anchors.get());
    }

    @Override
    protected void onDisable() {
        system().game().predictBreaks(false, false);
    }
}
