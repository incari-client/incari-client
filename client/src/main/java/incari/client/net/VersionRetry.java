package incari.client.net;

import com.mojang.logging.LogUtils;
import incari.via.IncariVia;
import java.nio.file.Path;
import java.util.HashSet;
import java.util.Set;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.ConnectScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.multiplayer.ServerData;
import net.minecraft.client.multiplayer.resolver.ServerAddress;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.contents.TranslatableContents;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;

/**
 * "Auto" that finds a server's version by joining (ADR 0018). When nothing says which version a
 * server speaks (an Eagler server's MOTD never does; a proxy's ping reports the client's own), the
 * client joins as itself. A server of another version refuses in words ("Outdated server! I'm
 * still on 1.8.8"); this reads the version from the refusal and joins again with it through Via.
 * An Eagler proxy that can't speak the client's version drops the login without a word; then 1.8
 * is tried, whose refusal by a newer backend names the right version. The version a join reaches
 * the world with is remembered for the address, so later joins go straight there.
 *
 * <p>Only a join whose version the player left on "Auto" is retried, at most {@link #MAX_JOINS}
 * times, never with a version it already tried, and only for a refusal soon after connecting.
 */
public final class VersionRetry {

    private static final Logger LOGGER = LogUtils.getLogger();
    /** Joins per attempt at a server: one proxy range and one backend refusal fit comfortably. */
    private static final int MAX_JOINS = 4;
    /** A refusal comes while logging in; a kick later on is something else. */
    private static final long WINDOW_NANOS = TimeUnit.SECONDS.toNanos(60);
    /** Versions to try, in order, when an Eagler proxy drops a login without a word: 1.8, 1.12.2. */
    private static final int[] EAGLER_GUESSES = {47, 340};

    /** One attempt at a server, over however many joins it takes. */
    private static final class Attempt {
        final ServerData server;
        final ServerAddress address;
        final Set<Integer> tried = new HashSet<>();
        int protocol;
        long startedNanos;
        /** The version the next join of this attempt must use, set by a retry. */
        int retryProtocol = -1;

        Attempt(ServerData server, ServerAddress address) {
            this.server = server;
            this.address = address;
        }
    }

    private static final Object LOCK = new Object();
    private static @Nullable Attempt current;
    private static @Nullable LearnedVersions learned;

    private VersionRetry() {}

    /**
     * {@link ServerConnections#prepare}: the version a retry chose for this join of {@code server},
     * or -1 when this is a fresh join.
     */
    static int retryProtocol(ServerData server) {
        synchronized (LOCK) {
            Attempt attempt = current;
            return attempt != null && attempt.server.equals(server) ? attempt.retryProtocol : -1;
        }
    }

    /** {@link ServerConnections#prepare}: the connect screen is joining {@code server} as {@code protocol}. */
    static void joining(ServerData server, ServerAddress address, int protocol) {
        synchronized (LOCK) {
            Attempt attempt = current;
            if (attempt == null || !attempt.server.equals(server) || attempt.retryProtocol < 0) {
                attempt = new Attempt(server, address);
                current = attempt;
            }
            attempt.retryProtocol = -1;
            attempt.protocol = protocol;
            attempt.startedNanos = System.nanoTime();
            attempt.tried.add(protocol);
        }
    }

    /** The version {@code address} was found to speak when it was last joined on "Auto", or -1. */
    static int learned(String address) {
        return learnedVersions().get(address);
    }

    /**
     * {@code DisconnectedScreen.init}: if this disconnection is a server refusing the version an
     * "Auto" join used, starts the next join with the version it asked for and returns true (the
     * screen then shows nothing: the connect screen replaces it). Render thread.
     */
    public static boolean retry(Minecraft minecraft, Screen parent, Component reason) {
        Attempt attempt;
        int next;
        synchronized (LOCK) {
            attempt = current;
            if (attempt == null
                    || attempt.server.incariVersion != ServerVersions.AUTO
                    || attempt.retryProtocol >= 0
                    || attempt.tried.size() >= MAX_JOINS
                    || System.nanoTime() - attempt.startedNanos > WINDOW_NANOS) {
                return false;
            }
            next = VersionRefusals.nextProtocol(
                    textOf(reason),
                    attempt.protocol,
                    attempt.tried,
                    IncariVia::protocolNamed,
                    ServerVersions::isCompatible);
            if (next < 0
                    && ServerConnections.lastJoinDroppedDuringLogin()
                    && dropIsAHint(attempt.protocol, learned(attempt.server.ip))) {
                next = fallback(attempt.tried);
            }
            if (next < 0) {
                current = null;
                return false;
            }
            attempt.retryProtocol = next;
        }
        LOGGER.info(
                "{} refused {} (\"{}\"); joining again as {}",
                attempt.address,
                ServerVersions.name(attempt.protocol),
                reason.getString(),
                ServerVersions.name(next));
        ServerData server = attempt.server;
        ServerAddress address = attempt.address;
        // After this screen's init: starting a connection replaces the screen being set up.
        minecraft.execute(() -> ConnectScreen.startConnecting(parent, minecraft, address, server, false, null));
        return true;
    }

    /**
     * {@link ServerConnections}: a join the connect screen prepared reached the world as {@code
     * protocol}. An "Auto" join remembers it for the address; the attempt is over either way.
     */
    static void joined(int protocol) {
        Attempt attempt;
        synchronized (LOCK) {
            attempt = current;
            current = null;
        }
        if (attempt != null && attempt.server.incariVersion == ServerVersions.AUTO) {
            learnedVersions().put(attempt.server.ip, protocol);
        }
    }

    /**
     * Whether a silent drop of a join as {@code joinedAs} hints at the wrong version. Not when this
     * address already reached the world with it ({@code learned}): then the drop is something else,
     * such as a rejoin right after a kick that the server turns away while the old session lingers,
     * and guessing 1.8 there joins a multi-version server as 1.8 and teaches it for good.
     */
    static boolean dropIsAHint(int joinedAs, int learned) {
        return joinedAs != learned;
    }

    /**
     * The next guess when an Eagler proxy dropped the login without naming a version: 1.8, which
     * nearly every Eagler server runs (and a newer backend refuses by name, which the next retry
     * reads), then 1.12.2, the other version Eagler clients speak.
     */
    private static int fallback(Set<Integer> tried) {
        for (int protocol : EAGLER_GUESSES) {
            if (!tried.contains(protocol) && ServerVersions.isCompatible(protocol)) {
                return protocol;
            }
        }
        return -1;
    }

    /** The refusal's text, and the arguments of its translations, which carry the version. */
    private static String textOf(Component reason) {
        StringBuilder text = new StringBuilder(reason.getString());
        appendArguments(reason, text, 0);
        return text.toString();
    }

    private static void appendArguments(Component component, StringBuilder text, int depth) {
        if (depth > 8) {
            return;
        }
        if (component.getContents() instanceof TranslatableContents translation) {
            text.append(' ').append(translation.getKey());
            for (Object argument : translation.getArgs()) {
                if (argument instanceof Component nested) {
                    text.append(' ').append(nested.getString());
                    appendArguments(nested, text, depth + 1);
                } else {
                    text.append(' ').append(argument);
                }
            }
        }
        for (Component sibling : component.getSiblings()) {
            appendArguments(sibling, text, depth + 1);
        }
    }

    private static LearnedVersions learnedVersions() {
        synchronized (LOCK) {
            LearnedVersions store = learned;
            if (store == null) {
                Path folder = Minecraft.getInstance().gameDirectory.toPath().resolve("incari");
                store = LearnedVersions.load(folder);
                learned = store;
            }
            return store;
        }
    }
}
