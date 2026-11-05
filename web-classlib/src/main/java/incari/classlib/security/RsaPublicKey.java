package incari.classlib.security;

import incari.classlib.substitutes.java.security.TPublicKey;
import java.util.Arrays;
import org.bouncycastle.crypto.params.RSAKeyParameters;

/**
 * An RSA public key for the browser build's {@code KeyFactory}, {@code Cipher} and
 * {@code Signature} (ADR 0070): Bouncy Castle's parameters and the X.509 encoding it came from.
 * {@link #getEncoded} hands back exactly those bytes, since the login handshake hashes the
 * server's key as the server sent it.
 */
public final class RsaPublicKey implements TPublicKey {

    private static final long serialVersionUID = 1L;

    private final transient RSAKeyParameters parameters;
    private final byte[] encoded;

    public RsaPublicKey(RSAKeyParameters parameters, byte[] encoded) {
        if (parameters.isPrivate()) {
            throw new IllegalArgumentException("Not a public key");
        }
        this.parameters = parameters;
        this.encoded = encoded.clone();
    }

    public RSAKeyParameters parameters() {
        return parameters;
    }

    @Override
    public String getAlgorithm() {
        return "RSA";
    }

    @Override
    public String getFormat() {
        return "X.509";
    }

    @Override
    public byte[] getEncoded() {
        return encoded.clone();
    }

    @Override
    public boolean equals(Object other) {
        return other instanceof RsaPublicKey that && Arrays.equals(encoded, that.encoded);
    }

    @Override
    public int hashCode() {
        return Arrays.hashCode(encoded);
    }

    @Override
    public String toString() {
        return "RSA public key, " + parameters.getModulus().bitLength() + " bits";
    }
}
