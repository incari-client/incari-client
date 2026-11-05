package incari.platform.web;

import org.teavm.interop.Async;
import org.teavm.interop.AsyncCallback;
import org.teavm.jso.JSBody;
import org.teavm.jso.JSFunctor;
import org.teavm.jso.JSObject;
import org.teavm.jso.typedarrays.Int8Array;

/**
 * Every JavaScript snippet the web platform calls (through TeaVM's interop), in one file so the
 * browser surface is easy to audit.
 *
 * <p>Runs on the page's only thread. Nothing here blocks except {@link #nextAnimationFrame}, which
 * suspends the calling coroutine and lets the page run.
 */
final class Dom {

    /** Frames the frame monitor keeps: about four and a half minutes at 60 frames a second. */
    static final int FRAME_ROWS = 16384;

    /** Numbers per frame in the frame monitor's table ({@link #installFrameMonitor}). */
    static final int FRAME_COLUMNS = 16;

    private Dom() {}

    // --- Time and frames -------------------------------------------------------------------

    @JSBody(script = "return performance.now();")
    static native double nowMillis();

    /** Suspends until the browser's next animation frame: where the page draws and handles input. */
    @Async
    static native Void nextAnimationFrame();

    private static void nextAnimationFrame(AsyncCallback<Void> callback) {
        requestAnimationFrame(() -> callback.complete(null));
    }

    @JSFunctor
    interface FrameCallback extends JSObject {
        void run();
    }

    /**
     * Calls back on the next animation frame, or after a quarter second if none comes: hidden tabs
     * get no animation frames, and the game would stop. A hidden page's own timers are held to a
     * wake-up a second, then a minute, too few to answer a server's keep-alives, so while hidden the
     * quarter second is timed by the page's worker timer ({@code window.incariTimer}, app-web's
     * {@code wakeups.ts}) where there is one.
     */
    @JSBody(
            params = "callback",
            script = "var done = false, cancel = null;"
                    + "function run() { if (done) return; done = true; if (cancel) cancel();"
                    + "  var frames = window.__incariFrames; if (frames) frames.begin();"
                    + "  callback(); if (frames) frames.returned(); }"
                    + "requestAnimationFrame(run); var fb = window.incariFallback, tm = fb ? 0 : setTimeout(run, 250);"
                    + "cancel = fb ? fb(run, 250) : function() { clearTimeout(tm); };")
    private static native void requestAnimationFrame(FrameCallback callback);

    /**
     * Installs the frame monitor, {@code window.__incariFrames}: how long each game frame takes
     * (animation frame to {@link #frameEnded}), and whether the render thread suspended mid-frame.
     * That's a blocking wait (a lock, a {@code join}, a {@code sleep}); it should only suspend
     * between frames (the threading audit, ADR 0015). The frame runs inside the animation frame
     * callback, so a callback that returns before the frame ends means the thread waited.
     *
     * <p>Every {@code wait} in the web build goes through web-classlib's {@code Waits}, which adds
     * the render thread's waits to its frame's {@code waitMillis}. A frame that waited 50 ms or more
     * in all counts in {@code waitStalls}, the threading audit's gate. Frames that waited, and
     * frames over 50 ms (waiting or not), are counted with the longest of each; checks read these.
     *
     * <p>It also keeps the last {@value #FRAME_ROWS} frames for the benchmark runner, as rows of
     * {@value #FRAME_COLUMNS} numbers:
     *
     * <ol>
     *   <li>when the frame ended, the time since the previous one ended, how long it took from its
     *       animation frame, and whether it waited;
     *   <li>the game's measurements ({@link #frameSample}): tick, extract, render and GPU
     *       milliseconds, draw calls, terrain draw calls and sections, the mesh queue, and the
     *       module's linear memory in megabytes;
     *   <li>the page's heap in megabytes;
     *   <li>the background pool's milliseconds since the previous frame ({@link IdleSlots}), and its
     *       latest run past a slot's deadline (for finding stutters).
     * </ol>
     *
     * <p>{@code rows(from)} returns the kept rows from frame {@code from} on, flat; {@code count} is
     * the next row's number. {@code marks} holds start-up's marks ({@link #mark}) in milliseconds
     * since navigation start. Heap drops over a megabyte between frames count as garbage collections
     * ({@code collections}), exact only with Chrome's {@code --enable-precise-memory-info}, which
     * the runner turns on.
     */
    @JSBody(
            script = "window.incariFallback = function(run, delay) { var t = window.incariTimer;"
                    + "  if (document.hidden && t) { var id = t.set(run, delay); return function() { t.clear(id); }; }"
                    + "  var timer = setTimeout(run, delay); return function() { clearTimeout(timer); }; };"
                    + "var f = { start: 0, ended: true, waited: false, frames: 0, waiting: 0, slow: 0, slowWaiting: 0,"
                    + " worst: 0, worstWaiting: 0, waitMillis: 0, waitStalls: 0, worstWait: 0, last: null,"
                    + " columns: " + FRAME_COLUMNS + ", capacity: " + FRAME_ROWS + ", count: 0, lastEnd: 0,"
                    + " sample: null, marks: {}, heap: 0, collections: 0, idle: 0, late: 0 };"
                    + "f.table = new Float64Array(f.capacity * f.columns);"
                    + "f.begin = function() { f.start = performance.now(); f.ended = false; f.waited = false; f.waitMillis = 0; };"
                    + "f.returned = function() { if (f.ended) return; f.waited = true;"
                    + "  var slots = window.__incariIdleSlots || []; for (var i = 0; i < slots.length; i++) slots[i].schedule(); };"
                    + "f.heapMegabytes = function() { var m = performance.memory; return m ? m.usedJSHeapSize / 1048576 : NaN; };"
                    + "f.record = function(now, took) { var heap = f.heapMegabytes();"
                    + "  if (heap < f.heap - 1) f.collections++; f.heap = heap;"
                    + "  var row = (f.count % f.capacity) * f.columns, t = f.table, s = f.sample;"
                    + "  t[row] = now; t[row + 1] = f.lastEnd ? now - f.lastEnd : NaN; t[row + 2] = took; t[row + 3] = f.waited ? 1 : 0;"
                    + "  for (var i = 0; i < 8; i++) t[row + 4 + i] = s ? s[i] : NaN;"
                    + "  t[row + 12] = heap; t[row + 13] = s ? s[8] : NaN; t[row + 14] = f.idle; t[row + 15] = f.late;"
                    + "  f.idle = 0; f.late = 0; f.sample = null; f.lastEnd = now; f.count++; };"
                    + "f.rows = function(from) { var first = Math.max(from, f.count - f.capacity, 0), out = [];"
                    + "  for (var n = first; n < f.count; n++) { var row = (n % f.capacity) * f.columns;"
                    + "    for (var i = 0; i < f.columns; i++) out.push(f.table[row + i]); } return out; };"
                    + "f.end = function() { f.ended = true; f.frames++; var now = performance.now();"
                    + "  if (!f.start) { f.record(now, NaN); return 0; }"
                    + "  f.record(now, now - f.start);"
                    + "  if (f.waited) f.waiting++;"
                    + "  if (f.waitMillis > f.worstWait) f.worstWait = f.waitMillis;"
                    + "  if (f.waitMillis >= 50) f.waitStalls++;"
                    + "  var took = now - f.start; f.start = 0; if (took <= 50) return 0;"
                    + "  f.slow++; f.worst = Math.max(f.worst, took);"
                    + "  if (f.waited) { f.slowWaiting++; f.worstWaiting = Math.max(f.worstWaiting, took); }"
                    + "  f.last = { took: took, waited: f.waited }; return f.waited ? -took : took; };"
                    + "window.__incariFrames = f;")
    static native void installFrameMonitor();

    /**
     * Marks the end of the game's frame, just before it waits for the next one. Returns 0 for a
     * frame of 50 ms or less, else its length, negative if the render thread waited in it.
     */
    @JSBody(script = "var f = window.__incariFrames; return f ? f.end() : 0;")
    static native double frameEnded();

    /** Marks an animation frame the game skips (a frame limit) as between frames, not part of one. */
    @JSBody(script = "var f = window.__incariFrames; if (f) { f.ended = true; f.start = 0; f.waited = false; }")
    static native void frameSkipped();

    /** Tells the host page's loader (app-web's {@code boot.ts}) that the game has drawn its first frame. */
    @JSBody(
            script =
                    "var loader = window.incariLoader; if (loader) loader.done();"
                            + "var f = window.__incariFrames; if (f && !f.marks.firstFrame) f.marks.firstFrame = performance.now();")
    static native void loaderDone();

    /**
     * The game's measurements of the frame about to end, for the frame monitor's table.
     * Numbers only: the monitor copies them into its row when the frame ends.
     */
    @JSBody(
            params = {
                "tick",
                "extract",
                "render",
                "gpu",
                "draws",
                "terrainDraws",
                "terrainSections",
                "meshQueue",
                "linear"
            },
            script = "var f = window.__incariFrames;"
                    + "if (f) f.sample = [tick, extract, render, gpu, draws, terrainDraws, terrainSections, meshQueue,"
                    + " linear / 1048576];")
    static native void frameSample(
            double tick,
            double extract,
            double render,
            double gpu,
            int draws,
            int terrainDraws,
            int terrainSections,
            int meshQueue,
            double linear);

    /** Records a start-up mark's time the first time it's reached ({@code marks}). */
    @JSBody(
            params = "name",
            script = "var f = window.__incariFrames; if (f && !(name in f.marks)) f.marks[name] = performance.now();")
    static native void mark(String name);

    /** The page's JavaScript heap in bytes, or -1 where the browser doesn't say (only Chrome does). */
    @JSBody(script = "var m = performance.memory; return m ? m.usedJSHeapSize : -1;")
    static native double heapUsed();

    /** The most the page's JavaScript heap may grow to, or -1 where the browser doesn't say. */
    @JSBody(script = "var m = performance.memory; return m ? m.jsHeapSizeLimit : -1;")
    static native double heapLimit();

    /** The size of the {@code ArrayBuffer} under {@code view}: for a direct buffer's view, the module's linear memory. */
    @JSBody(params = "view", script = "return view.buffer.byteLength;")
    static native double backingBytes(Int8Array view);

    /** Garbage collections the frame monitor has seen as drops in the heap. */
    @JSBody(script = "var f = window.__incariFrames; return f ? f.collections : -1;")
    static native double collections();

    /** Frames over 50 ms since the page started, and those of them in which the render thread waited. */
    @JSBody(
            script =
                    "var f = window.__incariFrames; return f ? f.waiting + ' waited (' + f.waitStalls + ' for 50 ms or more, longest '"
                            + " + Math.round(f.worstWait) + ' ms); ' + f.slow + ' over 50 ms (' + f.slowWaiting"
                            + " + ' waiting, longest ' + Math.round(f.worstWaiting) + ' ms), longest ' + Math.round(f.worst) + ' ms' : '';")
    static native String frameStats();

    /**
     * The background pool's slots ({@link IdleTimeExecutor}): after an animation frame, once the
     * game's frame (which runs inside its own animation frame callback) is done and the page has
     * drawn it, until shortly before the next frame is due.
     *
     * <p>The slot starts from a message posted in the animation frame callback, not the callback
     * itself: callbacks run before the page draws, so work there would hold the frame back. A slot
     * ends 2 ms before the next frame is due (the display's interval, measured) and lasts at least
     * 4 ms, so the pool moves even when the game's frames run long.
     *
     * <p>A render thread suspended mid-frame may be waiting for a task here (a {@code join}), so
     * once it has waited 8 ms, slots come without a frame, 4 ms at a time, until its frame ends: the
     * wait lasts about as long as the work, not until the next frame. Short waits (a sound handed to
     * the sound thread) end first and never start a task. In a hidden tab a slot comes after a
     * quarter second at the latest, as the game's frames do.
     */
    static final class IdleSlots implements IdleTimeExecutor.Slots {

        private final JSObject slot = createIdleSlot();

        @Override
        public void arm() {
            armIdleSlot(slot);
        }

        @Override
        public double await() {
            awaitIdleSlot(slot);
            return idleSlotDeadline(slot);
        }

        @Override
        public double now() {
            return nowMillis();
        }

        @Override
        public void worked(double millis, double pastDeadline) {
            poolWorked(millis, pastDeadline);
        }
    }

    /**
     * The least time a background-pool slot gets while the pool has work, in milliseconds (ADR 0035):
     * the time before the next frame, but never less than this.
     */
    @JSBody(params = "millis", script = "window.__incariPoolFloor = millis;")
    static native void setPoolFloor(double millis);

    /** The frame table's pool columns: the pool's work since the last frame, and its worst lateness. */
    @JSBody(
            params = {"millis", "late"},
            script = "var f = window.__incariFrames; if (f) { f.idle += millis; f.late = Math.max(f.late, late); }")
    private static native void poolWorked(double millis, double late);

    @JSBody(
            script =
                    "var s = { callback: null, armed: false, scheduled: false, urgentPosted: false, urgentFiring: false, helped: false,"
                            + " frame: 0,"
                            + " previous: 0, interval: 1000 / 60, deadline: 0 };"
                            // The render thread suspended mid-frame (the frame monitor saw it): it may be
                            // waiting for a task here, so the pool runs soon, a few milliseconds at a time.
                            + "function blocked() { var f = window.__incariFrames; return !!f && !f.ended && f.waited; }"
                            + "var channel = new MessageChannel();"
                            + "channel.port1.onmessage = function() { var urgent = s.urgentFiring; s.urgentFiring = false;"
                            // A slot that falls while the render thread is suspended mid-frame takes the
                            // delayed path below: the render thread may be about to resume, and a task
                            // started now would keep it waiting for as long as the task runs.
                            + "  if (!urgent && blocked()) { s.scheduled = false; s.schedule(); return; }"
                            + "  s.urgentPosted = false; var callback = s.callback;"
                            + "  if (!callback) return; s.callback = null; s.scheduled = false; s.armed = false;"
                            + "  var now = performance.now();"
                            // Until shortly before the next frame; a slot that starts late (after a long
                            // frame) still gets a moment, and its first task or part runs whatever the time.
                            // While work is queued, a slot is at least the pool's floor (ADR 0035): a frame
                            // that overran its interval would otherwise leave the pool 1 ms, and terrain
                            // after a teleport took minutes on a slow device.
                            + "  s.deadline = blocked() ? now + 4"
                            + "    : Math.max(s.frame + s.interval - 2, now + (window.__incariPoolFloor || 1)); callback(); };"
                            + "s.schedule = function() { if (!s.armed || !s.callback) return;"
                            // Not at once: most waits (a sound handed to the sound thread) end within a
                            // couple of timer turns, and a task started meanwhile would hold the render
                            // thread for as long as it runs. A timer, so a render thread just woken (by a
                            // timer too, queued earlier) always runs first.
                            + "  if (blocked()) { if (!s.urgentPosted) { s.urgentPosted = true; var first = !s.helped; s.helped = true;"
                            + "    setTimeout(function() { if (blocked()) { s.urgentFiring = true; channel.port2.postMessage(0); }"
                            + "      else { s.urgentPosted = false; s.schedule(); } },"
                            + "      first ? 8 : 1); } return; }"
                            + "  s.helped = false;"
                            + "  if (s.scheduled) return; s.scheduled = true;"
                            + "  var done = false, cancel = null;"
                            + "  function run(time) { if (done) return; done = true; if (cancel) cancel();"
                            + "    if (typeof time === 'number') {"
                            // The display's interval, from the steps between animation frames. Only a step
                            // near one interval counts: the pool asks for a slot only when it has work, so
                            // it often skips frames, and a two-frame step taken as the interval would let
                            // each slot run into the next frame's time (and the frames it delays then
                            // stretch the steps further). A faster display is learned from its shorter steps.
                            + "      var step = time - s.previous; if (s.previous && step > 4 && step < s.interval * 1.4)"
                            + "        s.interval += (step - s.interval) * 0.1;"
                            + "      s.previous = time; s.frame = time; } else { s.frame = performance.now(); }"
                            + "    channel.port2.postMessage(0); }"
                            + "  requestAnimationFrame(run); var fb = window.incariFallback, tm = fb ? 0 : setTimeout(run, 250);"
                            + "  cancel = fb ? fb(run, 250) : function() { clearTimeout(tm); }; };"
                            + "(window.__incariIdleSlots = window.__incariIdleSlots || []).push(s);"
                            + "return s;")
    private static native JSObject createIdleSlot();

    @JSBody(params = "slot", script = "slot.armed = true; slot.schedule();")
    private static native void armIdleSlot(JSObject slot);

    @JSBody(params = "slot", script = "return slot.deadline;")
    private static native double idleSlotDeadline(JSObject slot);

    /** Suspends the calling coroutine until the slot's next start (see {@link IdleSlots}). */
    @Async
    private static native Void awaitIdleSlot(JSObject slot);

    private static void awaitIdleSlot(JSObject slot, AsyncCallback<Void> callback) {
        setIdleSlotCallback(slot, () -> callback.complete(null));
    }

    @JSBody(
            params = {"slot", "callback"},
            script = "slot.callback = callback; slot.schedule();")
    private static native void setIdleSlotCallback(JSObject slot, FrameCallback callback);

    // --- Screen and canvas ------------------------------------------------------------------

    @JSBody(script = "return window.devicePixelRatio || 1;")
    static native double devicePixelRatio();

    @JSBody(script = "return screen.width;")
    static native int screenWidth();

    @JSBody(script = "return screen.height;")
    static native int screenHeight();

    /** The game's canvas: the page's element with this id, created full-window if it's missing. */
    @JSBody(
            params = "id",
            script = "var c = document.getElementById(id);"
                    + "if (!c) { c = document.createElement('canvas'); c.id = id;"
                    + " c.style.width = '100vw'; c.style.height = '100vh'; c.style.display = 'block';"
                    + " document.body.style.margin = '0'; document.body.appendChild(c); }"
                    + "c.tabIndex = 0; return c;")
    static native JSObject canvas(String id);

    /**
     * The canvas's CSS width, or the window's while the canvas isn't laid out yet (it's then
     * 0 wide). The page must size the canvas with CSS: its backing store is set to the CSS size
     * times the density, and a canvas without a CSS size would grow with it.
     */
    @JSBody(params = "canvas", script = "return canvas.clientWidth || window.innerWidth;")
    static native int cssWidth(JSObject canvas);

    @JSBody(params = "canvas", script = "return canvas.clientHeight || window.innerHeight;")
    static native int cssHeight(JSObject canvas);

    /** The single HTML file plays from the disk; otherwise the page's build says JSPI or not. */
    @JSBody(
            script = "if (location.protocol === 'file:') return 'html';"
                    + "var build = window.incariBuild && window.incariBuild.program;"
                    + "return build && build.jspi ? 'fast' : 'compatible';")
    static native String buildKind();

    @JSBody(
            params = {"canvas", "width", "height"},
            script = "if (canvas.width !== width) canvas.width = width;"
                    + "if (canvas.height !== height) canvas.height = height;")
    static native void setCanvasSize(JSObject canvas, int width, int height);

    @JSBody(
            params = {"canvas", "cursor"},
            script = "canvas.style.cursor = cursor;")
    static native void setCursor(JSObject canvas, String cursor);

    @JSBody(script = "return !!document.fullscreenElement;")
    static native boolean isFullscreen();

    /**
     * Whether a fullscreen request would be granted now: the page allows fullscreen and the player
     * just pressed a key or clicked. Browsers without the User Activation API are assumed to allow
     * it.
     */
    @JSBody(
            script = "return !!document.fullscreenEnabled"
                    + " && (!navigator.userActivation || navigator.userActivation.isActive);")
    static native boolean canEnterFullscreen();

    /**
     * Enters or leaves fullscreen. The answer comes as a "fullscreen" event: from the page's
     * fullscreenchange, or from here when the request is refused.
     */
    @JSBody(
            params = {"canvas", "enter"},
            script = "function refused() { var s = canvas.__incariSink;"
                    + "  if (s) s('fullscreen', '', '', document.fullscreenElement ? 1 : 0, 0, 0, 0, 0, 0); }"
                    + "try { var p = null;"
                    + "  if (enter) { if (canvas.requestFullscreen) p = canvas.requestFullscreen(); }"
                    + "  else if (document.fullscreenElement) { p = document.exitFullscreen(); }"
                    + "  if (p && p.catch) p.catch(refused); else if (!p) refused(); } catch (e) { refused(); }")
    static native void requestFullscreen(JSObject canvas, boolean enter);

    /**
     * Pointer Lock with unadjusted movement where offered: raw mouse, no OS acceleration. Browsers
     * that don't offer it refuse the option, and get a plain request.
     */
    @JSBody(
            params = {"canvas", "lock"},
            script =
                    "function ignore() {}"
                            + "try { if (lock) { var p = canvas.requestPointerLock({ unadjustedMovement: true });"
                            + "  if (p && p.catch) p.catch(function(err) { if (err && err.name === 'NotSupportedError') {"
                            + "    var q = canvas.requestPointerLock(); if (q && q.catch) q.catch(ignore); } }); }"
                            + " else if (document.pointerLockElement === canvas) { document.exitPointerLock(); } } catch (e) {}")
    static native void setPointerLock(JSObject canvas, boolean lock);

    /**
     * Whether the browser runs on macOS, where keys pressed while Command is held never report
     * their release.
     */
    @JSBody(
            script =
                    "var p = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || '';"
                            + "return /^mac/i.test(p);")
    static native boolean isMac();

    /**
     * Whether F then a digit types a function key ({@code DomEventQueue}): on ChromeOS, whose
     * keyboards have no function row the game can use, and nowhere else, where it would hold back
     * F (swap hands) until it's released. {@code ?fchord=on} or {@code off} in the page's address
     * overrides it.
     */
    @JSBody(
            script = "var q = null; try { q = new URLSearchParams(location.search).get('fchord'); } catch (e) {}"
                    + "if (q === 'on') return true; if (q === 'off') return false;"
                    + "var p = (navigator.userAgentData && navigator.userAgentData.platform) || '';"
                    + "return /^chrome ?os$/i.test(p) || /\\bCrOS\\b/.test(navigator.userAgent || '');")
    static native boolean functionKeyChords();

    // --- Shell -------------------------------------------------------------------------------

    @JSBody(params = "url", script = "window.open(url, '_blank', 'noopener');")
    static native void openUrl(String url);

    /**
     * Puts text on the clipboard without waiting: the async Clipboard API, or where it's missing
     * (pages served over plain http from another machine) or refuses, a copy command. Both need
     * the page focused and a recent key press or click, which the game's copy shortcuts give.
     */
    @JSBody(
            params = "text",
            script = "function viaCopyEvent() { function onCopy(e) { e.clipboardData.setData('text/plain', text);"
                    + "    e.preventDefault(); }"
                    + "  document.addEventListener('copy', onCopy, true);"
                    + "  try { document.execCommand('copy'); } catch (e) {}"
                    + "  document.removeEventListener('copy', onCopy, true); }"
                    + "if (navigator.clipboard && navigator.clipboard.writeText) {"
                    + "  navigator.clipboard.writeText(text).catch(viaCopyEvent); } else { viaCopyEvent(); }")
    static native void writeClipboard(String text);

    /**
     * Puts a PNG on the clipboard without waiting, through the async Clipboard API.
     *
     * @return false where the browser has no image clipboard (no {@code ClipboardItem})
     */
    @JSBody(
            params = "data",
            script = "if (!navigator.clipboard || !navigator.clipboard.write || typeof ClipboardItem === 'undefined') {"
                    + "  return false; }"
                    + "var blob = new Blob([data], {type: 'image/png'});"
                    + "navigator.clipboard.write([new ClipboardItem({'image/png': blob})]).catch(function(e) {"
                    + "  console.warn('[Incari] Couldn\\'t copy the image: ' + e); });"
                    + "return true;")
    static native boolean writeClipboardImage(Int8Array data);

    @JSBody(params = "message", script = "window.alert(message);")
    static native void alert(String message);

    @JSBody(params = "message", script = "return window.confirm(message);")
    static native boolean confirm(String message);

    @JSBody(params = "message", script = "console.log(message);")
    static native void consoleLog(String message);

    @JSBody(params = "message", script = "console.warn(message);")
    static native void consoleWarn(String message);

    @JSBody(params = "message", script = "console.error(message);")
    static native void consoleError(String message);

    @JSBody(script = "return navigator.userAgent;")
    static native String userAgent();

    // --- Events --------------------------------------------------------------------------------

    /**
     * Decides, while a key event is being dispatched, whether the page cancels its default action
     * ({@link DomEventQueue#consumesKey}). The one call from a DOM listener into Java: it must be a
     * pure function of its arguments. TeaVM compiles any Java that can reach a suspension point (in
     * this program, most collection and {@code Object} methods) as a coroutine, and a coroutine
     * can't start from a JavaScript callback, where there's no Java thread to suspend.
     */
    @JSFunctor
    interface KeyPolicy extends JSObject {
        boolean consumes(String code, String key, int flags);
    }

    /*
     * Events wait in a JavaScript array on the canvas, nine slots each, until the game's next poll
     * reads them on its own thread ({@link WebPlatform#pollEvents}): see KeyPolicy for why the
     * listeners don't call Java. The slots, which {@link DomEventQueue#event} reads:
     *   type, text, key, a, b, c, d, e, flags
     *   "keydown", "keyup": text {@code code}, key {@code key}, a repeat, b composing (1/0)
     *   "move": a, b the position in CSS pixels from the canvas's top left (may be outside it);
     *       c, d {@code movementX/Y}
     *   "down", "up": a the DOM button (0 left, 1 middle, 2 right, 3 back, 4 forward)
     *   "wheel": a, b {@code deltaX/Y}; c {@code deltaMode}; d, e the legacy {@code wheelDeltaX/Y},
     *       0 where missing
     *   "resize": a, b the CSS content size; c, d the device-pixel size, 0 where the browser can't
     *       tell; e {@code devicePixelRatio}
     *   "focus", "blur", "enter" (the cursor entered or left the page), "visibility" (a 1 if hidden),
     *   "fullscreen" (a 1 if fullscreen), "lock" (a 1 if the canvas holds Pointer Lock),
     *   "lockerror", "paste" (text the pasted text)
     *   "compose": text an input method's uncommitted composition ("" when it ends), a the cursor
     *       in it, b the length of the selection in it (listenTextInput)
     *   "commit": text committed text that came without a key event (listenTextInput)
     *   "keymap": text the keyboard layout, a "<code> <character>" line per key (watchLayout)
     *   "drop": a the id of the files in canvas.__incariDrops (listenDrops); WebPlatform copies
     *       them into the file system and hands the queue their paths, a line each, as text
     * flags: the modifier flags of key, mouse and wheel events (DomEventQueue.DOM_*).
     */

    /** Slots per queued event. */
    static final int EVENT_SLOTS = 9;

    @JSBody(params = "canvas", script = "var q = canvas.__incariEvents; return q ? q.length : 0;")
    static native int queuedSlots(JSObject canvas);

    @JSBody(
            params = {"canvas", "index"},
            script = "return String(canvas.__incariEvents[index]);")
    static native String queuedString(JSObject canvas, int index);

    @JSBody(
            params = {"canvas", "index"},
            script = "return +canvas.__incariEvents[index];")
    static native double queuedNumber(JSObject canvas, int index);

    @JSBody(params = "canvas", script = "var q = canvas.__incariEvents; if (q) q.length = 0;")
    static native void clearQueued(JSObject canvas);

    /**
     * Installs the page listeners once. Keys, mouse motion and releases are listened for on the
     * window, so a drag that leaves the canvas keeps reporting and always gets its release; presses
     * and the wheel only on the canvas.
     */
    @JSBody(
            params = {"canvas", "policy"},
            script = "var queue = []; var pressed = 0; canvas.__incariEvents = queue;\n"
                    // Queues an event and says whether to cancel its default action: keys by the
                    // policy, presses on the canvas and their releases, and the wheel.
                    + "function sink(type, text, key, a, b, c, d, e, f) {"
                    + "  queue.push(type, text, key, a, b, c, d, e, f);"
                    + "  switch (type) {"
                    + "    case 'keydown': case 'keyup': return !b && policy(text, key, f);"
                    + "    case 'down': if (a >= 0 && a < 30) { pressed |= 1 << a; return true; } return false;"
                    + "    case 'up': var bit = 1 << a; var was = (pressed & bit) !== 0; pressed &= ~bit; return was;"
                    + "    case 'wheel': return true;"
                    + "  } return false; }\n"
                    + "canvas.__incariSink = sink;\n"
                    // The page may be closed with no game frame to come: what must happen then
                    // (singleplayer's server saving) is registered in window.__incariOnLeave by the
                    // channels (JsChannel) and run straight from these listeners, at most once a second.
                    + "var lastLeave = 0;"
                    + "function leaving() { var now = Date.now(); if (now - lastLeave < 1000) return; lastLeave = now;"
                    + "  var hooks = window.__incariOnLeave || [];"
                    + "  for (var i = 0; i < hooks.length; i++) { try { hooks[i](); } catch (err) {} } }\n"
                    + "function flags(e) { var m = e.getModifierState ? function(k) { return e.getModifierState(k); }"
                    + " : function() { return false; };"
                    + " return (e.shiftKey ? 1 : 0) | (e.ctrlKey ? 2 : 0) | (e.altKey ? 4 : 0) | (e.metaKey ? 8 : 0)"
                    + " | (m('CapsLock') ? 16 : 0) | (m('NumLock') ? 32 : 0) | (m('AltGraph') ? 64 : 0); }\n"
                    // Keys are the game's while the game has focus: the canvas, its text input
                    // element, or nothing in particular (the body). Whatever else the host page
                    // shows beside the game (a text field, a button) keeps its keys, and their
                    // default actions.
                    + "function forPage(e) { var t = e.target; return !!t && t !== canvas && t !== canvas.__incariText"
                    + " && t !== document.body && t !== document.documentElement && t !== document && t !== window; }\n"
                    + "function key(type, e) { if (forPage(e)) return;"
                    + "  if (sink(type, e.code || '', e.key || '', e.repeat ? 1 : 0,"
                    + "      (e.isComposing || e.keyCode === 229) ? 1 : 0, 0, 0, 0, flags(e))) e.preventDefault(); }\n"
                    + "window.addEventListener('keydown', function(e) { key('keydown', e); });\n"
                    + "window.addEventListener('keyup', function(e) { key('keyup', e); });\n"
                    // Relative to the canvas's content box. Under Pointer Lock the position holds
                    // still and only movementX/Y count.
                    + "window.addEventListener('mousemove', function(e) { var r = canvas.getBoundingClientRect();"
                    + "  sink('move', '', '', e.clientX - r.left - canvas.clientLeft, e.clientY - r.top - canvas.clientTop,"
                    + "    e.movementX || 0, e.movementY || 0, 0, flags(e)); });\n"
                    // A click focuses the game: its text input element while a text field wants
                    // typing (setTextInput), else the canvas. The press itself is cancelled below,
                    // which would otherwise have moved the focus.
                    + "canvas.addEventListener('mousedown', function(e) {"
                    + "  var text = canvas.__incariTextOn && canvas.__incariText;"
                    + "  (text || canvas).focus({ preventScroll: true });"
                    + "  if (sink('down', '', '', e.button, 0, 0, 0, 0, flags(e))) e.preventDefault(); });\n"
                    + "window.addEventListener('mouseup', function(e) {"
                    + "  if (sink('up', '', '', e.button, 0, 0, 0, 0, flags(e))) e.preventDefault(); });\n"
                    + "canvas.addEventListener('wheel', function(e) {"
                    + "  if (sink('wheel', '', '', e.deltaX, e.deltaY, e.deltaMode, e.wheelDeltaX || 0, e.wheelDeltaY || 0,"
                    + "    flags(e))) e.preventDefault(); }, { passive: false });\n"
                    + "canvas.addEventListener('contextmenu', function(e) { e.preventDefault(); });\n"
                    + "document.documentElement.addEventListener('mouseenter', function() {"
                    + "  sink('enter', '', '', 0, 0, 0, 0, 0, 0); });\n"
                    + "document.documentElement.addEventListener('mouseleave', function() {"
                    + "  sink('enter', '', '', 0, 0, 0, 0, 0, 0); });\n"
                    + "window.addEventListener('focus', function() { sink('focus', '', '', 0, 0, 0, 0, 0, 0); });\n"
                    + "window.addEventListener('blur', function() { sink('blur', '', '', 0, 0, 0, 0, 0, 0); });\n"
                    // Hiding the tab counts as losing focus (DomEventQueue); showing it again gives
                    // focus back only if the page has it, which no focus event may announce.
                    + "document.addEventListener('visibilitychange', function() {"
                    + "  if (document.hidden) leaving();"
                    + "  sink('visibility', '', '', document.hidden ? 1 : 0, 0, 0, 0, 0, 0);"
                    + "  if (!document.hidden && document.hasFocus && document.hasFocus())"
                    + "    sink('focus', '', '', 0, 0, 0, 0, 0, 0); });\n"
                    // Sizes: a ResizeObserver sees every change of the canvas's box, from window
                    // resizes, fullscreen, page layout and zoom, and, where the browser has
                    // device-pixel-content-box, the exact device-pixel size (so moving to a denser
                    // display counts too). A media query on the density and the window's resize
                    // cover browsers without it.
                    + "function measure() { var d = window.devicePixelRatio || 1;"
                    + "  sink('resize', '', '', canvas.clientWidth, canvas.clientHeight, 0, 0, d, 0); }\n"
                    + "if (window.ResizeObserver) {"
                    + "  var observer = new ResizeObserver(function(entries) { var en = entries[entries.length - 1];"
                    + "    var box = en.contentBoxSize && en.contentBoxSize[0];"
                    + "    var dev = en.devicePixelContentBoxSize && en.devicePixelContentBoxSize[0];"
                    + "    sink('resize', '', '', box ? box.inlineSize : en.contentRect.width,"
                    + "      box ? box.blockSize : en.contentRect.height, dev ? dev.inlineSize : 0, dev ? dev.blockSize : 0,"
                    + "      window.devicePixelRatio || 1, 0); });"
                    + "  try { observer.observe(canvas, { box: 'device-pixel-content-box' }); }"
                    + "  catch (err) { observer.observe(canvas); } }\n"
                    + "function watchDensity() { if (!window.matchMedia) return;"
                    + "  var query = window.matchMedia('(resolution: ' + (window.devicePixelRatio || 1) + 'dppx)');"
                    + "  var changed = function() { query.removeEventListener('change', changed); measure(); watchDensity(); };"
                    + "  query.addEventListener('change', changed); }\n"
                    + "watchDensity();\n"
                    + "window.addEventListener('resize', measure);\n"
                    // In fullscreen the page can have the shortcuts browsers otherwise keep
                    // (Keyboard Lock, Chromium only): Ctrl+W, Ctrl+T, Ctrl+N and Ctrl+Q are
                    // sprinting while walking, chatting or dropping a stack, and Ctrl+1-9 are the
                    // hotbar while sprinting. Escape too, so it pauses the game instead of leaving
                    // fullscreen; holding it still leaves. Tab isn't locked, so Alt+Tab still
                    // switches windows.
                    + "var locked = ['Escape', 'KeyW', 'KeyT', 'KeyN', 'KeyQ'];"
                    + "for (var digit = 1; digit <= 9; digit++) locked.push('Digit' + digit);\n"
                    + "document.addEventListener('fullscreenchange', function() {"
                    + "  var kb = navigator.keyboard;"
                    + "  if (kb && kb.lock) { if (document.fullscreenElement === canvas) {"
                    + "    var p = kb.lock(locked); if (p && p.catch) p.catch(function() {}); } else { kb.unlock(); } }"
                    + "  sink('fullscreen', '', '', document.fullscreenElement ? 1 : 0, 0, 0, 0, 0, 0); });\n"
                    // The clipboard's text comes with the paste event Ctrl+V fires, before the game
                    // handles the key (DomEventQueue). Cancelled on the game's own elements, so it
                    // isn't also pasted into the text input element.
                    + "document.addEventListener('paste', function(e) {"
                    + "  var t = e.clipboardData ? e.clipboardData.getData('text') : '';"
                    + "  if (e.target === canvas || e.target === canvas.__incariText) e.preventDefault();"
                    + "  sink('paste', t || '', '', 0, 0, 0, 0, 0, 0); });\n"
                    // Closing the tab by accident while playing (Ctrl+W is sprint and walk) asks
                    // first. Only while the game holds the mouse (setLeaveGuard): a browser asks
                    // only after the player has used the page anyway.
                    + "window.addEventListener('beforeunload', function(e) { leaving();"
                    + "  if (canvas.__incariGuard) { e.preventDefault(); e.returnValue = ''; } });\n"
                    + "window.addEventListener('pagehide', function() { leaving(); });\n"
                    + "document.addEventListener('pointerlockchange', function() {"
                    + "  sink('lock', '', '', document.pointerLockElement === canvas ? 1 : 0, 0, 0, 0, 0, 0); });\n"
                    + "document.addEventListener('pointerlockerror', function() {"
                    + "  sink('lockerror', '', '', 0, 0, 0, 0, 0, 0); });\n"
                    // The state the page starts in, which no event will announce.
                    + "if (document.hasFocus && !document.hasFocus()) sink('blur', '', '', 0, 0, 0, 0, 0, 0);\n"
                    + "if (document.hidden) sink('visibility', '', '', 1, 0, 0, 0, 0, 0);")
    static native void listen(JSObject canvas, KeyPolicy policy);

    /**
     * Whether a page unload asks the player first (the "leave site?" dialog): on while the game
     * holds the mouse, so a stray Ctrl+W doesn't close a game in progress.
     */
    @JSBody(
            params = {"canvas", "guard"},
            script = "canvas.__incariGuard = guard;")
    static native void setLeaveGuard(JSObject canvas, boolean guard);

    // --- Text input ------------------------------------------------------------------------------

    /**
     * Adds the game's text input element: an invisible textarea beside the canvas that has the
     * focus while a text field wants typing. A canvas can't host an input method (Chinese,
     * Japanese, Korean), a dead key's composition or a touchscreen's keyboard; a text element can.
     * Plain typing still comes from key events, which are cancelled, so they never reach the
     * textarea; only what an input method composes does, and is reported from its events.
     * Installed once, after {@link #listen}.
     */
    @JSBody(
            params = "canvas",
            script = "var sink = canvas.__incariSink;"
                    + "var text = document.createElement('textarea');"
                    + "text.id = canvas.id + '-text';"
                    + "text.setAttribute('aria-label', 'Game text input');"
                    + "text.setAttribute('autocomplete', 'off'); text.setAttribute('autocorrect', 'off');"
                    + "text.setAttribute('autocapitalize', 'off'); text.spellcheck = false; text.tabIndex = -1;"
                    // Invisible, out of the way of the mouse, and fixed, so focusing it never
                    // scrolls the page. setTextInputArea moves it over the text field, where an
                    // input method places its candidate list.
                    + "text.style.cssText = 'position:fixed;left:0;top:0;width:1px;height:1px;opacity:0;"
                    + "padding:0;border:0;margin:0;outline:none;resize:none;overflow:hidden;white-space:pre;"
                    + "pointer-events:none;caret-color:transparent;z-index:-1;';"
                    + "(canvas.parentNode || document.body).insertBefore(text, canvas.nextSibling);"
                    + "canvas.__incariText = text;\n"
                    + "function compose() { var start = text.selectionStart || 0;"
                    + "  sink('compose', text.value, '', start, (text.selectionEnd || start) - start, 0, 0, 0, 0); }\n"
                    + "text.addEventListener('input', function(e) {"
                    + "  if (e.isComposing) { compose(); return; }"
                    // After compositionend some browsers report the commit again as an input.
                    + "  if (e.inputType === 'insertFromComposition' || e.inputType === 'deleteCompositionText') return;"
                    + "  if (text.value) sink('commit', text.value, '', 0, 0, 0, 0, 0, 0);"
                    + "  text.value = ''; });\n"
                    + "text.addEventListener('compositionend', function(e) {"
                    + "  sink('commit', e.data || '', '', 0, 0, 0, 0, 0, 0); text.value = ''; });")
    static native void listenTextInput(JSObject canvas);

    /**
     * Turns the text input element on or off. On, it takes the focus if the game has it; off, the
     * focus goes back to the canvas, which ends any composition.
     */
    @JSBody(
            params = {"canvas", "on"},
            script = "canvas.__incariTextOn = on; var text = canvas.__incariText; if (!text) return;"
                    + "var active = document.activeElement;"
                    + "if (on) { if (!active || active === canvas || active === document.body"
                    + "    || active === document.documentElement) text.focus({ preventScroll: true }); }"
                    + "else { if (active === text) { text.blur(); canvas.focus({ preventScroll: true }); }"
                    + "  text.value = ''; }")
    static native void setTextInput(JSObject canvas, boolean on);

    /** Moves the text input element over the text being edited, in CSS pixels from the canvas. */
    @JSBody(
            params = {"canvas", "x", "y", "width", "height"},
            script = "var text = canvas.__incariText; if (!text) return;"
                    + "var r = canvas.getBoundingClientRect(); var s = text.style;"
                    + "s.left = (r.left + x) + 'px'; s.top = (r.top + y) + 'px';"
                    + "s.width = Math.max(1, width) + 'px'; s.height = Math.max(1, height) + 'px';"
                    + "s.fontSize = Math.max(1, height) + 'px';")
    static native void setTextInputArea(JSObject canvas, int x, int y, int width, int height);

    // --- Keyboard layout ------------------------------------------------------------------------

    /**
     * Reads the player's keyboard layout with the Keyboard Map API (Chromium) now and whenever the
     * page regains focus, and queues a "keymap" event when it differs from the last one. Browsers
     * without the API report nothing, and the queue learns key names from typing instead.
     */
    @JSBody(
            params = "canvas",
            script = "var sink = canvas.__incariSink; var last = null;"
                    + "function read() { var kb = navigator.keyboard; if (!kb || !kb.getLayoutMap) return;"
                    // Refused in frames without permission, and while the page is hidden.
                    + "  kb.getLayoutMap().then(function(map) { var lines = [];"
                    + "    map.forEach(function(character, code) { lines.push(code + ' ' + character); });"
                    + "    var entries = lines.sort().join('\\n');"
                    + "    if (entries !== last) { last = entries; sink('keymap', entries, '', 0, 0, 0, 0, 0, 0); }"
                    + "  }, function() {}); }\n"
                    + "read(); window.addEventListener('focus', read);")
    static native void watchLayout(JSObject canvas);

    // --- Dropped files ---------------------------------------------------------------------------

    /**
     * Takes files and folders dropped anywhere on the page (so the browser doesn't navigate away to
     * show them): reads them all, folders recursively, keeps them in {@code canvas.__incariDrops}
     * under a new id as a flat array (relative path, then an {@code Int8Array}, or null for a
     * folder), and queues a "drop" event with the id. Reading is asynchronous: the event comes
     * once every file is in memory.
     */
    @JSBody(
            params = "canvas",
            script = "var sink = canvas.__incariSink; var next = 1; canvas.__incariDrops = {};\n"
                    + "function hasFiles(e) { var types = e.dataTransfer && e.dataTransfer.types;"
                    + "  if (!types) return false;"
                    + "  for (var i = 0; i < types.length; i++) if (types[i] === 'Files') return true; return false; }\n"
                    + "window.addEventListener('dragover', function(e) {"
                    + "  if (hasFiles(e)) { e.preventDefault(); e.dataTransfer.dropEffect = 'copy'; } });\n"
                    + "window.addEventListener('drop', function(e) { if (!hasFiles(e)) return; e.preventDefault();"
                    + "  var out = [], jobs = [];"
                    + "  function readFile(file, path) { return file.arrayBuffer().then("
                    + "    function(b) { out.push(path, new Int8Array(b)); }, function() {}); }"
                    + "  function walk(entry, path) {"
                    + "    if (entry.isFile) return new Promise(function(done) {"
                    + "      entry.file(function(f) { readFile(f, path).then(done); }, function() { done(); }); });"
                    + "    out.push(path, null); var reader = entry.createReader(), children = [];"
                    + "    return new Promise(function(done) { function finish() { Promise.all(children).then(done); }"
                    // readEntries hands out a folder's entries in batches, until an empty one.
                    // (No immediately invoked function here: TeaVM's printer drops its parentheses.)
                    + "      function more() { reader.readEntries(function(batch) { if (!batch.length) { finish(); return; }"
                    + "        for (var j = 0; j < batch.length; j++) children.push(walk(batch[j], path + '/' + batch[j].name));"
                    + "        more(); }, finish); }"
                    + "      more(); }); }"
                    // Entries must be taken while the drop is dispatched: the data is gone after.
                    + "  var items = e.dataTransfer.items;"
                    + "  if (items && items.length && items[0].webkitGetAsEntry) {"
                    + "    for (var i = 0; i < items.length; i++) { if (items[i].kind !== 'file') continue;"
                    + "      var entry = items[i].webkitGetAsEntry();"
                    + "      if (entry) { jobs.push(walk(entry, entry.name)); continue; }"
                    + "      var f = items[i].getAsFile(); if (f) jobs.push(readFile(f, f.name)); } }"
                    + "  else { var files = e.dataTransfer.files;"
                    + "    for (var k = 0; k < files.length; k++) jobs.push(readFile(files[k], files[k].name)); }"
                    + "  Promise.all(jobs).then(function() { canvas.__incariAddDrop(out); }); });\n"
                    // Also for files the player picks (chooseFiles): they reach the game as a drop.
                    + "canvas.__incariAddDrop = function(out) { if (!out.length) return; var id = next++;"
                    + "  canvas.__incariDrops[id] = out; sink('drop', '', '', id, 0, 0, 0, 0, 0); };")
    static native void listenDrops(JSObject canvas);

    /**
     * Opens the browser's file picker; the files the player picks arrive as a "drop" event, as if
     * dropped on the page (listenDrops must have run). The game asks for it on a click, and a
     * page may only open a picker while the player's click or key press is recent (a few
     * seconds): it's still recent when the game handles the click, a frame later.
     */
    @JSBody(
            params = "canvas",
            script = "var input = canvas.__incariPicker;\n"
                    + "if (!input) { input = document.createElement('input'); input.type = 'file'; input.multiple = true;"
                    + "  input.id = canvas.id + '-files'; input.style.display = 'none';"
                    + "  document.body.appendChild(input); canvas.__incariPicker = input;\n"
                    + "  input.addEventListener('change', function() { var files = input.files, out = [], jobs = [];"
                    + "    function read(file) { return file.arrayBuffer().then("
                    + "      function(b) { out.push(file.name, new Int8Array(b)); }, function() {}); }"
                    + "    for (var i = 0; i < files.length; i++) jobs.push(read(files[i]));"
                    + "    input.value = '';"
                    + "    Promise.all(jobs).then(function() { canvas.__incariAddDrop(out); }); }); }\n"
                    + "input.click();")
    static native void chooseFiles(JSObject canvas);

    /** Hands the player a file to save, as a download (a page can't show them its files otherwise). */
    @JSBody(
            params = {"name", "data", "type"},
            script = "var url = URL.createObjectURL(new Blob([data], {type: type}));"
                    + "var a = document.createElement('a'); a.href = url; a.download = name; a.style.display = 'none';"
                    + "document.body.appendChild(a); a.click(); a.remove();"
                    // The download has its own copy by the time it starts; a minute is plenty.
                    + "setTimeout(function() { URL.revokeObjectURL(url); }, 60000);")
    static native void download(String name, Int8Array data, String type);

    /** How many entries (files and folders) the drop with this id holds. */
    @JSBody(
            params = {"canvas", "id"},
            script = "var d = canvas.__incariDrops[id]; return d ? d.length / 2 : 0;")
    static native int droppedCount(JSObject canvas, int id);

    /** An entry's path relative to the drop, with "/" between folders. */
    @JSBody(
            params = {"canvas", "id", "index"},
            script = "return String(canvas.__incariDrops[id][2 * index]);")
    static native String droppedPath(JSObject canvas, int id, int index);

    @JSBody(
            params = {"canvas", "id", "index"},
            script = "return !canvas.__incariDrops[id][2 * index + 1];")
    static native boolean droppedIsFolder(JSObject canvas, int id, int index);

    /** A file entry's contents (not a folder's: {@link #droppedIsFolder}). */
    @JSBody(
            params = {"canvas", "id", "index"},
            script = "return canvas.__incariDrops[id][2 * index + 1];")
    static native Int8Array droppedBytes(JSObject canvas, int id, int index);

    /** Lets the page free a drop's contents once they're copied. */
    @JSBody(
            params = {"canvas", "id"},
            script = "delete canvas.__incariDrops[id];")
    static native void forgetDropped(JSObject canvas, int id);
}
