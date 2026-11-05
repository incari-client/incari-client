package incari.platform.web.storage;

import incari.platform.api.PlatformNotice;
import java.io.IOException;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import java.util.function.Predicate;
import org.jspecify.annotations.Nullable;
import org.teavm.jso.JSBody;
import org.teavm.jso.JSObject;
import org.teavm.jso.typedarrays.ArrayBuffer;
import org.teavm.jso.typedarrays.Int8Array;
import org.teavm.runtime.fs.VirtualFileSystemProvider;

/**
 * The browser build's file system (ADR 0011): an in-memory {@link TreeFileSystem} that every
 * {@code java.io} and {@code java.nio.file} call in the page goes through, loaded at startup from
 * the storage worker (OPFS, or IndexedDB) and written back to it behind the game.
 *
 * <p>Writes are stored within a frame: {@link #flush}, once a frame, sends the changes to the
 * worker. The game's save points (options, the server list, a world's save) are ordinary writes
 * and renames. A page closed mid-frame loses what that frame wrote.
 *
 * <p>A change the store couldn't keep (a full disk) is sent again, whole, a few seconds later: the
 * store is sent only the bytes that changed, and a range lost would leave the file damaged. That
 * saving fails, or can't happen at all, is told to the player ({@link #takeNotice}).
 */
public final class WebFileSystem {

    /** Paths under this folder survive a reload. */
    public static final String ROOT = "/incari";

    private static final String LOCK = "incari-storage";

    /** Off ({@code ?inplace=off}): worlds go to a worker with their bytes, as in ADR 0039, to compare. */
    private static boolean inPlaceHandOver = true;

    private final TreeFileSystem tree;
    /**
     * The storage worker: the page's own, or in a copy of the game in a worker the port the page
     * gave it (ADR 0041); null in a worker without one, whose page stores its changes for it.
     */
    private final @Nullable JSObject store;
    /** Whether this is a copy of the game in a worker (ADR 0039). */
    private final boolean detached;

    private final String backend;
    private final boolean readOnly;
    private @Nullable String lastProblem;
    /** In a worker's copy without a port: changes collected for the page, oldest first ({@link #takeChanges}). */
    private final List<StoreOp> pending = new ArrayList<>();
    /** In a worker's copy: whether the world it was handed was left in place (ADR 0041). */
    private boolean inPlaceSession;

    /** How long a change the store couldn't keep waits before it's sent again, in milliseconds. */
    private static final double RETRY_MILLIS = 5000;

    /** Paths whose change the store couldn't keep, to send again whole at {@link #retryAt}. */
    private final Set<String> failedPaths = new LinkedHashSet<>();

    private double retryAt;
    /** Notices for the player, oldest first ({@link #takeNotice}). */
    private final List<PlatformNotice> notices = new ArrayList<>();
    /** Saving failed and the player was told; cleared once a batch is stored whole again. */
    private boolean failureReported;

    private WebFileSystem(
            TreeFileSystem tree, @Nullable JSObject store, boolean detached, String backend, boolean readOnly) {
        this.tree = tree;
        this.store = store;
        this.detached = detached;
        this.backend = backend;
        this.readOnly = readOnly;
    }

    /** The entry point's choice: false hands worlds over with their bytes (ADR 0041's comparison). */
    public static void setInPlaceHandOver(boolean on) {
        inPlaceHandOver = on;
    }

    /**
     * The file system of a copy of the game in a worker (ADR 0039): empty at first, filled with the
     * files the page hands it ({@link #takeOver}). Changes go to the storage worker through the port
     * the page gave it (ADR 0041), or without a port to the page ({@link #takeChanges}).
     */
    public static WebFileSystem installDetached(List<String> transientRoots) throws IOException {
        TreeFileSystem tree = new TreeFileSystem(new Journal(ROOT, transientRoots), WebFileSystem::nowMillis);
        tree.mkdirs("/tmp", nowMillis(), false);
        tree.mkdirs(ROOT, nowMillis(), false);
        tree.collectChanges();
        VirtualFileSystemProvider.setInstance(tree);
        return new WebFileSystem(tree, BrowserStore.fromPort(), true, "page", false);
    }

    /**
     * The files under {@code directory}, encoded for another copy of the game (a worker's, or back
     * to the page), dropped from this tree without telling the store (ADR 0039). What was waiting to
     * be stored is sent first. Where the store is the origin private file system and this page may
     * write it, a world's region files stay there for the worker to open in place (ADR 0041): the
     * store is waited for until it has every write, and they go as their sizes alone. Otherwise
     * they go with their bytes, read from the store if need be. Suspends while it waits or reads.
     */
    public byte[] handOver(String directory) throws IOException {
        flush();
        boolean inPlace =
                detached ? inPlaceSession : inPlaceHandOver && store != null && backend.equals("opfs") && !readOnly;
        JSObject storage = store;
        if (inPlace && storage != null) {
            BrowserStore.sync(storage);
        }
        tree.closeStored(directory);
        List<FileBundle.Entry> entries = tree.export(directory, inPlace);
        tree.forget(directory);
        return FileBundle.encode(new FileBundle.Bundle(inPlace, entries));
    }

    /**
     * Takes files {@link #handOver} encoded into this tree, as if loaded from the store. In a
     * worker handed a world in place, its region files are opened where they are when the server
     * opens them; back on the page, they're known by size again, as the store has them now (the
     * worker may have made more of them, and may have ended before it could say).
     */
    public void takeOver(byte[] bundle) throws IOException {
        FileBundle.Bundle decoded = FileBundle.decode(bundle);
        if (detached) {
            inPlaceSession = decoded.inPlace();
            tree.setInPlaceStore(inPlaceSession ? new OpfsFiles(OpfsFiles.available()) : null);
        }
        for (FileBundle.Entry entry : decoded.entries()) {
            if (entry.stored()) {
                tree.loadStored(entry.path(), entry.lastModified(), entry.size());
            } else {
                tree.load(entry.path(), entry.directory(), entry.lastModified(), entry.contents());
            }
        }
        JSObject storage = store;
        if (!detached
                && decoded.inPlace()
                && storage != null
                && !decoded.entries().isEmpty()) {
            refreshStored(storage, decoded.entries().get(0).path());
        }
    }

    /** The in-place files under {@code directory} as the store has them: sizes and times, new ones too. */
    private void refreshStored(JSObject storage, String directory) throws IOException {
        JSObject reply = BrowserStore.stat(storage, directory);
        int count = BrowserStore.entryCount(reply);
        for (int i = 0; i < count; i++) {
            String path = BrowserStore.entryPath(reply, i);
            if (!BrowserStore.entryIsDirectory(reply, i) && InPlaceFiles.matches(path)) {
                tree.loadStored(
                        path, (long) BrowserStore.entryLastModified(reply, i), BrowserStore.entrySize(reply, i));
            }
        }
    }

    /** In a worker's copy: what changed since the last call, encoded for the page; null if nothing. */
    public byte @Nullable [] takeChanges() {
        flush();
        if (pending.isEmpty()) {
            return null;
        }
        byte[] encoded = FileBundle.encodeOps(pending);
        pending.clear();
        return encoded;
    }

    /** Sends a worker's changes ({@link #takeChanges}) to this page's store. */
    public void storeChanges(byte[] changes) {
        List<StoreOp> ops = FileBundle.decodeOps(changes);
        if (!readOnly && store != null && !ops.isEmpty()) {
            send(store, ops);
        }
    }

    /**
     * Mounts the entries of a zip already in a JavaScript buffer that {@code include} takes,
     * read-only, under {@code directory}: the client jar in a worker's copy of the game, read from
     * where the page's storage keeps it. The entries are copied out, still compressed, so the
     * buffer isn't held: a server worker keeps the jar's data packs, a few megabytes, not its 40.
     */
    public int mountZip(String directory, ArrayBuffer zip, Predicate<String> include) throws IOException {
        return tree.mount(directory, ZipArchive.open(new PageBuffer(zip)).compact(include), include);
    }

    /**
     * Starts the storage worker, loads everything stored under {@link #ROOT} into memory (suspends
     * until it's read), and makes the result the page's file system. Call it before anything
     * touches a file: TeaVM's {@code java.nio.file} binds the file system once, when first used.
     *
     * <p>Relative paths start at "/", as in TeaVM's own file system: TeaVM's
     * {@code Files.createDirectories} creates an absolute path's parents through relative
     * sub-paths, so any other working directory would put them in the wrong place.
     *
     * @param transientRoots folders under {@link #ROOT} that don't survive a reload
     */
    public static WebFileSystem install(List<String> transientRoots) throws IOException {
        double start = now();
        TreeFileSystem tree = new TreeFileSystem(new Journal(ROOT, transientRoots), WebFileSystem::nowMillis);
        JSObject store = BrowserStore.start(StorageWorkerSource.SCRIPT);
        JSObject reply = BrowserStore.open(store, ROOT, LOCK);
        String backend = BrowserStore.backend(reply);
        int count = BrowserStore.entryCount(reply);
        for (int i = 0; i < count; i++) {
            String path = BrowserStore.entryPath(reply, i);
            if (!Journal.within(path, ROOT)) {
                continue;
            }
            boolean directory = BrowserStore.entryIsDirectory(reply, i);
            long lastModified = (long) BrowserStore.entryLastModified(reply, i);
            if (lastModified == 0) {
                lastModified = nowMillis();
            }
            if (BrowserStore.entryIsStored(reply, i)) {
                tree.loadStored(path, lastModified, BrowserStore.entrySize(reply, i));
                continue;
            }
            byte[] contents =
                    directory ? null : BrowserStore.takeEntryData(reply, i).copyToJavaArray();
            tree.load(path, directory, lastModified, contents);
        }
        // Scratch space for Files.createTempFile() and friends, as on a desktop.
        tree.mkdirs("/tmp", nowMillis(), false);
        tree.collectChanges();
        VirtualFileSystemProvider.setInstance(tree);

        WebFileSystem files = new WebFileSystem(tree, store, false, backend, BrowserStore.readOnly(reply));
        tree.setInPlaceStore(new PageStore(store));
        long[] loaded = tree.measure(ROOT);
        log("[Incari] Storage: " + backend + ", " + loaded[0] + " files (" + (loaded[1] / 1024) + " KiB) loaded in "
                + Math.round(now() - start) + " ms; " + loaded[2] + " region files (" + (loaded[3] / 1024)
                + " KiB) left in place");
        String problems = BrowserStore.problems(reply);
        if (!problems.isEmpty()) {
            warn("[Incari] Storage: " + problems);
        }
        if (!BrowserStore.locked(reply)) {
            warn("[Incari] Storage: the game is open in another tab, which keeps its files; nothing this tab "
                    + "changes will be saved.");
            files.notices.add(PlatformNotice.SAVES_HELD_ELSEWHERE);
        } else if (backend.equals("memory")) {
            warn("[Incari] Storage: this browser offers the page no storage; nothing will be saved.");
            files.notices.add(PlatformNotice.SAVES_UNAVAILABLE);
        }
        return files;
    }

    /** Where the files are kept: "opfs", "indexeddb", or "memory" (nowhere). */
    public String backend() {
        return backend;
    }

    /**
     * Sends the store what changed since the last flush, without waiting for it to be stored.
     * Every frame: returns at once when nothing changed (hot path, don't allocate).
     */
    public void flush() {
        JSObject storage = store;
        if (storage != null && !readOnly) {
            retryFailed(storage);
        }
        if (tree.hasChanges()) {
            List<StoreOp> ops = tree.collectChanges();
            if (storage == null) {
                pending.addAll(ops);
            } else if (!readOnly && !ops.isEmpty()) {
                send(storage, ops);
            }
        }
        if (storage == null) {
            return;
        }
        String problem = BrowserStore.takeProblem(storage);
        if (problem != null && !problem.equals(lastProblem)) {
            // Once per kind of failure: a full disk would otherwise say so every frame.
            lastProblem = problem;
            warn("[Incari] Storage: " + problem);
        }
        if (problem != null && !failureReported && !detached) {
            failureReported = true;
            notices.add(PlatformNotice.SAVE_FAILED);
        } else if (failureReported && failedPaths.isEmpty() && BrowserStore.lastBatchStored(storage)) {
            // Saving works again: a later failure is news.
            failureReported = false;
            lastProblem = null;
        }
    }

    /**
     * Collects the paths the store reported it couldn't keep, and once their wait is over marks them
     * changed in the tree, so this flush sends them again whole (or deletes them, if they're gone).
     * Hot path: allocates nothing while nothing failed.
     */
    private void retryFailed(JSObject storage) {
        String path;
        while ((path = BrowserStore.takeFailedPath(storage)) != null) {
            if (failedPaths.isEmpty()) {
                retryAt = now() + RETRY_MILLIS;
            }
            failedPaths.add(path);
        }
        if (failedPaths.isEmpty() || now() < retryAt) {
            return;
        }
        for (String failed : failedPaths) {
            tree.resend(failed);
        }
        failedPaths.clear();
    }

    /** The oldest notice for the player not yet taken, or null. The page's thread, once a frame. */
    public @Nullable PlatformNotice takeNotice() {
        return notices.isEmpty() ? null : notices.remove(0);
    }

    private static void send(JSObject store, List<StoreOp> ops) {
        JSObject batch = BrowserStore.newBatch();
        for (StoreOp op : ops) {
            switch (op.kind()) {
                case MKDIR -> BrowserStore.addMkdir(batch, op.path(), (double) op.lastModified());
                case WRITE -> {
                    byte[] data = op.data();
                    BrowserStore.addWrite(
                            batch,
                            op.path(),
                            op.size(),
                            op.at(),
                            data == null ? null : Int8Array.copyFromJavaArray(data),
                            (double) op.lastModified());
                }
                case DELETE -> BrowserStore.addDelete(batch, op.path());
            }
        }
        BrowserStore.apply(store, batch);
    }

    /**
     * Mounts a zip from {@code url} read-only under {@code directory}, with the entries
     * {@code include} accepts. The zip is kept by the storage worker under {@code cacheKey}, so a
     * later load reads it from there instead of downloading it again (a new key replaces it). The
     * accepted entries' bytes stay in the page, outside the game's heap, copied out of the zip so
     * the rest of it isn't held (the client jar's classes: ADR 0043); each is inflated when opened.
     *
     * @return how many files were mounted, and whether the zip came from the cache
     */
    public Mounted mountZip(String directory, String url, String cacheKey, Predicate<String> include)
            throws IOException {
        JSObject storage = store;
        if (storage == null) {
            throw new IOException("A worker's copy of the game has no storage of its own to fetch " + url + " into");
        }
        JSObject reply = BrowserStore.download(storage, url, cacheKey);
        String error = BrowserStore.error(reply);
        if (error != null) {
            throw new IOException("Couldn't fetch " + url + ": " + error);
        }
        boolean cached = BrowserStore.cached(reply);
        ZipArchive archive;
        try {
            archive = ZipArchive.open(new PageBuffer(BrowserStore.data(reply)));
        } catch (IOException e) {
            if (!cached) {
                throw e;
            }
            // A damaged copy (the page closed while caching it?): drop it and download again.
            warn("[Incari] Storage: the cached " + cacheKey + " is damaged (" + e.getMessage()
                    + "); downloading it again");
            BrowserStore.forget(storage, cacheKey);
            return mountZip(directory, url, cacheKey, include);
        }
        return new Mounted(tree.mount(directory, archive.compact(include), include), cached);
    }

    /** What {@link #mountZip} did: how many files it mounted, and whether the zip was cached. */
    public record Mounted(int files, boolean cached) {}

    /**
     * The page's in-place files (ADR 0041): read whole from the storage worker when something on the
     * page opens one (singleplayer's server on the page, a world's download or backup).
     */
    private record PageStore(JSObject store) implements InPlaceStore {
        @Override
        public boolean opensInPlace() {
            return false;
        }

        @Override
        public byte[] read(String path) {
            ArrayBuffer data = BrowserStore.read(store, path);
            return data == null ? new byte[0] : new Int8Array(data).copyToJavaArray();
        }

        @Override
        public Handle open(String path) throws IOException {
            throw new IOException("The page reads in-place files whole: " + path);
        }
    }

    /** A zip's bytes in a JavaScript buffer, read a range at a time. */
    private record PageBuffer(ArrayBuffer buffer) implements ZipArchive.Source {
        @Override
        public int size() {
            return buffer.getByteLength();
        }

        @Override
        public byte[] read(int position, int length) throws IOException {
            if (position < 0 || length < 0 || position > size() - length) {
                throw new IOException("Read past the end of the zip");
            }
            return BrowserStore.slice(buffer, position, length).copyToJavaArray();
        }

        /** In a new buffer of the page's, outside the game's heap, as the zip's own bytes were. */
        @Override
        public ZipArchive.Kept keep(int[] localHeaders, int[] compressedSizes) throws IOException {
            JSObject entries = BrowserStore.newRanges();
            for (int i = 0; i < localHeaders.length; i++) {
                BrowserStore.addRange(entries, localHeaders[i], compressedSizes[i]);
            }
            JSObject copy = BrowserStore.copyEntries(buffer, entries);
            String error = BrowserStore.error(copy);
            if (error != null) {
                throw new IOException(error);
            }
            int[] starts = new int[localHeaders.length];
            for (int i = 0; i < starts.length; i++) {
                starts[i] = BrowserStore.copiedStart(copy, i);
            }
            return new ZipArchive.Kept(new PageBuffer(BrowserStore.data(copy)), starts);
        }
    }

    private static long nowMillis() {
        return (long) dateNow();
    }

    @JSBody(script = "return Date.now();")
    private static native double dateNow();

    @JSBody(script = "return performance.now();")
    private static native double now();

    @JSBody(params = "message", script = "console.log(message);")
    private static native void log(String message);

    @JSBody(params = "message", script = "console.warn(message);")
    private static native void warn(String message);
}
