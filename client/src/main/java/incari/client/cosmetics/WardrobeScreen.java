package incari.client.cosmetics;

import incari.client.IncariKeys;
import incari.client.online.AccountScreen;
import incari.client.online.IncariOnline;
import incari.client.profile.PlayerName;
import incari.client.ui.Icons;
import incari.client.ui.Panels;
import incari.client.ui.RoundRects;
import incari.client.ui.ScreenPanels;
import incari.client.ui.Theme;
import incari.client.ui.Ui;
import incari.framework.online.Online;
import incari.online.Catalog;
import incari.online.Cosmetic;
import incari.online.Loadout;
import incari.online.OnlineService;
import incari.online.Wardrobe;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractButton;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.client.gui.narration.NarrationElementOutput;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.InputWithModifiers;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.network.chat.CommonComponents;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import net.minecraft.util.ARGB;
import net.minecraft.util.FormattedCharSequence;
import org.jspecify.annotations.Nullable;

/**
 * The Wardrobe (ADR 0061): the player's cosmetics, laid out as the module menu is. Kinds down the
 * left (Capes, Hats, Wings & Back, Name Tags, Trails, Emotes, Outfits), a grid of cards in the
 * middle, and the player on the right wearing what's picked. Pointing at a card tries it on; a
 * click puts it on, and a click on what's worn (or on None) takes it off. Emotes play on the
 * preview. Outfits are what's worn saved under a name, put on again in a click.
 *
 * <p>Under the preview: whether other Incari players see what's worn (signed in) or only the player
 * does (with Sign In beside it), and the switch that hides other players' cosmetics for anyone who
 * finds them distracting.
 *
 * <p>Opened from the title screen's player card and the pause menu. Changes are made at once and
 * reach the service a moment later ({@link Wardrobe}).
 */
public final class WardrobeScreen extends Screen {

    private static final int SIDEBAR = 104;
    private static final int PREVIEW = 128;
    private static final int PADDING = 10;
    private static final int ENTRY = 16;
    private static final int CARD_WIDTH = 64;
    private static final int CARD_HEIGHT = 74;
    private static final int GAP = 6;
    private static final int SCROLL_STEP = 24;
    private static final String[] LETTERS = {"W", "A", "R", "D", "R", "O", "B", "E"};

    /** A kind of cosmetic down the left, and Outfits. */
    private enum Tab {
        CAPES(Cosmetic.Slot.CAPE),
        HATS(Cosmetic.Slot.HAT),
        BACK(Cosmetic.Slot.BACK),
        NAMETAGS(Cosmetic.Slot.NAMETAG),
        TRAILS(Cosmetic.Slot.TRAIL),
        EMOTES(Cosmetic.Slot.EMOTE),
        OUTFITS(null);

        final Cosmetic.@Nullable Slot slot;

        Tab(Cosmetic.@Nullable Slot slot) {
            this.slot = slot;
        }

        String label() {
            return slot == null ? "Outfits" : slot.label();
        }
    }

    /** The tab shown last, shown again next time. */
    private static Tab lastTab = Tab.CAPES;

    private final @Nullable Screen parent;
    private final OnlineService online = IncariOnline.service();
    private final Wardrobe wardrobe = online.wardrobe();
    private final List<Card> cards = new ArrayList<>();

    private Tab tab = lastTab;
    private double scroll;
    private int panelX;
    private int panelY;
    private int panelWidth;
    private int panelHeight;
    private int gridX;
    private int gridY;
    private int gridWidth;
    private int gridHeight;
    private int contentHeight;
    private int previewX;
    /** The preview's width: {@link #PREVIEW}, less on a narrow screen so a card still fits beside. */
    private int previewWidth = PREVIEW;

    private @Nullable PlayerPreview preview;
    private @Nullable Button signIn;
    private @Nullable EditBox outfitName;

    /** What the pointer is on, tried on in the preview: a card's loadout. */
    private @Nullable Loadout tryOn;

    private @Nullable Component notice;

    public WardrobeScreen(@Nullable Screen parent) {
        super(Component.literal("Wardrobe"));
        this.parent = parent;
    }

    /** The Wardrobe open on {@code slot}'s kind, or on Outfits for null (the smoke test's screenshots). */
    public static WardrobeScreen showing(@Nullable Screen parent, Cosmetic.@Nullable Slot slot) {
        WardrobeScreen screen = new WardrobeScreen(parent);
        for (Tab each : Tab.values()) {
            if (each.slot == slot) {
                screen.tab = each;
            }
        }
        return screen;
    }

    @Override
    protected void init() {
        panelWidth = Math.min(width - 24, 560);
        panelHeight = Math.min(height - 20, 300);
        panelX = (width - panelWidth) / 2;
        panelY = (height - panelHeight) / 2;
        ScreenPanels.clear(this);
        ScreenPanels.add(this, panelX, panelY, panelWidth, panelHeight);

        // The kinds, down the left.
        int x = panelX + 8;
        int entryWidth = SIDEBAR - 16;
        int y = panelY + 34;
        for (Tab each : Tab.values()) {
            addRenderableWidget(new Entry(Component.literal(each.label()), count(each), x, y, entryWidth, each));
            y += ENTRY + 2;
        }
        Button done = Ui.button(CommonComponents.GUI_DONE, entryWidth, this::onClose);
        done.setPosition(x, panelY + panelHeight - 8 - Theme.CONTROL_HEIGHT);
        addRenderableWidget(done);

        // The player, on the right, with whether others see it and the switch for others' below.
        previewWidth = Math.clamp(panelWidth - SIDEBAR - 2 * PADDING - 2 * CARD_WIDTH - GAP, 88, PREVIEW);
        previewX = panelX + panelWidth - PADDING - previewWidth;
        int previewTop = panelY + PADDING + 14;
        int previewHeight = panelHeight - PADDING - 14 - 64;
        PlayerPreview player =
                new PlayerPreview(previewX + 8, previewTop, previewWidth - 16, previewHeight, this::shown);
        Cosmetic.Slot facing = tab.slot;
        if (facing != null) {
            player.face(facing);
        }
        preview = player;
        addRenderableWidget(player);
        int belowY = previewTop + previewHeight + 4;
        Button signInButton = Ui.button(Component.literal("Sign In"), previewWidth, () -> {
            if (minecraft != null) {
                minecraft.gui.setScreen(new AccountScreen(this));
            }
        });
        signInButton.setPosition(previewX, belowY + 12);
        signInButton.setTooltip(Ui.tooltip("Sign in so other Incari players can see your cosmetics"));
        signIn = signInButton;
        addRenderableWidget(signInButton);
        addRenderableWidget(new OthersSwitch(previewX, panelY + panelHeight - 8 - 14, previewWidth));

        gridX = panelX + SIDEBAR + PADDING;
        gridWidth = previewX - PADDING - gridX;
        gridY = panelY + PADDING + 14;
        outfitName = null;
        if (tab == Tab.OUTFITS) {
            int saveWidth = 70;
            // A grid too narrow for both side by side puts Save under the name.
            boolean stacked = gridWidth < saveWidth + Theme.GAP + 80;
            int nameWidth = stacked ? gridWidth : gridWidth - saveWidth - Theme.GAP;
            EditBox name = Ui.textField(font, Component.literal("Outfit name"), nameWidth, 24);
            name.setMaxLength(Wardrobe.MAX_OUTFIT_NAME);
            name.setHint(Component.literal(Ui.fit(font, "Name this outfit", nameWidth - 8)));
            name.setPosition(gridX, gridY);
            addRenderableWidget(name);
            outfitName = name;
            if (stacked) {
                gridY += Theme.CONTROL_HEIGHT + Theme.GAP;
            }
            Button save = Ui.button(Component.literal("Save"), stacked ? gridWidth : saveWidth, this::saveOutfit);
            save.setPosition(stacked ? gridX : gridX + gridWidth - saveWidth, gridY);
            save.setTooltip(Ui.tooltip("Save what you're wearing as an outfit"));
            addRenderableWidget(save);
            gridY += Theme.CONTROL_HEIGHT + PADDING;
        }
        gridHeight = panelY + panelHeight - PADDING - gridY;

        cards.clear();
        if (tab == Tab.OUTFITS) {
            cards.add(new Card(null, null, null));
            for (Wardrobe.Outfit outfit : wardrobe.outfits()) {
                cards.add(new Card(null, null, outfit));
            }
        } else {
            Cosmetic.Slot slot = tab.slot;
            if (slot != null) {
                if (slot.worn()) {
                    cards.add(new Card(null, slot, null));
                }
                for (Cosmetic each : Catalog.inSlot(slot)) {
                    cards.add(new Card(each, slot, null));
                }
            }
        }
        for (Card card : cards) {
            // Drawn by this screen inside the grid's clip, not with the other widgets.
            addWidget(card);
        }
        layOutCards();
    }

    private String count(Tab each) {
        if (each.slot == null) {
            return String.valueOf(wardrobe.outfits().size());
        }
        int owned = 0;
        for (Cosmetic cosmetic : Catalog.inSlot(each.slot)) {
            if (wardrobe.owns(cosmetic)) {
                owned++;
            }
        }
        return String.valueOf(owned);
    }

    private void choose(Tab chosen) {
        tab = chosen;
        lastTab = chosen;
        scroll = 0;
        notice = null;
        rebuildWidgets();
    }

    private void layOutCards() {
        int columns = Math.max(1, (gridWidth + GAP) / (CARD_WIDTH + GAP));
        int cardWidth = (gridWidth - (columns - 1) * GAP) / columns;
        int shown = 0;
        for (Card card : cards) {
            int column = shown % columns;
            int row = shown / columns;
            card.setRectangle(
                    cardWidth,
                    CARD_HEIGHT,
                    gridX + column * (cardWidth + GAP),
                    gridY + row * (CARD_HEIGHT + GAP) - (int) scroll);
            shown++;
        }
        int rows = (shown + columns - 1) / columns;
        contentHeight = Math.max(0, rows * (CARD_HEIGHT + GAP) - GAP);
        double max = Math.max(0, contentHeight - gridHeight);
        if (scroll > max) {
            scroll = max;
            layOutCards();
        }
    }

    /** What the preview wears: what the pointer is on, or what's worn. */
    private Loadout shown() {
        Loadout trying = tryOn;
        return trying != null ? trying : wardrobe.worn();
    }

    private void saveOutfit() {
        EditBox name = outfitName;
        String typed = name == null ? "" : name.getValue();
        if (wardrobe.saveOutfit(typed)) {
            notice = Component.literal("Outfit saved.");
            rebuildWidgets();
        } else {
            notice = Component.literal("No room for more outfits. Delete one first.")
                    .withStyle(style -> style.withColor(Theme.WARNING));
        }
    }

    @Override
    public boolean mouseScrolled(double mouseX, double mouseY, double scrollX, double scrollY) {
        if (mouseX >= gridX && mouseX < gridX + gridWidth && mouseY >= gridY && mouseY < gridY + gridHeight) {
            scroll = Math.max(0, Math.min(Math.max(0, contentHeight - gridHeight), scroll - scrollY * SCROLL_STEP));
            layOutCards();
            return true;
        }
        return super.mouseScrolled(mouseX, mouseY, scrollX, scrollY);
    }

    @Override
    public void extractRenderState(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
        // Whatever the pointer is on is tried on.
        tryOn = null;
        for (Card card : cards) {
            if (card.isMouseOver(mouseX, mouseY)) {
                tryOn = card.tryOn();
            }
        }
        Button signInButton = signIn;
        if (signInButton != null) {
            signInButton.visible = !wardrobe.shared() && online.status() == Online.Status.SIGNED_OUT;
        }

        // The sidebar's mark, and the lines between the columns.
        Icons.HANGER.draw(graphics, panelX + 10, panelY + 10, 13, Theme.TEXT);
        int letterX = panelX + 28;
        for (String letter : LETTERS) {
            graphics.text(font, letter, letterX, panelY + 14, Theme.TEXT, false);
            letterX += font.width(letter) + 2;
        }
        graphics.fill(panelX + SIDEBAR, panelY + 8, panelX + SIDEBAR + 1, panelY + panelHeight - 8, 0x1AFFFFFF);
        graphics.fill(previewX - 5, panelY + 8, previewX - 4, panelY + panelHeight - 8, 0x1AFFFFFF);

        // The grid's heading: the kind, and a hint for the ones that need one.
        if (tab != Tab.OUTFITS) {
            String label = tab.label();
            graphics.text(font, label, gridX, panelY + PADDING, Theme.TEXT, false);
            // Cut on a narrow grid rather than run into the label.
            String hint = Ui.fit(font, hint(), gridWidth - font.width(label) - 8);
            graphics.text(font, hint, gridX + gridWidth - font.width(hint), panelY + PADDING, Theme.MUTED, false);
        }

        // The name over the preview, in the name tag cosmetic if one's shown.
        Cosmetic nametag = shown().in(Cosmetic.Slot.NAMETAG);
        String name = PlayerName.name();
        Component styled = nametag == null ? Component.literal(name) : NameTags.styled(name, nametag);
        FormattedCharSequence label = Ui.fit(font, styled, previewWidth - 6);
        int labelWidth = font.width(label);
        int labelX = previewX + (previewWidth - labelWidth) / 2;
        int labelY = panelY + PADDING;
        graphics.fill(labelX - 3, labelY - 2, labelX + labelWidth + 2, labelY + 9, 0x40000000);
        graphics.text(font, label, labelX, labelY, 0xFFFFFFFF, false);

        super.extractRenderState(graphics, mouseX, mouseY, a);

        // Who sees what's worn.
        PlayerPreview player = preview;
        int statusY = player == null ? panelY : player.getBottom() + 4;
        boolean shared = wardrobe.shared();
        String status = Ui.fit(font, shared ? "Incari players see these" : "Only you see these", previewWidth - 12);
        int statusWidth = font.width(status) + 12;
        int statusX = previewX + (previewWidth - statusWidth) / 2;
        (shared ? Icons.CHECK : Icons.EYE).draw(graphics, statusX, statusY, 9, shared ? Theme.POSITIVE : Theme.MUTED);
        graphics.text(font, status, statusX + 12, statusY + 1, shared ? Theme.POSITIVE : Theme.MUTED, false);

        graphics.enableScissor(gridX - 6, gridY, gridX + gridWidth + 6, gridY + gridHeight);
        for (Card card : cards) {
            if (card.getBottom() > gridY && card.getY() < gridY + gridHeight) {
                card.extractRenderState(graphics, mouseX, mouseY, a);
            }
        }
        graphics.disableScissor();
        if (tab == Tab.OUTFITS && cards.size() <= 1) {
            int lineY = gridY + CARD_HEIGHT + 16;
            for (FormattedCharSequence line :
                    Ui.wrap(font, "Put on what you like, name it above and press Save.", gridWidth, 3)) {
                graphics.centeredText(font, line, gridX + gridWidth / 2, lineY, Theme.MUTED);
                lineY += font.lineHeight + 1;
            }
        }
        Component shownNotice = notice;
        if (shownNotice != null) {
            graphics.text(
                    font,
                    Component.literal(Ui.fit(font, shownNotice.getString(), gridWidth))
                            .withStyle(shownNotice.getStyle()),
                    gridX,
                    panelY + panelHeight - PADDING + 1,
                    Theme.MUTED,
                    false);
        }
        if (contentHeight > gridHeight) {
            int thumb = Math.max(16, gridHeight * gridHeight / contentHeight);
            int top = gridY + (int) ((gridHeight - thumb) * scroll / (contentHeight - gridHeight));
            RoundRects.fill(graphics, gridX + gridWidth + 3, top, 2, thumb, 1, 0x80FFFFFF);
        }
    }

    private String hint() {
        return switch (tab) {
            case EMOTES ->
                "In a world, hold "
                        + IncariKeys.EMOTE_WHEEL.getTranslatedKeyMessage().getString();
            case NAMETAGS -> "Unless the server colors names";
            case HATS -> "Hidden under a helmet";
            case BACK -> "Hidden under an elytra";
            default -> "";
        };
    }

    @Override
    public void onClose() {
        if (minecraft != null) {
            minecraft.gui.setScreen(parent);
        }
    }

    @Override
    public boolean isPauseScreen() {
        return true;
    }

    /** A line in the sidebar: one kind of cosmetic, with how many the player has. */
    private final class Entry extends AbstractButton {

        private final String value;
        private final Tab target;

        Entry(Component label, String value, int x, int y, int width, Tab target) {
            super(x, y, width, ENTRY, label);
            this.value = value;
            this.target = target;
        }

        @Override
        public void onPress(InputWithModifiers input) {
            choose(target);
        }

        @Override
        protected void extractContents(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
            boolean on = tab == target;
            if (on || isHoveredOrFocused()) {
                RoundRects.fill(
                        graphics,
                        getX(),
                        getY(),
                        getWidth(),
                        getHeight(),
                        Theme.RADIUS,
                        on ? Theme.BUTTON_HOT : Theme.BUTTON);
            }
            if (on) {
                RoundRects.fill(graphics, getX(), getY() + 4, 2, getHeight() - 8, 1, Theme.ACCENT);
            }
            int textY = getY() + (getHeight() - 7) / 2;
            graphics.text(font, getMessage(), getX() + 7, textY, on ? Theme.TEXT : Theme.MUTED, false);
            graphics.text(font, value, getRight() - 6 - font.width(value), textY, Theme.MUTED, false);
        }

        @Override
        protected void updateWidgetNarration(NarrationElementOutput output) {
            defaultButtonNarrationText(output);
        }
    }

    /** The switch for other players' cosmetics. */
    private final class OthersSwitch extends AbstractButton {

        OthersSwitch(int x, int y, int width) {
            super(x, y, width, 12, Component.literal("Show others' cosmetics"));
            setTooltip(Ui.tooltip("Off hides other players' capes, hats, wings, trails and emotes. Yours still show."));
        }

        @Override
        public void onPress(InputWithModifiers input) {
            wardrobe.setShowOthers(!wardrobe.showOthers());
        }

        @Override
        protected void extractContents(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
            boolean on = wardrobe.showOthers();
            // Cut short of the switch beside a narrow preview; the tooltip says what it does.
            graphics.text(
                    font,
                    Ui.fit(font, getMessage(), getWidth() - Panels.SWITCH_WIDTH - 4),
                    getX(),
                    getY() + 2,
                    isHoveredOrFocused() ? Theme.TEXT : Theme.MUTED,
                    false);
            Panels.toggle(graphics, getRight() - Panels.SWITCH_WIDTH, getY() + 1, on, 1.0F);
        }

        @Override
        protected void updateWidgetNarration(NarrationElementOutput output) {
            defaultButtonNarrationText(output);
        }
    }

    /**
     * A card: a cosmetic, None for a kind (takes it off), an outfit, or, on the Outfits tab with
     * neither, Take Off All.
     */
    private final class Card extends AbstractButton {

        private static final int TRASH = 9;

        private final @Nullable Cosmetic cosmetic;
        private final Cosmetic.@Nullable Slot slot;
        private final Wardrobe.@Nullable Outfit outfit;
        private @Nullable Loadout tried;
        private @Nullable Loadout triedFrom;

        Card(@Nullable Cosmetic cosmetic, Cosmetic.@Nullable Slot slot, Wardrobe.@Nullable Outfit outfit) {
            super(0, 0, CARD_WIDTH, CARD_HEIGHT, label(cosmetic, outfit, slot));
            this.cosmetic = cosmetic;
            this.slot = slot;
            this.outfit = outfit;
        }

        private static Component label(
                @Nullable Cosmetic cosmetic, Wardrobe.@Nullable Outfit outfit, Cosmetic.@Nullable Slot slot) {
            if (cosmetic != null) {
                return Component.literal(cosmetic.name());
            }
            if (outfit != null) {
                return Component.literal(outfit.name());
            }
            return Component.literal(slot == null ? "Take Off All" : "None");
        }

        /** What the preview wears while the pointer's here. */
        @SuppressWarnings("ReferenceEquality") // the same loadout object, not an equal one
        @Nullable
        Loadout tryOn() {
            Loadout worn = wardrobe.worn();
            if (tried != null && triedFrom == worn) {
                return tried;
            }
            Loadout next;
            if (cosmetic != null) {
                next = cosmetic.slot().worn() && wardrobe.owns(cosmetic) ? worn.with(cosmetic) : null;
            } else if (outfit != null) {
                next = outfit.loadout();
            } else if (slot != null) {
                next = worn.without(slot);
            } else {
                next = Loadout.EMPTY;
            }
            tried = next;
            triedFrom = worn;
            return next;
        }

        private boolean worn() {
            Loadout worn = wardrobe.worn();
            if (cosmetic != null) {
                return worn.wears(cosmetic);
            }
            if (outfit != null) {
                return worn.equals(outfit.loadout());
            }
            return slot != null && slot.worn() && worn.in(slot) == null;
        }

        @Override
        public void onPress(InputWithModifiers input) {
            press(false);
        }

        @Override
        public void onClick(MouseButtonEvent event, boolean doubleClick) {
            press(outfit != null && overTrash(event.x(), event.y()));
        }

        private void press(boolean trash) {
            PlayerPreview player = preview;
            Wardrobe.Outfit saved = outfit;
            if (saved != null) {
                if (trash) {
                    wardrobe.deleteOutfit(saved);
                    notice = Component.literal("Deleted " + saved.name() + ".");
                    rebuildWidgets();
                } else {
                    wardrobe.wear(saved.loadout());
                }
                return;
            }
            Cosmetic picked = cosmetic;
            if (picked == null) {
                if (slot != null) {
                    wardrobe.takeOff(slot);
                } else {
                    wardrobe.wear(Loadout.EMPTY);
                }
                return;
            }
            if (!wardrobe.owns(picked)) {
                return;
            }
            if (picked.slot() == Cosmetic.Slot.EMOTE) {
                if (player != null) {
                    player.play(picked);
                }
                return;
            }
            if (wardrobe.worn().wears(picked)) {
                wardrobe.takeOff(picked.slot());
            } else {
                wardrobe.wear(picked);
                if (player != null) {
                    player.show(picked);
                }
            }
        }

        private boolean overTrash(double x, double y) {
            return x >= getRight() - 5 - TRASH - 2 && y <= getY() + 5 + TRASH + 2;
        }

        @Override
        public boolean isMouseOver(double mouseX, double mouseY) {
            return super.isMouseOver(mouseX, mouseY) && mouseY >= gridY && mouseY < gridY + gridHeight;
        }

        @Override
        protected void extractContents(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
            boolean on = worn();
            boolean owned = cosmetic == null || wardrobe.owns(cosmetic);
            Panels.card(
                    graphics, getX(), getY(), getWidth(), getHeight(), isHoveredOrFocused(), on, owned ? 1.0F : 0.5F);
            int artX = getX() + getWidth() / 2;
            int artY = getY() + 6;
            picture(graphics, artX, artY);
            // An outfit's name can be 24 characters: cut to the card, not into the next one.
            String message = Ui.fit(font, getMessage().getString(), getWidth() - 8);
            int textWidth = font.width(message);
            int textX = getX() + Math.max(4, (getWidth() - textWidth) / 2);
            graphics.text(font, message, textX, getBottom() - 14, owned ? Theme.TEXT : Theme.MUTED, false);
            if (on) {
                Icons.CHECK.draw(graphics, getRight() - 13, getY() + 5, 8, Theme.ACCENT);
            }
            if (outfit != null && isHovered()) {
                boolean hot = overTrash(mouseX, mouseY);
                Icons.TRASH.draw(
                        graphics, getRight() - 5 - TRASH, getY() + 5, TRASH, hot ? Theme.NEGATIVE : Theme.MUTED);
            }
            if (!owned) {
                graphics.text(font, "Locked", getX() + 5, getY() + 5, Theme.MUTED, false);
            }
        }

        /** The card's picture, 44 tall, centered on {@code x}. */
        private void picture(GuiGraphicsExtractor graphics, int x, int y) {
            Cosmetic shown = cosmetic;
            if (shown == null) {
                if (outfit != null) {
                    outfitPicture(graphics, x, y, outfit.loadout());
                } else {
                    Icons.CLOSE.draw(graphics, x - 8, y + 14, 16, Theme.MUTED);
                }
                return;
            }
            switch (shown.slot()) {
                case CAPE -> {
                    // The cape's outer side, as it hangs: 10x16 pixels at twice the size.
                    Identifier texture = CosmeticTextures.capeId(shown);
                    graphics.blit(
                            RenderPipelines.GUI_TEXTURED,
                            texture,
                            x - 13,
                            y + 1,
                            1,
                            1,
                            26,
                            42,
                            10,
                            16,
                            CosmeticArt.CAPE_WIDTH,
                            CosmeticArt.CAPE_HEIGHT);
                }
                case HAT, BACK -> {
                    Identifier icon = CosmeticTextures.icon(shown);
                    if (icon != null) {
                        graphics.blit(
                                RenderPipelines.GUI_TEXTURED,
                                icon,
                                x - 18,
                                y + 4,
                                0,
                                0,
                                36,
                                36,
                                CosmeticArt.ICON,
                                CosmeticArt.ICON,
                                CosmeticArt.ICON,
                                CosmeticArt.ICON);
                    }
                }
                case NAMETAG -> {
                    Component name = NameTags.styled(PlayerName.name(), shown);
                    int nameWidth = Math.min(getWidth() - 8, font.width(name));
                    graphics.fill(x - nameWidth / 2 - 3, y + 15, x + nameWidth / 2 + 3, y + 27, 0x60000000);
                    graphics.enableScissor(getX() + 3, y, getRight() - 3, y + 40);
                    graphics.text(font, name, x - font.width(name) / 2, y + 17, 0xFFFFFFFF, false);
                    graphics.disableScissor();
                }
                case TRAIL -> trailPicture(graphics, x, y, shown);
                case EMOTE -> Icons.PERSON.draw(graphics, x - 12, y + 8, 24, Theme.TEXT);
            }
        }

        private void trailPicture(GuiGraphicsExtractor graphics, int x, int y, Cosmetic trail) {
            int[] colors =
                    switch (trail.id()) {
                        case "trail.hearts" -> new int[] {0xFFFF5A7A, 0xFFFF8FA6};
                        case "trail.sparkles" -> new int[] {0xFFFFF4C2, 0xFFFFFFFF};
                        case "trail.notes" -> new int[] {0xFF6AF2B0, 0xFFA06CFF, 0xFFFFD36A};
                        default -> new int[] {0xFFFFFFFF, 0xFFA9DCFF};
                    };
            // Dots drifting up from the bottom left, bigger toward the start.
            for (int i = 0; i < 7; i++) {
                int dotX = x - 16 + i * 5;
                int dotY = y + 34 - i * 4 - (i % 2) * 3;
                int size = Math.max(2, 6 - i / 2);
                RoundRects.fill(graphics, dotX, dotY, size, size, size / 2, colors[i % colors.length]);
            }
        }

        private void outfitPicture(GuiGraphicsExtractor graphics, int x, int y, Loadout loadout) {
            // Up to four of what the outfit wears, small, in a row.
            List<Cosmetic> worn = new ArrayList<>();
            for (Cosmetic.Slot each : Cosmetic.Slot.values()) {
                Cosmetic in = loadout.in(each);
                if (in != null && worn.size() < 4) {
                    worn.add(in);
                }
            }
            if (worn.isEmpty()) {
                Icons.HANGER.draw(graphics, x - 10, y + 12, 20, Theme.MUTED);
                return;
            }
            int each = 14;
            int startX = x - (worn.size() * (each + 2)) / 2;
            for (int i = 0; i < worn.size(); i++) {
                Cosmetic c = worn.get(i);
                int cx = startX + i * (each + 2);
                Identifier icon = c.slot() == Cosmetic.Slot.CAPE ? null : CosmeticTextures.icon(c);
                if (c.slot() == Cosmetic.Slot.CAPE) {
                    graphics.blit(
                            RenderPipelines.GUI_TEXTURED,
                            CosmeticTextures.capeId(c),
                            cx + 2,
                            y + 10,
                            1,
                            1,
                            10,
                            16,
                            10,
                            16,
                            CosmeticArt.CAPE_WIDTH,
                            CosmeticArt.CAPE_HEIGHT);
                } else if (icon != null) {
                    graphics.blit(
                            RenderPipelines.GUI_TEXTURED,
                            icon,
                            cx,
                            y + 12,
                            0,
                            0,
                            each,
                            each,
                            CosmeticArt.ICON,
                            CosmeticArt.ICON,
                            CosmeticArt.ICON,
                            CosmeticArt.ICON);
                } else {
                    int color = c.slot() == Cosmetic.Slot.NAMETAG ? 0xFF000000 | NameTags.colors(c)[0] : Theme.MUTED;
                    RoundRects.fill(graphics, cx + 3, y + 15, 8, 8, 4, ARGB.opaque(color));
                }
            }
        }

        @Override
        protected void updateWidgetNarration(NarrationElementOutput output) {
            defaultButtonNarrationText(output);
        }
    }
}
