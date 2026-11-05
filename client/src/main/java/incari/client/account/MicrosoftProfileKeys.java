package incari.client.account;

import com.mojang.logging.LogUtils;
import incari.net.msa.MicrosoftAccount;
import incari.net.msa.ProfileCertificates;
import java.time.Duration;
import java.time.Instant;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import net.minecraft.client.multiplayer.ProfileKeyPairManager;
import net.minecraft.util.Crypt;
import net.minecraft.util.CryptException;
import net.minecraft.world.entity.player.ProfileKeyPair;
import net.minecraft.world.entity.player.ProfilePublicKey;
import org.slf4j.Logger;

/**
 * The signed-in account's chat-signing keys, as the game asks for them (ADR 0070): fetched from
 * Mojang through Incari's sign-in client, kept in memory only, and fetched again once Mojang says
 * they're due. Servers that enforce secure chat refuse players without them.
 *
 * <p>The game polls the future from the render thread only once it's done, so it never waits.
 */
public final class MicrosoftProfileKeys implements ProfileKeyPairManager {

    private static final Logger LOGGER = LogUtils.getLogger();
    /** At most one fetch an hour, as the game's own manager, whatever happens. */
    private static final Duration MINIMUM_REFRESH = Duration.ofHours(1);

    private final String accountId;
    private CompletableFuture<Optional<ProfileKeyPair>> keyPair = CompletableFuture.completedFuture(Optional.empty());
    private Instant nextFetch = Instant.EPOCH;

    MicrosoftProfileKeys(String accountId) {
        this.accountId = accountId;
    }

    /** Hook ({@code Minecraft.getProfileKeyPairManager}): these keys while signed in, else the game's. */
    public static ProfileKeyPairManager manager(ProfileKeyPairManager vanilla) {
        MicrosoftProfileKeys keys = MicrosoftAccounts.keys();
        return keys != null ? keys : vanilla;
    }

    boolean isFor(MicrosoftAccount account) {
        return accountId.equals(account.id());
    }

    @Override
    public synchronized CompletableFuture<Optional<ProfileKeyPair>> prepareKeyPair() {
        Optional<ProfileKeyPair> current = keyPair.getNow(Optional.empty());
        boolean fetching = !keyPair.isDone();
        if (fetching || (current.isPresent() && !current.get().dueRefresh())) {
            return keyPair;
        }
        nextFetch = Instant.now().plus(MINIMUM_REFRESH);
        keyPair = MicrosoftAccounts.fresh()
                .thenCompose(account -> MicrosoftAccounts.signIn().certificates(account))
                .thenApply(MicrosoftProfileKeys::keyPair)
                .exceptionally(failure -> {
                    LOGGER.warn("Couldn't get the chat key: {}", reason(failure));
                    return current;
                });
        return keyPair;
    }

    @Override
    public synchronized boolean shouldRefreshKeyPair() {
        if (!keyPair.isDone() || Instant.now().isBefore(nextFetch)) {
            return false;
        }
        return keyPair.getNow(Optional.empty()).map(ProfileKeyPair::dueRefresh).orElse(true);
    }

    private static Optional<ProfileKeyPair> keyPair(ProfileCertificates certificates) {
        try {
            ProfilePublicKey.Data publicKey = new ProfilePublicKey.Data(
                    Instant.parse(certificates.expiresAt()),
                    Crypt.stringToRsaPublicKey(certificates.publicKeyPem()),
                    certificates.publicKeySignature());
            return Optional.of(new ProfileKeyPair(
                    Crypt.stringToPemRsaPrivateKey(certificates.privateKeyPem()),
                    new ProfilePublicKey(publicKey),
                    Instant.parse(certificates.refreshedAfter())));
        } catch (CryptException | RuntimeException e) {
            // Never the key itself in the message: just what kind of failure.
            throw new IllegalStateException(
                    "Mojang's chat key can't be read (" + e.getClass().getSimpleName() + ")");
        }
    }

    private static String reason(Throwable failure) {
        Throwable cause = failure.getCause() != null ? failure.getCause() : failure;
        String message = cause.getMessage();
        return message != null ? message : cause.getClass().getSimpleName();
    }
}
