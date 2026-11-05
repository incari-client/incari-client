package incari.net.tls;

/** A server's certificate chain failed {@link CertificateChain}'s checks; the message says why, in plain words. */
public final class UntrustedCertificateException extends Exception {

    private static final long serialVersionUID = 1L;

    public UntrustedCertificateException(String message) {
        super(message);
    }

    public UntrustedCertificateException(String message, Throwable cause) {
        super(message, cause);
    }
}
