package incari.net.tls;

import java.io.IOException;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import org.bouncycastle.asn1.ASN1Encoding;
import org.bouncycastle.asn1.ASN1ObjectIdentifier;
import org.bouncycastle.asn1.ASN1String;
import org.bouncycastle.asn1.pkcs.PKCSObjectIdentifiers;
import org.bouncycastle.asn1.pkcs.RSAPublicKey;
import org.bouncycastle.asn1.sec.SECObjectIdentifiers;
import org.bouncycastle.asn1.x509.AlgorithmIdentifier;
import org.bouncycastle.asn1.x509.BasicConstraints;
import org.bouncycastle.asn1.x509.Certificate;
import org.bouncycastle.asn1.x509.ExtendedKeyUsage;
import org.bouncycastle.asn1.x509.Extension;
import org.bouncycastle.asn1.x509.Extensions;
import org.bouncycastle.asn1.x509.GeneralName;
import org.bouncycastle.asn1.x509.GeneralNames;
import org.bouncycastle.asn1.x509.KeyPurposeId;
import org.bouncycastle.asn1.x509.KeyUsage;
import org.bouncycastle.asn1.x509.SubjectPublicKeyInfo;
import org.bouncycastle.asn1.x509.Time;
import org.bouncycastle.asn1.x9.X9ObjectIdentifiers;
import org.bouncycastle.crypto.Digest;
import org.bouncycastle.crypto.Signer;
import org.bouncycastle.crypto.digests.SHA256Digest;
import org.bouncycastle.crypto.digests.SHA384Digest;
import org.bouncycastle.crypto.digests.SHA512Digest;
import org.bouncycastle.crypto.params.AsymmetricKeyParameter;
import org.bouncycastle.crypto.params.ECPublicKeyParameters;
import org.bouncycastle.crypto.params.RSAKeyParameters;
import org.bouncycastle.crypto.signers.DSADigestSigner;
import org.bouncycastle.crypto.signers.ECDSASigner;
import org.bouncycastle.crypto.signers.RSADigestSigner;
import org.bouncycastle.crypto.util.PublicKeyFactory;
import org.jspecify.annotations.Nullable;

/**
 * Checks a server's certificate chain the way RFC 5280 path validation would for the few hosts
 * Microsoft sign-in talks to: the leaf names the host and is for server authentication, every
 * certificate is in date, each one is signed by the next with a key that's a CA's, and the chain
 * ends in a certificate signed by one of the pinned roots ({@link TrustAnchors}).
 *
 * <p>Deliberately narrow, since it only has to accept Microsoft's and Mojang's chains: RSA (2048
 * bits or more) and ECDSA on P-256 or P-384, with SHA-256, -384 or -512. No SHA-1, no RSA-PSS
 * certificate signatures, no name constraints (a critical one fails the chain), no revocation
 * checks (browsers soft-fail those too). Chains are taken in the order the server sends them.
 */
public final class CertificateChain {

    /** Longer chains than this aren't what these hosts send; refusing them bounds the work. */
    static final int MAX_LENGTH = 6;

    private static final int MIN_RSA_BITS = 2048;

    /**
     * The largest RSA modulus checked, and the largest public exponent (bits). Keys are read before
     * anything is authenticated, and reading one tests its modulus for primality and a signature
     * check raises to its exponent: a hostile proxy's 16384-bit key with a huge exponent costs
     * seconds of CPU per certificate. Real CAs use 2048 to 4096 bits and 65537.
     */
    static final int MAX_RSA_BITS = 4096;

    static final int MAX_RSA_EXPONENT_BITS = 32;

    /** Critical extensions this check understands; any other critical one fails the chain. */
    private static final Set<ASN1ObjectIdentifier> UNDERSTOOD = Set.of(
            Extension.basicConstraints,
            Extension.keyUsage,
            Extension.extendedKeyUsage,
            Extension.subjectAlternativeName);

    private final List<Certificate> anchors;

    CertificateChain(List<Certificate> anchors) {
        this.anchors = List.copyOf(anchors);
    }

    /** A check against the pinned roots. */
    public static CertificateChain pinned() {
        return new CertificateChain(TrustAnchors.roots());
    }

    /**
     * Checks {@code chain} (DER certificates, leaf first, as the server sent them) for {@code host}
     * at {@code nowMillis}.
     *
     * @throws UntrustedCertificateException saying what's wrong
     */
    public void check(String host, List<byte[]> chain, long nowMillis) throws UntrustedCertificateException {
        if (chain.isEmpty()) {
            throw new UntrustedCertificateException("The server sent no certificate");
        }
        if (chain.size() > MAX_LENGTH) {
            throw new UntrustedCertificateException("The server's certificate chain is too long");
        }
        List<Certificate> certificates = new ArrayList<>(chain.size());
        for (byte[] encoded : chain) {
            certificates.add(parse(encoded));
        }

        Certificate leaf = certificates.get(0);
        if (!namesHost(leaf, host)) {
            throw new UntrustedCertificateException("The server's certificate isn't for " + host);
        }
        // Every public TLS certificate names server authentication; one that doesn't isn't for this.
        Extensions leafExtensions = leaf.getTBSCertificate().getExtensions();
        ExtendedKeyUsage usage = leafExtensions == null ? null : ExtendedKeyUsage.fromExtensions(leafExtensions);
        if (usage == null || !usage.hasKeyPurposeId(KeyPurposeId.id_kp_serverAuth)) {
            throw new UntrustedCertificateException("The server's certificate isn't for servers");
        }

        for (int i = 0; i < certificates.size(); i++) {
            Certificate certificate = certificates.get(i);
            checkDates(certificate, nowMillis);
            checkCriticalExtensions(certificate);
            if (signedByAnchor(certificate)) {
                return;
            }
            if (i + 1 == certificates.size()) {
                break;
            }
            Certificate issuer = certificates.get(i + 1);
            if (!certificate.getIssuer().equals(issuer.getSubject())) {
                throw new UntrustedCertificateException("The server's certificate chain is out of order");
            }
            // i intermediates sit below the issuer at i + 1 (the leaf isn't counted).
            checkIssuer(issuer, i);
            if (!verifies(certificate, issuer.getSubjectPublicKeyInfo())) {
                throw new UntrustedCertificateException("A signature in the server's certificate chain is wrong");
            }
        }
        throw new UntrustedCertificateException("The server's certificate isn't issued by a trusted root");
    }

    private static Certificate parse(byte[] encoded) throws UntrustedCertificateException {
        try {
            return Certificate.getInstance(encoded);
        } catch (RuntimeException e) {
            throw new UntrustedCertificateException("The server's certificate can't be read", e);
        }
    }

    // --- Names ---------------------------------------------------------------------------------

    /**
     * Whether a DNS name in the certificate's subject alternative names matches {@code host}. The
     * common name isn't consulted (RFC 6125; public CAs always fill in the alternative names).
     */
    static boolean namesHost(Certificate certificate, String host) {
        Extensions extensions = certificate.getTBSCertificate().getExtensions();
        if (extensions == null) {
            return false;
        }
        GeneralNames names;
        try {
            names = GeneralNames.fromExtensions(extensions, Extension.subjectAlternativeName);
        } catch (RuntimeException e) {
            return false;
        }
        if (names == null) {
            return false;
        }
        String wanted = normalize(host);
        for (GeneralName name : names.getNames()) {
            if (name.getTagNo() == GeneralName.dNSName
                    && name.getName() instanceof ASN1String dns
                    && matches(normalize(dns.getString()), wanted)) {
                return true;
            }
        }
        return false;
    }

    /**
     * Whether a certificate's DNS name matches a host: exactly, or as a wildcard for the whole
     * leftmost label ({@code *.mojang.com} matches {@code sessionserver.mojang.com}, not
     * {@code mojang.com} or {@code a.b.mojang.com}), and never a wildcard over fewer than two labels.
     */
    static boolean matches(String pattern, String host) {
        if (pattern.isEmpty() || host.isEmpty()) {
            return false;
        }
        if (!pattern.startsWith("*.")) {
            return pattern.indexOf('*') < 0 && pattern.equals(host);
        }
        String suffix = pattern.substring(1); // ".mojang.com"
        if (suffix.indexOf('*') >= 0 || suffix.indexOf('.', 1) < 0) {
            return false;
        }
        if (!host.endsWith(suffix)) {
            return false;
        }
        String label = host.substring(0, host.length() - suffix.length());
        return !label.isEmpty() && label.indexOf('.') < 0;
    }

    private static String normalize(String name) {
        String lower = name.toLowerCase(Locale.ROOT);
        return lower.endsWith(".") ? lower.substring(0, lower.length() - 1) : lower;
    }

    // --- Each certificate ----------------------------------------------------------------------

    private static void checkDates(Certificate certificate, long nowMillis) throws UntrustedCertificateException {
        long notBefore = epochMillis(certificate.getStartDate());
        long notAfter = epochMillis(certificate.getEndDate());
        if (nowMillis < notBefore || nowMillis > notAfter) {
            throw new UntrustedCertificateException(
                    "A certificate in the server's chain is out of date (check this device's clock)");
        }
    }

    /**
     * A certificate time in epoch milliseconds, read from its DER form by hand: RFC 5280 allows only
     * {@code YYMMDDHHMMSSZ} (UTCTime, years 1950 to 2049) and {@code YYYYMMDDHHMMSSZ}
     * (GeneralizedTime), and Bouncy Castle's own conversion goes through {@code SimpleDateFormat},
     * which the browser build lacks.
     */
    static long epochMillis(Time time) throws UntrustedCertificateException {
        byte[] encoded;
        try {
            encoded = time.toASN1Primitive().getEncoded(ASN1Encoding.DER);
        } catch (IOException e) {
            throw new UntrustedCertificateException("A certificate's date can't be read", e);
        }
        int tag = encoded.length > 2 ? encoded[0] & 0xFF : -1;
        int length = encoded.length > 2 ? encoded[1] & 0xFF : -1;
        String text = length == encoded.length - 2 ? new String(encoded, 2, length, StandardCharsets.US_ASCII) : "";
        int year;
        String rest;
        if (tag == 0x17 && text.length() == 13) {
            int yy = digits(text, 0, 2);
            year = yy < 50 ? 2000 + yy : 1900 + yy;
            rest = text.substring(2);
        } else if (tag == 0x18 && text.length() == 15) {
            year = digits(text, 0, 4);
            rest = text.substring(4);
        } else {
            throw new UntrustedCertificateException("A certificate's date isn't in the standard form");
        }
        if (rest.charAt(10) != 'Z') {
            throw new UntrustedCertificateException("A certificate's date isn't in UTC");
        }
        int month = digits(rest, 0, 2);
        int day = digits(rest, 2, 4);
        int hour = digits(rest, 4, 6);
        int minute = digits(rest, 6, 8);
        int second = digits(rest, 8, 10);
        if (month < 1 || month > 12 || day < 1 || day > 31 || hour > 23 || minute > 59 || second > 60) {
            throw new UntrustedCertificateException("A certificate's date isn't a real date");
        }
        long days = daysFromCivil(year, month, day);
        return ((days * 24 + hour) * 60 + minute) * 60_000L + second * 1000L;
    }

    private static int digits(String text, int from, int to) throws UntrustedCertificateException {
        int value = 0;
        for (int i = from; i < to; i++) {
            char c = text.charAt(i);
            if (c < '0' || c > '9') {
                throw new UntrustedCertificateException("A certificate's date isn't in the standard form");
            }
            value = value * 10 + (c - '0');
        }
        return value;
    }

    /** Days since 1970-01-01 in the proleptic Gregorian calendar (Howard Hinnant's algorithm). */
    static long daysFromCivil(int year, int month, int day) {
        int y = month <= 2 ? year - 1 : year;
        int era = Math.floorDiv(y, 400);
        int yearOfEra = y - era * 400;
        int monthIndex = month > 2 ? month - 3 : month + 9;
        int dayOfYear = (153 * monthIndex + 2) / 5 + day - 1;
        int dayOfEra = yearOfEra * 365 + yearOfEra / 4 - yearOfEra / 100 + dayOfYear;
        return era * 146097L + dayOfEra - 719468;
    }

    private static void checkCriticalExtensions(Certificate certificate) throws UntrustedCertificateException {
        Extensions extensions = certificate.getTBSCertificate().getExtensions();
        if (extensions == null) {
            return;
        }
        for (ASN1ObjectIdentifier oid : extensions.getCriticalExtensionOIDs()) {
            if (!UNDERSTOOD.contains(oid)) {
                throw new UntrustedCertificateException(
                        "A certificate in the server's chain has an unknown critical part");
            }
        }
    }

    /** That {@code issuer} may sign certificates, with {@code below} intermediates under it. */
    private static void checkIssuer(Certificate issuer, int below) throws UntrustedCertificateException {
        Extensions extensions = issuer.getTBSCertificate().getExtensions();
        BasicConstraints constraints = extensions == null ? null : BasicConstraints.fromExtensions(extensions);
        if (constraints == null || !constraints.isCA()) {
            throw new UntrustedCertificateException("A certificate in the server's chain was signed by a non-CA");
        }
        BigInteger pathLength = constraints.getPathLenConstraint();
        if (pathLength != null && pathLength.compareTo(BigInteger.valueOf(below)) < 0) {
            throw new UntrustedCertificateException("The server's certificate chain is longer than its CA allows");
        }
        KeyUsage keyUsage = KeyUsage.fromExtensions(extensions);
        if (keyUsage != null && !keyUsage.hasUsages(KeyUsage.keyCertSign)) {
            throw new UntrustedCertificateException(
                    "A certificate in the server's chain was signed by a key not for that");
        }
        // A CA limited to other purposes (code or document signing) can't vouch for a server.
        ExtendedKeyUsage purposes = ExtendedKeyUsage.fromExtensions(extensions);
        if (purposes != null
                && !purposes.hasKeyPurposeId(KeyPurposeId.id_kp_serverAuth)
                && !purposes.hasKeyPurposeId(KeyPurposeId.anyExtendedKeyUsage)) {
            throw new UntrustedCertificateException("A CA in the server's chain isn't for servers");
        }
    }

    /** Whether one of the pinned roots issued {@code certificate}: names match and its key verifies. */
    private boolean signedByAnchor(Certificate certificate) throws UntrustedCertificateException {
        for (Certificate anchor : anchors) {
            if (certificate.getIssuer().equals(anchor.getSubject())
                    && verifies(certificate, anchor.getSubjectPublicKeyInfo())) {
                return true;
            }
        }
        return false;
    }

    // --- Signatures ----------------------------------------------------------------------------

    /** Whether {@code certificate}'s signature verifies with {@code issuerKey}. */
    static boolean verifies(Certificate certificate, SubjectPublicKeyInfo issuerKey)
            throws UntrustedCertificateException {
        AlgorithmIdentifier algorithm = certificate.getSignatureAlgorithm();
        if (!algorithm.equals(certificate.getTBSCertificate().getSignature())) {
            throw new UntrustedCertificateException(
                    "A certificate in the server's chain names two signature algorithms");
        }
        Signer signer = signer(algorithm.getAlgorithm(), issuerKey);
        try {
            byte[] signed = certificate.getTBSCertificate().getEncoded(ASN1Encoding.DER);
            signer.update(signed, 0, signed.length);
            return signer.verifySignature(certificate.getSignature().getOctets());
        } catch (IOException | RuntimeException e) {
            return false;
        }
    }

    private static Signer signer(ASN1ObjectIdentifier algorithm, SubjectPublicKeyInfo issuerKey)
            throws UntrustedCertificateException {
        Digest rsaDigest = rsaDigest(algorithm);
        if (rsaDigest != null) {
            Signer signer = new RSADigestSigner(rsaDigest);
            signer.init(false, rsaKey(issuerKey));
            return signer;
        }
        Digest ecDigest = ecdsaDigest(algorithm);
        if (ecDigest != null) {
            // The curve is checked before the key is read, so no explicit curve is ever parsed.
            Object curve = issuerKey.getAlgorithm().getParameters();
            boolean knownCurve =
                    X9ObjectIdentifiers.prime256v1.equals(curve) || SECObjectIdentifiers.secp384r1.equals(curve);
            if (!X9ObjectIdentifiers.id_ecPublicKey.equals(
                            issuerKey.getAlgorithm().getAlgorithm())
                    || !knownCurve) {
                throw new UntrustedCertificateException(
                        "A key in the server's certificate chain is of an unknown kind");
            }
            AsymmetricKeyParameter key = readKey(issuerKey);
            if (!(key instanceof ECPublicKeyParameters ec)) {
                throw new UntrustedCertificateException(
                        "A key in the server's certificate chain is of an unknown kind");
            }
            Signer signer = new DSADigestSigner(new ECDSASigner(), ecDigest);
            signer.init(false, ec);
            return signer;
        }
        throw new UntrustedCertificateException("The server's certificate chain uses an unsupported signature");
    }

    /** An RSA key, its sizes checked from the raw numbers before Bouncy Castle reads it. */
    private static RSAKeyParameters rsaKey(SubjectPublicKeyInfo issuerKey) throws UntrustedCertificateException {
        if (!PKCSObjectIdentifiers.rsaEncryption.equals(issuerKey.getAlgorithm().getAlgorithm())) {
            throw new UntrustedCertificateException("A key in the server's certificate chain is of an unknown kind");
        }
        RSAPublicKey numbers;
        try {
            numbers = RSAPublicKey.getInstance(issuerKey.parsePublicKey());
        } catch (IOException | RuntimeException e) {
            throw new UntrustedCertificateException("A key in the server's certificate chain can't be read", e);
        }
        int bits = numbers.getModulus().bitLength();
        if (bits < MIN_RSA_BITS
                || bits > MAX_RSA_BITS
                || numbers.getPublicExponent().bitLength() > MAX_RSA_EXPONENT_BITS) {
            throw new UntrustedCertificateException("A key in the server's certificate chain is too weak or too big");
        }
        if (!(readKey(issuerKey) instanceof RSAKeyParameters rsa) || rsa.isPrivate()) {
            throw new UntrustedCertificateException("A key in the server's certificate chain can't be read");
        }
        return rsa;
    }

    private static AsymmetricKeyParameter readKey(SubjectPublicKeyInfo key) throws UntrustedCertificateException {
        try {
            return PublicKeyFactory.createKey(key);
        } catch (IOException | RuntimeException e) {
            throw new UntrustedCertificateException("A key in the server's certificate chain can't be read", e);
        }
    }

    private static @Nullable Digest rsaDigest(ASN1ObjectIdentifier algorithm) {
        if (PKCSObjectIdentifiers.sha256WithRSAEncryption.equals(algorithm)) {
            return new SHA256Digest();
        }
        if (PKCSObjectIdentifiers.sha384WithRSAEncryption.equals(algorithm)) {
            return new SHA384Digest();
        }
        if (PKCSObjectIdentifiers.sha512WithRSAEncryption.equals(algorithm)) {
            return new SHA512Digest();
        }
        return null;
    }

    private static @Nullable Digest ecdsaDigest(ASN1ObjectIdentifier algorithm) {
        if (X9ObjectIdentifiers.ecdsa_with_SHA256.equals(algorithm)) {
            return new SHA256Digest();
        }
        if (X9ObjectIdentifiers.ecdsa_with_SHA384.equals(algorithm)) {
            return new SHA384Digest();
        }
        if (X9ObjectIdentifiers.ecdsa_with_SHA512.equals(algorithm)) {
            return new SHA512Digest();
        }
        return null;
    }
}
