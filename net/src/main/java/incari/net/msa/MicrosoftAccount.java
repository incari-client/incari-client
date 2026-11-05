package incari.net.msa;

import com.google.gson.JsonObject;
import com.google.gson.JsonParseException;
import com.google.gson.JsonParser;
import org.jspecify.annotations.Nullable;

/**
 * A signed-in Minecraft account: who it is, the Minecraft token servers check, and the Microsoft
 * refresh token that gets a new one. The tokens are secrets: {@link #toString} leaves them out,
 * and nothing logs them.
 *
 * @param name the player's name
 * @param id the profile id, 32 hex digits without dashes, as Mojang's services write it
 * @param minecraftToken the token {@code sessionserver} checks on a join
 * @param minecraftExpiresAtMillis when it stops working, in epoch milliseconds
 * @param refreshToken Microsoft's refresh token, for a new Minecraft token without signing in again
 */
public record MicrosoftAccount(
        String name, String id, String minecraftToken, long minecraftExpiresAtMillis, String refreshToken) {

    public MicrosoftAccount {
        if (!isPlayerName(name)) {
            throw new IllegalArgumentException("Not a player name");
        }
        if (!isProfileId(id)) {
            throw new IllegalArgumentException("Not a profile id");
        }
        if (minecraftToken.isEmpty() || refreshToken.isEmpty()) {
            throw new IllegalArgumentException("Missing token");
        }
    }

    /** The profile id with dashes, as {@code java.util.UUID} reads it. */
    public String dashedId() {
        return id.substring(0, 8) + "-" + id.substring(8, 12) + "-" + id.substring(12, 16) + "-" + id.substring(16, 20)
                + "-" + id.substring(20);
    }

    /** Whether the Minecraft token is still good for at least {@code marginMillis} at {@code nowMillis}. */
    public boolean fresh(long nowMillis, long marginMillis) {
        return nowMillis + marginMillis < minecraftExpiresAtMillis;
    }

    /** The account as JSON, for its file. Holds the tokens: only ever written to the player's own storage. */
    public String toJson() {
        JsonObject json = new JsonObject();
        json.addProperty("version", 1);
        json.addProperty("name", name);
        json.addProperty("id", id);
        json.addProperty("minecraftToken", minecraftToken);
        json.addProperty("minecraftExpiresAt", minecraftExpiresAtMillis);
        json.addProperty("refreshToken", refreshToken);
        return json.toString();
    }

    /** The account {@link #toJson} wrote, or null if the text isn't one. */
    public static @Nullable MicrosoftAccount fromJson(String text) {
        try {
            JsonObject json = JsonParser.parseString(text).getAsJsonObject();
            if (json.get("version").getAsInt() != 1) {
                return null;
            }
            return new MicrosoftAccount(
                    json.get("name").getAsString(),
                    json.get("id").getAsString(),
                    json.get("minecraftToken").getAsString(),
                    json.get("minecraftExpiresAt").getAsLong(),
                    json.get("refreshToken").getAsString());
        } catch (JsonParseException
                | IllegalStateException
                | IllegalArgumentException
                | NullPointerException
                | UnsupportedOperationException e) {
            return null;
        }
    }

    static boolean isPlayerName(String name) {
        if (name.isEmpty() || name.length() > 16) {
            return false;
        }
        for (int i = 0; i < name.length(); i++) {
            char c = name.charAt(i);
            if (!((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '_')) {
                return false;
            }
        }
        return true;
    }

    static boolean isProfileId(String id) {
        if (id.length() != 32) {
            return false;
        }
        for (int i = 0; i < id.length(); i++) {
            char c = id.charAt(i);
            if (!((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f'))) {
                return false;
            }
        }
        return true;
    }

    @Override
    public String toString() {
        return "MicrosoftAccount[" + name + ", " + dashedId() + "]";
    }
}
