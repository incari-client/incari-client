package incari.client.net;

import net.minecraft.client.gui.components.EditBox;
import net.minecraft.client.gui.components.events.GuiEventListener;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.multiplayer.ServerData;
import net.minecraft.network.chat.contents.TranslatableContents;

/**
 * The edit-server screen's name and address as typed, kept in its entry before Incari's Via or
 * Wispcraft button opens a screen over it. Coming back, the game lays that screen out again from
 * the entry ({@code ManageServerScreen.init}), which held only what was saved: what the player had
 * typed was lost. The entry is the screen's own copy (Edit) or a new one (Add), so Cancel still
 * leaves the list as it was. Render thread.
 */
final class TypedServerFields {

    private static final String NAME = "manageServer.enterName";
    private static final String ADDRESS = "manageServer.enterIp";

    private TypedServerFields() {}

    /** Copies the name and address boxes of {@code screen}, if it has them, into {@code server}. */
    static void keep(Screen screen, ServerData server) {
        for (GuiEventListener child : screen.children()) {
            if (child instanceof EditBox box && box.getMessage().getContents() instanceof TranslatableContents label) {
                if (label.getKey().equals(NAME)) {
                    server.name = box.getValue();
                } else if (label.getKey().equals(ADDRESS)) {
                    server.ip = box.getValue();
                }
            }
        }
    }
}
