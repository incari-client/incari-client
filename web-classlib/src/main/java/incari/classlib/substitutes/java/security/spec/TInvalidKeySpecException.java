package incari.classlib.substitutes.java.security.spec;

import java.security.GeneralSecurityException;

/** {@code java.security.spec.InvalidKeySpecException}, which TeaVM lacks: a key's encoding can't be read (ADR 0070). */
public class TInvalidKeySpecException extends GeneralSecurityException {

    private static final long serialVersionUID = 1L;

    public TInvalidKeySpecException() {}

    public TInvalidKeySpecException(String message) {
        super(message);
    }

    public TInvalidKeySpecException(String message, Throwable cause) {
        super(message, cause);
    }

    public TInvalidKeySpecException(Throwable cause) {
        super(cause);
    }
}
