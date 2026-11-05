package incari.classlib.substitutes.javax.crypto;

import java.security.GeneralSecurityException;

/** {@code javax.crypto.ShortBufferException}, which TeaVM lacks: an output buffer too small for the result (ADR 0070). */
public class TShortBufferException extends GeneralSecurityException {

    private static final long serialVersionUID = 1L;

    public TShortBufferException() {}

    public TShortBufferException(String message) {
        super(message);
    }

    public TShortBufferException(String message, Throwable cause) {
        super(message, cause);
    }

    public TShortBufferException(Throwable cause) {
        super(cause);
    }
}
