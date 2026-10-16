package incari.online;

import static org.assertj.core.api.Assertions.assertThat;

import com.google.gson.JsonObject;
import incari.framework.online.IncariPlayer;
import incari.framework.online.Online.Status;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import org.junit.jupiter.api.Test;

class OnlineServiceTest {

    private static final String PASSWORD = "correct horse battery";
    private static final UUID ALEX = new UUID(1, 1);
    private static final UUID STEVE = new UUID(2, 2);

    private final FakeService fake = new FakeService();

    private Client started() {
        Client client = new Client(fake);
        client.service.start();
        return client.settle();
    }

    private Client signedUp(String username) {
        Client client = started();
        Result<List<String>> result = client.await(done -> client.service.signUp(username, PASSWORD, done));
        assertThat(result.ok()).isTrue();
        return client;
    }

    @Test
    void anEmptyUrlIsOffAndNeverConnects() {
        OnlineService off = new OnlineService(
                "", "test", fake, new Client.MemoryStorage(), Runnable::run, () -> {}, () -> 0, () -> 0, (m, e) -> {});
        off.start();
        off.tick();
        assertThat(off.status()).isEqualTo(Status.OFF);
        assertThat(fake.sockets).isEmpty();
    }

    @Test
    void nothingConnectsBeforeStart() {
        Client client = new Client(fake).settle();
        assertThat(fake.sockets).isEmpty();
        assertThat(client.service.status()).isEqualTo(Status.CONNECTING);
    }

    @Test
    void helloLeavesTheClientSignedOut() {
        Client client = started();
        assertThat(client.service.status()).isEqualTo(Status.SIGNED_OUT);
        assertThat(fake.received.get(0).get("t").getAsString()).isEqualTo("hello");
        assertThat(fake.received.get(0).get("v").getAsInt()).isEqualTo(OnlineService.PROTOCOL);
    }

    @Test
    void helloNamesTheLatestRelease() {
        assertThat(started().service.latestRelease()).isNull();
        fake.announceRelease("0.3.1");
        Client client = new Client(fake);
        long before = client.service.revision();
        client.service.start();
        client.settle();
        assertThat(client.service.latestRelease()).isEqualTo("0.3.1");
        assertThat(client.service.revision()).isNotEqualTo(before);
    }

    @Test
    void helloCarriesTheReleaseNotes() {
        assertThat(started().service.releaseNotes()).isEmpty();
        fake.announceRelease("0.3.1", "Faster chunk loading", "  A changelog  ");
        assertThat(started().service.releaseNotes()).containsExactly("Faster chunk loading", "A changelog");
    }

    @Test
    void releaseNotesThatAreNotPlainTextAreDropped() {
        String[] notes = new String[Releases.MAX_NOTES + 3];
        notes[0] = "hidden\u200Bspace";
        notes[1] = "x".repeat(Releases.MAX_NOTE + 1);
        for (int i = 2; i < notes.length; i++) {
            notes[i] = "Change " + i;
        }
        fake.announceRelease("0.3.1", notes);
        List<String> kept = started().service.releaseNotes();
        assertThat(kept).hasSize(Releases.MAX_NOTES).startsWith("Change 2").doesNotContain(notes[0]);
    }

    @Test
    void aReleaseThatIsNotAVersionIsIgnored() {
        fake.announceRelease("0.3.1 <script>");
        assertThat(started().service.latestRelease()).isNull();
    }

    @Test
    void signUpSolvesTheChallengeSignsInAndKeepsOnlyTheToken() {
        Client client = started();
        Result<List<String>> result = client.await(done -> client.service.signUp("Frost_1", PASSWORD, done));

        assertThat(result.ok()).isTrue();
        assertThat(result.value()).hasSize(10).allMatch(code -> RecoveryCodes.normalize(code) != null);
        assertThat(client.service.status()).isEqualTo(Status.SIGNED_IN);
        assertThat(client.service.accountName()).isEqualTo("Frost_1");
        String saved = client.storage.files.get(SessionStore.FILE);
        assertThat(saved).contains("token-").doesNotContain(PASSWORD);
    }

    @Test
    void aTakenUsernameSaysSo() {
        signedUp("Frost");
        Client other = started();
        Result<List<String>> result = other.await(done -> other.service.signUp("frost", PASSWORD, done));
        assertThat(result.failure()).isEqualTo(Failure.TAKEN);
        assertThat(other.service.status()).isEqualTo(Status.SIGNED_OUT);
    }

    @Test
    void aSavedSessionResumesOnTheNextStart() {
        Client first = signedUp("Frost");
        Client second = new Client(fake, first.storage);
        assertThat(second.service.accountName()).isEqualTo("Frost");
        second.service.start();
        second.settle();
        assertThat(second.service.status()).isEqualTo(Status.SIGNED_IN);
        assertThat(fake.last().account).isNotNull();
    }

    @Test
    void anEndedSessionIsForgottenOnResume() {
        Client first = signedUp("Frost");
        fake.accounts.get("frost").tokens.clear();
        Client second = new Client(fake, first.storage);
        second.service.start();
        second.settle();
        assertThat(second.service.status()).isEqualTo(Status.SIGNED_OUT);
        assertThat(second.service.accountName()).isNull();
        assertThat(second.storage.files).doesNotContainKey(SessionStore.FILE);
    }

    @Test
    void wrongPasswordsAndRateLimitsReadTheSame() {
        signedUp("Frost");
        Client client = started();
        assertThat(client.<Void>await(done -> client.service.signIn("Frost", "wrong password!", done))
                        .failure())
                .isEqualTo(Failure.WRONG_CREDENTIALS);
        assertThat(client.<Void>await(done -> client.service.signIn("Nobody", PASSWORD, done))
                        .failure())
                .isEqualTo(Failure.WRONG_CREDENTIALS);
        fake.limitNext = "signin";
        assertThat(client.<Void>await(done -> client.service.signIn("Frost", PASSWORD, done))
                        .failure())
                .isEqualTo(Failure.WRONG_CREDENTIALS);
        assertThat(client.<Void>await(done -> client.service.signIn("Frost", PASSWORD, done))
                        .ok())
                .isTrue();
    }

    @Test
    void aRecoveryCodeSetsANewPasswordOnce() {
        Client owner = started();
        List<String> codes = owner.<List<String>>await(done -> owner.service.signUp("Frost", PASSWORD, done))
                .value();
        Client client = started();
        String typed = codes.get(3).toLowerCase(java.util.Locale.ROOT).replace("-", " ");
        assertThat(client.<Void>await(done -> client.service.recover("Frost", typed, "a new long password", done))
                        .ok())
                .isTrue();
        assertThat(client.service.status()).isEqualTo(Status.SIGNED_IN);
        // Every other session ended.
        owner.settle();
        assertThat(owner.service.status()).isEqualTo(Status.SIGNED_OUT);

        Client again = started();
        assertThat(again.<Void>await(done -> again.service.recover("Frost", codes.get(3), "another password!", done))
                        .failure())
                .isEqualTo(Failure.WRONG_CODE);
        assertThat(again.<Void>await(done -> again.service.recover("Frost", "not a code", "another password!", done))
                        .failure())
                .isEqualTo(Failure.WRONG_CODE);
    }

    @Test
    void accountActionsNeedThePassword() {
        Client client = signedUp("Frost");
        assertThat(client.<Void>await(done -> client.service.changePassword("wrong", "a new long password", done))
                        .failure())
                .isEqualTo(Failure.WRONG_PASSWORD);
        assertThat(client.<Void>await(done -> client.service.changePassword(PASSWORD, "a new long password", done))
                        .ok())
                .isTrue();
        assertThat(client.<List<String>>await(done -> client.service.newRecoveryCodes(PASSWORD, done))
                        .failure())
                .isEqualTo(Failure.WRONG_PASSWORD);
        assertThat(client.<List<String>>await(done -> client.service.newRecoveryCodes("a new long password", done))
                        .value())
                .hasSize(10);
        assertThat(client.<Void>await(done -> client.service.deleteAccount("a new long password", done))
                        .ok())
                .isTrue();
        assertThat(client.service.status()).isEqualTo(Status.SIGNED_OUT);
        assertThat(fake.accounts).isEmpty();
        assertThat(client.storage.files).doesNotContainKey(SessionStore.FILE);
    }

    @Test
    void signOutEverywhereEndsTheOtherSessions() {
        Client one = signedUp("Frost");
        Client two = started();
        assertThat(two.<Void>await(done -> two.service.signIn("Frost", PASSWORD, done))
                        .ok())
                .isTrue();
        assertThat(one.<Void>await(done -> one.service.signOut(true, done)).ok())
                .isTrue();
        two.settle();
        assertThat(one.service.status()).isEqualTo(Status.SIGNED_OUT);
        assertThat(two.service.status()).isEqualTo(Status.SIGNED_OUT);
        assertThat(two.storage.files).doesNotContainKey(SessionStore.FILE);
    }

    @Test
    void signingOutOfflineStillForgetsTheSession() {
        Client client = signedUp("Frost");
        fake.reachable = false;
        fake.last().drop();
        client.settle();
        assertThat(client.service.status()).isEqualTo(Status.CONNECTING);
        assertThat(client.<Void>await(done -> client.service.signOut(false, done))
                        .ok())
                .isTrue();
        assertThat(client.service.accountName()).isNull();
        assertThat(client.storage.files).doesNotContainKey(SessionStore.FILE);
    }

    @Test
    void signingOutWhileResumingStaysSignedOut() {
        Client first = signedUp("Frost");
        Client second = new Client(fake, first.storage);
        second.service.start();
        // The first tick opens and says hello; the second hears hello's answer and sends resume.
        second.service.tick();
        second.service.tick();
        second.service.signOut(false, result -> {});
        second.settle();
        assertThat(second.service.status()).isEqualTo(Status.SIGNED_OUT);
        assertThat(second.storage.files).doesNotContainKey(SessionStore.FILE);
    }

    @Test
    void anOutdatedClientStopsTrying() {
        fake.protocol = OnlineService.PROTOCOL + 1;
        Client client = started();
        assertThat(client.service.status()).isEqualTo(Status.UPDATE_NEEDED);
        client.now += 3_600_000;
        client.settle();
        assertThat(fake.sockets).hasSize(1);
        assertThat(client.<Void>await(done -> client.service.signIn("Frost", PASSWORD, done))
                        .failure())
                .isEqualTo(Failure.UPDATE);
    }

    @Test
    void anUnreachableServiceIsRetriedWithBackoffAndRequestsFailAtOnce() {
        fake.reachable = false;
        Client client = started();
        assertThat(client.service.status()).isEqualTo(Status.CONNECTING);
        assertThat(client.<Void>await(done -> client.service.signIn("Frost", PASSWORD, done))
                        .failure())
                .isEqualTo(Failure.OFFLINE);
        fake.reachable = true;
        // The first wait is half to all of a second (here, three quarters).
        client.now += 700;
        client.settle();
        assertThat(client.service.status()).isEqualTo(Status.CONNECTING);
        client.now += 100;
        client.settle();
        assertThat(client.service.status()).isEqualTo(Status.SIGNED_OUT);
    }

    @Test
    void retryConnectsAtOnce() {
        fake.reachable = false;
        Client client = started();
        fake.reachable = true;
        client.service.retry();
        client.settle();
        assertThat(client.service.status()).isEqualTo(Status.SIGNED_OUT);
    }

    @Test
    void aSilentServiceTimesOut() {
        fake.answerHello = false;
        Client client = started();
        client.now += OnlineService.OPEN_TIMEOUT_MILLIS;
        client.settle();
        assertThat(client.service.status()).isEqualTo(Status.CONNECTING);
        assertThat(fake.sockets.get(0).isOpen()).isFalse();
    }

    @Test
    void aDroppedConnectionResumesTheSession() {
        Client client = signedUp("Frost");
        fake.last().drop();
        client.settle();
        assertThat(client.service.status()).isEqualTo(Status.CONNECTING);
        assertThat(client.service.accountName()).isEqualTo("Frost");
        client.now += 1_000;
        client.settle();
        assertThat(client.service.status()).isEqualTo(Status.SIGNED_IN);
    }

    @Test
    void playersOnTheSameServerSeeEachOther() {
        Client alex = signedUp("AlexAccount");
        Client steve = signedUp("SteveAccount");
        Map<UUID, String> tab = Map.of(ALEX, "Alex", STEVE, "Steve");

        alex.service.wardrobe().wear(java.util.Objects.requireNonNull(Catalog.byId("cape.frost")));
        alex.service.joined("play.example.com", "Alex");
        alex.service.tabList(tab);
        alex.settle();
        // The address goes to the service only for friends' sake, and only while shared.
        JsonObject presence = presence(0);
        assertThat(presence.get("serverHash").getAsString()).isEqualTo(ServerHash.of("Play.Example.com:25565"));
        assertThat(presence.get("server").getAsString()).isEqualTo("play.example.com");

        steve.service.joined("PLAY.example.com.", "Steve");
        steve.service.tabList(tab);
        steve.settle();

        // Steve's lookup found Alex; Alex heard Steve arrive.
        IncariPlayer alexSeen = steve.service.player(ALEX);
        assertThat(alexSeen).isNotNull();
        assertThat(alexSeen.cosmetics()).containsExactly("cape.frost");
        assertThat(steve.service.player(STEVE)).isNull();
        alex.settle();
        assertThat(alex.service.player(STEVE)).isNotNull();

        // Steve leaves: Alex hears it, once the grace for a server switch is over.
        steve.service.left();
        steve.now += OnlineService.PRESENCE_GRACE_MILLIS;
        steve.settle();
        alex.settle();
        assertThat(alex.service.player(STEVE)).isNull();
    }

    @Test
    void differentServersNeverMatch() {
        Client alex = signedUp("AlexAccount");
        Client steve = signedUp("SteveAccount");
        Map<UUID, String> tab = Map.of(ALEX, "Alex", STEVE, "Steve");
        alex.service.joined("play.example.com", "Alex");
        alex.service.tabList(tab);
        steve.service.joined("mc.example.com", "Steve");
        steve.service.tabList(tab);
        alex.settle();
        steve.settle();
        assertThat(alex.service.player(STEVE)).isNull();
        assertThat(steve.service.player(ALEX)).isNull();
    }

    @Test
    void notSharingTheServerLeavesTheAddressOut() {
        Client alex = signedUp("AlexAccount");
        assertThat(alex.<Void>await(done -> alex.service.setShareServer(false, done))
                        .ok())
                .isTrue();
        alex.service.joined("play.example.com", "Alex");
        alex.settle();
        assertThat(presence(0).has("server")).isFalse();
        assertThat(alex.service.sharesServer()).isFalse();
    }

    @Test
    void blockingHidesBothWaysAndUnblockingBringsThemBack() {
        Client alex = signedUp("AlexAccount");
        Client steve = signedUp("SteveAccount");
        Map<UUID, String> tab = Map.of(ALEX, "Alex", STEVE, "Steve");
        alex.service.joined("play.example.com", "Alex");
        alex.service.tabList(tab);
        steve.service.joined("play.example.com", "Steve");
        steve.service.tabList(tab);
        alex.settle();
        steve.settle();
        IncariPlayer steveSeen = alex.service.player(STEVE);
        assertThat(steveSeen).isNotNull();

        assertThat(alex.<Void>await(done -> alex.service.blockPlayer("Steve", steveSeen, done))
                        .ok())
                .isTrue();
        assertThat(alex.service.player(STEVE)).isNull();
        List<OnlineService.Block> blocks =
                alex.<List<OnlineService.Block>>await(alex.service::blocks).value();
        // Labelled by the name Alex saw, never the account's username.
        assertThat(blocks).extracting(OnlineService.Block::label).containsExactly("Steve");

        assertThat(alex.<Void>await(done -> alex.service.unblock(blocks.get(0), done))
                        .ok())
                .isTrue();
        alex.now += WorldPlayers.LOOKUP_INTERVAL_MILLIS;
        alex.settle();
        assertThat(alex.service.player(STEVE)).isNotNull();
    }

    @Test
    void reportsGoThrough() {
        Client client = signedUp("Frost");
        assertThat(client.<Void>await(done -> client.service.reportUsername(
                                "Someone", OnlineService.ReportReason.SPAM, "x".repeat(900), done))
                        .ok())
                .isTrue();
        JsonObject report = fake.received.get(fake.received.size() - 1);
        assertThat(report.get("reason").getAsString()).isEqualTo("spam");
        assertThat(report.get("note").getAsString()).hasSize(500);
    }

    @Test
    void signedOutClientsCantUseAccountRequests() {
        Client client = started();
        assertThat(client.<Void>await(done -> client.service.blockUsername("x", done))
                        .failure())
                .isEqualTo(Failure.SIGNED_OUT);
    }

    @Test
    void passwordsAreNeverWarnedAbout() {
        Client client = signedUp("Frost");
        client.<Void>await(done -> client.service.signIn("Frost", PASSWORD, done));
        assertThat(client.warnings).noneMatch(message -> message.contains(PASSWORD));
    }

    @Test
    void aSignUpCancelledBeforeItsFormMakesNoAccount() {
        Client client = started();
        List<Result<List<String>>> heard = new java.util.ArrayList<>();
        client.service.signUp("Frost", PASSWORD, heard::add);
        assertThat(client.service.cancelSignUp()).isTrue();
        client.settle();

        assertThat(heard).isEmpty();
        assertThat(fake.accounts).isEmpty();
        assertThat(fake.received)
                .noneMatch(message -> message.get("t").getAsString().equals("signup"));
        assertThat(client.service.status()).isEqualTo(Status.SIGNED_OUT);
    }

    @Test
    void aSignUpWhoseFormIsOutStillHandsOverTheCodes() {
        Client client = started();
        List<Result<List<String>>> heard = new java.util.ArrayList<>();
        client.service.signUp("Frost", PASSWORD, heard::add);
        // The challenge's answer, then the solved nonce: the form goes.
        client.service.tick();
        client.service.tick();
        assertThat(fake.received)
                .anyMatch(message -> message.get("t").getAsString().equals("signup"));

        assertThat(client.service.cancelSignUp()).isFalse();
        client.settle();
        assertThat(heard).hasSize(1);
        assertThat(heard.get(0).value()).hasSize(10);
        assertThat(client.service.status()).isEqualTo(Status.SIGNED_IN);
    }

    @Test
    void aSignInWithNoAnswerStartsAFreshConnection() {
        signedUp("Frost").service.signOut(false, result -> {});
        Client client = started();
        int sockets = fake.sockets.size();
        fake.unanswered.add("signin");
        List<Result<Void>> heard = new java.util.ArrayList<>();
        client.service.signIn("Frost", PASSWORD, heard::add);
        client.now += OnlineService.REQUEST_TIMEOUT_MILLIS;
        client.settle();

        // The service may have signed the old connection in: it goes, and the next starts signed out.
        assertThat(heard).extracting(Result::failure).containsExactly(Failure.OFFLINE);
        assertThat(fake.sockets.get(sockets - 1).open).isFalse();
        fake.unanswered.clear();
        client.now += 60_000;
        client.settle();
        assertThat(fake.sockets).hasSize(sockets + 1);
        assertThat(client.service.status()).isEqualTo(Status.SIGNED_OUT);
        assertThat(client.<Void>await(done -> client.service.signIn("Frost", PASSWORD, done))
                        .ok())
                .isTrue();
    }

    @Test
    void anAnswerThatCantBeReadFailsOnlyItsRequest() {
        Client alex = signedUp("AlexAccount");
        fake.unanswered.add("befriend");
        List<Result<Void>> heard = new java.util.ArrayList<>();
        alex.service.befriend("SomeoneElse", heard::add);
        alex.settle();
        int id = fake.received.get(fake.received.size() - 1).get("id").getAsInt();

        JsonObject bad = new JsonObject();
        bad.addProperty("t", "ok");
        bad.addProperty("id", id);
        bad.addProperty("request", "not a request");
        JsonObject request = new JsonObject();
        request.addProperty("t", "request");
        request.addProperty("rid", "r9");
        request.addProperty("incoming", true);
        request.addProperty("label", "Friendly");
        // Both in one tick: the bad answer mustn't take the event after it down with it.
        fake.last().say(bad);
        fake.last().say(request);
        alex.settle();

        assertThat(heard).extracting(Result::failure).containsExactly(Failure.UNAVAILABLE);
        assertThat(alex.service.social().requests())
                .extracting(Social.Request::rid)
                .containsExactly("r9");
    }

    private JsonObject presence(int which) {
        return fake.received.stream()
                .filter(message -> message.get("t").getAsString().equals("presence"))
                .skip(which)
                .findFirst()
                .orElseThrow();
    }
}
