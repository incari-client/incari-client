package incari.client.net;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.function.IntConsumer;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.client.gui.components.ObjectSelectionList;
import net.minecraft.client.gui.components.StringWidget;
import net.minecraft.client.gui.layouts.HeaderAndFooterLayout;
import net.minecraft.client.gui.layouts.LinearLayout;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.CommonComponents;
import net.minecraft.network.chat.Component;
import org.jspecify.annotations.Nullable;

/**
 * Every version a server can be joined as, in one scrolling list with a search box, opened from the
 * edit-server screen's version button. Cycling a button through some forty releases to reach 1.8
 * took a click each.
 *
 * <p>English only for now; Incari's screens don't have language files yet.
 */
final class VersionSelectScreen extends Screen {

    private static final int ROW_HEIGHT = 18;
    private static final int SEARCH_HEIGHT = 15;

    private final Screen parent;
    private final List<Choice> choices;
    private final IntConsumer onChoose;
    /**
     * The protocol last picked: kept here, not only in the list, so a search that hides it and is
     * then cleared (or a resize, which makes a new list) picks it again rather than the one the
     * screen opened with.
     */
    private int picked;

    private final HeaderAndFooterLayout layout = new HeaderAndFooterLayout(this);
    private @Nullable VersionList list;
    private @Nullable EditBox search;

    /** A choice: its protocol ({@link ServerVersions#AUTO} for automatic), name and a note. */
    record Choice(int protocol, String name, String note) {}

    VersionSelectScreen(Screen parent, List<Choice> choices, int initial, IntConsumer onChoose) {
        super(Component.literal("ViaVersion"));
        this.parent = parent;
        this.choices = choices;
        this.picked = initial;
        this.onChoose = onChoose;
    }

    @Override
    protected void init() {
        LinearLayout header = layout.addToHeader(LinearLayout.vertical().spacing(4));
        header.defaultCellSetting().alignHorizontallyCenter();
        header.addChild(new StringWidget(this.title, this.font));
        EditBox searchBox =
                header.addChild(new EditBox(this.font, 0, 0, 200, SEARCH_HEIGHT, Component.literal("Search")));
        searchBox.setHint(Component.literal("Search versions...").withStyle(EditBox.SEARCH_HINT_STYLE));
        search = searchBox;
        layout.setHeaderHeight(12 + 9 + 4 + SEARCH_HEIGHT);

        VersionList versions = layout.addToContents(new VersionList(this.minecraft));
        list = versions;
        searchBox.setResponder(versions::filter);

        LinearLayout footer = layout.addToFooter(LinearLayout.horizontal().spacing(8));
        footer.addChild(
                Button.builder(CommonComponents.GUI_DONE, button -> done()).build());
        footer.addChild(
                Button.builder(CommonComponents.GUI_CANCEL, button -> onClose()).build());

        layout.visitWidgets(this::addRenderableWidget);
        repositionElements();
    }

    @Override
    protected void setInitialFocus() {
        EditBox searchBox = search;
        if (searchBox != null) {
            setInitialFocus(searchBox);
        } else {
            super.setInitialFocus();
        }
    }

    @Override
    protected void repositionElements() {
        layout.arrangeElements();
        VersionList versions = list;
        if (versions != null) {
            versions.updateSize(this.width, layout);
        }
    }

    @Override
    public boolean keyPressed(KeyEvent event) {
        // Enter in the search box takes the version picked, or the only one the search left.
        if (event.isConfirmation() && search != null && search.isFocused() && list != null) {
            VersionList.Entry only =
                    list.children().size() == 1 ? list.children().getFirst() : null;
            if (list.getSelected() != null || only != null) {
                if (list.getSelected() == null) {
                    list.setSelected(only);
                }
                done();
                return true;
            }
        }
        return super.keyPressed(event);
    }

    private void done() {
        VersionList versions = list;
        VersionList.Entry picked = versions == null ? null : versions.getSelected();
        if (picked != null) {
            onChoose.accept(picked.choice.protocol());
        }
        onClose();
    }

    @Override
    public void onClose() {
        this.minecraft.gui.setScreen(parent);
    }

    private final class VersionList extends ObjectSelectionList<VersionList.Entry> {

        VersionList(Minecraft minecraft) {
            super(
                    minecraft,
                    VersionSelectScreen.this.width,
                    layout.getContentHeight(),
                    layout.getHeaderHeight(),
                    ROW_HEIGHT);
            filter("");
        }

        /** Shows the choices whose name holds {@code text} ("1.8", "26"), keeping the pick if it's still listed. */
        void filter(String text) {
            String wanted = text.trim().toLowerCase(Locale.ROOT);
            List<Entry> shown = new ArrayList<>();
            for (Choice choice : choices) {
                if (wanted.isEmpty() || choice.name().toLowerCase(Locale.ROOT).contains(wanted)) {
                    shown.add(new Entry(choice));
                }
            }
            replaceEntries(shown);
            for (Entry entry : shown) {
                if (entry.choice.protocol() == picked) {
                    setSelected(entry);
                    centerScrollOn(entry);
                }
            }
            refreshScrollAmount();
        }

        @Override
        public void setSelected(@Nullable Entry entry) {
            super.setSelected(entry);
            if (entry != null) {
                picked = entry.choice.protocol();
            }
        }

        private final class Entry extends ObjectSelectionList.Entry<Entry> {
            final Choice choice;

            Entry(Choice choice) {
                this.choice = choice;
            }

            @Override
            public void extractContent(
                    GuiGraphicsExtractor graphics, int mouseX, int mouseY, boolean hovered, float a) {
                int textY = getContentYMiddle() - 4;
                graphics.text(font, choice.name(), getContentX() + 4, textY, 0xFFFFFFFF);
                if (!choice.note().isEmpty()) {
                    graphics.text(
                            font, choice.note(), getContentRight() - 4 - font.width(choice.note()), textY, 0xFF808080);
                }
            }

            @Override
            public boolean mouseClicked(MouseButtonEvent event, boolean doubleClick) {
                setSelected(this);
                if (doubleClick) {
                    done();
                }
                return true;
            }

            @Override
            public boolean keyPressed(KeyEvent event) {
                if (event.isSelection()) {
                    setSelected(this);
                    done();
                    return true;
                }
                return super.keyPressed(event);
            }

            @Override
            public Component getNarration() {
                return Component.translatable("narrator.select", choice.name());
            }
        }
    }
}
