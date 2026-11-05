package incari.classlib.substitutes.javax.crypto;

import incari.classlib.substitutes.java.security.TKey;

/** {@code javax.crypto.SecretKey}, which TeaVM lacks (ADR 0070). */
public interface TSecretKey extends TKey {}
