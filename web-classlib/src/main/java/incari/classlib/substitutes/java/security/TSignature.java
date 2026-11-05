package incari.classlib.substitutes.java.security;

import incari.classlib.security.RsaPrivateKey;
import incari.classlib.security.RsaPublicKey;
import java.nio.ByteBuffer;
import java.security.NoSuchAlgorithmException;
import java.util.Locale;
import org.bouncycastle.crypto.CryptoException;
import org.bouncycastle.crypto.Digest;
import org.bouncycastle.crypto.digests.SHA1Digest;
import org.bouncycastle.crypto.digests.SHA256Digest;
import org.bouncycastle.crypto.signers.RSADigestSigner;
import org.jspecify.annotations.Nullable;

/**
 * {@code java.security.Signature}, which TeaVM lacks, for PKCS#1 v1.5 RSA with SHA-256 (chat
 * signing and its checks) and SHA-1 (authlib's profile properties), over Bouncy Castle's
 * {@code RSADigestSigner} (ADR 0070). Only what the game and authlib call is here.
 */
public class TSignature {

    private static final int UNINITIALIZED = 0;
    private static final int SIGN = 2;
    private static final int VERIFY = 3;

    private final String algorithm;
    private final RSADigestSigner signer;
    private int state = UNINITIALIZED;

    private TSignature(String algorithm, Digest digest) {
        this.algorithm = algorithm;
        this.signer = new RSADigestSigner(digest);
    }

    public static TSignature getInstance(String algorithm) throws NoSuchAlgorithmException {
        Digest digest = digest(algorithm);
        if (digest == null) {
            throw new NoSuchAlgorithmException(algorithm + " Signature not available");
        }
        return new TSignature(algorithm, digest);
    }

    private static @Nullable Digest digest(String algorithm) {
        return switch (algorithm.toUpperCase(Locale.ROOT)) {
            case "SHA256WITHRSA" -> new SHA256Digest();
            case "SHA1WITHRSA" -> new SHA1Digest();
            default -> null;
        };
    }

    public final String getAlgorithm() {
        return algorithm;
    }

    public final void initSign(TPrivateKey privateKey) throws TInvalidKeyException {
        if (!(privateKey instanceof RsaPrivateKey rsa)) {
            throw new TInvalidKeyException("Not an RSA private key");
        }
        signer.init(true, rsa.parameters());
        state = SIGN;
    }

    public final void initVerify(TPublicKey publicKey) throws TInvalidKeyException {
        if (!(publicKey instanceof RsaPublicKey rsa)) {
            throw new TInvalidKeyException("Not an RSA public key");
        }
        signer.init(false, rsa.parameters());
        state = VERIFY;
    }

    public final void update(byte b) throws TSignatureException {
        check();
        signer.update(b);
    }

    public final void update(byte[] data) throws TSignatureException {
        update(data, 0, data.length);
    }

    public final void update(byte[] data, int off, int len) throws TSignatureException {
        check();
        signer.update(data, off, len);
    }

    public final void update(ByteBuffer data) throws TSignatureException {
        check();
        byte[] bytes = new byte[data.remaining()];
        data.get(bytes);
        signer.update(bytes, 0, bytes.length);
    }

    public final byte[] sign() throws TSignatureException {
        if (state != SIGN) {
            throw new TSignatureException("Not initialized for signing");
        }
        try {
            return signer.generateSignature();
        } catch (CryptoException | RuntimeException e) {
            throw new TSignatureException("Signing failed", e);
        } finally {
            signer.reset();
        }
    }

    public final boolean verify(byte[] signature) throws TSignatureException {
        if (state != VERIFY) {
            throw new TSignatureException("Not initialized for verifying");
        }
        try {
            return signer.verifySignature(signature);
        } finally {
            signer.reset();
        }
    }

    private void check() throws TSignatureException {
        if (state == UNINITIALIZED) {
            throw new TSignatureException("Not initialized");
        }
    }

    @Override
    public String toString() {
        return "Signature object: " + algorithm;
    }
}
