package incari.platform.api.window;

import incari.platform.api.PlatformNotice;
import incari.platform.api.input.KeyAction;
import incari.platform.api.input.KeyModifiers;
import incari.platform.api.input.Scancodes;
import java.util.List;

/**
 * Receives window and input events from {@code Platform.pollEvents}. Every method has a no-op
 * default, so a listener implements only what it handles. Events arrive on the thread that polls,
 * in the order they happened.
 *
 * <p>Units: window positions and sizes are in screen coordinates, framebuffer sizes in pixels
 * (they differ on high-density displays); mouse positions are in window coordinates with the
 * origin at the top left. Key and modifier values are described in {@link Scancodes} and
 * {@link KeyModifiers}.
 */
public interface PlatformEventListener {

    /** The player asked to close the window: the close button, or the system's quit shortcut. */
    default void closeRequested() {}

    /** The system is ending the application; it must shut down now. */
    default void terminating() {}

    default void windowMoved(int x, int y) {}

    default void windowResized(int width, int height) {}

    /**
     * The drawable size changed, for example after a resize or a move to a denser display. The size
     * is the whole surface, as {@link PlatformWindow#framebufferWidth} reports it.
     */
    default void framebufferResized(int width, int height) {}

    default void minimizedChanged(boolean minimized) {}

    default void focusChanged(boolean focused) {}

    /** The mouse entered or left the window. */
    default void cursorEnteredOrLeft() {}

    /** The window moved to another display, or its display's scale changed. */
    default void displayChanged() {}

    /** The window entered or left fullscreen, whoever asked for it. */
    default void fullscreenChanged(boolean fullscreen) {}

    default void displayAdded(int displayId) {}

    default void displayRemoved(int displayId) {}

    default void displayModeChanged(int displayId) {}

    /**
     * A key was pressed, released or auto-repeated.
     *
     * @param scancode the physical key, as a USB HID usage id ({@link Scancodes})
     * @param keyCode the key's meaning in the current layout: its Unicode code point when it types
     *     a character, otherwise {@code scancode | 0x40000000} (SDL's keycode convention)
     * @param modifiers the modifiers held, as {@link KeyModifiers} bits
     */
    default void key(int scancode, int keyCode, int modifiers, KeyAction action) {}

    /** The keyboard layout changed, so key names may have too. */
    default void keymapChanged() {}

    /** Text was typed, after any input method composed it. */
    default void textInput(String text) {}

    /**
     * An input method is composing text that isn't committed yet.
     *
     * @param start the cursor position within {@code text}, in characters
     * @param length the length of the selection within {@code text}, in characters
     */
    default void textEditing(String text, int start, int length) {}

    /**
     * The mouse moved.
     *
     * @param x the new position in window coordinates
     * @param dx the relative motion; in relative mouse mode this is the only meaningful part
     */
    default void mouseMoved(double x, double y, double dx, double dy) {}

    /**
     * A mouse button changed state.
     *
     * @param button 1 left, 2 middle, 3 right, 4 and up for extra buttons
     * @param modifiers the keyboard modifiers held, as {@link KeyModifiers} bits
     */
    default void mouseButton(int button, boolean pressed, int modifiers) {}

    /**
     * The wheel or a trackpad scrolled.
     *
     * @param dx horizontal scroll, positive to the right
     * @param dy vertical scroll, positive away from the user
     * @param modifiers the keyboard modifiers held, as {@link KeyModifiers} bits
     */
    default void mouseWheel(double dx, double dy, int modifiers) {}

    /** Files were dropped onto the window, as platform paths or URLs. */
    default void filesDropped(List<String> paths) {}

    /**
     * Something the player must be told: that their game isn't being saved, for one. Each notice
     * comes once when it starts to apply, not every frame.
     */
    default void notice(PlatformNotice notice) {}
}
