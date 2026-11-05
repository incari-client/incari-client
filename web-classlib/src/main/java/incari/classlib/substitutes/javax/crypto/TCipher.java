package incari.classlib.substitutes.javax.crypto;

import incari.classlib.security.RsaPrivateKey;
import incari.classlib.security.RsaPublicKey;
import incari.classlib.substitutes.java.security.TInvalidAlgorithmParameterException;
import incari.classlib.substitutes.java.security.TInvalidKeyException;
import incari.classlib.substitutes.java.security.TKey;
import incari.classlib.substitutes.java.security.spec.TAlgorithmParameterSpec;
import incari.classlib.substitutes.javax.crypto.spec.TIvParameterSpec;
import java.io.ByteArrayOutputStream;
import java.security.NoSuchAlgorithmException;
import java.security.SecureRandom;
import java.util.Locale;
import org.bouncycastle.crypto.InvalidCipherTextException;
import org.bouncycastle.crypto.encodings.PKCS1Encoding;
import org.bouncycastle.crypto.engines.AESEngine;
import org.bouncycastle.crypto.engines.RSABlindedEngine;
import org.bouncycastle.crypto.modes.CFBBlockCipher;
import org.bouncycastle.crypto.params.KeyParameter;
import org.bouncycastle.crypto.params.ParametersWithIV;
import org.bouncycastle.crypto.params.ParametersWithRandom;
import org.bouncycastle.crypto.params.RSAKeyParameters;
import org.jspecify.annotations.Nullable;

/**
 * {@code javax.crypto.Cipher}, which TeaVM lacks, for the two ciphers the login handshake uses
 * (ADR 0070): RSA with PKCS#1 v1.5 padding (the shared secret, sent under the server's key) and
 * AES/CFB8 (the connection's stream, keyed and IV'd with that secret). Bouncy Castle does the work.
 *
 * <p>Hot path: once a connection is encrypted every byte in and out passes through
 * {@link #update(byte[], int, int, byte[], int)}, so it doesn't allocate.
 */
public class TCipher {

    public static final int ENCRYPT_MODE = 1;
    public static final int DECRYPT_MODE = 2;
    public static final int WRAP_MODE = 3;
    public static final int UNWRAP_MODE = 4;
    public static final int PUBLIC_KEY = 1;
    public static final int PRIVATE_KEY = 2;
    public static final int SECRET_KEY = 3;

    private enum Kind {
        RSA,
        AES_CFB8
    }

    private final String transformation;
    private final Kind kind;
    private @Nullable PKCS1Encoding rsa;
    private @Nullable CFBBlockCipher aes;
    private byte @Nullable [] iv;
    private final ByteArrayOutputStream rsaInput = new ByteArrayOutputStream();

    private TCipher(String transformation, Kind kind) {
        this.transformation = transformation;
        this.kind = kind;
    }

    public static TCipher getInstance(String transformation) throws NoSuchAlgorithmException, TNoSuchPaddingException {
        String normalized = transformation.replace(" ", "").toUpperCase(Locale.ROOT);
        return switch (normalized) {
            case "RSA", "RSA/ECB/PKCS1PADDING", "RSA/NONE/PKCS1PADDING" -> new TCipher(transformation, Kind.RSA);
            case "AES/CFB8/NOPADDING" -> new TCipher(transformation, Kind.AES_CFB8);
            default -> throw new NoSuchAlgorithmException(transformation + " Cipher not available");
        };
    }

    public final String getAlgorithm() {
        return transformation;
    }

    public final int getBlockSize() {
        return kind == Kind.AES_CFB8 ? 16 : 0;
    }

    public final byte @Nullable [] getIV() {
        return iv == null ? null : iv.clone();
    }

    public final void init(int opmode, TKey key) throws TInvalidKeyException {
        if (kind == Kind.AES_CFB8) {
            throw new TInvalidKeyException("AES/CFB8 needs an IV");
        }
        initRsa(opmode, key);
    }

    public final void init(int opmode, TKey key, SecureRandom random) throws TInvalidKeyException {
        init(opmode, key);
    }

    public final void init(int opmode, TKey key, TAlgorithmParameterSpec params)
            throws TInvalidKeyException, TInvalidAlgorithmParameterException {
        if (kind == Kind.RSA) {
            initRsa(opmode, key);
            return;
        }
        if (!(params instanceof TIvParameterSpec ivSpec) || ivSpec.getIV().length != 16) {
            throw new TInvalidAlgorithmParameterException("AES/CFB8 needs a 16-byte IV");
        }
        byte[] keyBytes = key.getEncoded();
        if (!key.getAlgorithm().equalsIgnoreCase("AES")
                || keyBytes == null
                || (keyBytes.length != 16 && keyBytes.length != 24 && keyBytes.length != 32)) {
            throw new TInvalidKeyException("Not an AES key");
        }
        @SuppressWarnings("deprecation") // newInstance() probes for native code the browser lacks.
        CFBBlockCipher cipher = new CFBBlockCipher(new AESEngine(), 8);
        iv = ivSpec.getIV();
        cipher.init(encrypting(opmode), new ParametersWithIV(new KeyParameter(keyBytes), iv));
        aes = cipher;
    }

    private void initRsa(int opmode, TKey key) throws TInvalidKeyException {
        RSAKeyParameters parameters;
        if (key instanceof RsaPublicKey publicKey) {
            parameters = publicKey.parameters();
        } else if (key instanceof RsaPrivateKey privateKey) {
            parameters = privateKey.parameters();
        } else {
            throw new TInvalidKeyException("Not an RSA key");
        }
        boolean encrypting = encrypting(opmode);
        PKCS1Encoding cipher = new PKCS1Encoding(new RSABlindedEngine());
        // Encryption pads with random bytes: the browser's crypto.getRandomValues, through TeaVM.
        cipher.init(encrypting, encrypting ? new ParametersWithRandom(parameters, new SecureRandom()) : parameters);
        rsa = cipher;
        rsaInput.reset();
    }

    private static boolean encrypting(int opmode) {
        return switch (opmode) {
            case ENCRYPT_MODE, WRAP_MODE -> true;
            case DECRYPT_MODE, UNWRAP_MODE -> false;
            default -> throw new IllegalArgumentException("Unknown cipher mode " + opmode);
        };
    }

    public final int getOutputSize(int inputLen) {
        if (kind == Kind.AES_CFB8) {
            return inputLen;
        }
        PKCS1Encoding cipher = rsaCipher();
        return cipher.getOutputBlockSize();
    }

    public final byte[] update(byte[] input) {
        return update(input, 0, input.length);
    }

    public final byte[] update(byte[] input, int inputOffset, int inputLen) {
        if (kind == Kind.RSA) {
            rsaInput.write(input, inputOffset, inputLen);
            return new byte[0];
        }
        byte[] out = new byte[inputLen];
        aesCipher().processBytes(input, inputOffset, inputLen, out, 0);
        return out;
    }

    public final int update(byte[] input, int inputOffset, int inputLen, byte[] output) throws TShortBufferException {
        return update(input, inputOffset, inputLen, output, 0);
    }

    public final int update(byte[] input, int inputOffset, int inputLen, byte[] output, int outputOffset)
            throws TShortBufferException {
        if (kind == Kind.RSA) {
            rsaInput.write(input, inputOffset, inputLen);
            return 0;
        }
        if (output.length - outputOffset < inputLen) {
            throw new TShortBufferException("Output buffer too short");
        }
        return aesCipher().processBytes(input, inputOffset, inputLen, output, outputOffset);
    }

    public final byte[] doFinal() throws TIllegalBlockSizeException, TBadPaddingException {
        return doFinal(new byte[0], 0, 0);
    }

    public final byte[] doFinal(byte[] input) throws TIllegalBlockSizeException, TBadPaddingException {
        return doFinal(input, 0, input.length);
    }

    public final byte[] doFinal(byte[] input, int inputOffset, int inputLen)
            throws TIllegalBlockSizeException, TBadPaddingException {
        if (kind == Kind.AES_CFB8) {
            // CFB8 is a stream: nothing is held back, and it carries on from here.
            return update(input, inputOffset, inputLen);
        }
        PKCS1Encoding cipher = rsaCipher();
        rsaInput.write(input, inputOffset, inputLen);
        byte[] all = rsaInput.toByteArray();
        rsaInput.reset();
        if (all.length > cipher.getInputBlockSize()) {
            throw new TIllegalBlockSizeException("Too much data for RSA");
        }
        try {
            return cipher.processBlock(all, 0, all.length);
        } catch (InvalidCipherTextException e) {
            throw new TBadPaddingException("RSA padding is wrong", e);
        }
    }

    private PKCS1Encoding rsaCipher() {
        PKCS1Encoding cipher = rsa;
        if (cipher == null) {
            throw new IllegalStateException("Cipher not initialized");
        }
        return cipher;
    }

    private CFBBlockCipher aesCipher() {
        CFBBlockCipher cipher = aes;
        if (cipher == null) {
            throw new IllegalStateException("Cipher not initialized");
        }
        return cipher;
    }

    @Override
    public String toString() {
        return "Cipher." + transformation;
    }
}
