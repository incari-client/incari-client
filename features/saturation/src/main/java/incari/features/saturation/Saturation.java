package incari.features.saturation;

import incari.framework.Category;
import incari.framework.Module;
import incari.framework.event.Events;
import incari.framework.setting.ColorSetting;

/**
 * Saturation: the hidden food value that keeps hunger from dropping, shown on the hunger bar. Each
 * shank's worth of saturation outlines that shank in a bright color, from the right as hunger
 * fills, as AppleSkin does; when no outline is left, hunger is about to start going down.
 */
public final class Saturation extends Module {

    final ColorSetting color =
            settings().color("color", "Color", 0xFFCC00).describedAs("The color saturated shanks are outlined in.");

    /** Saturation, off until the player turns it on. */
    public Saturation() {
        super("saturation", "Saturation", "Shows your saturation on the hunger bar.", Category.HUD);
        on(Events.TICK, this::apply);
    }

    private void apply() {
        system().game().overrideSaturation(color.get() & 0xFFFFFF);
    }

    @Override
    protected void onEnable() {
        apply();
    }

    @Override
    protected void onDisable() {
        system().game().overrideSaturation(-1);
    }
}
