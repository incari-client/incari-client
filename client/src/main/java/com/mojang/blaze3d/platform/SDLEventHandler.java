package com.mojang.blaze3d.platform;

import incari.client.modules.IncariModules;
import incari.client.platform.ClientPlatform;
import incari.client.platform.PlatformNotices;
import incari.platform.api.PlatformNotice;
import incari.platform.api.input.KeyAction;
import incari.platform.api.input.KeyModifiers;
import incari.platform.api.window.PlatformEventListener;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.options.controls.KeyBindsScreen;
import net.minecraft.client.input.InputQuirks;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonInfo;
import net.minecraft.client.input.PreeditEvent;
import net.minecraft.client.server.IntegratedServer;
import org.jspecify.annotations.Nullable;

/**
 * Incari's replacement for Mojang's SDL event pump (patches/classes.txt). The name stays because
 * the game refers to it; the events now come from the platform, and go to the same handlers.
 *
 * <p>Window events update the {@link Window} at once. Input events are handed to the game through
 * {@link Minecraft#execute}, as before, so they run in the game's task order.
 */
public class SDLEventHandler implements PlatformEventListener {

    /** The game's key action codes (see {@code KeyEvent.Action}). */
    private static final int ACTION_RELEASE = 0;

    private static final int ACTION_PRESS = 1;
    private static final int ACTION_REPEAT = -1;

    private final Minecraft minecraft;
    private final Window window;
    /** The game loop has run a pass, so frames have begun ({@link #pumpEvents}). */
    private boolean looping;

    /** The game's one handler, for Incari's own wait loops (singleplayer's server in a worker). */
    private static @Nullable SDLEventHandler current;

    public SDLEventHandler(final Minecraft minecraft, final Window window) {
        this.minecraft = minecraft;
        this.window = window;
        current = this;
    }

    /** The game's handler, once its window exists; null before. */
    public static @Nullable SDLEventHandler current() {
        return current;
    }

    /**
     * Once per pass of the game loop ({@code Minecraft.run}): ends the frame, which lets a browser
     * page run (its input, drawing and timers happen between the game's frames), then handles the
     * events that arrived.
     */
    public void pollEvents() {
        looping = true;
        ClientPlatform.get().endFrame();
        ClientPlatform.get().pollEvents(this);
        // In a world, closing the page loses what the last save didn't keep (a browser asks first),
        // and Ctrl+W, sprinting forward, is the browser's close-tab shortcut.
        this.window.platformWindow().setLeaveWarning(this.minecraft.level != null);
        PlatformNotices.tick(this.minecraft);
    }

    /**
     * Drops input queued during a stall, then handles what's left (window events). The game's wait
     * loops call it between the frames they draw themselves (loading a singleplayer world, saving
     * one on the way out), so once the game loop runs it ends the frame too: in the page, a loop
     * that never ends its frame never lets the integrated server and the background pool run,
     * which only get the page's thread between frames (world creation waited forever for its
     * first chunk). Before the first frame (start-up) it only handles events.
     */
    public void pumpEvents() {
        if (looping) {
            ClientPlatform.get().endFrame();
        }
        this.flushInputEvents();
        ClientPlatform.get().pollEvents(this);
    }

    public void flushInputEvents() {
        ClientPlatform.get().flushInputEvents();
    }

    // --- Window ---

    @Override
    public void closeRequested() {
        this.window.onCloseRequested();
    }

    @Override
    public void terminating() {
        this.window.onTerminating();
    }

    @Override
    public void windowMoved(final int x, final int y) {
        this.window.onMoved(x, y);
    }

    @Override
    public void windowResized(final int width, final int height) {
        this.window.onResized(width, height);
    }

    @Override
    public void framebufferResized(final int width, final int height) {
        this.window.onFramebufferResized(width, height);
    }

    @Override
    public void minimizedChanged(final boolean minimized) {
        if (minimized) {
            // A hidden page may be closed next, with nothing run after.
            IncariModules.saveNow();
            // Singleplayer's server on the page saves now too, as it does when the game pauses (a
            // world in a worker is told by the page itself: PlatformChannel.sendWhenLeaving).
            IntegratedServer server = this.minecraft.getSingleplayerServer();
            if (server != null) {
                server.execute(() -> server.saveEverything(true, false, false));
            }
        }
        this.window.onMinimized(minimized);
    }

    @Override
    public void focusChanged(final boolean focused) {
        this.window.onFocusChanged(focused);
    }

    @Override
    public void cursorEnteredOrLeft() {
        this.window.onCursorEnteredOrLeft();
    }

    @Override
    public void displayChanged() {
        this.window.onDisplayChanged();
    }

    @Override
    public void fullscreenChanged(final boolean fullscreen) {
        this.window.onFullscreenChanged();
    }

    @Override
    public void displayAdded(final int displayId) {
        this.window.onDisplayAdded(displayId);
    }

    @Override
    public void displayRemoved(final int displayId) {
        this.window.onDisplayRemoved(displayId);
    }

    @Override
    public void displayModeChanged(final int displayId) {
        this.window.onDisplayModeChanged(displayId);
    }

    // --- Keyboard ---

    @Override
    public void key(final int scancode, final int keyCode, final int modifiers, final KeyAction action) {
        int gameAction =
                switch (action) {
                    case PRESS -> ACTION_PRESS;
                    case RELEASE -> ACTION_RELEASE;
                    case REPEAT -> ACTION_REPEAT;
                };
        KeyEvent event = new KeyEvent(scancode, keyCode, modifiers);
        long handle = this.window.handle();
        this.minecraft.execute(() -> this.minecraft.keyboardHandler.keyPress(handle, gameAction, event));
    }

    @Override
    public void keymapChanged() {
        // Key names depend on the layout: refresh them where they're shown (the modules' HUD
        // names keys too, Keystrokes).
        this.minecraft.execute(() -> {
            IncariModules.keymapChanged();
            if (this.minecraft.gui.screen() instanceof KeyBindsScreen keyBinds) {
                keyBinds.refreshKeybindLabels();
            }
        });
    }

    @Override
    public void textInput(final String text) {
        long handle = this.window.handle();
        this.minecraft.execute(() -> this.minecraft.keyboardHandler.textInput(handle, text));
    }

    @Override
    public void textEditing(final String text, final int start, final int length) {
        PreeditEvent preedit = PreeditEvent.fromSdlTextEditing(text, start, length);
        long handle = this.window.handle();
        this.minecraft.execute(() -> this.minecraft.keyboardHandler.textEditing(handle, preedit));
    }

    // --- Mouse ---

    @Override
    public void mouseMoved(final double x, final double y, final double dx, final double dy) {
        long handle = this.window.handle();
        this.minecraft.execute(() -> this.minecraft.mouseHandler.onMove(handle, x, y, dx, dy));
    }

    @Override
    public void mouseButton(final int button, final boolean pressed, final int modifiers) {
        MouseButtonInfo info = new MouseButtonInfo(button, modifiers);
        int action = pressed ? ACTION_PRESS : ACTION_RELEASE;
        long handle = this.window.handle();
        this.minecraft.execute(() -> this.minecraft.mouseHandler.onButton(handle, info, action));
    }

    @Override
    public void mouseWheel(final double dx, final double dy, final int modifiers) {
        double scrollX = dx;
        double scrollY = dy;
        // macOS turns Shift+wheel into horizontal scrolling; the game wants it vertical again.
        if (InputQuirks.SHIFT_INVERTS_SCROLL_AXIS && dy == 0.0 && dx != 0.0 && (modifiers & KeyModifiers.SHIFT) != 0) {
            scrollX = 0.0;
            scrollY = -dx;
        }
        double finalX = scrollX;
        double finalY = scrollY;
        long handle = this.window.handle();
        this.minecraft.execute(() -> this.minecraft.mouseHandler.onScroll(handle, finalX, finalY));
    }

    @Override
    public void notice(final PlatformNotice notice) {
        PlatformNotices.show(this.minecraft, notice);
    }

    @Override
    public void filesDropped(final List<String> paths) {
        long handle = this.window.handle();
        this.minecraft.execute(() -> this.minecraft.mouseHandler.onDrop(handle, paths));
    }
}
