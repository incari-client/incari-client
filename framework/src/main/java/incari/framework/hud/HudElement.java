package incari.framework.hud;

import incari.framework.setting.ChoiceSetting;
import incari.framework.setting.FloatSetting;
import incari.framework.setting.Ids;
import incari.framework.setting.IntSetting;
import incari.framework.setting.Settings;

/**
 * Something a module draws on the HUD: its size, how to draw it, and where it goes. The placement
 * is settings of its own (an anchor, offsets from it, a scale), saved with the profile; the anchor
 * and scale show in the module's settings screen, and the HUD editor drags it about and resizes it
 * ({@link #place}).
 *
 * <p>Drawn only while its module is enabled and {@link #isVisible()} holds; in the HUD editor
 * while {@link #isPlaceable()} does, with a sample if there's nothing to show ({@link
 * HudCanvas#editing()}). Render thread.
 */
public abstract class HudElement {

    private final String id;
    private final String name;
    private final Settings settings = new Settings();
    private final ChoiceSetting<Anchor> anchor;
    private final IntSetting offsetX;
    private final IntSetting offsetY;
    private final FloatSetting scale;

    /** Where it was last drawn on the game's HUD (not the HUD editor's): left, top, width, height. */
    private int drawnX;

    private int drawnY;
    private int drawnWidth;
    private int drawnHeight;

    /** Whether it showed the last time the game's HUD drew it. */
    private boolean drawn;

    /**
     * @param id the config key, unique within the module
     * @param name what the settings screen and the HUD editor call it
     * @param anchor where it goes by default
     * @param offsetX how far in from the anchor by default, in GUI units
     * @param offsetY how far in from the anchor by default
     */
    protected HudElement(String id, String name, Anchor anchor, int offsetX, int offsetY) {
        this.id = Ids.require(id);
        this.name = name;
        this.anchor = settings.choice("anchor", "Position", Anchor.values(), anchor, Anchor::label);
        this.offsetX =
                settings.integer("x", "Offset across", -4096, 4096, offsetX).hidden();
        this.offsetY =
                settings.integer("y", "Offset down", -4096, 4096, offsetY).hidden();
        this.scale = settings.decimal("scale", "Scale", 0.5F, 3.0F, 0.25F, 1.0F).formattedAs(HudElement::formatScale);
    }

    /** The config key. */
    public final String id() {
        return id;
    }

    /** What the settings screen calls it. */
    public final String name() {
        return name;
    }

    /** Its placement settings, and any of its own a subclass adds. */
    public final Settings settings() {
        return settings;
    }

    /** Where it's placed from. */
    public final ChoiceSetting<Anchor> anchor() {
        return anchor;
    }

    /** How far in from the anchor across, in GUI units. */
    public final IntSetting offsetX() {
        return offsetX;
    }

    /** How far in from the anchor down. */
    public final IntSetting offsetY() {
        return offsetY;
    }

    /** How much bigger than the GUI scale it's drawn. */
    public final FloatSetting scale() {
        return scale;
    }

    /** Whether it has anything to show now; drawn only then. */
    public boolean isVisible() {
        return true;
    }

    /**
     * Whether the HUD editor shows it, its module being on: false only when a setting of its own
     * hides it for good (Toggle Sprint's status with Show status off).
     */
    public boolean isPlaceable() {
        return true;
    }

    /**
     * Whether it's drawn every frame rather than with the rest of the HUD, which the browser draws
     * only a few dozen times a second (ADR 0030). True for what follows the camera (a turning
     * minimap), which would otherwise lag. Live elements draw under the game's HUD, with the
     * crosshair; keep them cheap.
     */
    public boolean isLive() {
        return false;
    }

    /** Its width unscaled, in GUI units. */
    public abstract int width(HudCanvas canvas);

    /** Its height unscaled. */
    public abstract int height(HudCanvas canvas);

    /** Draws it with its top left at 0, 0, unscaled. Don't allocate per call where avoidable. */
    public abstract void draw(HudCanvas canvas);

    /** "1x", "1.25x", "2.5x". */
    private static String formatScale(double value) {
        long hundredths = Math.round(value * 100);
        String text = hundredths / 100 + "." + (hundredths % 100 < 10 ? "0" : "") + hundredths % 100;
        while (text.endsWith("0")) {
            text = text.substring(0, text.length() - 1);
        }
        if (text.endsWith(".")) {
            text = text.substring(0, text.length() - 1);
        }
        return text + "x";
    }

    /**
     * Where it's drawn on {@code canvas}, scaled: {@code out} gets its left, top, width and height
     * in GUI units. Allocates nothing.
     */
    public final void bounds(HudCanvas canvas, int[] out) {
        float s = scale.get();
        int w = Math.round(width(canvas) * s);
        int h = Math.round(height(canvas) * s);
        Anchor a = anchor.get();
        out[0] = a.x(offsetX.get(), w, canvas.screenWidth());
        out[1] = a.y(offsetY.get(), h, canvas.screenHeight());
        out[2] = w;
        out[3] = h;
    }

    /**
     * The HUD editor's: puts it with its top left at {@code x, y} on a screen of this size, its
     * size {@code width} by {@code height} scaled. The anchor becomes the nearest one ({@link
     * Anchor#nearest}) and the offsets are measured from it.
     */
    public final void place(int x, int y, int width, int height, int screenWidth, int screenHeight) {
        Anchor a = Anchor.nearest(x, y, width, height, screenWidth, screenHeight);
        anchor.set(a);
        offsetX.set(a.offsetX(x, width, screenWidth));
        offsetY.set(a.offsetY(y, height, screenHeight));
    }

    /**
     * Where it was last drawn on the game's HUD, into {@code out} as left, top, width and height in
     * GUI units; false if it showed nothing then. The module system's, to keep the game's own HUD
     * clear of it ({@link incari.framework.ModuleSystem#effectIconsShift}). Allocates nothing.
     */
    public final boolean lastDrawn(int[] out) {
        if (!drawn) {
            return false;
        }
        out[0] = drawnX;
        out[1] = drawnY;
        out[2] = drawnWidth;
        out[3] = drawnHeight;
        return true;
    }

    /** Back where its module put it, at its default size. */
    public final void resetPlacement() {
        anchor.reset();
        offsetX.reset();
        offsetY.reset();
        scale.reset();
    }

    /** The module system's: places, scales and draws it, if visible (placeable in the editor). */
    public final void render(HudCanvas canvas) {
        boolean editing = canvas.editing();
        if (editing ? !isPlaceable() : !isVisible()) {
            if (!editing) {
                drawn = false;
            }
            return;
        }
        float s = scale.get();
        int w = Math.round(width(canvas) * s);
        int h = Math.round(height(canvas) * s);
        Anchor a = anchor.get();
        int x = a.x(offsetX.get(), w, canvas.screenWidth());
        int y = a.y(offsetY.get(), h, canvas.screenHeight());
        if (!editing) {
            drawn = w > 0 && h > 0;
            drawnX = x;
            drawnY = y;
            drawnWidth = w;
            drawnHeight = h;
        }
        canvas.push(x, y, s);
        try {
            draw(canvas);
        } finally {
            canvas.pop();
        }
    }
}
