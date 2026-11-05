package incari.net.tls;

import static org.assertj.core.api.Assertions.assertThat;

import incari.net.https.HttpRequest;
import incari.net.https.HttpResponse;
import incari.net.https.HttpsClient;
import incari.platform.api.net.NetConnection;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.KeyStore;
import java.security.cert.X509Certificate;
import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.Date;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeUnit;
import javax.net.ssl.KeyManagerFactory;
import javax.net.ssl.SSLContext;
import javax.net.ssl.SSLServerSocket;
import javax.net.ssl.SSLSocket;
import org.bouncycastle.asn1.x500.X500Name;
import org.bouncycastle.asn1.x509.BasicConstraints;
import org.bouncycastle.asn1.x509.Certificate;
import org.bouncycastle.asn1.x509.ExtendedKeyUsage;
import org.bouncycastle.asn1.x509.Extension;
import org.bouncycastle.asn1.x509.GeneralName;
import org.bouncycastle.asn1.x509.GeneralNames;
import org.bouncycastle.asn1.x509.KeyPurposeId;
import org.bouncycastle.cert.X509v3CertificateBuilder;
import org.bouncycastle.cert.jcajce.JcaX509CertificateConverter;
import org.bouncycastle.cert.jcajce.JcaX509v3CertificateBuilder;
import org.bouncycastle.operator.jcajce.JcaContentSignerBuilder;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;

/**
 * {@link TlsConnection} and {@link HttpsClient} against the JDK's own TLS server on localhost, with a
 * chain made for the test: real handshakes (TLS 1.3 and 1.2), no network.
 */
class TlsConnectionTest {

    private static Certificate root;
    private static SSLContext serverContext;

    private SSLServerSocket server;
    private Thread serverThread;

    @BeforeAll
    static void chain() throws Exception {
        KeyPairGenerator rsa = KeyPairGenerator.getInstance("RSA");
        rsa.initialize(2048);
        KeyPair rootKey = rsa.generateKeyPair();
        KeyPair leafKey = rsa.generateKeyPair();
        Date from = Date.from(Instant.now().minus(1, ChronoUnit.DAYS));
        Date to = Date.from(Instant.now().plus(30, ChronoUnit.DAYS));

        X509v3CertificateBuilder rootBuilder = new JcaX509v3CertificateBuilder(
                new X500Name("CN=Local Root"),
                BigInteger.ONE,
                from,
                to,
                new X500Name("CN=Local Root"),
                rootKey.getPublic());
        rootBuilder.addExtension(Extension.basicConstraints, true, new BasicConstraints(true));
        root = rootBuilder
                .build(new JcaContentSignerBuilder("SHA256withRSA").build(rootKey.getPrivate()))
                .toASN1Structure();

        X509v3CertificateBuilder leafBuilder = new JcaX509v3CertificateBuilder(
                new X500Name("CN=Local Root"),
                BigInteger.TWO,
                from,
                to,
                new X500Name("CN=localhost"),
                leafKey.getPublic());
        leafBuilder.addExtension(
                Extension.subjectAlternativeName,
                false,
                new GeneralNames(new GeneralName(GeneralName.dNSName, "localhost")));
        leafBuilder.addExtension(
                Extension.extendedKeyUsage, false, new ExtendedKeyUsage(KeyPurposeId.id_kp_serverAuth));
        X509Certificate leaf = new JcaX509CertificateConverter()
                .getCertificate(
                        leafBuilder.build(new JcaContentSignerBuilder("SHA256withRSA").build(rootKey.getPrivate())));

        KeyStore store = KeyStore.getInstance("PKCS12");
        store.load(null, null);
        store.setKeyEntry("leaf", leafKey.getPrivate(), new char[0], new X509Certificate[] {leaf});
        KeyManagerFactory keys = KeyManagerFactory.getInstance(KeyManagerFactory.getDefaultAlgorithm());
        keys.init(store, new char[0]);
        serverContext = SSLContext.getInstance("TLS");
        serverContext.init(keys.getKeyManagers(), null, null);
    }

    @AfterEach
    void stop() throws Exception {
        if (server != null) {
            server.close();
        }
        if (truncatingServer != null) {
            truncatingServer.close();
        }
        if (serverThread != null) {
            serverThread.join(5000);
        }
    }

    @Test
    void sendsARequestAndReadsTheResponseOverTls13() throws Exception {
        start("TLSv1.3", "HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello", true);
        HttpResponse response = client("localhost").send(get()).get(10, TimeUnit.SECONDS);
        assertThat(response.status()).isEqualTo(200);
        assertThat(response.text()).isEqualTo("hello");
        assertThat(lastRequest).startsWith("GET /path HTTP/1.1\r\nHost: localhost\r\n");
    }

    @Test
    void worksOverTls12() throws Exception {
        start("TLSv1.2", "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok", true);
        assertThat(client("localhost").send(get()).get(10, TimeUnit.SECONDS).text())
                .isEqualTo("ok");
    }

    @Test
    void readsABodyThatEndsWithTheConnection() throws Exception {
        start("TLSv1.3", "HTTP/1.1 200 OK\r\n\r\nuntil close", true);
        assertThat(client("localhost").send(get()).get(10, TimeUnit.SECONDS).text())
                .isEqualTo("until close");
    }

    @Test
    void aBodyCutOffWithoutATlsCloseIsRefused() throws Exception {
        startTruncating("HTTP/1.1 200 OK\r\n\r\npartial");
        assertThat(client("localhost").send(get()))
                .failsWithin(10, TimeUnit.SECONDS)
                .withThrowableOfType(ExecutionException.class)
                .withMessageContaining("without a TLS close");
    }

    @Test
    void refusesAServerWhoseCertificateIsForAnotherName() throws Exception {
        start("TLSv1.3", "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n", true);
        // The leaf names localhost; connect to the same port as 127.0.0.1's "name".
        HttpsClient client = new HttpsClient(
                new LoopbackTransport(port()),
                Set.of("other.test"),
                new CertificateChain(List.of(root)),
                System::currentTimeMillis);
        assertThat(client.send(HttpRequest.get("https://other.test/path", Map.of())))
                .failsWithin(10, TimeUnit.SECONDS)
                .withThrowableOfType(ExecutionException.class)
                .withMessageContaining("isn't for other.test");
    }

    @Test
    void refusesAServerUnderAnUnpinnedRoot() throws Exception {
        start("TLSv1.3", "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n", true);
        HttpsClient client = HttpsClient.pinned(new LoopbackTransport(port()), Set.of("localhost"));
        assertThat(client.send(get()))
                .failsWithin(10, TimeUnit.SECONDS)
                .withThrowableOfType(ExecutionException.class)
                .withMessageContaining("trusted root");
    }

    // --- Helpers -------------------------------------------------------------------------------

    private volatile String lastRequest = "";

    private static HttpRequest get() {
        return HttpRequest.get("https://localhost/path", Map.of());
    }

    private HttpsClient client(String host) {
        return new HttpsClient(
                new LoopbackTransport(port()),
                Set.of(host),
                new CertificateChain(List.of(root)),
                System::currentTimeMillis);
    }

    private int port() {
        return truncatingServer != null ? truncatingServer.getLocalPort() : server.getLocalPort();
    }

    /** Serves one connection: reads the request head, writes {@code response}, closes. */
    private void start(String protocol, String response, boolean closeAfter) throws Exception {
        server = (SSLServerSocket) serverContext.getServerSocketFactory().createServerSocket(0);
        server.setEnabledProtocols(new String[] {protocol});
        serverThread = new Thread(() -> {
            try (SSLSocket socket = (SSLSocket) server.accept()) {
                InputStream in = socket.getInputStream();
                ByteArrayOutputStream head = new ByteArrayOutputStream();
                while (!head.toString(StandardCharsets.US_ASCII).endsWith("\r\n\r\n")) {
                    int b = in.read();
                    if (b < 0) {
                        return;
                    }
                    head.write(b);
                }
                lastRequest = head.toString(StandardCharsets.US_ASCII);
                OutputStream out = socket.getOutputStream();
                out.write(response.getBytes(StandardCharsets.US_ASCII));
                out.flush();
                if (closeAfter) {
                    socket.shutdownOutput(); // Sends close_notify.
                }
            } catch (Exception e) {
                // The client may hang up first in the failure tests.
            }
        });
        serverThread.setDaemon(true);
        serverThread.start();
    }

    /**
     * Serves one connection, then drops the TCP connection under TLS without a close_notify, as a
     * hostile proxy cutting the response short would.
     */
    private void startTruncating(String response) throws Exception {
        java.net.ServerSocket plain = new java.net.ServerSocket(0);
        server = (SSLServerSocket) serverContext.getServerSocketFactory().createServerSocket(0);
        server.close();
        truncatingServer = plain;
        serverThread = new Thread(() -> {
            try (java.net.Socket raw = plain.accept()) {
                SSLSocket tls =
                        (SSLSocket) serverContext.getSocketFactory().createSocket(raw, null, raw.getPort(), false);
                tls.setUseClientMode(false);
                InputStream in = tls.getInputStream();
                ByteArrayOutputStream head = new ByteArrayOutputStream();
                while (!head.toString(StandardCharsets.US_ASCII).endsWith("\r\n\r\n")) {
                    int b = in.read();
                    if (b < 0) {
                        return;
                    }
                    head.write(b);
                }
                tls.getOutputStream().write(response.getBytes(StandardCharsets.US_ASCII));
                tls.getOutputStream().flush();
                // The raw socket closes here, with no TLS close_notify.
            } catch (Exception e) {
                // The client hangs up.
            }
        });
        serverThread.setDaemon(true);
        serverThread.start();
    }

    private java.net.ServerSocket truncatingServer;

    /** Connects every host name to the local server's port. */
    private record LoopbackTransport(int port) implements Transport {
        @Override
        public NetConnection connect(String host, int ignoredPort, NetConnection.Listener listener) {
            return new SocketTransport().connect("127.0.0.1", port, listener);
        }
    }
}
