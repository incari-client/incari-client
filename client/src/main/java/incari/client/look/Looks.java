package incari.client.look;

import incari.client.relay.RelayScreen;
import incari.client.ui.Look;
import incari.client.ui.ScreenEdits;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.PauseScreen;
import net.minecraft.client.gui.screens.multiplayer.JoinMultiplayerScreen;

/** Puts the client's look on: the skin ({@link Look}) and the edits to the game's screens. */
public final class Looks {

    private static boolean started;

    private Looks() {}

    /** From the first client tick, or the title screen's first layout if that comes first. */
    public static void start(Minecraft minecraft) {
        if (started) {
            return;
        }
        started = true;
        Look.start(minecraft);
        ScreenEdits.edit(PauseScreen.class, PauseLook::edit);
        ScreenEdits.edit(JoinMultiplayerScreen.class, RelayScreen::fitRow);
    }
}
