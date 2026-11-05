package incari.client.net;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import java.util.function.IntPredicate;
import java.util.function.ToIntFunction;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * Reads a server's refusal of the client's version, and the version it asks for instead ("Auto",
 * ADR 0018). Servers say it in words, in every form there is:
 *
 * <ul>
 *   <li>vanilla and Spigot 1.8: "Outdated server! I'm still on 1.8.8";
 *   <li>BungeeCord, of itself: "Outdated server! I'm still on 1.8.x-1.21.x";
 *   <li>BungeeCord, of its backend: "Kicked whilst connecting to lobby: Outdated server! ...";
 *   <li>modern vanilla: "Incompatible client! Please use 1.20.4" (a translation with the version
 *       as its argument, which reaches here as the argument's text);
 *   <li>EaglerXServer's version mismatch, as Incari reports it: "... protocols 47 to 340".
 * </ul>
 *
 * The answer is a protocol to try next, or -1 when the message isn't about versions or names none
 * the client can speak and hasn't tried.
 */
final class VersionRefusals {

    /** Words every version refusal contains one of; anything else is some other kick. */
    private static final Pattern ABOUT_VERSIONS = Pattern.compile(
            "outdated|incompatible|unsupported (?:client|version|protocol)|please use|still on|protocols? \\d+ to \\d+");
    /** A release number: 1.8, 1.8.8, 1.8.x, 26.3. */
    private static final Pattern VERSION =
            Pattern.compile("(?<![\\d.])(\\d{1,2}\\.\\d{1,2}(?:\\.(?:\\d{1,2}|x))?)(?![\\d.])");
    /** Incari's own report of an Eagler version mismatch: protocol numbers. */
    private static final Pattern PROTOCOL_RANGE = Pattern.compile("protocols? (\\d+) to (\\d+)");

    private VersionRefusals() {}

    /** Whether {@code message} is a server refusing the client's version, in any of the forms above. */
    static boolean isAboutVersions(String message) {
        return ABOUT_VERSIONS.matcher(message.toLowerCase(Locale.ROOT)).find();
    }

    /**
     * The protocol to join with next, after a server refused {@code tried} with {@code message}.
     *
     * @param message the refusal's text, with the arguments of any translation in it appended
     * @param attempted protocols already tried for this join, which are never offered again
     * @param named the protocol of a release named in words ("1.8.8", "1.21.x"), or -1
     * @param joinable whether the client can join a server of a protocol (its own, or through Via)
     */
    static int nextProtocol(
            String message, int tried, Set<Integer> attempted, ToIntFunction<String> named, IntPredicate joinable) {
        if (!isAboutVersions(message)) {
            return -1;
        }
        String text = message.toLowerCase(Locale.ROOT);
        List<Integer> candidates = new ArrayList<>();
        Matcher versions = VERSION.matcher(text);
        while (versions.find()) {
            candidates.add(named.applyAsInt(versions.group(1)));
        }
        Matcher range = PROTOCOL_RANGE.matcher(text);
        while (range.find()) {
            // The newest of a range is the one worth trying: the proxy passes it on to its backend,
            // which may be newer than the range's bottom.
            candidates.add(Integer.parseInt(range.group(2)));
        }
        int below = -1;
        int above = -1;
        for (int candidate : candidates) {
            if (candidate < 0 || candidate == tried || attempted.contains(candidate) || !joinable.test(candidate)) {
                continue;
            }
            if (candidate < tried) {
                below = Math.max(below, candidate);
            } else if (above < 0 || candidate < above) {
                above = candidate;
            }
        }
        // The client is the newest release there is, so a refusal nearly always means an older
        // server: the newest version below is the likeliest. A newer one (a server ahead of the
        // client, through ViaBackwards) only when nothing below is offered.
        return below >= 0 ? below : above;
    }
}
