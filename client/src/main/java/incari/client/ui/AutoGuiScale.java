package incari.client.ui;

import com.mojang.blaze3d.platform.Window;

/**
 * The GUI scale "Auto" picks, which is also the default: one step below the largest that fits, so
 * a Chromebook's small screen keeps room for the game around the GUI. A player who wants the
 * largest picks its number.
 */
public final class AutoGuiScale {

    private AutoGuiScale() {}

    /** The scale for the {@code guiScale} option's value: {@code 0} is Auto, others pass through. */
    public static int of(Window window, int option, boolean enforceUnicode) {
        if (option != 0) {
            return window.calculateScale(option, enforceUnicode);
        }
        // The Unicode font needs an even scale, so its step down is two.
        int step = enforceUnicode ? 2 : 1;
        int largest = window.calculateScale(0, enforceUnicode);
        return window.calculateScale(Math.max(1, largest - step), enforceUnicode);
    }
}
