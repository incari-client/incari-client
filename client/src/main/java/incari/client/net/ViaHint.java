package incari.client.net;

import net.minecraft.ChatFormatting;
import net.minecraft.client.multiplayer.ServerData;
import net.minecraft.network.chat.Component;
import org.jspecify.annotations.Nullable;

/**
 * With Via off for a server (the default, ADR 0071), a server of another version refuses the join
 * ("Outdated server! I'm still on 1.8.8", "Incompatible client! Please use 1.20.4"), or an Eagler
 * proxy drops it without a word. The disconnection screen then says, below the server's reason,
 * how to turn Via on. Render thread.
 */
public final class ViaHint {

    // Literal English until Incari has its own language files. Says both ways in: a server from the
    // list is edited there; one joined by Direct Connect or a link isn't in the list the player sees
    // (the game keeps it hidden), and Direct Connect has a Via button of its own (DirectJoinVia).
    static final String HINT = "This server may run another Minecraft version. To join it with ViaVersion, set"
            + " Via to Auto: edit it in the server list, or use the Via button on Direct Connect.";

    private ViaHint() {}

    /** {@code DisconnectedScreen.init}: the reason to show, with the hint below it when it applies. */
    public static Component reason(Component reason) {
        ServerData server = ServerConnections.lastRefusedJoin();
        boolean refused = VersionRefusals.isAboutVersions(VersionRetry.textOf(reason));
        boolean featured = server != null && FeaturedServers.is(server);
        if (!applies(server, featured, refused, ServerConnections.lastJoinDroppedDuringLogin())) {
            return reason;
        }
        return Component.empty()
                .append(reason)
                .append("\n\n")
                .append(Component.literal(HINT).withStyle(ChatFormatting.YELLOW));
    }

    /**
     * Whether to hint: the last join, refused before it reached the world, was to a server with Via
     * off, and the refusal was about versions or a silent drop during an Eagler login. Never for a
     * featured server: it can't be edited, and it runs the game's version (ADR 0071), so a drop there
     * is something else, such as a rejoin while the old session lingers (ADR 0018).
     */
    static boolean applies(
            @Nullable ServerData server, boolean featured, boolean refusedForVersion, boolean droppedDuringLogin) {
        return server != null
                && !featured
                && !ServerVersions.usesVia(server)
                && (refusedForVersion || droppedDuringLogin);
    }
}
