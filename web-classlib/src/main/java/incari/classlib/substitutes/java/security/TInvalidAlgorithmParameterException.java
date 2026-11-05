package incari.classlib.substitutes.java.security;

import java.security.GeneralSecurityException;

/** {@code java.security.InvalidAlgorithmParameterException}, which TeaVM lacks: parameters an algorithm can't use (ADR 0070). */
public class TInvalidAlgorithmParameterException extends GeneralSecurityException {

    private static final long serialVersionUID = 1L;

    public TInvalidAlgorithmParameterException() {}

    public TInvalidAlgorithmParameterException(String message) {
        super(message);
    }

    public TInvalidAlgorithmParameterException(String message, Throwable cause) {
        super(message, cause);
    }

    public TInvalidAlgorithmParameterException(Throwable cause) {
        super(cause);
    }
}
