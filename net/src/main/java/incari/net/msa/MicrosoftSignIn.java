package incari.net.msa;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParseException;
import com.google.gson.JsonParser;
import incari.net.https.HttpRequest;
import incari.net.https.HttpResponse;
import incari.net.https.Https;
import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import java.util.function.LongSupplier;
import org.jspecify.annotations.Nullable;

/**
 * Microsoft sign-in as the Minecraft launcher does it, written from the services' public behaviour
 * (ADR 0070): a device code from {@code login.live.com}, the player approving it on any device, an
 * Xbox Live token and an XSTS token for Minecraft, then {@code login_with_xbox} for the Minecraft
 * token and the profile it belongs to. Also a session-server join and the chat-signing key pair.
 *
 * <p>Every call returns at once; the futures complete on the transport's thread. Failures are
 * {@link SignInException}s with a message for the player. No token is ever logged.
 */
public final class MicrosoftSignIn {

    /** Renew a Minecraft token with less than this left, so a join never races its expiry. */
    public static final long RENEW_MARGIN_MILLIS = 10 * 60 * 1000L;

    private static final String JSON = "application/json";
    private static final String FORM = "application/x-www-form-urlencoded";

    private final Https https;
    private final LongSupplier clock;

    /**
     * @param https a client allowed {@link MicrosoftEndpoints#HOSTS} (an {@link incari.net.https.HttpsClient})
     * @param clock epoch milliseconds, for expiry times
     */
    public MicrosoftSignIn(Https https, LongSupplier clock) {
        this.https = https;
        this.clock = clock;
    }

    // --- Device code ---------------------------------------------------------------------------

    /** Asks for a code for the player to type at Microsoft's page. */
    public CompletableFuture<DeviceCode> requestCode() {
        Map<String, String> form = new LinkedHashMap<>();
        form.put("client_id", MicrosoftEndpoints.CLIENT_ID);
        form.put("scope", MicrosoftEndpoints.SCOPE);
        form.put("response_type", "device_code");
        return post(MicrosoftEndpoints.DEVICE_CODE, FORM, form(form), Map.of()).thenApply(response -> {
            JsonObject json = json(response, "Microsoft");
            if (!response.ok()) {
                throw failure("Microsoft didn't give a sign-in code (" + errorCode(json, response) + ")");
            }
            String userCode = string(json, "user_code");
            String page = string(json, "verification_uri");
            if (!isCodePage(page) || !isPlainCode(userCode)) {
                throw failure("Microsoft's sign-in answer looked wrong");
            }
            long expiresIn = number(json, "expires_in");
            long interval = json.has("interval") ? number(json, "interval") : 5;
            return new DeviceCode(
                    userCode,
                    string(json, "device_code"),
                    page,
                    clock.getAsLong() + Math.max(0, expiresIn) * 1000,
                    (int) Math.max(1, Math.min(interval, 60)));
        });
    }

    /** What one poll found. */
    public enum PollState {
        /** The player hasn't typed the code yet: poll again after the interval. */
        WAITING,
        /** Microsoft asks for slower polling: add five seconds to the interval. */
        SLOW_DOWN,
        /** Signed in: {@link Poll#tokens} holds Microsoft's tokens. */
        DONE
    }

    /** A poll's answer; {@code tokens} is set when {@code state} is {@link PollState#DONE}. */
    public record Poll(PollState state, @Nullable MicrosoftTokens tokens) {}

    /** Microsoft's tokens: the access token for Xbox Live, and the refresh token to keep. */
    public record MicrosoftTokens(String accessToken, String refreshToken) {
        @Override
        public String toString() {
            return "MicrosoftTokens[…]";
        }
    }

    /** Asks whether the player has typed {@code code}'s code yet. */
    public CompletableFuture<Poll> poll(DeviceCode code) {
        if (clock.getAsLong() >= code.expiresAtMillis()) {
            return CompletableFuture.failedFuture(new SignInException("The sign-in code expired. Start again."));
        }
        Map<String, String> form = new LinkedHashMap<>();
        form.put("client_id", MicrosoftEndpoints.CLIENT_ID);
        form.put("grant_type", "urn:ietf:params:oauth:grant-type:device_code");
        form.put("device_code", code.deviceCode());
        return post(MicrosoftEndpoints.TOKEN, FORM, form(form), Map.of()).thenApply(response -> {
            JsonObject json = json(response, "Microsoft");
            if (response.ok()) {
                return new Poll(PollState.DONE, tokens(json));
            }
            String error = errorCode(json, response);
            return switch (error) {
                case "authorization_pending" -> new Poll(PollState.WAITING, null);
                case "slow_down" -> new Poll(PollState.SLOW_DOWN, null);
                case "expired_token" -> throw failure("The sign-in code expired. Start again.");
                case "authorization_declined", "access_denied" -> throw failure("Sign-in was cancelled.");
                default -> throw failure("Microsoft refused the sign-in (" + error + ")");
            };
        });
    }

    /** Swaps a refresh token for new tokens. */
    public CompletableFuture<MicrosoftTokens> refresh(String refreshToken) {
        Map<String, String> form = new LinkedHashMap<>();
        form.put("client_id", MicrosoftEndpoints.CLIENT_ID);
        form.put("scope", MicrosoftEndpoints.SCOPE);
        form.put("grant_type", "refresh_token");
        form.put("refresh_token", refreshToken);
        return post(MicrosoftEndpoints.TOKEN, FORM, form(form), Map.of()).thenApply(response -> {
            JsonObject json = json(response, "Microsoft");
            if (!response.ok()) {
                throw failure("The Microsoft sign-in has expired. Sign in again.");
            }
            return tokens(json);
        });
    }

    private static MicrosoftTokens tokens(JsonObject json) {
        return new MicrosoftTokens(string(json, "access_token"), string(json, "refresh_token"));
    }

    // --- Xbox Live and Minecraft ---------------------------------------------------------------

    /** Signs in to Minecraft with Microsoft's tokens: Xbox Live, XSTS, the Minecraft token, the profile. */
    public CompletableFuture<MicrosoftAccount> signIn(MicrosoftTokens tokens) {
        return xboxUser(tokens.accessToken(), "t=")
                .thenCompose(this::xsts)
                .thenCompose(this::minecraftToken)
                .thenCompose(login -> profile(login, tokens.refreshToken()));
    }

    /**
     * {@code account} with a Minecraft token good for at least {@link #RENEW_MARGIN_MILLIS}: itself
     * if it has one, else renewed through its refresh token.
     */
    public CompletableFuture<MicrosoftAccount> renew(MicrosoftAccount account) {
        if (account.fresh(clock.getAsLong(), RENEW_MARGIN_MILLIS)) {
            return CompletableFuture.completedFuture(account);
        }
        return refresh(account.refreshToken()).thenCompose(this::signIn);
    }

    private CompletableFuture<String> xboxUser(String microsoftToken, String ticketPrefix) {
        JsonObject properties = new JsonObject();
        properties.addProperty("AuthMethod", "RPS");
        properties.addProperty("SiteName", "user.auth.xboxlive.com");
        properties.addProperty("RpsTicket", ticketPrefix + microsoftToken);
        JsonObject body = new JsonObject();
        body.add("Properties", properties);
        body.addProperty("RelyingParty", "http://auth.xboxlive.com");
        body.addProperty("TokenType", "JWT");
        return post(MicrosoftEndpoints.XBOX_USER, JSON, body.toString(), Map.of("x-xbl-contract-version", "1"))
                .thenCompose(response -> {
                    // Tickets from login.live.com go as "t="; an Azure app's tokens as "d=". Xbox
                    // answers 400 to the wrong one, so try the other once.
                    if (response.status() == 400 && ticketPrefix.equals("t=")) {
                        return xboxUser(microsoftToken, "d=");
                    }
                    JsonObject json = json(response, "Xbox Live");
                    if (!response.ok()) {
                        throw failure("Xbox Live refused the sign-in (HTTP " + response.status() + ")");
                    }
                    return CompletableFuture.completedFuture(string(json, "Token"));
                });
    }

    private record XstsToken(String token, String userHash) {}

    private CompletableFuture<XstsToken> xsts(String xboxToken) {
        JsonArray userTokens = new JsonArray();
        userTokens.add(xboxToken);
        JsonObject properties = new JsonObject();
        properties.addProperty("SandboxId", "RETAIL");
        properties.add("UserTokens", userTokens);
        JsonObject body = new JsonObject();
        body.add("Properties", properties);
        body.addProperty("RelyingParty", "rp://api.minecraftservices.com/");
        body.addProperty("TokenType", "JWT");
        return post(MicrosoftEndpoints.XSTS, JSON, body.toString(), Map.of("x-xbl-contract-version", "1"))
                .thenApply(response -> {
                    JsonObject json = json(response, "Xbox Live");
                    if (response.status() == 401 && json.has("XErr")) {
                        throw failure(xboxError(json.get("XErr").getAsLong()));
                    }
                    if (!response.ok()) {
                        throw failure("Xbox Live refused the sign-in (HTTP " + response.status() + ")");
                    }
                    String userHash;
                    try {
                        userHash = json.getAsJsonObject("DisplayClaims")
                                .getAsJsonArray("xui")
                                .get(0)
                                .getAsJsonObject()
                                .get("uhs")
                                .getAsString();
                    } catch (RuntimeException e) {
                        throw failure("Xbox Live's answer looked wrong");
                    }
                    return new XstsToken(string(json, "Token"), userHash);
                });
    }

    /** Why XSTS refused, by Xbox's documented error codes. */
    static String xboxError(long code) {
        if (code == 2148916227L) {
            return "This Xbox account is banned.";
        }
        if (code == 2148916233L) {
            return "This Microsoft account has no Xbox profile. Sign in once at minecraft.net, then try again.";
        }
        if (code == 2148916235L) {
            return "Xbox Live isn't available in this account's country.";
        }
        if (code == 2148916236L || code == 2148916237L) {
            return "This account needs adult verification on xbox.com first.";
        }
        if (code == 2148916238L) {
            return "This is a child account: an adult has to add it to a Microsoft family first.";
        }
        return "Xbox Live refused the sign-in (error " + code + ").";
    }

    private record MinecraftLogin(String token, long expiresAtMillis) {}

    private CompletableFuture<MinecraftLogin> minecraftToken(XstsToken xsts) {
        JsonObject body = new JsonObject();
        body.addProperty("identityToken", "XBL3.0 x=" + xsts.userHash() + ";" + xsts.token());
        return post(MicrosoftEndpoints.LOGIN_WITH_XBOX, JSON, body.toString(), Map.of())
                .thenApply(response -> {
                    JsonObject json = json(response, "Minecraft");
                    if (!response.ok()) {
                        throw failure("Minecraft refused the sign-in (HTTP " + response.status() + ")");
                    }
                    long expiresIn = json.has("expires_in") ? number(json, "expires_in") : 3600;
                    return new MinecraftLogin(
                            string(json, "access_token"), clock.getAsLong() + Math.max(0, expiresIn) * 1000);
                });
    }

    private CompletableFuture<MicrosoftAccount> profile(MinecraftLogin login, String refreshToken) {
        return get(MicrosoftEndpoints.PROFILE, login.token()).thenApply(response -> {
            if (response.status() == 404) {
                throw failure("This Microsoft account doesn't own Minecraft: Java Edition.");
            }
            JsonObject json = json(response, "Minecraft");
            if (!response.ok()) {
                throw failure("Minecraft didn't give the profile (HTTP " + response.status() + ")");
            }
            String name = string(json, "name");
            String id = string(json, "id");
            if (!MicrosoftAccount.isPlayerName(name) || !MicrosoftAccount.isProfileId(id)) {
                throw failure("Minecraft's profile looked wrong");
            }
            return new MicrosoftAccount(name, id, login.token(), login.expiresAtMillis(), refreshToken);
        });
    }

    // --- Using the account ---------------------------------------------------------------------

    /**
     * Tells the session server {@code account} is joining the server whose hash is
     * {@code serverHash} (the game's {@code digestData}, as signed hex). The server then asks the
     * session server whether this account joined.
     */
    public CompletableFuture<JoinResult> join(MicrosoftAccount account, String serverHash) {
        JsonObject body = new JsonObject();
        body.addProperty("accessToken", account.minecraftToken());
        body.addProperty("selectedProfile", account.id());
        body.addProperty("serverId", serverHash);
        return post(MicrosoftEndpoints.JOIN, JSON, body.toString(), Map.of())
                .thenApply(response -> {
                    if (response.ok()) {
                        return JoinResult.OK;
                    }
                    String error = "";
                    try {
                        JsonElement parsed = JsonParser.parseString(response.text());
                        if (parsed.isJsonObject() && parsed.getAsJsonObject().has("error")) {
                            error = parsed.getAsJsonObject().get("error").getAsString();
                        }
                    } catch (JsonParseException | IllegalStateException | UnsupportedOperationException ignored) {
                        // No reason given.
                    }
                    if (error.endsWith("InsufficientPrivilegesException")) {
                        return JoinResult.INSUFFICIENT_PRIVILEGES;
                    }
                    if (error.endsWith("UserBannedException")) {
                        return JoinResult.BANNED;
                    }
                    if (response.status() == 401 || response.status() == 403) {
                        return JoinResult.INVALID_SESSION;
                    }
                    return JoinResult.UNAVAILABLE;
                })
                .exceptionally(failure -> JoinResult.UNAVAILABLE);
    }

    /** Fetches the account's chat-signing key pair. */
    public CompletableFuture<ProfileCertificates> certificates(MicrosoftAccount account) {
        Map<String, String> headers = Map.of("Authorization", "Bearer " + account.minecraftToken());
        return post(MicrosoftEndpoints.CERTIFICATES, JSON, "", headers).thenApply(response -> {
            JsonObject json = json(response, "Minecraft");
            if (!response.ok()) {
                throw failure("Minecraft didn't give a chat key (HTTP " + response.status() + ")");
            }
            try {
                JsonObject keyPair = json.getAsJsonObject("keyPair");
                byte[] signature = Base64.getDecoder().decode(string(json, "publicKeySignatureV2"));
                return new ProfileCertificates(
                        string(keyPair, "privateKey"),
                        string(keyPair, "publicKey"),
                        signature,
                        string(json, "expiresAt"),
                        string(json, "refreshedAfter"));
            } catch (IllegalArgumentException | ClassCastException | NullPointerException e) {
                throw failure("Minecraft's chat key looked wrong");
            }
        });
    }

    // --- HTTP and JSON -------------------------------------------------------------------------

    private CompletableFuture<HttpResponse> post(String url, String type, String body, Map<String, String> headers) {
        Map<String, String> all = new LinkedHashMap<>(headers);
        all.put("Accept", JSON);
        all.put("User-Agent", "Incari");
        return https.send(HttpRequest.post(url, type, body, all)).exceptionally(MicrosoftSignIn::unreachable);
    }

    private CompletableFuture<HttpResponse> get(String url, String bearer) {
        Map<String, String> headers = new LinkedHashMap<>();
        headers.put("Accept", JSON);
        headers.put("User-Agent", "Incari");
        headers.put("Authorization", "Bearer " + bearer);
        return https.send(HttpRequest.get(url, headers)).exceptionally(MicrosoftSignIn::unreachable);
    }

    private static HttpResponse unreachable(Throwable failure) {
        Throwable cause =
                failure instanceof CompletionException && failure.getCause() != null ? failure.getCause() : failure;
        String message = cause.getMessage();
        throw failure("Couldn't reach Microsoft's sign-in" + (message != null ? ": " + message : "."), cause);
    }

    /** The body as a JSON object; an empty object for an empty or non-JSON error body. */
    private static JsonObject json(HttpResponse response, String service) {
        String text = response.text();
        if (text.isBlank()) {
            if (response.ok()) {
                throw failure(service + " sent an empty answer");
            }
            return new JsonObject();
        }
        try {
            JsonElement parsed = JsonParser.parseString(text);
            if (parsed.isJsonObject()) {
                return parsed.getAsJsonObject();
            }
        } catch (JsonParseException e) {
            // Falls through.
        }
        if (response.ok()) {
            throw failure(service + " sent an answer that isn't JSON");
        }
        return new JsonObject();
    }

    private static String errorCode(JsonObject json, HttpResponse response) {
        JsonElement error = json.get("error");
        if (error != null && error.isJsonPrimitive()) {
            String code = error.getAsString();
            // Shown to the player: keep it to a plain code.
            if (code.length() <= 40 && code.chars().allMatch(c -> c == '_' || Character.isLetterOrDigit(c))) {
                return code;
            }
        }
        return "HTTP " + response.status();
    }

    private static String string(JsonObject json, String name) {
        JsonElement value = json.get(name);
        if (value == null || !value.isJsonPrimitive() || value.getAsString().isEmpty()) {
            throw failure("A sign-in answer was missing " + name);
        }
        return value.getAsString();
    }

    private static long number(JsonObject json, String name) {
        JsonElement value = json.get(name);
        try {
            if (value != null && value.isJsonPrimitive()) {
                return value.getAsLong();
            }
        } catch (NumberFormatException e) {
            // Falls through.
        }
        throw failure("A sign-in answer was missing " + name);
    }

    static boolean isCodePage(String url) {
        String prefix = "https://";
        if (!url.startsWith(prefix)) {
            return false;
        }
        String rest = url.substring(prefix.length());
        int slash = rest.indexOf('/');
        String host = slash < 0 ? rest : rest.substring(0, slash);
        return MicrosoftEndpoints.CODE_PAGE_HOSTS.contains(host) && url.chars().allMatch(c -> c > 0x20 && c < 0x7F);
    }

    private static boolean isPlainCode(String code) {
        return !code.isEmpty()
                && code.length() <= 16
                && code.chars().allMatch(c -> (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9'));
    }

    /** {@code application/x-www-form-urlencoded}, by hand (the browser build has no URLEncoder). */
    static String form(Map<String, String> fields) {
        StringBuilder out = new StringBuilder();
        for (Map.Entry<String, String> field : fields.entrySet()) {
            if (out.length() > 0) {
                out.append('&');
            }
            encode(field.getKey(), out);
            out.append('=');
            encode(field.getValue(), out);
        }
        return out.toString();
    }

    private static void encode(String text, StringBuilder out) {
        for (byte b : text.getBytes(StandardCharsets.UTF_8)) {
            int c = b & 0xFF;
            if ((c >= 'a' && c <= 'z')
                    || (c >= 'A' && c <= 'Z')
                    || (c >= '0' && c <= '9')
                    || c == '-'
                    || c == '.'
                    || c == '_'
                    || c == '*') {
                out.append((char) c);
            } else if (c == ' ') {
                out.append('+');
            } else {
                out.append('%')
                        .append(Character.toUpperCase(Character.forDigit(c >> 4, 16)))
                        .append(Character.toUpperCase(Character.forDigit(c & 0xF, 16)));
            }
        }
    }

    private static CompletionException failure(String message) {
        return new CompletionException(new SignInException(message));
    }

    private static CompletionException failure(String message, Throwable cause) {
        return new CompletionException(new SignInException(message, cause));
    }
}
