package incari.classlib.security;

import incari.classlib.substitutes.java.security.TPrivateKey;
import java.util.Arrays;
import org.bouncycastle.crypto.params.RSAKeyParameters;

/**
 * An RSA private key for the browser build's {@code KeyFactory} and {@code Signature} (ADR 0070):
 * Bouncy Castle's parameters and the PKCS#8 encoding it came from. Never printed.
 */
public final class RsaPrivateKey implements TPrivateKey {

    private static final long serialVersionUID = 1L;

    private final transient RSAKeyParameters parameters;
    private final byte[] encoded;

    public RsaPrivateKey(RSAKeyParameters parameters, byte[] encoded) {
        if (!parameters.isPrivate()) {
            throw new IllegalArgumentException("Not a private key");
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
        return "PKCS#8";
    }

    @Override
    public byte[] getEncoded() {
        return encoded.clone();
    }

    @Override
    public boolean equals(Object other) {
        return other instanceof RsaPrivateKey that && Arrays.equals(encoded, that.encoded);
    }

    @Override
    public int hashCode() {
        return Arrays.hashCode(encoded);
    }

    @Override
    public String toString() {
        return "RSA private key";
    }
}
