package incari.classlib.substitutes.javax.crypto;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import incari.classlib.substitutes.java.security.TInvalidKeyException;
import incari.classlib.substitutes.java.security.TKeyFactory;
import incari.classlib.substitutes.java.security.TPrivateKey;
import incari.classlib.substitutes.java.security.TPublicKey;
import incari.classlib.substitutes.java.security.TSignature;
import incari.classlib.substitutes.java.security.spec.TInvalidKeySpecException;
import incari.classlib.substitutes.java.security.spec.TPKCS8EncodedKeySpec;
import incari.classlib.substitutes.java.security.spec.TX509EncodedKeySpec;
import incari.classlib.substitutes.javax.crypto.spec.TIvParameterSpec;
import incari.classlib.substitutes.javax.crypto.spec.TSecretKeySpec;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.NoSuchAlgorithmException;
import java.security.Signature;
import java.util.Random;
import javax.crypto.Cipher;
import javax.crypto.spec.IvParameterSpec;
import javax.crypto.spec.SecretKeySpec;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;

/**
 * The browser build's Cipher, KeyFactory, Signature and KeyGenerator against the JDK's, as the login
 * handshake and chat signing use them (ADR 0070).
 */
class CryptoTest {

    private static KeyPair serverKey; // 1024-bit, as Minecraft servers make theirs
    private static KeyPair chatKey; // 2048-bit, as Mojang issues chat keys

    @BeforeAll
    static void keys() throws Exception {
        KeyPairGenerator generator = KeyPairGenerator.getInstance("RSA");
        generator.initialize(1024);
        serverKey = generator.generateKeyPair();
        generator.initialize(2048);
        chatKey = generator.generateKeyPair();
    }

    // --- AES/CFB8, the connection's stream -----------------------------------------------------

    @Test
    void aesCfb8MatchesTheJdkPieceByPiece() throws Exception {
        Random random = new Random(7);
        byte[] key = new byte[16];
        random.nextBytes(key);
        byte[] data = new byte[5000];
        random.nextBytes(data);

        TCipher ours = TCipher.getInstance("AES/CFB8/NoPadding");
        ours.init(TCipher.ENCRYPT_MODE, new TSecretKeySpec(key, "AES"), new TIvParameterSpec(key));
        Cipher jdk = Cipher.getInstance("AES/CFB8/NoPadding");
        jdk.init(Cipher.ENCRYPT_MODE, new SecretKeySpec(key, "AES"), new IvParameterSpec(key));

        byte[] oursOut = new byte[data.length];
        int at = 0;
        while (at < data.length) {
            int n = Math.min(data.length - at, 1 + random.nextInt(700));
            assertThat(ours.getOutputSize(n)).isEqualTo(n);
            assertThat(ours.update(data, at, n, oursOut, at)).isEqualTo(n);
            at += n;
        }
        assertThat(oursOut).isEqualTo(jdk.doFinal(data));

        TCipher back = TCipher.getInstance("AES/CFB8/NoPadding");
        back.init(TCipher.DECRYPT_MODE, new TSecretKeySpec(key, "AES"), new TIvParameterSpec(key));
        byte[] plain = new byte[data.length];
        back.update(oursOut, 0, oursOut.length, plain);
        assertThat(plain).isEqualTo(data);
    }

    @Test
    void aesNeedsAKeyAndIv() throws Exception {
        TCipher cipher = TCipher.getInstance("AES/CFB8/NoPadding");
        assertThatThrownBy(() -> cipher.init(TCipher.ENCRYPT_MODE, new TSecretKeySpec(new byte[16], "AES")))
                .isInstanceOf(TInvalidKeyException.class);
        assertThatThrownBy(() -> cipher.init(
                        TCipher.ENCRYPT_MODE,
                        new TSecretKeySpec(new byte[5], "AES"),
                        new TIvParameterSpec(new byte[16])))
                .isInstanceOf(TInvalidKeyException.class);
        cipher.init(TCipher.ENCRYPT_MODE, new TSecretKeySpec(new byte[16], "AES"), new TIvParameterSpec(new byte[16]));
        assertThatThrownBy(() -> cipher.update(new byte[4], 0, 4, new byte[3], 0))
                .isInstanceOf(TShortBufferException.class);
    }

    // --- RSA, the shared secret ----------------------------------------------------------------

    @Test
    void encryptsTheSharedSecretForTheServer() throws Exception {
        TPublicKey server = TKeyFactory.getInstance("RSA")
                .generatePublic(new TX509EncodedKeySpec(serverKey.getPublic().getEncoded()));
        assertThat(server.getEncoded()).isEqualTo(serverKey.getPublic().getEncoded());
        assertThat(server.getAlgorithm()).isEqualTo("RSA");

        byte[] secret = new byte[16];
        new Random(3).nextBytes(secret);
        TCipher ours = TCipher.getInstance(server.getAlgorithm());
        ours.init(TCipher.ENCRYPT_MODE, server);
        byte[] encrypted = ours.doFinal(secret);

        Cipher jdk = Cipher.getInstance("RSA");
        jdk.init(Cipher.DECRYPT_MODE, serverKey.getPrivate());
        assertThat(jdk.doFinal(encrypted)).isEqualTo(secret);
    }

    @Test
    void decryptsWhatTheJdkEncrypted() throws Exception {
        Cipher jdk = Cipher.getInstance("RSA");
        jdk.init(Cipher.ENCRYPT_MODE, serverKey.getPublic());
        byte[] encrypted = jdk.doFinal(new byte[] {1, 2, 3, 4});

        TPrivateKey key = TKeyFactory.getInstance("RSA")
                .generatePrivate(new TPKCS8EncodedKeySpec(serverKey.getPrivate().getEncoded()));
        TCipher ours = TCipher.getInstance("RSA");
        ours.init(TCipher.DECRYPT_MODE, key);
        assertThat(ours.doFinal(encrypted)).containsExactly(1, 2, 3, 4);

        encrypted[10] ^= 1;
        ours.init(TCipher.DECRYPT_MODE, key);
        assertThatThrownBy(() -> ours.doFinal(encrypted)).isInstanceOf(TBadPaddingException.class);
    }

    @Test
    void refusesKeysThatAreNotRsa() throws Exception {
        TKeyFactory factory = TKeyFactory.getInstance("RSA");
        assertThatThrownBy(() -> factory.generatePublic(new TX509EncodedKeySpec(new byte[] {1, 2, 3})))
                .isInstanceOf(TInvalidKeySpecException.class);
        KeyPairGenerator ec = KeyPairGenerator.getInstance("EC");
        ec.initialize(256);
        byte[] ecKey = ec.generateKeyPair().getPublic().getEncoded();
        assertThatThrownBy(() -> factory.generatePublic(new TX509EncodedKeySpec(ecKey)))
                .isInstanceOf(TInvalidKeySpecException.class);
        assertThatThrownBy(() -> TKeyFactory.getInstance("EC")).isInstanceOf(NoSuchAlgorithmException.class);
        // A hostile server's oversized key is refused before its modulus is tested for primality.
        java.math.BigInteger huge = java.math.BigInteger.ONE.shiftLeft(16383).add(java.math.BigInteger.ONE);
        byte[] oversized = new org.bouncycastle.asn1.x509.SubjectPublicKeyInfo(
                        new org.bouncycastle.asn1.x509.AlgorithmIdentifier(
                                org.bouncycastle.asn1.pkcs.PKCSObjectIdentifiers.rsaEncryption,
                                org.bouncycastle.asn1.DERNull.INSTANCE),
                        new org.bouncycastle.asn1.pkcs.RSAPublicKey(huge, java.math.BigInteger.valueOf(65537)))
                .getEncoded();
        assertThatThrownBy(() -> factory.generatePublic(new TX509EncodedKeySpec(oversized)))
                .isInstanceOf(TInvalidKeySpecException.class);
        assertThatThrownBy(() -> TCipher.getInstance("AES/CBC/PKCS5Padding"))
                .isInstanceOf(NoSuchAlgorithmException.class);
    }

    // --- Signatures, chat ----------------------------------------------------------------------

    @Test
    void signsChatAsTheJdkVerifies() throws Exception {
        TPrivateKey key = TKeyFactory.getInstance("RSA")
                .generatePrivate(new TPKCS8EncodedKeySpec(chatKey.getPrivate().getEncoded()));
        byte[] message = "hello, world".getBytes(java.nio.charset.StandardCharsets.UTF_8);

        TSignature ours = TSignature.getInstance("SHA256withRSA");
        ours.initSign(key);
        ours.update(message, 0, 5);
        ours.update(message, 5, message.length - 5);
        byte[] signature = ours.sign();

        Signature jdk = Signature.getInstance("SHA256withRSA");
        jdk.initVerify(chatKey.getPublic());
        jdk.update(message);
        assertThat(jdk.verify(signature)).isTrue();
    }

    @Test
    void verifiesWhatTheJdkSigned() throws Exception {
        byte[] message = {9, 8, 7};
        for (String algorithm : new String[] {"SHA256withRSA", "SHA1withRSA"}) {
            Signature jdk = Signature.getInstance(algorithm);
            jdk.initSign(chatKey.getPrivate());
            jdk.update(message);
            byte[] signature = jdk.sign();

            TPublicKey key = TKeyFactory.getInstance("RSA")
                    .generatePublic(new TX509EncodedKeySpec(chatKey.getPublic().getEncoded()));
            TSignature ours = TSignature.getInstance(algorithm);
            ours.initVerify(key);
            ours.update(message);
            assertThat(ours.verify(signature)).as(algorithm).isTrue();

            ours.initVerify(key);
            ours.update(new byte[] {9, 8, 6});
            assertThat(ours.verify(signature)).as(algorithm).isFalse();
        }
    }

    // --- Keys ----------------------------------------------------------------------------------

    @Test
    void generatesFreshAesKeys() throws Exception {
        TKeyGenerator generator = TKeyGenerator.getInstance("AES");
        generator.init(128);
        TSecretKey a = generator.generateKey();
        TSecretKey b = generator.generateKey();
        assertThat(a.getEncoded()).hasSize(16);
        assertThat(a.getAlgorithm()).isEqualTo("AES");
        assertThat(a.getFormat()).isEqualTo("RAW");
        assertThat(a.getEncoded()).isNotEqualTo(b.getEncoded());
    }
}
