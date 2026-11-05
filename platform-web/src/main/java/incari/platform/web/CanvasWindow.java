package incari.platform.web;

import incari.platform.api.window.DisplayMode;
import incari.platform.api.window.IconImage;
import incari.platform.api.window.PlatformWindow;
import incari.platform.api.window.SystemBehavior;
import incari.platform.api.window.SystemCursor;
import java.util.List;
import org.jspecify.annotations.Nullable;
import org.teavm.jso.JSObject;

/**
 * The page's canvas as the game's window. Screen coordinates are CSS pixels, with the canvas's
 * top left as the origin; the framebuffer is the canvas's backing store, CSS size times the device
 * pixel ratio. The size, cursor and Pointer Lock state live in the {@link DomEventQueue}, which
 * sees the page's events; this class answers from it and forwards the game's requests to the page.
 */
final class CanvasWindow implements PlatformWindow, DomEventQueue.Surface {

    /** A constant, non-zero handle: the game only compares handles to tell windows apart. */
    static final long HANDLE = 1L;

    private final JSObject canvas;
    private @Nullable DomEventQueue events;
    /** The game holds the mouse ({@link #setRelativeMouseMode}). */
    private boolean mouseHeld;
    /** The player is in a world ({@link #setLeaveWarning}). */
    private boolean inWorld;
    /** Whether the page's leave guard is on now, so it's set only when it changes. */
    private boolean guarded;

    CanvasWindow(JSObject canvas) {
        this.canvas = canvas;
    }

    /** Connects the queue; done once, right after both exist (each needs the other). */
    void connect(DomEventQueue queue) {
        this.events = queue;
    }

    JSObject canvas() {
        return canvas;
    }

    private DomEventQueue events() {
        DomEventQueue queue = events;
        if (queue == null) {
            throw new IllegalStateException("The canvas window isn't connected to its events yet");
        }
        return queue;
    }

    // --- What the queue asks of the page ---

    @Override
    public void setBackingSize(int pixelWidth, int pixelHeight) {
        Dom.setCanvasSize(canvas, pixelWidth, pixelHeight);
    }

    @Override
    public void setPointerLock(boolean locked) {
        Dom.setPointerLock(canvas, locked);
    }

    @Override
    public long nativeHandle() {
        return HANDLE;
    }

    @Override
    public int x() {
        return 0;
    }

    @Override
    public int y() {
        return 0;
    }

    @Override
    public int width() {
        return events().size().width();
    }

    @Override
    public int height() {
        return events().size().height();
    }

    @Override
    public int framebufferWidth() {
        return events().size().pixelWidth();
    }

    @Override
    public int framebufferHeight() {
        return events().size().pixelHeight();
    }

    @Override
    public int unusedWidth() {
        return 0;
    }

    @Override
    public float pixelDensity() {
        return (float) events().size().density();
    }

    @Override
    public int displayId() {
        return WebDisplays.SCREEN;
    }

    /** The tab keeps the page's own title, "Incari Client", whatever the game names its window. */
    @Override
    public void setTitle(String title) {}

    @Override
    public void setIcon(List<IconImage> sizes) {
        // The page's favicon belongs to the host page, not the game.
    }

    @Override
    public void setMaximumSize(int maxWidth, int maxHeight) {
        // The page decides the canvas size.
    }

    @Override
    public boolean setWindowed(int newX, int newY, int newWidth, int newHeight) {
        if (isFullscreen()) {
            events().fullscreenRequested(false);
            Dom.requestFullscreen(canvas, false);
        }
        // The page decides the canvas's size, not the game: report the size it has, which the
        // game then takes as its window size.
        events().forceSizeReport();
        return true;
    }

    @Override
    public boolean setFullscreen(@Nullable DisplayMode exclusiveMode) {
        if (isFullscreen()) {
            return true;
        }
        if (!Dom.canEnterFullscreen()) {
            // Refusing now, rather than letting the page refuse later, lets the game turn its
            // fullscreen option back off instead of asking again every frame.
            return false;
        }
        // A page can't change the display mode; fullscreen is always "borderless".
        events().fullscreenRequested(true);
        Dom.requestFullscreen(canvas, true);
        return true;
    }

    /**
     * Whether the canvas is fullscreen, or about to be: the page answers a request only later,
     * and the game checks right after asking.
     */
    @Override
    public boolean isFullscreen() {
        Boolean pending = events().pendingFullscreen();
        return pending != null ? pending : Dom.isFullscreen();
    }

    @Override
    public boolean isExclusiveFullscreen() {
        return false;
    }

    @Override
    public void setSystemBehavior(SystemBehavior behavior, boolean enabled) {
        // Desktop behaviors; nothing to do in a page.
    }

    @Override
    public @Nullable DisplayMode currentDisplayMode() {
        return WebDisplays.screenMode();
    }

    @Override
    public void setRelativeMouseMode(boolean enabled) {
        events().setRelativeMouseMode(enabled);
        // The game holds the mouse while the player plays: that's when closing the tab by
        // accident loses the most.
        mouseHeld = enabled;
        updateLeaveGuard();
    }

    /**
     * In a world, closing the tab loses what wasn't saved (an inventory or chat open doesn't pause
     * singleplayer, so doesn't save it), and Ctrl+W, sprinting forward, is the browser's close-tab
     * shortcut outside fullscreen: the page asks first.
     */
    @Override
    public void setLeaveWarning(boolean warn) {
        inWorld = warn;
        updateLeaveGuard();
    }

    private void updateLeaveGuard() {
        boolean guard = mouseHeld || inWorld;
        if (guard != guarded) {
            guarded = guard;
            Dom.setLeaveGuard(canvas, guard);
        }
    }

    @Override
    public void warpMouse(double x, double y) {
        // Pages can't move the cursor.
    }

    @Override
    public CursorPosition cursorPosition() {
        // Where the mouse last moved over the page. A page can't see the cursor outside itself.
        return new CursorPosition(events().cursorX(), events().cursorY());
    }

    @Override
    public void setCursor(SystemCursor cursor) {
        Dom.setCursor(
                canvas,
                switch (cursor) {
                    case DEFAULT, ARROW -> "default";
                    case TEXT -> "text";
                    case CROSSHAIR -> "crosshair";
                    case POINTING_HAND -> "pointer";
                    case RESIZE_NS -> "ns-resize";
                    case RESIZE_EW -> "ew-resize";
                    case RESIZE_ALL -> "move";
                    case NOT_ALLOWED -> "not-allowed";
                });
    }

    /**
     * Typed text comes from key events; input methods, dead keys and touchscreen keyboards need a
     * text element with the focus, which the page's text input element is while this is on.
     */
    @Override
    public void startTextInput() {
        events().setTextInput(true);
        Dom.setTextInput(canvas, true);
    }

    @Override
    public void stopTextInput() {
        events().setTextInput(false);
        Dom.setTextInput(canvas, false);
    }

    @Override
    public void setTextInputArea(int areaX, int areaY, int areaWidth, int areaHeight) {
        // Window coordinates are the canvas's CSS pixels already.
        Dom.setTextInputArea(canvas, areaX, areaY, areaWidth, areaHeight);
    }

    @Override
    public void setQuitShortcutEnabled(boolean enabled) {}

    @Override
    public void close() {}
}
