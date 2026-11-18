package incari.framework.config;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParseException;
import com.google.gson.JsonParser;
import incari.framework.Module;
import java.io.IOException;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.function.Consumer;
import org.jspecify.annotations.Nullable;

/**
 * The player's profiles: named sets of module states and settings ("PvP", "Building"), one active.
 * Each is a file, {@code profiles/<name>.json} ({@link ProfileCodec}); {@code profiles.json} lists
 * them and names the active one. A new profile starts as a copy of the active one.
 *
 * <p>Game thread. A file that can't be parsed is reported and read as empty (every default), and
 * isn't overwritten until something changes.
 */
public final class Profiles {

    /** The first profile's name. */
    public static final String DEFAULT = "Default";

    private static final String INDEX = "profiles.json";
    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();
    private static final int MAX_NAME = 24;

    private final ConfigStorage storage;
    private final List<Module> modules;
    private final Consumer<String> warnings;
    private final List<String> names = new ArrayList<>();
    private final List<String> namesView = Collections.unmodifiableList(names);
    private String active = DEFAULT;
    /** The active profile's entries for modules this build doesn't have. */
    private JsonObject unknown = new JsonObject();

    /**
     * @param warnings where problems with the files go (one line each, for the log)
     */
    public Profiles(ConfigStorage storage, List<Module> modules, Consumer<String> warnings) {
        this.storage = storage;
        this.modules = modules;
        this.warnings = warnings;
        names.add(DEFAULT);
    }

    /** Every profile's name, in the order they were made. */
    public List<String> names() {
        return namesView;
    }

    /** The active profile's name. */
    public String active() {
        return active;
    }

    /** Reads the profile list and applies the active profile; every default if there's none yet. */
    public void load() throws IOException {
        JsonObject index = parse(INDEX, storage.read(INDEX));
        names.clear();
        JsonElement listed = index.get("profiles");
        if (listed != null && listed.isJsonArray()) {
            for (JsonElement name : listed.getAsJsonArray()) {
                if (name.isJsonPrimitive() && validName(name.getAsString()) && indexOf(name.getAsString()) < 0) {
                    names.add(name.getAsString());
                }
            }
        }
        if (names.isEmpty()) {
            names.add(DEFAULT);
        }
        JsonElement chosen = index.get("active");
        String wanted = chosen != null && chosen.isJsonPrimitive() ? chosen.getAsString() : DEFAULT;
        int at = indexOf(wanted);
        active = names.get(at < 0 ? 0 : at);
        apply(active);
    }

    /** Writes the active profile and the list. */
    public void save() throws IOException {
        storage.write(file(active), pretty(ProfileCodec.write(modules, unknown)));
        writeIndex();
    }

    /**
     * Saves the active profile, then makes {@code name} active and applies it.
     *
     * @return false if there's no such profile
     */
    public boolean switchTo(String name) throws IOException {
        int at = indexOf(name);
        if (at < 0) {
            return false;
        }
        save();
        active = names.get(at);
        apply(active);
        writeIndex();
        return true;
    }

    /**
     * Saves the active profile, then makes a copy of it called {@code name} and makes that active.
     *
     * @return false if the name isn't {@linkplain #validName valid} or is taken
     */
    public boolean create(String name) throws IOException {
        if (!validName(name) || indexOf(name) >= 0) {
            return false;
        }
        save();
        names.add(name);
        active = name;
        save();
        return true;
    }

    /**
     * Deletes the profile {@code name}. If it's the active one, the first one left becomes active.
     *
     * @return false if there's no such profile or it's the only one
     */
    public boolean delete(String name) throws IOException {
        int at = indexOf(name);
        if (at < 0 || names.size() == 1) {
            return false;
        }
        String deleted = names.remove(at);
        storage.delete(file(deleted));
        if (deleted.equals(active)) {
            active = names.get(0);
            apply(active);
        }
        writeIndex();
        return true;
    }

    /**
     * Whether {@code name} can name a profile: 1 to 24 letters, digits, spaces, {@code _}, {@code -}
     * or {@code .}, not starting or ending with a space and not starting with a dot (the 1.8 PvP
     * preset's name has one). It's also the file's name.
     */
    public static boolean validName(String name) {
        if (name.isEmpty()
                || name.length() > MAX_NAME
                || name.isBlank()
                || !name.strip().equals(name)
                || name.charAt(0) == '.') {
            return false;
        }
        for (int i = 0; i < name.length(); i++) {
            char c = name.charAt(i);
            if (!(Character.isLetterOrDigit(c) || c == ' ' || c == '_' || c == '-' || c == '.') || c > 0x7F) {
                return false;
            }
        }
        return true;
    }

    /**
     * The profiles of a settings file: a name to the profile as {@link ProfileCodec} JSON, and the
     * name of the one that applies.
     */
    public record Transfer(String active, JsonObject data) {

        /** Builds one, dropping names that can't name a profile and entries that aren't objects. */
        public static Transfer of(@Nullable String active, JsonObject data) {
            JsonObject kept = new JsonObject();
            for (Map.Entry<String, JsonElement> entry : data.entrySet()) {
                if (validName(entry.getKey()) && entry.getValue().isJsonObject()) {
                    kept.add(entry.getKey(), entry.getValue());
                }
            }
            return new Transfer(active == null || active.isBlank() ? DEFAULT : active, kept);
        }
    }

    /**
     * Replaces every profile with {@code transfer}'s, keeping entries for modules this build doesn't
     * have as the file has them, then makes {@code transfer}'s active one active and applies it. An
     * active that isn't one of the names falls back to the first profile left.
     */
    public void replaceAll(Transfer transfer) throws IOException {
        Set<String> kept = new HashSet<>();
        List<String> keptNames = new ArrayList<>();
        for (Map.Entry<String, JsonElement> entry : transfer.data().entrySet()) {
            String name = entry.getKey();
            storage.write(file(name), pretty(entry.getValue().getAsJsonObject()));
            kept.add(name.toLowerCase(Locale.ROOT));
            keptNames.add(name);
        }
        for (String name : List.copyOf(names)) {
            if (!kept.contains(name.toLowerCase(Locale.ROOT))) {
                storage.delete(file(name));
            }
        }
        names.clear();
        if (keptNames.isEmpty()) {
            names.add(DEFAULT);
        } else {
            names.addAll(keptNames);
        }
        int at = indexOf(transfer.active());
        this.active = at >= 0 ? names.get(at) : names.get(0);
        apply(this.active);
        writeIndex();
    }

    private void apply(String name) throws IOException {
        unknown = ProfileCodec.read(parse(file(name), storage.read(file(name))), modules);
    }

    private void writeIndex() throws IOException {
        JsonObject index = new JsonObject();
        index.addProperty("format", ProfileCodec.FORMAT);
        index.addProperty("active", active);
        JsonArray list = new JsonArray();
        for (String name : names) {
            list.add(name);
        }
        index.add("profiles", list);
        storage.write(INDEX, pretty(index));
    }

    /** Profile names are matched without case: the browser's files and Windows' don't tell them apart. */
    private int indexOf(String name) {
        for (int i = 0; i < names.size(); i++) {
            if (names.get(i).toLowerCase(Locale.ROOT).equals(name.toLowerCase(Locale.ROOT))) {
                return i;
            }
        }
        return -1;
    }

    private static String file(String name) {
        return "profiles/" + name + ".json";
    }

    private JsonObject parse(String path, @Nullable String text) {
        if (text == null) {
            return new JsonObject();
        }
        try {
            JsonElement root = JsonParser.parseString(text);
            if (root.isJsonObject()) {
                return root.getAsJsonObject();
            }
            warnings.accept(path + " isn't a JSON object; using the defaults");
        } catch (JsonParseException e) {
            warnings.accept(path + " can't be read (" + e.getMessage() + "); using the defaults");
        }
        return new JsonObject();
    }

    private static String pretty(JsonObject root) {
        return GSON.toJson(root) + "\n";
    }
}
