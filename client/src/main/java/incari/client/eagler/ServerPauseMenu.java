package incari.client.eagler;

import com.mojang.blaze3d.Blaze3D;
import com.mojang.blaze3d.platform.NativeImage;
import com.mojang.logging.LogUtils;
import incari.client.ui.Ui;
import incari.net.eagler.EaglerPauseMenu;
import java.net.URI;
import java.net.URISyntaxException;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.Renderable;
import net.minecraft.client.gui.components.StringWidget;
import net.minecraft.client.gui.components.events.GuiEventListener;
import net.minecraft.client.gui.layouts.GridLayout;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.contents.TranslatableContents;
import net.minecraft.resources.Identifier;
import net.minecraft.util.FormattedCharSequence;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;

/**
 * The pause menu an EaglerXServer customizes (its {@code pause_menu.yml}): a "server info" button
 * and a Discord button, which open their links, and small icons beside the menu's buttons and title,
 * as Eagler's 1.8 client shows them. Kept for the connection that sent it. The game's thread only.
 *
 * <p>A server info button that shows markup the server sends, in a web view, isn't shown: the page
 * has no web view yet. One that shows an address in a web view opens it in a new tab instead.
 */
public final class ServerPauseMenu {

    private static final Logger LOGGER = LogUtils.getLogger();
    private static final int FULL_WIDTH = 204;
    private static final int HALF_WIDTH = 98;
    private static final int ICON_SIZE = 16;
    private static final int ICON_GAP = 4;

    /** The vanilla buttons' labels, by the name Eagler's icon keys give each. */
    private static final Map<String, String> BUTTON_KEYS = Map.of(
            "menu.returnToGame", "backToGame",
            "gui.advancements", "achievements",
            "gui.stats", "statistics",
            "menu.options", "options",
            "menu.disconnect", "disconnect",
            "menu.returnToMenu", "disconnect");

    private static @Nullable ServerPauseMenu current;

    private final EaglerPauseMenu menu;
    /** Each icon key's texture, and its image's size. */
    private final Map<String, Icon> icons = new HashMap<>();

    private record Icon(Identifier texture, int width, int height) {}

    private ServerPauseMenu(EaglerPauseMenu menu) {
        this.menu = menu;
        Map<EaglerPauseMenu.Image, Identifier> uploaded = new HashMap<>();
        menu.icons().forEach((key, image) -> {
            // Keys often share an image (the server sends each once); upload each once.
            Identifier texture = uploaded.computeIfAbsent(image, _ -> upload(uploaded.size(), image));
            icons.put(key, new Icon(texture, image.width(), image.height()));
        });
    }

    /** The message arrived (any thread): this connection's menu from now on. */
    static void received(EaglerPauseMenu menu) {
        Minecraft.getInstance().execute(() -> {
            clear();
            current = new ServerPauseMenu(menu);
        });
    }

    /** The connection ended (any thread). */
    static void closed() {
        Minecraft.getInstance().execute(ServerPauseMenu::clear);
    }

    private static void clear() {
        ServerPauseMenu menu = current;
        if (menu != null) {
            for (Icon icon : Map.copyOf(menu.icons).values()) {
                Minecraft.getInstance().getTextureManager().release(icon.texture());
            }
            current = null;
        }
    }

    private static Identifier upload(int index, EaglerPauseMenu.Image image) {
        Identifier id = Identifier.fromNamespaceAndPath("incari", "pause_menu/icon_" + index);
        NativeImage pixels = new NativeImage(image.width(), image.height(), false);
        for (int y = 0; y < image.height(); y++) {
            for (int x = 0; x < image.width(); x++) {
                pixels.setPixel(x, y, image.pixel(x, y));
            }
        }
        // Registering again under the same id closes the texture it replaces.
        Minecraft.getInstance().getTextureManager().register(id, new DynamicTexture(id::toString, pixels));
        return id;
    }

    /** Whether this connection's server sent a pause menu with any button of its own. */
    static boolean active() {
        ServerPauseMenu menu = current;
        return menu != null && (menu.menu.serverInfo() != null || menu.menu.discord() != null);
    }

    // --- The pause screen ----------------------------------------------------------------------

    /**
     * {@code PauseScreen}, below the row of icon buttons: the server's own buttons, side by side if
     * both fit in half the width.
     */
    public static void addButtons(GridLayout.RowHelper row) {
        ServerPauseMenu menu = current;
        if (menu == null) {
            return;
        }
        List<Button> buttons = new ArrayList<>();
        EaglerPauseMenu.Button serverInfo = menu.menu.serverInfo();
        if (serverInfo != null && serverInfo.url() != null) {
            buttons.add(menu.linkButton(serverInfo, "serverInfo"));
        } else if (serverInfo != null) {
            LOGGER.debug("The server's info button shows markup in a web view, which the page doesn't have");
        }
        EaglerPauseMenu.Button discord = menu.menu.discord();
        if (discord != null && discord.url() != null) {
            buttons.add(menu.linkButton(discord, "discord"));
        }
        Font font = Minecraft.getInstance().font;
        boolean halves = buttons.size() == 2
                && buttons.stream()
                        .allMatch(button ->
                                menu.contentWidth(font, button.getMessage(), keyOf(button)) <= HALF_WIDTH - 8);
        for (Button button : buttons) {
            button.setWidth(halves ? HALF_WIDTH : FULL_WIDTH);
            row.addChild(button, halves ? 1 : 2);
        }
    }

    /**
     * {@code PauseScreen}, after its widgets: the icons beside the buttons and the title, drawn over
     * them.
     */
    public static void addIcons(Screen screen, java.util.function.Consumer<Renderable> addRenderableOnly) {
        ServerPauseMenu menu = current;
        if (menu == null || menu.icons.isEmpty()) {
            return;
        }
        // Read as each frame draws, not copied now: the pause screen adds its title after this hook,
        // and ScreenEdits moves widgets after each layout.
        addRenderableOnly.accept((graphics, mouseX, mouseY, a) -> menu.extractIcons(graphics, screen.children()));
    }

    private Button linkButton(EaglerPauseMenu.Button button, String key) {
        URI uri = safeUri(button.url());
        Button widget = new KeyedButton(Component.literal(button.text()), key, pressed -> {
            if (uri != null) {
                Blaze3D.openUri(uri);
            }
        });
        widget.active = uri != null;
        return widget;
    }

    /** {@code url} if it's a web address; a server mustn't make the page run a {@code javascript:} one. */
    static @Nullable URI safeUri(@Nullable String url) {
        if (url == null) {
            return null;
        }
        try {
            URI uri = new URI(url.trim());
            String scheme = uri.getScheme() == null ? "" : uri.getScheme().toLowerCase(Locale.ROOT);
            return scheme.equals("https") || scheme.equals("http") ? uri : null;
        } catch (URISyntaxException e) {
            return null;
        }
    }

    private void extractIcons(GuiGraphicsExtractor graphics, List<? extends GuiEventListener> children) {
        Font font = Minecraft.getInstance().font;
        // Indexed: a frame's draw allocates no iterator.
        for (int i = 0; i < children.size(); i++) {
            GuiEventListener child = children.get(i);
            if (child instanceof AbstractWidget widget && widget.visible && !(child instanceof KeyedButton)) {
                String key = child instanceof StringWidget ? titleKey(widget.getMessage()) : keyOf(widget);
                if (key != null) {
                    int textWidth = font.width(widget.getMessage());
                    int middle = widget.getX() + widget.getWidth() / 2;
                    int y = widget.getY() + widget.getHeight() / 2 - ICON_SIZE / 2;
                    extractIcon(graphics, "icon_" + key + "_L", middle - textWidth / 2 - ICON_GAP - ICON_SIZE, y);
                    extractIcon(graphics, "icon_" + key + "_R", middle + (textWidth + 1) / 2 + ICON_GAP, y);
                }
            }
        }
    }

    private void extractIcon(GuiGraphicsExtractor graphics, String key, int x, int y) {
        Icon icon = icons.get(key);
        if (icon != null) {
            graphics.blit(
                    RenderPipelines.GUI_TEXTURED,
                    icon.texture(),
                    x,
                    y,
                    0,
                    0,
                    ICON_SIZE,
                    ICON_SIZE,
                    icon.width(),
                    icon.height(),
                    icon.width(),
                    icon.height());
        }
    }

    private int contentWidth(Font font, Component message, @Nullable String key) {
        int width = font.width(message);
        for (String side : new String[] {"_L", "_R"}) {
            if (key != null && icons.containsKey("icon_" + key + side)) {
                width += ICON_SIZE + ICON_GAP;
            }
        }
        return width;
    }

    /** A button's name in Eagler's icon keys: one of the server's, or a vanilla one by its label. */
    private static @Nullable String keyOf(AbstractWidget widget) {
        if (widget instanceof KeyedButton keyed) {
            return keyed.key;
        }
        return widget.getMessage().getContents() instanceof TranslatableContents label
                ? BUTTON_KEYS.get(label.getKey())
                : null;
    }

    private static @Nullable String titleKey(Component message) {
        return message.getContents() instanceof TranslatableContents label
                        && label.getKey().equals("menu.game")
                ? "title"
                : null;
    }

    /**
     * One of the server's buttons, with the name its icons are keyed by. It draws its icons itself,
     * centred with its text as one group, as Eagler's client does.
     */
    private final class KeyedButton extends Button.Plain {
        final String key;

        KeyedButton(Component message, String key, OnPress onPress) {
            super(0, 0, FULL_WIDTH, 20, message, onPress, DEFAULT_NARRATION);
            this.key = key;
        }

        @Override
        protected void extractContents(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
            extractDefaultSprite(graphics);
            Font font = Minecraft.getInstance().font;
            boolean left = icons.containsKey("icon_" + key + "_L");
            boolean right = icons.containsKey("icon_" + key + "_R");
            int iconsWidth = (left ? ICON_SIZE + ICON_GAP : 0) + (right ? ICON_SIZE + ICON_GAP : 0);
            // The server's own words: cut to fit the button with its pictures, never drawn past it.
            FormattedCharSequence text = Ui.fit(font, getMessage(), Math.max(0, getWidth() - 8 - iconsWidth));
            int textWidth = font.width(text);
            int groupWidth = textWidth + iconsWidth;
            int x = getX() + (getWidth() - groupWidth) / 2;
            int iconY = getY() + (getHeight() - ICON_SIZE) / 2;
            if (left) {
                extractIcon(graphics, "icon_" + key + "_L", x, iconY);
                x += ICON_SIZE + ICON_GAP;
            }
            graphics.text(font, text, x, getY() + (getHeight() - 8) / 2, active ? 0xFFFFFFFF : 0xFFA0A0A0);
            if (right) {
                extractIcon(graphics, "icon_" + key + "_R", x + textWidth + ICON_GAP, iconY);
            }
        }
    }
}
