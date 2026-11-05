package incari.classlib.substitutes.javax.crypto;

import java.security.GeneralSecurityException;

/** {@code javax.crypto.BadPaddingException}, which TeaVM lacks: decrypted data with broken padding (ADR 0070). */
public class TBadPaddingException extends GeneralSecurityException {

    private static final long serialVersionUID = 1L;

    public TBadPaddingException() {}

    public TBadPaddingException(String message) {
        super(message);
    }

    public TBadPaddingException(String message, Throwable cause) {
        super(message, cause);
    }

    public TBadPaddingException(Throwable cause) {
        super(cause);
    }
}
