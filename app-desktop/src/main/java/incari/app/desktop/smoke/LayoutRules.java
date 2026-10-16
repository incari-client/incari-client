package incari.app.desktop.smoke;

import java.util.ArrayList;
import java.util.List;
import org.jspecify.annotations.Nullable;

/**
 * What a screen drawn at one size must not do, from where its widgets and text landed (GUI
 * units, already cut to any clip they were drawn in): a widget off the screen or over another, text
 * past the widget it's in, over a widget it isn't in, over other text, or off the screen. The
 * geometry only; {@link LayoutCheck} gathers the boxes from a frame.
 */
final class LayoutRules {

    /**
     * Text's measured box takes in its shadow and anti-aliased edge, which may reach this far
     * past where a layout put it without anyone seeing a problem.
     */
    static final int SLACK = 1;

    private LayoutRules() {}

    /** A widget or a run of text, as drawn: {@code x1} and {@code y1} are exclusive. */
    record Box(String what, int x0, int y0, int x1, int y1) {

        boolean isEmpty() {
            return x1 <= x0 || y1 <= y0;
        }

        /** Whether {@code other} is inside this, give or take {@code slack}. */
        boolean holds(Box other, int slack) {
            return other.x0 >= x0 - slack && other.y0 >= y0 - slack && other.x1 <= x1 + slack && other.y1 <= y1 + slack;
        }

        /** Whether this and {@code other} share more than {@code slack} each way. */
        boolean overlaps(Box other, int slack) {
            return Math.min(x1, other.x1) - Math.max(x0, other.x0) > slack
                    && Math.min(y1, other.y1) - Math.max(y0, other.y0) > slack;
        }

        boolean containsPoint(int x, int y) {
            return x >= x0 && x < x1 && y >= y0 && y < y1;
        }

        @Override
        public String toString() {
            return what + " at " + x0 + "," + y0 + " " + (x1 - x0) + "x" + (y1 - y0);
        }
    }

    /** Everything wrong with one screen at {@code width} by {@code height}, in words; empty if fine. */
    static List<String> problems(int width, int height, List<Box> widgets, List<Box> texts) {
        return problems(width, height, false, widgets, texts);
    }

    /**
     * {@link #problems(int, int, List, List)} for a screen that may scroll its contents up
     * ({@code scrolls}): what's below its bottom edge is reached by scrolling, not cut off.
     */
    static List<String> problems(int width, int height, boolean scrolls, List<Box> widgets, List<Box> texts) {
        Box screen = new Box("the screen", 0, 0, width, scrolls ? Integer.MAX_VALUE / 2 : height);
        List<String> found = new ArrayList<>();
        List<Box> shown = widgets.stream().filter(box -> !box.isEmpty()).toList();
        List<Box> words = texts.stream().filter(box -> !box.isEmpty()).toList();
        for (int i = 0; i < shown.size(); i++) {
            Box widget = shown.get(i);
            if (!screen.holds(widget, 0)) {
                found.add(widget + " runs off the screen");
            }
            for (int j = i + 1; j < shown.size(); j++) {
                if (widget.overlaps(shown.get(j), 0)) {
                    found.add(widget + " overlaps " + shown.get(j));
                }
            }
        }
        for (int i = 0; i < words.size(); i++) {
            Box text = words.get(i);
            if (!screen.holds(text, SLACK)) {
                found.add(text + " runs off the screen");
            }
            Box owner = owner(text, shown);
            if (owner != null) {
                if (!owner.holds(text, SLACK)) {
                    found.add(text + " runs past " + owner);
                }
            } else {
                for (Box widget : shown) {
                    if (text.overlaps(widget, SLACK)) {
                        found.add(text + " runs into " + widget);
                    }
                }
            }
            for (int j = i + 1; j < words.size(); j++) {
                if (text.overlaps(words.get(j), SLACK)) {
                    found.add(text + " overlaps " + words.get(j));
                }
            }
        }
        return found;
    }

    /** The widget text starts in: where its first letter's middle is. */
    private static @Nullable Box owner(Box text, List<Box> widgets) {
        int x = text.x0 + SLACK;
        int y = (text.y0 + text.y1) / 2;
        for (Box widget : widgets) {
            if (widget.containsPoint(x, y)) {
                return widget;
            }
        }
        return null;
    }
}
