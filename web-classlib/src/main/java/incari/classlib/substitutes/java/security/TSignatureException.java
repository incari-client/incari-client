package incari.classlib.substitutes.java.security;

import java.security.GeneralSecurityException;

/** {@code java.security.SignatureException}, which TeaVM lacks: a signature couldn't be made or checked (ADR 0070). */
public class TSignatureException extends GeneralSecurityException {

    private static final long serialVersionUID = 1L;

    public TSignatureException() {}

    public TSignatureException(String message) {
        super(message);
    }

    public TSignatureException(String message, Throwable cause) {
        super(message, cause);
    }

    public TSignatureException(Throwable cause) {
        super(cause);
    }
}
