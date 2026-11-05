package incari.features.cps;

import incari.framework.Category;
import incari.framework.Module;
import incari.framework.event.ClickCounter;
import incari.framework.event.ClickListener;
import incari.framework.event.Events;
import incari.framework.hud.Anchor;
import incari.framework.hud.HudCanvas;
import incari.framework.hud.TextElement;
import incari.framework.setting.ChoiceSetting;
import org.jspecify.annotations.Nullable;

/** CPS: how many times the player clicked in the last second, left, right or both. */
public final class Cps extends Module {

    /** Which buttons are counted. */
    public enum Buttons {
        /** "12 | 4 CPS". */
        BOTH("Left and right"),
        /** "12 CPS". */
        LEFT("Left"),
        /** "4 CPS". */
        RIGHT("Right");

        private final String label;

        Buttons(String label) {
            this.label = label;
        }

        /** What the settings screen calls it. */
        public String label() {
            return label;
        }
    }

    final ChoiceSetting<Buttons> buttons = settings()
            .choice("buttons", "Buttons", Buttons.values(), Buttons.BOTH, Buttons::label)
            .describedAs("Which mouse buttons' clicks are counted.");

    private final ClickCounter left = new ClickCounter();
    private final ClickCounter right = new ClickCounter();

    /** CPS, off until the player turns it on. */
    public Cps() {
        super("cps", "CPS", "Shows how many times a second you click.", Category.HUD);
        on(Events.CLICK, this::onClick);
        hud(new Counter());
    }

    private void onClick(int button) {
        long now = system().game().millis();
        if (button == ClickListener.LEFT) {
            left.click(now);
        } else if (button == ClickListener.RIGHT) {
            right.click(now);
        }
    }

    @Override
    protected void onDisable() {
        left.clear();
        right.clear();
    }

    /** "12 | 4 CPS" for {@code buttons}. */
    static String text(Buttons buttons, int left, int right) {
        return switch (buttons) {
            case BOTH -> left + " | " + right + " CPS";
            case LEFT -> left + " CPS";
            case RIGHT -> right + " CPS";
        };
    }

    /** The count, on the glass. */
    private final class Counter extends TextElement {

        private int cachedLeft = -1;
        private int cachedRight = -1;
        private @Nullable Buttons cachedButtons;
        private String cached = "";

        Counter() {
            super("cps", "CPS", Anchor.TOP_LEFT, 2, 30);
        }

        @Override
        protected @Nullable String text(HudCanvas canvas) {
            long now = system().game().millis();
            int l = left.perSecond(now);
            int r = right.perSecond(now);
            Buttons chosen = buttons.get();
            if (l != cachedLeft || r != cachedRight || chosen != cachedButtons) {
                cachedLeft = l;
                cachedRight = r;
                cachedButtons = chosen;
                cached = Cps.text(chosen, l, r);
            }
            return cached;
        }

        @Override
        protected @Nullable String widest() {
            return buttons.get() == Buttons.BOTH ? "00 | 00 CPS" : "00 CPS";
        }
    }
}
