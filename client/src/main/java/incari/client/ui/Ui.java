package incari.client.ui;

import java.util.ArrayList;
import java.util.List;
import java.util.function.Consumer;
import java.util.function.DoubleConsumer;
import java.util.function.DoubleFunction;
import java.util.function.Function;
import java.util.function.IntConsumer;
import java.util.function.Supplier;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractSliderButton;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.CycleButton;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.client.gui.components.Tooltip;
import net.minecraft.locale.Language;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.FormattedText;
import net.minecraft.network.chat.Style;
import net.minecraft.util.ARGB;
import net.minecraft.util.FormattedCharSequence;
import org.jspecify.annotations.Nullable;

/**
 * Every control Incari's screens are built from. Screens ask here for a button, a toggle, a slider,
 * a choice or a field, and never make the game's widgets themselves. That keeps the look in one
 * place: the restyle swaps these for Incari's own widgets and every screen, generated or
 * written by hand, follows.
 *
 * <p>For now they're the game's widgets in the game's look, sized by {@link Theme}. Render thread.
 */
public final class Ui {

    private static final String ELLIPSIS = "...";

    private Ui() {}

    /** A button that runs {@code action} when pressed. */
    public static Button button(Component label, int width, Runnable action) {
        return Button.builder(label, button -> action.run())
                .size(width, Theme.CONTROL_HEIGHT)
                .build();
    }

    /**
     * A menu's button: its label on the left after an icon, an optional hint at the right in the
     * muted color, the label easing a little to the right under the pointer in Normal.
     */
    public static MenuButton menuButton(Component label, @Nullable Icons icon, int width, Runnable action) {
        return new MenuButton(label, icon, Component.empty(), width, action);
    }

    /** {@link #menuButton} with a hint at its right end ("2 saved", a key). */
    public static MenuButton menuButton(
            Component label, @Nullable Icons icon, Component hint, int width, Runnable action) {
        return new MenuButton(label, icon, hint, width, action);
    }

    /**
     * A tile: a glass card with an icon in an accent badge, a title and a line or two about what it
     * opens. Laid out by its row ({@link ControlList#across}), which sets its width.
     *
     * @param description supplied each frame, so it can say what's going on (recording, signed in)
     */
    public static Tile tile(Icons icon, Component title, Supplier<String> description, Runnable action) {
        return new Tile(icon, title, description, action);
    }

    /** An on/off switch showing only its state. */
    public static CycleButton<Boolean> toggle(Component name, boolean value, int width, Consumer<Boolean> set) {
        return CycleButton.onOffBuilder(value)
                .displayOnlyValue()
                .create(0, 0, width, Theme.CONTROL_HEIGHT, name, (button, on) -> set.accept(on));
    }

    /** A button that steps through {@code values}, showing only the current one. */
    public static <T> CycleButton<T> choice(
            Component name, List<T> values, T value, Function<T, Component> label, int width, Consumer<T> set) {
        return CycleButton.builder(label, value)
                .withValues(values)
                .displayOnlyValue()
                .create(0, 0, width, Theme.CONTROL_HEIGHT, name, (button, chosen) -> set.accept(chosen));
    }

    /**
     * A slider from {@code min} to {@code max}.
     *
     * @param label what it says for a value
     * @param set called with each value it's dragged to
     */
    public static Slider slider(
            double min, double max, double value, int width, DoubleFunction<Component> label, DoubleConsumer set) {
        return new Slider(min, max, value, width, label, set);
    }

    /**
     * A field for a color as {@code #RRGGBB} (or {@code #AARRGGBB} with alpha), showing a swatch of
     * it. Each valid color typed is set at once.
     */
    public static ColorField colorField(
            Font font, Component name, int argb, boolean alpha, int width, IntConsumer set) {
        return new ColorField(font, name, argb, alpha, width, set);
    }

    /** A plain text field. */
    public static EditBox textField(Font font, Component name, int width, int maxLength) {
        EditBox box = new EditBox(font, 0, 0, width, Theme.CONTROL_HEIGHT, name);
        box.setMaxLength(maxLength);
        return box;
    }

    /** A tooltip, or null for none if {@code text} is empty. */
    public static @Nullable Tooltip tooltip(String text) {
        return text.isEmpty() ? null : Tooltip.create(Component.literal(text));
    }

    /**
     * {@code text} cut to {@code width} with "..." at the end if it's wider; whole if it fits;
     * nothing where there's no room even for the "...".
     */
    public static String fit(Font font, String text, int width) {
        if (font.width(text) <= width) {
            return text;
        }
        if (width < font.width(ELLIPSIS)) {
            return "";
        }
        return cut(font, text, width);
    }

    /**
     * {@code text} cut to {@code width} with "..." at the end if it's wider, keeping its style;
     * whole if it fits. Draw-time: allocates only when it cuts.
     */
    public static FormattedCharSequence fit(Font font, Component text, int width) {
        if (font.width(text) <= width) {
            return text.getVisualOrderText();
        }
        if (width < font.width(ELLIPSIS)) {
            return FormattedCharSequence.EMPTY;
        }
        FormattedText kept = font.substrByWidth(text, Math.max(0, width - font.width(ELLIPSIS)));
        return Language.getInstance()
                .getVisualOrder(FormattedText.composite(kept, FormattedText.of(ELLIPSIS, text.getStyle())));
    }

    private static String cut(Font font, String text, int width) {
        int room = Math.max(0, width - font.width(ELLIPSIS));
        return font.plainSubstrByWidth(text, room).stripTrailing() + ELLIPSIS;
    }

    /**
     * {@code text} wrapped to {@code width}, at most {@code maxLines} lines; the last ends in "..."
     * when some was left out, so a cut sentence never looks whole.
     */
    public static List<FormattedCharSequence> wrap(Font font, String text, int width, int maxLines) {
        List<FormattedText> lines = font.getSplitter().splitLines(text, Math.max(1, width), Style.EMPTY);
        List<FormattedCharSequence> shown = new ArrayList<>(Math.min(lines.size(), maxLines));
        for (int i = 0; i < lines.size() && i < maxLines; i++) {
            String line = lines.get(i).getString();
            if (i == maxLines - 1 && lines.size() > maxLines) {
                line = cut(font, line, width);
            }
            shown.add(Component.literal(line).getVisualOrderText());
        }
        return shown;
    }

    /** A screen's title, centered on {@code centerX}. */
    public static void title(GuiGraphicsExtractor graphics, Font font, Component title, int centerX, int y) {
        graphics.centeredText(font, title, centerX, y, Theme.TEXT);
    }

    /** A line of secondary text, centered on {@code centerX}. */
    public static void caption(GuiGraphicsExtractor graphics, Font font, Component text, int centerX, int y) {
        graphics.centeredText(font, text, centerX, y, Theme.MUTED);
    }

    /** A menu's button ({@link #menuButton}). */
    public static final class MenuButton extends Button {

        private static final int ICON = 9;

        private final @Nullable Icons icon;
        private Component hint;
        private boolean centered;

        MenuButton(Component label, @Nullable Icons icon, Component hint, int width, Runnable action) {
            super(0, 0, width, Theme.CONTROL_HEIGHT, label, button -> action.run(), DEFAULT_NARRATION);
            this.icon = icon;
            this.hint = hint;
        }

        /** The hint at its right end; empty for none. */
        public void setHint(Component hint) {
            this.hint = hint;
        }

        /** Its icon and label centered as one, as the game's buttons beside it are (the pause menu). */
        public MenuButton centered() {
            centered = true;
            return this;
        }

        @Override
        protected void extractContents(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
            extractDefaultSprite(graphics);
            Font font = Minecraft.getInstance().font;
            int color = ARGB.multiplyAlpha(active ? Theme.TEXT : Theme.MUTED, alpha);
            float t = Motion.ease(
                    Motion.key(getX(), getY(), getWidth(), getHeight()), active && isHoveredOrFocused() ? 1 : 0);
            int x = getX() + 8 + Math.round(t * 3);
            int right = getRight() - 8;
            int iconRoom = icon != null ? ICON + 6 : 0;
            // The hint goes first when both don't fit: a narrow screen keeps the label readable.
            int hintWidth = hint.getString().isEmpty() ? 0 : font.width(hint);
            boolean showHint = hintWidth > 0 && x + iconRoom + font.width(getMessage()) + 6 + hintWidth <= right;
            int room = right - x - iconRoom;
            if (centered) {
                int content = Math.min(font.width(getMessage()), getWidth() - 16 - iconRoom) + iconRoom;
                x = getX() + (getWidth() - content) / 2;
                room = content - iconRoom;
            }
            int textY = getY() + (getHeight() - 7) / 2;
            if (icon != null) {
                icon.draw(graphics, x, getY() + (getHeight() - ICON) / 2, ICON, color);
                x += ICON + 6;
            }
            graphics.text(font, fit(font, getMessage(), room), x, textY, color, false);
            if (showHint) {
                graphics.text(font, hint, right - hintWidth, textY, ARGB.multiplyAlpha(Theme.MUTED, alpha), false);
            }
        }
    }

    /** A menu's tile ({@link #tile}). */
    public static final class Tile extends Button {

        /** Tall enough for the title and two lines of description. */
        public static final int HEIGHT = 44;

        private static final int BADGE = 22;
        private static final int ICON = 12;

        private final Icons icon;
        private final Supplier<String> description;
        private String wrappedText = "";
        private int wrappedWidth = -1;
        private int wrappedFont = -1;
        private List<FormattedCharSequence> lines = List.of();

        Tile(Icons icon, Component title, Supplier<String> description, Runnable action) {
            super(0, 0, 0, HEIGHT, title, button -> action.run(), DEFAULT_NARRATION);
            this.icon = icon;
            this.description = description;
        }

        @Override
        protected void extractContents(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
            Font font = Minecraft.getInstance().font;
            boolean hot = active && isHoveredOrFocused();
            Panels.card(graphics, getX(), getY(), getWidth(), getHeight(), hot, false, alpha);
            float t = Motion.ease(Motion.key(getX(), getY(), -getWidth(), getHeight()), hot ? 1 : 0);
            int badgeX = getX() + 8;
            int badgeY = getY() + (getHeight() - BADGE) / 2;
            // The badge brightens toward the accent under the pointer.
            int tint = active ? Theme.ACCENT : Theme.MUTED;
            int badge = Panels.mix(tint & 0x33FFFFFF, tint & 0x66FFFFFF, t);
            RoundRects.fill(graphics, badgeX, badgeY, BADGE, BADGE, Theme.RADIUS + 2, Panels.alpha(badge, alpha));
            icon.draw(
                    graphics,
                    badgeX + (BADGE - ICON) / 2,
                    badgeY + (BADGE - ICON) / 2,
                    ICON,
                    ARGB.multiplyAlpha(tint, alpha));
            int textX = badgeX + BADGE + 8;
            int textWidth = getRight() - 8 - textX;
            wrap(font, textWidth);
            int textY = getY() + (getHeight() - 9 - lines.size() * 10) / 2;
            int title = active ? Theme.TEXT : Theme.MUTED;
            graphics.text(
                    font, fit(font, getMessage(), textWidth), textX, textY, ARGB.multiplyAlpha(title, alpha), false);
            int muted = ARGB.multiplyAlpha(Theme.MUTED, alpha);
            for (FormattedCharSequence line : lines) {
                textY += 10;
                graphics.text(font, line, textX, textY, muted, false);
            }
        }

        /** Wraps the description to two lines at most, when it, the width or the font changed. */
        private void wrap(Font font, int width) {
            String text = description.get();
            if (width == wrappedWidth && text.equals(wrappedText) && SmoothFont.generation() == wrappedFont) {
                return;
            }
            wrappedWidth = width;
            wrappedText = text;
            wrappedFont = SmoothFont.generation();
            lines = Ui.wrap(font, text, width, 2);
        }
    }

    /** A slider whose value can be set from outside (a profile applied while it's shown). */
    public static final class Slider extends AbstractSliderButton {

        private final double min;
        private final double max;
        private final DoubleFunction<Component> label;
        private final DoubleConsumer set;

        Slider(double min, double max, double value, int width, DoubleFunction<Component> label, DoubleConsumer set) {
            super(0, 0, width, Theme.CONTROL_HEIGHT, Component.empty(), fraction(min, max, value));
            this.min = min;
            this.max = max;
            this.label = label;
            this.set = set;
            setMessage(label.apply(value));
        }

        /** Moves the handle to {@code value} without calling back. */
        public void show(double value) {
            this.value = fraction(min, max, value);
            setMessage(label.apply(value));
        }

        @Override
        protected void updateMessage() {
            setMessage(label.apply(actual()));
        }

        @Override
        protected void applyValue() {
            set.accept(actual());
        }

        private double actual() {
            return min + value * (max - min);
        }

        private static double fraction(double min, double max, double value) {
            return max <= min ? 0 : Math.max(0, Math.min(1, (value - min) / (max - min)));
        }
    }

    /** A hex color field with a swatch at its right end. */
    public static final class ColorField extends EditBox {

        private static final int SWATCH = 12;

        private final boolean withAlpha;
        private int shown;

        ColorField(Font font, Component name, int argb, boolean alpha, int width, IntConsumer set) {
            super(font, 0, 0, width, Theme.CONTROL_HEIGHT, name);
            this.withAlpha = alpha;
            this.shown = argb;
            setMaxLength(alpha ? 9 : 7);
            setValue(text(argb));
            setResponder(text -> {
                Integer parsed = parse(text);
                if (parsed != null) {
                    shown = parsed;
                    set.accept(parsed);
                    setTextColor(Theme.TEXT);
                } else {
                    setTextColor(Theme.NEGATIVE);
                }
            });
        }

        /** Shows {@code argb} unless the player is typing in the field. */
        public void show(int argb) {
            if (!isFocused() && argb != shown) {
                shown = argb;
                setValue(text(argb));
            }
        }

        private String text(int argb) {
            String hex = Integer.toHexString(withAlpha ? argb : argb & 0xFFFFFF).toUpperCase(java.util.Locale.ROOT);
            String padding = withAlpha ? "00000000" : "000000";
            return "#" + padding.substring(hex.length()) + hex;
        }

        private @Nullable Integer parse(String text) {
            int digits = withAlpha ? 8 : 6;
            if (text.length() != digits + 1 || text.charAt(0) != '#') {
                return null;
            }
            try {
                long value = Long.parseLong(text.substring(1), 16);
                return withAlpha ? (int) value : (int) value | 0xFF000000;
            } catch (NumberFormatException e) {
                return null;
            }
        }

        @Override
        public void extractWidgetRenderState(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
            super.extractWidgetRenderState(graphics, mouseX, mouseY, a);
            int x = getX() + getWidth() - SWATCH - 4;
            int y = getY() + (getHeight() - SWATCH) / 2;
            graphics.fill(x - 1, y - 1, x + SWATCH + 1, y + SWATCH + 1, Theme.MUTED);
            graphics.fill(x, y, x + SWATCH, y + SWATCH, shown | (withAlpha ? 0 : 0xFF000000));
        }
    }
}
