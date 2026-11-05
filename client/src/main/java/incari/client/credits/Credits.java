package incari.client.credits;

import com.viaversion.viaversion.util.VersionInfo;
import incari.client.IncariBuild;
import java.net.URI;
import java.util.ArrayList;
import java.util.List;

/**
 * What the credits screen lists: Incari and the third-party software it adds to the game, in
 * every build. Minecraft's own libraries are Mojang's to credit, which the vanilla screen beside
 * this one does.
 *
 * <p>A build adds what only it contains (the browser build: TeaVM's runtime) with {@link #add},
 * at start-up, before the game's first screen. Adding a library to every build means adding it to
 * {@code COMMON} here.
 */
public final class Credits {

    /**
     * Incari's public source (incari.properties' {@code incari.repository}). GPL-3.0's source offer
     * for every build points here.
     */
    public static final URI INCARI_SOURCE = URI.create(IncariBuild.REPOSITORY);

    /** The pause menu's Report Bugs: a new issue on the repository, from its bug template. */
    public static final URI REPORT_BUGS = URI.create(IncariBuild.REPOSITORY + "/issues/new?template=bug_report.yml");

    /** The pause menu's Give Feedback: a new discussion in the repository's Feedback category. */
    public static final URI FEEDBACK = URI.create(IncariBuild.REPOSITORY + "/discussions/new?category=feedback");

    public static final URI GPL_3 = URI.create("https://www.gnu.org/licenses/gpl-3.0.html");
    public static final URI APACHE_2 = URI.create("https://www.apache.org/licenses/LICENSE-2.0");
    public static final URI BSD_3_CLAUSE = URI.create("https://opensource.org/license/bsd-3-clause");
    public static final URI OFL_1_1 = URI.create("https://openfontlicense.org/open-font-license-official-text/");

    private static final List<Credit> COMMON = List.of(
            new Credit(
                    "Incari Client",
                    null,
                    "The Incari Client contributors",
                    "GPL-3.0-or-later, with an exception for Minecraft",
                    URI.create(INCARI_SOURCE + "/blob/main/LICENSE-EXCEPTION.txt"),
                    INCARI_SOURCE),
            // VERSION is a constant, so these are the versions the client was compiled against,
            // which are the ones the build bundles.
            new Credit(
                    "ViaVersion",
                    VersionInfo.VERSION,
                    "ViaVersion and contributors",
                    "GPL-3.0",
                    GPL_3,
                    URI.create("https://github.com/ViaVersion/ViaVersion")),
            new Credit(
                    "ViaBackwards",
                    com.viaversion.viabackwards.utils.VersionInfo.VERSION,
                    "ViaVersion and contributors",
                    "GPL-3.0",
                    GPL_3,
                    URI.create("https://github.com/ViaVersion/ViaBackwards")),
            // Microsoft sign-in's TLS, and the browser build's AES, RSA and chat signing (ADR 0070).
            new Credit(
                    "Bouncy Castle",
                    null,
                    "The Legion of the Bouncy Castle Inc.",
                    "MIT",
                    URI.create("https://www.bouncycastle.org/about/license/"),
                    URI.create("https://github.com/bcgit/bc-java")),
            // The look's fonts, in Incari's pack with their licences beside them.
            new Credit(
                    "Red Hat Display",
                    null,
                    "The Red Hat Project Authors",
                    "OFL-1.1",
                    OFL_1_1,
                    URI.create("https://github.com/RedHatOfficial/RedHatFont")),
            new Credit(
                    "Michroma",
                    null,
                    "The Michroma Project Authors",
                    "OFL-1.1",
                    OFL_1_1,
                    URI.create("https://github.com/googlefonts/Michroma-font")),
            // A theme's other fonts, subset to Red Hat Display's characters.
            new Credit(
                    "Inter",
                    null,
                    "The Inter Project Authors",
                    "OFL-1.1",
                    OFL_1_1,
                    URI.create("https://github.com/rsms/inter")),
            new Credit(
                    "Nunito",
                    null,
                    "The Nunito Project Authors",
                    "OFL-1.1",
                    OFL_1_1,
                    URI.create("https://github.com/googlefonts/nunito")),
            new Credit(
                    "Poppins",
                    null,
                    "The Poppins Project Authors",
                    "OFL-1.1",
                    OFL_1_1,
                    URI.create("https://github.com/itfoundry/Poppins")),
            new Credit(
                    "JetBrains Mono",
                    null,
                    "The JetBrains Mono Project Authors",
                    "OFL-1.1",
                    OFL_1_1,
                    URI.create("https://github.com/JetBrains/JetBrainsMono")));

    // Written at start-up, read by the screen later on the same thread.
    private static final List<Credit> added = new ArrayList<>();

    private Credits() {}

    /** Adds software only this build contains. */
    public static void add(Credit credit) {
        added.add(credit);
    }

    /** Everything this build credits: Incari's own entries first, then what the build added. */
    public static List<Credit> all() {
        List<Credit> all = new ArrayList<>(COMMON);
        all.addAll(added);
        return List.copyOf(all);
    }
}
