package incari.platform.api.window;

import java.util.List;
import org.jspecify.annotations.Nullable;

/**
 * The game's window: a desktop window, or the page's canvas in the browser.
 *
 * <p>State queries answer from the platform's current state; changes are also reported as
 * {@link PlatformEventListener} events. Call everything from the render thread.
 */
public interface PlatformWindow extends AutoCloseable {

    /**
     * The handle the graphics backend and the game use to identify this window: on the desktop,
     * the SDL window pointer; in the browser, a constant non-zero id. Never 0.
     */
    long nativeHandle();

    /** The left edge on the virtual desktop, in screen coordinates. */
    int x();

    int y();

    /** The width in screen coordinates. */
    int width();

    int height();

    /** The drawable surface's width in pixels, at least 1: what the graphics surface is sized to. */
    int framebufferWidth();

    int framebufferHeight();

    /**
     * Pixels at the right edge of the framebuffer the game shouldn't lay its UI out in, usually 0.
     * The desktop's borderless fullscreen makes the window one pixel wider than the display, so
     * drivers don't mistake it for exclusive fullscreen; that extra column is off screen.
     */
    int unusedWidth();

    /** Pixels per screen coordinate: 1 on normal displays, 2 on most high-density ones. */
    float pixelDensity();

    /** The id of the display the window is mostly on, or 0 if unknown. */
    int displayId();

    void setTitle(String title);

    /** Sets the window icon from one or more sizes, largest first. Ignored where there is none. */
    void setIcon(List<IconImage> sizes);

    /** Limits how large the player can make the window; 0 for no limit. */
    void setMaximumSize(int width, int height);

    /**
     * Leaves fullscreen, if in it, and places the window.
     *
     * @return false if the platform refused; the window is then unchanged
     */
    boolean setWindowed(int x, int y, int width, int height);

    /**
     * Enters fullscreen on the window's display.
     *
     * @param exclusiveMode the mode to switch the display to, or null for borderless fullscreen
     *     at the desktop's own mode. When the display has no mode close to it, the platform falls
     *     back to borderless and logs why.
     * @return false if the platform refused; the window is then unchanged
     */
    boolean setFullscreen(@Nullable DisplayMode exclusiveMode);

    boolean isFullscreen();

    /** Whether the window is fullscreen with the display switched to a mode of its own. */
    boolean isExclusiveFullscreen();

    /**
     * Turns a system behavior the player controls on or off. Platforms without it ignore the call.
     */
    void setSystemBehavior(SystemBehavior behavior, boolean enabled);

    /** The mode of the display the window is on, or null if it can't be determined. */
    @Nullable
    DisplayMode currentDisplayMode();

    /**
     * Relative mouse mode: the cursor is hidden and locked, and motion is reported as deltas
     * (the browser's Pointer Lock). Used while the player looks around.
     */
    void setRelativeMouseMode(boolean enabled);

    /**
     * Whether closing the window now would lose the player's game (they're in a world): a browser
     * then asks before the page is closed or reloaded, as it already does while the mouse is held.
     * Cheap to call every frame with the same value. The desktop ignores it.
     */
    default void setLeaveWarning(boolean warn) {}

    /** Moves the cursor to a position in window coordinates. Ignored where not allowed. */
    void warpMouse(double x, double y);

    /** The cursor position in window coordinates, even when it's outside the window. */
    CursorPosition cursorPosition();

    void setCursor(SystemCursor cursor);

    /** Starts delivering {@link PlatformEventListener#textInput} and composition events. */
    void startTextInput();

    void stopTextInput();

    /**
     * Where the text being typed is, in window coordinates, so an input method can place its
     * candidate list next to it.
     */
    void setTextInputArea(int x, int y, int width, int height);

    /**
     * Whether the system's close shortcut (Alt+F4 on Windows) closes the window. The game turns
     * it off while a key binding might use it.
     */
    void setQuitShortcutEnabled(boolean enabled);

    /** Destroys the window. */
    @Override
    void close();

    /** A cursor position in window coordinates. */
    record CursorPosition(double x, double y) {}
}
