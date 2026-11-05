package incari.classlib.substitutes.java.security;

import java.security.GeneralSecurityException;

/** {@code java.security.KeyException}, which TeaVM lacks: a key's problem (ADR 0070). */
public class TKeyException extends GeneralSecurityException {

    private static final long serialVersionUID = 1L;

    public TKeyException() {}

    public TKeyException(String message) {
        super(message);
    }

    public TKeyException(String message, Throwable cause) {
        super(message, cause);
    }

    public TKeyException(Throwable cause) {
        super(cause);
    }
}
