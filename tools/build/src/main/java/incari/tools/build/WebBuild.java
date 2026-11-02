package incari.tools.build;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * {@code ./incari build} (ADR 0028): from a set-up checkout to one folder a static web server
 * can serve. Compiles the game's two release builds with Gradle (ADR 0036), Compatible
 * ({@code :app-web:teavm -Pteavm.release}) and Fast (the same with {@code -Pteavm.fast}), or the one
 * {@code --only} names, then {@code :app-web:webFolder} copies them, the page, the client jar and the
 * sounds into the folder. With both, the page lets the player pick, Fast recommended. The
 * experimental Rust build ({@link Horizon}, ADR 0068) goes in its {@code rust/} folder when this
 * machine can build it; otherwise it's skipped, saying why.
 *
 * <p>With {@code --html} it packs that folder, with every sound but the music encoded smaller (about
 * 38 MB of 377; ffmpeg, else only the common sounds), into one HTML file that plays from the disk ({@code :app-web:singleFile}, ADR 0057), writing the folder under
 * {@code app-web/build/} on the way.
 *
 * <p>The folder holds Mojang's code and files. It is for the person who built it: never committed,
 * never shared or published. Inside the checkout it must be git-ignored, so it can't be committed.
 */
final class WebBuild {

    static final String NOTICE = """

            ------------------------------------------------------------------------------------
            DO NOT SHARE, UPLOAD OR PUBLISH THIS BUILD.
            It contains Minecraft's code and files, which Mojang doesn't allow redistributing.
            The Incari project never publishes builds and gives nobody permission to. It's for
            your own use; if you serve it, it's your server and your responsibility.
            See docs/self-hosting.md (and README.txt in the folder).
            ------------------------------------------------------------------------------------
            """;

    private WebBuild() {}

    /**
     * @param only true when {@code --only} named the one build: a Rust build that can't be made
     *     fails instead of being skipped
     * @param html true for one HTML file at {@code out} instead of a folder
     */
    static void build(
            Workspace ws, BuildConfig config, Path out, List<String> builds, boolean only, boolean sounds, boolean html)
            throws IOException, InterruptedException {
        if (!Files.isDirectory(ws.clientTree())) {
            throw new CliException("No patched tree in work/client yet. Run: ./incari setup");
        }
        out = out.toAbsolutePath().normalize();
        requireIgnored(ws, out, !html);
        // The single file is packed from a folder of the build's own, with its sounds smaller.
        Path folder = html ? ws.repoRoot().resolve("app-web/build/web-single") : out;
        if (sounds) {
            Log.step("Checking the game's sounds (./incari assets)");
            Assets.fetch(ws, config, new Downloader());
        }
        builds = new ArrayList<>(builds);
        if (builds.contains("rust") && !rust(ws, config, only)) {
            builds.remove("rust");
        }
        if (builds.contains("compatible")) {
            Log.step("Compiling the Compatible build (release; about ten minutes when the code changed)");
            gradle(ws, List.of(":app-web:teavm", "-Pteavm.release"));
        }
        if (builds.contains("fast")) {
            Log.step("Compiling the Fast build (release; about ten minutes when the code changed)");
            gradle(ws, List.of(":app-web:teavm", "-Pteavm.release", "-Pteavm.fast"));
        }
        Log.step("Writing " + out);
        List<String> task = new ArrayList<>(List.of(
                ":app-web:webFolder",
                "-Pteavm.release",
                "-Pweb.skipBuild",
                "-Pweb.builds=" + String.join(",", builds),
                "-Pweb.out=" + folder));
        if (!sounds) {
            task.add("-Pweb.noSounds");
        }
        if (html) {
            task.addAll(List.of(":app-web:singleFile", "-Pweb.smallSounds", "-Pweb.html=" + out));
        }
        gradle(ws, task);
        Log.step(
                html
                        ? "Done. Open " + out
                                + " in Chrome, Edge or Firefox; it plays without a server"
                                + (sounds ? " (no music)" : " (and without sounds)")
                        : "Done. Serve " + out + " with any static web server; its README.txt says how");
        System.out.print(NOTICE);
    }

    /**
     * Builds Rust (ADR 0068) into {@code app-web/build/rust/}, or says why it can't and returns false.
     * With {@code only}, a reason is an error.
     */
    private static boolean rust(Workspace ws, BuildConfig config, boolean only)
            throws IOException, InterruptedException {
        Horizon.Pin pin = Horizon.Pin.load(ws.repoRoot());
        Path override = Horizon.override();
        Path source = null;
        List<String> missing;
        try {
            source = override != null ? override : Horizon.fetch(ws, pin);
            missing = Horizon.missingTools(source);
        } catch (CliException e) {
            missing = List.of(e.getMessage());
        }
        if (!missing.isEmpty()) {
            String why = "The Rust build needs: " + String.join("; ", missing);
            if (only) {
                throw new CliException(why);
            }
            Log.warn(why + ". Skipping it; Fast and Compatible still build");
            return false;
        }
        if (source == null) {
            throw new IllegalStateException("A source with nothing missing");
        }
        Horizon.build(ws, config, source, pin);
        return true;
    }

    /**
     * Refuses an output folder (or file) inside the checkout that git would track: the build must
     * never be committed. The default, {@code dist/}, is ignored.
     */
    static void requireIgnored(Workspace ws, Path out, boolean folder) throws IOException, InterruptedException {
        if (!out.startsWith(ws.repoRoot())) {
            return;
        }
        if (out.equals(ws.repoRoot())) {
            throw new CliException(
                    "The build can't go in the checkout's root. Use the default under dist/, or somewhere outside it");
        }
        ProcessRunner.Result ignored = ProcessRunner.start(
                List.of("git", "check-ignore", "-q", "--no-index", ws.repoRoot().relativize(out) + (folder ? "/" : "")),
                ws.repoRoot(),
                Map.of(),
                false,
                null);
        if (ignored.exitCode() != 0) {
            throw new CliException(out + " is inside the checkout and not git-ignored, so the build could be "
                    + "committed. Use the default under dist/, or somewhere outside the checkout");
        }
    }

    private static void gradle(Workspace ws, List<String> arguments) throws IOException, InterruptedException {
        boolean windows = System.getProperty("os.name").startsWith("Windows");
        Path wrapper = ws.repoRoot().resolve(windows ? "gradlew.bat" : "gradlew");
        List<String> command = new ArrayList<>(List.of(wrapper.toString(), "--console=plain"));
        command.addAll(arguments);
        ProcessRunner.run(command, ws.repoRoot(), true);
    }
}
