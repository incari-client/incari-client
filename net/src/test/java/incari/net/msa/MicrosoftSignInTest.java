package incari.net.msa;

import static org.assertj.core.api.Assertions.assertThat;

import incari.net.https.HttpRequest;
import incari.net.https.HttpResponse;
import incari.net.https.Https;
import java.nio.charset.StandardCharsets;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;

class MicrosoftSignInTest {

    private static final long NOW = 1_800_000_000_000L;

    private final FakeServer server = new FakeServer();
    private final MicrosoftSignIn signIn = new MicrosoftSignIn(server, () -> NOW);

    // --- Device code ---------------------------------------------------------------------------

    @Test
    void asksForACodeAsTheLauncher() throws Exception {
        server.answer(
                "login.live.com/oauth20_connect.srf",
                200,
                "{\"user_code\":\"ABCD1234\",\"device_code\":\"secret\",\"verification_uri\":\"https://www.microsoft.com/link\","
                        + "\"interval\":5,\"expires_in\":900}");

        DeviceCode code = signIn.requestCode().get();

        assertThat(code.userCode()).isEqualTo("ABCD1234");
        assertThat(code.pageWithCode()).isEqualTo("https://www.microsoft.com/link?otc=ABCD1234");
        assertThat(code.expiresAtMillis()).isEqualTo(NOW + 900_000);
        assertThat(code.toString()).doesNotContain("secret");
        String form = server.bodies.getFirst();
        assertThat(form)
                .isEqualTo("client_id=00000000402b5328&scope=service%3A%3Auser.auth.xboxlive.com%3A%3AMBI_SSL"
                        + "&response_type=device_code");
    }

    @Test
    void refusesACodePageOffMicrosoft() {
        server.answer(
                "login.live.com/oauth20_connect.srf",
                200,
                "{\"user_code\":\"ABCD1234\",\"device_code\":\"secret\",\"verification_uri\":\"https://evil.example/link\","
                        + "\"interval\":5,\"expires_in\":900}");
        assertFails(signIn.requestCode(), "looked wrong");
    }

    @Test
    void pollsUntilThePlayerHasTypedTheCode() throws Exception {
        DeviceCode code = new DeviceCode("ABCD1234", "device", "https://www.microsoft.com/link", NOW + 1000, 5);
        server.answer("login.live.com/oauth20_token.srf", 400, "{\"error\":\"authorization_pending\"}");
        server.answer("login.live.com/oauth20_token.srf", 400, "{\"error\":\"slow_down\"}");
        server.answer(
                "login.live.com/oauth20_token.srf", 200, "{\"access_token\":\"ms\",\"refresh_token\":\"refresh\"}");

        assertThat(signIn.poll(code).get().state()).isEqualTo(MicrosoftSignIn.PollState.WAITING);
        assertThat(signIn.poll(code).get().state()).isEqualTo(MicrosoftSignIn.PollState.SLOW_DOWN);
        MicrosoftSignIn.Poll done = signIn.poll(code).get();
        assertThat(done.state()).isEqualTo(MicrosoftSignIn.PollState.DONE);
        assertThat(done.tokens()).isEqualTo(new MicrosoftSignIn.MicrosoftTokens("ms", "refresh"));
        assertThat(server.bodies.getFirst()).contains("device_code=device", "grant_type=urn%3Aietf%3Aparams");
    }

    @Test
    void sayWhenTheCodeExpiredOrWasDeclined() {
        DeviceCode code = new DeviceCode("ABCD1234", "device", "https://www.microsoft.com/link", NOW + 1000, 5);
        server.answer("login.live.com/oauth20_token.srf", 400, "{\"error\":\"expired_token\"}");
        assertFails(signIn.poll(code), "expired");
        server.answer("login.live.com/oauth20_token.srf", 400, "{\"error\":\"authorization_declined\"}");
        assertFails(signIn.poll(code), "cancelled");

        DeviceCode old = new DeviceCode("ABCD1234", "device", "https://www.microsoft.com/link", NOW - 1, 5);
        assertFails(signIn.poll(old), "expired");
    }

    // --- The token chain -----------------------------------------------------------------------

    @Test
    void signsInThroughXboxToMinecraft() throws Exception {
        answerTheChain();

        MicrosoftAccount account = signIn.signIn(new MicrosoftSignIn.MicrosoftTokens("ms", "refresh"))
                .get();

        assertThat(account.name()).isEqualTo("Steve");
        assertThat(account.id()).isEqualTo("0123456789abcdef0123456789abcdef");
        assertThat(account.minecraftToken()).isEqualTo("mc");
        assertThat(account.minecraftExpiresAtMillis()).isEqualTo(NOW + 86_400_000L);
        assertThat(account.refreshToken()).isEqualTo("refresh");
        assertThat(server.bodies.get(0))
                .contains("\"RpsTicket\":\"t=ms\"", "\"RelyingParty\":\"http://auth.xboxlive.com\"");
        assertThat(server.bodies.get(1)).contains("\"UserTokens\":[\"xbl\"]", "rp://api.minecraftservices.com/");
        assertThat(server.bodies.get(2)).isEqualTo("{\"identityToken\":\"XBL3.0 x=hash;xsts\"}");
        assertThat(server.headers.get(3)).containsEntry("Authorization", "Bearer mc");
    }

    @Test
    void triesTheOtherTicketFormOnce() throws Exception {
        server.answer("user.auth.xboxlive.com/user/authenticate", 400, "");
        answerTheChain();

        signIn.signIn(new MicrosoftSignIn.MicrosoftTokens("ms", "refresh")).get();

        assertThat(server.bodies.get(0)).contains("t=ms");
        assertThat(server.bodies.get(1)).contains("d=ms");
    }

    @Test
    void explainsXboxRefusals() {
        server.answer("user.auth.xboxlive.com/user/authenticate", 200, "{\"Token\":\"xbl\"}");
        server.answer("xsts.auth.xboxlive.com/xsts/authorize", 401, "{\"XErr\":2148916233}");
        assertFails(signIn.signIn(new MicrosoftSignIn.MicrosoftTokens("ms", "r")), "no Xbox profile");
        assertThat(MicrosoftSignIn.xboxError(2148916238L)).contains("child account");
    }

    @Test
    void saysWhenTheAccountDoesNotOwnTheGame() {
        server.answer("user.auth.xboxlive.com/user/authenticate", 200, "{\"Token\":\"xbl\"}");
        server.answer(
                "xsts.auth.xboxlive.com/xsts/authorize",
                200,
                "{\"Token\":\"xsts\",\"DisplayClaims\":{\"xui\":[{\"uhs\":\"hash\"}]}}");
        server.answer(
                "api.minecraftservices.com/authentication/login_with_xbox",
                200,
                "{\"access_token\":\"mc\",\"expires_in\":86400}");
        server.answer("api.minecraftservices.com/minecraft/profile", 404, "{\"error\":\"NOT_FOUND\"}");
        assertFails(signIn.signIn(new MicrosoftSignIn.MicrosoftTokens("ms", "r")), "doesn't own Minecraft");
    }

    @Test
    void refusesAProfileThatLooksWrong() {
        server.answer("user.auth.xboxlive.com/user/authenticate", 200, "{\"Token\":\"xbl\"}");
        server.answer(
                "xsts.auth.xboxlive.com/xsts/authorize",
                200,
                "{\"Token\":\"xsts\",\"DisplayClaims\":{\"xui\":[{\"uhs\":\"hash\"}]}}");
        server.answer("api.minecraftservices.com/authentication/login_with_xbox", 200, "{\"access_token\":\"mc\"}");
        server.answer(
                "api.minecraftservices.com/minecraft/profile",
                200,
                "{\"id\":\"../../etc\",\"name\":\"\\u00a7cSteve\"}");
        assertFails(signIn.signIn(new MicrosoftSignIn.MicrosoftTokens("ms", "r")), "looked wrong");
    }

    @Test
    void renewsOnlyATokenAboutToExpire() throws Exception {
        MicrosoftAccount fresh = account(NOW + MicrosoftSignIn.RENEW_MARGIN_MILLIS + 60_000);
        assertThat(signIn.renew(fresh).get()).isSameAs(fresh);
        assertThat(server.urls).isEmpty();

        server.answer(
                "login.live.com/oauth20_token.srf", 200, "{\"access_token\":\"ms2\",\"refresh_token\":\"refresh2\"}");
        answerTheChain();
        MicrosoftAccount renewed = signIn.renew(account(NOW + 1000)).get();
        assertThat(renewed.refreshToken()).isEqualTo("refresh2");
        assertThat(server.bodies.getFirst()).contains("grant_type=refresh_token", "refresh_token=old-refresh");
    }

    @Test
    void anExpiredRefreshTokenAsksToSignInAgain() {
        server.answer("login.live.com/oauth20_token.srf", 400, "{\"error\":\"invalid_grant\"}");
        assertFails(signIn.renew(account(NOW)), "Sign in again");
        assertThat(((SignInException) failureAfter(400, "invalid_grant")).temporary())
                .isFalse();
    }

    @Test
    void aServiceOutageOrNetworkFailureIsWorthTryingAgain() {
        server.answer("login.live.com/oauth20_token.srf", 503, "");
        Throwable outage = failure(signIn.renew(account(NOW)));
        assertThat(outage.getMessage()).contains("Try again").doesNotContain("Sign in again");
        assertThat(((SignInException) outage).temporary()).isTrue();

        server.fail("login.live.com/oauth20_token.srf");
        Throwable unreachable = failure(signIn.renew(account(NOW)));
        assertThat(((SignInException) unreachable).temporary()).isTrue();
    }

    @Test
    void aPollThatDoesntGetThroughIsTemporaryAndARefusalIsNot() {
        DeviceCode code = new DeviceCode("ABCD1234", "device", "https://www.microsoft.com/link", NOW + 1000, 5);
        server.fail("login.live.com/oauth20_token.srf");
        assertThat(((SignInException) failure(signIn.poll(code))).temporary()).isTrue();
        server.answer("login.live.com/oauth20_token.srf", 502, "<html>Bad gateway</html>");
        assertThat(((SignInException) failure(signIn.poll(code))).temporary()).isTrue();
        server.answer("login.live.com/oauth20_token.srf", 400, "{\"error\":\"expired_token\"}");
        assertThat(((SignInException) failure(signIn.poll(code))).temporary()).isFalse();
        server.answer("login.live.com/oauth20_token.srf", 400, "{\"error\":\"access_denied\"}");
        assertThat(((SignInException) failure(signIn.poll(code))).temporary()).isFalse();
    }

    private Throwable failureAfter(int status, String error) {
        server.answer("login.live.com/oauth20_token.srf", status, "{\"error\":\"" + error + "\"}");
        return failure(signIn.renew(account(NOW)));
    }

    // --- Joins and keys ------------------------------------------------------------------------

    @Test
    void joinsAndReadsTheSessionServersAnswers() throws Exception {
        server.answer("sessionserver.mojang.com/session/minecraft/join", 204, "");
        assertThat(signIn.join(account(NOW + 1000), "-6b2c").get()).isEqualTo(JoinResult.OK);
        assertThat(server.bodies.getFirst())
                .isEqualTo(
                        "{\"accessToken\":\"mc\",\"selectedProfile\":\"0123456789abcdef0123456789abcdef\",\"serverId\":\"-6b2c\"}");

        server.answer(
                "sessionserver.mojang.com/session/minecraft/join", 403, "{\"error\":\"ForbiddenOperationException\"}");
        assertThat(signIn.join(account(NOW), "x").get()).isEqualTo(JoinResult.INVALID_SESSION);
        server.answer(
                "sessionserver.mojang.com/session/minecraft/join",
                403,
                "{\"error\":\"InsufficientPrivilegesException\"}");
        assertThat(signIn.join(account(NOW), "x").get()).isEqualTo(JoinResult.INSUFFICIENT_PRIVILEGES);
        server.answer("sessionserver.mojang.com/session/minecraft/join", 403, "{\"error\":\"UserBannedException\"}");
        assertThat(signIn.join(account(NOW), "x").get()).isEqualTo(JoinResult.BANNED);
        server.answer("sessionserver.mojang.com/session/minecraft/join", 503, "");
        assertThat(signIn.join(account(NOW), "x").get()).isEqualTo(JoinResult.UNAVAILABLE);
        server.fail("sessionserver.mojang.com/session/minecraft/join");
        assertThat(signIn.join(account(NOW), "x").get()).isEqualTo(JoinResult.UNAVAILABLE);
    }

    @Test
    void fetchesTheChatKey() throws Exception {
        server.answer(
                "api.minecraftservices.com/player/certificates",
                200,
                "{\"keyPair\":{\"privateKey\":\"-----BEGIN RSA PRIVATE KEY-----\\nAAA\\n-----END RSA PRIVATE KEY-----\\n\","
                        + "\"publicKey\":\"-----BEGIN RSA PUBLIC KEY-----\\nBBB\\n-----END RSA PUBLIC KEY-----\\n\"},"
                        + "\"publicKeySignature\":\"AAEC\",\"publicKeySignatureV2\":\"AQID\","
                        + "\"expiresAt\":\"2027-01-02T03:04:05.123Z\",\"refreshedAfter\":\"2027-01-01T00:00:00Z\"}");

        ProfileCertificates keys = signIn.certificates(account(NOW + 1000)).get();

        assertThat(keys.publicKeySignature()).containsExactly(1, 2, 3);
        assertThat(keys.privateKeyPem()).startsWith("-----BEGIN RSA PRIVATE KEY-----");
        assertThat(keys.expiresAt()).isEqualTo("2027-01-02T03:04:05.123Z");
        assertThat(server.headers.getFirst()).containsEntry("Authorization", "Bearer mc");
        assertThat(keys.toString()).doesNotContain("PRIVATE");
    }

    // --- Accounts ------------------------------------------------------------------------------

    @Test
    void anAccountRoundTripsAndNeverPrintsItsTokens() {
        MicrosoftAccount account = account(NOW);
        assertThat(MicrosoftAccount.fromJson(account.toJson())).isEqualTo(account);
        assertThat(account.toString()).doesNotContain("mc", "old-refresh").contains("Steve");
        assertThat(account.dashedId()).isEqualTo("01234567-89ab-cdef-0123-456789abcdef");
        assertThat(MicrosoftAccount.fromJson("{\"version\":2}")).isNull();
        assertThat(MicrosoftAccount.fromJson("not json")).isNull();
        assertThat(MicrosoftAccount.fromJson("{\"version\":1,\"name\":\"a b\",\"id\":\"x\"}"))
                .isNull();
    }

    @Test
    void failuresNeverCarryTokens() {
        server.fail("login.live.com/oauth20_token.srf");
        Throwable thrown = failure(signIn.renew(account(NOW)));
        for (Throwable t = thrown; t != null; t = t.getCause()) {
            assertThat(String.valueOf(t.getMessage())).doesNotContain("old-refresh");
        }
    }

    // --- Helpers -------------------------------------------------------------------------------

    private void answerTheChain() {
        server.answer("user.auth.xboxlive.com/user/authenticate", 200, "{\"Token\":\"xbl\"}");
        server.answer(
                "xsts.auth.xboxlive.com/xsts/authorize",
                200,
                "{\"Token\":\"xsts\",\"DisplayClaims\":{\"xui\":[{\"uhs\":\"hash\"}]}}");
        server.answer(
                "api.minecraftservices.com/authentication/login_with_xbox",
                200,
                "{\"access_token\":\"mc\",\"expires_in\":86400}");
        server.answer(
                "api.minecraftservices.com/minecraft/profile",
                200,
                "{\"id\":\"0123456789abcdef0123456789abcdef\",\"name\":\"Steve\"}");
    }

    private static MicrosoftAccount account(long expiresAt) {
        return new MicrosoftAccount("Steve", "0123456789abcdef0123456789abcdef", "mc", expiresAt, "old-refresh");
    }

    private static void assertFails(CompletableFuture<?> future, String message) {
        Throwable thrown = failure(future);
        assertThat(thrown).isInstanceOf(SignInException.class);
        assertThat(thrown.getMessage()).contains(message);
    }

    private static Throwable failure(CompletableFuture<?> future) {
        try {
            future.get(5, TimeUnit.SECONDS);
        } catch (ExecutionException e) {
            Throwable cause = e.getCause();
            while (cause instanceof java.util.concurrent.CompletionException && cause.getCause() != null) {
                cause = cause.getCause();
            }
            return cause;
        } catch (Exception e) {
            throw new AssertionError(e);
        }
        throw new AssertionError("Didn't fail");
    }

    /** Answers by host and path, in the order answers were given for each. */
    private static final class FakeServer implements Https {
        private final Map<String, Deque<HttpResponse>> answers = new LinkedHashMap<>();
        final List<String> urls = new ArrayList<>();
        final List<String> bodies = new ArrayList<>();
        final List<Map<String, String>> headers = new ArrayList<>();

        void answer(String where, int status, String body) {
            answers.computeIfAbsent(where, k -> new ArrayDeque<>())
                    .add(new HttpResponse(status, Map.of(), body.getBytes(StandardCharsets.UTF_8)));
        }

        void fail(String where) {
            answers.computeIfAbsent(where, k -> new ArrayDeque<>()).add(new HttpResponse(-1, Map.of(), new byte[0]));
        }

        @Override
        public CompletableFuture<HttpResponse> send(HttpRequest request) {
            String where = request.host() + request.path();
            urls.add(where);
            bodies.add(new String(request.body(), StandardCharsets.UTF_8));
            headers.add(request.headers());
            Deque<HttpResponse> queue = answers.get(where);
            HttpResponse response = queue == null ? null : queue.poll();
            if (response == null) {
                return CompletableFuture.failedFuture(new AssertionError("Unexpected request to " + where));
            }
            if (response.status() < 0) {
                return CompletableFuture.failedFuture(new java.io.IOException("connection refused"));
            }
            return CompletableFuture.completedFuture(response);
        }
    }
}
