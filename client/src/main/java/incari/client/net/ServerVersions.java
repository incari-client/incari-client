package incari.client.net;

import incari.via.IncariVia;
import java.util.ArrayList;
import java.util.List;
import java.util.function.IntSupplier;
import net.minecraft.SharedConstants;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.multiplayer.ServerData;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.network.chat.Component;
import org.jspecify.annotations.Nullable;

/**
 * Which Minecraft version the game speaks to a server (ADR 0017, ADR 0071). Each server in the list
 * has a choice, saved with it: Via off (the default: the game's own version, no translation),
 * automatic, or one release. Automatic means the version the server reports, or else the one an
 * earlier join found; a server that doesn't say (Eagler servers' MOTDs never do) is joined as the
 * game's own version, and a refusal names the right one ({@link VersionRetry}, ADR 0018). Via
 * translates whenever the version joined with isn't the game's own.
 */
public final class ServerVersions {

    /** The saved choice for "work it out" through Via. */
    public static final int AUTO = -1;

    /** The default choice: no Via, join as the game's own version (ADR 0071). */
    public static final int OFF = -3;

    /**
     * The server list's protocol for a server that didn't say which version it speaks (an Eagler
     * server): joinable, and "Auto" finds the version when it joins.
     */
    public static final int UNKNOWN = -2;

    /**
     * Not {@code incariVersion}, which builds before ADR 0071 saved with "Auto" as the default: those
     * choices are left unread, so every server starts with Via off.
     */
    private static final String TAG = "incariVia";

    /** A quick play join's Via choice from the page's {@code ?via=}: the address and the choice, or null. */
    private static volatile @Nullable QuickPlayVia quickPlay;

    private record QuickPlayVia(String address, int choice) {}

    private ServerVersions() {}

    /**
     * The browser page's {@code ?server=...&via=...}: the Via choice for that quick play join, as
     * "off", "auto" or a release ("1.8.9"). The game makes the server's entry itself, with Via off;
     * {@link #applyQuickPlay} then sets the choice on it. Returns false for a choice it can't read.
     */
    public static boolean setQuickPlay(String address, String choice) {
        int parsed = parseChoice(choice);
        if (parsed == UNKNOWN) {
            return false;
        }
        quickPlay = new QuickPlayVia(address, parsed);
        return true;
    }

    /** "off", "auto" or a release Via knows, as a choice; {@link #UNKNOWN} for anything else. */
    static int parseChoice(String choice) {
        String text = choice.trim().toLowerCase(java.util.Locale.ROOT);
        if (text.equals("off")) {
            return OFF;
        }
        if (text.equals("auto")) {
            return AUTO;
        }
        int protocol = IncariVia.protocolNamed(text);
        return protocol >= 0 ? protocol : UNKNOWN;
    }

    /** {@link ServerConnections#prepare}: gives the quick play join's entry the page's Via choice, once. */
    static void applyQuickPlay(ServerData server) {
        QuickPlayVia pending = quickPlay;
        if (pending != null && pending.address().equalsIgnoreCase(server.ip)) {
            server.incariVersion = pending.choice();
            quickPlay = null;
        }
    }

    /** The game's own protocol. */
    public static int gameProtocol() {
        return SharedConstants.getCurrentVersion().protocolVersion();
    }

    /** {@code ServerData.write}: saves a choice that isn't the default. */
    public static void write(ServerData server, CompoundTag tag) {
        if (server.incariVersion != OFF) {
            tag.putInt(TAG, server.incariVersion);
        }
    }

    /** {@code ServerData.read}. */
    public static void read(ServerData server, CompoundTag tag) {
        server.incariVersion = tag.getIntOr(TAG, OFF);
    }

    /** Whether the player turned Via on for {@code server}. */
    public static boolean usesVia(ServerData server) {
        return server.incariVersion != OFF;
    }

    /**
     * For a server an Eagler server redirects to: "Auto" if the player turned Via on for the server
     * last joined, else off.
     */
    public static int redirectChoice() {
        ServerConnections.Join join = ServerConnections.lastJoin();
        return join != null && usesVia(join.server()) ? AUTO : OFF;
    }

    /**
     * The server list's check: whether the game can join {@code server}, whose ping reported its
     * protocol. Other versions only with Via on; the rest are marked incompatible, as in the game.
     */
    public static boolean isCompatible(ServerData server) {
        // A featured server runs the game's version and can't be edited (ADR 0071): what an older
        // build's "Auto" learned for it doesn't count.
        return isCompatible(server, () -> FeaturedServers.is(server) ? -1 : VersionRetry.learned(server.ip));
    }

    /** {@link #isCompatible(ServerData)}, given the version an earlier join found for it (-1 for none). */
    static boolean isCompatible(ServerData server, IntSupplier learned) {
        int protocol = server.protocol;
        if (protocol == UNKNOWN) {
            // An Eagler server never says its version: a join with Via on finds it. With Via off, one
            // an earlier join found to run another version can't be joined until Via is turned on.
            return usesVia(server) || !needsVia(learned.getAsInt());
        }
        return protocol == gameProtocol() || (usesVia(server) && IncariVia.supports(protocol));
    }

    /** Whether a server found to speak {@code protocol} (-1 for not known) needs Via to be joined. */
    static boolean needsVia(int protocol) {
        return protocol >= 0 && protocol != gameProtocol();
    }

    /** Whether the game can join a server whose ping reported {@code protocol}: its own version, or one Via translates. */
    public static boolean isCompatible(int protocol) {
        return protocol == UNKNOWN || protocol == gameProtocol() || IncariVia.supports(protocol);
    }

    /** A version's name for the player: "1.8.x", "26.3". */
    public static String name(int protocol) {
        if (protocol == UNKNOWN) {
            return "Auto";
        }
        if (protocol == gameProtocol()) {
            return SharedConstants.getCurrentVersion().name();
        }
        String name = IncariVia.versionName(protocol);
        return name != null ? name : "protocol " + protocol;
    }

    /**
     * The edit-server screen's Via button: shows the choice, off, automatic or one release, and
     * opens the list of them ({@link VersionSelectScreen}), which saves the pick into {@code server}.
     */
    public static Button button(ServerData server, int x, int y) {
        List<VersionSelectScreen.Choice> choices = new ArrayList<>();
        choices.add(new VersionSelectScreen.Choice(
                OFF, "Off", "joins as " + SharedConstants.getCurrentVersion().name()));
        choices.add(new VersionSelectScreen.Choice(AUTO, "Auto", "the server's own version"));
        for (IncariVia.Release release : IncariVia.releases()) {
            String note = release.protocol() == gameProtocol() ? "native" : "";
            choices.add(new VersionSelectScreen.Choice(release.protocol(), release.name(), note));
        }
        if (choices.stream().noneMatch(choice -> choice.protocol() == server.incariVersion)) {
            server.incariVersion = OFF;
        }
        return new VersionButton(server, x, y, pressed -> {
            Minecraft minecraft = Minecraft.getInstance();
            Screen editScreen = minecraft.gui.screen();
            if (editScreen != null) {
                // Coming back lays the edit screen out again from the entry: what was typed goes in it first.
                TypedServerFields.keep(editScreen, server);
                minecraft.gui.setScreen(new VersionSelectScreen(editScreen, choices, server.incariVersion, protocol -> {
                    server.incariVersion = protocol;
                    pressed.setMessage(label(protocol));
                }));
            }
        });
    }

    /** The Via button, knowing its server: the edit-server screen's edit puts Wispcraft's beside it. */
    static final class VersionButton extends Button.Plain {

        /** Its width as the game's layout places it: a whole row. */
        static final int FULL_WIDTH = 200;

        private final ServerData server;

        VersionButton(ServerData server, int x, int y, OnPress onPress) {
            super(x, y, FULL_WIDTH, 20, label(server.incariVersion), onPress, DEFAULT_NARRATION);
            this.server = server;
        }

        ServerData server() {
            return server;
        }
    }

    // Literal English until Incari has its own language files.
    private static Component label(int protocol) {
        String choice = protocol == OFF ? "Off" : protocol == AUTO ? "Auto" : name(protocol);
        return Component.literal("Via: " + choice);
    }
}
