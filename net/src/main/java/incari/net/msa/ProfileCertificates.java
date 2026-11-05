package incari.net.msa;

/**
 * The key pair Mojang issues an account for signing chat, with Mojang's signature over its public
 * half. Servers that enforce secure chat refuse players without one.
 */
public final class ProfileCertificates {

    private final String privateKeyPem;
    private final String publicKeyPem;
    private final byte[] publicKeySignature;
    private final String expiresAt;
    private final String refreshedAfter;

    /**
     * @param privateKeyPem the private key, PEM ({@code -----BEGIN RSA PRIVATE KEY-----}); secret
     * @param publicKeyPem the public key, PEM ({@code -----BEGIN RSA PUBLIC KEY-----})
     * @param publicKeySignature Mojang's signature over the public key and profile id (v2)
     * @param expiresAt when the key stops being accepted, ISO-8601
     * @param refreshedAfter when to fetch a new one, ISO-8601
     */
    public ProfileCertificates(
            String privateKeyPem,
            String publicKeyPem,
            byte[] publicKeySignature,
            String expiresAt,
            String refreshedAfter) {
        this.privateKeyPem = privateKeyPem;
        this.publicKeyPem = publicKeyPem;
        this.publicKeySignature = publicKeySignature.clone();
        this.expiresAt = expiresAt;
        this.refreshedAfter = refreshedAfter;
    }

    public String privateKeyPem() {
        return privateKeyPem;
    }

    public String publicKeyPem() {
        return publicKeyPem;
    }

    public byte[] publicKeySignature() {
        return publicKeySignature.clone();
    }

    public String expiresAt() {
        return expiresAt;
    }

    public String refreshedAfter() {
        return refreshedAfter;
    }

    @Override
    public String toString() {
        return "ProfileCertificates[expires " + expiresAt + "]";
    }
}
