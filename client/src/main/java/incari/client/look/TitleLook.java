package incari.client.look;

import incari.client.IncariBuild;
import incari.client.cosmetics.Cosmetics;
import incari.client.cosmetics.WardrobeScreen;
import incari.client.modules.IncariModules;
import incari.client.online.AccountScreen;
import incari.client.online.IncariOnline;
import incari.client.online.UpdateNotice;
import incari.client.profile.PlayerName;
import incari.client.profile.TitleProfile;
import incari.client.settings.IncariSettingsScreen;
import incari.client.ui.Icons;
import incari.client.ui.Logo;
import incari.client.ui.Look;
import incari.client.ui.MenuBackdrop;
import incari.client.ui.Panels;
import incari.client.ui.RoundRects;
import incari.client.ui.Theme;
import incari.client.ui.Ui;
import incari.framework.MenuButton;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Consumer;
import net.minecraft.SharedConstants;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.CommonButtons;
import net.minecraft.client.gui.components.PlainTextButton;
import net.minecraft.client.gui.components.SpriteIconButton;
import net.minecraft.client.gui.screens.CreditsAndAttributionScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.multiplayer.JoinMultiplayerScreen;
import net.minecraft.client.gui.screens.multiplayer.SafetyScreen;
import net.minecraft.client.gui.screens.options.AccessibilityOptionsScreen;
import net.minecraft.client.gui.screens.options.LanguageSelectScreen;
import net.minecraft.client.gui.screens.options.OptionsScreen;
import net.minecraft.client.gui.screens.worldselection.SelectWorldScreen;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.FontDescription;
import net.minecraft.network.chat.Style;
import net.minecraft.resources.Identifier;
import net.minecraft.util.ARGB;
import net.minecraft.util.Util;
import org.jspecify.annotations.Nullable;

/**
 * The title screen in the client's look: the backdrop under a cold tint that deepens toward
 * the menu, Incari's wordmark over a column of menu buttons on the left, the player on a glass card
 * on the right, and in Normal, falling snow and the menu rising in as the screen opens.
 *
 * <p>The game's {@code TitleScreen} stays the screen (the game checks for it by class): two hooks
 * hand its layout ({@code init}) and its drawing ({@code extractRenderState}) over to this. Realms,
 * the splash text and the Friends button are left out; Incari plays on its own servers and offline.
 * Render thread.
 */
public final class TitleLook {

    private static final Identifier WORDMARK_FONT = Identifier.fromNamespaceAndPath("incari", "wordmark");
    private static final Style WORDMARK =
            Style.EMPTY.withFont(new FontDescription.Resource(WORDMARK_FONT)).withoutShadow();
    private static final String NAME = "INCARI";
    private static final Component[] LETTERS = new Component[NAME.length()];
    private static final Component SUBTITLE = Component.literal("CLIENT  ·  " + IncariBuild.VERSION);
    private static final Component CARD_LABEL = Component.literal("OFFLINE MODE");
    private static final Component UPDATE_HINT = Component.literal("GitHub");
    private static final Component COPYRIGHT = Component.translatable("title.credits");
    private static final String VERSION =
            "Incari  ·  Minecraft " + SharedConstants.getCurrentVersion().name();

    private static final int WORDMARK_HEIGHT = 26;
    private static final int LETTER_SPACING = 3;
    private static final int ROW = Theme.CONTROL_HEIGHT;
    private static final int ROW_GAP = 5;
    /** How long the menu takes to rise in, in Normal. */
    private static final float RISE_MILLIS = 550.0F;

    private static final Snowfall SNOW = new Snowfall();
    private static final List<AbstractWidget> RISING = new ArrayList<>();

    private static TitleBackdrop backdrop =
            (graphics, width, height, a) -> MenuBackdrop.extract(graphics, width, height);
    private static @Nullable Screen laidOut;
    private static long shownAt;
    private static int columnX;
    private static int wordmarkY;
    private static int cardX;
    private static int cardY;
    private static int cardWidth;
    private static int cardHeight;
    private static int modelBottom;
    /** Where the small screens' links in the top left corner end; 0 with the card. */
    private static int linksRight;
    /** The card's Incari Account button, named for the account once signed in. */
    private static @Nullable Button account;

    private static long accountRevision = -1;
    /** The update notice at the top, hidden until the online service names a newer release. */
    private static @Nullable Button update;

    private static long updateRevision = -1;

    static {
        for (int i = 0; i < NAME.length(); i++) {
            LETTERS[i] = Component.literal(String.valueOf(NAME.charAt(i))).withStyle(WORDMARK);
        }
    }

    private TitleLook() {}

    /** Shows {@code next} behind the title screen from the next frame. */
    public static void backdrop(TitleBackdrop next) {
        backdrop = next;
    }

    /**
     * {@code TitleScreen.init}: lays the title screen out in the client's look, adding its widgets
     * with {@code add}. Returns false for the game to lay it out its own way.
     */
    @SuppressWarnings("ReferenceEquality") // the title screen object, not an equal one
    public static boolean init(Screen title, Consumer<AbstractWidget> add) {
        Minecraft minecraft = Minecraft.getInstance();
        Looks.start(minecraft);
        if (laidOut != title) {
            shownAt = Util.getMillis();
        }
        laidOut = title;
        RISING.clear();
        int width = title.width;
        int height = title.height;

        int columnWidth = Math.clamp(Math.round(width * 0.27F), 124, 168);
        columnX = Math.max(14, Math.round(width * 0.07F));
        // Incari Settings and modules' own buttons on a row under Multiplayer (the gallery is in there).
        List<MenuButton> extra = new ArrayList<>();
        extra.add(new MenuButton(
                "incari",
                IncariSettingsScreen.TITLE.getString(),
                "incari",
                () -> minecraft.gui.setScreen(new IncariSettingsScreen(minecraft.gui.screen()))));
        extra.addAll(IncariModules.system().menuButtons());
        int rows = 4;
        int block = WORDMARK_HEIGHT + 18 + rows * ROW + (rows - 1) * ROW_GAP;
        wordmarkY = Math.max(8, (height - block) / 2 - 6);
        int y = wordmarkY + WORDMARK_HEIGHT + 24;

        rise(
                add,
                place(
                        Ui.menuButton(
                                Component.translatable("menu.singleplayer"),
                                Icons.CUBE,
                                columnWidth,
                                () -> minecraft.gui.setScreen(new SelectWorldScreen(title))),
                        columnX,
                        y));
        y += ROW + ROW_GAP;
        Button multiplayer = Ui.menuButton(Component.translatable("menu.multiplayer"), Icons.GLOBE, columnWidth, () -> {
            Screen next = minecraft.options.skipMultiplayerWarning
                    ? new JoinMultiplayerScreen(title)
                    : new SafetyScreen(title);
            minecraft.gui.setScreen(next);
        });
        multiplayer.active = minecraft.allowsMultiplayer();
        rise(add, place(multiplayer, columnX, y));
        y += ROW + ROW_GAP;
        if (!extra.isEmpty()) {
            int count = Math.min(3, extra.size());
            int each = (columnWidth - (count - 1) * ROW_GAP) / count;
            for (int i = 0; i < count; i++) {
                MenuButton button = extra.get(i);
                int buttonWidth = i == count - 1 ? columnWidth - (count - 1) * (each + ROW_GAP) : each;
                rise(
                        add,
                        place(
                                Ui.menuButton(
                                        Component.literal(button.label()),
                                        MenuIcons.of(button.icon()),
                                        buttonWidth,
                                        button.action()),
                                columnX + i * (each + ROW_GAP),
                                y));
            }
            y += ROW + ROW_GAP;
        }
        rise(
                add,
                place(
                        Ui.menuButton(
                                Component.translatable("menu.options"),
                                Icons.GEAR,
                                columnWidth,
                                () -> minecraft.gui.setScreen(new OptionsScreen(title, minecraft.options))),
                        columnX,
                        y));

        layOutCard(title, add, width, height);

        // A newer Incari Client: shown once the online service says so (release builds only).
        Button notice = Ui.menuButton(Component.empty(), Icons.ARROW, UPDATE_HINT, 0, () -> UpdateNotice.open(title));
        notice.setTooltip(Ui.tooltip("Open Incari Client on GitHub"));
        notice.visible = false;
        update = notice;
        updateRevision = -1;
        rise(add, notice);

        // The game's language and accessibility buttons, small, in the top right corner.
        SpriteIconButton language = CommonButtons.language(
                ROW,
                button -> minecraft.gui.setScreen(
                        new LanguageSelectScreen(title, minecraft.options, minecraft.getLanguageManager())),
                true);
        language.setPosition(width - 2 * ROW - 10, 6);
        add.accept(language);
        SpriteIconButton accessibility = CommonButtons.accessibility(
                ROW, button -> minecraft.gui.setScreen(new AccessibilityOptionsScreen(title, minecraft.options)), true);
        accessibility.setPosition(width - ROW - 6, 6);
        add.accept(accessibility);

        Font font = minecraft.font;
        int copyrightWidth = font.width(COPYRIGHT);
        add.accept(new PlainTextButton(
                width - copyrightWidth - 6,
                height - 12,
                copyrightWidth,
                10,
                COPYRIGHT,
                button -> minecraft.gui.setScreen(new CreditsAndAttributionScreen(title)),
                font));
        return true;
    }

    /** The player card on the right, or on a narrow screen, the profile link in its place. */
    private static void layOutCard(Screen title, Consumer<AbstractWidget> add, int width, int height) {
        cardWidth = Math.clamp(Math.round(width * 0.2F), 104, 132);
        int modelHeight = Math.min(112, Math.round(height * 0.38F));
        int modelWidth = modelHeight * 11 / 20;
        cardHeight = 12 + 9 + 8 + modelHeight + 8 + 9 + 8 + 3 * ROW + 2 * ROW_GAP + 12;
        cardX = width - columnX - cardWidth;
        account = null;
        linksRight = 0;
        if (cardX < columnX + 168 + 24 || cardHeight > height - 40) {
            cardWidth = 0;
            AbstractWidget profile = TitleProfile.link(title);
            AbstractWidget cosmeticsLink = TitleProfile.cosmeticsLink(title);
            AbstractWidget accountLink = TitleProfile.accountLink(title);
            add.accept(profile);
            add.accept(cosmeticsLink);
            add.accept(accountLink);
            linksRight = Math.max(profile.getRight(), Math.max(cosmeticsLink.getRight(), accountLink.getRight()));
            return;
        }
        cardY = (height - cardHeight) / 2;
        int modelTop = cardY + 12 + 9 + 8;
        modelBottom = modelTop + modelHeight;
        rise(
                add,
                TitleProfile.playerView(
                        title, cardX + (cardWidth - modelWidth) / 2, modelTop, modelWidth, modelHeight, false));
        rise(
                add,
                place(
                        Ui.button(
                                Component.literal("Edit Profile"),
                                cardWidth - 24,
                                () -> TitleProfile.openProfile(title)),
                        cardX + 12,
                        modelBottom + 8 + 9 + 8));
        // Cosmetics (ADR 0061): what the player on this card wears.
        rise(
                add,
                place(
                        Ui.button(
                                Component.literal("Cosmetics"), cardWidth - 24, () -> open(new WardrobeScreen(title))),
                        cardX + 12,
                        modelBottom + 8 + 9 + 8 + ROW + ROW_GAP));
        // Incari accounts (ADR 0058): not a module, so here rather than in the module menu.
        Button accountButton =
                Ui.button(Component.literal("Incari Account"), cardWidth - 24, () -> open(new AccountScreen(title)));
        account = accountButton;
        accountRevision = -1;
        rise(add, place(accountButton, cardX + 12, modelBottom + 8 + 9 + 8 + 2 * (ROW + ROW_GAP)));
    }

    private static void open(Screen screen) {
        Minecraft.getInstance().gui.setScreen(screen);
    }

    /** The account button says who's signed in, kept current as the connection comes and goes. */
    private static void refreshAccount() {
        Button button = account;
        if (button == null) {
            return;
        }
        var online = IncariOnline.service();
        long revision = online.revision();
        if (revision == accountRevision) {
            return;
        }
        accountRevision = revision;
        String name = online.signedIn() ? online.accountName() : null;
        button.setMessage(Component.literal(name == null ? "Incari Account" : "Incari: " + name));
    }

    /**
     * The update notice follows what the service last said: centered at the top, between the small
     * screens' links on the left and the language and accessibility buttons on the right.
     */
    private static void refreshUpdate(int width) {
        Button button = update;
        if (button == null) {
            return;
        }
        var online = IncariOnline.service();
        long revision = online.revision();
        if (revision == updateRevision) {
            return;
        }
        updateRevision = revision;
        String newer = UpdateNotice.newer(online);
        button.visible = newer != null;
        if (newer == null) {
            return;
        }
        Component label = Component.literal(
                newer.isEmpty() ? "A new Incari Client is out" : "Incari Client " + newer + " is out");
        Font font = Minecraft.getInstance().font;
        // The menu button's insets: 8 and the icon (9 + 6) before the label, the hint's gap, 8 after.
        int buttonWidth = 8 + 15 + font.width(label) + 14 + font.width(UPDATE_HINT) + 8;
        button.setMessage(label);
        button.setWidth(buttonWidth);
        int left = linksRight + 6;
        int right = width - 2 * ROW - 16;
        button.setPosition(Math.max(left, Math.min((width - buttonWidth) / 2, right - buttonWidth)), 6);
    }

    private static AbstractWidget place(AbstractWidget widget, int x, int y) {
        widget.setPosition(x, y);
        return widget;
    }

    private static void rise(Consumer<AbstractWidget> add, AbstractWidget widget) {
        RISING.add(widget);
        add.accept(widget);
    }

    /** Whether the title screen is laid out in the client's look, so drawn by {@link #extractBehind} and {@link #extractFront}. */
    @SuppressWarnings("ReferenceEquality") // the title screen object, not an equal one
    public static boolean active(Screen title) {
        return title == laidOut;
    }

    /** {@code TitleScreen.extractRenderState}, before its widgets: the backdrop, snow, wordmark and card. */
    public static void extractBehind(Screen title, GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
        int width = title.width;
        int height = title.height;
        boolean normal = Look.normal();
        backdrop.extract(graphics, width, height, a);
        graphics.fill(0, 0, width, height, normal ? 0x1E101822 : 0x40101822);
        RoundRects.fadeRight(graphics, 0, 0, Math.round(width * 0.6F), height, normal ? 0x8C0C131B : 0xB00C131B);
        if (normal) {
            SNOW.extract(graphics, width, height);
        }

        float shown = normal ? Math.min(1.0F, (Util.getMillis() - shownAt) / RISE_MILLIS) : 1.0F;
        float eased = 1.0F - (1.0F - shown) * (1.0F - shown) * (1.0F - shown);
        for (AbstractWidget widget : RISING) {
            widget.setAlpha(eased);
        }
        graphics.pose().pushMatrix();
        graphics.pose().translate(0, (1.0F - eased) * 8.0F);
        extractWordmark(graphics, eased);
        refreshUpdate(width);
        if (cardWidth > 0) {
            refreshAccount();
            if (normal) {
                // Normal's glass is light, made for a menu's dark tint; out here it would sit on the
                // photo's bright cloud and its white text would wash out.
                RoundRects.fill(
                        graphics,
                        cardX,
                        cardY,
                        cardWidth,
                        cardHeight,
                        Theme.PANEL_RADIUS,
                        ARGB.multiplyAlpha(0x800C131B, eased));
            }
            Panels.panel(graphics, cardX, cardY, cardWidth, cardHeight, eased);
            Font font = Minecraft.getInstance().font;
            graphics.centeredText(
                    font, CARD_LABEL, cardX + cardWidth / 2, cardY + 12, ARGB.multiplyAlpha(Theme.MUTED, eased));
            // In the player's name tag cosmetic, if they wear one (ADR 0061).
            Component name = Cosmetics.ownName(PlayerName.name());
            graphics.centeredText(
                    font, name, cardX + cardWidth / 2, modelBottom + 8, ARGB.multiplyAlpha(Theme.TEXT, eased));
        }
        graphics.pose().popMatrix();
    }

    /** {@code TitleScreen.extractRenderState}, after its widgets: the version in the bottom left corner. */
    public static void extractFront(Screen title, GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
        Font font = Minecraft.getInstance().font;
        graphics.text(font, VERSION, 6, title.height - 12, Theme.MUTED, false);
    }

    /** Incari's mark and INCARI, letter-spaced, and the line under them. */
    private static void extractWordmark(GuiGraphicsExtractor graphics, float alpha) {
        Font font = Minecraft.getInstance().font;
        int color = ARGB.multiplyAlpha(Theme.TEXT, alpha);
        if (Look.normal()) {
            RoundRects.shadow(
                    graphics,
                    columnX + 4,
                    wordmarkY + 4,
                    WORDMARK_HEIGHT - 8,
                    WORDMARK_HEIGHT - 8,
                    (WORDMARK_HEIGHT - 8) / 2,
                    8,
                    ARGB.multiplyAlpha(Theme.ACCENT & 0x55FFFFFF, alpha));
        }
        Logo.draw(graphics, columnX, wordmarkY, WORDMARK_HEIGHT, alpha);
        int x = columnX + WORDMARK_HEIGHT + 9;
        // The wordmark font's baseline is 7 units down, as every font's is; its capitals stand 13.5 tall.
        int letters = wordmarkY + 9;
        for (Component letter : LETTERS) {
            graphics.text(font, letter, x, letters, color, false);
            x += font.width(letter) + LETTER_SPACING;
        }
        graphics.text(
                font,
                SUBTITLE,
                columnX + WORDMARK_HEIGHT + 10,
                wordmarkY + WORDMARK_HEIGHT - 3,
                ARGB.multiplyAlpha(Theme.MUTED, alpha),
                false);
    }
}
