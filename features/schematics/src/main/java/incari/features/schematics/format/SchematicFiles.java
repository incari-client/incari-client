package incari.features.schematics.format;

import java.io.IOException;
import java.util.List;
import java.util.Locale;

/** Reads any schematic file the client knows, telling the formats apart by what's in them. */
public final class SchematicFiles {

    /** The files the chooser lists. */
    public static final List<String> EXTENSIONS = List.of(".litematic", ".schem", ".nbt");

    private SchematicFiles() {}

    /**
     * The schematic in {@code bytes}, a file called {@code fileName}.
     *
     * @throws SchematicException if it isn't one, or is broken or too big; the message says which
     */
    public static Schematic read(String fileName, byte[] bytes) throws SchematicException {
        Nbt.Compound root;
        try {
            root = Nbt.read(bytes);
        } catch (IOException | RuntimeException e) {
            throw new SchematicException("Not a schematic, or a broken one", e);
        }
        String name = baseName(fileName);
        if (LitematicReader.accepts(root)) {
            return LitematicReader.read(root, name);
        }
        Nbt.Compound sponge = SpongeReader.body(root);
        if (sponge != null) {
            return SpongeReader.read(sponge, name);
        }
        if (StructureReader.accepts(root)) {
            return StructureReader.read(root, name);
        }
        if (root.has("Materials") || root.has("Blocks")) {
            throw new SchematicException("Old schematics from before 1.13 aren't supported");
        }
        throw new SchematicException("Not a schematic this client knows");
    }

    /** {@code castle.litematic} without its extension. */
    static String baseName(String fileName) {
        String name = fileName;
        int slash = Math.max(name.lastIndexOf('/'), name.lastIndexOf('\\'));
        if (slash >= 0) {
            name = name.substring(slash + 1);
        }
        int dot = name.lastIndexOf('.');
        if (dot > 0 && EXTENSIONS.contains(name.substring(dot).toLowerCase(Locale.ROOT))) {
            name = name.substring(0, dot);
        }
        return name;
    }
}
