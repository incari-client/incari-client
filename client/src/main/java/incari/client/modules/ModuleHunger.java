package incari.client.modules;

import java.util.Arrays;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.renderer.texture.SpriteContents;
import net.minecraft.data.AtlasIds;
import net.minecraft.resources.Identifier;
import net.minecraft.util.ARGB;
import net.minecraft.world.entity.player.Player;

/**
 * Saturation on the hunger bar ({@code Game.overrideSaturation}): each shank's worth of saturation
 * outlines that shank in a bright color, as AppleSkin does. The outline is traced from the shank
 * sprite's own pixels (its opaque edge), so it fits whatever resource pack draws the shanks.
 * Saturation never tops hunger, so an outlined shank is always a filled one, and a half's outline
 * is the half sprite's. Render thread.
 */
public final class ModuleHunger {

    /** The color a module holds, or -1 for the game's own bar. */
    private static int saturation = -1;

    /**
     * Traced sprites: the full and half shanks, plain and with Hunger. Keyed by the sprite's
     * contents, which a resource reload replaces, so a reload traces afresh.
     */
    private static final Outline[] OUTLINES = new Outline[4];

    private ModuleHunger() {}

    /** {@code Game.overrideSaturation}'s. */
    static void saturation(int rgb) {
        saturation = rgb;
    }

    /**
     * Hook ({@code Hud.extractFood}, after each shank): the {@code index}th shank from the right,
     * outlined if it's saturated. Hot path, don't allocate (but on a sprite's first trace).
     */
    public static void shank(
            GuiGraphicsExtractor graphics, Player player, int index, int x, int y, Identifier half, Identifier full) {
        int rgb = saturation;
        if (rgb < 0) {
            return;
        }
        float level = player.getFoodData().getSaturationLevel();
        if (level <= 0) {
            return;
        }
        // Half-shank points, as hunger counts them; any saturation at all shows.
        int points = Math.max(1, Math.round(level));
        int first = index * 2 + 1;
        Identifier sprite;
        if (first < points) {
            sprite = full;
        } else if (first == points) {
            sprite = half;
        } else {
            return;
        }
        outline(sprite).draw(graphics, x, y, ARGB.opaque(rgb));
    }

    @SuppressWarnings("ReferenceEquality") // the same sprite contents, not equal ones: a reload makes new
    private static Outline outline(Identifier sprite) {
        SpriteContents contents = Minecraft.getInstance()
                .getAtlasManager()
                .getAtlasOrThrow(AtlasIds.GUI)
                .getSprite(sprite)
                .contents();
        int slot = -1;
        for (int i = 0; i < OUTLINES.length; i++) {
            Outline outline = OUTLINES[i];
            if (outline != null && outline.contents == contents) {
                return outline;
            }
            // A stale trace of the same sprite (reloaded), or an empty slot.
            if (outline == null ? slot < 0 : outline.name.equals(sprite)) {
                slot = i;
            }
        }
        Outline traced = Outline.trace(contents, sprite);
        OUTLINES[slot < 0 ? 0 : slot] = traced;
        return traced;
    }

    /** A sprite's opaque edge as runs of pixels along its rows. */
    private static final class Outline {

        final SpriteContents contents;
        final Identifier name;
        final int width;
        final int height;

        /** Runs as {x, y, length} triples. */
        final int[] runs;

        private Outline(SpriteContents contents, Identifier name, int[] runs) {
            this.contents = contents;
            this.name = name;
            this.width = contents.width();
            this.height = contents.height();
            this.runs = runs;
        }

        /** Traces the first frame: opaque pixels with a clear (or no) neighbor up, down or across. */
        static Outline trace(SpriteContents contents, Identifier name) {
            int w = contents.width();
            int h = contents.height();
            int[] runs = new int[w * h * 3];
            int count = 0;
            for (int y = 0; y < h; y++) {
                int start = -1;
                for (int x = 0; x <= w; x++) {
                    boolean edge = x < w && edge(contents, w, h, x, y);
                    if (edge && start < 0) {
                        start = x;
                    } else if (!edge && start >= 0) {
                        runs[count++] = start;
                        runs[count++] = y;
                        runs[count++] = x - start;
                        start = -1;
                    }
                }
            }
            return new Outline(contents, name, Arrays.copyOf(runs, count));
        }

        private static boolean edge(SpriteContents contents, int w, int h, int x, int y) {
            return !clear(contents, w, h, x, y)
                    && (clear(contents, w, h, x - 1, y)
                            || clear(contents, w, h, x + 1, y)
                            || clear(contents, w, h, x, y - 1)
                            || clear(contents, w, h, x, y + 1));
        }

        private static boolean clear(SpriteContents contents, int w, int h, int x, int y) {
            return x < 0 || y < 0 || x >= w || y >= h || contents.isTransparent(0, x, y);
        }

        /** Draws the edge over a 9 by 9 shank at {@code x, y}, whatever the sprite's own size. */
        void draw(GuiGraphicsExtractor graphics, int x, int y, int argb) {
            graphics.pose().pushMatrix().translate(x, y).scale(9F / width, 9F / height);
            for (int i = 0; i < runs.length; i += 3) {
                int rx = runs[i];
                int ry = runs[i + 1];
                graphics.fill(rx, ry, rx + runs[i + 2], ry + 1, argb);
            }
            graphics.pose().popMatrix();
        }
    }
}
