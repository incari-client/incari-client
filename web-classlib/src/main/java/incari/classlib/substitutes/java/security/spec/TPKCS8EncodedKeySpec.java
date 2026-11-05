package incari.classlib.substitutes.java.security.spec;

/**
 * {@code java.security.spec.PKCS8EncodedKeySpec}, which TeaVM lacks: a private key as PKCS#8, as
 * Mojang issues the chat-signing key (ADR 0070).
 */
public class TPKCS8EncodedKeySpec extends TEncodedKeySpec {

    public TPKCS8EncodedKeySpec(byte[] encodedKey) {
        super(encodedKey);
    }

    public TPKCS8EncodedKeySpec(byte[] encodedKey, String algorithm) {
        super(encodedKey, algorithm);
    }

    @Override
    public final String getFormat() {
        return "PKCS#8";
    }
}
