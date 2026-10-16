package incari.client.online;

import incari.client.ui.Icons;
import incari.client.ui.RoundRects;
import incari.client.ui.ScreenEdits;
import incari.client.ui.Theme;
import incari.online.OnlineService;
import incari.online.Social;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.ComponentPath;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractButton;
import net.minecraft.client.gui.components.Tooltip;
import net.minecraft.client.gui.narration.NarrationElementOutput;
import net.minecraft.client.gui.navigation.FocusNavigationEvent;
import net.minecraft.client.gui.screens.ChatScreen;
import net.minecraft.client.gui.screens.PauseScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.input.InputWithModifiers;
import net.minecraft.network.chat.Component;
import org.jspecify.annotations.Nullable;

/**
 * Ways into the chat menu (ADR 0059): the game's Friends key (O by default; Mojang's own friends
 * need a Microsoft account, which Incari never has) and a button right below the chat box. Also
 * shows what arrives while the menu is closed as toasts. Game thread.
 */
public final class ChatMenu {

    private static boolean installed;

    private ChatMenu() {}

    /** Opens the chat menu over {@code parent} (null for the game). */
    public static void open(@Nullable Screen parent) {
        Minecraft.getInstance().gui.setScreen(new ChatMenuScreen(parent));
    }

    /**
     * Hook ({@code OnlineOptionsScreen.confirmFriendsListEnabled}, which the title screen's and the
     * pause menu's Friends buttons go through): the chat menu over {@code parent} instead of
     * Mojang's friends, which need a Microsoft account. Always true: Mojang's never open.
     */
    public static boolean openInstead(@Nullable Screen parent) {
        open(parent);
        return true;
    }

    /**
     * Hook ({@code Minecraft.toggleFriendsScreen}): the Friends key closes the chat menu if it's
     * open, and opens it over the game, the title screen or the pause menu. Returns whether it did
     * either; false leaves the key to the screen that's open.
     */
    public static boolean toggle(Minecraft minecraft) {
        Screen current = minecraft.gui.screen();
        if (current instanceof ChatMenuScreen menu) {
            menu.onClose();
            return true;
        }
        if (current == null || current instanceof TitleScreen || current instanceof PauseScreen) {
            open(current);
            return true;
        }
        return false;
    }

    /** From {@link IncariOnline#tick}: the chat box's button, once, and the toasts. */
    static void tick(Minecraft minecraft, OnlineService online) {
        if (!installed) {
            installed = true;
            ScreenEdits.edit(ChatScreen.class, edited -> {
                ChatButton button = new ChatButton(online);
                // Under the chat box's last line (40 above the bottom), over the input field (14).
                button.setPosition(2, edited.height() - 33);
                edited.add(button);
            });
        }
        if (minecraft.gui.screen() instanceof ChatMenuScreen) {
            online.social().takeNotices();
            return;
        }
        for (Social.Notice notice : online.social().takeNotices()) {
            // One card a sender: a burst of messages updates it rather than queueing a card each.
            ChatToast shown = minecraft.gui.toastManager().getToast(ChatToast.class, ChatToast.token(notice));
            if (shown != null) {
                shown.merge(notice);
            } else {
                minecraft.gui.toastManager().addToast(new ChatToast(notice));
            }
        }
    }

    /** The button under the chat box: a chat icon, "Incari Chat", and a red count of what's unread. */
    private static final class ChatButton extends AbstractButton {

        private static final int HEIGHT = 14;

        private final OnlineService online;

        ChatButton(OnlineService online) {
            super(0, 0, 0, HEIGHT, Component.literal("Incari Chat"));
            this.online = online;
            Font font = Minecraft.getInstance().font;
            setWidth(18 + font.width(getMessage()) + 8);
            setTooltip(Tooltip.create(Component.literal("Friends and messages (")
                    .append(Minecraft.getInstance().options.keyFriends.getTranslatedKeyMessage())
                    .append(")")));
        }

        @Override
        public void onPress(InputWithModifiers input) {
            open(Minecraft.getInstance().gui.screen());
        }

        @Override
        protected void extractContents(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
            Font font = Minecraft.getInstance().font;
            boolean hot = isHoveredOrFocused();
            RoundRects.fill(graphics, getX(), getY(), getWidth(), HEIGHT, HEIGHT / 2, hot ? 0xCC2A3542 : 0x99161C24);
            Icons.CHAT.draw(graphics, getX() + 5, getY() + 3, 8, hot ? Theme.ACCENT : Theme.TEXT);
            graphics.text(font, getMessage(), getX() + 17, getY() + 3, Theme.TEXT, false);
            Social social = online.social();
            int count = social.unread() + social.pendingRequests();
            if (count > 0) {
                String label = count > 9 ? "9+" : String.valueOf(count);
                int badge = Math.max(10, font.width(label) + 5);
                int x = getRight() - badge / 2 - 1;
                RoundRects.fill(graphics, x, getY() - 3, badge, 10, 5, 0xFFE0565B);
                graphics.text(font, label, x + (badge - font.width(label)) / 2 + 1, getY() - 2, Theme.TEXT, false);
            }
        }

        @Override
        protected void updateWidgetNarration(NarrationElementOutput output) {
            defaultButtonNarrationText(output);
        }

        @Override
        public @Nullable ComponentPath nextFocusPath(FocusNavigationEvent navigationEvent) {
            // Never takes the keyboard's focus: the chat screen tries arrow navigation before its
            // own history keys, so a focusable button here would eat Up and Down.
            return null;
        }
    }
}
