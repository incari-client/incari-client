package incari.tools.webbuild;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import incari.tools.build.CliException;
import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;
import java.util.zip.ZipOutputStream;
import org.jspecify.annotations.Nullable;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class WebFolderTest {

    @TempDir
    Path dir;

    private Path in;

    @BeforeEach
    void fakeBuild() throws Exception {
        in = dir.resolve("in");
        write("teavm/incari.wasm", "module");
        write("teavm/incari.wasm-runtime.js", "runtime");
        write("teavm/problems.txt", "a report, not part of the build");
        write("teavm-fast/incari.wasm", "fast module");
        write("teavm-fast/incari.wasm-runtime.js", "runtime");
        write("mesher/mesher.wasm", "mesher");
        write("mesher/mesher.wasm-runtime.js", "mesher runtime");
        write("page/index.html", "<html>");
        for (String script :
                new String[] {"boot", "loader", "features", "jspi", "wakeups", "cache-policy", "sw", "game-worker"}) {
            write("page/scripts/" + script + ".js", script);
        }
        write("page/incari.pack", "pack");
        Files.write(
                in.resolve("client.jar"),
                jar(
                        "net/minecraft/client/Main.class",
                        "META-INF/MANIFEST.MF",
                        "version.json",
                        "assets/minecraft/lang/en_us.json"));
        write("index.json", """
                {"objects": {
                  "minecraft/sounds.json": {"hash": "aa11", "size": 2},
                  "minecraft/sounds/a.ogg": {"hash": "bb22", "size": 1},
                  "minecraft/sounds/b.ogg": {"hash": "bb22", "size": 1},
                  "minecraft/sounds/dig/stone1.ogg": {"hash": "ff66", "size": 1},
                  "minecraft/lang/de_de.json": {"hash": "cc33", "size": 1},
                  "minecraft/textures/gui/title/background/panorama_0.png": {"hash": "ee55", "size": 1},
                  "icons/icon_16x16.png": {"hash": "dd44", "size": 1}
                }}
                """);
        write("objects/aa/aa11", """
                {"block.stone.break": {"sounds": ["dig/stone1"], "subtitle": "subtitles.block.generic.break"},
                 "music.game": {"sounds": [{"name": "a", "stream": true}, "minecraft:b"]},
                 "alias": {"sounds": [{"name": "block.stone.break", "type": "event"}]},
                 "gone": {"sounds": [{"name": "minecraft:music.game", "type": "event"}]}}
                """);
        write("objects/ff/ff66", "stone");
        write("objects/bb/bb22", "ogg");
        write("objects/cc/cc33", "lang");
        write("objects/ee/ee55", "panorama");
        write("objects/dd/dd44", "icon");
    }

    private WebFolder.Options options(Path out, boolean sounds) {
        return options(out, sounds, in.resolve("teavm"), in.resolve("teavm-fast"));
    }

    private WebFolder.Options options(Path out, boolean sounds, @Nullable Path compatible, @Nullable Path fast) {
        return options(out, sounds, WebFolder.SoundFiles.ALL, compatible, fast);
    }

    private WebFolder.Options options(
            Path out, boolean sounds, WebFolder.SoundFiles soundFiles, @Nullable Path compatible, @Nullable Path fast) {
        return options(out, sounds, soundFiles, compatible, fast, null);
    }

    private WebFolder.Options options(
            Path out,
            boolean sounds,
            WebFolder.SoundFiles soundFiles,
            @Nullable Path compatible,
            @Nullable Path fast,
            @Nullable Path rust) {
        return new WebFolder.Options(
                out,
                "26.3",
                compatible,
                fast,
                rust,
                in.resolve("mesher"),
                in.resolve("page/index.html"),
                in.resolve("page/scripts"),
                in.resolve("page/incari.pack"),
                in.resolve("client.jar"),
                sounds ? in.resolve("index.json") : null,
                sounds ? in.resolve("objects") : null,
                soundFiles);
    }

    @Test
    void writesTheBuildThePageAndTheAssetsThePageFetches() throws Exception {
        Path out = dir.resolve("web");
        WebFolder.Summary summary = WebFolder.write(options(out, true));

        assertThat(out.resolve("incari.wasm")).hasContent("module");
        assertThat(out.resolve("incari-fast.wasm")).hasContent("fast module");
        assertThat(out.resolve("boot.js")).exists();
        // Only what the page reads from it: no classes, no META-INF.
        assertThat(entries(out.resolve("client.jar")))
                .containsExactly("version.json", "assets/minecraft/lang/en_us.json");
        assertThat(out.resolve("asset-index.json")).exists();
        assertThat(out.resolve("assets/objects/aa/aa11")).exists();
        assertThat(out.resolve("assets/objects/bb/bb22")).exists();
        assertThat(out.resolve("assets/objects/cc/cc33")).exists();
        // The panorama: menus show Incari's photo instead.
        assertThat(out.resolve("assets/objects/ee/ee55")).doesNotExist();
        assertThat(out.resolve("assets/objects/dd/dd44")).doesNotExist();
        assertThat(out.resolve("problems.txt")).doesNotExist();
        assertThat(Files.readString(out.resolve("README.txt")))
                .contains("DO NOT SHARE")
                .contains("26.3");
        assertThat(out.resolve(".htaccess")).exists();
        assertThat(summary.release()).isTrue();
        assertThat(summary.sounds()).isTrue();
        assertThat(summary.builds()).containsExactly("Fast", "Compatible");

        JsonObject manifest = JsonParser.parseString(Files.readString(out.resolve("build.json")))
                .getAsJsonObject();
        assertThat(manifest.get("game").getAsString()).isEqualTo("26.3");
        assertThat(manifest.get("format").getAsInt()).isEqualTo(2);
        assertThat(manifest.getAsJsonObject("fast").get("module").getAsString()).isEqualTo("incari-fast.wasm");
        assertThat(manifest.getAsJsonObject("compatible").get("module").getAsString())
                .isEqualTo("incari.wasm");
        assertThat(manifest.getAsJsonObject("programs").keySet()).containsExactly("mesher");
        assertThat(manifest.getAsJsonArray("files")).hasSize(6);
        assertThat(manifest.has("rust")).isFalse();
    }

    @Test
    void putsTheRustBuildInItsOwnFolderAndListsItsPage() throws Exception {
        write("rust/index.html", "<html>horizon");
        write("rust/pkg/horizon-client_bg.wasm", "rust module");
        Path out = dir.resolve("web");
        WebFolder.Summary summary = WebFolder.write(options(
                out,
                false,
                WebFolder.SoundFiles.ALL,
                in.resolve("teavm"),
                in.resolve("teavm-fast"),
                in.resolve("rust")));

        assertThat(out.resolve("rust/index.html")).hasContent("<html>horizon");
        assertThat(out.resolve("rust/pkg/horizon-client_bg.wasm")).hasContent("rust module");
        assertThat(summary.builds()).containsExactly("Fast", "Compatible", "Rust");
        JsonObject manifest = JsonParser.parseString(Files.readString(out.resolve("build.json")))
                .getAsJsonObject();
        assertThat(manifest.getAsJsonObject("rust").get("page").getAsString()).isEqualTo("rust/index.html");
        // Horizon's service worker keeps its files, so Incari's list leaves them out.
        assertThat(manifest.getAsJsonArray("files")).hasSize(6);
    }

    @Test
    void leavesTheSoundsOutButKeepsTheLanguages() throws Exception {
        Path out = dir.resolve("web");
        WebFolder.Summary summary = WebFolder.write(
                options(out, true, WebFolder.SoundFiles.NONE, in.resolve("teavm"), in.resolve("teavm-fast")));

        assertThat(summary.sounds()).isFalse();
        assertThat(out.resolve("assets/objects/bb/bb22")).doesNotExist();
        assertThat(out.resolve("assets/objects/aa/aa11")).doesNotExist();
        assertThat(out.resolve("assets/objects/cc/cc33")).exists();
        assertThat(out.resolve("assets/objects/ee/ee55")).doesNotExist();
        // The game lists no sound it can't fetch.
        JsonObject objects = JsonParser.parseString(Files.readString(out.resolve("asset-index.json")))
                .getAsJsonObject()
                .getAsJsonObject("objects");
        assertThat(objects.keySet())
                .containsExactlyInAnyOrder(
                        "minecraft/lang/de_de.json",
                        "minecraft/textures/gui/title/background/panorama_0.png",
                        "icons/icon_16x16.png");
    }

    @Test
    void keepsOnlyTheCoreSoundsAndTrimsTheSoundListToThem() throws Exception {
        Path out = dir.resolve("web");
        WebFolder.Summary summary = WebFolder.write(
                options(out, true, WebFolder.SoundFiles.CORE, in.resolve("teavm"), in.resolve("teavm-fast")));

        assertThat(summary.sounds()).isTrue();
        assertThat(out.resolve("assets/objects/ff/ff66")).hasContent("stone");
        assertThat(out.resolve("assets/objects/bb/bb22")).doesNotExist();
        assertThat(out.resolve("assets/objects/aa/aa11")).doesNotExist();
        JsonObject objects = JsonParser.parseString(Files.readString(out.resolve("asset-index.json")))
                .getAsJsonObject()
                .getAsJsonObject("objects");
        assertThat(objects.keySet())
                .containsExactlyInAnyOrder(
                        "minecraft/sounds.json",
                        "minecraft/sounds/dig/stone1.ogg",
                        "minecraft/lang/de_de.json",
                        "minecraft/textures/gui/title/background/panorama_0.png",
                        "icons/icon_16x16.png");

        // The list is an object of its own, named by its hash, with only events that still play.
        JsonObject listed = objects.getAsJsonObject("minecraft/sounds.json");
        String hash = listed.get("hash").getAsString();
        Path list = out.resolve("assets/objects/" + hash.substring(0, 2) + "/" + hash);
        assertThat(Files.size(list)).isEqualTo(listed.get("size").getAsLong());
        JsonObject events = JsonParser.parseString(Files.readString(list)).getAsJsonObject();
        assertThat(events.keySet()).containsExactlyInAnyOrder("block.stone.break", "alias");
        assertThat(events.getAsJsonObject("block.stone.break").get("subtitle").getAsString())
                .isEqualTo("subtitles.block.generic.break");
    }

    @Test
    void writesOneBuildAlone() throws Exception {
        Path out = dir.resolve("web");
        WebFolder.Summary summary = WebFolder.write(options(out, false, null, in.resolve("teavm-fast")));

        assertThat(summary.builds()).containsExactly("Fast");
        assertThat(out.resolve("incari.wasm")).doesNotExist();
        JsonObject manifest = JsonParser.parseString(Files.readString(out.resolve("build.json")))
                .getAsJsonObject();
        assertThat(manifest.has("compatible")).isFalse();
        assertThat(manifest.has("fast")).isTrue();
    }

    @Test
    void refusesAReleaseBuildBesideADevelopmentOne() throws Exception {
        write("teavm-fast/incari.wasm-deobfuscator.wasm", "deobfuscator");
        assertThatThrownBy(() -> WebFolder.write(options(dir.resolve("web"), false)))
                .isInstanceOf(CliException.class)
                .hasMessageContaining("both be release builds");
    }

    @Test
    void rewritesItsOwnFolderButNeverSomeoneElses() throws Exception {
        Path out = dir.resolve("web");
        WebFolder.write(options(out, true));
        WebFolder.write(options(out, false));
        assertThat(out.resolve("assets")).doesNotExist();

        Path other = dir.resolve("other");
        write("../other/keep.txt", "mine");
        assertThatThrownBy(() -> WebFolder.write(options(other, false))).isInstanceOf(CliException.class);
        assertThat(other.resolve("keep.txt")).hasContent("mine");
    }

    @Test
    void failsOnAMissingSound() throws Exception {
        Files.delete(in.resolve("objects/bb/bb22"));
        assertThatThrownBy(() -> WebFolder.write(options(dir.resolve("web"), true)))
                .isInstanceOf(CliException.class)
                .hasMessageContaining("./incari assets");
    }

    private static byte[] jar(String... names) throws Exception {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        try (ZipOutputStream zip = new ZipOutputStream(bytes)) {
            for (String name : names) {
                zip.putNextEntry(new ZipEntry(name));
                zip.write(name.getBytes(StandardCharsets.UTF_8));
                zip.closeEntry();
            }
        }
        return bytes.toByteArray();
    }

    private static List<String> entries(Path jar) throws Exception {
        List<String> names = new ArrayList<>();
        try (ZipInputStream zip = new ZipInputStream(Files.newInputStream(jar))) {
            for (ZipEntry entry = zip.getNextEntry(); entry != null; entry = zip.getNextEntry()) {
                names.add(entry.getName());
            }
        }
        return names;
    }

    private void write(String name, String content) throws Exception {
        Path file = in.resolve(name).normalize();
        Files.createDirectories(file.getParent());
        Files.writeString(file, content);
    }
}
