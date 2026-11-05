package incari.net.msa;

import java.io.IOException;

/**
 * Sign-in, a refresh or a join failed. The message is plain words for the player's screen, and
 * never holds a token.
 */
public final class SignInException extends IOException {

    private static final long serialVersionUID = 1L;

    public SignInException(String message) {
        super(message);
    }

    public SignInException(String message, Throwable cause) {
        super(message, cause);
    }
}
