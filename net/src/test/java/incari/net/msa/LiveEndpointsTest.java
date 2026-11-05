package incari.net.msa;

import static org.assertj.core.api.Assertions.assertThat;

import incari.net.https.HttpRequest;
import incari.net.https.HttpResponse;
import incari.net.https.HttpsClient;
import incari.net.tls.SocketTransport;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.condition.EnabledIfEnvironmentVariable;

/**
 * The real hosts, through the real TLS and certificate checks: off unless {@code INCARI_LIVE=1}, as
 * it needs the network. Signs nobody in: it asks for a code and sends tokens that are refused.
 */
@EnabledIfEnvironmentVariable(named = "INCARI_LIVE", matches = "1")
class LiveEndpointsTest {

    private final HttpsClient https = HttpsClient.pinned(new SocketTransport(), MicrosoftEndpoints.HOSTS);
    private final MicrosoftSignIn signIn = new MicrosoftSignIn(https, System::currentTimeMillis);

    @Test
    void microsoftGivesADeviceCode() throws Exception {
        DeviceCode code = signIn.requestCode().get(30, TimeUnit.SECONDS);
        assertThat(code.userCode()).isNotEmpty();
        assertThat(code.pageUrl()).startsWith("https://www.microsoft.com/");

        MicrosoftSignIn.Poll poll = signIn.poll(code).get(30, TimeUnit.SECONDS);
        assertThat(poll.state()).isEqualTo(MicrosoftSignIn.PollState.WAITING);
    }

    @Test
    void everyHostPassesTheCertificateChecks() throws Exception {
        for (String host : MicrosoftEndpoints.HOSTS) {
            HttpResponse response = https.send(HttpRequest.get("https://" + host + "/", Map.of()))
                    .get(30, TimeUnit.SECONDS);
            assertThat(response.status()).as(host).isBetween(200, 599);
        }
    }

    @Test
    void theSessionServerRefusesAMadeUpToken() throws Exception {
        MicrosoftAccount account =
                new MicrosoftAccount("Nobody", "0123456789abcdef0123456789abcdef", "not-a-token", Long.MAX_VALUE, "x");
        assertThat(signIn.join(account, "-1234abcd").get(30, TimeUnit.SECONDS)).isEqualTo(JoinResult.INVALID_SESSION);
    }

    @Test
    void aHostOffTheListIsRefusedWithoutConnecting() {
        HttpsClient narrow = HttpsClient.pinned(new SocketTransport(), Set.of("login.live.com"));
        assertThat(narrow.send(HttpRequest.get("https://example.com/", Map.of())))
                .failsWithin(1, TimeUnit.SECONDS)
                .withThrowableOfType(ExecutionException.class)
                .withMessageContaining("example.com");
    }

    @Test
    void anUntrustedChainFailsTheHandshake() {
        // example.com chains to a root that isn't pinned.
        HttpsClient other = HttpsClient.pinned(new SocketTransport(), Set.of("example.com"));
        assertThat(other.send(HttpRequest.get("https://example.com/", Map.of())))
                .failsWithin(30, TimeUnit.SECONDS)
                .withThrowableOfType(ExecutionException.class)
                .withMessageContaining("trusted root");
    }
}
