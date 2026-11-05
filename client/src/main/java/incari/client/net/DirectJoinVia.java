package incari.client.net;

import incari.client.ui.EditedScreen;
import incari.client.ui.Theme;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.screens.DirectJoinServerScreen;
import net.minecraft.client.multiplayer.ServerData;
import net.minecraft.client.multiplayer.ServerList;
import org.jspecify.annotations.Nullable;

/**
 * Direct Connect's Via button (ADR 0071): Via is off for a server joined there unless the player
 * sets it here, as on the edit-server screen. The game keeps a direct join's server as a hidden list
 * entry the player can't edit, so without this a server of another version had no way to Via.
 *
 * <p>The button holds its choice in a {@link ServerData} of its own. It starts from the saved choice
 * of the last address typed, if the list has it; the join then takes the choice only if the player
 * changed it, so an address typed afterwards keeps its own. Render thread.
 */
public final class DirectJoinVia {

    /** Below the address box, which the game puts at y 116, 20 high. */
    private static final int BUTTON_Y = 116 + 20 + Theme.GAP;

    /** The Direct Connect screen the button is for: a new one starts the choice again. */
    private static @Nullable DirectJoinServerScreen shown;

    /** The button's choice, in its {@code incariVersion}. */
    private static ServerData choice = holder(ServerVersions.OFF);

    /** The choice the screen opened with: the join takes the button's only if it differs. */
    private static int opened = ServerVersions.OFF;

    private DirectJoinVia() {}

    /** {@code ScreenEdits}, after each layout of Direct Connect: the Via button under the address. */
    @SuppressWarnings("ReferenceEquality") // This very screen, not an equal one.
    public static void edit(EditedScreen<DirectJoinServerScreen> edited) {
        Minecraft minecraft = Minecraft.getInstance();
        if (shown != edited.screen()) {
            shown = edited.screen();
            opened = savedChoice(minecraft, minecraft.options.lastMpIp);
            choice = holder(opened);
        }
        int x = edited.width() / 2 - ServerVersions.VersionButton.FULL_WIDTH / 2;
        // The Select and Cancel buttons move down if a short screen put them in the button's way.
        int below = BUTTON_Y + Theme.CONTROL_HEIGHT + Theme.GAP;
        List<AbstractWidget> widgets = edited.widgets();
        int top = Integer.MAX_VALUE;
        for (AbstractWidget widget : widgets) {
            if (widget.getY() > BUTTON_Y - Theme.GAP) {
                top = Math.min(top, widget.getY());
            }
        }
        if (top < below) {
            for (AbstractWidget widget : widgets) {
                if (widget.getY() > BUTTON_Y - Theme.GAP) {
                    widget.setY(widget.getY() + below - top);
                }
            }
        }
        edited.add(ServerVersions.button(choice, x, BUTTON_Y));
    }

    /**
     * Hook ({@code JoinMultiplayerScreen.directJoinCallback}): gives the server about to be joined
     * the choice made on Direct Connect, if the player made one. Returns whether it changed, so
     * the list is saved.
     */
    public static boolean apply(ServerData server) {
        int picked = choice.incariVersion;
        shown = null;
        if (picked == opened || server.incariVersion == picked) {
            return false;
        }
        server.incariVersion = picked;
        return true;
    }

    /** The Via choice the list holds for {@code address}, hidden entries too; off if it has none. */
    private static int savedChoice(Minecraft minecraft, String address) {
        if (address == null || address.isBlank()) {
            return ServerVersions.OFF;
        }
        ServerList servers = new ServerList(minecraft);
        servers.load();
        ServerData saved = servers.get(address);
        return saved != null ? saved.incariVersion : ServerVersions.OFF;
    }

    private static ServerData holder(int version) {
        ServerData data = new ServerData("", "", ServerData.Type.OTHER);
        data.incariVersion = version;
        return data;
    }
}
