package incari.platform.web.storage;

import java.io.IOException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.function.LongSupplier;
import java.util.function.Predicate;
import org.jspecify.annotations.Nullable;
import org.teavm.runtime.fs.VirtualFile;
import org.teavm.runtime.fs.VirtualFileAccessor;
import org.teavm.runtime.fs.VirtualFileSystem;

/**
 * The page's file system: a tree in memory, which TeaVM's {@code java.io} and {@code java.nio.file}
 * classes read and write through its {@link VirtualFileSystem} interface, and whose changes a
 * {@link Journal} collects for the persistent store (ADR 0011).
 *
 * <p>Everything is synchronous: the game reads and writes files as on the desktop. One kind of
 * file call suspends (must not block: coroutines): opening a world's region file that's kept in
 * place ({@link StoredFile}, ADR 0041), which the page reads from the store then and the server's
 * worker opens where it is; and moving one, on the page. No JavaScript callback opens one. Game
 * thread only.
 *
 * <p>Where TeaVM's own in-memory file system differs from the JDK, this one follows the JDK:
 * opening a file for reading and writing ({@code RandomAccessFile(file, "rw")}) keeps its bytes,
 * relative paths resolve against the working directory ("/" unless set), a file grown by a seek
 * past its end reads zeros in the gap, and a rename may replace a file.
 */
@SuppressWarnings("ReferenceEquality") // Nodes are identities: the same file is the same object.
public final class TreeFileSystem implements VirtualFileSystem {

    private final Directory root;
    private final Journal journal;
    private final LongSupplier clock;
    private String workingDirectory = "/";
    /** Where in-place files keep their bytes (ADR 0041); null where there are none. */
    private @Nullable InPlaceStore inPlace;

    /**
     * @param journal where changes under its persistent root are recorded
     * @param clock milliseconds since the epoch, for file times
     */
    TreeFileSystem(Journal journal, LongSupplier clock) {
        this.journal = journal;
        this.clock = clock;
        this.root = new Directory("", clock.getAsLong(), true);
    }

    Journal journal() {
        return journal;
    }

    /** Where the tree's in-place files ({@link StoredFile}) are read from or opened (ADR 0041). */
    void setInPlaceStore(@Nullable InPlaceStore store) {
        this.inPlace = store;
    }

    /** Relative paths resolve against this folder, which must exist. */
    void setWorkingDirectory(String path) throws IOException {
        String absolute = normalize(path);
        mkdirs(absolute, clock.getAsLong(), false);
        workingDirectory = absolute;
    }

    // --- VirtualFileSystem -------------------------------------------------------------------

    @Override
    public String getUserDir() {
        return workingDirectory;
    }

    @Override
    public VirtualFile getFile(String path) {
        return new TreeFile(normalize(path));
    }

    @Override
    public boolean isWindows() {
        return false;
    }

    @Override
    public String canonicalize(String path) {
        return normalize(path);
    }

    @Override
    public String[] getRoots() {
        return new String[] {"/"};
    }

    // --- The tree ----------------------------------------------------------------------------

    /** An absolute path with no empty, "." or ".." parts; relative paths start at the working directory. */
    String normalize(String path) {
        String full = path.startsWith("/") ? path : workingDirectory + "/" + path;
        List<String> parts = new ArrayList<>();
        int start = 0;
        while (start <= full.length()) {
            int end = full.indexOf('/', start);
            if (end < 0) {
                end = full.length();
            }
            String part = full.substring(start, end);
            if (part.equals("..")) {
                if (!parts.isEmpty()) {
                    parts.removeLast();
                }
            } else if (!part.isEmpty() && !part.equals(".")) {
                parts.add(part);
            }
            start = end + 1;
        }
        return parts.isEmpty() ? "/" : "/" + String.join("/", parts);
    }

    /** The node at a normalized absolute path, or null. */
    @Nullable
    Node find(String path) {
        Node node = root;
        int start = 1;
        while (start < path.length()) {
            int end = path.indexOf('/', start);
            if (end < 0) {
                end = path.length();
            }
            if (!(node instanceof Directory directory)) {
                return null;
            }
            node = directory.child(path.substring(start, end));
            if (node == null) {
                return null;
            }
            start = end + 1;
        }
        return node;
    }

    /**
     * Makes the folder at a normalized path and any missing parents.
     *
     * @param record whether to note new folders in the journal (not when loading from the store)
     */
    Directory mkdirs(String path, long lastModified, boolean record) throws IOException {
        Directory directory = root;
        int start = 1;
        while (start < path.length()) {
            int end = path.indexOf('/', start);
            if (end < 0) {
                end = path.length();
            }
            String name = path.substring(start, end);
            Node child = directory.child(name);
            if (child == null) {
                Directory created = new Directory(name, lastModified, false);
                directory.attach(name, created);
                if (record) {
                    journal.reshaped(path.substring(0, end));
                }
                child = created;
            } else if (!(child instanceof Directory)) {
                throw new IOException("Not a folder: " + path.substring(0, end));
            }
            directory = (Directory) child;
            start = end + 1;
        }
        return directory;
    }

    /** Puts a file or folder the store had into the tree, without noting it as a change. */
    void load(String path, boolean isDirectory, long lastModified, byte @Nullable [] contents) throws IOException {
        String absolute = normalize(path);
        if (isDirectory) {
            mkdirs(absolute, lastModified, false).lastModified = lastModified;
            return;
        }
        int slash = absolute.lastIndexOf('/');
        Directory parent = mkdirs(slash == 0 ? "/" : absolute.substring(0, slash), lastModified, false);
        parent.attach(
                absolute.substring(slash + 1),
                new DataFile("", lastModified, contents == null ? new byte[0] : contents));
    }

    /**
     * Puts an in-place file the store has into the tree (ADR 0041): its size and time, its bytes left
     * in the store.
     */
    void loadStored(String path, long lastModified, int size) throws IOException {
        String absolute = normalize(path);
        int slash = absolute.lastIndexOf('/');
        Directory parent = mkdirs(slash == 0 ? "/" : absolute.substring(0, slash), lastModified, false);
        String name = absolute.substring(slash + 1);
        if (parent.child(name) instanceof StoredFile known) {
            known.size = size;
            known.lastModified = lastModified;
        } else if (!(parent.child(name) instanceof DataFile)) {
            parent.attach(name, new StoredFile("", lastModified, size));
        }
    }

    /**
     * The folder at {@code path} and everything in it that the tree keeps itself (not mounted
     * files), as the store would load them, with every file's bytes: for another copy of the
     * program (ADR 0039). In-place files are read from the store to go with it.
     */
    List<FileBundle.Entry> export(String path) throws IOException {
        return export(path, false);
    }

    /**
     * As {@link #export(String)}; with {@code inPlace}, in-place files go as stored entries without
     * their bytes (the other copy opens them where they are, ADR 0041), those still in memory too:
     * the caller has made sure the store has every write to them.
     */
    List<FileBundle.Entry> export(String path, boolean inPlace) throws IOException {
        List<FileBundle.Entry> entries = new ArrayList<>();
        String absolute = normalize(path);
        Node node = find(absolute);
        if (node != null) {
            export(node, absolute, entries, inPlace);
        }
        return entries;
    }

    private void export(Node node, String path, List<FileBundle.Entry> entries, boolean inPlace) throws IOException {
        if (node instanceof Directory directory) {
            entries.add(FileBundle.Entry.folder(path, directory.lastModified));
            for (String name : directory.names()) {
                Node child = directory.child(name);
                if (child != null) {
                    export(child, path.equals("/") ? "/" + name : path + "/" + name, entries, inPlace);
                }
            }
        } else if (node instanceof StoredFile stored) {
            if (inPlace) {
                entries.add(FileBundle.Entry.stored(path, stored.lastModified, stored.size));
            } else {
                entries.add(FileBundle.Entry.file(path, stored.lastModified, readStored(path)));
            }
        } else if (node instanceof DataFile file) {
            if (inPlace && InPlaceFiles.matches(path)) {
                entries.add(FileBundle.Entry.stored(path, file.lastModified, file.size));
            } else {
                entries.add(FileBundle.Entry.file(path, file.lastModified, Arrays.copyOf(file.data, file.size)));
            }
        }
    }

    /** Lets go of every in-place file open under {@code path}, so another program may open them. */
    void closeStored(String path) {
        Node node = find(normalize(path));
        if (node != null) {
            closeStored(node);
        }
    }

    private static void closeStored(Node node) {
        if (node instanceof Directory directory) {
            for (Node child : directory.children()) {
                closeStored(child);
            }
        } else if (node instanceof StoredFile stored) {
            stored.close();
        }
    }

    /** Whether {@code node} is an in-place file or a folder with one in it. */
    private static boolean holdsStored(Node node) {
        if (node instanceof StoredFile) {
            return true;
        }
        if (node instanceof Directory directory) {
            for (Node child : directory.children()) {
                if (holdsStored(child)) {
                    return true;
                }
            }
        }
        return false;
    }

    private byte[] readStored(String path) throws IOException {
        InPlaceStore store = inPlace;
        if (store == null) {
            throw new IOException("No store to read " + path + " from");
        }
        return store.read(path);
    }

    /**
     * Reads an in-place file whole and puts it in the tree as a {@link DataFile}, unchanged as far as
     * the store is concerned (the page's way to open one, ADR 0041). Suspends while it's read.
     */
    private DataFile materialize(StoredFile stored, String path) throws IOException {
        byte[] bytes = readStored(path);
        Directory parent = stored.parent;
        DataFile file = new DataFile(stored.name, stored.lastModified, bytes);
        file.readOnly = stored.readOnly;
        if (parent != null) {
            parent.attach(stored.name, file);
        }
        return file;
    }

    /** Reads every in-place file under {@code node} into memory (before the page moves it). */
    private void materializeAll(Node node, String path) throws IOException {
        if (node instanceof Directory directory) {
            for (String name : directory.names()) {
                Node child = directory.child(name);
                if (child != null) {
                    materializeAll(child, path + "/" + name);
                }
            }
        } else if (node instanceof StoredFile stored) {
            materialize(stored, path);
        }
    }

    /**
     * Drops the folder at {@code path} from the tree without noting it as a change: the store keeps
     * it, and another copy of the program has it now (ADR 0039). The caller has flushed first, so
     * nothing under it is waiting to be stored.
     */
    void forget(String path) {
        Node node = find(normalize(path));
        if (node != null && node.parent != null) {
            node.parent.detach(node);
        }
    }

    /**
     * Mounts the entries of {@code archive} that {@code include} accepts (by their names in the
     * zip) under the folder at {@code path}, read-only. Mounted files are never persisted: the zip
     * is the store.
     *
     * @return how many files were mounted
     */
    int mount(String path, ZipArchive archive, Predicate<String> include) throws IOException {
        long now = clock.getAsLong();
        String base = normalize(path);
        Directory top = mkdirs(base, now, false);
        List<Directory> made = new ArrayList<>();
        int files = 0;
        for (ZipArchive.Entry entry : archive.entries()) {
            String name = entry.name();
            if (name.endsWith("/") || !include.test(name)) {
                continue;
            }
            String relative = normalize("/" + name);
            if (relative.equals("/")) {
                continue;
            }
            Directory parent = top;
            int start = 1;
            int end = relative.indexOf('/', start);
            while (end > 0) {
                String part = relative.substring(start, end);
                Node child = parent.child(part);
                if (child == null) {
                    Directory created = new Directory(part, now, false);
                    parent.attach(part, created);
                    made.add(created);
                    child = created;
                } else if (!(child instanceof Directory)) {
                    throw new IOException("Zip entry " + name + " is inside a file");
                }
                parent = (Directory) child;
                start = end + 1;
                end = relative.indexOf('/', start);
            }
            parent.attach(relative.substring(start), new MountedFile("", now, archive, entry));
            files++;
        }
        // Read-only once filled: the game can't add to or change the jar's resources.
        for (Directory directory : made) {
            directory.readOnly = true;
        }
        top.readOnly = true;
        return files;
    }

    /**
     * Marks {@code path} changed as a whole, so the next {@link #collectChanges} sends all of it
     * (or its deletion, if it's gone): the store failed to keep its last change.
     */
    void resend(String path) {
        journal.reshaped(path);
    }

    /** The changes to persist since the last call. */
    List<StoreOp> collectChanges() {
        return journal.collect(this);
    }

    /** Whether anything changed since the last {@link #collectChanges}; cheap, for every frame. */
    boolean hasChanges() {
        return !journal.isEmpty();
    }

    /**
     * How many files, and bytes in them, are in the tree under {@code path} (mounted files
     * excepted): those in memory, then the in-place files left in the store.
     */
    long[] measure(String path) {
        long[] totals = new long[4];
        Node node = find(normalize(path));
        if (node != null) {
            measure(node, totals);
        }
        return totals;
    }

    private static void measure(Node node, long[] totals) {
        if (node instanceof Directory directory) {
            for (Node child : directory.children()) {
                measure(child, totals);
            }
        } else if (node instanceof DataFile file) {
            totals[0]++;
            totals[1] += file.size;
        } else if (node instanceof StoredFile stored) {
            totals[2]++;
            totals[3] += stored.size;
        }
    }

    /** A path in the tree, as TeaVM's file classes see it: looked up on every call, like a path string. */
    private final class TreeFile implements VirtualFile {

        private final String path;

        TreeFile(String path) {
            this.path = path;
        }

        private @Nullable Node node() {
            return find(path);
        }

        @Override
        public String getName() {
            return path.substring(path.lastIndexOf('/') + 1);
        }

        @Override
        public boolean isDirectory() {
            return node() instanceof Directory;
        }

        @Override
        public boolean isFile() {
            return node() instanceof FileNode;
        }

        @Override
        public String @Nullable [] listFiles() {
            return node() instanceof Directory directory ? directory.names() : null;
        }

        @Override
        public @Nullable VirtualFileAccessor createAccessor(boolean readable, boolean writable, boolean append) {
            Node node = node();
            if (!(node instanceof FileNode file)) {
                return null;
            }
            if (file instanceof StoredFile stored) {
                try {
                    InPlaceStore store = inPlace;
                    if (store != null && store.opensInPlace()) {
                        return openInPlace(stored, store, readable, writable, append);
                    }
                    // The page's way: the whole file read into memory, then opened as any other.
                    file = materialize(stored, path);
                } catch (IOException e) {
                    System.err.println("[Incari] Can't open " + path + ": " + e.getMessage());
                    return null;
                }
            }
            if (!(file instanceof DataFile data)) {
                if (writable) {
                    return null;
                }
                try {
                    return new FileAccessor(file.contents(), clock);
                } catch (IOException e) {
                    // Opening fails as a missing file would; say why in the console.
                    System.err.println("[Incari] Can't read " + path + ": " + e.getMessage());
                    return null;
                }
            }
            if (!writable) {
                return new FileAccessor(data, null, clock, 0);
            }
            if (data.readOnly) {
                return null;
            }
            // TeaVM's streams ask for write-only to replace a file's contents; RandomAccessFile
            // asks for read-write and, as in the JDK, keeps them.
            if (!readable && !append && data.size > 0) {
                data.setSize(0);
                data.resized = true;
                data.lastModified = clock.getAsLong();
                journal.written(data);
            }
            return new FileAccessor(data, journal, clock, append ? data.size : 0);
        }

        /** Opens an in-place file where it is (singleplayer's server worker, ADR 0041). */
        private @Nullable VirtualFileAccessor openInPlace(
                StoredFile stored, InPlaceStore store, boolean readable, boolean writable, boolean append)
                throws IOException {
            if (writable && stored.readOnly) {
                return null;
            }
            InPlaceStore.Handle handle = stored.handle;
            if (handle == null) {
                handle = store.open(path);
                stored.handle = handle;
                stored.size = handle.size();
            }
            if (writable && !readable && !append && stored.size > 0) {
                // As for a file in memory: write-only replaces the contents.
                handle.truncate(0);
                stored.size = 0;
                stored.lastModified = clock.getAsLong();
            }
            return new StoredFileAccessor(stored, handle, writable, clock, append ? stored.size : 0);
        }

        @Override
        public boolean createFile(String fileName) throws IOException {
            Node node = node();
            if (!(node instanceof Directory directory)) {
                throw new IOException(node == null ? "No such folder: " + path : "Not a folder: " + path);
            }
            if (directory.readOnly) {
                throw new IOException("Read-only folder: " + path);
            }
            if (directory.child(fileName) != null) {
                return false;
            }
            long now = clock.getAsLong();
            InPlaceStore store = inPlace;
            String created = child(fileName);
            if (store != null && store.opensInPlace() && InPlaceFiles.matches(created)) {
                // Made in the store when first opened; the journal has nothing to send for it.
                directory.attach(fileName, new StoredFile(fileName, now, 0));
                directory.lastModified = now;
                return true;
            }
            directory.attach(fileName, new DataFile(fileName, now));
            directory.lastModified = now;
            journal.reshaped(created);
            return true;
        }

        @Override
        public boolean createDirectory(String fileName) {
            if (!(node() instanceof Directory directory) || directory.readOnly || directory.child(fileName) != null) {
                return false;
            }
            long now = clock.getAsLong();
            directory.attach(fileName, new Directory(fileName, now, false));
            directory.lastModified = now;
            journal.reshaped(child(fileName));
            return true;
        }

        @Override
        public boolean delete() {
            Node node = node();
            Directory parent = node == null ? null : node.parent;
            if (node == null || parent == null || parent.readOnly) {
                return false;
            }
            if (node instanceof Directory directory && !directory.isEmpty()) {
                return false;
            }
            if (node instanceof StoredFile stored) {
                // The store deletes it at the next flush; it can't while this program holds it.
                stored.close();
            }
            parent.detach(node);
            parent.lastModified = clock.getAsLong();
            journal.reshaped(path);
            return true;
        }

        @Override
        public boolean adopt(VirtualFile file, String fileName) {
            if (!(node() instanceof Directory directory) || directory.readOnly || !(file instanceof TreeFile moved)) {
                return false;
            }
            Node node = moved.node();
            Directory from = node == null ? null : node.parent;
            if (node == null || from == null || from.readOnly || directory.isWithin(node)) {
                return false;
            }
            Node existing = directory.child(fileName);
            if (existing == node) {
                return true;
            }
            if (existing instanceof Directory replaced && (!replaced.isEmpty() || !(node instanceof Directory))) {
                // As on the desktop: a folder can only replace an empty folder, and a file none.
                return false;
            }
            if (existing instanceof FileNode && node instanceof Directory) {
                return false;
            }
            if (holdsStored(node)) {
                InPlaceStore store = inPlace;
                if (store == null || store.opensInPlace()) {
                    // In-place files open where they are don't move: the store has no rename.
                    return false;
                }
                // The page's way: read into memory, then moved and sent whole like any other.
                try {
                    materializeAll(node, moved.path);
                } catch (IOException e) {
                    System.err.println("[Incari] Can't move " + moved.path + ": " + e.getMessage());
                    return false;
                }
                node = moved.node();
                if (node == null) {
                    return false;
                }
            }
            if (existing instanceof StoredFile replaced) {
                replaced.close();
            }
            long now = clock.getAsLong();
            from.detach(node);
            directory.attach(fileName, node);
            from.lastModified = now;
            directory.lastModified = now;
            journal.reshaped(moved.path);
            journal.reshaped(child(fileName));
            return true;
        }

        @Override
        public boolean canRead() {
            return node() != null;
        }

        @Override
        public boolean canWrite() {
            Node node = node();
            return node != null && !node.readOnly;
        }

        @Override
        public long lastModified() {
            Node node = node();
            return node == null ? 0 : node.lastModified;
        }

        @Override
        public boolean setLastModified(long lastModified) {
            Node node = node();
            if (node == null || node.readOnly) {
                return false;
            }
            node.lastModified = lastModified;
            if (node instanceof DataFile data) {
                data.resized = true;
                journal.written(data);
            }
            return true;
        }

        @Override
        public boolean setReadOnly(boolean readOnly) {
            Node node = node();
            if (node == null || node instanceof MountedFile) {
                return false;
            }
            node.readOnly = readOnly;
            return true;
        }

        @Override
        public int length() {
            return node() instanceof FileNode file ? file.length() : 0;
        }

        private String child(String name) {
            return path.equals("/") ? "/" + name : path + "/" + name;
        }
    }
}
