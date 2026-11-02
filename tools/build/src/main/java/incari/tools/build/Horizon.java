package incari.tools.build;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.IOException;
import java.io.Reader;
import java.net.URI;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Properties;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;
import org.jspecify.annotations.Nullable;

/**
 * The experimental Rust build (ADR 0068): Horizon's browser port with Incari's native mods.
 *
 * <p>Horizon is pinned in {@code incari.properties} and cloned into {@code work/horizon}, never
 * committed, as the game is (ADR 0001). {@code INCARI_HORIZON_DIR} names a checkout to build
 * instead, for work on Horizon itself. {@link #build} prepares Horizon's resources from the game
 * files {@code ./incari fetch} and {@code ./incari assets} downloaded, applies {@code rust/mods}
 * with horizon-mixins into {@code work/horizon-stage}, and runs Horizon's {@code web/package.py} there.
 * The folder it writes, {@code app-web/build/rust/}, is what the page offers as Rust.
 */
final class Horizon {

    /** A checkout to build instead of the pinned one. */
    static final String DIR_VARIABLE = "INCARI_HORIZON_DIR";

    /** What {@code incari.properties} pins. */
    record Pin(String repository, String revision, String mixinsRepository, String mixinsRevision) {

        static Pin load(Path repoRoot) throws IOException {
            Path file = repoRoot.resolve(BuildConfig.FILE_NAME);
            Properties props = new Properties();
            try (Reader reader = Files.newBufferedReader(file)) {
                props.load(reader);
            }
            return new Pin(
                    require(props, file, "horizon.repository"),
                    require(props, file, "horizon.revision"),
                    require(props, file, "horizon.mixins.repository"),
                    require(props, file, "horizon.mixins.revision"));
        }

        private static String require(Properties props, Path file, String key) {
            String value = props.getProperty(key);
            if (value == null || value.isBlank()) {
                throw new CliException("Missing '" + key + "' in " + file);
            }
            return value.trim();
        }
    }

    private Horizon() {}

    static Path pinnedSource(Workspace ws) {
        return ws.workDir().resolve("horizon");
    }

    static Path stage(Workspace ws) {
        return ws.workDir().resolve("horizon-stage");
    }

    /** Where the browser build lands, for {@code :app-web:serve} and {@code :app-web:webFolder}. */
    static Path output(Workspace ws) {
        return ws.repoRoot().resolve("app-web/build/rust");
    }

    static Path mods(Workspace ws) {
        return ws.repoRoot().resolve("rust/mods");
    }

    /** The checkout {@code INCARI_HORIZON_DIR} names, or null to use the pinned one. */
    static @Nullable Path override() {
        String dir = System.getenv(DIR_VARIABLE);
        if (dir == null || dir.isBlank()) {
            return null;
        }
        Path path = Path.of(System.getProperty("incari.cwd", "."))
                .resolve(dir)
                .toAbsolutePath()
                .normalize();
        if (!Files.isRegularFile(path.resolve("web/package.py"))) {
            throw new CliException(
                    DIR_VARIABLE + "=" + dir + " isn't a checkout of Horizon's browser port" + " (no web/package.py)");
        }
        return path;
    }

    /**
     * Clones the pinned commit into {@code work/horizon}, or moves the clone to it. Refuses to move
     * a clone with changes in it: unsaved work is never dropped.
     */
    static Path fetch(Workspace ws, Pin pin) throws IOException, InterruptedException {
        Path dir = pinnedSource(ws);
        if (!Files.isDirectory(dir.resolve(".git"))) {
            Log.step("Cloning Horizon " + pin.revision().substring(0, 12) + " into work/horizon");
            Files.createDirectories(dir);
            git(dir, "init", "--quiet");
            git(dir, "remote", "add", "origin", pin.repository());
        } else if (head(dir).equals(pin.revision())) {
            return dir;
        } else if (!git(dir, "status", "--porcelain").stdout().isBlank()) {
            throw new CliException("work/horizon has changes and isn't at the pinned commit. Commit them in your"
                    + " Horizon fork, or set " + DIR_VARIABLE + " to build a checkout of your own");
        } else {
            Log.step("Moving work/horizon to " + pin.revision().substring(0, 12));
            git(dir, "remote", "set-url", "origin", pin.repository());
        }
        git(dir, "fetch", "--quiet", "--depth", "1", "origin", pin.revision());
        git(dir, "checkout", "--quiet", "--detach", "FETCH_HEAD");
        if (!head(dir).equals(pin.revision())) {
            throw new CliException(
                    "Fetched " + head(dir) + " but " + BuildConfig.FILE_NAME + " pins " + pin.revision());
        }
        return dir;
    }

    /**
     * What the Rust build needs that this machine lacks, each with how to get it; empty when it can
     * build. Reads the versions from the source: the nightly {@code web/build.sh} names, and the
     * wasm-bindgen of its {@code Cargo.lock}.
     */
    static List<String> missingTools(Path source) throws IOException, InterruptedException {
        List<String> missing = new ArrayList<>();
        if (!runs("python3", "--version")) {
            missing.add("python3");
        }
        if (!runs("rustup", "--version") || !runs("cargo", "--version")) {
            missing.add("Rust (https://rustup.rs)");
            return missing;
        }
        String nightly = match(Files.readString(source.resolve("web/build.sh")), "(?m)^nightly=(\\S+)");
        String components = ProcessRunner.start(
                        List.of("rustup", "component", "list", "--installed", "--toolchain", nightly),
                        source,
                        Map.of(),
                        false,
                        null)
                .stdout();
        if (!components.contains("rust-src") || !components.contains("rust-std-wasm32-unknown-unknown")) {
            missing.add("Rust " + nightly + ": rustup toolchain install " + nightly
                    + " --component rust-src --target wasm32-unknown-unknown");
        }
        String bindgen = match(
                Files.readString(source.resolve("Cargo.lock")), "name = \"wasm-bindgen\"\\nversion = \"([^\"]+)\"");
        ProcessRunner.Result installed =
                ProcessRunner.start(List.of("wasm-bindgen", "--version"), source, Map.of(), false, null);
        if (installed.exitCode() != 0 || !installed.stdout().strip().equals("wasm-bindgen " + bindgen)) {
            missing.add("wasm-bindgen " + bindgen + ": cargo install wasm-bindgen-cli --version " + bindgen);
        }
        if (!runs("wasm-opt", "--version")) {
            missing.add("binaryen's wasm-opt (your package manager's binaryen)");
        }
        return missing;
    }

    /**
     * Builds the Rust build into {@link #output}: Horizon's resources, the mods, both of its
     * WebAssembly builds (threaded, and single-threaded for pages that can't share memory).
     */
    static void build(Workspace ws, BuildConfig config, Path checkout, Pin pin)
            throws IOException, InterruptedException {
        Path source = checkout.equals(pinnedSource(ws)) ? checkout : snapshot(ws, checkout);
        Path resources = prepare(ws, config, source);
        incariPack(ws, resources);
        Path tool = mixins(ws, pin);
        Path stage = stage(ws);
        // web/package.py writes its builds inside the source it runs in, and horizon-mixins refuses
        // to restage over them (a symlink among them). They were copied out after the last build.
        for (String built : List.of("web/dist", "web/dist-st", "web/out")) {
            FileTrees.deleteRecursively(stage.resolve(built));
        }
        Log.step("Applying rust/mods to Horizon (horizon-mixins)");
        // horizon-mixins rewrites the stage in place, keeping unchanged files' times, so Cargo
        // recompiles only what the mods touch.
        ProcessRunner.run(
                List.of(
                        tool.toString(),
                        "apply",
                        "--workspace",
                        source.toString(),
                        "--mods",
                        mods(ws).toString(),
                        "--out",
                        stage.toString()),
                ws.repoRoot(),
                false);
        Log.step("Compiling Horizon for the browser (threaded and single-threaded; long the first time)");
        Path packaged = ws.workDir().resolve("horizon-out");
        ProcessRunner.Result result = ProcessRunner.start(
                List.of("python3", "web/package.py", "folder", "--out", packaged.toString()),
                stage,
                Map.of(
                        "HORIZON_RESOURCES", resources.toString(),
                        // Outside the stage, which horizon-mixins owns.
                        "CARGO_TARGET_DIR",
                                ws.workDir().resolve("horizon-target/web").toString()),
                true,
                null);
        if (result.exitCode() != 0) {
            throw new CliException("Horizon's web build failed (exit " + result.exitCode() + "); its output is above");
        }
        Path out = output(ws);
        FileTrees.deleteRecursively(out);
        FileTrees.copyTree(packaged.resolve("horizon-web"), out, "");
        brandPage(ws, out);
        Log.step("Rust build in " + ws.repoRoot().relativize(out));
    }

    /** The tab's icon, served beside the page. */
    private static final String TAB_ICON = "icon.png";

    /** Horizon's page is titled "Horizon" with no icon; the tab says "Incari Client" with Incari's icon, as the main page does. */
    static void brandPage(Workspace ws, Path out) throws IOException {
        Files.copy(
                ws.repoRoot().resolve(PACK).resolve("assets/incari/textures/gui/icon_256.png"),
                out.resolve(TAB_ICON),
                StandardCopyOption.REPLACE_EXISTING);
        Path page = out.resolve("index.html");
        String html = Files.readString(page);
        String branded = html.replace(
                "<title>Horizon</title>",
                "<title>Incari Client</title>\n<link rel=\"icon\" type=\"image/png\" href=\"" + TAB_ICON + "\">");
        if (branded.equals(html)) {
            throw new CliException(page + " has no <title>Horizon</title> to rename; Horizon's page changed");
        }
        Files.writeString(page, branded);
    }

    /**
     * Horizon's resources, from the jars and assets {@code ./incari setup} and {@code assets} fetched:
     * {@code work/horizon-resources/<version>}. Skipped when {@code horizon-prepare --check} says
     * they're complete for this preparer.
     */
    static Path prepare(Workspace ws, BuildConfig config, Path source) throws IOException, InterruptedException {
        String version = config.minecraftVersion();
        Path out = ws.workDir().resolve("horizon-resources").resolve(version);
        Map<String, String> env = Map.of(
                "CARGO_TARGET_DIR", ws.workDir().resolve("horizon-target/tools").toString());
        List<String> prepare = List.of("cargo", "run", "--quiet", "--release", "-p", "horizon-prepare", "--");
        List<String> check = new ArrayList<>(prepare);
        check.addAll(List.of("--check", "--root", source.toString(), "--output", out.toString()));
        if (ProcessRunner.start(check, source, env, false, null).exitCode() == 0) {
            return out;
        }
        GameFiles game = GameFiles.load(ws, config);
        Log.step("Checking the game's sounds (./incari assets)");
        Assets.fetch(ws, config, new Downloader());
        Path serverJar = serverJar(ws, config);
        Log.step("Preparing Horizon's resources from Minecraft " + version + " (several minutes, once)");
        List<String> run = new ArrayList<>(prepare);
        run.addAll(List.of(
                "--prepare",
                "--complete",
                "--root",
                source.toString(),
                "--output",
                out.toString(),
                "--version-json",
                ws.versionJson(version).toString(),
                "--client-jar",
                game.clientJar().toString(),
                "--libraries-dir",
                ws.librariesDir().toString(),
                "--assets-dir",
                ws.assetsDir().toString(),
                "--server-jar",
                serverJar.toString(),
                "--java-home",
                System.getProperty("java.home")));
        ProcessRunner.Result result = ProcessRunner.start(run, source, env, true, null);
        if (result.exitCode() != 0) {
            throw new CliException("horizon-prepare failed (exit " + result.exitCode() + "); its output is above");
        }
        return out;
    }

    /** Incari's pack, as the Java client serves it: listed in {@code files.txt}, by path in the pack. */
    private static final String PACK = "client/src/main/resources/incari-pack";

    /** The pack's files {@code rust/mods/incari} compiles in (ADR 0069), into its git-ignored {@code data/}. */
    private static final List<String> BUNDLE_DATA = List.of(
            "assets/incari/textures/gui/icon_256.png",
            "assets/incari/textures/gui/title_backdrop.png",
            "assets/incari/font/michroma.ttf");

    /**
     * Incari's pack over Horizon's prepared resources, as the Java client always has it on: its
     * fonts, the game's default font made Incari's (ADR 0069). Also the files the client's bundle
     * compiles in. One copy of each in the repo: copied at each build.
     */
    static void incariPack(Workspace ws, Path resources) throws IOException {
        Path pack = ws.repoRoot().resolve(PACK);
        for (String line : Files.readAllLines(pack.resolve("files.txt"))) {
            String entry = line.strip();
            if (entry.isEmpty() || entry.startsWith("#")) {
                continue;
            }
            int eq = entry.indexOf('=');
            String served = (eq < 0 ? entry : entry.substring(0, eq)).strip();
            String file = (eq < 0 ? entry : entry.substring(eq + 1)).strip();
            Path to = resources.resolve(served);
            Files.createDirectories(to.getParent());
            Files.copy(pack.resolve(file), to, StandardCopyOption.REPLACE_EXISTING);
        }
        Path data = mods(ws).resolve("incari").resolve("data");
        Files.createDirectories(data);
        for (String file : BUNDLE_DATA) {
            Path from = pack.resolve(file);
            Files.copy(from, data.resolve(from.getFileName()), StandardCopyOption.REPLACE_EXISTING);
        }
        // The version, repository and featured servers the client shows.
        Files.copy(
                ws.repoRoot().resolve(BuildConfig.FILE_NAME),
                data.resolve(BuildConfig.FILE_NAME),
                StandardCopyOption.REPLACE_EXISTING);
    }

    /** The server jar, which Horizon's resources are exported from too, checked against its SHA-1. */
    private static Path serverJar(Workspace ws, BuildConfig config) throws IOException, InterruptedException {
        Path versionJson = ws.versionJson(config.minecraftVersion());
        JsonObject server = JsonParser.parseString(Files.readString(versionJson))
                .getAsJsonObject()
                .getAsJsonObject("downloads")
                .getAsJsonObject("server");
        if (server == null) {
            throw new CliException(versionJson + " names no server jar");
        }
        Path jar = ws.versionDir(config.minecraftVersion()).resolve("server.jar");
        new Downloader()
                .fetch(
                        URI.create(server.get("url").getAsString()),
                        server.get("sha1").getAsString(),
                        jar);
        return jar;
    }

    /** horizon-mixins at the pinned commit, built once into {@code work/cache/horizon-mixins/<commit>}. */
    private static Path mixins(Workspace ws, Pin pin) throws IOException, InterruptedException {
        Path root = ws.cacheDir().resolve("horizon-mixins").resolve(pin.mixinsRevision());
        boolean windows = System.getProperty("os.name").startsWith("Windows");
        Path tool = root.resolve("bin").resolve(windows ? "horizon-mixins.exe" : "horizon-mixins");
        if (Files.isRegularFile(tool)) {
            return tool;
        }
        Log.step("Building horizon-mixins " + pin.mixinsRevision().substring(0, 12) + " (once)");
        ProcessRunner.run(
                List.of(
                        "cargo",
                        "install",
                        "--quiet",
                        "--locked",
                        "--git",
                        pin.mixinsRepository(),
                        "--rev",
                        pin.mixinsRevision(),
                        "--root",
                        root.toString(),
                        "horizon-mixins-cli"),
                ws.repoRoot(),
                true);
        return tool;
    }

    /**
     * The files of a checkout {@code INCARI_HORIZON_DIR} names, as git sees them (tracked, and new
     * ones it doesn't ignore), copied into {@code work/horizon-override}: horizon-mixins reads every
     * file of its workspace, and a checkout's own web build leaves a symlink in its ignored
     * {@code web/dist} that it refuses. Unchanged files keep their times, so the stage stays
     * incremental.
     */
    static Path snapshot(Workspace ws, Path checkout) throws IOException, InterruptedException {
        Path copy = ws.workDir().resolve("horizon-override");
        Log.step("Copying " + checkout + " into work/horizon-override");
        Set<Path> kept = new HashSet<>();
        String names = git(checkout, "ls-files", "-z", "--cached", "--others", "--exclude-standard")
                .stdout();
        for (String name : names.split("\0", -1)) {
            Path from = checkout.resolve(name);
            // A tracked file deleted in the checkout, or a link: neither is staged.
            if (name.isEmpty() || !Files.isRegularFile(from, LinkOption.NOFOLLOW_LINKS)) {
                continue;
            }
            Path to = copy.resolve(name);
            kept.add(to);
            if (Files.isRegularFile(to, LinkOption.NOFOLLOW_LINKS)
                    && Files.size(to) == Files.size(from)
                    && Files.getLastModifiedTime(to).equals(Files.getLastModifiedTime(from))) {
                continue;
            }
            Files.createDirectories(to.getParent());
            Files.copy(from, to, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.COPY_ATTRIBUTES);
        }
        List<Path> stale;
        try (Stream<Path> files = Files.walk(copy)) {
            stale = files.filter(f -> !Files.isDirectory(f, LinkOption.NOFOLLOW_LINKS) && !kept.contains(f))
                    .toList();
        }
        for (Path file : stale) {
            Files.delete(file);
        }
        return copy;
    }

    private static String head(Path dir) throws IOException, InterruptedException {
        ProcessRunner.Result result = ProcessRunner.start(
                List.of("git", "rev-parse", "--verify", "--quiet", "HEAD"), dir, Map.of(), false, null);
        return result.exitCode() == 0 ? result.stdout().strip() : "";
    }

    private static ProcessRunner.Result git(Path dir, String... args) throws IOException, InterruptedException {
        List<String> command = new ArrayList<>(List.of("git"));
        command.addAll(Arrays.asList(args));
        return ProcessRunner.run(command, dir, false);
    }

    private static boolean runs(String... command) {
        try {
            return ProcessRunner.start(List.of(command), Path.of("."), Map.of(), false, null)
                            .exitCode()
                    == 0;
        } catch (IOException e) {
            return false;
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return false;
        }
    }

    static String match(String text, String regex) {
        Matcher m = Pattern.compile(regex).matcher(text);
        if (!m.find()) {
            throw new CliException("Horizon's source changed: no match for " + regex);
        }
        return m.group(1);
    }
}
