package incari.classlib.substitutes.java.io;

import java.io.IOException;
import java.io.InputStream;

/**
 * {@code java.io.ObjectInputStream}: present only so libraries that probe for it by reflection
 * link (Gson looks for its instance allocator and falls back when the lookup fails), and so Bouncy
 * Castle's key factories link (their XMSS keys deserialize; sign-in's TLS never meets one, ADR
 * 0070). The browser build has no Java serialization: opening one fails.
 */
public class TObjectInputStream extends InputStream {

    protected TObjectInputStream() {}

    public TObjectInputStream(InputStream in) throws IOException {
        throw new IOException("The browser build has no Java serialization");
    }

    public final Object readObject() throws IOException, ClassNotFoundException {
        throw new IOException("The browser build has no Java serialization");
    }

    @Override
    public int read() {
        throw unsupported();
    }

    @Override
    public int read(byte[] buffer, int offset, int length) {
        throw unsupported();
    }

    private static UnsupportedOperationException unsupported() {
        return new UnsupportedOperationException("The browser build has no Java serialization");
    }
}
