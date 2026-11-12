package incari.client.modules;

import incari.client.cosmetics.WardrobeScreen;
import incari.client.ui.GuiPreset;
import incari.client.ui.Icons;
import incari.client.ui.Logo;
import incari.client.ui.Look;
import incari.client.ui.Panels;
import incari.client.ui.RoundRects;
import incari.client.ui.ScreenPanels;
import incari.client.ui.SmoothFont;
import incari.client.ui.Theme;
import incari.client.ui.Ui;
import incari.framework.Category;
import incari.framework.Module;
import incari.framework.ModuleSystem;
import incari.framework.setting.ChoiceSetting;
import java.util.ArrayList;
import java.util.EnumMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractButton;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.client.gui.narration.NarrationElementOutput;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.InputWithModifiers;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.CommonComponents;
import net.minecraft.network.chat.Component;
import net.minecraft.util.ARGB;
import net.minecraft.util.FormattedCharSequence;
import org.jspecify.annotations.Nullable;

/**
 * The module menu (ADR 0046, in ADR 0048's look): one glass panel, categories down the left with
 * their counts, the active profile and the GUI preset below them, a search field over a grid of
 * module cards, and Cosmetics and Edit HUD beside the search, which open the Wardrobe and the HUD
 * editor. A card's switch shows
 * whether the module is on; a click on the card turns it on or off, a click on its gear opens its
 * settings. Opened by the module menu key (Right Shift), the
 * title screen or the pause menu.
 */
final class ModuleListScreen extends Screen {

    private static final int SIDEBAR = 98;
    private static final int PADDING = 10;
    private static final int ENTRY = 16;
    private static final int CARD_HEIGHT = 46;
    private static final int GAP = 6;
    private static final int SCROLL_STEP = 24;
    private static final Component EDIT_HUD = Component.literal("编辑 HUD");
    private static final Component COSMETICS = Component.literal("外观");
    /** The narrowest the search field gets before Edit HUD and Cosmetics lose their words. */
    private static final int MIN_SEARCH = 80;
    /** An icon-only menu button: the icon and its margins. */
    private static final int ICON_BUTTON = 25;
    /** The sidebar's mark, a letter at a time: spaced out wider than the font sets it. */
    private static final String[] LETTERS = {"I", "N", "C", "A", "R", "I"};

    private final @Nullable Screen parent;
    private final ModuleSystem system;
    private final List<Card> cards = new ArrayList<>();

    private @Nullable Category category;
    private String query = "";
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
    private int shown;

    ModuleListScreen(@Nullable Screen parent, ModuleSystem system) {
        super(Component.literal("模块"));
        this.parent = parent;
        this.system = system;
    }

    @Override
    protected void init() {
        panelWidth = Math.min(width - 24, 480);
        panelHeight = Math.min(height - 20, 296);
        panelX = (width - panelWidth) / 2;
        panelY = (height - panelHeight) / 2;
        ScreenPanels.clear(this);
        ScreenPanels.add(this, panelX, panelY, panelWidth, panelHeight);

        Map<Category, Integer> counts = new EnumMap<>(Category.class);
        int listed = 0;
        for (Module module : system.modules()) {
            if (!module.isAlwaysOn()) {
                counts.merge(module.category(), 1, Integer::sum);
                listed++;
            }
        }
        int x = panelX + 8;
        int entryWidth = SIDEBAR - 16;
        int y = panelY + 34;
        int bottom = panelY + panelHeight - 8 - Theme.CONTROL_HEIGHT;
        // On a short screen (GUI scale 3 or 4 on a small window) the entries close up so the
        // categories don't run into the Theme, GUI and Profile lines.
        int step = y + counts.size() * (ENTRY + 2) + ENTRY + 2 <= bottom - 3 * (ENTRY + 2) ? ENTRY + 2 : ENTRY;
        addRenderableWidget(new Entry(
                Component.literal("全部"),
                String.valueOf(listed),
                x,
                y,
                entryWidth,
                () -> category == null,
                () -> choose(null)));
        for (Category each : Category.values()) {
            Integer count = counts.get(each);
            if (count == null) {
                continue;
            }
            y += step;
            addRenderableWidget(new Entry(
                    Component.literal(each.label()),
                    String.valueOf(count),
                    x,
                    y,
                    entryWidth,
                    () -> category == each,
                    () -> choose(each)));
        }
        // The theme is the client's, not a module: a line here, like the GUI preset.
        Module theme = system.module("theme");
        if (theme != null) {
            addRenderableWidget(new Entry(
                    Component.literal("主题"),
                    theme.settings().get("preset") instanceof ChoiceSetting<?> preset ? preset.label() : "",
                    x,
                    bottom - 3 * step,
                    entryWidth,
                    () -> false,
                    () -> IncariModules.openSettings(this, theme)));
        }
        addRenderableWidget(new Entry(
                Component.literal("界面"), label(Look.preset()), x, bottom - 2 * step, entryWidth, () -> false, () -> {
                    Look.set(Look.normal() ? GuiPreset.POTATO : GuiPreset.NORMAL);
                    rebuildWidgets();
                }));
        addRenderableWidget(new Entry(
                Component.literal("配置"),
                system.activeProfile(),
                x,
                bottom - step,
                entryWidth,
                () -> false,
                () -> IncariModules.openProfiles(this)));
        var done = Ui.button(CommonComponents.GUI_DONE, entryWidth, this::onClose);
        done.setPosition(x, bottom);
        addRenderableWidget(done);

        gridX = panelX + SIDEBAR + PADDING;
        gridWidth = panelX + panelWidth - PADDING - gridX;
        // The HUD editor, beside the search: the layout is a click from the menu's first view.
        // Cosmetics (ADR 0061) beside it: what's worn isn't a module, but players look for it here.
        // Where their words would leave the search too narrow to type in, only their icons show.
        int editWidth = 24 + font.width(EDIT_HUD) + 10;
        int cosmeticsWidth = 24 + font.width(COSMETICS) + 10;
        boolean iconsOnly = gridWidth - editWidth - cosmeticsWidth - 2 * Theme.GAP < MIN_SEARCH;
        if (iconsOnly) {
            editWidth = ICON_BUTTON;
            cosmeticsWidth = ICON_BUTTON;
        }
        var editHud = Ui.menuButton(
                iconsOnly ? Component.empty() : EDIT_HUD,
                Icons.LAYOUT,
                editWidth,
                () -> IncariModules.openHudEditor(this));
        editHud.setPosition(gridX + gridWidth - editWidth, panelY + PADDING);
        addRenderableWidget(editHud);
        var cosmetics = Ui.menuButton(
                iconsOnly ? Component.empty() : COSMETICS,
                Icons.HANGER,
                cosmeticsWidth,
                () -> minecraft.gui.setScreen(new WardrobeScreen(this)));
        cosmetics.setPosition(gridX + gridWidth - editWidth - Theme.GAP - cosmeticsWidth, panelY + PADDING);
        addRenderableWidget(cosmetics);
        if (iconsOnly) {
            editHud.setTooltip(Ui.tooltip(EDIT_HUD.getString()));
            cosmetics.setTooltip(Ui.tooltip(COSMETICS.getString()));
        }
        EditBox search = Ui.textField(
                font, Component.literal("搜索"), gridWidth - editWidth - cosmeticsWidth - 2 * Theme.GAP, 64);
        search.setPosition(gridX, panelY + PADDING);
        search.setHint(Component.literal("搜索模块"));
        search.setValue(query);
        search.setResponder(text -> {
            query = text;
            scroll = 0;
            layOutCards();
        });
        addRenderableWidget(search);
        gridY = panelY + PADDING + Theme.CONTROL_HEIGHT + PADDING;
        gridHeight = panelY + panelHeight - PADDING - gridY;

        cards.clear();
        for (Module module : system.modules()) {
            if (module.isAlwaysOn()) {
                continue;
            }
            Card card = new Card(module);
            cards.add(card);
            // Drawn by this screen inside the grid's clip, not with the other widgets.
            addWidget(card);
        }
        layOutCards();
    }

    private void choose(@Nullable Category chosen) {
        category = chosen;
        scroll = 0;
        layOutCards();
    }

    /** Places the cards that match the category and the search, in rows, scrolled. */
    private void layOutCards() {
        // One column where two would cut most names short (a small window at a large GUI scale).
        int columns = gridWidth >= 420 ? 3 : gridWidth >= 260 ? 2 : 1;
        int cardWidth = (gridWidth - (columns - 1) * GAP) / columns;
        String needle = query.trim().toLowerCase(Locale.ROOT);
        shown = 0;
        for (Card card : cards) {
            Module module = card.module;
            boolean matches = (category == null || module.category() == category)
                    && (needle.isEmpty()
                            || module.name().toLowerCase(Locale.ROOT).contains(needle)
                            || module.description().toLowerCase(Locale.ROOT).contains(needle));
            card.visible = matches;
            if (matches) {
                int column = shown % columns;
                int row = shown / columns;
                card.setRectangle(
                        cardWidth,
                        CARD_HEIGHT,
                        gridX + column * (cardWidth + GAP),
                        gridY + row * (CARD_HEIGHT + GAP) - (int) scroll);
                card.wrap(font);
                shown++;
            }
        }
        int rows = (shown + columns - 1) / columns;
        contentHeight = Math.max(0, rows * (CARD_HEIGHT + GAP) - GAP);
        double max = Math.max(0, contentHeight - gridHeight);
        if (scroll > max) {
            scroll = max;
            layOutCards();
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
        // The sidebar's mark, and the line between it and the grid.
        Logo.draw(graphics, panelX + 11, panelY + 11, 12, 1);
        int letterX = panelX + 29;
        for (String letter : LETTERS) {
            graphics.text(font, letter, letterX, panelY + 14, Theme.TEXT, false);
            letterX += font.width(letter) + 2;
        }
        graphics.fill(panelX + SIDEBAR, panelY + 8, panelX + SIDEBAR + 1, panelY + panelHeight - 8, 0x1AFFFFFF);
        super.extractRenderState(graphics, mouseX, mouseY, a);

        graphics.enableScissor(gridX - 6, gridY, gridX + gridWidth + 6, gridY + gridHeight);
        for (Card card : cards) {
            if (card.visible && card.getBottom() > gridY && card.getY() < gridY + gridHeight) {
                card.extractRenderState(graphics, mouseX, mouseY, a);
            }
        }
        graphics.disableScissor();
        if (shown == 0) {
            graphics.centeredText(
                    font,
                    Component.literal("没有符合 \"" + query.trim() + "\" 的模块"),
                    gridX + gridWidth / 2,
                    gridY + 24,
                    Theme.MUTED);
        }
        if (contentHeight > gridHeight) {
            int thumb = Math.max(16, gridHeight * gridHeight / contentHeight);
            int top = gridY + (int) ((gridHeight - thumb) * scroll / (contentHeight - gridHeight));
            RoundRects.fill(graphics, gridX + gridWidth + 3, top, 2, thumb, 1, 0x80FFFFFF);
        }
    }

    @Override
    public void onClose() {
        minecraft.gui.setScreen(parent);
    }

    @Override
    public void removed() {
        system.flush();
    }

    @Override
    public boolean isPauseScreen() {
        // Like the game's own menus: singleplayer pauses while it's open.
        return true;
    }

    private static String label(GuiPreset preset) {
        return switch (preset) {
            case POTATO -> "土豆";
            case NORMAL -> "普通";
        };
    }

    /** A line in the sidebar: a category, the GUI preset, the profile. */
    private static final class Entry extends AbstractButton {

        private final String value;
        private final java.util.function.BooleanSupplier chosen;
        private final Runnable action;

        Entry(
                Component label,
                String value,
                int x,
                int y,
                int width,
                java.util.function.BooleanSupplier chosen,
                Runnable action) {
            super(x, y, width, ENTRY, label);
            this.value = value;
            this.chosen = chosen;
            this.action = action;
            // A value cut to fit (a long profile name) is shown whole under the pointer.
            Font font = net.minecraft.client.Minecraft.getInstance().font;
            if (!shown(font).equals(value)) {
                setTooltip(Ui.tooltip(label.getString() + ": " + value));
            }
        }

        /** The value as drawn: cut short of the label. */
        private String shown(Font font) {
            int labelRight = getX() + 7 + font.width(getMessage()) + 6;
            return Ui.fit(font, value, getRight() - 6 - labelRight);
        }

        @Override
        public void onPress(InputWithModifiers input) {
            action.run();
        }

        @Override
        protected void extractContents(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
            boolean on = chosen.getAsBoolean();
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
            Font font = net.minecraft.client.Minecraft.getInstance().font;
            int textY = getY() + (getHeight() - 7) / 2;
            graphics.text(font, getMessage(), getX() + 7, textY, on ? Theme.TEXT : Theme.MUTED, false);
            // A long profile name is cut, never drawn over the label.
            String shown = shown(font);
            graphics.text(font, shown, getRight() - 6 - font.width(shown), textY, Theme.MUTED, false);
        }

        @Override
        protected void updateWidgetNarration(NarrationElementOutput output) {
            defaultButtonNarrationText(output);
        }
    }

    /** A module's card. */
    private final class Card extends AbstractButton {

        private static final int GEAR = 8;

        private final Module module;
        private final boolean hasSettings;
        private List<FormattedCharSequence> description = List.of();
        private String title = "";
        private int wrappedWidth = -1;
        private int wrappedFont = -1;

        Card(Module module) {
            super(0, 0, 0, CARD_HEIGHT, Component.literal(module.name()));
            this.module = module;
            this.hasSettings =
                    module.settings().anyShown() || !module.hudElements().isEmpty();
        }

        /** Fits the name and wraps the description to the card's width, if it or the font changed. */
        void wrap(Font font) {
            if (wrappedWidth != getWidth() || wrappedFont != SmoothFont.generation()) {
                wrappedWidth = getWidth();
                wrappedFont = SmoothFont.generation();
                // The name stops short of the switch; cut, it's shown whole under the pointer.
                title = Ui.fit(font, module.name(), getWidth() - 16 - Panels.SWITCH_WIDTH - 4);
                setTooltip(title.equals(module.name()) ? null : Ui.tooltip(module.name()));
                description = Ui.wrap(font, module.description(), getWidth() - 16 - (hasSettings ? GEAR + 4 : 0), 2);
            }
        }

        @Override
        public void onPress(InputWithModifiers input) {
            module.setEnabled(!module.isEnabled());
        }

        @Override
        public void onClick(MouseButtonEvent event, boolean doubleClick) {
            if (hasSettings && overGear(event.x(), event.y())) {
                IncariModules.openSettings(ModuleListScreen.this, module);
            } else {
                onPress(event);
            }
        }

        private boolean overGear(double x, double y) {
            return x >= getRight() - 8 - GEAR - 3 && y >= getBottom() - 8 - GEAR - 3;
        }

        @Override
        public boolean isMouseOver(double mouseX, double mouseY) {
            return super.isMouseOver(mouseX, mouseY) && mouseY >= gridY && mouseY < gridY + gridHeight;
        }

        @Override
        protected void extractContents(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
            boolean on = module.isEnabled();
            Panels.card(graphics, getX(), getY(), getWidth(), getHeight(), isHoveredOrFocused(), on, 1.0F);
            // Laid out again when a theme's font has just loaded.
            wrap(font);
            graphics.text(font, title, getX() + 8, getY() + 8, Theme.TEXT, false);
            Panels.toggle(graphics, getRight() - 8 - Panels.SWITCH_WIDTH, getY() + 7, on, 1.0F);
            int y = getY() + 21;
            for (FormattedCharSequence line : description) {
                graphics.text(font, line, getX() + 8, y, Theme.MUTED, false);
                y += 10;
            }
            if (hasSettings) {
                boolean hot = isHovered() && overGear(mouseX, mouseY);
                Icons.GEAR.draw(
                        graphics,
                        getRight() - 8 - GEAR,
                        getBottom() - 8 - GEAR,
                        GEAR,
                        hot ? Theme.ACCENT : ARGB.multiplyAlpha(Theme.MUTED, 0.8F));
            }
        }

        @Override
        protected void updateWidgetNarration(NarrationElementOutput output) {
            defaultButtonNarrationText(output);
        }
    }
}
