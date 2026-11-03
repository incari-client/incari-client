package incari.client.settings;

import incari.client.IncariBuild;
import incari.client.cosmetics.WardrobeScreen;
import incari.client.modules.IncariModules;
import incari.client.modules.ModuleScreenshots;
import incari.client.online.AccountScreen;
import incari.client.online.ChatMenu;
import incari.client.online.IncariOnline;
import incari.client.recorder.RecorderScreen;
import incari.client.recorder.ScreenRecorder;
import incari.client.ui.ControlList;
import incari.client.ui.Icons;
import incari.client.ui.ListScreen;
import incari.client.ui.Theme;
import incari.client.ui.Ui;
import incari.framework.online.Online;
import incari.online.OnlineService;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.components.StringWidget;
import net.minecraft.client.gui.layouts.LinearLayout;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.CommonComponents;
import net.minecraft.network.chat.Component;
import org.jspecify.annotations.Nullable;

/**
 * Incari Settings: the pause menu's one way into everything Incari adds (the module menu, the HUD
 * editor, the Wardrobe, the screenshot gallery, the screen recorder, Incari Chat and the account),
 * and the client's own switches ({@link ClientSettings}). The title carries Incari's badge.
 */
public final class IncariSettingsScreen extends ListScreen {

    /** The title, also the pause menu's button's label. */
    public static final Component TITLE = Component.literal("Incari Settings");

    private long revision;
    private @Nullable StringWidget titleWidget;

    public IncariSettingsScreen(@Nullable Screen parent) {
        super(TITLE, parent);
    }

    @Override
    protected Component caption() {
        return Component.literal("Incari Client " + IncariBuild.VERSION);
    }

    @Override
    protected List<ControlList.Row> rows(ControlList list) {
        List<ControlList.Row> rows = new ArrayList<>();
        rows.add(list.heading(Component.literal("Menus")));
        List<Ui.Tile> tiles = new ArrayList<>();
        tiles.add(tile(Icons.GRID, "Modules", "Turn modules on and off and change their settings.", () -> {
            IncariModules.openMenu(this);
        }));
        tiles.add(tile(Icons.LAYOUT, "Edit HUD", "Move and resize HUD elements.", () -> {
            IncariModules.openHudEditor(this);
        }));
        tiles.add(tile(Icons.HANGER, "Cosmetics", "Capes, hats, wings, trails and emotes.", () -> {
            minecraft.gui.setScreen(new WardrobeScreen(this));
        }));
        tiles.add(tile(Icons.CAMERA, "Screenshots", "View, rename and download screenshots.", () -> {
            ModuleScreenshots.openGallery(this);
        }));
        // Shown even where it can't record (the desktop), so it's clear where it lives.
        Ui.Tile record = Ui.tile(
                Icons.TARGET,
                Component.literal("Record Screen"),
                () -> !ScreenRecorder.supported()
                        ? "Only in the browser build."
                        : ScreenRecorder.active()
                                ? "Recording now. Open to stop and save."
                                : "Record the game, its sound and your mic.",
                () -> minecraft.gui.setScreen(new RecorderScreen(this)));
        record.active = ScreenRecorder.supported();
        tiles.add(record);
        tiles.add(tile(Icons.CHAT, "Incari Chat", "Friends and messages with Incari players.", () -> {
            ChatMenu.open(this);
        }));
        tiles.add(Ui.tile(
                Icons.PERSON,
                Component.literal("Incari Account"),
                () -> IncariOnline.service().status() == Online.Status.SIGNED_IN
                        ? "Signed in. Manage your account."
                        : "Sign in to chat, add friends and save cosmetics.",
                () -> minecraft.gui.setScreen(new AccountScreen(this))));
        // Two to a row.
        for (int i = 0; i < tiles.size(); i += 2) {
            rows.add(list.across(tiles.subList(i, Math.min(i + 2, tiles.size()))));
        }

        rows.add(list.heading(Component.literal("Settings")));
        rows.add(toggle(
                list,
                "Copy screenshots",
                "Copies each screenshot (F2) to the clipboard too.",
                ClientSettings.COPY_SCREENSHOTS));
        rows.add(toggle(
                list,
                "Show Incari badges",
                "Incari's badge beside other signed-in Incari players' names.",
                ClientSettings.SHOW_BADGES));
        rows.add(showMyBadge(list));
        rows.add(toggle(
                list,
                "Auto reconnect",
                "After a kick, joins the server again in 5 seconds.",
                ClientSettings.AUTO_RECONNECT));
        rows.add(toggle(
                list,
                "Usage counts",
                "Once a day, sends Incari a random number, the version and the build, nothing else. "
                        + "It helps us know how many people play.",
                ClientSettings.USAGE_COUNTS));
        return rows;
    }

    private static Ui.Tile tile(Icons icon, String title, String description, Runnable action) {
        return Ui.tile(icon, Component.literal(title), () -> description, action);
    }

    private ControlList.Row toggle(ControlList list, String label, String tooltip, ClientSettings.Flag flag) {
        var button = Ui.toggle(Component.literal(label), flag.get(), Theme.CONTROL_WIDTH / 2, on -> {
            flag.set(on);
            revision++;
        });
        return list.row(Component.literal(label), tooltip, List.of(button), () -> button.setValue(flag.get()));
    }

    /** The account's badge setting (ADR 0062): kept by the service, so only while signed in. */
    private ControlList.Row showMyBadge(ControlList list) {
        OnlineService online = IncariOnline.service();
        var button = Ui.toggle(
                Component.literal("Show others I use Incari"), online.showsBadge(), Theme.CONTROL_WIDTH / 2, on -> {
                    online.setShowBadge(on, result -> revision++);
                });
        Runnable refresh = () -> {
            button.active = online.status() == Online.Status.SIGNED_IN;
            button.setValue(online.showsBadge());
        };
        refresh.run();
        return list.row(
                Component.literal("Show others I use Incari"),
                "Shows Incari's badge beside your name to other Incari players. Needs an Incari account."
                        + " Turning it off doesn't hide you from friends or messages.",
                List.of(button),
                refresh);
    }

    @Override
    protected long revision() {
        return revision
                + IncariModules.system().revision()
                + IncariOnline.service().revision();
    }

    @Override
    protected void footer(LinearLayout footer) {
        footer.addChild(Ui.button(CommonComponents.GUI_DONE, Theme.FOOTER_BUTTON_WIDTH, this::onClose));
    }

    @Override
    protected void init() {
        super.init();
        titleWidget = null;
        for (var child : children()) {
            if (child instanceof StringWidget text
                    && text.getMessage().getString().equals(TITLE.getString())) {
                titleWidget = text;
                break;
            }
        }
    }

    @Override
    public void extractRenderState(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
        super.extractRenderState(graphics, mouseX, mouseY, a);
        AbstractWidget title = titleWidget;
        if (title != null) {
            // Incari's badge to the title's left, a little taller than the text.
            Icons.INCARI.draw(graphics, title.getX() - 14, title.getY() - 2, 11, Theme.ACCENT);
        }
    }

    @Override
    public boolean isPauseScreen() {
        return parent == null || parent.isPauseScreen();
    }
}
