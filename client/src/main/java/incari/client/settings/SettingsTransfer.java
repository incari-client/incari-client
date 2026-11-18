package incari.client.settings;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParseException;
import com.google.gson.JsonParser;
import com.mojang.logging.LogUtils;
import incari.client.modules.IncariModules;
import incari.client.online.IncariOnline;
import incari.client.modules.ModuleStorage;
import incari.client.platform.ClientPlatform;
import incari.framework.config.ConfigStorage;
import incari.framework.config.Profiles;
import incari.framework.online.Online;
import incari.online.OnlineService;
import incari.platform.api.PlatformShell;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.toasts.SystemToast;
import net.minecraft.client.gui.screens.ConfirmScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;

/**
 * The settings in one file ({@code incari-settings.json}), out and back in: the client's own
 * switches, the client's keybinds and every profile. Export writes the file: beside the game where
 * the platform shows folders, a download where it doesn't. Import goes through the game's file
 * chooser, the way modules pick files up, and replaces everything it covers after a confirm.
 * A file of another kind or a newer format is refused whole, never half-read.
 *
 * <p>Game thread.
 */
public final class SettingsTransfer {

    private static final Logger LOGGER = LogUtils.getLogger();

    /** What the file says it is: one of any other kind is refused. */
    public static final String APP = "incari-settings";

    /** The file format's version; a newer one is refused rather than read half-understood. */
    public static final int FORMAT = 1;

    private static final String FILE = "incari-settings.json";
    private static final String FOLDER = "incari/settings";
    private static final SystemToast.SystemToastId TOAST = new SystemToast.SystemToastId();
    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();

    private SettingsTransfer() {}

    /**
     * Writes the settings file out. A platform that shows folders gets it beside the game and the
     * folder opened; a page has no folder to show, so it downloads instead.
     */
    public static void export() {
        Minecraft minecraft = Minecraft.getInstance();
        Path gameDirectory = minecraft.gameDirectory.toPath();
        JsonObject data = bundle(gameDirectory);
        String text = GSON.toJson(data);
        syncToAccount(data.toString());
        PlatformShell shell = ClientPlatform.get().shell();
        if (shell.showsFolders()) {
            Path folder = gameDirectory.resolve("incari").resolve("settings");
            Path file = folder.resolve(FILE);
            try {
                Files.createDirectories(folder);
                Files.writeString(file, text, StandardCharsets.UTF_8);
            } catch (IOException e) {
                LOGGER.warn("Couldn't write {}: {}", file, e.toString());
                toast(minecraft, "Couldn't export", "The settings file couldn't be written");
                return;
            }
            shell.openPath(folder.toAbsolutePath().toString());
            return;
        }
        shell.download(FILE, text.getBytes(StandardCharsets.UTF_8));
    }

    /**
     * Opens the file chooser over the import folder. In the browser the file the player picks (or
     * drops on the window) is copied into it, on the desktop the player puts it there; then the
     * file comes back here to be confirmed and applied.
     */
    public static void openImport(Screen parent) {
        IncariModules.system()
                .game()
                .chooseFile("Import settings", FOLDER, List.of("json"), (name, bytes) -> confirm(parent, name, bytes));
    }

    /** Reads the file back and asks before anything changes. */
    private static void confirm(Screen parent, String name, byte[] bytes) {
        Minecraft minecraft = Minecraft.getInstance();
        JsonObject bundle = parse(new String(bytes, StandardCharsets.UTF_8));
        if (bundle == null || !APP.equals(text(bundle, "app")) || newerThanThisBuild(bundle)) {
            toast(minecraft, "Couldn't import", name + " isn't a settings file this build reads");
            return;
        }
        minecraft.gui.setScreen(new ConfirmScreen(
                imported -> {
                    if (imported) {
                        apply(minecraft, bundle);
                    }
                    minecraft.gui.setScreen(parent);
                },
                Component.literal("Import " + name + "?"),
                Component.literal("Replaces every profile, the client's keybinds and the switches on this screen.")));
    }

    private static void apply(Minecraft minecraft, JsonObject bundle) {
        JsonObject client = object(bundle, "client");
        ClientSettings.importFrom(client);
        IncariKeys.importFrom(object(bundle, "keys"));
        JsonObject profiles = object(bundle, "profiles");
        boolean applied = IncariModules.system()
                .replaceProfiles(Profiles.Transfer.of(text(profiles, "active"), object(profiles, "data")));
        toast(
                minecraft,
                applied ? "Settings imported" : "Couldn't import",
                applied ? "Profiles, keybinds and switches replaced" : "The profiles couldn't be written");
        if (!applied) {
            LOGGER.warn("Couldn't apply the profiles of an imported settings file");
        }
        if (applied) {
            syncToAccount(bundle.toString());
        }
    }

    /**
     * Pushes the settings bundle to the player's Incari account, so signing in elsewhere can
     * restore it (issue #29). Quietly skipped when the player isn't signed in or the service
     * doesn't keep client state.
     */
    static void syncToAccount(String json) {
        OnlineService online = IncariOnline.service();
        if (online == null || online.status() != Online.Status.SIGNED_IN) {
            return;
        }
        online.saveClientState(json, result -> {});
    }

    /**
     * Pulls the settings bundle the account holds and asks before anything changes: the restore
     * side of the account sync (issue #29). A toast says what's missing when there's nothing yet.
     */
    public static void importFromAccount(Screen parent) {
        Minecraft minecraft = Minecraft.getInstance();
        OnlineService online = IncariOnline.service();
        if (online == null || online.status() != Online.Status.SIGNED_IN) {
            toast(minecraft, "Not signed in", "Sign in to the Incari account first");
            return;
        }
        String json = online.clientState();
        if (json == null) {
            toast(minecraft, "Nothing saved yet", "Export on a signed-in device first");
            return;
        }
        confirm(parent, "the account's settings", json.getBytes(StandardCharsets.UTF_8));
    }

    /** The whole file: what it is, the switches, the keybinds and the profiles. */
    private static JsonObject bundle(Path gameDirectory) {
        JsonObject bundle = new JsonObject();
        bundle.addProperty("app", APP);
        bundle.addProperty("format", FORMAT);
        bundle.add("client", ClientSettings.export());
        bundle.add("keys", IncariKeys.export());
        bundle.add("profiles", profiles(gameDirectory));
        return bundle;
    }

    /** The profiles as they are on disk: the active one, the names and each profile's file. */
    private static JsonObject profiles(Path gameDirectory) {
        JsonObject profiles = new JsonObject();
        ConfigStorage storage = ModuleStorage.folder(gameDirectory, "modules");
        try {
            JsonObject index = parse(storage.read("profiles.json"));
            if (index == null) {
                return profiles;
            }
            String active = text(index, "active");
            if (active != null) {
                profiles.addProperty("active", active);
            }
            JsonObject data = new JsonObject();
            JsonArray names = new JsonArray();
            JsonElement listed = index.get("profiles");
            if (listed != null && listed.isJsonArray()) {
                for (JsonElement name : listed.getAsJsonArray()) {
                    if (!name.isJsonPrimitive() || !Profiles.validName(name.getAsString())) {
                        continue;
                    }
                    String id = name.getAsString();
                    JsonObject profile = parse(storage.read("profiles/" + id + ".json"));
                    if (profile == null) {
                        continue;
                    }
                    data.add(id, profile);
                    names.add(id);
                }
            }
            profiles.add("names", names);
            profiles.add("data", data);
        } catch (IOException e) {
            LOGGER.warn("Couldn't read the module profiles: {}", e.toString());
        }
        return profiles;
    }

    private static boolean newerThanThisBuild(JsonObject bundle) {
        JsonElement format = bundle.get("format");
        return format != null && format.isJsonPrimitive() && format.getAsInt() > FORMAT;
    }

    /** The member as an object, or an empty one (a missing section imports as nothing). */
    private static JsonObject object(JsonObject object, String name) {
        JsonElement value = object.get(name);
        return value != null && value.isJsonObject() ? value.getAsJsonObject() : new JsonObject();
    }

    /** The member as text, or null. */
    private static @Nullable String text(JsonObject object, String name) {
        JsonElement value = object.get(name);
        return value != null && value.isJsonPrimitive() ? value.getAsString() : null;
    }

    /** The text as an object, or null: not there, not an object, or not JSON. */
    private static @Nullable JsonObject parse(@Nullable String text) {
        if (text == null) {
            return null;
        }
        try {
            JsonElement parsed = JsonParser.parseString(text);
            return parsed.isJsonObject() ? parsed.getAsJsonObject() : null;
        } catch (JsonParseException e) {
            LOGGER.warn("Couldn't parse a settings file: {}", e.toString());
            return null;
        }
    }

    private static void toast(Minecraft minecraft, String title, String text) {
        SystemToast.addOrUpdate(minecraft.gui.toastManager(), TOAST, Component.literal(title), Component.literal(text));
    }
}
