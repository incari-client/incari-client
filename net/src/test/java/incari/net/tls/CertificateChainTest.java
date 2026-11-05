package incari.net.tls;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatCode;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import java.math.BigInteger;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.spec.ECGenParameterSpec;
import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.ArrayList;
import java.util.Date;
import java.util.List;
import org.bouncycastle.asn1.ASN1ObjectIdentifier;
import org.bouncycastle.asn1.DERNull;
import org.bouncycastle.asn1.pkcs.PKCSObjectIdentifiers;
import org.bouncycastle.asn1.pkcs.RSAPublicKey;
import org.bouncycastle.asn1.x500.X500Name;
import org.bouncycastle.asn1.x509.AlgorithmIdentifier;
import org.bouncycastle.asn1.x509.BasicConstraints;
import org.bouncycastle.asn1.x509.Certificate;
import org.bouncycastle.asn1.x509.ExtendedKeyUsage;
import org.bouncycastle.asn1.x509.Extension;
import org.bouncycastle.asn1.x509.GeneralName;
import org.bouncycastle.asn1.x509.GeneralNames;
import org.bouncycastle.asn1.x509.KeyPurposeId;
import org.bouncycastle.asn1.x509.KeyUsage;
import org.bouncycastle.asn1.x509.SubjectPublicKeyInfo;
import org.bouncycastle.asn1.x509.Time;
import org.bouncycastle.cert.X509v3CertificateBuilder;
import org.bouncycastle.cert.jcajce.JcaX509v3CertificateBuilder;
import org.bouncycastle.operator.jcajce.JcaContentSignerBuilder;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;

class CertificateChainTest {

    private static final long NOW = Instant.parse("2026-11-04T12:00:00Z").toEpochMilli();

    private static KeyPair rootKey;
    private static KeyPair intermediateKey;
    private static KeyPair leafKey;
    private static KeyPair otherKey;
    private static KeyPair ecKey;
    private static Certificate root;

    @BeforeAll
    static void keys() throws Exception {
        KeyPairGenerator rsa = KeyPairGenerator.getInstance("RSA");
        rsa.initialize(2048);
        rootKey = rsa.generateKeyPair();
        intermediateKey = rsa.generateKeyPair();
        leafKey = rsa.generateKeyPair();
        otherKey = rsa.generateKeyPair();
        KeyPairGenerator ec = KeyPairGenerator.getInstance("EC");
        ec.initialize(new ECGenParameterSpec("secp256r1"));
        ecKey = ec.generateKeyPair();
        root = new Spec("Test Root", "Test Root", rootKey, rootKey).ca(-1).build();
    }

    // --- Accepted ------------------------------------------------------------------------------

    @Test
    void acceptsALeafAndIntermediateUnderThePinnedRoot() {
        assertThatCode(() -> chain().check("api.example.com", standard(), NOW)).doesNotThrowAnyException();
    }

    @Test
    void acceptsAChainThatAlsoCarriesTheRoot() throws Exception {
        List<byte[]> withRoot = new ArrayList<>(standard());
        withRoot.add(root.getEncoded());
        assertThatCode(() -> chain().check("api.example.com", withRoot, NOW)).doesNotThrowAnyException();
    }

    @Test
    void acceptsAnEcdsaIntermediate() throws Exception {
        Certificate intermediate =
                new Spec("EC CA", "Test Root", ecKey, rootKey).ca(0).build();
        Certificate leaf = new Spec("api.example.com", "EC CA", leafKey, ecKey)
                .signedWith("SHA256withECDSA")
                .build();
        assertThatCode(() ->
                        chain().check("api.example.com", List.of(leaf.getEncoded(), intermediate.getEncoded()), NOW))
                .doesNotThrowAnyException();
    }

    // --- Refused -------------------------------------------------------------------------------

    @Test
    void refusesAnotherHost() {
        assertThatThrownBy(() -> chain().check("evil.example.net", standard(), NOW))
                .isInstanceOf(UntrustedCertificateException.class)
                .hasMessageContaining("isn't for evil.example.net");
    }

    @Test
    void refusesAnExpiredLeaf() {
        long later = Instant.parse("2031-01-01T00:00:00Z").toEpochMilli();
        assertThatThrownBy(() -> chain().check("api.example.com", standard(), later))
                .hasMessageContaining("out of date");
    }

    @Test
    void refusesALeafNotYetValid() {
        long earlier = Instant.parse("2020-01-01T00:00:00Z").toEpochMilli();
        assertThatThrownBy(() -> chain().check("api.example.com", standard(), earlier))
                .hasMessageContaining("out of date");
    }

    @Test
    void refusesAChainUnderAnotherRoot() throws Exception {
        Certificate intermediate = new Spec("Test CA", "Other Root", intermediateKey, otherKey)
                .ca(0)
                .build();
        Certificate leaf = new Spec("api.example.com", "Test CA", leafKey, intermediateKey).build();
        assertThatThrownBy(() ->
                        chain().check("api.example.com", List.of(leaf.getEncoded(), intermediate.getEncoded()), NOW))
                .hasMessageContaining("trusted root");
    }

    @Test
    void refusesARootNameWithAnotherKey() throws Exception {
        // Claims the pinned root as issuer, but signed by a different key.
        Certificate intermediate = new Spec("Test CA", "Test Root", intermediateKey, otherKey)
                .ca(0)
                .build();
        Certificate leaf = new Spec("api.example.com", "Test CA", leafKey, intermediateKey).build();
        assertThatThrownBy(() ->
                        chain().check("api.example.com", List.of(leaf.getEncoded(), intermediate.getEncoded()), NOW))
                .hasMessageContaining("trusted root");
    }

    @Test
    void refusesALeafSignedByANonCa() throws Exception {
        Certificate notCa = new Spec("Not A CA", "Test Root", intermediateKey, rootKey).build();
        Certificate leaf = new Spec("api.example.com", "Not A CA", leafKey, intermediateKey).build();
        assertThatThrownBy(() -> chain().check("api.example.com", List.of(leaf.getEncoded(), notCa.getEncoded()), NOW))
                .hasMessageContaining("non-CA");
    }

    @Test
    void refusesACaWithoutCertificateSigning() throws Exception {
        Certificate intermediate = new Spec("Test CA", "Test Root", intermediateKey, rootKey)
                .ca(0)
                .keyUsage(KeyUsage.digitalSignature)
                .build();
        Certificate leaf = new Spec("api.example.com", "Test CA", leafKey, intermediateKey).build();
        assertThatThrownBy(() ->
                        chain().check("api.example.com", List.of(leaf.getEncoded(), intermediate.getEncoded()), NOW))
                .hasMessageContaining("not for that");
    }

    @Test
    void refusesAPathLongerThanItsCaAllows() throws Exception {
        KeyPair second = otherKey;
        Certificate top =
                new Spec("Top CA", "Test Root", intermediateKey, rootKey).ca(0).build();
        Certificate middle =
                new Spec("Middle CA", "Top CA", second, intermediateKey).ca(-1).build();
        Certificate leaf = new Spec("api.example.com", "Middle CA", leafKey, second).build();
        assertThatThrownBy(() -> chain().check(
                                "api.example.com",
                                List.of(leaf.getEncoded(), middle.getEncoded(), top.getEncoded()),
                                NOW))
                .hasMessageContaining("longer than its CA allows");
    }

    @Test
    void refusesATamperedSignature() throws Exception {
        byte[] leaf = standard().get(0);
        leaf[leaf.length - 5] ^= 0x01;
        List<byte[]> tampered = List.of(leaf, standard().get(1));
        assertThatThrownBy(() -> chain().check("api.example.com", tampered, NOW))
                .isInstanceOf(UntrustedCertificateException.class);
    }

    @Test
    void refusesALeafForClientsOnly() throws Exception {
        Certificate intermediate = intermediate();
        Certificate leaf = new Spec("api.example.com", "Test CA", leafKey, intermediateKey)
                .usage(KeyPurposeId.id_kp_clientAuth)
                .build();
        assertThatThrownBy(() ->
                        chain().check("api.example.com", List.of(leaf.getEncoded(), intermediate.getEncoded()), NOW))
                .hasMessageContaining("isn't for servers");
    }

    @Test
    void refusesALeafThatNamesNoPurpose() throws Exception {
        Certificate intermediate = intermediate();
        Certificate leaf = new Spec("api.example.com", "Test CA", leafKey, intermediateKey)
                .noUsage()
                .build();
        assertThatThrownBy(() ->
                        chain().check("api.example.com", List.of(leaf.getEncoded(), intermediate.getEncoded()), NOW))
                .hasMessageContaining("isn't for servers");
    }

    @Test
    void refusesACaLimitedToCodeSigning() throws Exception {
        Certificate intermediate = new Spec("Test CA", "Test Root", intermediateKey, rootKey)
                .ca(0)
                .caUsage(KeyPurposeId.id_kp_codeSigning)
                .build();
        Certificate leaf = new Spec("api.example.com", "Test CA", leafKey, intermediateKey).build();
        assertThatThrownBy(() ->
                        chain().check("api.example.com", List.of(leaf.getEncoded(), intermediate.getEncoded()), NOW))
                .hasMessageContaining("CA in the server's chain isn't for servers");
    }

    @Test
    void refusesHugeRsaKeysBeforeDoingTheirMath() throws Exception {
        Certificate leaf = new Spec("api.example.com", "Test CA", leafKey, intermediateKey).build();
        AlgorithmIdentifier rsa = new AlgorithmIdentifier(PKCSObjectIdentifiers.rsaEncryption, DERNull.INSTANCE);
        BigInteger huge = BigInteger.ONE.shiftLeft(16383).add(BigInteger.ONE);
        SubjectPublicKeyInfo bigModulus =
                new SubjectPublicKeyInfo(rsa, new RSAPublicKey(huge, BigInteger.valueOf(65537)));
        assertThatThrownBy(() -> CertificateChain.verifies(leaf, bigModulus)).hasMessageContaining("too big");
        BigInteger modulus = BigInteger.ONE.shiftLeft(2047).add(BigInteger.ONE);
        SubjectPublicKeyInfo bigExponent = new SubjectPublicKeyInfo(rsa, new RSAPublicKey(modulus, huge));
        assertThatThrownBy(() -> CertificateChain.verifies(leaf, bigExponent)).hasMessageContaining("too big");
    }

    @Test
    void refusesAnUnknownCriticalExtension() throws Exception {
        Certificate intermediate = intermediate();
        Certificate leaf = new Spec("api.example.com", "Test CA", leafKey, intermediateKey)
                .critical(new ASN1ObjectIdentifier("1.2.3.4.5"))
                .build();
        assertThatThrownBy(() ->
                        chain().check("api.example.com", List.of(leaf.getEncoded(), intermediate.getEncoded()), NOW))
                .hasMessageContaining("unknown critical");
    }

    @Test
    void refusesSha1Signatures() throws Exception {
        Certificate intermediate = intermediate();
        Certificate leaf = new Spec("api.example.com", "Test CA", leafKey, intermediateKey)
                .signedWith("SHA1withRSA")
                .build();
        assertThatThrownBy(() ->
                        chain().check("api.example.com", List.of(leaf.getEncoded(), intermediate.getEncoded()), NOW))
                .hasMessageContaining("unsupported signature");
    }

    @Test
    void refusesAChainOutOfOrder() {
        List<byte[]> swapped = List.of(standard().get(1), standard().get(0));
        assertThatThrownBy(() -> chain().check("api.example.com", swapped, NOW))
                .isInstanceOf(UntrustedCertificateException.class);
    }

    @Test
    void refusesNothingAndRubbish() {
        assertThatThrownBy(() -> chain().check("api.example.com", List.of(), NOW))
                .hasMessageContaining("no certificate");
        assertThatThrownBy(() -> chain().check("api.example.com", List.of(new byte[] {1, 2, 3}), NOW))
                .hasMessageContaining("can't be read");
    }

    @Test
    void refusesOverlongChains() {
        List<byte[]> many = new ArrayList<>();
        for (int i = 0; i <= CertificateChain.MAX_LENGTH; i++) {
            many.add(standard().get(0));
        }
        assertThatThrownBy(() -> chain().check("api.example.com", many, NOW)).hasMessageContaining("too long");
    }

    // --- Names and dates -----------------------------------------------------------------------

    @Test
    void wildcardsCoverOneWholeLabel() {
        assertThat(CertificateChain.matches("*.mojang.com", "sessionserver.mojang.com"))
                .isTrue();
        assertThat(CertificateChain.matches("*.mojang.com", "mojang.com")).isFalse();
        assertThat(CertificateChain.matches("*.mojang.com", "a.b.mojang.com")).isFalse();
        assertThat(CertificateChain.matches("*.mojang.com", "evilmojang.com")).isFalse();
        assertThat(CertificateChain.matches("*.com", "mojang.com")).isFalse();
        assertThat(CertificateChain.matches("s*.mojang.com", "sessionserver.mojang.com"))
                .isFalse();
        assertThat(CertificateChain.matches("*.*.com", "a.b.com")).isFalse();
        assertThat(CertificateChain.matches("login.live.com", "login.live.com")).isTrue();
        assertThat(CertificateChain.matches("login.live.com", "xlogin.live.com"))
                .isFalse();
    }

    @Test
    void readsBothDerTimeForms() throws Exception {
        assertThat(CertificateChain.epochMillis(new Time(new org.bouncycastle.asn1.DERUTCTime("491231235959Z"))))
                .isEqualTo(Instant.parse("2049-12-31T23:59:59Z").toEpochMilli());
        assertThat(CertificateChain.epochMillis(new Time(new org.bouncycastle.asn1.DERUTCTime("500101000000Z"))))
                .isEqualTo(Instant.parse("1950-01-01T00:00:00Z").toEpochMilli());
        assertThat(CertificateChain.epochMillis(
                        new Time(new org.bouncycastle.asn1.DERGeneralizedTime("20500101000000Z"))))
                .isEqualTo(Instant.parse("2050-01-01T00:00:00Z").toEpochMilli());
        assertThat(CertificateChain.daysFromCivil(2024, 2, 29)).isEqualTo(19782);
    }

    @Test
    void thePinnedRootsParse() {
        assertThat(TrustAnchors.roots()).hasSize(4);
    }

    // --- Helpers -------------------------------------------------------------------------------

    private static CertificateChain chain() {
        return new CertificateChain(List.of(root));
    }

    private static Certificate intermediate() throws Exception {
        return new Spec("Test CA", "Test Root", intermediateKey, rootKey).ca(0).build();
    }

    private static List<byte[]> standard() {
        try {
            Certificate leaf = new Spec("api.example.com", "Test CA", leafKey, intermediateKey).build();
            return List.of(leaf.getEncoded(), intermediate().getEncoded());
        } catch (Exception e) {
            throw new AssertionError(e);
        }
    }

    /** A certificate to build: subject, issuer, its key, the issuer's key, and the parts tests vary. */
    private static final class Spec {
        private final String subject;
        private final String issuer;
        private final KeyPair key;
        private final KeyPair issuerKey;
        private int pathLength = -2; // -2: not a CA
        private int keyUsage = -1;
        private KeyPurposeId usage = KeyPurposeId.id_kp_serverAuth;
        private boolean noUsage;
        private KeyPurposeId caUsage;
        private ASN1ObjectIdentifier critical;
        private String algorithm = "SHA256withRSA";

        Spec(String subject, String issuer, KeyPair key, KeyPair issuerKey) {
            this.subject = subject;
            this.issuer = issuer;
            this.key = key;
            this.issuerKey = issuerKey;
        }

        /** A CA, with a path length or -1 for none. */
        Spec ca(int pathLength) {
            this.pathLength = pathLength;
            return this;
        }

        Spec keyUsage(int keyUsage) {
            this.keyUsage = keyUsage;
            return this;
        }

        Spec usage(KeyPurposeId usage) {
            this.usage = usage;
            return this;
        }

        Spec noUsage() {
            this.noUsage = true;
            return this;
        }

        Spec caUsage(KeyPurposeId usage) {
            this.caUsage = usage;
            return this;
        }

        Spec critical(ASN1ObjectIdentifier oid) {
            this.critical = oid;
            return this;
        }

        Spec signedWith(String algorithm) {
            this.algorithm = algorithm;
            return this;
        }

        Certificate build() throws Exception {
            Date from = Date.from(Instant.parse("2026-01-01T00:00:00Z"));
            Date to = Date.from(Instant.parse("2026-01-01T00:00:00Z").plus(1500, ChronoUnit.DAYS));
            X509v3CertificateBuilder builder = new JcaX509v3CertificateBuilder(
                    new X500Name("CN=" + issuer),
                    BigInteger.valueOf(System.nanoTime()),
                    from,
                    to,
                    new X500Name("CN=" + subject),
                    key.getPublic());
            if (pathLength >= -1) {
                builder.addExtension(
                        Extension.basicConstraints,
                        true,
                        pathLength >= 0 ? new BasicConstraints(pathLength) : new BasicConstraints(true));
                builder.addExtension(
                        Extension.keyUsage,
                        true,
                        new KeyUsage(keyUsage >= 0 ? keyUsage : KeyUsage.keyCertSign | KeyUsage.cRLSign));
                if (caUsage != null) {
                    builder.addExtension(Extension.extendedKeyUsage, false, new ExtendedKeyUsage(caUsage));
                }
            } else {
                builder.addExtension(
                        Extension.subjectAlternativeName,
                        false,
                        new GeneralNames(new GeneralName(GeneralName.dNSName, subject)));
                if (!noUsage) {
                    builder.addExtension(Extension.extendedKeyUsage, false, new ExtendedKeyUsage(usage));
                }
            }
            if (critical != null) {
                builder.addExtension(critical, true, DERNull.INSTANCE);
            }
            String signing = issuerKey.getPrivate().getAlgorithm().equals("EC") ? "SHA256withECDSA" : algorithm;
            return builder.build(new JcaContentSignerBuilder(signing).build(issuerKey.getPrivate()))
                    .toASN1Structure();
        }
    }
}
