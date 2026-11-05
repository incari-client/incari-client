package incari.platform.web;

import incari.platform.api.AppInfo;
import incari.platform.api.NativeSurfaceFactory;
import incari.platform.api.Platform;
import incari.platform.api.PlatformNotice;
import incari.platform.api.PlatformRecorder;
import incari.platform.api.PlatformShell;
import incari.platform.api.audio.PlatformAudio;
import incari.platform.api.fonts.PlatformFonts;
import incari.platform.api.input.PlatformInput;
import incari.platform.api.net.PlatformNetwork;
import incari.platform.api.profile.PlatformProfiling;
import incari.platform.api.rtc.PlatformRtc;
import incari.platform.api.storage.PlatformStorage;
import incari.platform.api.threads.PlatformChannel;
import incari.platform.api.threads.PlatformThreads;
import incari.platform.api.threads.WorkerPool;
import incari.platform.api.window.PlatformDisplays;
import incari.platform.api.window.PlatformEventListener;
import incari.platform.api.window.PlatformWindow;
import incari.platform.api.window.WindowSpec;
import incari.platform.web.audio.WebAudio;
import incari.platform.web.fonts.CanvasFonts;
import incari.platform.web.rtc.WebRtc;
import incari.platform.web.storage.WebFileSystem;
import java.io.File;
import java.io.IOException;
import java.net.URI;
import java.nio.file.Files;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.ExecutorService;
import org.jspecify.annotations.Nullable;
import org.teavm.jso.JSObject;
import org.teavm.jso.typedarrays.Int8Array;

/**
 * The platform in a browser tab, over TeaVM's JavaScript interop: a canvas window with keyboard
 * (layout-aware key names, input methods), mouse (Pointer Lock), fullscreen, the clipboard, file
 * drops, links and dialogs; WebSocket connections to servers; the file system over OPFS,
 * file downloads and the file picker; Web Audio.
 *
 * <p>Everything runs on the page's one thread. The game's loop yields to the page every frame
 * ({@link #endFrame}); that's when DOM events are queued, to be delivered on the next poll.
 */
public final class WebPlatform implements Platform {

    /** The id of the host page's canvas; created full-window if the page has none. */
    public static final String CANVAS_ID = "incari-canvas";

    /** Where files dropped on the page are copied for the game to open ({@link DroppedFiles}). */
    public static final String DROP_DIRECTORY = "/incari/dropped";

    private static final double HEARTBEAT_MILLIS = 5000;

    private final PlatformFonts fonts = new CanvasFonts();
    private final PlatformStorage storage;
    private final WebFileSystem files;
    private final WebDisplays displays = new WebDisplays();
    private final WebAudio audio = new WebAudio();
    private final DroppedFiles drops = new DroppedFiles(new File(DROP_DIRECTORY));
    private final WebSocketNetwork network = new WebSocketNetwork();
    private final WebRtc rtc = new WebRtc();
    private final WebProfiling profiling = new WebProfiling();
    /**
     * Java threads are coroutines on the page's one thread, so extra pool threads would only add
     * switching; the background pool runs in the time the game's frames leave over
     * ({@link IdleTimeExecutor}).
     */
    private final PlatformThreads threads = new PlatformThreads() {
        @Override
        public int backgroundThreads(int suggested) {
            return 0;
        }

        @Override
        public ExecutorService backgroundExecutor(String name) {
            return IdleTimeExecutor.forPage("Worker-" + name);
        }

        @Override
        public void pauseIfLate() {
            IdleTimeExecutor.pauseIfLate();
        }

        @Override
        public boolean oneThread() {
            return true;
        }

        @Override
        public int backgroundQueued() {
            return IdleTimeExecutor.queuedEverywhere();
        }

        @Override
        public @Nullable WorkerPool workers(String program) {
            return pools.computeIfAbsent(program, name -> Optional.ofNullable(WebWorkerPool.start(name)))
                    .orElse(null);
        }

        @Override
        public @Nullable PlatformChannel startGameWorker(String role) {
            return JsChannel.startWorker(role);
        }
    };
    /** Each program's workers, started the first time they're asked for. */
    private final Map<String, Optional<WorkerPool>> pools = new HashMap<>();

    private @Nullable CanvasWindow window;
    private @Nullable DomEventQueue events;
    /**
     * Animation frames come every display interval, give or take; a frame that would be this early
     * for the limit still counts, so a 60 fps limit on a 60 Hz display doesn't skip every other one.
     */
    private static final double FRAME_LIMIT_SLACK_MILLIS = 4;

    private long frames;
    private double frameStart = Double.NEGATIVE_INFINITY;
    private double frameInterval;
    private double lastHeartbeat = Double.NaN;

    /**
     * The least time the background pool gets in a frame while it has work, in milliseconds
     * (ADR 0035). Its slots end shortly before the next frame is due, so without this a slow frame
     * left it 1 ms, part of one section's snapshot.
     */
    public static void setPoolFloor(double millis) {
        Dom.setPoolFloor(Math.max(1.0, millis));
    }

    /**
     * To run just before every wait off the render thread: a background pool task that blocks gives
     * the pool up, so the work it waits for can run (the entry point wires it to web-classlib's
     * {@code Waits}).
     */
    public static void beforeWait() {
        IdleTimeExecutor.beforeWait();
    }

    /**
     * The platform for this page.
     *
     * @param files the page's file system, already installed ({@link WebFileSystem#install}); the
     *     platform sends its changes to storage once a frame
     */
    public WebPlatform(PlatformStorage storage, WebFileSystem files) {
        this.storage = new FileStorage(storage, files);
        this.files = files;
    }

    @Override
    public String description() {
        return "Browser (" + Dom.userAgent() + ")";
    }

    @Override
    public String buildKind() {
        return Dom.buildKind();
    }

    private final WebRecorder recorder = new WebRecorder();

    @Override
    public PlatformRecorder recorder() {
        return recorder;
    }

    @Override
    public void initialize(AppInfo app) {
        Dom.installFrameMonitor();
        CanvasWindow canvasWindow = new CanvasWindow(Dom.canvas(CANVAS_ID));
        JSObject canvas = canvasWindow.canvas();
        CanvasSize size = CanvasSize.initial(Dom.cssWidth(canvas), Dom.cssHeight(canvas), Dom.devicePixelRatio());
        DomEventQueue queue = new DomEventQueue(canvasWindow, size, Dom.isMac(), Dom.functionKeyChords());
        canvasWindow.connect(queue);
        Dom.listen(canvas, DomEventQueue::consumesKey);
        Dom.listenTextInput(canvas);
        Dom.watchLayout(canvas);
        Dom.listenDrops(canvas);
        window = canvasWindow;
        events = queue;
    }

    @Override
    public void shutdown() {}

    @Override
    public long nanoTime() {
        return (long) (Dom.nowMillis() * 1_000_000.0);
    }

    @Override
    public PlatformDisplays displays() {
        return displays;
    }

    @Override
    public PlatformWindow createWindow(WindowSpec spec, NativeSurfaceFactory surface) {
        CanvasWindow canvasWindow = requireInitialized(window);
        // The canvas is the surface; backends that need a native window get the constant handle.
        surface.create(spec.title(), spec.width(), spec.height(), 0L);
        canvasWindow.setTitle(spec.title());
        return canvasWindow;
    }

    @Override
    public void pollEvents(PlatformEventListener listener) {
        DomEventQueue queue = requireInitialized(events);
        takeQueued(queue);
        queue.drainTo(listener);
        PlatformNotice notice;
        while ((notice = files.takeNotice()) != null) {
            listener.notice(notice);
        }
        // Network callbacks queue in the page like input does; the game's thread delivers them.
        network.deliver();
        rtc.deliver();
    }

    /** Moves the events the page queued since the last poll into {@code queue}, on the game's thread. */
    private void takeQueued(DomEventQueue queue) {
        JSObject canvas = requireInitialized(window).canvas();
        int slots = Dom.queuedSlots(canvas);
        for (int i = 0; i + Dom.EVENT_SLOTS <= slots; i += Dom.EVENT_SLOTS) {
            String type = Dom.queuedString(canvas, i);
            if (type.equals("drop")) {
                queue.event(type, storeDrop(canvas, (int) Dom.queuedNumber(canvas, i + 3)), "", 0, 0, 0, 0, 0, 0);
                continue;
            }
            queue.event(
                    type,
                    Dom.queuedString(canvas, i + 1),
                    Dom.queuedString(canvas, i + 2),
                    Dom.queuedNumber(canvas, i + 3),
                    Dom.queuedNumber(canvas, i + 4),
                    Dom.queuedNumber(canvas, i + 5),
                    Dom.queuedNumber(canvas, i + 6),
                    Dom.queuedNumber(canvas, i + 7),
                    (int) Dom.queuedNumber(canvas, i + 8));
        }
        Dom.clearQueued(canvas);
    }

    /**
     * Copies the files of a drop from the page into the file system, and frees the page's copy.
     *
     * @return the dropped paths, one per line, as the queue's "drop" event takes them; empty if
     *     they couldn't be stored (logged)
     */
    private String storeDrop(JSObject canvas, int id) {
        try {
            List<String> paths = drops.store(id, new DroppedFiles.Source() {
                @Override
                public int count() {
                    return Dom.droppedCount(canvas, id);
                }

                @Override
                public String path(int index) {
                    return Dom.droppedPath(canvas, id, index);
                }

                @Override
                public boolean isFolder(int index) {
                    return Dom.droppedIsFolder(canvas, id, index);
                }

                @Override
                public byte[] bytes(int index) {
                    return Dom.droppedBytes(canvas, id, index).copyToJavaArray();
                }
            });
            return String.join("\n", paths);
        } catch (IOException e) {
            Dom.consoleError("[Incari] Couldn't store the dropped files: " + e);
            return "";
        } finally {
            Dom.forgetDropped(canvas, id);
        }
    }

    @Override
    public void endFrame() {
        // What the game wrote this frame goes to storage now, while the page draws: a save point
        // (options, the server list, a world) is durable a frame after the game reaches it.
        files.flush();
        if (frames == 0 && Double.isNaN(lastHeartbeat)) {
            // The first frame is drawn: the host page's loading screen can go.
            Dom.loaderDone();
        }
        double stall = Dom.frameEnded();
        if (stall < 0) {
            // The render thread waited mid-frame; a wait of 50 ms or more prints where it was.
            Dom.consoleWarn("[Incari] Stall: a " + Math.round(-stall) + " ms frame in which the render thread waited");
        }
        // The page's only chance to run: input, drawing, timers, network callbacks. Under a frame
        // limit, as many animation frames as it takes: the page keeps drawing and handling input,
        // the game waits between its frames rather than inside one.
        Dom.nextAnimationFrame();
        while (frameInterval > 0 && Dom.nowMillis() - frameStart < frameInterval - FRAME_LIMIT_SLACK_MILLIS) {
            // A skipped animation frame is between the game's frames, not a wait inside one.
            Dom.frameSkipped();
            Dom.nextAnimationFrame();
        }
        frameStart = Dom.nowMillis();
        frameInterval = 0;
        heartbeat();
    }

    @Override
    public boolean limitFrameRate(int framesPerSecond) {
        // For the frame about to end; the game asks again every frame it wants limited.
        frameInterval = framesPerSecond > 0 ? 1000.0 / framesPerSecond : 0;
        return true;
    }

    /** Logs the loop's progress every few seconds: the first sign in the console that the game runs. */
    private void heartbeat() {
        frames++;
        double now = Dom.nowMillis();
        if (Double.isNaN(lastHeartbeat)) {
            lastHeartbeat = now;
        } else if (now - lastHeartbeat >= HEARTBEAT_MILLIS) {
            Dom.consoleLog("[Incari] Main loop running: " + frames + " frames, "
                    + Math.round(frames * 1000.0 / (now - lastHeartbeat)) + " per second; " + Dom.frameStats());
            frames = 0;
            lastHeartbeat = now;
        }
    }

    @Override
    public void flushInputEvents() {
        DomEventQueue queue = requireInitialized(events);
        // Take what the page queued first: flushing keeps releases, which must not be lost.
        takeQueued(queue);
        queue.flushInput();
    }

    @Override
    public PlatformInput input() {
        DomEventQueue queue = requireInitialized(events);
        return new PlatformInput() {
            @Override
            public boolean isKeyDown(int scancode) {
                return queue.isKeyDown(scancode);
            }

            @Override
            public int modifiers() {
                return queue.modifiers();
            }

            @Override
            public @Nullable String keyName(int scancode) {
                return queue.keyName(scancode);
            }
        };
    }

    @Override
    public PlatformShell shell() {
        return new PlatformShell() {
            @Override
            public @Nullable String clipboardText() {
                return events == null ? "" : events.lastPaste();
            }

            @Override
            public boolean setClipboardText(String text) {
                // Asynchronous: the browser answers later, so this can't say whether it worked.
                // The game never waits on it (must not block: coroutines).
                if (events != null) {
                    events.copied(text);
                }
                Dom.writeClipboard(text);
                return true;
            }

            @Override
            public boolean setClipboardImage(byte[] png) {
                // Asynchronous, as text is: the browser may still refuse (no focus, no permission).
                return Dom.writeClipboardImage(Int8Array.copyFromJavaArray(png));
            }

            @Override
            public void open(URI uri) {
                Dom.openUrl(uri.toString());
            }

            @Override
            public void openPath(String path) {
                File file = new File(path);
                if (file.isDirectory()) {
                    // A page can't show a folder; the picker lets the player add files to it,
                    // which the game takes as a drop (the pack screens copy them in).
                    CanvasWindow canvasWindow = window;
                    if (canvasWindow != null) {
                        Dom.chooseFiles(canvasWindow.canvas());
                    }
                } else if (file.isFile()) {
                    try {
                        byte[] bytes = Files.readAllBytes(file.toPath());
                        Dom.download(file.getName(), Int8Array.copyFromJavaArray(bytes), mediaType(file.getName()));
                    } catch (IOException e) {
                        Dom.consoleError("[Incari] Couldn't read " + path + " to download it: " + e);
                    }
                } else {
                    Dom.consoleWarn("[Incari] Nothing to open at " + path);
                }
            }

            @Override
            public boolean showsFolders() {
                return false;
            }

            @Override
            public void download(String fileName, byte[] bytes) {
                Dom.download(fileName, Int8Array.copyFromJavaArray(bytes), mediaType(fileName));
            }

            @Override
            public boolean showErrorDialog(String title, String message, boolean askToContinue) {
                if (askToContinue) {
                    return Dom.confirm(title + "\n\n" + message);
                }
                Dom.alert(title + "\n\n" + message);
                return false;
            }
        };
    }

    @Override
    public PlatformAudio audio() {
        return audio;
    }

    @Override
    public PlatformNetwork network() {
        return network;
    }

    @Override
    public PlatformRtc rtc() {
        return rtc;
    }

    @Override
    public void crashNatively() {
        throw new InternalError("Debug crash requested; a page can't crash natively");
    }

    @Override
    public PlatformStorage storage() {
        return storage;
    }

    @Override
    public PlatformThreads threads() {
        return threads;
    }

    @Override
    public PlatformProfiling profiling() {
        return profiling;
    }

    @Override
    public PlatformFonts fonts() {
        return fonts;
    }

    /** The media type a download of {@code name} is offered as. */
    static String mediaType(String name) {
        String lower = name.toLowerCase(Locale.ROOT);
        if (lower.endsWith(".png")) {
            return "image/png";
        } else if (lower.endsWith(".zip")) {
            return "application/zip";
        } else if (lower.endsWith(".json") || lower.endsWith(".mcmeta")) {
            return "application/json";
        } else if (lower.endsWith(".txt") || lower.endsWith(".log")) {
            return "text/plain";
        }
        return "application/octet-stream";
    }

    /** Logs to the browser console; the entry point wires the game's logging here. */
    public static void log(String level, String message) {
        switch (level) {
            case "ERROR" -> Dom.consoleError(message);
            case "WARN" -> Dom.consoleWarn(message);
            default -> Dom.consoleLog(message);
        }
    }

    private static <T> T requireInitialized(@Nullable T part) {
        if (part == null) {
            throw new IllegalStateException("The web platform isn't initialized yet");
        }
        return part;
    }
}
