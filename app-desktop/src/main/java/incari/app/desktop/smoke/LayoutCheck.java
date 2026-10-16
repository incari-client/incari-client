package incari.app.desktop.smoke;

import incari.app.desktop.smoke.LayoutRules.Box;
import incari.client.ui.GuiSkin;
import incari.client.ui.Skins;
import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.components.events.ContainerEventHandler;
import net.minecraft.client.gui.navigation.ScreenRectangle;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.renderer.state.gui.GuiRenderState;
import net.minecraft.client.renderer.state.gui.GuiTextRenderState;
import net.minecraft.resources.Identifier;
import net.minecraft.util.FormattedCharSequence;
import org.jspecify.annotations.Nullable;

/**
 * A screen's layout as drawn: one frame of it extracted off the window, with the pointer away,
 * recording where each widget and each run of text landed, for {@link LayoutRules} to judge.
 * Widgets are seen through a {@link GuiSkin} that records and passes everything on to the skin in
 * use; text through the frame's text states, measured as layouts measure it. Game thread.
 */
final class LayoutCheck {

    private LayoutCheck() {}

    /** What's wrong with the screen shown now, in words; empty if its layout is fine. */
    static List<String> problems(Minecraft minecraft, Screen screen) {
        GuiRenderState state = new GuiRenderState();
        GuiGraphicsExtractor graphics = new GuiGraphicsExtractor(minecraft, state, -1, -1);
        Recorder recorder = new Recorder(Skins.current());
        Skins.use(recorder);
        try {
            screen.extractRenderStateWithTooltipAndSubtitles(graphics, -1, -1, 0);
        } finally {
            Skins.use(recorder.inner);
        }
        List<Box> texts = new ArrayList<>();
        state.forEachText(text -> {
            Box box = laidOut(text);
            if (box != null) {
                texts.add(box);
            }
        });
        return LayoutRules.problems(
                graphics.guiWidth(), graphics.guiHeight(), scrolls(screen), recorder.widgets, texts);
    }

    /** The account screens' panel scrolls when it's taller than the screen ({@code FormScreen}). */
    private static boolean scrolls(Screen screen) {
        for (Class<?> type = screen.getClass(); type != null; type = type.getSuperclass()) {
            if (type.getName().equals("incari.client.online.FormScreen")) {
                return true;
            }
        }
        return false;
    }

    private static Box box(String what, ScreenRectangle rectangle) {
        return new Box(what, rectangle.left(), rectangle.top(), rectangle.right(), rectangle.bottom());
    }

    private static String describe(AbstractWidget widget) {
        Class<?> type = widget.getClass();
        while (type.getSimpleName().isEmpty()) {
            type = type.getSuperclass();
        }
        return type.getSimpleName() + " \"" + shortened(widget.getMessage().getString()) + "\"";
    }

    private static String shortened(String text) {
        return text.length() <= 40 ? text : text.substring(0, 37) + "...";
    }

    /**
     * Where a run of text was laid out: its font's width by its line height, as the screen measured
     * it, cut to its clip; null if the clip hides it. Not the glyphs' own box, which takes in the
     * shadow and, in the pixel font, some padding, so it reaches past where any layout put it.
     */
    private static @Nullable Box laidOut(GuiTextRenderState state) {
        Font font = field(state, "font", Font.class);
        FormattedCharSequence text = field(state, "text", FormattedCharSequence.class);
        int x = field(state, "x", Integer.class);
        int y = field(state, "y", Integer.class);
        ScreenRectangle laid =
                new ScreenRectangle(x, y, font.width(text), font.lineHeight).transformMaxBounds(state.pose);
        ScreenRectangle shown = state.scissor != null ? state.scissor.intersection(laid) : laid;
        if (shown == null) {
            return null;
        }
        StringBuilder words = new StringBuilder();
        text.accept((position, style, codepoint) -> {
            words.appendCodePoint(codepoint);
            return true;
        });
        return box("text \"" + shortened(words.toString()) + "\"", shown);
    }

    /** A text state keeps what it draws to itself. */
    private static <T> T field(GuiTextRenderState state, String name, Class<T> type) {
        try {
            Field field = GuiTextRenderState.class.getDeclaredField(name);
            field.setAccessible(true);
            return type.cast(field.get(state));
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException("GuiTextRenderState has no " + name, e);
        }
    }

    /** Records each widget drawn, cut to its clip, and hands every call on to {@code inner}. */
    private static final class Recorder implements GuiSkin {

        final @Nullable GuiSkin inner;
        final List<Box> widgets = new ArrayList<>();

        Recorder(@Nullable GuiSkin inner) {
            this.inner = inner;
        }

        @Override
        public boolean widget(GuiGraphicsExtractor graphics, AbstractWidget widget, int mouseX, int mouseY, float a) {
            // A list or a panel is judged by what's in it, not as one widget over its rows.
            if (!(widget instanceof ContainerEventHandler)) {
                ScreenRectangle drawn = widget.getRectangle();
                ScreenRectangle clip = graphics.incariScissor();
                ScreenRectangle shown = clip != null ? clip.intersection(drawn) : drawn;
                if (shown != null) {
                    widgets.add(box(describe(widget), shown));
                }
            }
            return inner != null && inner.widget(graphics, widget, mouseX, mouseY, a);
        }

        @Override
        public boolean sprite(
                GuiGraphicsExtractor graphics, Identifier sprite, int x, int y, int width, int height, int color) {
            return inner != null && inner.sprite(graphics, sprite, x, y, width, height, color);
        }

        @Override
        public boolean spritePart(
                GuiGraphicsExtractor graphics,
                Identifier sprite,
                int spriteWidth,
                int spriteHeight,
                int textureX,
                int textureY,
                int x,
                int y,
                int width,
                int height,
                int color) {
            return inner != null
                    && inner.spritePart(
                            graphics,
                            sprite,
                            spriteWidth,
                            spriteHeight,
                            textureX,
                            textureY,
                            x,
                            y,
                            width,
                            height,
                            color);
        }

        @Override
        public boolean texture(
                GuiGraphicsExtractor graphics,
                Identifier texture,
                int x,
                int y,
                int width,
                int height,
                float u,
                float v,
                int srcWidth,
                int srcHeight,
                int textureWidth,
                int textureHeight,
                int color) {
            return inner != null
                    && inner.texture(
                            graphics,
                            texture,
                            x,
                            y,
                            width,
                            height,
                            u,
                            v,
                            srcWidth,
                            srcHeight,
                            textureWidth,
                            textureHeight,
                            color);
        }

        @Override
        public boolean background(GuiGraphicsExtractor graphics, Screen screen, float a) {
            return inner != null && inner.background(graphics, screen, a);
        }
    }
}
