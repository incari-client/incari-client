package incari.classlib.substitutes.java.security;

import incari.classlib.security.RsaPrivateKey;
import incari.classlib.security.RsaPublicKey;
import incari.classlib.substitutes.java.security.spec.TInvalidKeySpecException;
import incari.classlib.substitutes.java.security.spec.TKeySpec;
import incari.classlib.substitutes.java.security.spec.TPKCS8EncodedKeySpec;
import incari.classlib.substitutes.java.security.spec.TX509EncodedKeySpec;
import java.io.IOException;
import java.security.NoSuchAlgorithmException;
import org.bouncycastle.asn1.pkcs.PKCSObjectIdentifiers;
import org.bouncycastle.asn1.pkcs.PrivateKeyInfo;
import org.bouncycastle.asn1.pkcs.RSAPublicKey;
import org.bouncycastle.asn1.x509.SubjectPublicKeyInfo;
import org.bouncycastle.crypto.params.AsymmetricKeyParameter;
import org.bouncycastle.crypto.params.RSAKeyParameters;
import org.bouncycastle.crypto.util.PrivateKeyFactory;
import org.bouncycastle.crypto.util.PublicKeyFactory;

/**
 * {@code java.security.KeyFactory}, which TeaVM lacks, for RSA only (ADR 0070): a server's public
 * key from the login handshake (X.509), and the chat-signing key Mojang issues (PKCS#8), parsed
 * with Bouncy Castle.
 */
public class TKeyFactory {

    /**
     * The largest modulus and public exponent read. A server's key arrives before anything checks
     * it, and reading a key tests its modulus for primality: a hostile server's huge key would
     * freeze the page. The JDK caps exponents at 64 bits too. Minecraft's keys are 1024 (servers),
     * 2048 (chat) and 4096 bits (Mojang's).
     */
    static final int MAX_MODULUS_BITS = 4096;

    static final int MAX_EXPONENT_BITS = 64;

    private TKeyFactory() {}

    public static TKeyFactory getInstance(String algorithm) throws NoSuchAlgorithmException {
        if (!algorithm.equalsIgnoreCase("RSA")) {
            throw new NoSuchAlgorithmException(algorithm + " KeyFactory not available");
        }
        return new TKeyFactory();
    }

    public final String getAlgorithm() {
        return "RSA";
    }

    public final TPublicKey generatePublic(TKeySpec keySpec) throws TInvalidKeySpecException {
        if (!(keySpec instanceof TX509EncodedKeySpec x509)) {
            throw new TInvalidKeySpecException("Only X.509 public keys");
        }
        byte[] encoded = x509.getEncoded();
        try {
            SubjectPublicKeyInfo info = SubjectPublicKeyInfo.getInstance(encoded);
            if (!PKCSObjectIdentifiers.rsaEncryption.equals(info.getAlgorithm().getAlgorithm())) {
                throw new TInvalidKeySpecException("Not an RSA key");
            }
            RSAPublicKey numbers = RSAPublicKey.getInstance(info.parsePublicKey());
            if (numbers.getModulus().bitLength() > MAX_MODULUS_BITS
                    || numbers.getPublicExponent().bitLength() > MAX_EXPONENT_BITS) {
                throw new TInvalidKeySpecException("RSA key too big");
            }
            AsymmetricKeyParameter key = PublicKeyFactory.createKey(info);
            if (!(key instanceof RSAKeyParameters rsa) || rsa.isPrivate()) {
                throw new TInvalidKeySpecException("Not an RSA public key");
            }
            return new RsaPublicKey(rsa, encoded);
        } catch (IOException | RuntimeException e) {
            throw new TInvalidKeySpecException("Can't read the public key", e);
        }
    }

    public final TPrivateKey generatePrivate(TKeySpec keySpec) throws TInvalidKeySpecException {
        if (!(keySpec instanceof TPKCS8EncodedKeySpec pkcs8)) {
            throw new TInvalidKeySpecException("Only PKCS#8 private keys");
        }
        byte[] encoded = pkcs8.getEncoded();
        try {
            PrivateKeyInfo info = PrivateKeyInfo.getInstance(encoded);
            if (!PKCSObjectIdentifiers.rsaEncryption.equals(
                    info.getPrivateKeyAlgorithm().getAlgorithm())) {
                throw new TInvalidKeySpecException("Not an RSA key");
            }
            AsymmetricKeyParameter key = PrivateKeyFactory.createKey(info);
            if (!(key instanceof RSAKeyParameters rsa) || !rsa.isPrivate()) {
                throw new TInvalidKeySpecException("Not an RSA private key");
            }
            return new RsaPrivateKey(rsa, encoded);
        } catch (IOException | RuntimeException e) {
            throw new TInvalidKeySpecException("Can't read the private key", e);
        }
    }
}
