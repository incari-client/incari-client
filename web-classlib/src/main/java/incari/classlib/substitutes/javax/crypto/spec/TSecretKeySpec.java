package incari.classlib.substitutes.javax.crypto.spec;

import incari.classlib.substitutes.java.security.spec.TKeySpec;
import incari.classlib.substitutes.javax.crypto.TSecretKey;
import java.util.Arrays;
import java.util.Locale;

/**
 * {@code javax.crypto.spec.SecretKeySpec}, which TeaVM lacks: raw key bytes named by algorithm, as
 * the login handshake's AES key is (ADR 0070).
 */
public class TSecretKeySpec implements TKeySpec, TSecretKey {

    private static final long serialVersionUID = 1L;

    private final byte[] key;
    private final String algorithm;

    public TSecretKeySpec(byte[] key, String algorithm) {
        this(key, 0, key.length, algorithm);
    }

    public TSecretKeySpec(byte[] key, int offset, int len, String algorithm) {
        if (len <= 0 || offset < 0 || offset + len > key.length) {
            throw new IllegalArgumentException("Empty or out-of-range key");
        }
        this.key = Arrays.copyOfRange(key, offset, offset + len);
        this.algorithm = algorithm;
    }

    @Override
    public String getAlgorithm() {
        return algorithm;
    }

    @Override
    public String getFormat() {
        return "RAW";
    }

    @Override
    public byte[] getEncoded() {
        return key.clone();
    }

    @Override
    public boolean equals(Object other) {
        return other instanceof TSecretKeySpec that
                && algorithm.equalsIgnoreCase(that.algorithm)
                && Arrays.equals(key, that.key);
    }

    @Override
    public int hashCode() {
        return Arrays.hashCode(key) ^ algorithm.toLowerCase(Locale.ROOT).hashCode();
    }
}
