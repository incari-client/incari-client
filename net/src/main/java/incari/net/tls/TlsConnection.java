package incari.net.tls;

import incari.platform.api.net.NetConnection;
import java.io.EOFException;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.security.SecureRandom;
import java.util.ArrayList;
import java.util.List;
import java.util.Vector;
import java.util.function.LongSupplier;
import org.bouncycastle.tls.AlertDescription;
import org.bouncycastle.tls.CertificateRequest;
import org.bouncycastle.tls.DefaultTlsClient;
import org.bouncycastle.tls.NameType;
import org.bouncycastle.tls.ProtocolName;
import org.bouncycastle.tls.ProtocolVersion;
import org.bouncycastle.tls.ServerName;
import org.bouncycastle.tls.TlsAuthentication;
import org.bouncycastle.tls.TlsClientProtocol;
import org.bouncycastle.tls.TlsCredentials;
import org.bouncycastle.tls.TlsFatalAlert;
import org.bouncycastle.tls.TlsServerCertificate;
import org.bouncycastle.tls.crypto.TlsCertificate;
import org.bouncycastle.tls.crypto.impl.bc.BcTlsCrypto;
import org.jspecify.annotations.Nullable;

/**
 * A TLS 1.2 or 1.3 client connection over another connection's bytes ({@link Transport}): Bouncy
 * Castle's protocol engine in its non-blocking mode, fed as bytes arrive, so nothing waits. What it
 * hands its listener is the decrypted stream, and {@link Listener#connected} comes only once the
 * handshake has finished and the server's certificate passed {@link CertificateChain}.
 *
 * <p>Encryption is end to end: a Wisp proxy in between carries only ciphertext, and can't read or
 * change what's sent without the handshake or a record failing.
 *
 * <p>Closing: {@link Listener#closed} gets null only after the server's close_notify; a transport
 * that ends without one gets an {@link EOFException}, so a reader framing by connection close
 * can tell a truncated stream from a whole one.
 */
public final class TlsConnection implements NetConnection {

    private final String host;
    private final Listener listener;
    private final CertificateChain certificates;
    private final LongSupplier clock;
    private final TlsClientProtocol protocol = new TlsClientProtocol();
    private @Nullable NetConnection transport;

    // Guarded by this.
    private boolean handshaken;
    private boolean closed;
    /** What was sent before the handshake finished, sent once it has. */
    private final List<byte[]> early = new ArrayList<>();

    private TlsConnection(String host, Listener listener, CertificateChain certificates, LongSupplier clock) {
        this.host = host;
        this.listener = listener;
        this.certificates = certificates;
        this.clock = clock;
    }

    /**
     * Connects to {@code host}:{@code port} over {@code transport} and starts the handshake.
     * Returns at once.
     *
     * @param clock the time certificates are checked against, in epoch milliseconds
     */
    public static NetConnection open(
            Transport transport,
            String host,
            int port,
            CertificateChain certificates,
            LongSupplier clock,
            Listener listener) {
        TlsConnection connection = new TlsConnection(host, listener, certificates, clock);
        synchronized (connection) {
            // Held so the transport's first callbacks wait for the field.
            connection.transport = transport.connect(host, port, connection.new TransportListener());
        }
        return connection;
    }

    @Override
    public void send(ByteBuffer data) {
        byte[] bytes = new byte[data.remaining()];
        data.get(bytes);
        @Nullable IOException failure = null;
        synchronized (this) {
            if (closed) {
                return;
            }
            if (!handshaken) {
                early.add(bytes);
                return;
            }
            try {
                protocol.writeApplicationData(bytes, 0, bytes.length);
                drain();
            } catch (IOException e) {
                failure = e;
            }
        }
        if (failure != null) {
            fail(failure);
        }
    }

    @Override
    public void close() {
        boolean tell;
        synchronized (this) {
            tell = !closed;
            if (tell) {
                try {
                    protocol.close();
                    drain();
                } catch (IOException ignored) {
                    // Closing anyway.
                }
            }
            finish();
        }
        if (tell) {
            listener.closed(null);
        }
    }

    @Override
    public synchronized boolean isOpen() {
        return handshaken && !closed;
    }

    @Override
    public String remoteDescription() {
        NetConnection current = transport;
        return "TLS to " + host + (current != null ? " over " + current.remoteDescription() : "");
    }

    // --- Under the lock ------------------------------------------------------------------------

    /** Sends whatever the protocol has written (handshake messages, records, alerts). */
    private void drain() {
        int available = protocol.getAvailableOutputBytes();
        NetConnection current = transport;
        if (available <= 0 || current == null) {
            return;
        }
        byte[] out = new byte[available];
        int read = protocol.readOutput(out, 0, available);
        current.send(ByteBuffer.wrap(out, 0, read));
        current.flush();
    }

    private void finish() {
        closed = true;
        early.clear();
        NetConnection current = transport;
        if (current != null) {
            current.close();
        }
    }

    private void fail(Throwable cause) {
        boolean tell;
        synchronized (this) {
            tell = !closed;
            if (tell) {
                drain(); // A fatal alert, if the protocol wrote one.
            }
            finish();
        }
        if (tell) {
            listener.closed(cause);
        }
    }

    // --- Callbacks -----------------------------------------------------------------------------

    private final class TransportListener implements Listener {

        @Override
        public void connected() {
            @Nullable IOException failure = null;
            synchronized (TlsConnection.this) {
                if (closed) {
                    return;
                }
                try {
                    protocol.connect(new Client());
                    drain();
                } catch (IOException e) {
                    failure = e;
                }
            }
            if (failure != null) {
                fail(failure);
            }
        }

        @Override
        public void received(ByteBuffer data) {
            byte[] bytes = new byte[data.remaining()];
            data.get(bytes);
            boolean nowConnected = false;
            byte @Nullable [] plain = null;
            boolean peerClosed = false;
            @Nullable Throwable failure = null;
            synchronized (TlsConnection.this) {
                if (closed) {
                    return;
                }
                try {
                    boolean wasHandshaken = handshaken;
                    protocol.offerInput(bytes);
                    if (!wasHandshaken && handshaken) {
                        nowConnected = true;
                        for (byte[] queued : early) {
                            protocol.writeApplicationData(queued, 0, queued.length);
                        }
                        early.clear();
                    }
                    drain();
                    int available = protocol.getAvailableInputBytes();
                    if (available > 0) {
                        plain = new byte[available];
                        protocol.readInput(plain, 0, available);
                    }
                    peerClosed = protocol.isClosed();
                } catch (IOException | RuntimeException e) {
                    failure = e;
                }
            }
            if (failure != null) {
                fail(failure);
                return;
            }
            if (nowConnected) {
                listener.connected();
            }
            if (plain != null) {
                listener.received(ByteBuffer.wrap(plain));
            }
            if (peerClosed) {
                boolean tell;
                synchronized (TlsConnection.this) {
                    tell = !closed;
                    finish();
                }
                if (tell) {
                    listener.closed(null);
                }
            }
        }

        @Override
        public void closed(@Nullable Throwable cause) {
            boolean tell;
            boolean wasHandshaken;
            synchronized (TlsConnection.this) {
                tell = !closed;
                wasHandshaken = handshaken;
                closed = true;
                early.clear();
            }
            if (tell) {
                Throwable reason = cause != null
                        ? cause
                        : new EOFException(
                                wasHandshaken
                                        ? "The connection to " + host + " ended without a TLS close"
                                        : "The connection to " + host + " ended during the TLS handshake");
                listener.closed(reason);
            }
        }
    }

    /** Bouncy Castle's client side: TLS 1.2 and 1.3, the host's name, and the pinned roots. */
    private final class Client extends DefaultTlsClient {

        Client() {
            super(new BcTlsCrypto(new SecureRandom()));
        }

        @Override
        protected ProtocolVersion[] getSupportedVersions() {
            return ProtocolVersion.TLSv13.downTo(ProtocolVersion.TLSv12);
        }

        @Override
        @SuppressWarnings({"JdkObsolete", "rawtypes", "unchecked"}) // Bouncy Castle's API takes a Vector.
        protected Vector getSNIServerNames() {
            Vector names = new Vector(1);
            names.add(new ServerName(NameType.host_name, host.getBytes(StandardCharsets.US_ASCII)));
            return names;
        }

        @Override
        @SuppressWarnings({"JdkObsolete", "rawtypes", "unchecked"}) // Bouncy Castle's API takes a Vector.
        protected Vector getProtocolNames() {
            Vector names = new Vector(1);
            names.add(ProtocolName.HTTP_1_1);
            return names;
        }

        @Override
        public TlsAuthentication getAuthentication() {
            return new TlsAuthentication() {
                @Override
                public void notifyServerCertificate(TlsServerCertificate serverCertificate) throws IOException {
                    TlsCertificate[] chain = serverCertificate.getCertificate().getCertificateList();
                    List<byte[]> encoded = new ArrayList<>(chain.length);
                    for (TlsCertificate certificate : chain) {
                        encoded.add(certificate.getEncoded());
                    }
                    try {
                        certificates.check(host, encoded, clock.getAsLong());
                    } catch (UntrustedCertificateException e) {
                        throw new TlsFatalAlert(AlertDescription.bad_certificate, e.getMessage(), e);
                    }
                }

                @Override
                public @Nullable TlsCredentials getClientCredentials(CertificateRequest request) {
                    return null;
                }
            };
        }

        @Override
        public void notifyHandshakeComplete() throws IOException {
            super.notifyHandshakeComplete();
            // Called from inside offerInput, under the connection's lock.
            handshaken = true;
        }
    }
}
