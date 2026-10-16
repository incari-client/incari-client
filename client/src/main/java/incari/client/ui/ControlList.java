package incari.client.ui;

import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.components.ContainerObjectSelectionList;
import net.minecraft.client.gui.components.events.GuiEventListener;
import net.minecraft.client.gui.narration.NarratableEntry;
import net.minecraft.network.chat.Component;
import org.jspecify.annotations.Nullable;

/**
 * A scrolling list of rows: a heading, or a label with controls at the row's right. The module
 * menu, the generated settings screens and the profiles screen are all one of these, so the restyle
 * changes how a row looks once.
 */
public final class ControlList extends ContainerObjectSelectionList<ControlList.Row> {

    /** A label's description is wrapped to this, as the game wraps its own tooltips. */
    private static final int TOOLTIP_WIDTH = 200;

    /**
     * Kept clear at each side of the rows: the scrollbar (its width and gap) and the panel's edge
     * ({@link ListScreen}), so a screen as narrow as the game allows (320) never cuts them off.
     */
    public static final int EDGE = 16;

    /** An empty list the width of the screen, between {@code y} and {@code y + height}. */
    public ControlList(Minecraft minecraft, int width, int height, int y) {
        super(minecraft, width, height, y, Theme.ROW_HEIGHT);
    }

    /** Replaces every row, keeping the scroll position where it can. */
    public void setRows(List<Row> rows) {
        double scroll = scrollAmount();
        replaceEntries(rows);
        setScrollAmount(scroll);
    }

    /** Brings every row's controls up to date with what they show. */
    public void refresh() {
        for (Row row : children()) {
            row.refresh();
        }
    }

    @Override
    public int getRowWidth() {
        return Math.max(0, Math.min(Theme.ROW_WIDTH, getWidth() - 2 * EDGE));
    }

    @Override
    protected int addEntry(Row row) {
        return addEntry(row, row.height());
    }

    /** A centered heading: a category, a HUD element. */
    public Row heading(Component text) {
        return new Heading(text);
    }

    /**
     * A label with controls right-aligned after it.
     *
     * @param tooltip shown over the label; empty for none
     * @param refresh run to bring the controls up to date with their values
     */
    public Row row(Component label, String tooltip, List<? extends AbstractWidget> controls, Runnable refresh) {
        return new Labelled(label, tooltip, controls, refresh);
    }

    /**
     * Widgets side by side across the whole row, each as wide as the others: a row of {@link
     * Ui#tile tiles}. The row is as tall as the tallest, and a gap.
     */
    public Row across(List<? extends AbstractWidget> widgets) {
        return new Across(widgets);
    }

    /** One row of the list. */
    public abstract static class Row extends ContainerObjectSelectionList.Entry<Row> {

        /** Brings the row's controls up to date. */
        void refresh() {}

        /** How tall the row is, with its padding. */
        int height() {
            return Theme.ROW_HEIGHT;
        }
    }

    private static final class Across extends Row {

        private final List<AbstractWidget> widgets;
        private final int height;

        Across(List<? extends AbstractWidget> widgets) {
            this.widgets = new ArrayList<>(widgets);
            int tallest = 0;
            for (AbstractWidget widget : widgets) {
                tallest = Math.max(tallest, widget.getHeight());
            }
            this.height = tallest + Theme.GAP + 2 * CONTENT_PADDING;
        }

        @Override
        int height() {
            return height;
        }

        @Override
        public void extractContent(GuiGraphicsExtractor graphics, int mouseX, int mouseY, boolean hovered, float a) {
            int count = widgets.size();
            if (count == 0) {
                return;
            }
            int each = (getContentWidth() - (count - 1) * Theme.GAP) / count;
            for (int i = 0; i < count; i++) {
                AbstractWidget widget = widgets.get(i);
                // The last one takes what rounding left, so the row's right edge lines up.
                int width = i == count - 1 ? getContentRight() - (getContentX() + i * (each + Theme.GAP)) : each;
                widget.setWidth(width);
                widget.setPosition(getContentX() + i * (each + Theme.GAP), getContentY());
                widget.extractRenderState(graphics, mouseX, mouseY, a);
            }
        }

        @Override
        public List<? extends GuiEventListener> children() {
            return widgets;
        }

        @Override
        public List<? extends NarratableEntry> narratables() {
            return widgets;
        }
    }

    private final class Heading extends Row {

        private final Component text;

        Heading(Component text) {
            this.text = text;
        }

        @Override
        public void extractContent(GuiGraphicsExtractor graphics, int mouseX, int mouseY, boolean hovered, float a) {
            int y = getContentBottom() - minecraft.font.lineHeight - 2;
            graphics.centeredText(
                    minecraft.font,
                    Ui.fit(minecraft.font, text, getContentWidth()),
                    getContentXMiddle(),
                    y,
                    Theme.ACCENT);
        }

        @Override
        public List<? extends GuiEventListener> children() {
            return List.of();
        }

        @Override
        public List<? extends NarratableEntry> narratables() {
            return List.of();
        }
    }

    private final class Labelled extends Row {

        private final Component label;
        private final @Nullable Component tooltip;
        private final List<AbstractWidget> controls;
        private final Runnable refresh;

        Labelled(Component label, String tooltip, List<? extends AbstractWidget> controls, Runnable refresh) {
            this.label = label;
            this.tooltip = tooltip.isEmpty() ? null : Component.literal(tooltip);
            this.controls = new ArrayList<>(controls);
            this.refresh = refresh;
        }

        @Override
        void refresh() {
            refresh.run();
        }

        @Override
        public void extractContent(GuiGraphicsExtractor graphics, int mouseX, int mouseY, boolean hovered, float a) {
            int x = getContentRight();
            int y = getContentY() + (getContentHeight() - Theme.CONTROL_HEIGHT) / 2;
            for (int i = controls.size() - 1; i >= 0; i--) {
                AbstractWidget control = controls.get(i);
                x -= control.getWidth();
                control.setPosition(x, y);
                control.extractRenderState(graphics, mouseX, mouseY, a);
                x -= Theme.GAP;
            }
            int labelY = getContentYMiddle() - minecraft.font.lineHeight / 2;
            // The label gets what the controls leave; cut, it shows whole in its tooltip.
            int room = x - getContentX();
            boolean cut = minecraft.font.width(label) > room;
            graphics.text(minecraft.font, Ui.fit(minecraft.font, label, room), getContentX(), labelY, Theme.TEXT);
            int labelRight = getContentX() + Math.min(room, minecraft.font.width(label));
            if ((cut || tooltip != null)
                    && mouseX >= getContentX()
                    && mouseX < labelRight
                    && mouseY >= getContentY()
                    && mouseY < getContentBottom()) {
                Component description = tooltip;
                Component shown = cut || description == null ? whole() : description;
                graphics.setTooltipForNextFrame(
                        minecraft.font, minecraft.font.split(shown, TOOLTIP_WIDTH), mouseX, mouseY);
            }
        }

        /** The label and its description, for a label cut short. */
        private Component whole() {
            Component description = tooltip;
            return description == null
                    ? label
                    : Component.empty().append(label).append("\n").append(description);
        }

        @Override
        public List<? extends GuiEventListener> children() {
            return controls;
        }

        @Override
        public List<? extends NarratableEntry> narratables() {
            return controls;
        }
    }
}
