package incari.features.reachdisplay;

import incari.framework.Category;
import incari.framework.Module;
import incari.framework.event.Events;
import incari.framework.hud.Anchor;
import incari.framework.hud.HudCanvas;
import incari.framework.hud.TextElement;
import incari.framework.hud.Tone;
import incari.framework.setting.BooleanSetting;
import incari.framework.setting.IntSetting;
import org.jspecify.annotations.Nullable;

/**
 * Reach Display: how far the player's last hit reached, from their eyes to where the crosshair met
 * the target ({@link Events#ATTACK}), for a few seconds after it.
 */
public final class ReachDisplay extends Module {

    /** Survival's reach against entities: hits beyond it are worth a second look. */
    static final double SURVIVAL_REACH = 3.0;

    final IntSetting decimals =
            settings().integer("decimals", "Decimals", 1, 3, 2).describedAs("How many decimal places to show.");
    final IntSetting hideAfter = settings()
            .integer("hide-after", "Hide after", 0, 10, 3)
            .formattedAs(seconds -> seconds == 0 ? "Never" : seconds + " s")
            .describedAs("How long the last hit's reach shows; Never keeps it until the next.");
    final BooleanSetting farColor =
            settings().bool("far-color", "Mark long hits", false).describedAs("Hits past 3 blocks show in amber.");

    private double reach = -1;
    private long hitAt;

    /** Reach Display, off until the player turns it on. */
    public ReachDisplay() {
        super("reachdisplay", "Reach Display", "Shows how far your last hit reached.", Category.COMBAT);
        on(Events.ATTACK, this::onAttack);
        hud(new Distance());
    }

    private void onAttack(double distance) {
        reach = distance;
        hitAt = system().game().millis();
    }

    @Override
    protected void onDisable() {
        reach = -1;
    }

    /** "3.04 blocks", with {@code decimals} places. */
    static String text(double reach, int decimals) {
        long scale = decimals == 1 ? 10 : decimals == 2 ? 100 : 1000;
        long whole = Math.round(reach * scale);
        String fraction = Long.toString(whole % scale);
        while (fraction.length() < decimals) {
            fraction = "0" + fraction;
        }
        return whole / scale + "." + fraction + " blocks";
    }

    /** The last hit's reach, on the glass. */
    private final class Distance extends TextElement {

        private double cachedReach = Double.NaN;
        private int cachedDecimals;
        private String cached = "";

        Distance() {
            super("reach", "Reach", Anchor.TOP_LEFT, 2, 46);
        }

        private boolean showing() {
            int hide = hideAfter.get();
            return reach >= 0 && (hide == 0 || system().game().millis() - hitAt < hide * 1000L);
        }

        @Override
        public boolean isVisible() {
            return showing();
        }

        @Override
        protected @Nullable String text(HudCanvas canvas) {
            double shown = showing() ? reach : canvas.editing() ? SURVIVAL_REACH : -1;
            if (shown < 0) {
                return null;
            }
            int places = decimals.get();
            if (shown != cachedReach || places != cachedDecimals) {
                cachedReach = shown;
                cachedDecimals = places;
                cached = ReachDisplay.text(shown, places);
            }
            return cached;
        }

        @Override
        protected Tone tone() {
            return farColor.get() && reach > SURVIVAL_REACH ? Tone.WARNING : Tone.TEXT;
        }
    }
}
