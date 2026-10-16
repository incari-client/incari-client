package incari.client.online;

import incari.client.gui.HiddenPasswords;
import incari.client.ui.ScreenPanels;
import incari.client.ui.Theme;
import incari.client.ui.Ui;
import incari.online.Credentials;
import incari.online.Failure;
import incari.online.OnlineService;
import incari.online.Result;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Consumer;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.network.chat.Component;
import net.minecraft.util.FormattedCharSequence;
import org.jspecify.annotations.Nullable;

/**
 * The shape of the account screens (ADR 0058): one glass panel in the middle, a title, then lines
 * of text, labelled fields and buttons stacked down it, and a line for what happened last. A
 * subclass builds its contents in {@link #build}; this stacks them, sizes the panel to fit and
 * centres it. Enter runs the screen's main action; Escape goes back.
 *
 * <p>While a request is out, the screen is busy: its main action does nothing and the status line
 * says what it's waiting for. Answers come back on a later tick, on the game thread.
 *
 * <p>English only for now; Incari's screens don't have language files yet.
 */
abstract class FormScreen extends Screen {

    static final int CONTENT_WIDTH = 236;
    private static final int PADDING = 14;
    private static final int LINE = 10;
    private static final int GAP = 6;

    /** Where Escape and Back go. */
    protected final Screen parent;

    protected final OnlineService online = IncariOnline.service();

    /** Text drawn on the panel, each line at its height in the stack. */
    private record Line(FormattedCharSequence text, int y, int color, boolean centred) {}

    private final List<Line> lines = new ArrayList<>();
    private final List<AbstractWidget> stacked = new ArrayList<>();
    private final Map<String, String> kept = new HashMap<>();
    private int cursor;
    private int left;
    private int top;
    private int panelHeight;
    /** How far the panel is scrolled up, when it's taller than the screen; 0 when it fits. */
    private int scroll;

    private int maxScroll;
    /** Each stacked widget's y with the panel unscrolled. */
    private int[] baseY = new int[0];

    private int statusY = -1;

    /** In {@link #init}: a rebuild asked for now waits until it's done ({@link #rebuildWidgets}). */
    private boolean laying;

    private boolean layAgain;

    private Component status = Component.empty();
    private int statusColor = Theme.MUTED;
    private boolean busy;
    private @Nullable Runnable submit;

    protected FormScreen(String title, Screen parent) {
        super(Component.literal(title));
        this.parent = parent;
    }

    /** Adds the screen's contents, top to bottom, with the helpers below. */
    protected abstract void build();

    @Override
    protected final void init() {
        laying = true;
        try {
            lay();
        } finally {
            laying = false;
        }
        if (layAgain) {
            layAgain = false;
            rebuildWidgets();
        }
    }

    /**
     * An answer that comes at once (offline, signed out) asks for a rebuild from inside {@link
     * #build}: done there, the new layout would stack its widgets onto the half-built old one's.
     */
    @Override
    protected void rebuildWidgets() {
        if (laying) {
            layAgain = true;
            return;
        }
        super.rebuildWidgets();
    }

    private void lay() {
        // A resize lays the screen out again: what was typed stays.
        kept.clear();
        for (AbstractWidget widget : stacked) {
            if (widget instanceof EditBox box) {
                kept.put(box.getMessage().getString(), box.getValue());
            }
        }
        lines.clear();
        stacked.clear();
        submit = null;
        statusY = -1;
        left = this.width / 2 - CONTENT_WIDTH / 2;
        cursor = 0;
        build();
        panelHeight = cursor + 2 * PADDING + LINE + GAP;
        top = Math.max(4, (this.height - panelHeight) / 2);
        // A short screen (a Chromebook in fullscreen at GUI scale 3) scrolls the panel instead of
        // cutting off its bottom buttons.
        maxScroll = Math.max(0, top + panelHeight + 4 - this.height);
        scroll = Math.min(scroll, maxScroll);
        int shift = top + PADDING + LINE + GAP;
        baseY = new int[stacked.size()];
        for (int i = 0; i < stacked.size(); i++) {
            baseY[i] = stacked.get(i).getY() + shift;
        }
        applyScroll();
    }

    private void applyScroll() {
        for (int i = 0; i < stacked.size(); i++) {
            stacked.get(i).setY(baseY[i] - scroll);
        }
        ScreenPanels.clear(this);
        ScreenPanels.add(this, left - PADDING, top - scroll, CONTENT_WIDTH + 2 * PADDING, panelHeight);
    }

    @Override
    public boolean mouseScrolled(double mouseX, double mouseY, double scrollX, double scrollY) {
        if (maxScroll > 0) {
            scroll = Math.max(0, Math.min(maxScroll, scroll - (int) Math.round(scrollY * 12)));
            applyScroll();
            return true;
        }
        return super.mouseScrolled(mouseX, mouseY, scrollX, scrollY);
    }

    // ---------------------------------------------------------------------------------------------
    // Building
    // ---------------------------------------------------------------------------------------------

    /** Wrapped text across the panel. */
    protected final void text(String text, int color) {
        for (FormattedCharSequence line : this.font.split(Component.literal(text), CONTENT_WIDTH)) {
            lines.add(new Line(line, cursor, color, false));
            cursor += LINE;
        }
        cursor += GAP - 2;
    }

    /** One line of text, centred. */
    protected final void centred(String text, int color) {
        lines.add(new Line(Component.literal(text).getVisualOrderText(), cursor, color, true));
        cursor += LINE + GAP - 2;
    }

    /** Space down the panel. */
    protected final void gap(int height) {
        cursor += height;
    }

    /** A labelled text field across the panel. */
    protected final EditBox field(String label, int maxLength) {
        return addField(label, Ui.textField(this.font, Component.literal(label), CONTENT_WIDTH, maxLength));
    }

    /** A field for an Incari username: only the characters one may hold. */
    protected final EditBox usernameField(String label) {
        EditBox box = new EditBox(this.font, 0, 0, CONTENT_WIDTH, Theme.CONTROL_HEIGHT, Component.literal(label)) {
            @Override
            public void insertText(String input) {
                StringBuilder allowed = new StringBuilder(input.length());
                for (int i = 0; i < input.length(); i++) {
                    if (Credentials.usernameCharacter(input.charAt(i))) {
                        allowed.append(input.charAt(i));
                    }
                }
                super.insertText(allowed.toString());
            }
        };
        box.setMaxLength(Credentials.MAX_USERNAME);
        return addField(label, box);
    }

    private EditBox addField(String label, EditBox box) {
        lines.add(new Line(Component.literal(label).getVisualOrderText(), cursor, Theme.MUTED, false));
        cursor += LINE;
        String typed = kept.get(label);
        if (typed != null) {
            box.setValue(typed);
        }
        place(box, CONTENT_WIDTH, 0);
        return box;
    }

    /** A password field: shown as stars, never kept anywhere but the box. */
    protected final EditBox passwordField(String label) {
        EditBox box = field(label, Credentials.MAX_PASSWORD);
        HiddenPasswords.hide(box);
        return box;
    }

    /** A button across the panel. */
    protected final Button button(String label, Runnable action) {
        Button button = Ui.button(Component.literal(label), CONTENT_WIDTH, action);
        place(button, CONTENT_WIDTH, 0);
        return button;
    }

    /** Buttons side by side across the panel, sharing its width. */
    protected final List<Button> row(String[] labels, Runnable[] actions) {
        int each = (CONTENT_WIDTH - (labels.length - 1) * Theme.GAP) / labels.length;
        List<Button> buttons = new ArrayList<>();
        for (int i = 0; i < labels.length; i++) {
            int width = i == labels.length - 1 ? CONTENT_WIDTH - i * (each + Theme.GAP) : each;
            Button button = Ui.button(Component.literal(labels[i]), width, actions[i]);
            button.setX(left + i * (each + Theme.GAP));
            button.setY(cursor);
            stacked.add(button);
            addRenderableWidget(button);
            buttons.add(button);
        }
        cursor += Theme.CONTROL_HEIGHT + GAP;
        return buttons;
    }

    /** A name on the left, cut to fit, and buttons for it on the right. */
    protected final void item(String label, String[] buttons, Runnable[] actions) {
        int each = 58;
        int buttonsWidth = buttons.length * each + (buttons.length - 1) * Theme.GAP;
        String shown = this.font.plainSubstrByWidth(label, CONTENT_WIDTH - buttonsWidth - 6);
        lines.add(new Line(
                Component.literal(shown).getVisualOrderText(),
                cursor + (Theme.CONTROL_HEIGHT - 8) / 2,
                Theme.TEXT,
                false));
        for (int i = 0; i < buttons.length; i++) {
            Button button = Ui.button(Component.literal(buttons[i]), each, actions[i]);
            button.setX(left + CONTENT_WIDTH - buttonsWidth + i * (each + Theme.GAP));
            button.setY(cursor);
            stacked.add(button);
            addRenderableWidget(button);
        }
        cursor += Theme.CONTROL_HEIGHT + GAP - 2;
    }

    /** A control of the caller's own (a choice), across the panel. */
    protected final <T extends AbstractWidget> T widget(T widget) {
        place(widget, CONTENT_WIDTH, 0);
        return widget;
    }

    /** A setting: its name on the left, an on/off switch on the right. */
    protected final void toggle(String label, boolean value, Consumer<Boolean> set) {
        int width = 52;
        lines.add(new Line(
                Component.literal(label).getVisualOrderText(),
                cursor + (Theme.CONTROL_HEIGHT - 8) / 2,
                Theme.TEXT,
                false));
        AbstractWidget toggle = Ui.toggle(Component.literal(label), value, width, set);
        place(toggle, width, CONTENT_WIDTH - width);
    }

    /** How far down the panel's contents the stack has reached: where the next part goes. */
    protected final int stackHeight() {
        return cursor;
    }

    /** Where the panel's contents start on the screen: add {@link #stackHeight} positions to it. */
    protected final int contentTop() {
        return top - scroll + PADDING + LINE + GAP;
    }

    /** The status line: what the last request came to, or what one is waiting for; two lines at most. */
    protected final void statusLine() {
        statusY = cursor;
        cursor += 2 * LINE + GAP;
    }

    /** What Enter does. */
    protected final void onSubmit(Runnable action) {
        submit = action;
    }

    private void place(AbstractWidget widget, int width, int x) {
        widget.setWidth(width);
        widget.setX(left + x);
        widget.setY(cursor);
        stacked.add(widget);
        addRenderableWidget(widget);
        cursor += widget.getHeight() + GAP;
    }

    // ---------------------------------------------------------------------------------------------
    // Requests
    // ---------------------------------------------------------------------------------------------

    /** Whether a request is out. */
    protected final boolean busy() {
        return busy;
    }

    /** A request is going out: the status line says {@code waiting} until {@link #answered}. */
    protected final void sending(String waiting) {
        busy = true;
        status(waiting, Theme.MUTED);
    }

    /**
     * The request's answer is in. True if it worked; if it didn't, the status line says why and the
     * caller does nothing more.
     */
    protected final boolean answered(Result<?> result) {
        busy = false;
        Failure failure = result.failure();
        if (failure != null) {
            status(failure.message(), Theme.NEGATIVE);
            return false;
        }
        status("", Theme.MUTED);
        return true;
    }

    /** Says {@code text} on the status line. */
    protected final void status(String text, int color) {
        status = Component.literal(text);
        statusColor = color;
    }

    /** Says a problem with what was typed, before anything is sent. */
    protected final void problem(String text) {
        status(text, Theme.NEGATIVE);
    }

    /**
     * Shows {@code next}; the account screens chain through here. Only while this screen is the one
     * shown: an answer that arrives after the player went elsewhere doesn't pull them back.
     */
    protected final void show(Screen next) {
        if (current()) {
            this.minecraft.gui.setScreen(next);
        }
    }

    /** Whether this is the screen shown now. */
    @SuppressWarnings("ReferenceEquality") // This very screen, not an equal one.
    protected final boolean current() {
        return this.minecraft.gui.screen() == this;
    }

    // ---------------------------------------------------------------------------------------------
    // The screen
    // ---------------------------------------------------------------------------------------------

    @Override
    public boolean keyPressed(KeyEvent event) {
        if (super.keyPressed(event)) {
            return true;
        }
        Runnable action = submit;
        if (event.isConfirmation() && action != null && !busy) {
            action.run();
            return true;
        }
        return false;
    }

    @Override
    public void extractRenderState(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
        super.extractRenderState(graphics, mouseX, mouseY, a);
        Ui.title(graphics, this.font, this.title, this.width / 2, top - scroll + PADDING);
        int base = contentTop();
        for (Line line : lines) {
            int y = base + line.y();
            if (line.centred()) {
                graphics.centeredText(this.font, line.text(), this.width / 2, y, line.color());
            } else {
                graphics.text(this.font, line.text(), left, y, line.color(), false);
            }
        }
        if (statusY >= 0 && !status.getString().isEmpty()) {
            int y = base + statusY;
            for (FormattedCharSequence line : Ui.wrap(this.font, status.getString(), CONTENT_WIDTH, 2)) {
                graphics.centeredText(this.font, line, this.width / 2, y, statusColor);
                y += LINE;
            }
        }
    }

    @Override
    public void onClose() {
        this.minecraft.gui.setScreen(parent);
    }
}
