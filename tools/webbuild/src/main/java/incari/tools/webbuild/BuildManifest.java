package incari.tools.webbuild;

import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import incari.tools.build.CliException;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Collections;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.SequencedMap;

/**
 * Writes {@code build.json}, the build manifest the page's loader reads (ADR 0014): which builds
 * there are (Fast and Compatible, ADR 0036: each a WebAssembly GC module, with TeaVM's loader beside
 * them), the asset pack, and every file's size and SHA-256. The loader asks for each file with its hash as the version
 * ({@code incari.wasm?v=...}), so the service worker keeps it exactly until a build changes it.
 *
 * <p>Run by {@code ./gradlew :app-web:buildManifest}:
 * {@code --out build.json --game 26.3 [--rust <folder>] --file <name>=<path>...}, where each name is
 * where the page finds the file (the dev server serves it there), and {@code --rust} is the Rust
 * build's folder, listed when it has a page.
 */
public final class BuildManifest {

    /**
     * The game's builds by the manifest's name for them (ADR 0036), each by the name the page fetches
     * its module by: Fast (JSPI) and Compatible (coroutines). Compatible keeps the name every build
     * had before, which an experiment's build (no manifest) still has.
     */
    static final SequencedMap<String, String> BUILDS = builds();

    /** TeaVM's loader, one file for both builds. */
    static final String RUNTIME = "incari.wasm-runtime.js";

    /**
     * The experimental Rust build's page (ADR 0068): Horizon's own page and files in {@code rust/},
     * which the loader opens instead of loading a module. Horizon's service worker keeps those files,
     * so they aren't in the file list.
     */
    static final String RUST_PAGE = "rust/index.html";

    static final String DEOBFUSCATOR = "-deobfuscator.wasm";
    static final String DEBUG_INFO = ".teadbg";

    private BuildManifest() {}

    private static SequencedMap<String, String> builds() {
        SequencedMap<String, String> builds = new LinkedHashMap<>();
        builds.put("fast", "incari-fast.wasm");
        builds.put("compatible", "incari.wasm");
        return Collections.unmodifiableSequencedMap(builds);
    }

    public static void main(String[] args) throws IOException {
        Path out = null;
        String game = null;
        boolean rust = false;
        Map<String, Path> files = new LinkedHashMap<>();
        for (int i = 0; i < args.length; i++) {
            switch (args[i]) {
                case "--out" -> out = Path.of(args[++i]);
                case "--game" -> game = args[++i];
                case "--rust" -> rust = Files.isRegularFile(Path.of(args[++i]).resolve("index.html"));
                case "--file" -> {
                    String option = args[++i];
                    int equals = option.indexOf('=');
                    if (equals <= 0) {
                        throw new CliException("Expected <name>=<path>, got '" + option + "'");
                    }
                    Path path = Path.of(option.substring(equals + 1));
                    // Optional files (a build not compiled, a debug build's deobfuscator) may be missing.
                    if (Files.isRegularFile(path)) {
                        files.put(option.substring(0, equals), path);
                    }
                }
                default -> throw new CliException("Unknown option '" + args[i] + "'");
            }
        }
        if (out == null || game == null) {
            throw new CliException(
                    "Usage: BuildManifest --out <file> --game <version> [--rust <folder>] [--file <name>=<path>]...");
        }
        JsonObject manifest = manifest(game, files, rust);
        Files.createDirectories(out.toAbsolutePath().getParent());
        Files.writeString(out, new GsonBuilder().setPrettyPrinting().create().toJson(manifest) + "\n");
        System.out.println("==> Wrote " + out + " (build "
                + manifest.get("id").getAsString().substring(0, 12) + ")");
    }

    static JsonObject manifest(String game, Map<String, Path> files, boolean rust) throws IOException {
        JsonArray list = new JsonArray();
        MessageDigest id = sha256();
        for (Map.Entry<String, Path> entry : files.entrySet()) {
            String hash = hash(entry.getValue());
            JsonObject file = new JsonObject();
            file.addProperty("path", entry.getKey());
            file.addProperty("size", Files.size(entry.getValue()));
            file.addProperty("sha256", hash);
            list.add(file);
            id.update((entry.getKey() + "=" + hash + "\n").getBytes(java.nio.charset.StandardCharsets.UTF_8));
        }
        JsonObject manifest = new JsonObject();
        manifest.addProperty("format", 2);
        manifest.addProperty("id", HexFormat.of().formatHex(id.digest()));
        manifest.addProperty("game", game);
        // A development build has TeaVM's stack deobfuscator beside it; a release build doesn't.
        boolean release = BUILDS.values().stream().noneMatch(module -> files.containsKey(module + DEOBFUSCATOR));
        manifest.addProperty("release", release);
        for (Map.Entry<String, String> build : BUILDS.entrySet()) {
            String module = build.getValue();
            if (!files.containsKey(module)) {
                continue;
            }
            JsonObject entry = new JsonObject();
            entry.addProperty("runtime", RUNTIME);
            entry.addProperty("module", module);
            if (!release && files.containsKey(module + DEBUG_INFO)) {
                entry.addProperty("deobfuscator", module + DEOBFUSCATOR);
                entry.addProperty("debugInfo", module + DEBUG_INFO);
            }
            manifest.add(build.getKey(), entry);
        }
        if (rust) {
            JsonObject entry = new JsonObject();
            entry.addProperty("page", RUST_PAGE);
            manifest.add("rust", entry);
        }
        // Programs the page's workers run beside the game: each its own module and loader.
        JsonObject programs = new JsonObject();
        for (String name : files.keySet()) {
            if (name.endsWith(".wasm") && !BUILDS.containsValue(name) && !name.endsWith(DEOBFUSCATOR)) {
                String program = name.substring(0, name.length() - ".wasm".length());
                if (files.containsKey(program + ".wasm-runtime.js")) {
                    JsonObject entry = new JsonObject();
                    entry.addProperty("runtime", program + ".wasm-runtime.js");
                    entry.addProperty("module", name);
                    programs.add(program, entry);
                }
            }
        }
        if (!programs.isEmpty()) {
            manifest.add("programs", programs);
        }
        if (files.containsKey("incari.pack")) {
            manifest.addProperty("pack", "incari.pack");
        }
        manifest.add("files", list);
        return manifest;
    }

    private static String hash(Path file) throws IOException {
        MessageDigest digest = sha256();
        byte[] buffer = new byte[1 << 20];
        try (InputStream in = Files.newInputStream(file)) {
            int read;
            while ((read = in.read(buffer)) > 0) {
                digest.update(buffer, 0, read);
            }
        }
        return HexFormat.of().formatHex(digest.digest());
    }

    private static MessageDigest sha256() {
        try {
            return MessageDigest.getInstance("SHA-256");
        } catch (NoSuchAlgorithmException e) {
            throw new IllegalStateException("Every JDK has SHA-256", e);
        }
    }
}
