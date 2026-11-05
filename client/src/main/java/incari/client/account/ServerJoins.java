package incari.client.account;

import com.mojang.logging.LogUtils;
import incari.client.profile.PlayerName;
import incari.net.msa.JoinResult;
import incari.net.msa.MicrosoftAccount;
import incari.net.msa.SignInException;
import java.util.concurrent.CompletionException;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import net.minecraft.network.chat.Component;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;

/**
 * Online-mode joins (ADR 0070): when a server asks the client to prove who it is, tell Mojang's
 * session server through Incari's sign-in client, so the server finds the player there. Without an
 * account, say how to get one instead of the game's bare "invalid session".
 *
 * <p>Runs on the game's IO pool, where the handshake already waits for the session server; in a
 * browser that thread suspends, so the page carries on.
 */
public final class ServerJoins {

    private static final Logger LOGGER = LogUtils.getLogger();
    /** Renewing the token and joining: a few requests through a proxy at most. */
    private static final long TIMEOUT_SECONDS = 60;

    private ServerJoins() {}

    /**
     * Hook ({@code ClientHandshakePacketListenerImpl.authenticateServer}): whether Incari answers
     * this join. Not for a launcher's own account (the desktop launched with a real token), which the
     * game's session service answers as before.
     */
    public static boolean handles() {
        return MicrosoftAccounts.account() != null || PlayerName.launchedOffline();
    }

    /**
     * Hook (same): joins the server whose hash is {@code serverHash} as the signed-in account.
     *
     * @return null when joined, else why not, for the disconnect screen
     */
    public static @Nullable Component authenticate(String serverHash) {
        if (MicrosoftAccounts.account() == null) {
            return failed(Component.literal(
                    "This server checks Minecraft accounts. Sign in with Microsoft in Edit Profile to join it."));
        }
        try {
            MicrosoftAccount account = MicrosoftAccounts.fresh().get(TIMEOUT_SECONDS, TimeUnit.SECONDS);
            JoinResult result =
                    MicrosoftAccounts.signIn().join(account, serverHash).get(TIMEOUT_SECONDS, TimeUnit.SECONDS);
            return switch (result) {
                case OK -> null;
                case INVALID_SESSION -> failed(Component.translatable("disconnect.loginFailedInfo.invalidSession"));
                case INSUFFICIENT_PRIVILEGES ->
                    failed(Component.translatable("disconnect.loginFailedInfo.insufficientPrivileges"));
                case BANNED -> failed(Component.translatable("disconnect.loginFailedInfo.userBanned"));
                case UNAVAILABLE -> failed(Component.translatable("disconnect.loginFailedInfo.serversUnavailable"));
            };
        } catch (ExecutionException | CompletionException e) {
            Throwable cause = e.getCause() != null ? e.getCause() : e;
            if (cause instanceof SignInException && cause.getMessage() != null) {
                return failed(Component.literal(cause.getMessage()));
            }
            LOGGER.warn("Joining with the Microsoft account failed: {}", cause.toString());
            return failed(Component.translatable("disconnect.loginFailedInfo.serversUnavailable"));
        } catch (TimeoutException e) {
            return failed(Component.literal("Mojang's session server took too long to answer."));
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return failed(Component.translatable("disconnect.loginFailedInfo.serversUnavailable"));
        }
    }

    private static Component failed(Component reason) {
        return Component.translatable("disconnect.loginFailedInfo", reason);
    }
}
