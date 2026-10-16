package incari.client.relay;

import com.mojang.logging.LogUtils;
import incari.client.platform.ClientPlatform;
import incari.client.ui.EditedScreen;
import incari.net.WebSocketUrl;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;
import net.minecraft.ChatFormatting;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.client.gui.layouts.LinearLayout;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.multiplayer.JoinMultiplayerScreen;
import net.minecraft.network.chat.CommonComponents;
import net.minecraft.network.chat.Component;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;

/**
 * Joining a world an Eagler player shares (ADR 0019): the relay the host shared it through,
 * and the world's code. The relay last used is remembered in {@code incari/relay.txt}.
 *
 * <p>English only for now; Incari's screens don't have language files yet.
 */
public final class RelayScreen extends Screen {

    private static final Logger LOGGER = LogUtils.getLogger();
    /** The relay Eagler's clients try first, until the player names another. */
    static final String DEFAULT_RELAY = "wss://relay.deev.is/";

    private final Screen parent;
    private @Nullable EditBox relay;
    private @Nullable EditBox code;
    private @Nullable Button join;
    private Component message = CommonComponents.EMPTY;

    public RelayScreen(Screen parent) {
        super(Component.literal("Join Shared World"));
        this.parent = parent;
    }

    /** {@code JoinMultiplayerScreen}: the button to this screen. */
    public static void addButton(LinearLayout row, Screen multiplayerScreen) {
        row.addChild(Button.builder(
                        Component.literal("Shared..."),
                        button -> Minecraft.getInstance().gui.setScreen(new RelayScreen(multiplayerScreen)))
                .width(ROW_BUTTON)
                .build());
    }

    /** The multiplayer screen's bottom buttons' width, as the game and {@link #addButton} make them. */
    private static final int ROW_BUTTON = 74;

    private static final int ROW_SPACING = 4;

    /**
     * An edit to {@code JoinMultiplayerScreen} ({@code Looks}): its bottom row, which Incari's
     * button makes wider than the narrowest screen (320), shares the screen's width when it doesn't
     * fit. The top row is centred again too: the game centred both by the bottom row's old width.
     */
    public static void fitRow(EditedScreen<JoinMultiplayerScreen> edited) {
        fit(edited, "selectServer.select", 100);
        fit(edited, "gui.back", ROW_BUTTON);
    }

    /** The row with the button labelled {@code key}, its buttons {@code full} wide where they fit, centred. */
    private static void fit(EditedScreen<JoinMultiplayerScreen> edited, String key, int full) {
        AbstractWidget anchor = edited.find(key);
        if (anchor == null) {
            return;
        }
        List<AbstractWidget> row = new ArrayList<>();
        for (AbstractWidget widget : edited.widgets()) {
            if (widget.getY() == anchor.getY()) {
                row.add(widget);
            }
        }
        row.sort(Comparator.comparingInt(AbstractWidget::getX));
        int count = row.size();
        int each = Math.min(full, (edited.width() - 2 * 6 - (count - 1) * ROW_SPACING) / count);
        int x = (edited.width() - count * each - (count - 1) * ROW_SPACING) / 2;
        for (AbstractWidget button : row) {
            // Set every layout: a screen made wider again gets its full-width buttons back.
            button.setWidth(each);
            button.setX(x);
            x += each + ROW_SPACING;
        }
    }

    @Override
    protected void init() {
        int x = this.width / 2 - 100;
        EditBox relayBox = new EditBox(this.font, x, 70, 200, 20, relay, Component.literal("Relay"));
        relayBox.setMaxLength(256);
        if (relay == null) {
            relayBox.setValue(lastRelay());
        }
        relayBox.setResponder(text -> update());
        relay = this.addRenderableWidget(relayBox);
        EditBox codeBox = new EditBox(this.font, x, 110, 200, 20, code, Component.literal("World code"));
        codeBox.setMaxLength(32);
        codeBox.setResponder(text -> update());
        code = this.addRenderableWidget(codeBox);
        join = this.addRenderableWidget(Button.builder(Component.literal("Join"), button -> join())
                .bounds(x, 150, 98, 20)
                .build());
        this.addRenderableWidget(Button.builder(CommonComponents.GUI_CANCEL, button -> onClose())
                .bounds(x + 102, 150, 98, 20)
                .build());
        this.setInitialFocus(codeBox);
        update();
    }

    private void update() {
        Button button = join;
        if (button == null) {
            return;
        }
        boolean supported = ClientPlatform.get().rtc().supported();
        button.active = supported
                && WebSocketUrl.isWebSocketUrl(text(relay))
                && !text(code).isBlank();
        message = supported
                ? CommonComponents.EMPTY
                : Component.literal("Shared worlds only work in the browser.").withStyle(ChatFormatting.RED);
    }

    private void join() {
        String relayUrl = text(relay).trim();
        saveRelay(relayUrl);
        Minecraft.getInstance()
                .gui
                .setScreen(new RelayConnectScreen(parent, relayUrl, text(code).trim()));
    }

    private static String text(@Nullable EditBox box) {
        return box == null ? "" : box.getValue();
    }

    private static Path relayFile() {
        return Minecraft.getInstance().gameDirectory.toPath().resolve("incari").resolve("relay.txt");
    }

    private static String lastRelay() {
        try {
            Path file = relayFile();
            if (Files.isRegularFile(file)) {
                String saved = Files.readString(file, StandardCharsets.UTF_8).trim();
                if (WebSocketUrl.isWebSocketUrl(saved)) {
                    return saved;
                }
            }
        } catch (IOException e) {
            LOGGER.warn("Couldn't read the last relay: {}", e.toString());
        }
        return DEFAULT_RELAY;
    }

    private static void saveRelay(String url) {
        try {
            Files.createDirectories(relayFile().getParent());
            Files.writeString(relayFile(), url.toLowerCase(Locale.ROOT), StandardCharsets.UTF_8);
        } catch (IOException e) {
            LOGGER.warn("Couldn't save the relay: {}", e.toString());
        }
    }

    @Override
    public void extractRenderState(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
        super.extractRenderState(graphics, mouseX, mouseY, a);
        graphics.centeredText(this.font, this.title, this.width / 2, 20, 0xFFFFFFFF);
        int x = this.width / 2 - 100;
        graphics.text(this.font, Component.literal("Relay (ws:// or wss://)"), x, 58, 0xFFA0A0A0);
        graphics.text(this.font, Component.literal("World code"), x, 98, 0xFFA0A0A0);
        graphics.centeredText(this.font, message, this.width / 2, 180, 0xFFFFFFFF);
    }

    @Override
    public void onClose() {
        Minecraft.getInstance().gui.setScreen(parent);
    }
}
