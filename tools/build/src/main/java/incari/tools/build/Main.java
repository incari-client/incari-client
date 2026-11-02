package incari.tools.build;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * The {@code incari} build CLI. Run it through {@code ./incari} at the repo root.
 *
 * <p>Pipeline: {@code fetch} (Mojang jar + libraries) → {@code decompile} (Vineflower) →
 * {@code apply} (vendor commit, class manifest, patches into {@code work/client}).
 * {@code setup} runs all three. {@code rebuild} goes the other way, from commits in
 * {@code work/client} back to {@code patches/}. {@code build} compiles the browser build into a
 * folder for a web server ({@link WebBuild}).
 */
public final class Main {

    private static final String USAGE = """
            Usage: ./incari <command> [options]

            Commands:
              setup              fetch + decompile + apply: a fresh clone to a patched tree
              fetch              download the pinned Minecraft client jar and its libraries
              assets             download the game's assets (sounds, languages; ~480 MB), for the
                                 desktop runtime only
              decompile          decompile the client jar with Vineflower into work/decompiled
              apply [--force]    rebuild work/client from the decompiled sources and patches/
                                 (--force discards unsaved commits and edits in work/client)
              rebuild [--check]  regenerate patches/ from the commits in work/client
                                 (--check writes nothing, fails if patches/ is out of date)
              build [--out <dir>] [--only fast|compatible|rust] [--no-sounds] [--html]
                                 compile the release browser builds, Fast, Compatible and the
                                 experimental Rust (or just one with --only), into one folder for
                                 a static web server (default dist/web), or with --html into one
                                 HTML file that plays from the disk, without music or Rust
                                 (smaller sounds need ffmpeg) (default dist/incari.html).
                                 Rust is skipped, saying why, without its tools or work/horizon;
                                 INCARI_HORIZON_DIR builds another Horizon checkout.
                                 It holds Minecraft's code and files: for your own use, never
                                 shared or published
              survey             TeaVM compile survey, see tools/survey/README.md
              copycheck          fail if Incari code repeats decompiled game code, see tools/copycheck
              help               show this message
            """;

    private Main() {}

    public static void main(String[] args) {
        try {
            System.exit(run(List.of(args)));
        } catch (CliException e) {
            System.err.println("error: " + e.getMessage());
            System.exit(1);
        } catch (Exception e) {
            e.printStackTrace();
            System.exit(2);
        }
    }

    static int run(List<String> args) throws Exception {
        if (args.isEmpty() || args.getFirst().equals("help") || args.contains("--help") || args.contains("-h")) {
            System.out.print(USAGE);
            return args.isEmpty() ? 1 : 0;
        }
        String command = args.getFirst();
        List<String> flags = new ArrayList<>(args.subList(1, args.size()));

        Workspace ws = Workspace.discover(Path.of(System.getProperty("incari.cwd", ".")));
        BuildConfig config = BuildConfig.load(ws.repoRoot());

        switch (command) {
            case "setup" -> {
                noFlags(command, flags);
                GameFiles game = GameFiles.fetch(ws, config, new Downloader());
                Decompiler.decompile(ws, game);
                new ClientTree(ws).apply(ws.decompiledDir(config.minecraftVersion()), config.minecraftVersion(), false);
                GitHooks.install(ws);
                // The Rust build's source (ADR 0068). Optional: Fast and Compatible build without it.
                try {
                    Horizon.fetch(ws, Horizon.Pin.load(ws.repoRoot()));
                } catch (CliException e) {
                    Log.warn("Horizon not fetched, so ./incari build will skip the Rust build: " + e.getMessage());
                }
                Log.step("Done. The patched tree is in work/client");
            }
            case "fetch" -> {
                noFlags(command, flags);
                GameFiles.fetch(ws, config, new Downloader());
            }
            case "assets" -> {
                noFlags(command, flags);
                String index = Assets.fetch(ws, config, new Downloader());
                Log.step("Done. Asset index " + index + " is in work/cache/assets");
            }
            case "decompile" -> {
                noFlags(command, flags);
                Decompiler.decompile(ws, GameFiles.load(ws, config));
            }
            case "apply" -> {
                boolean force = flags.remove("--force");
                noFlags(command, flags);
                Path decompiled = ws.decompiledDir(config.minecraftVersion());
                if (!decompiled.resolve(".incari-decompile-stamp").toFile().isFile()) {
                    throw new CliException("No decompiled sources yet. Run: ./incari setup");
                }
                new ClientTree(ws).apply(decompiled, config.minecraftVersion(), force);
                Log.step("Done. The patched tree is in work/client");
            }
            case "rebuild" -> {
                boolean check = flags.remove("--check");
                noFlags(command, flags);
                ClientTree.RebuildResult result = new ClientTree(ws).rebuildPatches(check);
                Log.step((check ? "Checked" : "Rebuilt") + " patches: " + result.written() + " changed, "
                        + result.unchanged() + " unchanged, " + result.removed() + " removed");
                if (check && result.changed()) {
                    throw new CliException(
                            "patches/ doesn't match work/client. Run ./incari rebuild and commit the result.");
                }
            }
            case "build" -> {
                boolean sounds = !flags.remove("--no-sounds");
                boolean html = flags.remove("--html");
                // The single file holds TeaVM's builds only; Rust is its own page (ADR 0068).
                List<String> builds = html ? List.of("fast", "compatible") : List.of("fast", "compatible", "rust");
                boolean only = false;
                int onlyFlag = flags.indexOf("--only");
                if (onlyFlag >= 0) {
                    if (onlyFlag + 1 >= flags.size() || !builds.contains(flags.get(onlyFlag + 1))) {
                        throw new CliException(
                                html
                                        ? "--only takes fast or compatible with --html"
                                        : "--only takes fast, compatible or rust");
                    }
                    builds = List.of(flags.get(onlyFlag + 1));
                    only = true;
                    flags.subList(onlyFlag, onlyFlag + 2).clear();
                }
                Path out = ws.repoRoot().resolve("dist").resolve(html ? "incari.html" : "web");
                int outFlag = flags.indexOf("--out");
                if (outFlag >= 0) {
                    if (outFlag + 1 >= flags.size()) {
                        throw new CliException(html ? "--out needs a file name" : "--out needs a folder");
                    }
                    out = Path.of(System.getProperty("incari.cwd", ".")).resolve(flags.get(outFlag + 1));
                    flags.subList(outFlag, outFlag + 2).clear();
                }
                noFlags(command, flags);
                WebBuild.build(ws, config, out, builds, only, sounds, html);
            }
            case "survey", "copycheck" ->
                throw new CliException(
                        "'" + command + "' runs in its own JVM; use ./incari " + command + " (the launcher script)");
            default -> {
                System.err.print(USAGE);
                throw new CliException("Unknown command '" + command + "'");
            }
        }
        return 0;
    }

    private static void noFlags(String command, List<String> flags) {
        if (!flags.isEmpty()) {
            throw new CliException("Unknown option(s) for " + command + ": " + String.join(" ", flags));
        }
    }
}
