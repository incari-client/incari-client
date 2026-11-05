package incari.classlib.substitutes.java.security.spec;

import org.jspecify.annotations.Nullable;

/** {@code java.security.spec.EncodedKeySpec}, which TeaVM lacks: a key in an encoded form (ADR 0070). */
public abstract class TEncodedKeySpec implements TKeySpec {

    private final byte[] encoded;
    private final @Nullable String algorithm;

    public TEncodedKeySpec(byte[] encodedKey) {
        this.encoded = encodedKey.clone();
        this.algorithm = null;
    }

    protected TEncodedKeySpec(byte[] encodedKey, String algorithm) {
        this.encoded = encodedKey.clone();
        this.algorithm = algorithm;
    }

    public byte[] getEncoded() {
        return encoded.clone();
    }

    public @Nullable String getAlgorithm() {
        return algorithm;
    }

    public abstract String getFormat();
}
