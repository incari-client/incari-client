package incari.features.schematics.format;

/** A schematic file that can't be read; the message says why in the player's words. */
public final class SchematicException extends Exception {

    private static final long serialVersionUID = 1L;

    public SchematicException(String message) {
        super(message);
    }

    public SchematicException(String message, Throwable cause) {
        super(message, cause);
    }
}
