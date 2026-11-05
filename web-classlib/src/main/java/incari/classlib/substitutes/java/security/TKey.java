package incari.classlib.substitutes.java.security;

import java.io.Serializable;

/**
 * {@code java.security.Key}, which TeaVM lacks: a key's algorithm, encoding format and encoded
 * form. The browser build's keys are {@code incari.classlib.security}'s RSA keys and
 * {@code SecretKeySpec} (ADR 0070).
 */
public interface TKey extends Serializable {

    String getAlgorithm();

    String getFormat();

    byte[] getEncoded();
}
