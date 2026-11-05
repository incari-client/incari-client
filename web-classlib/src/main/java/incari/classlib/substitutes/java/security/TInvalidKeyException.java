package incari.classlib.substitutes.java.security;

/** {@code java.security.InvalidKeyException}, which TeaVM lacks: a key of the wrong kind or form (ADR 0070). */
public class TInvalidKeyException extends TKeyException {

    private static final long serialVersionUID = 1L;

    public TInvalidKeyException() {}

    public TInvalidKeyException(String message) {
        super(message);
    }

    public TInvalidKeyException(String message, Throwable cause) {
        super(message, cause);
    }

    public TInvalidKeyException(Throwable cause) {
        super(cause);
    }
}
