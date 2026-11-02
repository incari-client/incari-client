package incari.tools.webbuild;

import com.google.gson.GsonBuilder;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import incari.tools.build.CliException;
import incari.tools.build.FileTrees;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashSet;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.function.Predicate;
import java.util.stream.Stream;
import java.util.zip.Deflater;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;
import java.util.zip.ZipOutputStream;
import org.jspecify.annotations.Nullable;

/**
 * Writes the browser build as one folder any static web server can serve (ADR 0028): what the
 * dev server serves by name ({@link Serve}), laid out as the page fetches it, with its own
 * {@code build.json} over exactly the files copied, an Apache {@code .htaccess} with the headers, and
 * a {@code README.txt} that says how to serve it and that it must not be shared.
 *
 * <p>Run by {@code ./gradlew :app-web:webFolder}, which {@code ./incari build} calls. The folder
 * holds Mojang's code and files: it is written outside the tracked tree and never published.
 */
public final class WebFolder {

    /** Marks a folder this tool wrote, so a rerun may replace it and nothing else is ever deleted. */
    static final String MARKER = ".incari-web-folder";

    /** The page's start-up scripts (src/main/ts, types stripped), beside {@code index.html}. */
    private static final List<String> SCRIPTS =
            List.of("boot", "loader", "features", "jspi", "wakeups", "cache-policy", "sw", "game-worker");

    /**
     * The asset index's files the page fetches besides the sounds (its {@code WebAssets} and the
     * client's {@code RemoteAssets}): those read at start, then the Unicode font and the languages
     * before the reloads that read them. A name ending in {@code /} is a folder.
     */
    private static final String SOUND_LIST = "minecraft/sounds.json";

    private static final List<String> PAGE_ASSETS =
            List.of(SOUND_LIST, "pack.mcmeta", "minecraft/font/", "minecraft/lang/", "minecraft/resourcepacks/");

    private WebFolder() {}

    /** The command line, parsed. */
    record Options(
            Path out,
            String game,
            @Nullable Path compatible,
            @Nullable Path fast,
            @Nullable Path rust,
            Path mesher,
            Path page,
            Path scripts,
            Path pack,
            Path clientJar,
            @Nullable Path assetIndex,
            @Nullable Path assetObjects,
            SoundFiles soundFiles) {}

    /**
     * Which sound files go in: all; all but music, encoded smaller ({@link SmallSounds}, the single
     * file's); the few common ones as they are ({@link CoreSounds}); or none.
     */
    enum SoundFiles {
        ALL,
        SMALL,
        CORE,
        NONE
    }

    public static void main(String[] args) throws IOException {
        Options options = parse(args);
        Summary summary = write(options);
        System.out.println("==> Wrote " + options.out() + ": " + summary.files() + " files, "
                + summary.bytes() / (1024 * 1024) + " MB"
                + (summary.release() ? "" : " (a DEVELOPMENT build: larger and slower; ./incari build makes a release)")
                + (!summary.sounds()
                        ? ", no sounds"
                        : options.soundFiles() == SoundFiles.ALL ? "" : ", sounds but music")
                + ", builds: " + String.join(" and ", summary.builds()));
    }

    static Options parse(String[] args) {
        Map<String, String> values = new LinkedHashMap<>();
        for (int i = 0; i < args.length; i++) {
            String name = args[i];
            if (!name.startsWith("--") || i + 1 >= args.length) {
                throw new CliException("Unexpected argument '" + name + "'");
            }
            values.put(name.substring(2), args[++i]);
        }
        List<String> known = List.of(
                "out",
                "game",
                "compatible",
                "fast",
                "rust",
                "mesher",
                "page",
                "scripts",
                "pack",
                "client-jar",
                "asset-index",
                "asset-objects",
                "sound-files");
        for (String name : values.keySet()) {
            if (!known.contains(name)) {
                throw new CliException("Unknown option '--" + name + "'");
            }
        }
        if (values.get("compatible") == null && values.get("fast") == null && values.get("rust") == null) {
            throw new CliException("Give --fast, --compatible, --rust or more: the builds to put in the folder");
        }
        if ((values.get("asset-index") == null) != (values.get("asset-objects") == null)) {
            throw new CliException("--asset-index and --asset-objects go together");
        }
        SoundFiles soundFiles =
                switch (values.getOrDefault("sound-files", "yes")) {
                    case "yes" -> SoundFiles.ALL;
                    case "small" -> SoundFiles.SMALL;
                    case "core" -> SoundFiles.CORE;
                    case "no" -> SoundFiles.NONE;
                    default -> throw new CliException("--sound-files takes yes, small, core or no");
                };
        return new Options(
                Path.of(required(values, "out")),
                required(values, "game"),
                optionalPath(values, "compatible"),
                optionalPath(values, "fast"),
                optionalPath(values, "rust"),
                Path.of(required(values, "mesher")),
                Path.of(required(values, "page")),
                Path.of(required(values, "scripts")),
                Path.of(required(values, "pack")),
                Path.of(required(values, "client-jar")),
                optionalPath(values, "asset-index"),
                optionalPath(values, "asset-objects"),
                soundFiles);
    }

    record Summary(int files, long bytes, boolean release, boolean sounds, List<String> builds) {}

    static Summary write(Options options) throws IOException {
        Path out = options.out().toAbsolutePath().normalize();
        clear(out);
        Files.createDirectories(out);

        // The builds' files by the name the page fetches them by (BuildManifest.BUILDS): each build's
        // folder has its module as incari.wasm; TeaVM's loader is one file for both.
        Map<String, Path> build = new LinkedHashMap<>();
        Map<String, @Nullable Path> folders = new LinkedHashMap<>();
        folders.put("fast", options.fast());
        folders.put("compatible", options.compatible());
        List<String> builds = new ArrayList<>();
        // Whether the builds so far are development builds (TeaVM's deobfuscator beside them): -1 none yet.
        int development = -1;
        for (Map.Entry<String, String> entry : BuildManifest.BUILDS.entrySet()) {
            Path folder = folders.get(entry.getKey());
            if (folder == null) {
                continue;
            }
            String module = entry.getValue();
            requireFile(
                    folder.resolve("incari.wasm"),
                    "Compile it first: ./gradlew :app-web:teavm -Pteavm.release"
                            + (entry.getKey().equals("fast") ? " -Pteavm.fast" : ""));
            build.putIfAbsent(
                    BuildManifest.RUNTIME, requireFile(folder.resolve(BuildManifest.RUNTIME), "TeaVM's loader"));
            build.put(module, folder.resolve("incari.wasm"));
            putIfFile(
                    build,
                    module + BuildManifest.DEOBFUSCATOR,
                    folder.resolve("incari.wasm" + BuildManifest.DEOBFUSCATOR));
            putIfFile(
                    build, module + BuildManifest.DEBUG_INFO, folder.resolve("incari.wasm" + BuildManifest.DEBUG_INFO));
            int isDevelopment = Files.isRegularFile(folder.resolve("incari.wasm" + BuildManifest.DEOBFUSCATOR)) ? 1 : 0;
            if (development >= 0 && development != isDevelopment) {
                throw new CliException("The Fast and Compatible builds must both be release builds (-Pteavm.release)"
                        + " or both development builds");
            }
            development = isDevelopment;
            builds.add(entry.getKey().equals("fast") ? "Fast" : "Compatible");
        }
        for (String name : List.of("mesher.wasm", "mesher.wasm-runtime.js")) {
            build.put(name, requireFile(options.mesher().resolve(name), "Run ./gradlew :app-web:teavmMesher"));
        }
        build.put("incari.pack", requireFile(options.pack(), "Run ./gradlew :app-web:assetPack"));

        Map<String, Path> copied = new LinkedHashMap<>();
        for (Map.Entry<String, Path> entry : build.entrySet()) {
            copied.put(entry.getKey(), copy(entry.getValue(), out.resolve(entry.getKey())));
        }
        // Rust is its own page and files, in rust/ (ADR 0068).
        Path rust = options.rust();
        if (rust != null) {
            requireFile(rust.resolve("index.html"), "Build it first: ./incari build --only rust");
            FileTrees.copyTree(rust, out.resolve("rust"), "");
            builds.add("Rust");
        }
        JsonObject manifest = BuildManifest.manifest(options.game(), copied, rust != null);
        Files.writeString(
                out.resolve("build.json"),
                new GsonBuilder().setPrettyPrinting().create().toJson(manifest) + "\n");

        copy(requireFile(options.page(), "the page's index.html"), out.resolve("index.html"));
        for (String script : SCRIPTS) {
            copy(
                    requireFile(options.scripts().resolve(script + ".js"), "Run ./gradlew :app-web:pageScripts"),
                    out.resolve(script + ".js"));
        }
        Files.write(
                out.resolve("client.jar"),
                resourcesOnly(Files.readAllBytes(requireFile(options.clientJar(), "Run ./incari setup"))));

        Path assetIndex = options.assetIndex();
        Path assetObjects = options.assetObjects();
        boolean sounds = false;
        if (assetIndex != null && assetObjects != null) {
            copyAssets(assetIndex, assetObjects, out, options.soundFiles());
            sounds = options.soundFiles() != SoundFiles.NONE;
        }
        Files.writeString(out.resolve(".htaccess"), resource("htaccess"));
        Files.writeString(out.resolve("README.txt"), resource("README.txt").replace("{game}", options.game()));
        Files.writeString(out.resolve(MARKER), "Written by ./incari build. Never publish this folder.\n");

        long[] totals = new long[2];
        try (Stream<Path> files = Files.walk(out)) {
            files.filter(Files::isRegularFile).forEach(file -> {
                totals[0]++;
                totals[1] += size(file);
            });
        }
        return new Summary(
                (int) totals[0], totals[1], manifest.get("release").getAsBoolean(), sounds, List.copyOf(builds));
    }

    /**
     * Copies the asset index and the objects the page fetches from it: every sound and
     * {@link #PAGE_ASSETS}, the built-in packs among them (ADR 0055). The rest (the launcher's icons,
     * Realms' pictures) stays out: the page doesn't fetch it.
     *
     * <p>With {@link SoundFiles#NONE}, the sounds stay out too, and the index is written without them
     * and without the sound list, so the game lists no sound it can't fetch and plays silent. With
     * {@link SoundFiles#SMALL} (the single file, ADR 0057: all of them are 377 MB), every sound but
     * the music is encoded smaller and listed by its new hash; without ffmpeg, or with
     * {@link SoundFiles#CORE}, only {@link CoreSounds}' are copied as they are. Either way the sound
     * list is trimmed to the sounds kept and written as an object of its own.
     */
    static void copyAssets(Path index, Path objects, Path out, SoundFiles soundFiles) throws IOException {
        requireFile(index, "Run ./incari assets");
        JsonObject root = JsonParser.parseString(Files.readString(index)).getAsJsonObject();
        JsonObject entries = root.getAsJsonObject("objects");
        if (soundFiles == SoundFiles.SMALL && !SmallSounds.available()) {
            System.out.println("==> No ffmpeg with Vorbis here, so only the common sounds go in."
                    + " Install ffmpeg for every sound but the music");
            soundFiles = SoundFiles.CORE;
        }
        Set<String> written = new HashSet<>();
        if (soundFiles == SoundFiles.ALL) {
            copy(index, out.resolve("asset-index.json"));
        } else {
            Predicate<String> keeps =
                    switch (soundFiles) {
                        case SMALL -> SmallSounds::keeps;
                        case CORE -> CoreSounds::keeps;
                        default -> name -> false;
                    };
            JsonObject kept = new JsonObject();
            entries.entrySet().stream()
                    .filter(entry -> !isSound(entry.getKey()) || keeps.test(entry.getKey()))
                    .forEach(entry -> kept.add(entry.getKey(), entry.getValue()));
            if (soundFiles == SoundFiles.SMALL) {
                encodeSmaller(kept, objects, out, written);
            }
            if (soundFiles != SoundFiles.NONE) {
                written.add(SOUND_LIST);
                kept.add(SOUND_LIST, writeSoundList(entries, objects, out, kept));
            }
            root.add("objects", kept);
            Files.writeString(
                    out.resolve("asset-index.json"), new GsonBuilder().create().toJson(root));
            entries = kept;
        }
        List<String> missing = new ArrayList<>();
        for (Map.Entry<String, JsonElement> entry : entries.entrySet()) {
            String name = entry.getKey();
            if ((!name.endsWith(".ogg") && !forPage(name)) || written.contains(name)) {
                continue;
            }
            String hash = entry.getValue().getAsJsonObject().get("hash").getAsString();
            String path = hash.substring(0, 2) + "/" + hash;
            Path source = objects.resolve(path);
            if (!Files.isRegularFile(source)) {
                missing.add(name);
                continue;
            }
            Path target = out.resolve("assets/objects").resolve(path);
            // Content-addressed: two names with one file share it.
            if (!Files.exists(target)) {
                copy(source, target);
            }
        }
        if (!missing.isEmpty()) {
            throw new CliException(missing.size() + " assets aren't downloaded (" + missing.getFirst()
                    + ", ...). Run ./incari assets, or build without sounds");
        }
    }

    /**
     * The client jar without its classes and {@code META-INF}, which the page and the server worker
     * never read (the page's {@code ClientJar}, {@code GameWorkerMain}): 41 MB to 8 on a first visit.
     */
    static byte[] resourcesOnly(byte[] jar) throws IOException {
        ByteArrayOutputStream kept = new ByteArrayOutputStream(jar.length / 4);
        try (ZipInputStream in = new ZipInputStream(new ByteArrayInputStream(jar));
                ZipOutputStream zip = new ZipOutputStream(kept)) {
            zip.setLevel(Deflater.BEST_COMPRESSION);
            for (ZipEntry entry = in.getNextEntry(); entry != null; entry = in.getNextEntry()) {
                String name = entry.getName();
                if (name.endsWith(".class") || name.startsWith("META-INF/")) {
                    continue;
                }
                ZipEntry copy = new ZipEntry(name);
                copy.setTime(entry.getTime());
                zip.putNextEntry(copy);
                in.transferTo(zip);
                zip.closeEntry();
            }
        }
        return kept.toByteArray();
    }

    private static boolean isSound(String name) {
        return name.endsWith(".ogg") || name.equals(SOUND_LIST);
    }

    /**
     * Writes the sound list trimmed to the sound files in {@code kept} as an object of its own,
     * named by its hash as the index's are, and returns its index entry.
     */
    private static JsonObject writeSoundList(JsonObject entries, Path objects, Path out, JsonObject kept)
            throws IOException {
        JsonElement listed = entries.get(SOUND_LIST);
        if (listed == null) {
            throw new CliException("The asset index has no " + SOUND_LIST + ". Run ./incari assets");
        }
        String hash = listed.getAsJsonObject().get("hash").getAsString();
        Path source = requireFile(objects.resolve(hash.substring(0, 2) + "/" + hash), "Run ./incari assets");
        JsonObject trimmed =
                CoreSounds.trim(JsonParser.parseString(Files.readString(source)).getAsJsonObject(), kept::has);
        byte[] bytes = new GsonBuilder().create().toJson(trimmed).getBytes(StandardCharsets.UTF_8);
        return writeObject(out, bytes);
    }

    /** Writes {@code bytes} as an object named by its hash, as the index's are; returns its entry. */
    private static JsonObject writeObject(Path out, byte[] bytes) throws IOException {
        String sha1;
        try {
            sha1 = HexFormat.of().formatHex(MessageDigest.getInstance("SHA-1").digest(bytes));
        } catch (NoSuchAlgorithmException e) {
            throw new IllegalStateException(e);
        }
        Path target = out.resolve("assets/objects").resolve(sha1.substring(0, 2) + "/" + sha1);
        Files.createDirectories(target.getParent());
        Files.write(target, bytes);
        JsonObject entry = new JsonObject();
        entry.addProperty("hash", sha1);
        entry.addProperty("size", bytes.length);
        return entry;
    }

    /**
     * Encodes every sound in {@code kept} smaller ({@link SmallSounds}), side by side, writes each as
     * an object of its own and lists it in {@code kept} by its new hash; their names go in
     * {@code written}.
     */
    private static void encodeSmaller(JsonObject kept, Path objects, Path out, Set<String> written) throws IOException {
        Map<String, String> sounds = new TreeMap<>();
        for (Map.Entry<String, JsonElement> entry : kept.entrySet()) {
            if (entry.getKey().endsWith(".ogg")) {
                sounds.put(
                        entry.getKey(),
                        entry.getValue().getAsJsonObject().get("hash").getAsString());
            }
        }
        List<String> missing = new ArrayList<>();
        for (Map.Entry<String, String> sound : sounds.entrySet()) {
            String hash = sound.getValue();
            if (!Files.isRegularFile(objects.resolve(hash.substring(0, 2) + "/" + hash))) {
                missing.add(sound.getKey());
            }
        }
        if (!missing.isEmpty()) {
            throw new CliException(missing.size() + " sounds aren't downloaded (" + missing.getFirst()
                    + ", ...). Run ./incari assets, or build without sounds");
        }
        System.out.println("==> Encoding " + sounds.size() + " sounds smaller (once; kept for the next build)");
        Map<String, Future<byte[]>> encoded = new LinkedHashMap<>();
        int threads = Math.max(1, Runtime.getRuntime().availableProcessors());
        try (ExecutorService pool = Executors.newFixedThreadPool(threads)) {
            for (Map.Entry<String, String> sound : sounds.entrySet()) {
                encoded.put(sound.getKey(), pool.submit(() -> SmallSounds.encode(objects, sound.getValue())));
            }
            for (Map.Entry<String, Future<byte[]>> sound : encoded.entrySet()) {
                byte[] bytes;
                try {
                    bytes = sound.getValue().get();
                } catch (ExecutionException e) {
                    throw e.getCause() instanceof IOException io ? io : new IOException(e.getCause());
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    throw new IOException("Interrupted encoding the sounds", e);
                }
                kept.add(sound.getKey(), writeObject(out, bytes));
                written.add(sound.getKey());
            }
        }
    }

    private static boolean forPage(String name) {
        for (String asset : PAGE_ASSETS) {
            if (asset.endsWith("/") ? name.startsWith(asset) : name.equals(asset)) {
                return true;
            }
        }
        return false;
    }

    /**
     * Empties {@code out} for a fresh write, but only if it's missing, empty, or a folder this tool
     * wrote: never someone's other files.
     */
    static void clear(Path out) throws IOException {
        if (!Files.exists(out)) {
            return;
        }
        if (!Files.isDirectory(out)) {
            throw new CliException(out + " exists and isn't a folder");
        }
        boolean empty;
        try (Stream<Path> children = Files.list(out)) {
            empty = children.findAny().isEmpty();
        }
        if (empty) {
            return;
        }
        if (!Files.isRegularFile(out.resolve(MARKER))) {
            throw new CliException(
                    out + " already has files that ./incari build didn't write. Pick an empty " + "or new folder");
        }
        try (Stream<Path> files = Files.walk(out)) {
            for (Path file : files.sorted(Comparator.reverseOrder())
                    .filter(file -> !file.equals(out))
                    .toList()) {
                Files.delete(file);
            }
        }
    }

    private static Path copy(Path source, Path target) throws IOException {
        Files.createDirectories(target.getParent());
        Files.copy(source, target, StandardCopyOption.REPLACE_EXISTING);
        return target;
    }

    private static void putIfFile(Map<String, Path> files, String name, Path path) {
        if (Files.isRegularFile(path)) {
            files.put(name, path);
        }
    }

    private static Path requireFile(Path path, String hint) {
        if (!Files.isRegularFile(path)) {
            throw new CliException("Missing " + path + ". " + hint);
        }
        return path;
    }

    private static String required(Map<String, String> values, String name) {
        String value = values.get(name);
        if (value == null) {
            throw new CliException("Missing --" + name);
        }
        return value;
    }

    private static @Nullable Path optionalPath(Map<String, String> values, String name) {
        String value = values.get(name);
        return value == null || value.isEmpty() ? null : Path.of(value);
    }

    private static long size(Path file) {
        try {
            return Files.size(file);
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }

    private static String resource(String name) throws IOException {
        try (InputStream in = WebFolder.class.getResourceAsStream("/web-folder/" + name)) {
            if (in == null) {
                throw new IllegalStateException("Missing resource web-folder/" + name);
            }
            return new String(in.readAllBytes(), StandardCharsets.UTF_8);
        }
    }
}
