package incari.client.net;

import incari.client.settings.ClientSettings;
import incari.client.ui.EditedScreen;
import incari.client.ui.Theme;
import incari.client.ui.Ui;
import java.util.List;
import java.util.Objects;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.ConnectScreen;
import net.minecraft.client.gui.screens.DisconnectedScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.gui.screens.multiplayer.JoinMultiplayerScreen;
import net.minecraft.network.chat.Component;
import org.jspecify.annotations.Nullable;

/**
 * Reconnect and Auto Reconnect on the disconnection screen, as PvP clients have them: Reconnect
 * joins the last server again at once; Auto Reconnect, a switch kept in {@link ClientSettings},
 * counts {@link ReconnectCountdown#SECONDS} down on its label and then joins. Only after a server
 * the connect screen joined, not a shared world, and not when {@link JoinFailures} took the
 * failure up (the screen then has no widgets).
 *
 * <p>The countdown runs as the button draws: a page in a background tab doesn't draw, so it waits
 * there. Render thread.
 */
public final class Reconnect {

    private static final Component RECONNECT = Component.literal("Reconnect");
    private static final Component AUTO_OFF = Component.literal("Auto Reconnect: Off");

    /** The disconnection screen the countdown is for; a new one starts it again. */
    private static @Nullable Screen counting;

    private static final ReconnectCountdown COUNTDOWN = new ReconnectCountdown();

    private Reconnect() {}

    /** {@code ScreenEdits}, after each layout: the two buttons under the screen's last one. */
    public static void edit(EditedScreen<DisconnectedScreen> edited) {
        List<AbstractWidget> widgets = edited.widgets();
        ServerConnections.Join join = ServerConnections.rejoinable();
        if (widgets.isEmpty() || join == null) {
            return;
        }
        DisconnectedScreen screen = edited.screen();
        if (!Objects.equals(counting, screen)) {
            // A resize lays the same screen out again: the countdown carries on.
            counting = screen;
            COUNTDOWN.start(System.nanoTime());
        }
        // Two rows more: the screen's column moves up by one to stay centred.
        int row = Theme.CONTROL_HEIGHT + Theme.GAP;
        for (AbstractWidget widget : widgets) {
            widget.setY(widget.getY() - row);
        }
        AbstractWidget back = widgets.getLast();
        int y = back.getY() + back.getHeight() + Theme.GAP;
        Button reconnect = Ui.button(RECONNECT, back.getWidth(), () -> join(join));
        reconnect.setPosition(back.getX(), y);
        edited.add(reconnect);
        AutoButton auto = new AutoButton(screen, join, back.getWidth());
        auto.setPosition(back.getX(), y + row);
        edited.add(auto);
    }

    private static void join(ServerConnections.Join join) {
        counting = null;
        Minecraft minecraft = Minecraft.getInstance();
        // The disconnection screen's own parent is private; the server list is where its back
        // button leads after a join from it.
        Screen parent = new JoinMultiplayerScreen(new TitleScreen());
        ConnectScreen.startConnecting(parent, minecraft, join.address(), join.server(), false, null);
    }

    private static Component countdownLabel(int seconds) {
        return Component.literal("Auto Reconnect: " + seconds + "s");
    }

    /** The switch, its label counting down while it's on. */
    private static final class AutoButton extends Button.Plain {

        private final Screen screen;
        private final ServerConnections.Join join;
        /** The seconds the label shows, -1 for off: the label is made again only when they change. */
        private int shown;
        /** The countdown ran out and the join is on its way: the label stays as it is. */
        private boolean fired;

        AutoButton(Screen screen, ServerConnections.Join join, int width) {
            super(0, 0, width, Theme.CONTROL_HEIGHT, AUTO_OFF, Reconnect::toggle, DEFAULT_NARRATION);
            this.screen = screen;
            this.join = join;
            this.shown = -1;
            update();
        }

        @Override
        protected void extractContents(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
            update();
            super.extractContents(graphics, mouseX, mouseY, a);
        }

        private void update() {
            if (fired) {
                return;
            }
            int seconds = ClientSettings.AUTO_RECONNECT.get() && Objects.equals(counting, screen)
                    ? COUNTDOWN.secondsLeft(System.nanoTime())
                    : -1;
            if (seconds == 0) {
                Minecraft minecraft = Minecraft.getInstance();
                fired = true;
                counting = null;
                // Queued, not run now (execute would, on this thread): joining replaces the
                // screen being drawn.
                minecraft.schedule(() -> {
                    if (Objects.equals(minecraft.gui.screen(), screen)) {
                        join(join);
                    }
                });
            }
            if (seconds != shown) {
                shown = seconds;
                setMessage(seconds < 0 ? AUTO_OFF : countdownLabel(seconds));
            }
        }
    }

    private static void toggle(Button button) {
        boolean on = !ClientSettings.AUTO_RECONNECT.get();
        ClientSettings.AUTO_RECONNECT.set(on);
        if (button instanceof AutoButton auto) {
            if (on) {
                // Turned on here: a full countdown from now.
                counting = auto.screen;
                COUNTDOWN.start(System.nanoTime());
            }
            auto.update();
        }
    }
}
