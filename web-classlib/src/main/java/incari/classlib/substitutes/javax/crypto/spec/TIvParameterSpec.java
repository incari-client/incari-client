package incari.classlib.substitutes.javax.crypto.spec;

import incari.classlib.substitutes.java.security.spec.TAlgorithmParameterSpec;
import java.util.Arrays;

/** {@code javax.crypto.spec.IvParameterSpec}, which TeaVM lacks: an initialisation vector (ADR 0070). */
public class TIvParameterSpec implements TAlgorithmParameterSpec {

    private final byte[] iv;

    public TIvParameterSpec(byte[] iv) {
        this(iv, 0, iv.length);
    }

    public TIvParameterSpec(byte[] iv, int offset, int len) {
        if (offset < 0 || len < 0 || offset + len > iv.length) {
            throw new IllegalArgumentException("IV out of range");
        }
        this.iv = Arrays.copyOfRange(iv, offset, offset + len);
    }

    public byte[] getIV() {
        return iv.clone();
    }
}
