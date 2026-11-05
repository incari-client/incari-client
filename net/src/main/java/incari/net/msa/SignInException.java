package incari.net.msa;

import java.io.IOException;

/**
 * Sign-in, a refresh or a join failed. The message is plain words for the player's screen, and
 * never holds a token. A {@linkplain #temporary temporary} one (the network, or a service's own
 * trouble) is worth trying again; any other is Microsoft's or Xbox's answer, final for this try.
 */
public final class SignInException extends IOException {

    private static final long serialVersionUID = 1L;

    private final boolean temporary;

    public SignInException(String message) {
        this(message, false);
    }

    public SignInException(String message, Throwable cause) {
        this(message, cause, false);
    }

    public SignInException(String message, boolean temporary) {
        super(message);
        this.temporary = temporary;
    }

    public SignInException(String message, Throwable cause, boolean temporary) {
        super(message, cause);
        this.temporary = temporary;
    }

    /** Whether trying the same again may work: the request didn't get through, or the service was down. */
    public boolean temporary() {
        return temporary;
    }
}
