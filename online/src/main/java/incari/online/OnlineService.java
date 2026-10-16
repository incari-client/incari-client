package incari.online;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.JsonPrimitive;
import incari.framework.config.ConfigStorage;
import incari.framework.online.IncariPlayer;
import incari.framework.online.Online;
import incari.platform.api.net.NetConnection;
import incari.platform.api.net.PlatformNetwork;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.Executor;
import java.util.function.Consumer;
import java.util.function.DoubleSupplier;
import java.util.function.LongSupplier;
import org.jspecify.annotations.Nullable;

/**
 * The client's side of Incari's online service (ADR 0058): one WebSocket, kept open and reopened
 * with back-off. Over it the player signs up, signs in and manages their account, and the client
 * says which server the player is on and asks which players there use Incari.
 * The wire format is {@code docs/online-protocol.md}.
 *
 * <p>Nothing here waits. Requests return at once and answer through their callback on a later
 * {@link #tick}; while the service can't be reached they fail with {@link Failure#OFFLINE}, and the
 * game carries on as if there were no service.
 *
 * <p>Game thread only. The platform's network callbacks and the proof of work's result are queued
 * and handled in {@link #tick}.
 */
public final class OnlineService implements Online {

    /** The protocol version this client speaks. */
    public static final int PROTOCOL = 1;

    /** How long a request may wait for its answer before it fails as {@link Failure#OFFLINE}. */
    static final long REQUEST_TIMEOUT_MILLIS = 20_000;

    /** How long opening the connection and saying hello may take before it's tried again. */
    static final long OPEN_TIMEOUT_MILLIS = 15_000;

    /** The service's limit on a message; the client never sends one larger. */
    static final int MAX_MESSAGE = 16 * 1024;

    /** Nonces tried between yields: a few milliseconds' work on the reference Chromebook. */
    private static final int POW_STEP = 4096;

    /** The reasons a report may give: the service accepts these and no others. */
    public enum ReportReason {
        HARASSMENT("harassment", "Harassment or bullying"),
        INAPPROPRIATE_NAME("inappropriate_name", "Inappropriate name"),
        SPAM("spam", "Spam or scams"),
        IMPERSONATION("impersonation", "Pretending to be someone"),
        OTHER("other", "Something else");

        private final String wire;
        private final String label;

        ReportReason(String wire, String label) {
            this.wire = wire;
            this.label = label;
        }

        /** What the screens call it. */
        public String label() {
            return label;
        }
    }

    /** A blocked account in the block list: an id to unblock it by, and what to call it. */
    public record Block(String id, String label) {}

    /** An Incari player in the world played an emote (ADR 0061). */
    public record PlayedEmote(UUID player, Cosmetic emote) {}

    /** A server pinned atop the server list, marked Sponsored (ADR 0062). */
    public record Featured(String name, String address) {

        /** The most a {@code hello} may name; more are left out. */
        static final int MAX = 4;

        /** Whether it keeps the protocol's bounds: a name of 1 to 32 characters, an address of 1 to 128. */
        static boolean valid(String name, String address) {
            return !name.isBlank()
                    && name.length() <= 32
                    && !address.isBlank()
                    && address.length() <= 128
                    && name.chars().noneMatch(Character::isISOControl)
                    && address.chars().noneMatch(c -> Character.isISOControl(c) || Character.isWhitespace(c));
        }
    }

    /** At least this long between two {@code equip}s: the wardrobe's clicks are gathered into one. */
    static final long EQUIP_INTERVAL_MILLIS = 1_000;

    /** At least this long between two emotes sent; the service limits them too. */
    static final long EMOTE_INTERVAL_MILLIS = 2_000;

    /**
     * How long the service keeps the player's place after they leave a world. A server switch
     * behind a proxy rejoins within it, so it doesn't count as a new server (the service allows
     * about 30 an hour).
     */
    static final long PRESENCE_GRACE_MILLIS = 10_000;

    /** The first wait before a presence the service turned away (limited, or its trouble) goes again. */
    static final long PRESENCE_RETRY_MILLIS = 60_000;

    /** The longest wait between presence retries: the wait doubles up to it. */
    static final long PRESENCE_RETRY_MAX_MILLIS = 15 * 60_000;

    /** Where problems are reported: the client's log. Never given a password, token or code. */
    public interface Log {
        void warn(String message, @Nullable Throwable error);
    }

    private final String url;
    private final String build;
    private final PlatformNetwork network;
    private final SessionStore sessions;
    private final Executor background;
    private final Runnable pause;
    private final LongSupplier clock;
    private final Log log;
    private final Backoff backoff;
    private final WorldPlayers world = new WorldPlayers();
    private final Social social = new Social();
    private final Wardrobe wardrobe;
    /** Emotes other players played, for the client to show on its next tick. */
    private final List<PlayedEmote> emotes = new ArrayList<>();
    /** Whether an {@code equip} is waiting for its answer, and when the next may go. */
    private boolean equipping;

    private long nextEquip;
    private long nextEmote;
    /** The conversation the chat menu shows, whose messages arrive read. */
    private Social.@Nullable Peer viewing;

    private int localIds;

    /** What the platform's threads hand over, handled in {@link #tick}. */
    private final List<Runnable> inbox = new ArrayList<>();

    private final Map<Integer, Pending> pending = new HashMap<>();

    private Status status;
    private boolean started;
    private @Nullable NetConnection connection;
    /** Which connection is the current one; callbacks from an older one are dropped. */
    private int generation;
    /** True once the service answered hello on the current connection. */
    private boolean greeted;

    private long openDeadline = -1;
    private long reconnectAt = -1;
    private int nextId = 1;
    private @Nullable String accountName;
    /** The featured servers the service named in its last {@code hello}; null if it named none. */
    private @Nullable List<Featured> featured;

    /** The latest release the service named in its last {@code hello} answer. */
    private @Nullable String latestRelease;

    /** That release's changelog, one change a line; empty when the service gave none. */
    private List<String> releaseNotes = List.of();

    private boolean shareServer = true;
    private boolean badge = true;
    private long revision;
    /**
     * Counts sign-ups and local sign-outs: a proof of work or resume answer that finds it changed is
     * for something the player cancelled.
     */
    private int signUpRun;

    /** The place the service holds for this connection, as {@link #presenceKey}; "" for none. */
    private String presenceHeld = "";

    /** Whether a {@code presence} is waiting for its answer. */
    private boolean presenceOut;

    /** No presence goes before this; set after one was turned away. */
    private long presenceRetryAt;

    private long presenceRetryDelay = PRESENCE_RETRY_MILLIS;
    /** Until when a left world's place is kept with the service; -1 when not leaving. */
    private long presenceGraceUntil = -1;
    /** Whether the sign-up's form is out: from then on it can't be cancelled. */
    private boolean signUpSent;

    /**
     * A request waiting for its answer. {@code signsIn} marks the requests that sign the
     * connection in: if one times out, the service may have signed in without the client knowing.
     */
    private record Pending(long deadline, Consumer<JsonObject> answer, boolean signsIn) {}

    /**
     * @param url the service's {@code wss://} URL; empty for a build without a service ({@link Status#OFF})
     * @param build this build's id, for the service's {@code hello}
     * @param network the platform's WebSockets
     * @param storage the client's {@code incari/online} folder
     * @param background where proof of work runs: the game's background pool
     * @param pause called between proof-of-work steps, for the page's pool to yield
     *     ({@code PlatformThreads.pauseIfLate})
     * @param clock milliseconds, from any fixed point
     * @param random numbers in [0, 1), for the back-off's jitter
     */
    public OnlineService(
            String url,
            String build,
            PlatformNetwork network,
            ConfigStorage storage,
            Executor background,
            Runnable pause,
            LongSupplier clock,
            DoubleSupplier random,
            Log log) {
        this.url = url.strip();
        this.build = build;
        this.network = network;
        this.background = background;
        this.pause = pause;
        this.clock = clock;
        this.log = log;
        this.backoff = new Backoff(random);
        this.sessions = new SessionStore(storage, log::warn);
        this.wardrobe = new Wardrobe(storage, log);
        this.status = this.url.isEmpty() ? Status.OFF : Status.CONNECTING;
        SessionStore.Saved saved = this.url.isEmpty() ? null : sessions.load();
        this.accountName = saved == null ? null : saved.username();
    }

    // ---------------------------------------------------------------------------------------------
    // What the client calls
    // ---------------------------------------------------------------------------------------------

    /**
     * Opens the connection, once. The client calls this after the title screen shows, never while
     * it loads (ADR 0058).
     */
    public void start() {
        if (started || status == Status.OFF) {
            return;
        }
        started = true;
        connect();
    }

    /** Whether {@link #start} was called. */
    public boolean started() {
        return started;
    }

    /** Once a client tick: handles what arrived, retries, timeouts and lookups. */
    public void tick() {
        List<Runnable> arrived;
        synchronized (inbox) {
            if (inbox.isEmpty()) {
                arrived = List.of();
            } else {
                arrived = new ArrayList<>(inbox);
                inbox.clear();
            }
        }
        for (Runnable each : arrived) {
            try {
                each.run();
            } catch (RuntimeException e) {
                // One bad message (or a screen's callback failing) mustn't drop the rest of the tick's.
                log.warn("Something from the Incari service couldn't be handled", e);
            }
        }
        if (!started || status == Status.OFF || status == Status.UPDATE_NEEDED) {
            return;
        }
        long now = clock.getAsLong();
        if (connection == null && reconnectAt >= 0 && now >= reconnectAt) {
            connect();
        }
        if (connection != null && !greeted && openDeadline >= 0 && now >= openDeadline) {
            log.warn("The Incari service didn't answer in time; trying again later", null);
            dropConnection(null);
        }
        if (!pending.isEmpty()) {
            expireRequests(now);
        }
        if (status == Status.SIGNED_IN) {
            syncPresence();
            List<String> batch = world.takeLookup(now);
            if (!batch.isEmpty()) {
                lookup(batch);
            }
            if (!equipping && now >= nextEquip && connection != null && greeted) {
                Loadout unsent = wardrobe.unsent();
                if (unsent != null) {
                    equip(unsent, now);
                }
            }
        }
    }

    /** Tries the service again now, instead of waiting out the back-off: the account screen's Retry. */
    public void retry() {
        if (started && connection == null && status == Status.CONNECTING) {
            backoff.reset();
            connect();
        }
    }

    /**
     * The player joined a server at {@code address} as {@code name}. Sent to the service if signed
     * in; singleplayer never calls this.
     */
    public void joined(String address, String name) {
        world.join(address, name);
        presenceGraceUntil = -1;
        revision++;
        syncPresence();
    }

    /**
     * The player left the world (or is in singleplayer, which the service never hears of). The
     * service keeps the place for {@link #PRESENCE_GRACE_MILLIS}, so a server switch isn't a new
     * server each time.
     */
    public void left() {
        boolean wasIn = world.place() != null;
        world.leave();
        social.leftServer();
        revision++;
        if (wasIn) {
            presenceGraceUntil = clock.getAsLong() + PRESENCE_GRACE_MILLIS;
        }
    }

    /** The tab list's players, by UUID, as it is now. Called about once a second while in a world. */
    public void tabList(Map<UUID, String> players) {
        world.tabList(players);
    }

    /** Whether the player shares the server they're on with their friends. */
    public boolean sharesServer() {
        return shareServer;
    }

    /** The Incari players in the world by their name there, for the block and report screens. */
    public Map<String, IncariPlayer> worldPlayers() {
        return world.players();
    }

    /** Whether the service hears where the player is: signed in, and on a server. */
    public boolean inWorld() {
        return world.place() != null;
    }

    // ---------------------------------------------------------------------------------------------
    // Online
    // ---------------------------------------------------------------------------------------------

    @Override
    public Status status() {
        return status;
    }

    @Override
    public @Nullable String accountName() {
        return accountName;
    }

    @Override
    public @Nullable IncariPlayer player(UUID id) {
        return status == Status.SIGNED_IN ? world.player(id) : null;
    }

    @Override
    public long revision() {
        return revision + world.revision() + social.revision() + wardrobe.revision();
    }

    /**
     * The latest Incari release, as the service named it when it last answered {@code hello}
     * ({@link Releases}); null before then, or from a service that doesn't say. Kept while
     * disconnected.
     */
    public @Nullable String latestRelease() {
        return latestRelease;
    }

    /**
     * What's new in {@link #latestRelease}, one change a line, as the service gave it; empty if it
     * gave none. Lines that aren't plain text ({@link Releases#note}) are dropped.
     */
    public List<String> releaseNotes() {
        return releaseNotes;
    }

    /** Whether a session is saved on this device: the account screen shows "Signing in" while it resumes. */
    public boolean hasSavedSession() {
        return accountName != null;
    }

    // ---------------------------------------------------------------------------------------------
    // Accounts
    // ---------------------------------------------------------------------------------------------

    /**
     * Creates an account and signs in to it: asks for a challenge, solves it on the background pool,
     * then sends the form. Answers with the ten recovery codes, to show once.
     */
    public void signUp(String username, String password, Consumer<Result<List<String>>> done) {
        if (!signedOutAndConnected(done)) {
            return;
        }
        int run = ++signUpRun;
        signUpSent = false;
        send(request("challenge"), answer -> {
            if (run != signUpRun) {
                // Cancelled before anything was made: nobody is waiting any more.
                return;
            }
            Failure failure = failure(answer, Failure.WRONG_CREDENTIALS);
            if (failure != null) {
                done.accept(Result.failed(failure));
                return;
            }
            byte[] challenge;
            int bits;
            try {
                challenge = ProofOfWork.base64Url(answer.get("challenge").getAsString());
                bits = answer.get("bits").getAsInt();
                if (challenge.length != 16 || bits < 0 || bits > 40) {
                    throw new IllegalArgumentException("bits " + bits);
                }
            } catch (RuntimeException e) {
                log.warn("The Incari service sent a challenge this client can't use", e);
                done.accept(Result.failed(Failure.UNAVAILABLE));
                return;
            }
            String challengeText = answer.get("challenge").getAsString();
            solve(run, new ProofOfWork(challenge, bits), nonce -> {
                if (run != signUpRun) {
                    return;
                }
                JsonObject form = request("signup");
                form.addProperty("username", username);
                form.addProperty("password", password);
                form.addProperty("challenge", challengeText);
                form.addProperty("nonce", nonce);
                // Can't be cancelled from here: the account may exist before the answer comes, and
                // the player must get its recovery codes.
                signUpSent = true;
                send(form, true, signedUp -> {
                    signUpSent = false;
                    Failure signUpFailure = failure(signedUp, Failure.REJECTED);
                    if (signUpFailure != null) {
                        done.accept(Result.failed(signUpFailure));
                        return;
                    }
                    List<String> codes = codes(signedUp);
                    if (!signedIn(signedUp, username) && codes.isEmpty()) {
                        done.accept(Result.failed(Failure.UNAVAILABLE));
                        return;
                    }
                    // A malformed answer doesn't sign in, but the account exists: the player still
                    // gets its codes and can sign in with the password.
                    done.accept(Result.ok(codes));
                });
            });
        });
    }

    /**
     * Cancels the sign-up if its form hasn't been sent (still fetching or solving the challenge):
     * the callback is never called and no account is made. False once the form is out, as the
     * account may exist; the callback then still gets the answer and recovery codes.
     */
    public boolean cancelSignUp() {
        if (signUpSent) {
            return false;
        }
        signUpRun++;
        return true;
    }

    /** Signs in. A wrong username and a wrong password fail the same way, on purpose. */
    public void signIn(String username, String password, Consumer<Result<Void>> done) {
        if (!signedOutAndConnected(done)) {
            return;
        }
        JsonObject form = request("signin");
        form.addProperty("username", username);
        form.addProperty("password", password);
        send(form, true, answer -> {
            Failure failure = failure(answer, Failure.WRONG_CREDENTIALS);
            // A limited sign-in reads as a wrong one: the service means it to.
            if (failure == Failure.RATE_LIMITED) {
                failure = Failure.WRONG_CREDENTIALS;
            }
            if (failure != null) {
                done.accept(Result.failed(failure));
            } else {
                done.accept(signedIn(answer, username) ? Result.done() : Result.failed(Failure.UNAVAILABLE));
            }
        });
    }

    /**
     * Sets a new password with a recovery code (Forgot Password), which is then used up, and signs
     * in. Every other session the account had ends.
     */
    public void recover(String username, String code, String newPassword, Consumer<Result<Void>> done) {
        if (!signedOutAndConnected(done)) {
            return;
        }
        String normal = RecoveryCodes.normalize(code);
        if (normal == null) {
            done.accept(Result.failed(Failure.WRONG_CODE));
            return;
        }
        JsonObject form = request("recover");
        form.addProperty("username", username);
        form.addProperty("code", normal);
        form.addProperty("newPassword", newPassword);
        send(form, true, answer -> {
            Failure failure = failure(answer, Failure.WRONG_CODE);
            if (failure != null) {
                done.accept(Result.failed(failure));
            } else {
                done.accept(signedIn(answer, username) ? Result.done() : Result.failed(Failure.UNAVAILABLE));
            }
        });
    }

    /** Changes the password. Every other session ends; this one stays signed in. */
    public void changePassword(String oldPassword, String newPassword, Consumer<Result<Void>> done) {
        if (!signedInAndConnected(done)) {
            return;
        }
        JsonObject form = request("password");
        form.addProperty("old", oldPassword);
        form.addProperty("new", newPassword);
        send(form, answer -> done.accept(done(answer, Failure.WRONG_PASSWORD)));
    }

    /** Ten new recovery codes; the old ten stop working. */
    public void newRecoveryCodes(String password, Consumer<Result<List<String>>> done) {
        if (!signedInAndConnected(done)) {
            return;
        }
        JsonObject form = request("codes");
        form.addProperty("password", password);
        send(form, answer -> {
            Failure failure = failure(answer, Failure.WRONG_PASSWORD);
            List<String> codes = failure == null ? codes(answer) : List.of();
            if (failure == null && codes.isEmpty()) {
                failure = Failure.UNAVAILABLE;
            }
            done.accept(failure == null ? Result.ok(codes) : Result.failed(failure));
        });
    }

    /**
     * Signs out on this device, at once and whatever the service says. With {@code everywhere}, the
     * service also ends every other session the account has; that needs the connection.
     */
    public void signOut(boolean everywhere, Consumer<Result<Void>> done) {
        if (everywhere && !signedInAndConnected(done)) {
            return;
        }
        if (status == Status.SIGNED_IN && connection != null && greeted) {
            JsonObject form = request("signout");
            form.addProperty("everywhere", everywhere);
            send(form, answer -> {
                Failure failure = failure(answer, Failure.UNAVAILABLE);
                signedOutHere();
                done.accept(failure == null ? Result.done() : Result.failed(failure));
            });
        } else {
            // Offline: the token is forgotten here; the service lets it lapse unused.
            signedOutHere();
            done.accept(Result.done());
        }
    }

    /** Deletes the account and everything tied to it, then signs out. */
    public void deleteAccount(String password, Consumer<Result<Void>> done) {
        if (!signedInAndConnected(done)) {
            return;
        }
        JsonObject form = request("delete");
        form.addProperty("password", password);
        send(form, answer -> {
            Result<Void> result = done(answer, Failure.WRONG_PASSWORD);
            if (result.ok()) {
                signedOutHere();
            }
            done.accept(result);
        });
    }

    /** Whether the player's friends see which server they're on; kept by the service. */
    public void setShareServer(boolean share, Consumer<Result<Void>> done) {
        if (!signedInAndConnected(done)) {
            return;
        }
        JsonObject form = request("settings");
        form.addProperty("shareServer", share);
        send(form, answer -> {
            Result<Void> result = done(answer, Failure.REJECTED);
            if (result.ok()) {
                shareServer = share;
                revision++;
                // The address goes, or comes, with the next presence.
                syncPresence();
            }
            done.accept(result);
        });
    }

    /**
     * The featured servers the service named in its last {@code hello} (ADR 0062), which replace
     * the client's own; null if it named none, or hasn't answered yet.
     */
    public @Nullable List<Featured> featuredServers() {
        return featured;
    }

    /**
     * Tells the service this install ran today (ADR 0062's usage counts): {@code install} is the
     * install's random id, never tied to the account. Fails offline without trying; the caller
     * sends it at most once a UTC day.
     */
    public void count(String install, String version, String kind, Consumer<Result<Void>> done) {
        if (connection == null || !greeted) {
            done.accept(Result.failed(Failure.OFFLINE));
            return;
        }
        JsonObject form = request("count");
        form.addProperty("install", install);
        form.addProperty("version", version);
        form.addProperty("kind", kind);
        send(form, answer -> done.accept(done(answer, Failure.REJECTED)));
    }

    /** Whether the service has answered {@code hello} on the open connection. */
    public boolean greeted() {
        return connection != null && greeted;
    }

    /** Whether others see the player uses Incari (Incari's badge by their name); kept by the service. */
    public boolean showsBadge() {
        return badge;
    }

    /** Shows or hides the player's Incari badge to others (ADR 0062); kept by the service. */
    public void setShowBadge(boolean show, Consumer<Result<Void>> done) {
        if (!signedInAndConnected(done)) {
            return;
        }
        JsonObject form = request("settings");
        form.addProperty("badge", show);
        send(form, answer -> {
            Result<Void> result = done(answer, Failure.REJECTED);
            if (result.ok()) {
                badge = show;
                revision++;
            }
            done.accept(result);
        });
    }

    // ---------------------------------------------------------------------------------------------
    // Blocking and reporting
    // ---------------------------------------------------------------------------------------------

    /**
     * Blocks the Incari player called {@code name} in this world, by their per-server id. The service
     * keeps {@code name} as the block list's label: it never tells the player the account's username.
     */
    public void blockPlayer(String name, IncariPlayer player, Consumer<Result<Void>> done) {
        if (!signedInAndConnected(done)) {
            return;
        }
        JsonObject form = request("block");
        form.addProperty("pid", player.pid());
        form.addProperty("name", name);
        send(form, answer -> {
            Result<Void> result = done(answer, Failure.REJECTED);
            if (result.ok()) {
                world.gone(player.pid());
            }
            done.accept(result);
        });
    }

    /** Blocks the account with this Incari username. */
    public void blockUsername(String username, Consumer<Result<Void>> done) {
        if (!signedInAndConnected(done)) {
            return;
        }
        JsonObject form = request("block");
        form.addProperty("username", username);
        send(form, answer -> done.accept(done(answer, Failure.REJECTED)));
    }

    /**
     * Blocks a friend: by their username, which friends know. The service ends the friendship with
     * the block (ADR 0059); it goes here at once.
     */
    public void blockFriend(Social.Friend friend, Consumer<Result<Void>> done) {
        blockUsername(friend.username(), result -> {
            if (result.ok()) {
                social.unfriended(friend.fid());
            }
            done.accept(result);
        });
    }

    /** Unblocks an account from the block list. */
    public void unblock(Block block, Consumer<Result<Void>> done) {
        if (!signedInAndConnected(done)) {
            return;
        }
        JsonObject form = request("unblock");
        form.addProperty("bid", block.id());
        send(form, answer -> {
            Result<Void> result = done(answer, Failure.REJECTED);
            if (result.ok()) {
                // Whoever it was may be an Incari player here again.
                world.askAll();
            }
            done.accept(result);
        });
    }

    /** The accounts the player has blocked. */
    public void blocks(Consumer<Result<List<Block>>> done) {
        if (!signedInAndConnected(done)) {
            return;
        }
        send(request("blocks"), answer -> {
            Failure failure = failure(answer, Failure.UNAVAILABLE);
            if (failure != null) {
                done.accept(Result.failed(failure));
                return;
            }
            List<Block> blocks = new ArrayList<>();
            try {
                for (JsonElement each : answer.getAsJsonArray("blocks")) {
                    JsonObject block = each.getAsJsonObject();
                    blocks.add(new Block(
                            block.get("bid").getAsString(), block.get("label").getAsString()));
                }
            } catch (RuntimeException e) {
                log.warn("The Incari service sent a block list this client can't read", e);
                done.accept(Result.failed(Failure.UNAVAILABLE));
                return;
            }
            done.accept(Result.ok(List.copyOf(blocks)));
        });
    }

    /** Reports an Incari player in this world, by their per-server id. */
    public void reportPlayer(IncariPlayer player, ReportReason reason, String note, Consumer<Result<Void>> done) {
        JsonObject form = request("report");
        form.addProperty("pid", player.pid());
        report(form, reason, note, done);
    }

    /** Reports the account with this Incari username. */
    public void reportUsername(String username, ReportReason reason, String note, Consumer<Result<Void>> done) {
        JsonObject form = request("report");
        form.addProperty("username", username);
        report(form, reason, note, done);
    }

    private void report(JsonObject form, ReportReason reason, String note, Consumer<Result<Void>> done) {
        if (!signedInAndConnected(done)) {
            return;
        }
        form.addProperty("reason", reason.wire);
        form.addProperty("note", note.length() > 500 ? note.substring(0, 500) : note);
        send(form, answer -> done.accept(done(answer, Failure.REJECTED)));
    }

    // ---------------------------------------------------------------------------------------------
    // Cosmetics (ADR 0061)
    // ---------------------------------------------------------------------------------------------

    /** The player's own cosmetics: what they wear, own and saved, and whether they see others'. */
    public Wardrobe wardrobe() {
        return wardrobe;
    }

    /**
     * The player plays {@code emote}: Incari players on the same server hear of it, if the player is
     * signed in there. The client shows it on the player itself. False if it can't go yet (one every
     * {@link #EMOTE_INTERVAL_MILLIS}) or isn't the player's.
     */
    public boolean playEmote(Cosmetic emote) {
        long now = clock.getAsLong();
        if (emote.slot() != Cosmetic.Slot.EMOTE || !wardrobe.owns(emote) || now < nextEmote) {
            return false;
        }
        nextEmote = now + EMOTE_INTERVAL_MILLIS;
        if (status == Status.SIGNED_IN && connection != null && greeted && world.place() != null) {
            JsonObject form = request("emote");
            form.addProperty("emote", emote.id());
            send(form, answer -> {
                String error = error(answer);
                if (error != null && !"offline".equals(error) && !"rate_limited".equals(error)) {
                    log.warn("The Incari service didn't take an emote (" + error + ")", null);
                }
            });
        }
        return true;
    }

    /** The emotes Incari players in the world played since the last call, oldest first. */
    public List<PlayedEmote> takeEmotes() {
        if (emotes.isEmpty()) {
            return List.of();
        }
        List<PlayedEmote> taken = List.copyOf(emotes);
        emotes.clear();
        return taken;
    }

    /** After signing in, and on each reconnect: the account's wardrobe. */
    private void loadWardrobe() {
        send(request("wardrobe"), answer -> {
            String error = error(answer);
            if (error != null) {
                // "bad_request": a service without cosmetics yet; what's worn stays this device's own.
                if (!"bad_request".equals(error) && !"offline".equals(error)) {
                    log.warn("The Incari service couldn't send the wardrobe (" + error + ")", null);
                }
                return;
            }
            try {
                fromAccount(answer);
            } catch (RuntimeException e) {
                log.warn("The Incari service sent a wardrobe this client can't read", e);
            }
        });
    }

    private void fromAccount(JsonObject json) {
        wardrobe.fromAccount(new HashSet<>(strings(json, "owned")), Loadout.of(strings(json, "equipped")));
    }

    private void equip(Loadout loadout, long now) {
        equipping = true;
        nextEquip = now + EQUIP_INTERVAL_MILLIS;
        JsonObject form = request("equip");
        JsonArray ids = new JsonArray();
        for (String id : loadout.ids()) {
            ids.add(id);
        }
        form.add("cosmetics", ids);
        send(form, answer -> {
            equipping = false;
            String error = error(answer);
            if (error == null) {
                wardrobe.sent(loadout, Loadout.of(strings(answer, "equipped")));
            } else if ("not_owned".equals(error) || "bad_request".equals(error)) {
                // This client and the service disagree on what the account owns: take the service's.
                log.warn("The Incari service turned down what's worn (" + error + ")", null);
                wardrobe.sent(loadout, loadout);
                loadWardrobe();
            }
            // Offline or limited: still unsent, so it goes again later.
        });
    }

    // ---------------------------------------------------------------------------------------------
    // Friends and messages (ADR 0059)
    // ---------------------------------------------------------------------------------------------

    /** Friends, requests and conversations, for the chat menu. */
    public Social social() {
        return social;
    }

    /**
     * The conversation the chat menu shows, or null: its messages arrive read and raise no toast.
     * Setting one marks it read.
     */
    public void viewing(Social.@Nullable Peer peer) {
        viewing = peer;
        if (peer != null) {
            markRead(peer);
        }
    }

    /** Who may message the player; kept by the service. */
    public void setMessagesFrom(Social.MessagesFrom from, Consumer<Result<Void>> done) {
        JsonObject form = request("settings");
        form.addProperty("messages", from.wire);
        saveSetting(form, () -> social.settings(from, social.friendRequests()), done);
    }

    /** Whether others can send the player friend requests; kept by the service. */
    public void setFriendRequests(boolean allowed, Consumer<Result<Void>> done) {
        JsonObject form = request("settings");
        form.addProperty("friendRequests", allowed);
        saveSetting(form, () -> social.settings(social.messagesFrom(), allowed), done);
    }

    private void saveSetting(JsonObject form, Runnable saved, Consumer<Result<Void>> done) {
        if (!signedInAndConnected(done)) {
            return;
        }
        send(form, answer -> {
            Result<Void> result = done(answer, Failure.REJECTED);
            if (result.ok()) {
                saved.run();
            }
            done.accept(result);
        });
    }

    /** Sends a friend request to the Incari player called {@code name} on this server. */
    public void befriend(String name, IncariPlayer player, Consumer<Result<Void>> done) {
        JsonObject form = request("befriend");
        form.addProperty("pid", player.pid());
        form.addProperty("name", name);
        sendRequest(form, player.pid(), done);
    }

    /** Sends a friend request to the account with this Incari username. */
    public void befriend(String username, Consumer<Result<Void>> done) {
        JsonObject form = request("befriend");
        form.addProperty("username", username);
        sendRequest(form, null, done);
    }

    /** @param pid the per-server id of the player here it goes to, which the request is known by after */
    private void sendRequest(JsonObject form, @Nullable String pid, Consumer<Result<Void>> done) {
        if (!signedInAndConnected(done)) {
            return;
        }
        send(form, answer -> {
            Result<Void> result = done(answer, Failure.REJECTED);
            if (result.ok() && answer.has("request")) {
                Social.Request sent = request(answer.getAsJsonObject("request"));
                social.request(sent);
                if (pid != null) {
                    social.requestSentTo(sent.rid(), pid);
                }
            }
            if (result.ok() && answer.has("friend")) {
                // They had asked the player already: friends at once.
                social.friend(friend(answer.getAsJsonObject("friend")), false);
            }
            done.accept(result);
        });
    }

    /** Accepts a friend request received. */
    public void accept(Social.Request request, Consumer<Result<Void>> done) {
        answerRequest("accept", request, done);
    }

    /** Declines a request received, or withdraws one sent. */
    public void decline(Social.Request request, Consumer<Result<Void>> done) {
        answerRequest("decline", request, done);
    }

    private void answerRequest(String type, Social.Request request, Consumer<Result<Void>> done) {
        if (!signedInAndConnected(done)) {
            return;
        }
        JsonObject form = request(type);
        form.addProperty("rid", request.rid());
        send(form, answer -> {
            Result<Void> result = done(answer, Failure.REJECTED);
            if (result.ok()) {
                social.requestGone(request.rid());
                if (answer.has("friend")) {
                    social.friend(friend(answer.getAsJsonObject("friend")), false);
                }
            }
            done.accept(result);
        });
    }

    /** Ends a friendship; the conversation goes with it. */
    public void unfriend(Social.Friend friend, Consumer<Result<Void>> done) {
        if (!signedInAndConnected(done)) {
            return;
        }
        JsonObject form = request("unfriend");
        form.addProperty("fid", friend.fid());
        send(form, answer -> {
            Result<Void> result = done(answer, Failure.REJECTED);
            if (result.ok()) {
                social.unfriended(friend.fid());
            }
            done.accept(result);
        });
    }

    /**
     * Sends a message. It shows at once as sending, then as sent, or as failed if the service won't
     * deliver it (they don't take messages from the player, a block, or too many too fast).
     */
    public void sendMessage(Social.Peer peer, String text, Consumer<Result<Void>> done) {
        String trimmed = text.strip();
        if (trimmed.isEmpty()) {
            done.accept(Result.failed(Failure.REJECTED));
            return;
        }
        String body = trimmed.length() > MAX_TEXT ? trimmed.substring(0, MAX_TEXT) : trimmed;
        if (!signedInAndConnected(done)) {
            return;
        }
        String local = "local-" + ++localIds;
        social.message(peer, new Social.Message(local, true, body, clock.getAsLong(), Social.Delivery.SENDING), true);
        JsonObject form = request("send");
        form.addProperty(peer.friend() ? "fid" : "pid", peer.id());
        form.addProperty("text", body);
        send(form, answer -> {
            Failure failure = failure(answer, Failure.REJECTED);
            if (failure == null) {
                String mid = answer.has("mid") ? answer.get("mid").getAsString() : local;
                long at = answer.has("at") ? answer.get("at").getAsLong() : clock.getAsLong();
                social.replace(peer, local, new Social.Message(mid, true, body, at, Social.Delivery.SENT));
                done.accept(Result.done());
            } else {
                social.replace(
                        peer, local, new Social.Message(local, true, body, clock.getAsLong(), Social.Delivery.FAILED));
                done.accept(Result.failed("closed".equals(error(answer)) ? Failure.CLOSED : failure));
            }
        });
    }

    /** The longest message the service takes. */
    public static final int MAX_TEXT = 500;

    /**
     * Loads the page of messages before the oldest held, for a friend's conversation: the first
     * time it's shown, and as the player scrolls back. After a reconnect (the conversation is
     * stale), the newest page instead, for what came meanwhile. Players on this server have no
     * history.
     */
    public void loadOlder(Social.Peer peer) {
        Social.Conversation conversation = social.conversation(peer);
        if (!peer.friend() || conversation.loading()) {
            return;
        }
        boolean newest = conversation.stale();
        if (!newest && conversation.loaded() && !conversation.hasMore()) {
            return;
        }
        if (status != Status.SIGNED_IN || connection == null || !greeted) {
            return;
        }
        JsonObject form = request("history");
        form.addProperty("fid", peer.id());
        if (!newest) {
            for (Social.Message each : conversation.messages()) {
                if (each.delivery() == Social.Delivery.SENT) {
                    form.addProperty("before", each.mid());
                    break;
                }
            }
        }
        conversation.startLoading();
        send(form, answer -> {
            if (error(answer) != null) {
                social.loadFailed(peer);
                return;
            }
            try {
                List<Social.Message> page = new ArrayList<>();
                for (JsonElement each : answer.getAsJsonArray("messages")) {
                    page.add(message(each.getAsJsonObject()));
                }
                page.sort((a, b) -> Long.compare(a.at(), b.at()));
                boolean more = answer.has("more") && answer.get("more").getAsBoolean();
                if (newest) {
                    social.newest(peer, page, more);
                } else {
                    social.older(peer, page, more);
                }
            } catch (RuntimeException e) {
                log.warn("The Incari service sent a history this client can't read", e);
                social.loadFailed(peer);
                return;
            }
            Social.Peer open = viewing;
            if (newest && open != null && open.key().equals(peer.key())) {
                // Read up to the newest now that it's here (markRead held it back while stale).
                sendRead(peer, conversation);
            }
        });
    }

    private void markRead(Social.Peer peer) {
        Social.Conversation conversation = social.existing(peer);
        boolean unread = conversation != null && conversation.unread() > 0;
        social.read(peer);
        if (conversation == null || !unread || !peer.friend()) {
            return;
        }
        if (conversation.stale()) {
            // What's held may stop short of the newest message: the read goes once the newest
            // page is in (loadOlder), or the service would count unseen messages read.
            loadOlder(peer);
            return;
        }
        sendRead(peer, conversation);
    }

    /** Tells the service the player has read the conversation up to the newest message held. */
    private void sendRead(Social.Peer peer, Social.Conversation conversation) {
        if (status != Status.SIGNED_IN || connection == null) {
            return;
        }
        String last = null;
        for (Social.Message each : conversation.messages()) {
            if (!each.mine()) {
                last = each.mid();
            }
        }
        JsonObject form = request("read");
        form.addProperty("fid", peer.id());
        if (last != null) {
            form.addProperty("mid", last);
        }
        send(form, answer -> {});
    }

    /** After signing in: the friend list, requests and unread counts. */
    private void loadFriends() {
        send(request("friends"), answer -> {
            String error = error(answer);
            if ("bad_request".equals(error)) {
                // A service that doesn't speak friends yet (before the backend's B4).
                social.available(false);
                return;
            }
            if (error != null) {
                return;
            }
            try {
                List<Social.Friend> friends = new ArrayList<>();
                Map<String, Integer> unread = new HashMap<>();
                for (JsonElement each : answer.getAsJsonArray("friends")) {
                    JsonObject json = each.getAsJsonObject();
                    Social.Friend friend = friend(json);
                    friends.add(friend);
                    if (json.has("unread") && json.get("unread").getAsInt() > 0) {
                        unread.put(friend.fid(), json.get("unread").getAsInt());
                    }
                }
                List<Social.Request> requests = new ArrayList<>();
                if (answer.has("requests")) {
                    for (JsonElement each : answer.getAsJsonArray("requests")) {
                        requests.add(request(each.getAsJsonObject()));
                    }
                }
                social.loaded(friends, requests, unread);
                social.available(true);
                Social.Peer open = viewing;
                if (open != null && open.friend()) {
                    // The conversation on screen catches up with what came while disconnected.
                    loadOlder(open);
                    markRead(open);
                }
            } catch (RuntimeException e) {
                log.warn("The Incari service sent a friend list this client can't read", e);
            }
        });
    }

    private void received(JsonObject message) {
        Social.Message body = new Social.Message(
                message.get("mid").getAsString(),
                false,
                message.get("text").getAsString(),
                message.has("at") ? message.get("at").getAsLong() : clock.getAsLong(),
                Social.Delivery.SENT);
        Social.Peer peer;
        if (message.has("fid")) {
            String fid = message.get("fid").getAsString();
            Social.Friend friend = social.friend(fid);
            peer = new Social.Peer(true, fid, friend == null ? "A friend" : friend.username());
        } else {
            peer = new Social.Peer(
                    false, message.get("pid").getAsString(), message.get("name").getAsString());
        }
        Social.Peer open = viewing;
        boolean shown = open != null && open.key().equals(peer.key());
        social.message(peer, body, shown);
        if (shown) {
            markRead(peer);
        }
    }

    private static Social.Friend friend(JsonObject json) {
        JsonElement server = json.get("server");
        return new Social.Friend(
                json.get("fid").getAsString(),
                json.get("username").getAsString(),
                json.has("online") && json.get("online").getAsBoolean(),
                server == null || server.isJsonNull() ? null : server.getAsString());
    }

    private static Social.Request request(JsonObject json) {
        return new Social.Request(
                json.get("rid").getAsString(),
                json.get("incoming").getAsBoolean(),
                json.get("label").getAsString());
    }

    private static Social.Message message(JsonObject json) {
        return new Social.Message(
                json.get("mid").getAsString(),
                "me".equals(json.get("from").getAsString()),
                json.get("text").getAsString(),
                json.get("at").getAsLong(),
                Social.Delivery.SENT);
    }

    // ---------------------------------------------------------------------------------------------
    // The connection
    // ---------------------------------------------------------------------------------------------

    private void connect() {
        reconnectAt = -1;
        greeted = false;
        setStatus(Status.CONNECTING);
        int ours = ++generation;
        openDeadline = clock.getAsLong() + OPEN_TIMEOUT_MILLIS;
        try {
            connection = network.openWebSocket(url, new NetConnection.Listener() {
                @Override
                public void connected() {
                    post(ours, OnlineService.this::opened);
                }

                @Override
                public void received(ByteBuffer data) {
                    // The service speaks text; a binary message is a mistake on its side.
                    byte[] bytes = new byte[data.remaining()];
                    data.get(bytes);
                    String text = new String(bytes, StandardCharsets.UTF_8);
                    post(ours, () -> receive(text));
                }

                @Override
                public void receivedText(String text) {
                    post(ours, () -> receive(text));
                }

                @Override
                public void closed(@Nullable Throwable cause) {
                    post(ours, () -> dropConnection(cause));
                }
            });
        } catch (RuntimeException e) {
            connection = null;
            dropConnection(e);
        }
    }

    /** Queues {@code task} for the next tick, if connection {@code from} is still the current one. */
    private void post(int from, Runnable task) {
        synchronized (inbox) {
            inbox.add(() -> {
                if (from == generation) {
                    task.run();
                }
            });
        }
    }

    private void opened() {
        JsonObject hello = request("hello");
        hello.addProperty("v", PROTOCOL);
        hello.addProperty("client", build);
        send(hello, answer -> {
            String error = error(answer);
            if ("update".equals(error)) {
                log.warn("The Incari service no longer speaks protocol " + PROTOCOL + "; update the client", null);
                closeConnection();
                setStatus(Status.UPDATE_NEEDED);
                return;
            }
            if (error != null) {
                log.warn("The Incari service refused hello (" + error + ")", null);
                dropConnection(null);
                return;
            }
            greeted = true;
            openDeadline = -1;
            JsonObject named = answer.get("release") instanceof JsonObject object ? object : null;
            String release = named != null && named.get("version") instanceof JsonPrimitive version
                    ? Releases.valid(version.getAsString())
                    : null;
            if (named != null && release != null) {
                List<String> notes = releaseNotes(named);
                if (!release.equals(latestRelease) || !notes.equals(releaseNotes)) {
                    latestRelease = release;
                    releaseNotes = notes;
                    revision++;
                }
            }
            List<Featured> servers = featured(answer);
            if (servers != null && !servers.equals(featured)) {
                featured = servers;
                revision++;
            }
            backoff.connected(clock.getAsLong());
            SessionStore.Saved saved = sessions.load();
            if (saved == null) {
                accountName = null;
                setStatus(Status.SIGNED_OUT);
                return;
            }
            JsonObject resume = request("resume");
            resume.addProperty("token", saved.token());
            int run = signUpRun;
            send(resume, true, resumed -> {
                if (run != signUpRun) {
                    // Signed out on this device while the resume was out: it stays signed out.
                    return;
                }
                String resumeError = error(resumed);
                if (resumeError == null && signedIn(resumed, saved.username())) {
                    return;
                }
                if ("unauthorized".equals(resumeError) || "no_session".equals(resumeError)) {
                    // The session ended elsewhere (Sign Out Everywhere, a new password, 90 days unused).
                    sessions.clear();
                    accountName = null;
                    setStatus(Status.SIGNED_OUT);
                } else if (resumeError != null) {
                    log.warn("The Incari service couldn't resume the session (" + resumeError + ")", null);
                    dropConnection(null);
                }
            });
        });
    }

    private void receive(String text) {
        JsonObject message;
        try {
            message = JsonParser.parseString(text).getAsJsonObject();
        } catch (RuntimeException e) {
            log.warn("The Incari service sent a message this client can't read", e);
            return;
        }
        JsonElement id = message.get("id");
        if (id != null && id.isJsonPrimitive()) {
            Pending waiting;
            try {
                waiting = pending.remove(id.getAsInt());
            } catch (RuntimeException e) {
                log.warn("The Incari service sent an answer to no request", e);
                return;
            }
            if (waiting != null) {
                answer(waiting, message);
            }
            return;
        }
        try {
            event(message);
        } catch (RuntimeException e) {
            log.warn("The Incari service sent an event this client can't read", e);
        }
    }

    /**
     * Hands a request its answer. One the request can't read fails it as the service's trouble
     * instead, so that whatever waits on it (a screen saying "Sending...") hears something.
     */
    private void answer(Pending waiting, JsonObject message) {
        try {
            waiting.answer().accept(message);
        } catch (RuntimeException e) {
            log.warn("The Incari service sent an answer this client can't read", e);
            JsonObject unavailable = new JsonObject();
            unavailable.addProperty("t", "err");
            unavailable.addProperty("code", "unavailable");
            try {
                waiting.answer().accept(unavailable);
            } catch (RuntimeException again) {
                log.warn("A request's failure couldn't be handled", again);
            }
        }
    }

    /** Something the service says of its own accord. */
    private void event(JsonObject message) {
        String type = message.has("t") ? message.get("t").getAsString() : "";
        switch (type) {
            case "seen" -> world.seen(message.get("name").getAsString(), incariPlayer(message));
            case "gone" -> world.gone(message.get("pid").getAsString());
            case "cosmetics" -> world.cosmetics(message.get("pid").getAsString(), strings(message, "cosmetics"));
            case "badge" ->
                world.badge(
                        message.get("pid").getAsString(), message.get("badge").getAsBoolean());
            case "request" -> social.request(request(message));
            case "requestgone" -> social.requestGone(message.get("rid").getAsString());
            case "friend" -> social.friend(friend(message.getAsJsonObject("friend")), social.loaded());
            case "unfriended" -> social.unfriended(message.get("fid").getAsString());
            case "message" -> received(message);
            case "wardrobe" -> fromAccount(message);
            case "emote" -> {
                UUID player = world.uuid(message.get("pid").getAsString());
                Cosmetic emote = Catalog.byId(message.get("emote").getAsString());
                if (player != null && emote != null && emote.slot() == Cosmetic.Slot.EMOTE) {
                    emotes.add(new PlayedEmote(player, emote));
                }
            }
            case "signedout" -> {
                // Ended elsewhere: Sign Out Everywhere, a new password, the account deleted or banned.
                if (status == Status.SIGNED_IN) {
                    signedOutHere();
                }
            }
            default -> {
                // Events of later protocol additions are ignored by this client.
            }
        }
    }

    /** The connection is gone (or never opened): every waiting request fails, and a retry is set. */
    private void dropConnection(@Nullable Throwable cause) {
        boolean signedIn = status == Status.SIGNED_IN;
        closeConnection();
        if (status == Status.UPDATE_NEEDED || status == Status.OFF) {
            return;
        }
        if (cause != null && !signedIn) {
            log.warn("Can't reach the Incari service: " + cause.getMessage(), null);
        }
        long wait = backoff.failed(clock.getAsLong());
        reconnectAt = clock.getAsLong() + wait;
        setStatus(Status.CONNECTING);
        wardrobe.disconnected();
        emotes.clear();
        if (signedIn) {
            // Back on the next connection; the service forgot the world with this one.
            world.askAll();
        }
    }

    private void closeConnection() {
        NetConnection open = connection;
        connection = null;
        greeted = false;
        openDeadline = -1;
        generation++;
        if (open != null) {
            open.close();
        }
        failPending();
        // The service forgets a connection's place when the connection goes.
        forgetPresence();
    }

    private void failPending() {
        if (pending.isEmpty()) {
            return;
        }
        List<Pending> failed = new ArrayList<>(pending.values());
        pending.clear();
        JsonObject offline = offline();
        for (Pending each : failed) {
            answer(each, offline);
        }
    }

    private void expireRequests(long now) {
        List<Pending> expired = new ArrayList<>();
        for (Iterator<Pending> each = pending.values().iterator(); each.hasNext(); ) {
            Pending waiting = each.next();
            if (now >= waiting.deadline()) {
                expired.add(waiting);
                each.remove();
            }
        }
        JsonObject offline = offline();
        boolean signingIn = false;
        int ours = generation;
        for (Pending each : expired) {
            signingIn |= each.signsIn();
            answer(each, offline);
        }
        if (signingIn && ours == generation && connection != null) {
            // The service may have signed this connection in after all, and the client doesn't
            // know the account or its token: a fresh connection starts both sides signed out.
            log.warn("The Incari service didn't answer a sign-in in time; connecting again", null);
            dropConnection(null);
        }
    }

    /** What a request that never got its answer reads as. */
    private static JsonObject offline() {
        JsonObject answer = new JsonObject();
        answer.addProperty("t", "err");
        answer.addProperty("code", "offline");
        return answer;
    }

    private JsonObject request(String type) {
        JsonObject message = new JsonObject();
        message.addProperty("t", type);
        message.addProperty("id", nextId++);
        return message;
    }

    /** Sends a request; {@code answer} hears the service's answer, or an {@code offline} error. */
    private void send(JsonObject message, Consumer<JsonObject> answer) {
        send(message, false, answer);
    }

    /**
     * Sends a request; {@code answer} hears the service's answer, or an {@code offline} error.
     *
     * @param signsIn whether the request signs the connection in ({@code signup}, {@code signin},
     *     {@code recover}, {@code resume}): if it times out, the connection is opened again
     */
    private void send(JsonObject message, boolean signsIn, Consumer<JsonObject> answer) {
        NetConnection open = connection;
        String text = message.toString();
        if (open == null || text.length() > MAX_MESSAGE) {
            answer.accept(offline());
            return;
        }
        pending.put(
                message.get("id").getAsInt(), new Pending(clock.getAsLong() + REQUEST_TIMEOUT_MILLIS, answer, signsIn));
        open.sendText(text);
    }

    // ---------------------------------------------------------------------------------------------
    // Signing in and out
    // ---------------------------------------------------------------------------------------------

    /**
     * A sign-in's answer ({@code signup}, {@code signin}, {@code resume}, {@code recover}): keeps the
     * token, if there's a new one, and the account's settings. False if the answer is malformed.
     */
    private boolean signedIn(JsonObject answer, String username) {
        String name = username;
        try {
            JsonObject account = answer.has("account") ? answer.getAsJsonObject("account") : null;
            if (account != null) {
                if (account.has("username")) {
                    name = account.get("username").getAsString();
                }
                JsonObject settings = account.has("settings") ? account.getAsJsonObject("settings") : null;
                if (settings != null) {
                    readSettings(settings);
                }
            }
            if (answer.has("token")) {
                sessions.save(new SessionStore.Saved(name, answer.get("token").getAsString()));
            }
        } catch (RuntimeException e) {
            log.warn("The Incari service sent a sign-in this client can't read", e);
            return false;
        }
        accountName = name;
        setStatus(Status.SIGNED_IN);
        world.askAll();
        forgetPresence();
        syncPresence();
        loadFriends();
        loadWardrobe();
        return true;
    }

    private void readSettings(JsonObject settings) {
        if (settings.has("shareServer")) {
            shareServer = settings.get("shareServer").getAsBoolean();
        }
        // A service before ADR 0062 leaves it out: shown.
        badge = !settings.has("badge") || settings.get("badge").getAsBoolean();
        social.settings(
                settings.has("messages")
                        ? Social.MessagesFrom.of(settings.get("messages").getAsString())
                        : social.messagesFrom(),
                settings.has("friendRequests")
                        ? settings.get("friendRequests").getAsBoolean()
                        : social.friendRequests());
    }

    private void signedOutHere() {
        signUpRun++;
        social.clear();
        sessions.clear();
        wardrobe.signedOut();
        emotes.clear();
        accountName = null;
        shareServer = true;
        badge = true;
        world.askAll();
        if (connection != null && greeted) {
            setStatus(Status.SIGNED_OUT);
        } else {
            revision++;
        }
    }

    /**
     * Brings the service's idea of where the player is in line with the world: sends the place, or
     * clears it once a left world's grace is over, if that isn't what the service already holds.
     * One at a time; one the service turns away goes again later, each wait twice the last.
     */
    private void syncPresence() {
        if (status != Status.SIGNED_IN || connection == null || !greeted || presenceOut) {
            return;
        }
        long now = clock.getAsLong();
        if (now < presenceRetryAt) {
            return;
        }
        WorldPlayers.Place place = world.place();
        if (place == null && presenceGraceUntil >= 0) {
            if (now < presenceGraceUntil) {
                // Left moments ago: maybe a server switch, which joins the same place again.
                return;
            }
            presenceGraceUntil = -1;
        }
        String wanted = place == null ? "" : presenceKey(place);
        if (wanted.equals(presenceHeld)) {
            return;
        }
        JsonObject presence = request("presence");
        if (place == null) {
            presence.add("serverHash", JsonNull.INSTANCE);
        } else {
            presence.addProperty("serverHash", place.serverHash());
            presence.addProperty("name", place.name());
            if (shareServer) {
                // Only friends ever see it, and the service keeps it in memory only (ADR 0058).
                presence.addProperty("server", place.address());
            }
        }
        presenceOut = true;
        int ours = generation;
        send(presence, answer -> {
            if (ours != generation) {
                // The connection it went on is gone, and the service forgot the place with it.
                return;
            }
            presenceOut = false;
            String error = error(answer);
            if (error == null) {
                presenceHeld = wanted;
                presenceRetryDelay = PRESENCE_RETRY_MILLIS;
                // The world may have changed while this was out.
                syncPresence();
                return;
            }
            Failure failure = failure(answer, Failure.UNAVAILABLE);
            if (failure == Failure.REJECTED) {
                // Sending it again won't change the answer: wait for the next place.
                log.warn("The Incari service turned down the player's server (" + error + ")", null);
                presenceHeld = wanted;
            } else if (failure != Failure.SIGNED_OUT && failure != Failure.UPDATE) {
                // Limited (the service counts servers joined), offline or its trouble: later.
                if (failure != Failure.RATE_LIMITED && failure != Failure.OFFLINE) {
                    log.warn("The Incari service didn't take the player's server (" + error + ")", null);
                }
                presenceRetryAt = clock.getAsLong() + presenceRetryDelay;
                presenceRetryDelay = Math.min(presenceRetryDelay * 2, PRESENCE_RETRY_MAX_MILLIS);
            }
        });
    }

    /** What a presence for {@code place} says, to tell whether the service holds it already. */
    private String presenceKey(WorldPlayers.Place place) {
        return place.serverHash() + '\n' + place.name() + '\n' + (shareServer ? place.address() : "");
    }

    /** A new connection or account: the service holds no place for it yet. */
    private void forgetPresence() {
        presenceHeld = "";
        presenceOut = false;
        presenceRetryAt = 0;
        presenceRetryDelay = PRESENCE_RETRY_MILLIS;
    }

    private void lookup(List<String> names) {
        JsonObject form = request("lookup");
        JsonArray list = new JsonArray(names.size());
        for (String name : names) {
            list.add(name);
        }
        form.add("names", list);
        send(form, answer -> {
            if (error(answer) != null) {
                world.askAgain(names);
                return;
            }
            try {
                for (JsonElement each : answer.getAsJsonArray("players")) {
                    JsonObject player = each.getAsJsonObject();
                    world.seen(player.get("name").getAsString(), incariPlayer(player));
                }
            } catch (RuntimeException e) {
                log.warn("The Incari service sent a lookup answer this client can't read", e);
            }
        });
    }

    // ---------------------------------------------------------------------------------------------
    // Proof of work
    // ---------------------------------------------------------------------------------------------

    /** Solves {@code work} on the background pool, a step at a time, then hands the nonce to {@code found}. */
    private void solve(int run, ProofOfWork work, Consumer<Long> found) {
        background.execute(() -> {
            try {
                while (!work.step(POW_STEP)) {
                    // A cancelled sign-up stops; the field is read racily, which is fine for a stop flag.
                    if (run != signUpRun) {
                        return;
                    }
                    pause.run();
                }
                long nonce = work.nonce();
                synchronized (inbox) {
                    inbox.add(() -> found.accept(nonce));
                }
            } catch (RuntimeException e) {
                log.warn("Sign-up's proof of work failed", e);
            }
        });
    }

    // ---------------------------------------------------------------------------------------------
    // Answers
    // ---------------------------------------------------------------------------------------------

    private <T> boolean signedOutAndConnected(Consumer<Result<T>> done) {
        if (status != Status.SIGNED_OUT) {
            done.accept(Result.failed(status == Status.UPDATE_NEEDED ? Failure.UPDATE : Failure.OFFLINE));
            return false;
        }
        return true;
    }

    private <T> boolean signedInAndConnected(Consumer<Result<T>> done) {
        if (status == Status.SIGNED_IN && connection != null && greeted) {
            return true;
        }
        done.accept(Result.failed(
                status == Status.SIGNED_OUT
                        ? Failure.SIGNED_OUT
                        : status == Status.UPDATE_NEEDED ? Failure.UPDATE : Failure.OFFLINE));
        return false;
    }

    /** The answer's error code, or null for {@code ok}. */
    private static @Nullable String error(JsonObject answer) {
        String type = answer.has("t") ? answer.get("t").getAsString() : "";
        if (type.equals("ok")) {
            return null;
        }
        return answer.has("code") ? answer.get("code").getAsString() : "unavailable";
    }

    /**
     * Why the answer failed, or null if it didn't.
     *
     * @param unauthorized what {@code unauthorized} means for this request
     */
    private @Nullable Failure failure(JsonObject answer, Failure unauthorized) {
        String error = error(answer);
        if (error == null) {
            return null;
        }
        return switch (error) {
            case "unauthorized" -> unauthorized;
            case "no_session" -> {
                if (status == Status.SIGNED_IN) {
                    signedOutHere();
                }
                yield Failure.SIGNED_OUT;
            }
            case "taken" -> Failure.TAKEN;
            case "bad_request", "weak_password" -> Failure.REJECTED;
            case "rate_limited" -> Failure.RATE_LIMITED;
            case "offline" -> Failure.OFFLINE;
            case "update" -> {
                closeConnection();
                setStatus(Status.UPDATE_NEEDED);
                yield Failure.UPDATE;
            }
            default -> Failure.UNAVAILABLE;
        };
    }

    private Result<Void> done(JsonObject answer, Failure unauthorized) {
        Failure failure = failure(answer, unauthorized);
        return failure == null ? Result.done() : Result.failed(failure);
    }

    private static List<String> codes(JsonObject answer) {
        List<String> codes = new ArrayList<>();
        for (String code : strings(answer, "recoveryCodes")) {
            codes.add(RecoveryCodes.display(code));
        }
        return List.copyOf(codes);
    }

    /** {@code release.notes}: the first {@link Releases#MAX_NOTES} lines that are plain text, the rest dropped. */
    private static List<String> releaseNotes(JsonObject release) {
        if (!(release.get("notes") instanceof JsonArray list)) {
            return List.of();
        }
        List<String> notes = new ArrayList<>();
        for (JsonElement entry : list) {
            if (notes.size() < Releases.MAX_NOTES
                    && entry instanceof JsonPrimitive note
                    && note.isString()
                    && Releases.note(note.getAsString())) {
                notes.add(note.getAsString().strip());
            }
        }
        return List.copyOf(notes);
    }

    /** {@code hello}'s {@code featured}, the entries in bounds; null if it named none. */
    private static @Nullable List<Featured> featured(JsonObject answer) {
        if (!(answer.get("featured") instanceof JsonArray list)) {
            return null;
        }
        List<Featured> servers = new ArrayList<>();
        for (JsonElement entry : list) {
            if (servers.size() < Featured.MAX
                    && entry instanceof JsonObject server
                    && server.get("name") instanceof JsonPrimitive name
                    && name.isString()
                    && server.get("address") instanceof JsonPrimitive address
                    && address.isString()
                    && Featured.valid(name.getAsString(), address.getAsString())) {
                servers.add(new Featured(
                        name.getAsString().strip(), address.getAsString().strip()));
            }
        }
        return List.copyOf(servers);
    }

    private static IncariPlayer incariPlayer(JsonObject json) {
        boolean friend = json.has("friend") && json.get("friend").getAsBoolean();
        // A service before ADR 0062 leaves the badge out: every account shows it.
        boolean badge = !json.has("badge") || json.get("badge").getAsBoolean();
        return new IncariPlayer(json.get("pid").getAsString(), friend, strings(json, "cosmetics"), badge);
    }

    private static List<String> strings(JsonObject json, String key) {
        if (!json.has(key) || !json.get(key).isJsonArray()) {
            return List.of();
        }
        List<String> out = new ArrayList<>();
        for (JsonElement each : json.getAsJsonArray(key)) {
            out.add(each.getAsString());
        }
        return List.copyOf(out);
    }

    private void setStatus(Status next) {
        if (status != next) {
            status = next;
            revision++;
        }
    }
}
