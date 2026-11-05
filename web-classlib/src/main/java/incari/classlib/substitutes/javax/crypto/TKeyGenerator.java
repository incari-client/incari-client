package incari.classlib.substitutes.javax.crypto;

import incari.classlib.substitutes.javax.crypto.spec.TSecretKeySpec;
import java.security.NoSuchAlgorithmException;
import java.security.SecureRandom;

/**
 * {@code javax.crypto.KeyGenerator}, which TeaVM lacks, for AES only: the login handshake's shared
 * secret, from the browser's {@code crypto.getRandomValues} through TeaVM's {@code SecureRandom}
 * (ADR 0070).
 */
public class TKeyGenerator {

    private int bits = 128;
    private SecureRandom random = new SecureRandom();

    private TKeyGenerator() {}

    public static TKeyGenerator getInstance(String algorithm) throws NoSuchAlgorithmException {
        if (!algorithm.equalsIgnoreCase("AES")) {
            throw new NoSuchAlgorithmException(algorithm + " KeyGenerator not available");
        }
        return new TKeyGenerator();
    }

    public final String getAlgorithm() {
        return "AES";
    }

    public final void init(int keysize) {
        if (keysize != 128 && keysize != 192 && keysize != 256) {
            throw new IllegalArgumentException("AES keys are 128, 192 or 256 bits");
        }
        bits = keysize;
    }

    public final void init(int keysize, SecureRandom random) {
        init(keysize);
        this.random = random;
    }

    public final void init(SecureRandom random) {
        this.random = random;
    }

    public final TSecretKey generateKey() {
        byte[] key = new byte[bits / 8];
        random.nextBytes(key);
        return new TSecretKeySpec(key, "AES");
    }
}
