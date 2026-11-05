package incari.features.fpsdisplay;

import incari.framework.Category;
import incari.framework.Module;
import incari.framework.hud.Anchor;
import incari.framework.hud.HudCanvas;
import incari.framework.hud.TextElement;
import incari.framework.hud.Tone;
import incari.framework.setting.BooleanSetting;
import incari.framework.setting.ChoiceSetting;
import org.jspecify.annotations.Nullable;

/** FPS Display: the frames drawn in the last second, as the debug screen counts them. */
public final class FpsDisplay extends Module {

    /** How the count is written. */
    public enum Format {
        /** "120 FPS". */
        AFTER("120 FPS"),
        /** "FPS: 120". */
        BEFORE("FPS: 120"),
        /** "120". */
        NUMBER("120");

        private final String label;

        Format(String label) {
            this.label = label;
        }

        /** What the settings screen calls it. */
        public String label() {
            return label;
        }
    }

    final ChoiceSetting<Format> format = settings()
            .choice("format", "Format", Format.values(), Format.AFTER, Format::label)
            .describedAs("How the FPS is shown.");
    final BooleanSetting speedColors = settings()
            .bool("speed-colors", "Color by speed", false)
            .describedAs("Green from 60 frames a second, amber from 30, red below.");

    /** FPS Display, off until the player turns it on. */
    public FpsDisplay() {
        super("fpsdisplay", "FPS Display", "Shows how many frames a second the game draws.", Category.HUD);
        hud(new Counter());
    }

    /** "120 FPS" in {@code format}. */
    static String text(Format format, int fps) {
        return switch (format) {
            case AFTER -> fps + " FPS";
            case BEFORE -> "FPS: " + fps;
            case NUMBER -> Integer.toString(fps);
        };
    }

    /** The count, on the glass. */
    private final class Counter extends TextElement {

        private int cachedFps = -1;
        private @Nullable Format cachedFormat;
        private String cached = "";

        Counter() {
            super("fps", "FPS", Anchor.TOP_LEFT, 2, 14);
        }

        @Override
        protected @Nullable String text(HudCanvas canvas) {
            int fps = system().game().fps();
            Format chosen = format.get();
            if (fps != cachedFps || chosen != cachedFormat) {
                cachedFps = fps;
                cachedFormat = chosen;
                cached = FpsDisplay.text(chosen, fps);
            }
            return cached;
        }

        @Override
        protected @Nullable String widest() {
            return switch (format.get()) {
                case AFTER -> "000 FPS";
                case BEFORE -> "FPS: 000";
                case NUMBER -> "000";
            };
        }

        @Override
        protected Tone tone() {
            if (!speedColors.get()) {
                return Tone.TEXT;
            }
            int fps = system().game().fps();
            return fps >= 60 ? Tone.POSITIVE : fps >= 30 ? Tone.WARNING : Tone.NEGATIVE;
        }
    }
}
