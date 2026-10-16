package incari.client.online;

import incari.client.ui.Theme;
import java.util.List;
import net.minecraft.client.gui.screens.Screen;

/**
 * What's new in a newer Incari Client, opened from the title screen's update notice: its changelog
 * as the online service gave it, one change a line, and the way to the repository to get it.
 */
final class ReleaseNotesScreen extends FormScreen {

    private final List<String> notes;

    ReleaseNotesScreen(Screen parent, String version, List<String> notes) {
        super("Incari Client " + version + " is out", parent);
        this.notes = List.copyOf(notes);
    }

    @Override
    protected void build() {
        text("What's new:", Theme.MUTED);
        for (String note : notes) {
            text("\u2022 " + note, Theme.TEXT);
        }
        gap(4);
        row(
                new String[] {"Get It on GitHub", "Back"},
                new Runnable[] {() -> UpdateNotice.openRepository(this), this::onClose});
    }
}
