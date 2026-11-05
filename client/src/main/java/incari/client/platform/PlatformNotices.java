package incari.client.platform;

import incari.platform.api.PlatformNotice;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.toasts.SystemToast;
import net.minecraft.network.chat.Component;

/**
 * Shows the player what the platform says they must know ({@link PlatformNotice}): mostly that their
 * game isn't being saved. Also the browser's one hint about Ctrl+W, which closes a windowed tab.
 */
public final class PlatformNotices {

    /** Long enough to read twice: these change what happens to the player's game. */
    private static final SystemToast.SystemToastId SAVES = new SystemToast.SystemToastId(12_000L);

    private static final SystemToast.SystemToastId HINT = new SystemToast.SystemToastId(10_000L);

    /** The Ctrl+W hint was shown, or isn't needed: once a page load. */
    private static boolean hinted;

    private PlatformNotices() {}

    /** A notice from {@code pollEvents}; shown once the game's tasks run (the toasts exist by then). */
    public static void show(Minecraft minecraft, PlatformNotice notice) {
        Component title;
        Component message;
        switch (notice) {
            case SAVES_HELD_ELSEWHERE -> {
                title = Component.literal("Nothing here will be saved");
                message = Component.literal(
                        "The game is open in another tab. Close the other tab, then reload this one.");
            }
            case SAVES_UNAVAILABLE -> {
                title = Component.literal("Nothing will be saved");
                message = Component.literal("This browser gives the game no storage (a private window?).");
            }
            case SAVE_FAILED -> {
                title = Component.literal("Saving failed");
                message = Component.literal("The disk may be full. The game keeps trying; free some space.");
            }
            default -> {
                return;
            }
        }
        minecraft.execute(() -> SystemToast.addOrUpdate(minecraft.gui.toastManager(), SAVES, title, message));
    }

    /**
     * Every frame: the first time the player is in a world in a browser tab that isn't fullscreen,
     * with sprint on Ctrl, says that Ctrl+W there asks to close the tab (only fullscreen lets the
     * page have it), and how to avoid it.
     */
    public static void tick(Minecraft minecraft) {
        if (hinted || minecraft.level == null || minecraft.player == null) {
            return;
        }
        hinted = true;
        if ("desktop".equals(ClientPlatform.get().buildKind())
                || minecraft.getWindow().platformWindow().isFullscreen()
                || !minecraft.options.keySprint.saveString().endsWith(".control")) {
            return;
        }
        SystemToast.addOrUpdate(
                minecraft.gui.toastManager(),
                HINT,
                Component.literal("Ctrl+W asks to close the tab"),
                Component.literal("Turn on Fullscreen in Video Settings so sprinting with Ctrl+W stays in the game."));
    }
}
