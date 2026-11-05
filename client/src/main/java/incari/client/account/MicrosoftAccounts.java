package incari.client.account;

import com.mojang.logging.LogUtils;
import incari.client.net.WispServers;
import incari.client.platform.ClientPlatform;
import incari.net.https.HttpsClient;
import incari.net.msa.MicrosoftAccount;
import incari.net.msa.MicrosoftEndpoints;
import incari.net.msa.MicrosoftSignIn;
import incari.net.tls.Transport;
import incari.platform.api.net.PlatformNetwork;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.AtomicMoveNotSupportedException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.attribute.PosixFilePermission;
import java.util.EnumSet;
import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.CompletableFuture;
import net.minecraft.client.User;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;

/**
 * The Microsoft account the player signed in with (ADR 0070), if any: kept in
 * {@code incari/microsoft-account.json} in the game directory (the browser's own storage for the
 * page), and answered as the player wherever the game asks ({@code PlayerName.user}).
 *
 * <p>The file holds the Minecraft token and Microsoft's refresh token, so it never leaves this
 * device: nothing logs it, and signing out deletes it. Sign-in's requests go straight to the hosts
 * on the desktop and through the player's Wisp proxy in a browser, end-to-end encrypted either way.
 */
public final class MicrosoftAccounts {

    private static final Logger LOGGER = LogUtils.getLogger();
    private static final String FILE = "microsoft-account.json";

    // Set once, on the first question.
    private static @Nullable Path file;
    private static volatile @Nullable MicrosoftAccount account;
    private static volatile @Nullable User user;
    /** The account's chat keys, made when it's first asked for. */
    private static volatile @Nullable MicrosoftProfileKeys keys;

    private MicrosoftAccounts() {}

    /** Reads the saved account, once, from {@code gameDirectory}. */
    public static synchronized void load(Path gameDirectory) {
        if (file != null) {
            return;
        }
        Path saved = gameDirectory.resolve("incari").resolve(FILE);
        file = saved;
        if (!Files.isRegularFile(saved)) {
            return;
        }
        try {
            MicrosoftAccount read = MicrosoftAccount.fromJson(Files.readString(saved, StandardCharsets.UTF_8));
            if (read == null) {
                LOGGER.warn("Ignoring {}: it isn't a saved account", saved);
                return;
            }
            use(read);
            LOGGER.info("Playing as Microsoft account {}", read.name());
        } catch (IOException e) {
            LOGGER.warn("Couldn't read {}: {}", saved, e.toString());
        }
    }

    /** The signed-in account, or null. */
    public static @Nullable MicrosoftAccount account() {
        return account;
    }

    /** The player as the game knows them while signed in, or null when not. */
    public static @Nullable User user() {
        return user;
    }

    /** Signs in as {@code signedIn} and keeps it for the next start. */
    public static synchronized void signedIn(MicrosoftAccount signedIn) {
        MicrosoftAccount previous = account;
        use(signedIn);
        if (previous == null || !previous.id().equals(signedIn.id())) {
            keys = null;
        }
        save(signedIn);
    }

    /** Signs out and deletes the saved tokens. */
    public static synchronized void signOut() {
        account = null;
        user = null;
        keys = null;
        Path target = file;
        if (target != null) {
            try {
                Files.deleteIfExists(target);
            } catch (IOException e) {
                LOGGER.warn("Couldn't delete {}: {}", target, e.toString());
            }
        }
    }

    /**
     * The account with a Minecraft token good for a while yet, renewed through Microsoft if it was
     * about to expire (and saved). Fails if there's no account or the renewal was refused.
     */
    public static CompletableFuture<MicrosoftAccount> fresh() {
        MicrosoftAccount current = account;
        if (current == null) {
            return CompletableFuture.failedFuture(new IOException("Not signed in"));
        }
        return signIn().renew(current).thenApply(renewed -> {
            if (!renewed.equals(current)) {
                synchronized (MicrosoftAccounts.class) {
                    // Only if nobody signed out or in meanwhile.
                    if (current.equals(account)) {
                        signedIn(renewed);
                    }
                }
            }
            return renewed;
        });
    }

    /** A sign-in client over this platform's route: a socket, or the Wisp proxy in a browser. */
    public static MicrosoftSignIn signIn() {
        PlatformNetwork network = ClientPlatform.get().network();
        Transport transport = network.canOpenSockets()
                ? Transport.direct(network)
                : Transport.wisp(network, WispServers.signInProxy());
        return new MicrosoftSignIn(HttpsClient.pinned(transport, MicrosoftEndpoints.HOSTS), System::currentTimeMillis);
    }

    /** How sign-in reaches Microsoft, for the screen: "directly", or the proxy's address. */
    public static String route() {
        return ClientPlatform.get().network().canOpenSockets() ? "directly" : "through " + WispServers.signInProxy();
    }

    /** The chat keys for the signed-in account, or null when signed out. */
    static @Nullable MicrosoftProfileKeys keys() {
        MicrosoftAccount current = account;
        if (current == null) {
            return null;
        }
        MicrosoftProfileKeys made = keys;
        if (made == null || !made.isFor(current)) {
            made = new MicrosoftProfileKeys(current.id());
            keys = made;
        }
        return made;
    }

    private static void use(MicrosoftAccount signedIn) {
        account = signedIn;
        user = new User(
                signedIn.name(),
                UUID.fromString(signedIn.dashedId()),
                signedIn.minecraftToken(),
                Optional.empty(),
                Optional.empty());
    }

    private static void save(MicrosoftAccount signedIn) {
        Path target = file;
        if (target == null) {
            return;
        }
        // Owner-only (it's a refresh token: on a shared machine, others could take the account
        // with it), and written beside then moved over, so a crash never leaves half a file.
        Path temporary = target.resolveSibling(FILE + ".tmp");
        try {
            Files.createDirectories(target.getParent());
            Files.deleteIfExists(temporary);
            Files.createFile(temporary);
            try {
                Files.setPosixFilePermissions(
                        temporary, EnumSet.of(PosixFilePermission.OWNER_READ, PosixFilePermission.OWNER_WRITE));
            } catch (UnsupportedOperationException e) {
                // Windows: the user's profile folder is already private.
            }
            Files.writeString(temporary, signedIn.toJson(), StandardCharsets.UTF_8);
            try {
                Files.move(temporary, target, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
            } catch (AtomicMoveNotSupportedException e) {
                Files.move(temporary, target, StandardCopyOption.REPLACE_EXISTING);
            }
        } catch (IOException e) {
            LOGGER.warn("Couldn't save the Microsoft account: {}", e.toString());
        }
    }

    /** Forgets everything, as a new start would. Tests only. */
    public static synchronized void reset() {
        file = null;
        account = null;
        user = null;
        keys = null;
    }
}
