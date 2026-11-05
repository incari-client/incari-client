package incari.classlib.substitutes.javax.crypto;

import java.security.GeneralSecurityException;

/** {@code javax.crypto.IllegalBlockSizeException}, which TeaVM lacks: input that isn't a whole number of blocks (ADR 0070). */
public class TIllegalBlockSizeException extends GeneralSecurityException {

    private static final long serialVersionUID = 1L;

    public TIllegalBlockSizeException() {}

    public TIllegalBlockSizeException(String message) {
        super(message);
    }

    public TIllegalBlockSizeException(String message, Throwable cause) {
        super(message, cause);
    }

    public TIllegalBlockSizeException(Throwable cause) {
        super(cause);
    }
}
