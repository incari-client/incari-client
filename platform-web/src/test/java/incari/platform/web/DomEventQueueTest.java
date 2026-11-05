package incari.platform.web;

import static incari.platform.web.DomEventQueue.DOM_ALT;
import static incari.platform.web.DomEventQueue.DOM_ALT_GRAPH;
import static incari.platform.web.DomEventQueue.DOM_CAPS_LOCK;
import static incari.platform.web.DomEventQueue.DOM_CTRL;
import static incari.platform.web.DomEventQueue.DOM_META;
import static incari.platform.web.DomEventQueue.DOM_SHIFT;
import static org.assertj.core.api.Assertions.assertThat;

import incari.platform.api.input.KeyAction;
import incari.platform.api.input.KeyModifiers;
import incari.platform.api.input.Scancodes;
import incari.platform.api.window.PlatformEventListener;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import org.junit.jupiter.api.Test;

class DomEventQueueTest {

    private final FakeSurface surface = new FakeSurface();
    private final DomEventQueue queue =
            new DomEventQueue(surface, new CanvasSize(800, 600, 1600, 1200, 2), false, true);

    // --- Keys ---

    @Test
    void keyPressThenRelease() {
        keyDown("KeyW", "w", 0);
        keyUp("KeyW", "w", 0);
        assertThat(drain()).containsExactly("key 26 119 0 PRESS", "key 26 119 0 RELEASE");
    }

    // --- F+digit function keys (Chromebooks) ---

    @Test
    void withoutChordsFIsReportedAtOnce() {
        DomEventQueue plain = new DomEventQueue(surface, new CanvasSize(800, 600, 800, 600, 1), false, false);
        plain.event("keydown", "KeyF", "f", 0, 0, 0, 0, 0, 0);
        assertThat(drain(plain)).containsExactly("key 9 102 0 PRESS");
        plain.event("keydown", "Digit3", "3", 0, 0, 0, 0, 0, 0);
        plain.event("keyup", "Digit3", "3", 0, 0, 0, 0, 0, 0);
        plain.event("keyup", "KeyF", "f", 0, 0, 0, 0, 0, 0);
        assertThat(drain(plain)).containsExactly("key 32 51 0 PRESS", "key 32 51 0 RELEASE", "key 9 102 0 RELEASE");
    }

    // --- Lost graphics ---

    @Test
    void lostGraphicsKeepTheWindowMinimized() {
        queue.event("graphics", "", "", 1, 0, 0, 0, 0, 0);
        queue.event("visibility", "", "", 1, 0, 0, 0, 0, 0);
        queue.event("visibility", "", "", 0, 0, 0, 0, 0, 0);
        assertThat(drain())
                .filteredOn(e -> e.startsWith("minimized"))
                .containsExactly("minimized true", "minimized true", "minimized true");
    }

    @Test
    void fThenDigitIsAFunctionKey() {
        keyDown("KeyF", "f", 0);
        keyDown("Digit3", "3", 0);
        keyUp("Digit3", "3", 0);
        keyUp("KeyF", "f", 0);
        assertThat(drain()).containsExactly("key 60 1073741884 0 PRESS", "key 60 1073741884 0 RELEASE");
    }

    @Test
    void fPlusDigitPlusAnotherKeyKeepsTheFunctionKeyHeld() {
        keyDown("KeyF", "f", 0);
        keyDown("Digit3", "3", 0);
        assertThat(queue.isKeyDown(Scancodes.F1 + 2)).isTrue();
        keyDown("KeyH", "h", 0);
        keyUp("KeyH", "h", 0);
        keyUp("Digit3", "3", 0);
        assertThat(queue.isKeyDown(Scancodes.F1 + 2)).isFalse();
        assertThat(drain()).hasSize(4).first().isEqualTo("key 60 1073741884 0 PRESS");
    }

    @Test
    void zeroIsF10() {
        keyDown("KeyF", "f", 0);
        keyDown("Digit0", "0", 0);
        assertThat(drain()).containsExactly("key 67 1073741891 0 PRESS");
    }

    @Test
    void fAloneIsReportedWhenReleased() {
        keyDown("KeyF", "f", 0);
        assertThat(drain()).isEmpty();
        keyUp("KeyF", "f", 0);
        assertThat(drain()).containsExactly("key 9 102 0 PRESS", "key 9 102 0 RELEASE");
    }

    @Test
    void fIsReportedFirstWhenAnotherKeyFollows() {
        keyDown("KeyF", "f", 0);
        keyDown("KeyW", "w", 0);
        assertThat(drain()).containsExactly("key 9 102 0 PRESS", "key 26 119 0 PRESS");
    }

    @Test
    void digitsAloneAreDigits() {
        keyDown("Digit3", "3", 0);
        assertThat(drain()).containsExactly("key 32 51 0 PRESS");
    }

    @Test
    void noChordWhileTyping() {
        queue.setTextInput(true);
        keyDown("KeyF", "f", 0);
        keyDown("Digit3", "3", 0);
        assertThat(drain()).contains("key 9 102 0 PRESS", "key 32 51 0 PRESS");
    }

    @Test
    void focusLossReleasesAChordedKey() {
        keyDown("KeyF", "f", 0);
        keyDown("Digit5", "5", 0);
        queue.event("blur", "", "", 0, 0, 0, 0, 0, 0);
        assertThat(drain()).contains("key 62 1073741886 0 PRESS", "key 62 1073741886 0 RELEASE");
    }

    @Test
    void repeatsAreReportedAsRepeats() {
        keyDown("KeyW", "w", 0);
        queue.event("keydown", "KeyW", "w", 1, 0, 0, 0, 0, 0);
        assertThat(drain()).containsExactly("key 26 119 0 PRESS", "key 26 119 0 REPEAT");
    }

    @Test
    void releasesWithoutAPressAreDropped() {
        keyUp("KeyW", "w", 0);
        assertThat(drain()).isEmpty();
    }

    @Test
    void keysKnownNeitherByPlaceNorMeaningAreDropped() {
        // The game's unbound binds all sit on the unknown key: one such press would fire them all.
        keyDown("", "Unidentified", 0);
        keyUp("", "Unidentified", 0);
        assertThat(drain()).isEmpty();
    }

    @Test
    void codelessKeysWithAMeaningStay() {
        // A virtual keyboard's Backspace comes without a code; text fields go by its keycode.
        keyDown("", "Backspace", 0);
        assertThat(drain()).containsExactly("key 0 8 0 PRESS");
    }

    @Test
    void keyStateFollowsTheEvents() {
        keyDown("KeyW", "w", 0);
        assertThat(queue.isKeyDown(Scancodes.W)).isTrue();
        keyUp("KeyW", "w", 0);
        assertThat(queue.isKeyDown(Scancodes.W)).isFalse();
        assertThat(queue.isKeyDown(-1)).isFalse();
        assertThat(queue.isKeyDown(Scancodes.COUNT)).isFalse();
    }

    @Test
    void textArrivesOnlyWhileTextInputIsOn() {
        keyDown("KeyT", "t", 0);
        queue.setTextInput(true);
        keyDown("KeyH", "h", 0);
        assertThat(drain()).containsExactly("key 23 116 0 PRESS", "key 11 104 0 PRESS", "text h");
    }

    @Test
    void shortcutsTypeNothingButAltGrDoes() {
        queue.setTextInput(true);
        keyDown("ControlLeft", "Control", DOM_CTRL);
        keyDown("KeyC", "c", DOM_CTRL);
        // AltGr+Q on a German keyboard in Windows: Ctrl and Alt both held, and "@" typed.
        keyDown("KeyQ", "@", DOM_CTRL | DOM_ALT | DOM_ALT_GRAPH);
        keyDown("Enter", "Enter", 0);
        assertThat(drain()).filteredOn(e -> e.startsWith("text")).containsExactly("text @");
    }

    @Test
    void composingKeysBelongToTheInputMethod() {
        queue.setTextInput(true);
        boolean cancelled = queue.event("keydown", "KeyA", "Process", 0, 1, 0, 0, 0, 0);
        assertThat(cancelled).isFalse();
        assertThat(drain()).isEmpty();
        assertThat(queue.isKeyDown(Scancodes.A)).isFalse();
    }

    @Test
    void modifiersSayWhichSide() {
        keyDown("ShiftRight", "Shift", DOM_SHIFT);
        keyDown("KeyA", "A", DOM_SHIFT | DOM_CAPS_LOCK);
        assertThat(queue.modifiers()).isEqualTo(KeyModifiers.RIGHT_SHIFT | KeyModifiers.CAPS_LOCK);
        keyDown("ShiftLeft", "Shift", DOM_SHIFT);
        assertThat(queue.modifiers()).isEqualTo(KeyModifiers.LEFT_SHIFT | KeyModifiers.RIGHT_SHIFT);
        assertThat(drain())
                .containsExactly("key 229 1073742053 2 PRESS", "key 4 97 8194 PRESS", "key 225 1073742049 3 PRESS");
    }

    @Test
    void modifiersOfAnUnseenKeyAreTheLeftOnes() {
        keyDown("KeyA", "a", DOM_CTRL | DOM_ALT | DOM_META);
        assertThat(queue.modifiers())
                .isEqualTo(KeyModifiers.LEFT_CTRL | KeyModifiers.LEFT_ALT | KeyModifiers.LEFT_SUPER);
    }

    @Test
    void browserShortcutsTheGameCantUseKeepTheirDefault() {
        assertThat(DomEventQueue.consumesKey("Tab", "", 0)).isTrue();
        assertThat(DomEventQueue.consumesKey("F5", "", 0)).isTrue();
        assertThat(DomEventQueue.consumesKey("F11", "", 0)).isTrue();
        assertThat(DomEventQueue.consumesKey("KeyV", "", 0)).isTrue();
        assertThat(DomEventQueue.consumesKey("F12", "", 0)).isFalse();
        assertThat(DomEventQueue.consumesKey("KeyV", "", DOM_CTRL)).isFalse();
        assertThat(DomEventQueue.consumesKey("KeyV", "", DOM_META)).isFalse();
        assertThat(DomEventQueue.consumesKey("KeyI", "", DOM_CTRL | DOM_SHIFT)).isFalse();
        assertThat(DomEventQueue.consumesKey("KeyI", "", DOM_CTRL)).isTrue();
    }

    @Test
    void deadKeysKeepTheirDefaultForTheComposition() {
        assertThat(DomEventQueue.consumesKey("BracketLeft", "Dead", 0)).isFalse();
        assertThat(DomEventQueue.consumesKey("BracketLeft", "ü", 0)).isTrue();
        // The game still sees the key, as SDL reports a dead key's press.
        assertThat(keyDown("BracketLeft", "Dead", 0)).isFalse();
        assertThat(drain()).containsExactly("key 47 " + (47 | DomKeys.NON_PRINTABLE) + " 0 PRESS");
    }

    @Test
    void fullscreenLocksAreKeptForTheGame() {
        // Keyboard Lock sends these to the page in fullscreen; the page must cancel them too.
        for (String code : List.of("KeyW", "KeyT", "KeyN", "KeyQ", "Digit1", "Escape")) {
            assertThat(DomEventQueue.consumesKey(code, "", DOM_CTRL)).as(code).isTrue();
        }
    }

    @Test
    void copiedTextReadsBack() {
        queue.copied("copied");
        assertThat(queue.lastPaste()).isEqualTo("copied");
        queue.event("paste", "pasted later", "", 0, 0, 0, 0, 0, 0);
        assertThat(queue.lastPaste()).isEqualTo("pasted later");
    }

    // --- Input methods ---

    @Test
    void aCompositionIsShownThenCommitted() {
        queue.setTextInput(true);
        queue.event("compose", "に", "", 1, 0, 0, 0, 0, 0);
        queue.event("compose", "にほ", "", 2, 0, 0, 0, 0, 0);
        queue.event("commit", "日本", "", 0, 0, 0, 0, 0, 0);
        assertThat(drain()).containsExactly("editing 'に' 1 0", "editing 'にほ' 2 0", "editing '' 0 0", "text 日本");
    }

    @Test
    void aCancelledCompositionIsCleared() {
        queue.setTextInput(true);
        queue.event("compose", "´", "", 1, 0, 0, 0, 0, 0);
        queue.event("compose", "", "", 0, 0, 0, 0, 0, 0);
        queue.event("compose", "", "", 0, 0, 0, 0, 0, 0);
        assertThat(drain()).containsExactly("editing '´' 1 0", "editing '' 0 0");
    }

    @Test
    void compositionCursorsStayInsideTheText() {
        queue.setTextInput(true);
        queue.event("compose", "ab", "", 5, 9, 0, 0, 0, 0);
        queue.event("compose", "ab", "", 0, 9, 0, 0, 0, 0);
        assertThat(drain()).containsExactly("editing 'ab' 2 0", "editing 'ab' 0 2");
    }

    @Test
    void textWithoutKeysIsTyped() {
        // A touchscreen keyboard or voice input: text with no key event the page could cancel.
        queue.setTextInput(true);
        queue.event("commit", "hi\n\tthere", "", 0, 0, 0, 0, 0, 0);
        assertThat(drain()).containsExactly("text hithere");
    }

    @Test
    void compositionsOutsideTextInputAreIgnored() {
        queue.event("compose", "a", "", 1, 0, 0, 0, 0, 0);
        queue.event("commit", "a", "", 0, 0, 0, 0, 0, 0);
        assertThat(drain()).isEmpty();
    }

    @Test
    void stoppingTextInputEndsTheComposition() {
        queue.setTextInput(true);
        queue.event("compose", "a", "", 1, 0, 0, 0, 0, 0);
        queue.setTextInput(false);
        queue.event("commit", "a", "", 0, 0, 0, 0, 0, 0);
        assertThat(drain()).containsExactly("editing 'a' 1 0");
    }

    // --- Key names ---

    @Test
    void theKeyboardMapNamesKeys() {
        // AZERTY: the key right of Tab types "a", the next "z".
        queue.event("keymap", "Digit1 &\nKeyQ a\nKeyW z\nSemicolon m\nSpace  \nNumpad1 1", "", 0, 0, 0, 0, 0, 0);
        assertThat(drain()).containsExactly("keymap");
        assertThat(queue.keyName(Scancodes.Q)).isEqualTo("a");
        assertThat(queue.keyName(Scancodes.W)).isEqualTo("z");
        assertThat(queue.keyName(51)).isEqualTo("m");
        assertThat(queue.keyName(Scancodes.DIGIT_1)).isEqualTo("&");
        // Keys whose names don't depend on the layout are the game's to name.
        assertThat(queue.keyName(Scancodes.SPACE)).isNull();
        assertThat(queue.keyName(89)).isNull();
        assertThat(queue.keyName(Scancodes.F1)).isNull();
        assertThat(queue.keyName(0)).isNull();
        assertThat(queue.keyName(Scancodes.COUNT)).isNull();
    }

    @Test
    void anUnchangedKeyboardMapIsntAnnounced() {
        queue.event("keymap", "KeyQ a", "", 0, 0, 0, 0, 0, 0);
        drain();
        queue.event("keymap", "KeyQ a", "", 0, 0, 0, 0, 0, 0);
        assertThat(drain()).isEmpty();
        queue.event("keymap", "KeyQ q", "", 0, 0, 0, 0, 0, 0);
        assertThat(drain()).containsExactly("keymap");
    }

    @Test
    void theKeyboardMapGivesShiftedKeysTheirKeycode() {
        // German: the key right of 0 types "ß", and "?" with Shift.
        queue.event("keymap", "Minus ß", "", 0, 0, 0, 0, 0, 0);
        drain();
        keyDown("Minus", "?", DOM_SHIFT);
        assertThat(drain()).containsExactly("key 45 223 " + KeyModifiers.LEFT_SHIFT + " PRESS");
    }

    @Test
    void withoutAKeyboardMapNamesAreLearnedFromTyping() {
        keyDown("KeyQ", "a", 0);
        keyDown("KeyW", "Z", DOM_SHIFT);
        keyDown("KeyE", "E", DOM_CAPS_LOCK);
        keyDown("Space", " ", 0);
        keyDown("Numpad1", "1", DOM_CAPS_LOCK);
        assertThat(queue.keyName(Scancodes.Q)).isEqualTo("a");
        // Shift changes what a key types, not its name: the key keeps its US name until it types
        // unshifted.
        assertThat(queue.keyName(Scancodes.W)).isEqualTo("w");
        assertThat(queue.keyName(8)).isEqualTo("e");
        assertThat(queue.keyName(Scancodes.SPACE)).isNull();
        assertThat(queue.keyName(89)).isNull();
        assertThat(drain()).doesNotContain("keymap");
        // A key that types something new: the player switched layouts.
        keyDown("KeyQ", "q", 0);
        assertThat(drain()).containsExactly("keymap", "key 20 113 0 PRESS");
    }

    @Test
    void theKeyboardMapWinsOverTyping() {
        queue.event("keymap", "KeyQ a", "", 0, 0, 0, 0, 0, 0);
        keyDown("KeyQ", "q", 0);
        assertThat(queue.keyName(Scancodes.Q)).isEqualTo("a");
        assertThat(drain()).containsExactly("keymap", "key 20 113 0 PRESS");
    }

    // --- Files ---

    @Test
    void droppedFilesReachTheGame() {
        queue.event("drop", "/incari/dropped/1/pack.zip\n/incari/dropped/1/Folder", "", 0, 0, 0, 0, 0, 0);
        queue.event("drop", "", "", 0, 0, 0, 0, 0, 0);
        assertThat(drain()).containsExactly("files [/incari/dropped/1/pack.zip, /incari/dropped/1/Folder]");
    }

    @Test
    void aDropSurvivesAFlush() {
        queue.event("drop", "/incari/dropped/1/pack.zip", "", 0, 0, 0, 0, 0, 0);
        keyDown("KeyW", "w", 0);
        queue.flushInput();
        assertThat(drain()).containsExactly("files [/incari/dropped/1/pack.zip]");
    }

    // --- Visibility ---

    @Test
    void hidingThePageReleasesEverythingAndLosesFocus() {
        keyDown("KeyW", "w", 0);
        queue.event("visibility", "", "", 1, 0, 0, 0, 0, 0);
        assertThat(drain())
                .containsExactly("key 26 119 0 PRESS", "key 26 119 0 RELEASE", "focus false", "minimized true");
        assertThat(queue.isKeyDown(Scancodes.W)).isFalse();
    }

    @Test
    void hidingAfterBlurDoesntBlurTwice() {
        queue.event("blur", "", "", 0, 0, 0, 0, 0, 0);
        queue.event("visibility", "", "", 1, 0, 0, 0, 0, 0);
        queue.event("visibility", "", "", 0, 0, 0, 0, 0, 0);
        queue.event("focus", "", "", 0, 0, 0, 0, 0, 0);
        assertThat(drain()).containsExactly("focus false", "minimized true", "minimized false", "focus true");
    }

    @Test
    void pasteTextIsThereWhenTheGameHandlesCtrlV() {
        // The page sends the keydown, then (because it wasn't cancelled) the paste event; the
        // game reads the clipboard only when it handles the key, on its next poll.
        boolean cancelled = keyDown("KeyV", "v", DOM_CTRL);
        queue.event("paste", "pasted", "", 0, 0, 0, 0, 0, 0);
        assertThat(cancelled).isFalse();
        var seen = new ArrayList<String>();
        queue.drainTo(new PlatformEventListener() {
            @Override
            public void key(int scancode, int keyCode, int modifiers, KeyAction action) {
                seen.add(keyCode + " " + queue.lastPaste());
            }
        });
        assertThat(seen).containsExactly("118 pasted");
    }

    @Test
    void blurReleasesEverythingHeld() {
        keyDown("KeyW", "w", 0);
        queue.event("down", "", "", 0, 0, 0, 0, 0, 0);
        drain();
        queue.event("blur", "", "", 0, 0, 0, 0, 0, 0);
        assertThat(drain()).containsExactly("key 26 119 0 RELEASE", "button 1 false 0", "focus false");
        assertThat(queue.isKeyDown(Scancodes.W)).isFalse();
        // The real releases, if they come, are no longer news.
        keyUp("KeyW", "w", 0);
        queue.event("up", "", "", 0, 0, 0, 0, 0, 0);
        assertThat(drain()).isEmpty();
    }

    @Test
    void commandReleasesKeysOnMac() {
        DomEventQueue mac = new DomEventQueue(surface, new CanvasSize(800, 600, 800, 600, 1), true, true);
        mac.event("keydown", "MetaLeft", "Meta", 0, 0, 0, 0, 0, DOM_META);
        mac.event("keydown", "KeyC", "c", 0, 0, 0, 0, 0, DOM_META);
        mac.event("keyup", "MetaLeft", "Meta", 0, 0, 0, 0, 0, 0);
        assertThat(drain(mac))
                .containsExactly(
                        "key 227 1073742051 1024 PRESS",
                        "key 6 99 1024 PRESS",
                        "key 6 99 0 RELEASE",
                        "key 227 1073742051 0 RELEASE");
        assertThat(mac.isKeyDown(6)).isFalse();
    }

    // --- Mouse ---

    @Test
    void buttonsAreNumberedFromOne() {
        queue.event("down", "", "", 2, 0, 0, 0, 0, DOM_SHIFT);
        queue.event("up", "", "", 2, 0, 0, 0, 0, 0);
        assertThat(drain()).containsExactly("button 3 true 1", "button 3 false 0");
    }

    @Test
    void releasesOfPressesOutsideTheCanvasAreDropped() {
        boolean cancelled = queue.event("up", "", "", 0, 0, 0, 0, 0, 0);
        assertThat(cancelled).isFalse();
        assertThat(drain()).isEmpty();
    }

    @Test
    void releasesOfPressesOnTheCanvasAreCancelled() {
        // Cancelling the release stops the back button from leaving the page.
        queue.event("down", "", "", 3, 0, 0, 0, 0, 0);
        assertThat(queue.event("up", "", "", 3, 0, 0, 0, 0, 0)).isTrue();
        assertThat(drain()).containsExactly("button 4 true 0", "button 4 false 0");
    }

    @Test
    void movesBetweenFramesMerge() {
        move(10, 20, 1, 2);
        move(12, 25, 2, 5);
        queue.event("down", "", "", 0, 0, 0, 0, 0, 0);
        move(13, 25, 1, 0);
        assertThat(drain()).containsExactly("move 12.0 25.0 3.0 7.0", "button 1 true 0", "move 13.0 25.0 1.0 0.0");
        assertThat(queue.cursorX()).isEqualTo(13);
        assertThat(queue.cursorY()).isEqualTo(25);
    }

    @Test
    void pointerLockKeepsThePositionAndReportsMotion() {
        move(100, 50, 0, 0);
        queue.setRelativeMouseMode(true);
        assertThat(surface.lockRequests).containsExactly(true);
        // Until the lock is granted the cursor is free, and its motion mustn't turn the camera.
        move(110, 50, 10, 0);
        lock(true);
        move(100, 50, 7, -3);
        assertThat(drain()).containsExactly("move 110.0 50.0 7.0 -3.0");
        assertThat(queue.cursorX()).isEqualTo(110);
    }

    @Test
    void escapeOutOfPointerLockReachesTheGame() {
        queue.setRelativeMouseMode(true);
        lock(true);
        drain();
        // The browser takes the Escape that leaves the lock; the game gets one in its place.
        lock(false);
        assertThat(drain()).containsExactly("key 41 27 0 PRESS", "key 41 27 0 RELEASE");
    }

    @Test
    void aRealEscapeIsNotDoubled() {
        queue.setRelativeMouseMode(true);
        lock(true);
        keyDown("Escape", "Escape", 0);
        lock(false);
        assertThat(drain()).containsExactly("key 41 27 0 PRESS");
    }

    @Test
    void losingTheLockWithFocusMakesNoEscape() {
        queue.setRelativeMouseMode(true);
        lock(true);
        lock(false);
        queue.event("blur", "", "", 0, 0, 0, 0, 0, 0);
        assertThat(drain()).containsExactly("focus false");
    }

    @Test
    void releasingTheMouseMakesNoEscape() {
        queue.setRelativeMouseMode(true);
        lock(true);
        queue.setRelativeMouseMode(false);
        lock(false);
        assertThat(drain()).isEmpty();
        assertThat(surface.lockRequests).containsExactly(true, false);
    }

    @Test
    void wantingTheMouseBackBeforeTheLockEndsMakesNoEscape() {
        queue.setRelativeMouseMode(true);
        lock(true);
        drain();
        // A death screen opens and a respawn closes it before the page has left the lock.
        queue.setRelativeMouseMode(false);
        queue.setRelativeMouseMode(true);
        lock(false);
        assertThat(drain()).isEmpty();
        assertThat(surface.lockRequests).containsExactly(true, false, true);
        lock(true);
        // The player's own Escape afterwards still reaches the game.
        lock(false);
        assertThat(drain()).containsExactly("key 41 27 0 PRESS", "key 41 27 0 RELEASE");
    }

    @Test
    void aClickTakesTheLockBack() {
        queue.setRelativeMouseMode(true);
        lock(true);
        lock(false);
        drain();
        // Asking again without a click would fail, so the queue doesn't.
        queue.setRelativeMouseMode(true);
        assertThat(surface.lockRequests).containsExactly(true);
        // The click that takes it back is the lock's, not the game's.
        assertThat(queue.event("down", "", "", 0, 0, 0, 0, 0, 0)).isTrue();
        queue.event("up", "", "", 0, 0, 0, 0, 0, 0);
        assertThat(surface.lockRequests).containsExactly(true, true);
        assertThat(drain()).isEmpty();
    }

    @Test
    void aLockGrantedAfterTheGameLetGoIsLeft() {
        queue.setRelativeMouseMode(true);
        queue.setRelativeMouseMode(false);
        lock(true);
        assertThat(surface.lockRequests).containsExactly(true, false);
    }

    // --- Wheel ---

    @Test
    void wheelNotchesAreOneEach() {
        // Chrome: 100 pixels a notch at 100% zoom, and wheelDelta 120.
        assertThat(DomEventQueue.wheelSteps(100, 0, 0)).isEqualTo(1);
        assertThat(DomEventQueue.wheelSteps(50, 0, 120)).isEqualTo(1);
        // Firefox: 3 lines a notch.
        assertThat(DomEventQueue.wheelSteps(3, 1, 0)).isEqualTo(1);
        assertThat(DomEventQueue.wheelSteps(1, 2, 0)).isEqualTo(1);
    }

    @Test
    void wheelPointsUpAndRight() {
        // A notch toward the player (DOM deltaY positive), then one right.
        queue.event("wheel", "", "", 0, 100, 0, 0, -120, 0);
        drain();
        queue.event("wheel", "", "", 3, 0, 1, 0, 0, 0);
        assertThat(drain()).containsExactly("wheel 1.0 0.0 0");
        queue.event("wheel", "", "", 0, -100, 0, 0, 120, 0);
        assertThat(drain()).containsExactly("wheel 0.0 1.0 0");
    }

    @Test
    void wheelTurnsBetweenFramesAddUp() {
        queue.event("wheel", "", "", 0, -100, 0, 0, 120, 0);
        queue.event("wheel", "", "", 0, -100, 0, 0, 120, 0);
        assertThat(drain()).containsExactly("wheel 0.0 2.0 0");
    }

    @Test
    void shiftWheelStaysVertical() {
        // Chrome sends Shift+wheel-down as scrolling right.
        queue.event("wheel", "", "", 100, 0, 0, -120, 0, DOM_SHIFT);
        assertThat(drain()).containsExactly("wheel 0.0 -1.0 1");
    }

    // --- Canvas size ---

    @Test
    void resizesAreReportedOnTheNextPoll() {
        queue.event("resize", "", "", 1000.4, 500, 2001, 1000, 2, 0);
        assertThat(queue.size().width()).isEqualTo(800);
        assertThat(drain()).containsExactly("windowResized 1000 500", "framebufferResized 2001 1000");
        assertThat(queue.size()).isEqualTo(new CanvasSize(1000, 500, 2001, 1000, 2));
        assertThat(surface.backingSizes).containsExactly("1600x1200", "2001x1000");
    }

    @Test
    void manyResizesBetweenFramesAreOne() {
        queue.event("resize", "", "", 900, 600, 0, 0, 2, 0);
        queue.event("resize", "", "", 1000, 600, 0, 0, 2, 0);
        assertThat(drain()).containsExactly("windowResized 1000 600", "framebufferResized 2000 1200");
    }

    @Test
    void aDensityChangeResizesTheFramebufferOnly() {
        // Moving to a 1x display: same CSS size, half the pixels.
        queue.event("resize", "", "", 800, 600, 0, 0, 1, 0);
        assertThat(drain()).containsExactly("framebufferResized 800 600");
        assertThat(queue.size().density()).isEqualTo(1);
    }

    @Test
    void anUnchangedSizeIsntReported() {
        queue.event("resize", "", "", 800, 600, 1600, 1200, 2, 0);
        assertThat(drain()).isEmpty();
    }

    @Test
    void theSizeCanBeReportedOnRequest() {
        queue.forceSizeReport();
        assertThat(drain()).containsExactly("windowResized 800 600", "framebufferResized 1600 1200");
        assertThat(drain()).isEmpty();
    }

    @Test
    void anEmptyCanvasKeepsItsSize() {
        queue.event("resize", "", "", 0, 0, 0, 0, 2, 0);
        assertThat(drain()).isEmpty();
    }

    @Test
    void measurementsWithoutDevicePixelsKeepTheExactOnes() {
        CanvasSize exact = new CanvasSize(1001, 500, 2003, 1000, 2);
        assertThat(CanvasSize.measure(1001, 500, 0, 0, 2, exact)).isEqualTo(exact);
        assertThat(CanvasSize.measure(1002, 500, 0, 0, 2, exact)).isEqualTo(new CanvasSize(1002, 500, 2004, 1000, 2));
        assertThat(CanvasSize.initial(1280, 720, 1.25)).isEqualTo(new CanvasSize(1280, 720, 1600, 900, 1.25));
    }

    // --- Window events and flushing ---

    @Test
    void windowEventsAreDelivered() {
        queue.event("visibility", "", "", 1, 0, 0, 0, 0, 0);
        queue.event("focus", "", "", 0, 0, 0, 0, 0, 0);
        queue.event("fullscreen", "", "", 1, 0, 0, 0, 0, 0);
        queue.event("enter", "", "", 0, 0, 0, 0, 0, 0);
        assertThat(drain())
                .containsExactly("focus false", "minimized true", "focus true", "fullscreen true", "entered");
    }

    @Test
    void aFullscreenAnswerEndsTheRequest() {
        queue.fullscreenRequested(true);
        assertThat(queue.pendingFullscreen()).isTrue();
        queue.event("fullscreen", "", "", 0, 0, 0, 0, 0, 0);
        assertThat(queue.pendingFullscreen()).isNull();
    }

    @Test
    void flushingKeepsReleasesAndWindowEvents() {
        keyDown("KeyW", "w", 0);
        drain();
        move(1, 1, 1, 1);
        keyDown("KeyE", "e", 0);
        keyUp("KeyW", "w", 0);
        queue.event("focus", "", "", 0, 0, 0, 0, 0, 0);
        queue.flushInput();
        assertThat(drain()).containsExactly("key 26 119 0 RELEASE", "focus true");
    }

    @Test
    void theQueueIsBounded() {
        for (int i = 0; i < 2 * DomEventQueue.MAX_PENDING; i++) {
            queue.event("wheel", "", "", 0, 0, 0, 0, 0, 0);
            keyDown("KeyW", "w", 0);
            keyUp("KeyW", "w", 0);
        }
        List<String> delivered = drain();
        // Past the bound only releases get in, so every key the game saw pressed is released.
        assertThat(delivered.size()).isLessThanOrEqualTo(DomEventQueue.MAX_PENDING + 2 * DomEventQueue.MAX_PENDING);
        long presses = delivered.stream().filter(e -> e.endsWith("PRESS")).count();
        long releases = delivered.stream().filter(e -> e.endsWith("RELEASE")).count();
        assertThat(releases).isGreaterThanOrEqualTo(presses);
        assertThat(delivered.get(delivered.size() - 1)).isEqualTo("key 26 119 0 RELEASE");
    }

    // --- Helpers ---

    private boolean keyDown(String code, String key, int flags) {
        return queue.event("keydown", code, key, 0, 0, 0, 0, 0, flags);
    }

    private void keyUp(String code, String key, int flags) {
        queue.event("keyup", code, key, 0, 0, 0, 0, 0, flags);
    }

    private void move(double x, double y, double dx, double dy) {
        queue.event("move", "", "", x, y, dx, dy, 0, 0);
    }

    private void lock(boolean locked) {
        queue.event("lock", "", "", locked ? 1 : 0, 0, 0, 0, 0, 0);
    }

    private List<String> drain() {
        return drain(queue);
    }

    private static List<String> drain(DomEventQueue from) {
        Recorder recorder = new Recorder();
        from.drainTo(recorder);
        return recorder.events;
    }

    private static final class FakeSurface implements DomEventQueue.Surface {
        final List<String> backingSizes = new ArrayList<>();
        final List<Boolean> lockRequests = new ArrayList<>();

        @Override
        public void setBackingSize(int pixelWidth, int pixelHeight) {
            backingSizes.add(pixelWidth + "x" + pixelHeight);
        }

        @Override
        public void setPointerLock(boolean locked) {
            lockRequests.add(locked);
        }
    }

    /** Writes each event as a line, so tests compare what the game would see. */
    private static final class Recorder implements PlatformEventListener {
        final List<String> events = new ArrayList<>();

        @Override
        public void windowResized(int width, int height) {
            events.add("windowResized " + width + " " + height);
        }

        @Override
        public void framebufferResized(int width, int height) {
            events.add("framebufferResized " + width + " " + height);
        }

        @Override
        public void displayChanged() {
            events.add("displayChanged");
        }

        @Override
        public void minimizedChanged(boolean minimized) {
            events.add("minimized " + minimized);
        }

        @Override
        public void focusChanged(boolean focused) {
            events.add("focus " + focused);
        }

        @Override
        public void fullscreenChanged(boolean fullscreen) {
            events.add("fullscreen " + fullscreen);
        }

        @Override
        public void cursorEnteredOrLeft() {
            events.add("entered");
        }

        @Override
        public void key(int scancode, int keyCode, int modifiers, KeyAction action) {
            events.add("key " + scancode + " " + keyCode + " " + modifiers + " " + action);
        }

        @Override
        public void textInput(String text) {
            events.add("text " + text);
        }

        @Override
        public void textEditing(String text, int start, int length) {
            events.add("editing '" + text + "' " + start + " " + length);
        }

        @Override
        public void keymapChanged() {
            events.add("keymap");
        }

        @Override
        public void filesDropped(List<String> paths) {
            events.add("files " + paths);
        }

        @Override
        public void mouseMoved(double x, double y, double dx, double dy) {
            events.add(String.format(Locale.ROOT, "move %.1f %.1f %.1f %.1f", x, y, dx, dy));
        }

        @Override
        public void mouseButton(int button, boolean pressed, int modifiers) {
            events.add("button " + button + " " + pressed + " " + modifiers);
        }

        @Override
        public void mouseWheel(double dx, double dy, int modifiers) {
            events.add(String.format(Locale.ROOT, "wheel %.1f %.1f %d", dx, dy, modifiers));
        }
    }
}
