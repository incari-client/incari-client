package incari.client.net;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mojang.logging.LogUtils;
import incari.client.platform.ClientPlatform;
import incari.net.WebSocketUrl;
import incari.net.eagler.EaglerQuery;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import net.minecraft.ChatFormatting;
import net.minecraft.client.multiplayer.ServerData;
import net.minecraft.client.multiplayer.ServerStatusPinger;
import net.minecraft.client.multiplayer.resolver.ServerAddress;
import net.minecraft.network.chat.CommonComponents;
import net.minecraft.network.chat.Component;
import net.minecraft.network.protocol.status.ServerStatus;
import net.minecraft.util.Util;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;

/**
 * The server list's ping for Eagler servers (ADR 0016): they don't answer the vanilla status
 * ping, so a WebSocket server is asked for its MOTD instead ({@link EaglerQuery}), and the answer
 * fills the list's entry the way a status response would.
 */
public final class EaglerPings {

    private static final Logger LOGGER = LogUtils.getLogger();
    private static final Component CANT_CONNECT =
            Component.translatable("multiplayer.status.cannot_connect").withColor(0xFFFF0000);
    /** How long a version check before joining may wait for the MOTD. */
    private static final long DETECT_TIMEOUT_SECONDS = 5;

    private EaglerPings() {}

    /**
     * {@code ServerStatusPinger.pingServer}: pings {@code server} if it's a WebSocket address and
     * returns true; returns false for the vanilla ping to go ahead.
     */
    public static boolean ping(
            ServerData server, ServerAddress address, Runnable onPersistentDataChange, Runnable onPongResponse) {
        WebSocketUrl url = address.webSocketUrl();
        if (url == null) {
            return false;
        }
        server.motd = Component.translatable("multiplayer.status.pinging");
        server.playerList = List.of();
        EaglerQuery.motd(ClientPlatform.get().network(), url.toString(), Util::getMillis, new EaglerQuery.Callback() {
            @Override
            public void answered(String json, byte @Nullable [] icon, long latencyMillis) {
                try {
                    fill(server, json, icon, onPersistentDataChange);
                } catch (RuntimeException e) {
                    failed("an unreadable answer (" + e + ")");
                    return;
                }
                server.ping = latencyMillis;
                onPongResponse.run();
            }

            @Override
            public void failed(String reason) {
                LOGGER.error("Can't ping {}: {}", server.ip, reason);
                server.motd = CANT_CONNECT;
                server.status = CommonComponents.EMPTY;
            }
        });
        return true;
    }

    /**
     * The protocol the Eagler server at {@code url} names in its MOTD, or {@link
     * ServerVersions#UNKNOWN} if it names none or doesn't answer. Waits up to five seconds: the
     * connect screen's connector thread only.
     */
    static int detectProtocol(WebSocketUrl url) {
        CompletableFuture<Integer> result = new CompletableFuture<>();
        EaglerQuery query = EaglerQuery.motd(
                ClientPlatform.get().network(), url.toString(), Util::getMillis, new EaglerQuery.Callback() {
                    @Override
                    public void answered(String json, byte @Nullable [] icon, long latencyMillis) {
                        result.complete(protocolIn(JsonParser.parseString(json).getAsJsonObject()));
                    }

                    @Override
                    public void failed(String reason) {
                        result.complete(ServerVersions.UNKNOWN);
                    }
                });
        try {
            return result.get(DETECT_TIMEOUT_SECONDS, TimeUnit.SECONDS);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            query.cancel();
            return ServerVersions.UNKNOWN;
        } catch (ExecutionException | TimeoutException | RuntimeException e) {
            query.cancel();
            return ServerVersions.UNKNOWN;
        }
    }

    private static void fill(ServerData server, String json, byte @Nullable [] icon, Runnable onPersistentDataChange) {
        JsonObject answer = JsonParser.parseString(json).getAsJsonObject();
        JsonObject data = answer.getAsJsonObject("data");
        List<String> lines = new ArrayList<>();
        for (JsonElement line : data.getAsJsonArray("motd")) {
            lines.add(line.getAsString());
        }
        // Eagler MOTDs use legacy formatting codes, which the game's font still draws.
        server.motd = Component.literal(String.join("\n", lines));
        int online = data.get("online").getAsInt();
        int max = data.get("max").getAsInt();
        server.status = ServerStatusPinger.formatPlayerCount(online, max);
        server.players = new ServerStatus.Players(max, online, List.of());
        JsonArray names = data.getAsJsonArray("players");
        List<Component> playerList = new ArrayList<>();
        for (JsonElement name : names) {
            playerList.add(Component.literal(name.getAsString()));
        }
        if (names.size() < online && !playerList.isEmpty()) {
            playerList.add(Component.translatable("multiplayer.status.and_more", online - names.size()));
        }
        server.playerList = playerList;
        server.protocol = protocolIn(answer);
        int shown = server.protocol != ServerVersions.UNKNOWN ? server.protocol : VersionRetry.learned(server.ip);
        server.version = Component.literal(
                        versionLabel(shown, ServerVersions.usesVia(server) || FeaturedServers.is(server)))
                .withStyle(ChatFormatting.GRAY);
        if (icon != null) {
            byte[] png = iconPng(icon);
            if (png != null && !Arrays.equals(png, server.getIconBytes())) {
                server.setIconBytes(ServerData.validateIcon(png));
                onPersistentDataChange.run();
            }
        }
    }

    /**
     * The server list's version for an Eagler server known to speak {@code protocol} (-1 for not
     * known). One of another version with Via off says it needs Via: the list marks it incompatible
     * ({@link ServerVersions#isCompatible(ServerData)}) and shows this in red. Literal English until
     * Incari has its own language files.
     */
    static String versionLabel(int protocol, boolean viaOn) {
        if (protocol < 0) {
            return "Eagler";
        }
        String name = ServerVersions.name(protocol);
        return !viaOn && ServerVersions.needsVia(protocol) ? "Needs Via: " + name : "Eagler " + name;
    }

    /**
     * The protocol an answer names: Incari's own servers and the development bridge add {@code
     * protocol} to the MOTD; EaglerXServer doesn't, and "Auto" finds out when it joins (ADR 0018).
     */
    private static int protocolIn(JsonObject answer) {
        JsonObject data =
                answer.has("data") && answer.get("data").isJsonObject() ? answer.getAsJsonObject("data") : null;
        if (data != null && data.has("protocol") && data.get("protocol").isJsonPrimitive()) {
            int protocol = data.get("protocol").getAsInt();
            if (ServerVersions.isCompatible(protocol)) {
                return protocol;
            }
        }
        return ServerVersions.UNKNOWN;
    }

    /** The MOTD icon (64x64 RGBA) as the PNG the server list keeps. */
    private static byte @Nullable [] iconPng(byte[] rgba) {
        return rgba.length == EaglerQuery.ICON_BYTES ? PngBytes.encode(64, 64, rgba) : null;
    }
}
