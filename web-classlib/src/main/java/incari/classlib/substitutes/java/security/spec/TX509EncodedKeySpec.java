package incari.classlib.substitutes.java.security.spec;

/**
 * {@code java.security.spec.X509EncodedKeySpec}, which TeaVM lacks: a public key as an X.509
 * SubjectPublicKeyInfo, as servers send theirs in the login handshake (ADR 0070).
 */
public class TX509EncodedKeySpec extends TEncodedKeySpec {

    public TX509EncodedKeySpec(byte[] encodedKey) {
        super(encodedKey);
    }

    public TX509EncodedKeySpec(byte[] encodedKey, String algorithm) {
        super(encodedKey, algorithm);
    }

    @Override
    public final String getFormat() {
        return "X.509";
    }
}
