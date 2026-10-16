package incari.client.ui;

import java.util.List;
import java.util.Objects;
import net.minecraft.client.gui.components.StringWidget;
import net.minecraft.client.gui.layouts.HeaderAndFooterLayout;
import net.minecraft.client.gui.layouts.LinearLayout;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;
import org.jspecify.annotations.Nullable;

/**
 * The shape of Incari's menu screens: a title and a caption, a scrolling {@link ControlList}, and a
 * row of buttons at the bottom. A subclass gives the rows and the buttons; this lays them out and
 * keeps them current, rebuilding the rows when {@link #shape()} changes (a setting appears or goes)
 * and refreshing their controls when only {@link #revision()} does (a profile applied, a key bound).
 * The caption is checked every tick too, so it can say what just went wrong.
 */
public abstract class ListScreen extends Screen {

    private static final int HEADER_HEIGHT = 44;
    private static final int FOOTER_HEIGHT = 36;

    /** The screen to go back to; null for the game. */
    protected final @Nullable Screen parent;

    /** Made in each {@link #init}: a layout holds its screen, which isn't whole until then. */
    private @Nullable HeaderAndFooterLayout layout;

    private @Nullable ControlList list;
    private @Nullable StringWidget captionWidget;
    private Component shownCaption = Component.empty();
    private long shownRevision;
    private @Nullable Object shownShape;

    protected ListScreen(Component title, @Nullable Screen parent) {
        super(title);
        this.parent = parent;
    }

    /** The line under the title; empty for none. */
    protected Component caption() {
        return Component.empty();
    }

    /** {@code caption} in the muted color, unless it has a color of its own (a warning). */
    private static Component muted(Component caption) {
        return caption.getStyle().getColor() != null ? caption : caption.copy().withColor(Theme.MUTED);
    }

    /** The rows, built fresh. */
    protected abstract List<ControlList.Row> rows(ControlList list);

    /** Adds the footer's buttons. */
    protected abstract void footer(LinearLayout footer);

    /** A number that changes whenever what the rows show might have; checked every tick. */
    protected long revision() {
        return 0;
    }

    /** What decides which rows there are; the rows are rebuilt when it changes. */
    protected @Nullable Object shape() {
        return null;
    }

    @Override
    protected void init() {
        HeaderAndFooterLayout layout = new HeaderAndFooterLayout(this, HEADER_HEIGHT, FOOTER_HEIGHT);
        this.layout = layout;
        LinearLayout header = layout.addToHeader(LinearLayout.vertical().spacing(Theme.LINE_GAP));
        header.defaultCellSetting().alignHorizontallyCenter();
        header.addChild(new StringWidget(this.title.copy().withColor(Theme.TEXT), this.font));
        Component caption = caption();
        shownCaption = caption;
        captionWidget = null;
        if (!caption.getString().isEmpty()) {
            captionWidget = header.addChild(new StringWidget(muted(caption), this.font));
        }
        ControlList rows = layout.addToContents(
                new ControlList(this.minecraft, this.width, layout.getContentHeight(), layout.getHeaderHeight()));
        list = rows;
        shownRevision = revision();
        shownShape = shape();
        rows.setRows(rows(rows));
        footer(layout.addToFooter(LinearLayout.horizontal().spacing(Theme.GAP * 2)));
        layout.visitWidgets(this::addRenderableWidget);
        repositionElements();
    }

    @Override
    protected void repositionElements() {
        HeaderAndFooterLayout current = layout;
        if (current == null) {
            return;
        }
        StringWidget caption = captionWidget;
        if (caption != null) {
            // Here, not only in init: a resize lays the screen out again without init.
            caption.setMaxWidth(this.width - 40);
        }
        current.arrangeElements();
        ControlList rows = list;
        if (rows != null) {
            rows.updateSize(this.width, current);
        }
        // A glass panel behind the rows, drawn with the backdrop.
        ScreenPanels.clear(this);
        int rowWidth = rows != null ? rows.getRowWidth() : Theme.ROW_WIDTH;
        ScreenPanels.add(
                this,
                this.width / 2 - rowWidth / 2 - 12,
                current.getHeaderHeight() + 4,
                rowWidth + 24,
                current.getContentHeight() - 8);
    }

    @Override
    public void tick() {
        super.tick();
        Component caption = caption();
        if (!caption.equals(shownCaption)) {
            StringWidget widget = captionWidget;
            if (widget != null && !caption.getString().isEmpty()) {
                shownCaption = caption;
                widget.setMessage(muted(caption));
                repositionElements();
            } else {
                // A caption appearing or going changes the header's height: lay it all out again.
                rebuildWidgets();
                return;
            }
        }
        ControlList rows = list;
        long revision = revision();
        if (rows == null || revision == shownRevision) {
            return;
        }
        shownRevision = revision;
        Object shape = shape();
        if (Objects.equals(shape, shownShape)) {
            rows.refresh();
        } else {
            shownShape = shape;
            rows.setRows(rows(rows));
        }
    }

    /** Brings every row's controls up to date now, not at the next tick. */
    protected final void refreshRows() {
        ControlList rows = list;
        if (rows != null) {
            rows.refresh();
        }
    }

    @Override
    public void onClose() {
        this.minecraft.gui.setScreen(parent);
    }
}
