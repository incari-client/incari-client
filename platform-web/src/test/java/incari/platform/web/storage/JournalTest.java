package incari.platform.web.storage;

import static incari.platform.web.storage.TreeFileSystemTest.bytes;
import static org.assertj.core.api.Assertions.assertThat;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.List;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.teavm.runtime.fs.VirtualFile;
import org.teavm.runtime.fs.VirtualFileAccessor;

/** What a flush tells the store, for the ways the game changes its files. */
class JournalTest {

    private TreeFileSystem files;

    @BeforeEach
    void setUp() throws IOException {
        files = new TreeFileSystem(
                new Journal("/incari", List.of("/incari/dropped", "/incari/classpath")), () -> 1_000);
        files.load("/incari/game", true, 1, null);
        files.load("/incari/dropped", true, 1, null);
        files.collectChanges();
    }

    @Test
    void aNewFileIsSentWholeOnce() throws IOException {
        game().createFile("options.txt");
        write("/incari/game/options.txt", "fov:90\n");
        write("/incari/game/options.txt", "fov:100\n");
        assertThat(describe(files.collectChanges())).containsExactly("write /incari/game/options.txt 8 @0 fov:100\n");
        assertThat(files.hasChanges()).isFalse();
        assertThat(files.collectChanges()).isEmpty();
    }

    @Test
    void writesToAnExistingFileSendOnlyWhatChanged() throws IOException {
        files.load("/incari/game/r.0.0.mca", false, 1, bytes("0123456789"));
        VirtualFileAccessor region = files.getFile("/incari/game/r.0.0.mca").createAccessor(true, true, false);
        region.seek(2);
        region.write(bytes("ab"), 0, 2);
        region.seek(6);
        region.write(bytes("cd"), 0, 2);
        assertThat(describe(files.collectChanges())).containsExactly("write /incari/game/r.0.0.mca 10 @2 ab45cd");
        region.resize(4);
        assertThat(describe(files.collectChanges())).containsExactly("write /incari/game/r.0.0.mca 4 @0 -");
    }

    @Test
    void aFileTheStoreFailedToKeepIsSentWholeAgain() throws IOException {
        files.load("/incari/game/r.0.0.mca", false, 1, bytes("0123456789"));
        VirtualFileAccessor region = files.getFile("/incari/game/r.0.0.mca").createAccessor(true, true, false);
        region.seek(2);
        region.write(bytes("ab"), 0, 2);
        files.collectChanges();
        // That range never reached the store: what's sent now must cover it too.
        files.resend("/incari/game/r.0.0.mca");
        assertThat(describe(files.collectChanges())).containsExactly("write /incari/game/r.0.0.mca 10 @0 01ab456789");
    }

    @Test
    void aDeleteTheStoreFailedToKeepIsSentAgain() throws IOException {
        files.resend("/incari/game/gone.txt");
        assertThat(describe(files.collectChanges())).containsExactly("delete /incari/game/gone.txt");
    }

    @Test
    void writesFarApartAreSentApart() throws IOException {
        // A region file's write: the chunk's offset in the header, then its sectors much further on.
        files.load("/incari/game/r.0.0.mca", false, 1, new byte[20_000]);
        VirtualFileAccessor region = files.getFile("/incari/game/r.0.0.mca").createAccessor(true, true, false);
        region.seek(16_384);
        region.write(bytes("chunk"), 0, 5);
        region.seek(8);
        region.write(bytes("hd"), 0, 2);
        assertThat(describe(files.collectChanges()))
                .containsExactly(
                        "write /incari/game/r.0.0.mca 20000 @8 hd", "write /incari/game/r.0.0.mca 20000 @16384 chunk");
    }

    @Test
    void theServerListsSafeReplaceLeavesTheNewFileInPlace() throws IOException {
        files.load("/incari/game/servers.dat", false, 1, bytes("old"));
        // Util.safeReplaceFile: a temporary file, the old one moved to a backup, then the rename.
        game().createFile("servers42.dat");
        write("/incari/game/servers42.dat", "new");
        game().adopt(files.getFile("/incari/game/servers.dat"), "servers.dat_old");
        game().adopt(files.getFile("/incari/game/servers42.dat"), "servers.dat");
        assertThat(describe(files.collectChanges()))
                .containsExactly(
                        "write /incari/game/servers.dat 3 @0 new",
                        "write /incari/game/servers.dat_old 3 @0 old",
                        "delete /incari/game/servers42.dat");
    }

    @Test
    void aNewFolderIsSentWithEverythingInIt() throws IOException {
        game().createDirectory("resourcepacks");
        files.getFile("/incari/game/resourcepacks").createFile("pack.zip");
        write("/incari/game/resourcepacks/pack.zip", "PK");
        assertThat(describe(files.collectChanges()))
                .containsExactly(
                        "mkdir /incari/game/resourcepacks", "write /incari/game/resourcepacks/pack.zip 2 @0 PK");
    }

    @Test
    void aMovedFolderIsSentWholeAtItsNewPathAndDeletedAtItsOld() throws IOException {
        files.load("/incari/game/saves/A/level.dat", false, 1, bytes("L"));
        files.getFile("/incari/game/saves").adopt(files.getFile("/incari/game/saves/A"), "B");
        assertThat(describe(files.collectChanges()))
                .containsExactly(
                        "mkdir /incari/game/saves/B",
                        "write /incari/game/saves/B/level.dat 1 @0 L",
                        "delete /incari/game/saves/A");
    }

    @Test
    void aDeletedFolderIsOneDelete() throws IOException {
        files.load("/incari/game/saves/A/level.dat", false, 1, bytes("L"));
        files.getFile("/incari/game/saves/A/level.dat").delete();
        files.getFile("/incari/game/saves/A").delete();
        assertThat(describe(files.collectChanges())).containsExactly("delete /incari/game/saves/A");
    }

    @Test
    void aFileCreatedAndDeletedInOneFrameStillDeletesWhateverWasStored() throws IOException {
        game().createFile("tmp");
        write("/incari/game/tmp", "x");
        files.getFile("/incari/game/tmp").delete();
        // The store may have a file there from before; deleting nothing is harmless.
        assertThat(describe(files.collectChanges())).containsExactly("delete /incari/game/tmp");
    }

    @Test
    void transientFoldersAndPathsOutsideTheRootArentSent() throws IOException {
        files.getFile("/incari/dropped").createDirectory("1");
        files.getFile("/incari/dropped/1").createFile("pack.zip");
        write("/incari/dropped/1/pack.zip", "PK");
        files.getFile("/").createDirectory("tmp");
        files.getFile("/tmp").createFile("x");
        assertThat(files.collectChanges()).isEmpty();
    }

    @Test
    void aFileCopiedOutOfATransientFolderIsSent() throws IOException {
        files.load("/incari/dropped/1/pack.zip", false, 1, bytes("PK"));
        files.getFile("/incari/game").createDirectory("resourcepacks");
        files.getFile("/incari/game/resourcepacks").adopt(files.getFile("/incari/dropped/1/pack.zip"), "pack.zip");
        assertThat(describe(files.collectChanges()))
                .containsExactly(
                        "mkdir /incari/game/resourcepacks", "write /incari/game/resourcepacks/pack.zip 2 @0 PK");
    }

    @Test
    void settingATimeSendsNoBytes() throws IOException {
        files.load("/incari/game/level.dat", false, 1, bytes("L"));
        files.getFile("/incari/game/level.dat").setLastModified(99);
        assertThat(files.collectChanges()).singleElement().satisfies(op -> {
            assertThat(op.data()).isNull();
            assertThat(op.lastModified()).isEqualTo(99);
            assertThat(op.size()).isEqualTo(1);
        });
    }

    private VirtualFile game() {
        return files.getFile("/incari/game");
    }

    private void write(String path, String text) throws IOException {
        VirtualFileAccessor accessor = files.getFile(path).createAccessor(false, true, false);
        byte[] bytes = bytes(text);
        accessor.write(bytes, 0, bytes.length);
        accessor.close();
    }

    private static List<String> describe(List<StoreOp> ops) {
        return ops.stream()
                .map(op -> switch (op.kind()) {
                    case MKDIR -> "mkdir " + op.path();
                    case DELETE -> "delete " + op.path();
                    case WRITE -> {
                        byte[] data = op.data();
                        yield "write " + op.path() + " " + op.size() + " @" + op.at() + " "
                                + (data == null ? "-" : new String(data, StandardCharsets.UTF_8));
                    }
                })
                .toList();
    }
}
