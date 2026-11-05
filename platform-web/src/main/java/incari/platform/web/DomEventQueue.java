package incari.platform.web;

import incari.platform.api.input.KeyAction;
import incari.platform.api.input.KeyModifiers;
import incari.platform.api.input.Scancodes;
import incari.platform.api.window.PlatformEventListener;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Objects;
import org.jspecify.annotations.Nullable;

/**
 * DOM events, turned into the platform's events and queued until the game's next
 * {@code pollEvents}, plus the keyboard, mouse and canvas state they imply.
 *
 * <p>The browser calls {@link #event} from its event handlers, between the game's frames (the
 * game yields every frame), so everything here runs on the page's one thread. No JavaScript here:
 * {@link Dom} flattens each DOM event to primitives and does what the queue asks through
 * {@link Surface}, which keeps this logic testable on a JVM.
 *
 * <p>The events follow SDL3's rules, which the game was written against: a key or button is
 * released only after it was pressed, everything held is released when focus leaves, text arrives
 * only while text input is on, and the window's size is reported as it changes.
 */
final class DomEventQueue {

    /** What the queue asks of the page. */
    interface Surface {

        /** Sizes the canvas's backing store: the framebuffer the game draws into. */
        void setBackingSize(int pixelWidth, int pixelHeight);

        /** Requests or leaves Pointer Lock. The result arrives later as a "lock" event. */
        void setPointerLock(boolean locked);
    }

    // The modifier flags Dom packs from a DOM event.
    static final int DOM_SHIFT = 1;
    static final int DOM_CTRL = 2;
    static final int DOM_ALT = 4;
    static final int DOM_META = 8;
    static final int DOM_CAPS_LOCK = 16;
    static final int DOM_NUM_LOCK = 32;
    static final int DOM_ALT_GRAPH = 64;

    /**
     * The most input events held between two polls, so a long stall (a slow load, a tab whose
     * frames stopped) can't grow the queue without bound. Past it, new presses, moves and text are
     * dropped; releases and window events are always kept (so nothing stays held), bounded by the
     * number of keys and buttons.
     */
    static final int MAX_PENDING = 256;

    /** Wheel movement per notch in each {@code WheelEvent.deltaMode}; the game wants 1 a notch. */
    private static final double PIXELS_PER_NOTCH = 100;

    private static final double LINES_PER_NOTCH = 3;
    /** {@code WheelEvent.wheelDelta}'s unit: 120 per notch in every browser that has it. */
    private static final double LEGACY_WHEEL_PER_NOTCH = 120;

    private static final int ESCAPE_KEYCODE = 0x1B;

    private final Surface surface;
    /**
     * Whether keys pressed while Command is held never report their release, as on macOS: the
     * browser drops those keyups, so they're released when Command is.
     */
    private final boolean metaSwallowsKeyUps;
    /**
     * Whether F then a digit types a function key ({@link #chord}): on ChromeOS, whose keyboards
     * have no function row the game can use. Elsewhere F is reported the moment it goes down.
     */
    private final boolean functionKeyChords;

    private final ArrayDeque<Event> pending = new ArrayDeque<>();
    private final boolean[] keysDown = new boolean[Scancodes.COUNT];
    /** The unshifted character each key typed last, for keycodes (see {@link DomKeys#keyCode}). */
    private final int[] learnedKeyCodes = new int[Scancodes.COUNT];
    /** The keycode each held key was pressed with, for the releases the queue makes up. */
    private final int[] heldKeyCodes = new int[Scancodes.COUNT];
    /**
     * Each key's label in the player's layout, from the Keyboard Map API ({@code "keymap"}
     * events), or null where the browser has none for it.
     */
    private final @Nullable String[] layoutNames = new String[Scancodes.COUNT];
    /**
     * Each key's label as learned from typing, for browsers without the Keyboard Map API (Firefox,
     * Safari): what it typed last without Shift or AltGr.
     */
    private final @Nullable String[] typedNames = new String[Scancodes.COUNT];
    /** The F key, which with a digit types a function key (see {@link #chord}). */
    private static final int CHORD_KEY = Scancodes.A + 5;

    /** F is down and not yet reported: a digit may still turn it into a function key. */
    private boolean chordKeyPending;
    /** F is down and a digit already turned it into a function key; its release isn't reported. */
    private boolean chordKeyUsed;

    private int chordKeyCode;
    private int chordKeyModifiers;
    /** The function key each held digit stands for, or 0, so its release releases that key. */
    private final int[] chordedKeys = new int[Scancodes.COUNT];
    /** A bit per platform mouse button (1 << button) pressed on the canvas and not yet released. */
    private int buttonsDown;

    private int modifiers;
    private double cursorX;
    private double cursorY;
    private String lastPaste = "";
    /** Whether an input method's composition is showing, so its end must be reported. */
    private boolean composing;

    private boolean focused = true;
    /** The tab is hidden ("visibility"). */
    private boolean hidden;
    /**
     * The WebGL context was lost ("graphics", sent by render-webgl2's {@code WebGl}): the canvas
     * can't be drawn again, so the game is told its window is minimized, which stops it drawing
     * and holds it to a few frames a second until the page is reloaded.
     */
    private boolean graphicsLost;

    private boolean textInput;

    /** Whether the game wants relative mouse mode; {@link #pointerLocked} is whether it has it. */
    private boolean relativeWanted;

    private boolean pointerLocked;
    /** Pointer Lock ended while the game still wanted it: the player pressed Escape. */
    private boolean lockLostByPlayer;
    /**
     * The queue asked the page to leave Pointer Lock and the page hasn't confirmed. The game may
     * want the mouse back before then (a respawn closes the death screen a tick later), and that
     * lock's end mustn't look like the player's Escape.
     */
    private boolean exitAsked;

    private boolean escapeSinceDrain;
    private boolean blurSinceDrain;
    /** The fullscreen state asked for and not yet confirmed by the page, or null. */
    private @Nullable Boolean fullscreenTarget;

    private CanvasSize applied;
    private CanvasSize latest;
    private boolean sizeReportForced;

    DomEventQueue(Surface surface, CanvasSize initialSize, boolean metaSwallowsKeyUps, boolean functionKeyChords) {
        this.surface = surface;
        this.metaSwallowsKeyUps = metaSwallowsKeyUps;
        this.functionKeyChords = functionKeyChords;
        this.applied = initialSize;
        this.latest = initialSize;
        surface.setBackingSize(initialSize.pixelWidth(), initialSize.pixelHeight());
    }

    /**
     * One DOM event, flattened by {@link Dom#listen} (its slots are described there) and taken from
     * the page's queue at the game's poll.
     *
     * @return whether the page should cancel the event's default action
     */
    boolean event(String type, String text, String key, double a, double b, double c, double d, double e, int flags) {
        return switch (type) {
            case "keydown" -> key(true, text, key, a != 0, b != 0, flags);
            case "compose" -> {
                compose(text, (int) a, (int) b);
                yield false;
            }
            case "commit" -> {
                commit(text);
                yield false;
            }
            case "keymap" -> {
                keymap(text);
                yield false;
            }
            case "drop" -> {
                drop(text);
                yield false;
            }
            case "keyup" -> key(false, text, key, false, b != 0, flags);
            case "move" -> move(a, b, c, d, flags);
            case "down" -> button((int) a + 1, true, flags);
            case "up" -> button((int) a + 1, false, flags);
            case "wheel" -> wheel(a, b, (int) c, d, e, flags);
            case "focus" -> {
                focused = true;
                add(Event.window(Kind.FOCUS, true));
                yield false;
            }
            case "blur" -> {
                blur();
                yield false;
            }
            case "visibility" -> {
                hidden = a != 0;
                if (hidden && focused) {
                    // A hidden tab can't be played. Browsers usually send blur first, but not in
                    // every case (some systems minimize without it): release everything and let
                    // the game pause, as it does when focus leaves.
                    blur();
                }
                add(Event.window(Kind.MINIMIZED, hidden || graphicsLost));
                yield false;
            }
            case "graphics" -> {
                boolean lost = a != 0;
                if (lost && !graphicsLost) {
                    graphicsLost = true;
                    add(Event.window(Kind.MINIMIZED, true));
                }
                // A restored context doesn't bring back what was on the GPU: the page must be
                // reloaded, so the game stays minimized.
                yield false;
            }
            case "enter" -> {
                add(Event.window(Kind.ENTERED_OR_LEFT, true));
                yield false;
            }
            case "resize" -> {
                latest = CanvasSize.measure(a, b, c, d, e, latest);
                yield false;
            }
            case "fullscreen" -> {
                fullscreenTarget = null;
                add(Event.window(Kind.FULLSCREEN, a != 0));
                yield false;
            }
            case "lock" -> {
                lockChanged(a != 0);
                yield false;
            }
            case "paste" -> {
                lastPaste = text;
                yield false;
            }
            // "lockerror", and event types a later Dom adds.
            default -> false;
        };
    }

    // --- Keyboard ------------------------------------------------------------------------------

    private boolean key(boolean down, String code, String key, boolean repeat, boolean composing, int flags) {
        if (composing) {
            // An input method owns these keys until it commits, and must get their default
            // action; what it composes arrives as "compose" and "commit" events.
            return false;
        }
        int scancode = DomKeys.scancode(code);
        boolean tracked = scancode > 0 && scancode < keysDown.length;
        boolean wasDown = tracked && keysDown[scancode];
        if (tracked) {
            keysDown[scancode] = down;
        }
        modifiers = modifiers(flags);
        if (!down && tracked && !wasDown) {
            // A release whose press the game never saw: focus came after the press, or the keys
            // were released already (on blur, or with Command on macOS).
            return consumesKey(code, key, flags);
        }
        if (!down && metaSwallowsKeyUps && (scancode == Scancodes.LEFT_SUPER || scancode == Scancodes.RIGHT_SUPER)) {
            releaseKeysExceptModifiers();
        }
        if (down && scancode == Scancodes.ESCAPE) {
            escapeSinceDrain = true;
        }
        boolean levelShifted = (flags & (DOM_SHIFT | DOM_ALT | DOM_ALT_GRAPH)) != 0;
        if (down && tracked && !levelShifted && !code.startsWith("Numpad")) {
            learnName(scancode, key);
        }
        int keyCode = DomKeys.keyCode(code, scancode, key, levelShifted, learnedKeyCodes);
        KeyAction action = !down ? KeyAction.RELEASE : repeat && wasDown ? KeyAction.REPEAT : KeyAction.PRESS;
        if (tracked && action == KeyAction.PRESS) {
            heldKeyCodes[scancode] = keyCode;
        }
        // A key the page knows neither by place nor by meaning is no key to the game: its
        // unbound binds all sit on that same unknown key, and would fire. One with a keycode
        // stays (a virtual keyboard's Backspace has no code, and text fields need it).
        boolean unknown = scancode == Scancodes.UNKNOWN && (keyCode & ~DomKeys.NON_PRINTABLE) == 0;
        if (!unknown && !chord(down, scancode, keyCode, action, flags)) {
            add(Event.key(scancode, keyCode, modifiers, action));
        }
        if (down && textInput) {
            // Like SDL, text follows its key event, and Ctrl or Command shortcuts type nothing.
            // AltGr is Ctrl+Alt on Windows, and does type ("@" on a German keyboard).
            int character = DomKeys.singleCodePoint(key);
            boolean shortcut = (flags & (DOM_CTRL | DOM_META)) != 0 && (flags & DOM_ALT_GRAPH) == 0;
            if (character >= 0x20 && character != 0x7F && !shortcut) {
                add(Event.text(key));
            }
        }
        return consumesKey(code, key, flags);
    }

    /**
     * On ChromeOS ({@link #functionKeyChords}), whose keyboards have no function row keys the game
     * can use, F then a digit types the function key: F+3 is F3, F+5 is F5, and F+3+H is F3+H (F+1..9 are F1..F9, F+0 is F10). Not while
     * typing text, where "f3" is just text.
     *
     * <p>F alone still presses F, but only when it's released or another key goes down, because the
     * queue can't know a digit is coming. A chord's F is never reported, so it doesn't also act as
     * its own key (swap hands).
     *
     * @return whether the queue handled the key, so the caller must not report it as it came
     */
    private boolean chord(boolean down, int scancode, int keyCode, KeyAction action, int flags) {
        if (!functionKeyChords) {
            return false;
        }
        boolean plain = (flags & (DOM_CTRL | DOM_ALT | DOM_META | DOM_ALT_GRAPH)) == 0;
        if (scancode == CHORD_KEY) {
            if (down) {
                if (action == KeyAction.PRESS && plain && !textInput) {
                    chordKeyPending = true;
                    chordKeyCode = keyCode;
                    chordKeyModifiers = modifiers;
                    return true;
                }
                // The game saw this F, or it's a repeat of one the chord swallowed.
                return action == KeyAction.REPEAT && (chordKeyPending || chordKeyUsed);
            }
            if (chordKeyPending) {
                chordKeyPending = false;
                add(Event.key(scancode, chordKeyCode, chordKeyModifiers, KeyAction.PRESS));
                return false;
            }
            if (chordKeyUsed) {
                chordKeyUsed = false;
                return true;
            }
            return false;
        }
        if (scancode < 0 || scancode >= chordedKeys.length) {
            return false;
        }
        int functionKey = chordedKeys[scancode];
        if (!down) {
            if (functionKey == 0) {
                return false;
            }
            chordedKeys[scancode] = 0;
            releaseFunctionKey(functionKey);
            return true;
        }
        boolean digit = scancode >= Scancodes.DIGIT_1 && scancode <= Scancodes.DIGIT_0;
        if (digit && plain && (chordKeyPending || chordKeyUsed)) {
            int target = Scancodes.F1 + (scancode == Scancodes.DIGIT_0 ? 9 : scancode - Scancodes.DIGIT_1);
            if (action == KeyAction.PRESS) {
                chordKeyPending = false;
                chordKeyUsed = true;
                chordedKeys[scancode] = target;
                keysDown[target] = true;
                heldKeyCodes[target] = target | DomKeys.NON_PRINTABLE;
            } else if (functionKey == 0) {
                return true;
            } else {
                target = functionKey;
            }
            add(Event.key(target, target | DomKeys.NON_PRINTABLE, modifiers, action));
            return true;
        }
        if (chordKeyPending && scancode < Scancodes.LEFT_CTRL) {
            // Not a chord after all: the F goes first, then this key. Modifiers don't end the
            // wait, so F+Shift+3 works.
            chordKeyPending = false;
            add(Event.key(CHORD_KEY, chordKeyCode, chordKeyModifiers, KeyAction.PRESS));
        }
        return false;
    }

    private void releaseFunctionKey(int functionKey) {
        keysDown[functionKey] = false;
        add(Event.key(functionKey, functionKey | DomKeys.NON_PRINTABLE, modifiers, KeyAction.RELEASE));
    }

    /**
     * Whether the browser's default action for a key is cancelled (the game gets every key either
     * way). Everything is cancelled (Tab would move focus, F5 reload, Space scroll, F3 search, Alt
     * the menu bar) except:
     * <ul>
     *   <li>developer tools;
     *   <li>paste: the browser only fires the paste event, with the clipboard's text, for its own
     *       Ctrl+V;
     *   <li>dead keys, which start a composition ("´" then "e" types "é") that the page's text
     *       input element has to see.
     * </ul>
     * The browser keeps some shortcuts (Ctrl+W, Ctrl+T, Ctrl+1) whatever the page does, except in
     * fullscreen, where the page locks them ({@link Dom#listen}).
     *
     * <p>Called from the page's key listener while the event is dispatched: must stay a pure
     * function of its arguments (see {@link Dom.KeyPolicy}).
     */
    static boolean consumesKey(String code, String key, int flags) {
        if ("Dead".equals(key)) {
            return false;
        }
        boolean shortcut = (flags & (DOM_CTRL | DOM_META)) != 0;
        return switch (code) {
            case "F12" -> false;
            case "KeyV" -> !shortcut;
            case "KeyI", "KeyJ" -> !(shortcut && (flags & DOM_SHIFT) != 0);
            default -> true;
        };
    }

    // --- Text input and key names ---------------------------------------------------------------

    /**
     * An input method's composition changed ({@code text} empty when it's cancelled). The page's
     * text input element reports it; the game draws it in the focused text field.
     */
    private void compose(String text, int cursor, int selection) {
        if (!textInput) {
            composing = false;
            return;
        }
        if (text.isEmpty()) {
            if (composing) {
                composing = false;
                add(Event.editing("", 0, 0));
            }
            return;
        }
        composing = true;
        int start = Math.max(0, Math.min(cursor, text.length()));
        int length = Math.max(0, Math.min(selection, text.length() - start));
        add(Event.editing(text, start, length));
    }

    /**
     * Text an input method committed, or text that came without a key event the page could
     * cancel: a virtual keyboard, voice input, an input method's candidate list.
     */
    private void commit(String text) {
        if (composing) {
            composing = false;
            add(Event.editing("", 0, 0));
        }
        if (!textInput) {
            return;
        }
        String typed = withoutControls(text);
        if (!typed.isEmpty()) {
            add(Event.text(typed));
        }
    }

    /** {@code text} without control characters: a text field takes Enter and Tab as keys, not text. */
    private static String withoutControls(String text) {
        StringBuilder kept = new StringBuilder(text.length());
        for (int i = 0; i < text.length(); i++) {
            char c = text.charAt(i);
            if (c >= 0x20 && (c < 0x7F || c >= 0xA0)) {
                kept.append(c);
            }
        }
        return kept.toString();
    }

    /**
     * The player's keyboard layout from the Keyboard Map API: one {@code "<code> <character>"}
     * entry per line, as {@link Dom#listen} flattens it. Reported when it changes, which is when
     * the page starts and whenever it regains focus (the player may have switched layouts).
     */
    private void keymap(String entries) {
        @Nullable String[] names = new String[Scancodes.COUNT];
        int start = 0;
        while (start < entries.length()) {
            int end = entries.indexOf('\n', start);
            if (end < 0) {
                end = entries.length();
            }
            int space = entries.indexOf(' ', start);
            if (space > start && space < end) {
                String code = entries.substring(start, space);
                String name = nameFor(entries.substring(space + 1, end));
                int scancode = DomKeys.scancode(code);
                if (name != null && scancode > 0 && scancode < names.length && !code.startsWith("Numpad")) {
                    names[scancode] = name;
                    // What the key types unshifted, for its keycode when it's pressed with Shift.
                    learnedKeyCodes[scancode] = name.codePointAt(0);
                }
            }
            start = end + 1;
        }
        boolean changed = false;
        for (int scancode = 0; scancode < names.length; scancode++) {
            if (!Objects.equals(names[scancode], layoutNames[scancode])) {
                layoutNames[scancode] = names[scancode];
                changed = true;
            }
        }
        if (changed) {
            add(Event.window(Kind.KEYMAP, true));
        }
    }

    /**
     * Learns a key's label from what it typed, where the browser has no keyboard map. A key that
     * types something else than it did means the player switched layouts, which is announced; the
     * first thing a key types isn't, or every new key would be.
     */
    private void learnName(int scancode, String key) {
        String name = nameFor(key);
        String before = typedNames[scancode];
        if (name == null || name.equals(before)) {
            return;
        }
        typedNames[scancode] = name;
        if (before != null && layoutNames[scancode] == null) {
            add(Event.window(Kind.KEYMAP, true));
        }
    }

    /**
     * A key's label from the character it types: the character in lower case (the game shows key
     * names in upper case, and Caps Lock mustn't change them), or null if it can't label a key.
     */
    private static @Nullable String nameFor(String character) {
        int c = DomKeys.singleCodePoint(character);
        if (c <= 0x20 || (c >= 0x7F && c <= 0xA0) || Character.isWhitespace(c)) {
            return null;
        }
        return new String(Character.toChars(Character.toLowerCase(c)));
    }

    /**
     * The key's label in the player's layout: from the browser's keyboard map, else what the key
     * last typed, else its US keyboard character (the game has no letter key names of its own, as
     * SDL always supplies them). Null for keys the game names itself (F1, Enter).
     */
    @Nullable
    String keyName(int scancode) {
        if (scancode <= 0 || scancode >= layoutNames.length) {
            return null;
        }
        String name = layoutNames[scancode];
        if (name == null) {
            name = typedNames[scancode];
        }
        return name != null ? name : DomKeys.usName(scancode);
    }

    // --- Files -----------------------------------------------------------------------------------

    /** Files dropped on the page, already copied into the file system: their paths, one per line. */
    private void drop(String paths) {
        List<String> files = new ArrayList<>();
        int start = 0;
        while (start < paths.length()) {
            int end = paths.indexOf('\n', start);
            if (end < 0) {
                end = paths.length();
            }
            if (end > start) {
                files.add(paths.substring(start, end));
            }
            start = end + 1;
        }
        if (!files.isEmpty()) {
            add(Event.files(files));
        }
    }

    private void releaseKeysExceptModifiers() {
        for (int scancode = 1; scancode < Scancodes.LEFT_CTRL; scancode++) {
            if (keysDown[scancode]) {
                keysDown[scancode] = false;
                add(Event.key(scancode, heldKeyCodes[scancode], modifiers, KeyAction.RELEASE));
            }
        }
    }

    /**
     * The platform's modifier bits for a DOM event's flags. The DOM says only whether a Shift is
     * held; which one comes from the keys the queue saw go down.
     */
    private int modifiers(int flags) {
        int bits = 0;
        if ((flags & DOM_SHIFT) != 0) {
            bits |= side(
                    Scancodes.LEFT_SHIFT, Scancodes.RIGHT_SHIFT, KeyModifiers.LEFT_SHIFT, KeyModifiers.RIGHT_SHIFT);
        }
        if ((flags & DOM_CTRL) != 0) {
            bits |= side(Scancodes.LEFT_CTRL, Scancodes.RIGHT_CTRL, KeyModifiers.LEFT_CTRL, KeyModifiers.RIGHT_CTRL);
        }
        if ((flags & DOM_ALT) != 0) {
            bits |= side(Scancodes.LEFT_ALT, Scancodes.RIGHT_ALT, KeyModifiers.LEFT_ALT, KeyModifiers.RIGHT_ALT);
        }
        if ((flags & DOM_META) != 0) {
            bits |= side(
                    Scancodes.LEFT_SUPER, Scancodes.RIGHT_SUPER, KeyModifiers.LEFT_SUPER, KeyModifiers.RIGHT_SUPER);
        }
        if ((flags & DOM_CAPS_LOCK) != 0) {
            bits |= KeyModifiers.CAPS_LOCK;
        }
        if ((flags & DOM_NUM_LOCK) != 0) {
            bits |= KeyModifiers.NUM_LOCK;
        }
        return bits;
    }

    private int side(int leftKey, int rightKey, int leftBit, int rightBit) {
        boolean left = keysDown[leftKey];
        boolean right = keysDown[rightKey];
        if (left == right) {
            // Both, or neither seen (held since before the page had focus): say left, as SDL
            // does for a modifier it can't place.
            return right ? leftBit | rightBit : leftBit;
        }
        return left ? leftBit : rightBit;
    }

    // --- Mouse ---------------------------------------------------------------------------------

    private boolean move(double x, double y, double movementX, double movementY, int flags) {
        modifiers = modifiers(flags);
        if (!pointerLocked) {
            cursorX = x;
            cursorY = y;
        }
        // Under Pointer Lock the cursor stays where it was locked: only the motion means anything.
        // While the game wants the lock but doesn't have it (it's being requested, or the player
        // left it), the cursor is free and its motion mustn't turn the camera.
        boolean motionCounts = pointerLocked || !relativeWanted;
        double dx = motionCounts ? movementX : 0;
        double dy = motionCounts ? movementY : 0;
        Event last = pending.peekLast();
        if (last != null && last.kind == Kind.MOVE) {
            // Hot path: a fast mouse fires several moves a frame; the game only needs where the
            // cursor ended up and how far it went.
            last.x = cursorX;
            last.y = cursorY;
            last.dx += dx;
            last.dy += dy;
        } else {
            add(Event.move(cursorX, cursorY, dx, dy));
        }
        return false;
    }

    private boolean button(int button, boolean pressed, int flags) {
        if (button < 1 || button > 30) {
            return false;
        }
        int bit = 1 << button;
        modifiers = modifiers(flags);
        if (pressed) {
            if (relativeWanted && !pointerLocked) {
                // The game is looking around but lost the lock (the player pressed Escape, or the
                // request failed without a click to allow it). A click allows it: take the lock
                // back and keep the click, which would otherwise hit whatever the camera faced.
                surface.setPointerLock(true);
                return true;
            }
            buttonsDown |= bit;
            add(Event.button(button, true, modifiers));
            return true;
        }
        if ((buttonsDown & bit) == 0) {
            // Released without a press on the canvas: a drag that began elsewhere on the page.
            return false;
        }
        buttonsDown &= ~bit;
        add(Event.button(button, false, modifiers));
        // Also stops the back and forward buttons from navigating away from the game.
        return true;
    }

    private boolean wheel(double deltaX, double deltaY, int mode, double legacyX, double legacyY, int flags) {
        modifiers = modifiers(flags);
        double stepsX = wheelSteps(deltaX, mode, -legacyX);
        double stepsY = wheelSteps(deltaY, mode, -legacyY);
        if ((flags & DOM_SHIFT) != 0 && stepsY == 0 && stepsX != 0) {
            // Chrome turns Shift+wheel into horizontal scrolling; the game wants it vertical, as a
            // desktop reports it.
            stepsY = stepsX;
            stepsX = 0;
        }
        // The DOM's deltas point down and right; the platform's vertical axis points up.
        double dx = stepsX;
        double dy = 0.0 - stepsY; // not -stepsY, which makes a -0.0 of no scroll
        Event last = pending.peekLast();
        if (last != null && last.kind == Kind.WHEEL) {
            last.dx += dx;
            last.dy += dy;
            last.modifiers = modifiers;
        } else {
            add(Event.wheel(dx, dy, modifiers));
        }
        return true;
    }

    /**
     * Wheel notches from one axis of a {@code WheelEvent}, positive down or right.
     *
     * @param legacy the legacy {@code wheelDelta} with its sign flipped to match {@code delta}, or
     *     0 where the browser has none. It counts notches exactly (120 each) whatever the zoom or
     *     platform, where pixel deltas vary (100 per notch in Chrome on Windows, other amounts
     *     elsewhere, and scaled by the page zoom).
     */
    static double wheelSteps(double delta, int mode, double legacy) {
        if (mode == 0 && legacy != 0) {
            return legacy / LEGACY_WHEEL_PER_NOTCH;
        }
        return switch (mode) {
            case 1 -> delta / LINES_PER_NOTCH;
            // A page per notch.
            case 2 -> delta;
            default -> delta / PIXELS_PER_NOTCH;
        };
    }

    // --- Focus and Pointer Lock ------------------------------------------------------------------

    private void blur() {
        focused = false;
        blurSinceDrain = true;
        // Nothing held when focus left will see its release, so release it all now, as SDL does.
        modifiers = 0;
        chordKeyPending = false;
        chordKeyUsed = false;
        Arrays.fill(chordedKeys, 0);
        for (int scancode = 1; scancode < keysDown.length; scancode++) {
            if (keysDown[scancode]) {
                keysDown[scancode] = false;
                add(Event.key(scancode, heldKeyCodes[scancode], 0, KeyAction.RELEASE));
            }
        }
        for (int button = 1; button <= 30; button++) {
            if ((buttonsDown & (1 << button)) != 0) {
                add(Event.button(button, false, 0));
            }
        }
        buttonsDown = 0;
        add(Event.window(Kind.FOCUS, false));
    }

    private void lockChanged(boolean locked) {
        if (!locked && exitAsked) {
            // The end the queue asked for, not the player's. If the game wants the mouse again by
            // now, ask again: a lock the page let go of comes back without a click (and a click
            // takes it back if it doesn't).
            exitAsked = false;
            pointerLocked = false;
            if (relativeWanted) {
                surface.setPointerLock(true);
            }
            return;
        }
        if (pointerLocked && !locked && relativeWanted) {
            lockLostByPlayer = true;
        }
        pointerLocked = locked;
        if (locked && !relativeWanted) {
            // The game let go of the mouse while the lock was still being granted.
            exitLock();
        }
    }

    private void exitLock() {
        exitAsked = true;
        surface.setPointerLock(false);
    }

    /** Relative mouse mode for the game: Pointer Lock, requested or left as the game switches. */
    void setRelativeMouseMode(boolean enabled) {
        if (enabled == relativeWanted) {
            // Already asked for. Asking the browser again without a click would only fail.
            return;
        }
        relativeWanted = enabled;
        lockLostByPlayer = false;
        if (enabled == pointerLocked) {
            // Nothing to ask for. (Or the mouse is wanted back while the lock the queue let go
            // of is still ending: lockChanged asks again when it has.)
            return;
        }
        if (enabled) {
            surface.setPointerLock(true);
        } else {
            exitLock();
        }
    }

    void setTextInput(boolean enabled) {
        textInput = enabled;
        if (!enabled) {
            // The page ends the composition with the text input (Dom.setTextInput); whatever
            // it commits then has no field to go to.
            composing = false;
        }
    }

    // --- Canvas size and fullscreen --------------------------------------------------------------

    /**
     * Makes the next poll report the canvas's size even if it didn't change. The game asked for a
     * window size, and a page can't give it one: this tells it the size it has instead.
     */
    void forceSizeReport() {
        sizeReportForced = true;
    }

    /** Records a fullscreen request the page hasn't answered yet (it answers asynchronously). */
    void fullscreenRequested(boolean enter) {
        fullscreenTarget = enter;
    }

    /** The fullscreen state last asked for, while the page hasn't confirmed it; else null. */
    @Nullable
    Boolean pendingFullscreen() {
        return fullscreenTarget;
    }

    CanvasSize size() {
        return applied;
    }

    private void reportSize(PlatformEventListener listener) {
        CanvasSize now = latest;
        CanvasSize before = applied;
        boolean forced = sizeReportForced;
        sizeReportForced = false;
        if (!forced && now.equals(before)) {
            return;
        }
        applied = now;
        // Resized here, as the game starts its frame, not when the page measured: resizing the
        // backing store clears it, which would blank a frame already drawn.
        surface.setBackingSize(now.pixelWidth(), now.pixelHeight());
        boolean pixelsChanged = now.pixelWidth() != before.pixelWidth() || now.pixelHeight() != before.pixelHeight();
        if (forced || now.width() != before.width() || now.height() != before.height()) {
            listener.windowResized(now.width(), now.height());
        }
        if (forced || pixelsChanged) {
            listener.framebufferResized(now.pixelWidth(), now.pixelHeight());
        } else if (now.density() != before.density()) {
            listener.displayChanged();
        }
    }

    // --- Delivery ----------------------------------------------------------------------------

    private void add(Event event) {
        if (pending.size() >= MAX_PENDING && event.isInput() && !event.isRelease()) {
            return;
        }
        pending.add(event);
    }

    void drainTo(PlatformEventListener listener) {
        // The size first: the game applies window events at once and runs input later, in its
        // task order, so input is handled against the new size either way.
        reportSize(listener);
        if (lockLostByPlayer) {
            lockLostByPlayer = false;
            // Escape leaves Pointer Lock, and browsers keep that keypress from the page. The game
            // would go on thinking it has the mouse; give it the Escape it would have seen, which
            // pauses (or closes whatever it had open). Not when a real Escape arrived, or when
            // focus left, which releases the mouse through the game's own pause-on-focus-loss.
            if (!escapeSinceDrain && !blurSinceDrain && focused && relativeWanted) {
                add(Event.key(Scancodes.ESCAPE, ESCAPE_KEYCODE, modifiers, KeyAction.PRESS));
                add(Event.key(Scancodes.ESCAPE, ESCAPE_KEYCODE, modifiers, KeyAction.RELEASE));
            }
        }
        escapeSinceDrain = false;
        blurSinceDrain = false;
        Event event;
        while ((event = pending.poll()) != null) {
            event.deliverTo(listener);
        }
    }

    /**
     * Drops queued input after a stall, as SDL's flush does. Releases stay, so nothing the game
     * saw pressed stays held; window events (focus, fullscreen) stay because they aren't input.
     */
    void flushInput() {
        for (int remaining = pending.size(); remaining > 0; remaining--) {
            Event event = pending.poll();
            if (event != null && (!event.isInput() || event.isRelease())) {
                pending.add(event);
            }
        }
    }

    boolean isKeyDown(int scancode) {
        return scancode > 0 && scancode < keysDown.length && keysDown[scancode];
    }

    int modifiers() {
        return modifiers;
    }

    /** The last cursor position in window coordinates; it may be outside the canvas. */
    double cursorX() {
        return cursorX;
    }

    double cursorY() {
        return cursorY;
    }

    /**
     * The text of the last paste event, or of the game's last copy if that came after: the
     * clipboard as far as a page can read it.
     */
    String lastPaste() {
        return lastPaste;
    }

    /**
     * The game put {@code text} on the clipboard. Reading it back gives it, as a system clipboard
     * would, even where the browser refused the write (no permission, no secure context).
     */
    void copied(String text) {
        lastPaste = text;
    }

    boolean isPointerLocked() {
        return pointerLocked;
    }

    // --- Queued events -------------------------------------------------------------------------

    private enum Kind {
        KEY,
        TEXT,
        MOVE,
        BUTTON,
        WHEEL,
        FOCUS,
        MINIMIZED,
        FULLSCREEN,
        ENTERED_OR_LEFT,
        EDITING,
        KEYMAP,
        FILES
    }

    /** One queued event. Mutable so consecutive moves and wheel turns can merge into one. */
    private static final class Event {
        final Kind kind;
        int code;
        int keyCode;
        int modifiers;
        KeyAction action = KeyAction.PRESS;
        boolean flag;
        double x;
        double y;
        double dx;
        double dy;
        String text = "";
        List<String> paths = List.of();

        private Event(Kind kind) {
            this.kind = kind;
        }

        static Event key(int scancode, int keyCode, int modifiers, KeyAction action) {
            Event event = new Event(Kind.KEY);
            event.code = scancode;
            event.keyCode = keyCode;
            event.modifiers = modifiers;
            event.action = action;
            return event;
        }

        static Event text(String text) {
            Event event = new Event(Kind.TEXT);
            event.text = text;
            return event;
        }

        /** An input method's composition: {@code code} is the cursor, {@code keyCode} the selection length. */
        static Event editing(String text, int cursor, int selection) {
            Event event = new Event(Kind.EDITING);
            event.text = text;
            event.code = cursor;
            event.keyCode = selection;
            return event;
        }

        static Event files(List<String> paths) {
            Event event = new Event(Kind.FILES);
            event.paths = paths;
            return event;
        }

        static Event move(double x, double y, double dx, double dy) {
            Event event = new Event(Kind.MOVE);
            event.x = x;
            event.y = y;
            event.dx = dx;
            event.dy = dy;
            return event;
        }

        static Event button(int button, boolean pressed, int modifiers) {
            Event event = new Event(Kind.BUTTON);
            event.code = button;
            event.flag = pressed;
            event.modifiers = modifiers;
            return event;
        }

        static Event wheel(double dx, double dy, int modifiers) {
            Event event = new Event(Kind.WHEEL);
            event.dx = dx;
            event.dy = dy;
            event.modifiers = modifiers;
            return event;
        }

        static Event window(Kind kind, boolean flag) {
            Event event = new Event(kind);
            event.flag = flag;
            return event;
        }

        boolean isInput() {
            return switch (kind) {
                case KEY, TEXT, EDITING, MOVE, BUTTON, WHEEL -> true;
                // A drop is kept through a flush: the player chose those files, whenever the
                // game gets to them.
                case FOCUS, MINIMIZED, FULLSCREEN, ENTERED_OR_LEFT, KEYMAP, FILES -> false;
            };
        }

        boolean isRelease() {
            return (kind == Kind.KEY && action == KeyAction.RELEASE) || (kind == Kind.BUTTON && !flag);
        }

        void deliverTo(PlatformEventListener listener) {
            switch (kind) {
                case KEY -> listener.key(code, keyCode, modifiers, action);
                case TEXT -> listener.textInput(text);
                case MOVE -> listener.mouseMoved(x, y, dx, dy);
                case BUTTON -> listener.mouseButton(code, flag, modifiers);
                case WHEEL -> listener.mouseWheel(dx, dy, modifiers);
                case FOCUS -> listener.focusChanged(flag);
                case MINIMIZED -> listener.minimizedChanged(flag);
                case FULLSCREEN -> listener.fullscreenChanged(flag);
                case ENTERED_OR_LEFT -> listener.cursorEnteredOrLeft();
                case EDITING -> listener.textEditing(text, code, keyCode);
                case KEYMAP -> listener.keymapChanged();
                case FILES -> listener.filesDropped(paths);
            }
        }
    }
}
