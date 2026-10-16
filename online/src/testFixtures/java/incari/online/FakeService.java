package incari.online;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import incari.platform.api.net.NetConnection;
import incari.platform.api.net.PlatformNetwork;
import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import org.jspecify.annotations.Nullable;

/**
 * Incari's service in the test's process, speaking {@code docs/online-protocol.md} as the backend
 * does, closely enough to drive {@link OnlineService} through every request. Answers arrive at once
 * (queued by the service for its next tick, as real ones are). Not a model of the backend's security:
 * no hashing, no rate limits unless a test sets one. {@code tools/onlinedev} puts it behind a real
 * WebSocket for trying the client's screens before the backend exists.
 *
 * <p>One thread at a time.
 */
public final class FakeService implements PlatformNetwork {

    static final int BITS = 8;

    final Map<String, Account> accounts = new HashMap<>();
    final List<Socket> sockets = new ArrayList<>();
    /** Every message the clients sent, for tests that check what went over the wire. */
    final List<JsonObject> received = new ArrayList<>();

    int protocol = OnlineService.PROTOCOL;
    boolean reachable = true;
    boolean answerHello = true;

    /** The latest release {@code hello} names, or none. */
    @Nullable
    String release;

    /** That release's changelog, sent as {@code release.notes} when there's any. */
    List<String> releaseNotes = List.of();

    /** The featured servers {@code hello} names as {name, address} (ADR 0062), or none. */
    @Nullable
    List<String[]> featured;

    /** The {@code count}s heard (ADR 0062): install, version and kind, one array each. */
    final List<String[]> counts = new ArrayList<>();

    @Nullable
    String limitNext;

    /** Request types the service takes and never answers, as a stalled service would. */
    final Set<String> unanswered = new HashSet<>();

    /** Friendships, each as its two usernames' keys joined by "+", lower first. */
    final List<String> friendships = new ArrayList<>();

    /** Friend requests by rid: {from key, to key, the label the sender gave}. */
    final Map<String, String[]> requests = new java.util.LinkedHashMap<>();

    /** Messages between friends: {fid, sender key, text, mid, at}. */
    final List<String[]> messages = new ArrayList<>();

    /** How far into {@link #messages} each "fid/reader key" has read. */
    private final Map<String, Integer> readUpTo = new HashMap<>();

    private int ids;
    private int tokens;
    private int challenges;
    private final Map<String, Boolean> openChallenges = new HashMap<>();

    static final class Account {
        final String username;
        String password;
        final List<String> tokens = new ArrayList<>();
        List<String> codes = new ArrayList<>();
        final List<String> blocks = new ArrayList<>();
        boolean shareServer = true;
        String messages = "friends";
        boolean friendRequests = true;
        boolean badge = true;
        /** What the account wears (ADR 0061), and the cosmetics it was given past the free ones. */
        List<String> equipped = new ArrayList<>();

        final Set<String> granted = new HashSet<>();

        Account(String username, String password) {
            this.username = username;
            this.password = password;
        }
    }

    /** One client's connection. */
    final class Socket implements NetConnection {
        final NetConnection.Listener listener;

        @Nullable
        Account account;

        @Nullable
        String serverHash;

        @Nullable
        String name;

        @Nullable
        String server;

        /** Names this connection has looked up, in lower case: only these get {@code seen} and {@code gone}. */
        final Set<String> asked = new HashSet<>();

        boolean open = true;

        Socket(NetConnection.Listener listener) {
            this.listener = listener;
        }

        @Override
        public void send(ByteBuffer data) {
            throw new AssertionError("The client sent binary");
        }

        @Override
        public void sendText(String text) {
            if (open) {
                JsonObject message = JsonParser.parseString(text).getAsJsonObject();
                received.add(message);
                handle(this, message);
            }
        }

        @Override
        public void close() {
            if (open) {
                open = false;
                listener.closed(null);
            }
        }

        @Override
        public boolean isOpen() {
            return open;
        }

        @Override
        public String remoteDescription() {
            return "fake";
        }

        void say(JsonObject message) {
            if (open) {
                listener.receivedText(message.toString());
            }
        }

        /** The service drops the connection, as on a restart. */
        void drop() {
            if (open) {
                open = false;
                listener.closed(new java.io.IOException("dropped"));
            }
        }
    }

    /** Names {@code version} as the latest release in every {@code hello} answer from now on (null: none). */
    public void announceRelease(@Nullable String version, String... notes) {
        release = version;
        releaseNotes = List.of(notes);
    }

    /** The client went away (its WebSocket closed): what it said about where it was goes, as on the backend. */
    public void disconnected(NetConnection connection) {
        if (connection instanceof Socket socket && socket.open) {
            left(socket);
            socket.open = false;
            Account account = socket.account;
            socket.account = null;
            if (account != null) {
                presenceChanged(account);
            }
        }
    }

    @Override
    public NetConnection connect(String host, int port, NetConnection.Listener listener) {
        throw new UnsupportedOperationException();
    }

    @Override
    public NetConnection openWebSocket(String url, NetConnection.Listener listener) {
        Socket socket = new Socket(listener);
        if (!reachable) {
            socket.open = false;
            listener.closed(new java.io.IOException("unreachable"));
            return socket;
        }
        sockets.add(socket);
        listener.connected();
        return socket;
    }

    Socket last() {
        return sockets.get(sockets.size() - 1);
    }

    private void handle(Socket socket, JsonObject message) {
        String type = message.get("t").getAsString();
        int id = message.get("id").getAsInt();
        if (limitNext != null && limitNext.equals(type)) {
            limitNext = null;
            socket.say(err(id, "rate_limited"));
            return;
        }
        if (unanswered.contains(type)) {
            return;
        }
        switch (type) {
            case "hello" -> {
                if (!answerHello) {
                    return;
                }
                if (message.get("v").getAsInt() != protocol) {
                    socket.say(err(id, "update"));
                    return;
                }
                JsonObject ok = ok(id);
                ok.addProperty("v", protocol);
                if (release != null) {
                    JsonObject named = new JsonObject();
                    named.addProperty("version", release);
                    if (!releaseNotes.isEmpty()) {
                        JsonArray notes = new JsonArray();
                        releaseNotes.forEach(notes::add);
                        named.add("notes", notes);
                    }
                    ok.add("release", named);
                }
                if (featured != null) {
                    JsonArray servers = new JsonArray();
                    for (String[] server : featured) {
                        JsonObject entry = new JsonObject();
                        entry.addProperty("name", server[0]);
                        entry.addProperty("address", server[1]);
                        servers.add(entry);
                    }
                    ok.add("featured", servers);
                }
                socket.say(ok);
            }
            case "count" -> {
                String install = message.get("install").getAsString();
                if (install.length() != 22 || !message.has("version") || !message.has("kind")) {
                    socket.say(err(id, "bad_request"));
                    return;
                }
                counts.add(new String[] {
                    install,
                    message.get("version").getAsString(),
                    message.get("kind").getAsString()
                });
                socket.say(ok(id));
            }
            case "challenge" -> {
                byte[] bytes = new byte[16];
                bytes[0] = (byte) ++challenges;
                String challenge = base64(bytes);
                openChallenges.put(challenge, true);
                JsonObject ok = ok(id);
                ok.addProperty("challenge", challenge);
                ok.addProperty("bits", BITS);
                socket.say(ok);
            }
            case "signup" -> {
                String challenge = message.get("challenge").getAsString();
                long nonce = message.get("nonce").getAsLong();
                // As the service: used up by a right nonce only.
                if (!openChallenges.containsKey(challenge)
                        || !new ProofOfWork(ProofOfWork.base64Url(challenge), BITS).passes(nonce)) {
                    socket.say(err(id, "bad_request"));
                    return;
                }
                openChallenges.remove(challenge);
                String username = message.get("username").getAsString();
                if (accounts.containsKey(key(username))) {
                    socket.say(err(id, "taken"));
                    return;
                }
                Account account = new Account(username, message.get("password").getAsString());
                accounts.put(key(username), account);
                account.codes = newCodes();
                JsonObject ok = signIn(socket, account, id);
                JsonArray codes = new JsonArray();
                account.codes.forEach(codes::add);
                ok.add("recoveryCodes", codes);
                socket.say(ok);
            }
            case "signin" -> {
                Account account = accounts.get(key(message.get("username").getAsString()));
                if (account == null
                        || !account.password.equals(message.get("password").getAsString())) {
                    socket.say(err(id, "unauthorized"));
                    return;
                }
                socket.say(signIn(socket, account, id));
            }
            case "resume" -> {
                String token = message.get("token").getAsString();
                for (Account account : accounts.values()) {
                    if (account.tokens.contains(token)) {
                        socket.account = account;
                        presenceChanged(account);
                        JsonObject ok = ok(id);
                        ok.add("account", account(account));
                        socket.say(ok);
                        return;
                    }
                }
                socket.say(err(id, "unauthorized"));
            }
            case "recover" -> {
                Account account = accounts.get(key(message.get("username").getAsString()));
                String code = message.get("code").getAsString();
                if (account == null || !account.codes.remove(code)) {
                    socket.say(err(id, "unauthorized"));
                    return;
                }
                account.password = message.get("newPassword").getAsString();
                account.tokens.clear();
                endOtherSessions(account, null);
                socket.say(signIn(socket, account, id));
            }
            default -> signedIn(socket, type, id, message);
        }
    }

    private void signedIn(Socket socket, String type, int id, JsonObject message) {
        Account account = socket.account;
        if (account == null) {
            socket.say(err(id, "no_session"));
            return;
        }
        switch (type) {
            case "presence" -> {
                JsonElement hash = message.get("serverHash");
                if (hash == null || hash.isJsonNull()) {
                    left(socket);
                } else {
                    left(socket);
                    socket.serverHash = hash.getAsString();
                    socket.name = message.get("name").getAsString();
                    socket.server =
                            message.has("server") ? message.get("server").getAsString() : null;
                    for (Socket other : sockets) {
                        if (other != socket
                                && other.open
                                && socket.serverHash.equals(other.serverHash)
                                && other.asked.contains(socket.name.toLowerCase(Locale.ROOT))) {
                            other.say(seen(socket));
                        }
                    }
                }
                socket.say(ok(id));
            }
            case "lookup" -> {
                JsonArray players = new JsonArray();
                for (JsonElement name : message.getAsJsonArray("names")) {
                    socket.asked.add(name.getAsString().toLowerCase(Locale.ROOT));
                    for (Socket other : sockets) {
                        if (other != socket
                                && other.open
                                && other.account != null
                                && socket.serverHash != null
                                && socket.serverHash.equals(other.serverHash)
                                && name.getAsString().equals(other.name)
                                && !blocked(socket, other)) {
                            players.add(player(other));
                        }
                    }
                }
                JsonObject ok = ok(id);
                ok.add("players", players);
                socket.say(ok);
            }
            case "settings" -> {
                if (message.has("shareServer")) {
                    account.shareServer = message.get("shareServer").getAsBoolean();
                }
                if (message.has("messages")) {
                    account.messages = message.get("messages").getAsString();
                }
                if (message.has("friendRequests")) {
                    account.friendRequests = message.get("friendRequests").getAsBoolean();
                }
                boolean badgeChanged =
                        message.has("badge") && message.get("badge").getAsBoolean() != account.badge;
                if (message.has("badge")) {
                    account.badge = message.get("badge").getAsBoolean();
                }
                socket.say(ok(id));
                if (badgeChanged) {
                    // As cosmetics: everyone who looked the account up on a server it's on.
                    for (Socket other : sockets) {
                        for (Socket wearer : sockets) {
                            if (wearer.account == account
                                    && other.open
                                    && wearer.open
                                    && other.account != account
                                    && wearer.serverHash != null
                                    && wearer.serverHash.equals(other.serverHash)
                                    && wearer.name != null
                                    && other.asked.contains(wearer.name.toLowerCase(Locale.ROOT))
                                    && !blocked(other, wearer)) {
                                JsonObject changed = event("badge");
                                changed.addProperty("pid", pid(wearer));
                                changed.addProperty("badge", account.badge);
                                other.say(changed);
                            }
                        }
                    }
                }
            }
            case "friends" -> {
                if (!friendsSpoken) {
                    socket.say(err(id, "bad_request"));
                    return;
                }
                JsonArray friends = new JsonArray();
                for (String pair : friendships) {
                    String other = otherOf(pair, key(account.username));
                    if (other != null) {
                        JsonObject entry = friendEntry(pair, accounts.get(other));
                        entry.addProperty("unread", unread(pair, key(account.username)));
                        friends.add(entry);
                    }
                }
                JsonArray list = new JsonArray();
                for (Map.Entry<String, String[]> each : requests.entrySet()) {
                    String[] r = each.getValue();
                    if (r[0].equals(key(account.username)) || r[1].equals(key(account.username))) {
                        list.add(requestEntry(each.getKey(), r, key(account.username)));
                    }
                }
                JsonObject ok = ok(id);
                ok.add("friends", friends);
                ok.add("requests", list);
                socket.say(ok);
            }
            case "befriend" -> befriend(socket, account, id, message);
            case "accept", "decline" -> {
                String rid = message.get("rid").getAsString();
                String[] r = requests.get(rid);
                String me = key(account.username);
                if (r == null || (!r[1].equals(me) && !(type.equals("decline") && r[0].equals(me)))) {
                    socket.say(ok(id));
                    return;
                }
                requests.remove(rid);
                String other = r[0].equals(me) ? r[1] : r[0];
                JsonObject gone = event("requestgone");
                gone.addProperty("rid", rid);
                tell(other, gone);
                JsonObject ok = ok(id);
                if (type.equals("accept")) {
                    String pair = pair(me, other);
                    friendships.add(pair);
                    ok.add("friend", friendEntry(pair, accounts.get(other)));
                    JsonObject event = event("friend");
                    event.add("friend", friendEntry(pair, account));
                    tell(other, event);
                }
                socket.say(ok);
            }
            case "unfriend" -> {
                String fid = message.get("fid").getAsString();
                String other = otherOf(fid, key(account.username));
                if (other != null && friendships.remove(fid)) {
                    JsonObject event = event("unfriended");
                    event.addProperty("fid", fid);
                    tell(other, event);
                }
                socket.say(ok(id));
            }
            case "send" -> send(socket, account, id, message);
            case "history" -> {
                String fid = message.get("fid").getAsString();
                String before = message.has("before") ? message.get("before").getAsString() : null;
                List<String[]> found = new ArrayList<>();
                for (String[] each : messages) {
                    if (each[0].equals(fid)) {
                        if (each[3].equals(before)) {
                            break;
                        }
                        found.add(each);
                    }
                }
                int from = Math.max(0, found.size() - HISTORY_PAGE);
                JsonArray page = new JsonArray();
                for (String[] each : found.subList(from, found.size())) {
                    JsonObject entry = new JsonObject();
                    entry.addProperty("mid", each[3]);
                    entry.addProperty("from", each[1].equals(key(account.username)) ? "me" : "them");
                    entry.addProperty("text", each[2]);
                    entry.addProperty("at", Long.parseLong(each[4]));
                    page.add(entry);
                }
                JsonObject ok = ok(id);
                ok.add("messages", page);
                ok.addProperty("more", from > 0);
                socket.say(ok);
            }
            case "read" -> {
                String fid = message.get("fid").getAsString();
                String upTo = message.has("mid") ? message.get("mid").getAsString() : null;
                // Up to the given message (or every one), as the backend keeps it.
                int at = messages.size();
                for (int i = 0; i < messages.size(); i++) {
                    if (messages.get(i)[3].equals(upTo)) {
                        at = i + 1;
                    }
                }
                readUpTo.put(fid + "/" + key(account.username), at);
                socket.say(ok(id));
            }
            case "password" -> {
                if (!account.password.equals(message.get("old").getAsString())) {
                    socket.say(err(id, "unauthorized"));
                    return;
                }
                account.password = message.get("new").getAsString();
                endOtherSessions(account, socket);
                socket.say(ok(id));
            }
            case "codes" -> {
                if (!account.password.equals(message.get("password").getAsString())) {
                    socket.say(err(id, "unauthorized"));
                    return;
                }
                account.codes = newCodes();
                JsonObject ok = ok(id);
                JsonArray codes = new JsonArray();
                account.codes.forEach(codes::add);
                ok.add("recoveryCodes", codes);
                socket.say(ok);
            }
            case "signout" -> {
                if (message.has("everywhere") && message.get("everywhere").getAsBoolean()) {
                    account.tokens.clear();
                    endOtherSessions(account, socket);
                }
                left(socket);
                socket.account = null;
                socket.say(ok(id));
            }
            case "delete" -> {
                if (!account.password.equals(message.get("password").getAsString())) {
                    socket.say(err(id, "unauthorized"));
                    return;
                }
                accounts.remove(key(account.username));
                endOtherSessions(account, socket);
                left(socket);
                socket.account = null;
                socket.say(ok(id));
            }
            case "block" -> {
                String target;
                if (message.has("pid")) {
                    target = message.get("pid").getAsString();
                    for (Socket other : sockets) {
                        if (other.account != null && pid(other).equals(target)) {
                            account.blocks.add(other.account.username + "|"
                                    + message.get("name").getAsString());
                        }
                    }
                } else {
                    Account other = accounts.get(key(message.get("username").getAsString()));
                    if (other != null) {
                        account.blocks.add(other.username + "|" + other.username);
                    }
                }
                endFriendship(account);
                socket.say(ok(id));
            }
            case "blocks" -> {
                JsonArray blocks = new JsonArray();
                for (String block : account.blocks) {
                    JsonObject entry = new JsonObject();
                    entry.addProperty("bid", "b-" + block.substring(0, block.indexOf('|')));
                    entry.addProperty("label", block.substring(block.indexOf('|') + 1));
                    blocks.add(entry);
                }
                JsonObject ok = ok(id);
                ok.add("blocks", blocks);
                socket.say(ok);
            }
            case "unblock" -> {
                String username = message.get("bid").getAsString().substring(2);
                account.blocks.removeIf(block -> block.startsWith(username + "|"));
                socket.say(ok(id));
            }
            case "report" -> socket.say(ok(id));
            case "wardrobe" -> {
                if (!cosmeticsSpoken) {
                    socket.say(err(id, "bad_request"));
                    return;
                }
                JsonObject ok = wardrobe(account);
                ok.addProperty("t", "ok");
                ok.addProperty("id", id);
                socket.say(ok);
            }
            case "equip" -> equip(socket, account, id, message);
            case "emote" -> emote(socket, account, id, message);
            default -> socket.say(err(id, "bad_request"));
        }
    }

    /** Whether the fake answers cosmetics' requests; false acts as a service before cosmetics. */
    boolean cosmeticsSpoken = true;

    /** Gives {@code username}'s account a cosmetic that isn't free, as the backend's grants would. */
    public void grant(String username, String cosmeticId) {
        Account account = accounts.get(key(username));
        if (account != null) {
            account.granted.add(cosmeticId);
            JsonObject event = wardrobe(account);
            event.addProperty("t", "wardrobe");
            tell(key(username), event);
        }
    }

    private static JsonObject wardrobe(Account account) {
        JsonObject json = new JsonObject();
        JsonArray owned = new JsonArray();
        for (Cosmetic each : Catalog.all()) {
            if (each.free() || account.granted.contains(each.id())) {
                owned.add(each.id());
            }
        }
        json.add("owned", owned);
        JsonArray equipped = new JsonArray();
        account.equipped.forEach(equipped::add);
        json.add("equipped", equipped);
        return json;
    }

    private void equip(Socket socket, Account account, int id, JsonObject message) {
        if (!cosmeticsSpoken) {
            socket.say(err(id, "bad_request"));
            return;
        }
        List<String> next = new ArrayList<>();
        Set<Cosmetic.Slot> slots = new HashSet<>();
        for (JsonElement each : message.getAsJsonArray("cosmetics")) {
            Cosmetic cosmetic = Catalog.byId(each.getAsString());
            // As the service: unknown ids, emotes and two in a slot are malformed.
            if (cosmetic == null || !cosmetic.slot().worn() || !slots.add(cosmetic.slot())) {
                socket.say(err(id, "bad_request"));
                return;
            }
            if (!cosmetic.free() && !account.granted.contains(cosmetic.id())) {
                socket.say(err(id, "not_owned"));
                return;
            }
            next.add(cosmetic.id());
        }
        account.equipped = next;
        JsonObject ok = ok(id);
        JsonArray equipped = new JsonArray();
        next.forEach(equipped::add);
        ok.add("equipped", equipped);
        socket.say(ok);
        // The account's other sessions, and the players who see it on its server.
        JsonObject event = wardrobe(account);
        event.addProperty("t", "wardrobe");
        for (Socket other : sockets) {
            if (other == socket || !other.open) {
                continue;
            }
            if (other.account == account) {
                other.say(event);
            }
            for (Socket wearer : sockets) {
                if (wearer.account == account
                        && wearer.open
                        && wearer.serverHash != null
                        && wearer.serverHash.equals(other.serverHash)
                        && wearer.name != null
                        && other.account != account
                        && other.asked.contains(wearer.name.toLowerCase(Locale.ROOT))
                        && !blocked(other, wearer)) {
                    JsonObject changed = event("cosmetics");
                    changed.addProperty("pid", pid(wearer));
                    changed.add("cosmetics", equipped.deepCopy());
                    other.say(changed);
                }
            }
        }
    }

    private void emote(Socket socket, Account account, int id, JsonObject message) {
        Cosmetic emote = Catalog.byId(message.get("emote").getAsString());
        if (!cosmeticsSpoken || emote == null || emote.slot() != Cosmetic.Slot.EMOTE) {
            socket.say(err(id, "bad_request"));
            return;
        }
        if (!emote.free() && !account.granted.contains(emote.id())) {
            socket.say(err(id, "not_owned"));
            return;
        }
        socket.say(ok(id));
        if (socket.serverHash == null || socket.name == null) {
            return;
        }
        JsonObject event = event("emote");
        event.addProperty("pid", pid(socket));
        event.addProperty("emote", emote.id());
        for (Socket other : sockets) {
            if (other != socket
                    && other.open
                    && socket.serverHash.equals(other.serverHash)
                    && other.asked.contains(socket.name.toLowerCase(Locale.ROOT))
                    && !blocked(other, socket)) {
                other.say(event);
            }
        }
    }

    /** Whether the fake answers friends' requests; false acts as a service before the backend's B4. */
    boolean friendsSpoken = true;

    static final int HISTORY_PAGE = 50;

    private void befriend(Socket socket, Account account, int id, JsonObject message) {
        Account target = null;
        String label;
        if (message.has("pid")) {
            label = message.get("name").getAsString();
            for (Socket other : sockets) {
                if (other.account != null
                        && other.open
                        && other.serverHash != null
                        && other.serverHash.equals(socket.serverHash)
                        && pid(other).equals(message.get("pid").getAsString())) {
                    target = other.account;
                }
            }
        } else {
            label = message.get("username").getAsString();
            target = accounts.get(key(label));
        }
        if (target == null || target == account) {
            socket.say(err(id, "bad_request"));
            return;
        }
        String me = key(account.username);
        String them = key(target.username);
        if (friendships.contains(pair(me, them))) {
            socket.say(ok(id));
            return;
        }
        for (Map.Entry<String, String[]> each : List.copyOf(requests.entrySet())) {
            String[] r = each.getValue();
            if (r[0].equals(them) && r[1].equals(me)) {
                // They asked first: friends now.
                requests.remove(each.getKey());
                String pair = pair(me, them);
                friendships.add(pair);
                JsonObject gone = event("requestgone");
                gone.addProperty("rid", each.getKey());
                tell(them, gone);
                JsonObject event = event("friend");
                event.add("friend", friendEntry(pair, account));
                tell(them, event);
                JsonObject ok = ok(id);
                ok.add("friend", friendEntry(pair, target));
                socket.say(ok);
                return;
            }
        }
        for (Map.Entry<String, String[]> each : requests.entrySet()) {
            if (each.getValue()[0].equals(me) && each.getValue()[1].equals(them)) {
                JsonObject ok = ok(id);
                ok.add("request", requestEntry(each.getKey(), each.getValue(), me));
                socket.say(ok);
                return;
            }
        }
        String rid = "r" + ++ids;
        String[] r = {me, them, label};
        requests.put(rid, r);
        if (target.friendRequests && !blocks(account, target)) {
            JsonObject event = event("request");
            event.addProperty("rid", rid);
            event.addProperty("incoming", true);
            event.addProperty("label", account.username);
            tell(them, event);
        }
        JsonObject ok = ok(id);
        ok.add("request", requestEntry(rid, r, me));
        socket.say(ok);
    }

    private void send(Socket socket, Account account, int id, JsonObject message) {
        String text = message.get("text").getAsString();
        String mid = "m" + ++ids;
        long at = System.currentTimeMillis();
        if (message.has("fid")) {
            String fid = message.get("fid").getAsString();
            String other = otherOf(fid, key(account.username));
            Account target = other == null ? null : accounts.get(other);
            if (target == null || !friendships.contains(fid) || target.messages.equals("nobody")) {
                socket.say(err(id, "closed"));
                return;
            }
            messages.add(new String[] {fid, key(account.username), text, mid, Long.toString(at)});
            JsonObject event = event("message");
            event.addProperty("mid", mid);
            event.addProperty("at", at);
            event.addProperty("text", text);
            event.addProperty("fid", fid);
            tell(other, event);
        } else {
            String pid = message.get("pid").getAsString();
            Socket target = null;
            for (Socket other : sockets) {
                if (other.open
                        && other.account != null
                        && other.serverHash != null
                        && other.serverHash.equals(socket.serverHash)
                        && pid(other).equals(pid)) {
                    target = other;
                }
            }
            if (target == null
                    || target.account == null
                    || !target.account.messages.equals("everyone")
                    || blocks(account, target.account)) {
                socket.say(err(id, "closed"));
                return;
            }
            JsonObject event = event("message");
            event.addProperty("mid", mid);
            event.addProperty("at", at);
            event.addProperty("text", text);
            event.addProperty("pid", pid(socket));
            event.addProperty("name", socket.name);
            target.say(event);
        }
        JsonObject ok = ok(id);
        ok.addProperty("mid", mid);
        ok.addProperty("at", at);
        socket.say(ok);
    }

    /** A block ends any friendship and request between the two, telling the other side. */
    private void endFriendship(Account account) {
        for (String block : account.blocks) {
            Account other = accounts.get(key(block.substring(0, block.indexOf('|'))));
            if (other == null) {
                continue;
            }
            String me = key(account.username);
            String them = key(other.username);
            String pair = pair(me, them);
            if (friendships.remove(pair)) {
                JsonObject event = event("unfriended");
                event.addProperty("fid", pair);
                tell(them, event);
                tell(me, event);
            }
            for (Map.Entry<String, String[]> each : List.copyOf(requests.entrySet())) {
                String[] r = each.getValue();
                if ((r[0].equals(me) && r[1].equals(them)) || (r[0].equals(them) && r[1].equals(me))) {
                    requests.remove(each.getKey());
                    JsonObject gone = event("requestgone");
                    gone.addProperty("rid", each.getKey());
                    tell(them, gone);
                    tell(me, gone);
                }
            }
        }
    }

    private boolean blocks(Account a, Account b) {
        for (String block : a.blocks) {
            if (block.startsWith(b.username + "|")) {
                return true;
            }
        }
        for (String block : b.blocks) {
            if (block.startsWith(a.username + "|")) {
                return true;
            }
        }
        return false;
    }

    /** The friend's messages in {@code fid} that {@code reader} hasn't marked read. */
    private int unread(String fid, String reader) {
        int from = readUpTo.getOrDefault(fid + "/" + reader, 0);
        int count = 0;
        for (int i = from; i < messages.size(); i++) {
            String[] each = messages.get(i);
            if (each[0].equals(fid) && !each[1].equals(reader)) {
                count++;
            }
        }
        return count;
    }

    private JsonObject friendEntry(String fid, @Nullable Account friend) {
        JsonObject json = new JsonObject();
        json.addProperty("fid", fid);
        json.addProperty("username", friend == null ? "?" : friend.username);
        boolean online = false;
        String server = null;
        for (Socket each : sockets) {
            if (each.open && each.account == friend) {
                online = true;
                if (friend != null && friend.shareServer && each.server != null) {
                    server = each.server;
                }
            }
        }
        json.addProperty("online", online);
        if (server != null) {
            json.addProperty("server", server);
        }
        return json;
    }

    private static JsonObject requestEntry(String rid, String[] request, String asker) {
        JsonObject json = new JsonObject();
        json.addProperty("rid", rid);
        boolean incoming = request[1].equals(asker);
        json.addProperty("incoming", incoming);
        json.addProperty("label", incoming ? request[0] : request[2]);
        return json;
    }

    private void tell(String accountKey, JsonObject event) {
        Account target = accounts.get(accountKey);
        for (Socket each : List.copyOf(sockets)) {
            if (each.open && target != null && each.account == target) {
                each.say(event);
            }
        }
    }

    private static String pair(String a, String b) {
        return a.compareTo(b) < 0 ? a + "+" + b : b + "+" + a;
    }

    private static @Nullable String otherOf(String pair, String me) {
        int plus = pair.indexOf('+');
        if (plus < 0) {
            return null;
        }
        String first = pair.substring(0, plus);
        String second = pair.substring(plus + 1);
        return first.equals(me) ? second : second.equals(me) ? first : null;
    }

    private static JsonObject event(String type) {
        JsonObject json = new JsonObject();
        json.addProperty("t", type);
        return json;
    }

    private boolean blocked(Socket asker, Socket other) {
        Account a = asker.account;
        Account b = other.account;
        if (a == null || b == null) {
            return true;
        }
        for (String block : a.blocks) {
            if (block.startsWith(b.username + "|")) {
                return true;
            }
        }
        for (String block : b.blocks) {
            if (block.startsWith(a.username + "|")) {
                return true;
            }
        }
        return false;
    }

    private void left(Socket socket) {
        if (socket.serverHash == null || socket.account == null) {
            socket.serverHash = null;
            return;
        }
        JsonObject gone = new JsonObject();
        gone.addProperty("t", "gone");
        gone.addProperty("pid", pid(socket));
        for (Socket other : sockets) {
            if (other != socket
                    && other.open
                    && socket.serverHash.equals(other.serverHash)
                    && socket.name != null
                    && other.asked.contains(socket.name.toLowerCase(Locale.ROOT))) {
                other.say(gone);
            }
        }
        socket.serverHash = null;
    }

    private void endOtherSessions(Account account, @Nullable Socket keep) {
        for (Socket other : sockets) {
            if (other != keep && other.account == account && other.open) {
                other.account = null;
                JsonObject event = new JsonObject();
                event.addProperty("t", "signedout");
                other.say(event);
            }
        }
    }

    /** Tells {@code account}'s friends how it stands now: online or not, and where. */
    private void presenceChanged(Account account) {
        for (String pair : friendships) {
            String other = otherOf(pair, key(account.username));
            if (other != null) {
                JsonObject event = event("friend");
                event.add("friend", friendEntry(pair, account));
                tell(other, event);
            }
        }
    }

    private JsonObject signIn(Socket socket, Account account, int id) {
        String token = "token-" + ++tokens;
        account.tokens.add(token);
        socket.account = account;
        presenceChanged(account);
        JsonObject ok = ok(id);
        ok.addProperty("token", token);
        ok.add("account", account(account));
        return ok;
    }

    private static JsonObject account(Account account) {
        JsonObject json = new JsonObject();
        json.addProperty("username", account.username);
        JsonObject settings = new JsonObject();
        settings.addProperty("shareServer", account.shareServer);
        settings.addProperty("messages", account.messages);
        settings.addProperty("friendRequests", account.friendRequests);
        settings.addProperty("badge", account.badge);
        json.add("settings", settings);
        return json;
    }

    private JsonObject seen(Socket socket) {
        JsonObject event = player(socket);
        event.addProperty("t", "seen");
        return event;
    }

    private JsonObject player(Socket socket) {
        JsonObject json = new JsonObject();
        json.addProperty("name", socket.name);
        json.addProperty("pid", pid(socket));
        json.addProperty("friend", false);
        JsonArray cosmetics = new JsonArray();
        if (socket.account != null) {
            socket.account.equipped.forEach(cosmetics::add);
        }
        json.add("cosmetics", cosmetics);
        json.addProperty("badge", socket.account == null || socket.account.badge);
        return json;
    }

    private static String pid(Socket socket) {
        Account account = socket.account;
        return account == null ? "" : Integer.toHexString((account.username + socket.serverHash).hashCode());
    }

    private List<String> newCodes() {
        List<String> codes = new ArrayList<>();
        for (int i = 0; i < 10; i++) {
            codes.add(String.format(Locale.ROOT, "AB%02d-CDEF-GHJK", tokens * 10 + i)
                    .substring(0, 14));
        }
        return codes;
    }

    private static String key(String username) {
        return username.toLowerCase(Locale.ROOT);
    }

    private static JsonObject ok(int id) {
        JsonObject json = new JsonObject();
        json.addProperty("t", "ok");
        json.addProperty("id", id);
        return json;
    }

    private static JsonObject err(int id, String code) {
        JsonObject json = new JsonObject();
        json.addProperty("t", "err");
        json.addProperty("id", id);
        json.addProperty("code", code);
        return json;
    }

    private static String base64(byte[] bytes) {
        return java.util.Base64.getUrlEncoder().withoutPadding().encodeToString(bytes);
    }
}
