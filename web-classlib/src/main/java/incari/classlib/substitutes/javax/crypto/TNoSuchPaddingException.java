package incari.classlib.substitutes.javax.crypto;

import java.security.GeneralSecurityException;

/** {@code javax.crypto.NoSuchPaddingException}, which TeaVM lacks: an unknown padding (ADR 0070). */
public class TNoSuchPaddingException extends GeneralSecurityException {

    private static final long serialVersionUID = 1L;

    public TNoSuchPaddingException() {}

    public TNoSuchPaddingException(String message) {
        super(message);
    }

    public TNoSuchPaddingException(String message, Throwable cause) {
        super(message, cause);
    }

    public TNoSuchPaddingException(Throwable cause) {
        super(cause);
    }
}
