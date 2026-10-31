package incari.features.customsky;

import java.util.Locale;
import org.jspecify.annotations.Nullable;

/**
 * One layer of a resource pack's custom sky, in OptiFine's format: a {@code sky<N>.properties} in
 * {@code optifine/sky/world0} (or MCPatcher's {@code mcpatcher/sky/world0}), naming a texture that
 * holds a cube's six faces in a 3 by 2 grid. Read here without the game: the client finds the
 * files, loads the texture, and draws the cube at {@link #brightness}, turned by {@link #angle}.
 *
 * <p>Understood: {@code source}, the four fade times, {@code blend}, {@code rotate}, {@code speed},
 * {@code axis} and {@code weather}. Not yet: {@code biomes}, {@code heights}, {@code days} and
 * {@code transition}; a layer that has them shows everywhere, every day.
 */
public final class SkyLayer {

    /** How a layer is mixed with what's behind it, OptiFine's names. */
    public enum Blend {
        ALPHA,
        ADD,
        SUBTRACT,
        MULTIPLY,
        DODGE,
        BURN,
        SCREEN,
        OVERLAY,
        REPLACE
    }

    /** Ticks in a day; 0 is 6:00, as the game counts. */
    static final int DAY = 24000;

    private final String source;
    /** The fades in ticks from dawn; {@code startFadeIn} -1 for a layer always shown. */
    private final int startFadeIn;

    private final int endFadeIn;
    private final int startFadeOut;
    private final int endFadeOut;
    private final Blend blend;
    private final boolean rotate;
    private final float speed;
    private final float axisX;
    private final float axisY;
    private final float axisZ;
    private final boolean clear;
    private final boolean rain;
    private final boolean thunder;

    private SkyLayer(
            String source,
            int[] fades,
            Blend blend,
            boolean rotate,
            float speed,
            float[] axis,
            boolean clear,
            boolean rain,
            boolean thunder) {
        this.source = source;
        this.startFadeIn = fades[0];
        this.endFadeIn = fades[1];
        this.startFadeOut = fades[2];
        this.endFadeOut = fades[3];
        this.blend = blend;
        this.rotate = rotate;
        this.speed = speed;
        this.axisX = axis[0];
        this.axisY = axis[1];
        this.axisZ = axis[2];
        this.clear = clear;
        this.rain = rain;
        this.thunder = thunder;
    }

    /**
     * The layer {@code text} describes, or null if it can't be drawn (fade times missing or
     * unreadable, a source that isn't a path).
     *
     * @param file where it was found, {@code optifine/sky/world0/sky1.properties}: a source is
     *     relative to its folder, and its own name with {@code .png} is the default source
     */
    public static @Nullable SkyLayer parse(String text, String file) {
        String folder = file.lastIndexOf('/') < 0 ? "" : file.substring(0, file.lastIndexOf('/'));
        String base = file.endsWith(".properties") ? file.substring(0, file.length() - ".properties".length()) : file;
        String sourceText = null;
        String[] fadeText = new String[4];
        Blend blend = Blend.ADD;
        boolean rotate = true;
        float speed = 1;
        float[] axis = {0, 0, 1};
        boolean clear = true;
        boolean rain = false;
        boolean thunder = false;
        for (String raw : text.split("\r?\n", -1)) {
            String line = raw.strip();
            if (line.isEmpty() || line.charAt(0) == '#' || line.charAt(0) == '!') {
                continue;
            }
            int split = line.indexOf('=');
            if (split < 0) {
                split = line.indexOf(':');
            }
            if (split <= 0) {
                continue;
            }
            String key = line.substring(0, split).strip();
            String value = line.substring(split + 1).strip();
            switch (key) {
                case "source" -> sourceText = value;
                case "startFadeIn" -> fadeText[0] = value;
                case "endFadeIn" -> fadeText[1] = value;
                case "startFadeOut" -> fadeText[2] = value;
                case "endFadeOut" -> fadeText[3] = value;
                case "blend" -> blend = blend(value);
                case "rotate" -> rotate = !value.equalsIgnoreCase("false");
                case "speed" -> speed = number(value, 1);
                case "axis" -> axis = axis(value);
                case "weather" -> {
                    String weathers = " " + value.toLowerCase(Locale.ROOT) + " ";
                    clear = weathers.contains(" clear ");
                    rain = weathers.contains(" rain ");
                    thunder = weathers.contains(" thunder ");
                }
                default -> {
                    // Biomes, heights, days, transition: not yet; the layer shows regardless.
                }
            }
        }
        int[] fades = fades(fadeText);
        String source = resolve(sourceText == null ? base + ".png" : sourceText, folder);
        if (fades == null || source == null) {
            return null;
        }
        return new SkyLayer(source, fades, blend, rotate, speed, axis, clear, rain, thunder);
    }

    /** The texture, {@code namespace:path}. */
    public String source() {
        return source;
    }

    public Blend blend() {
        return blend;
    }

    /** The axis the layer turns on, in the world's frame (x east, y up, z south), of length 1. */
    public float axisX() {
        return axisX;
    }

    public float axisY() {
        return axisY;
    }

    public float axisZ() {
        return axisZ;
    }

    /** How far the layer is turned, in radians, with the sun at {@code sunAngle}. */
    public float angle(float sunAngle) {
        return rotate ? sunAngle * speed : 0;
    }

    /**
     * How strongly it shows, 0 to 1, at {@code timeOfDay} ticks from dawn, with the rain and thunder
     * as shown (0 to 1 each).
     */
    public float brightness(long timeOfDay, float rainLevel, float thunderLevel) {
        float weather = 0;
        if (clear) {
            weather += 1 - rainLevel;
        }
        if (rain) {
            weather += Math.max(0, rainLevel - thunderLevel);
        }
        if (thunder) {
            weather += thunderLevel;
        }
        return Math.min(1, Math.max(0, weather)) * fade(timeOfDay);
    }

    private float fade(long timeOfDay) {
        if (startFadeIn < 0) {
            return 1;
        }
        // Every time from the start of the fade in, so a layer that shows over midnight wraps.
        int fullIn = wrap(endFadeIn - startFadeIn);
        int out = wrap(startFadeOut - startFadeIn);
        int gone = wrap(endFadeOut - startFadeIn);
        int now = wrap(timeOfDay - startFadeIn);
        if (now < fullIn) {
            return (float) now / fullIn;
        }
        if (now < out) {
            return 1;
        }
        if (now < gone) {
            return 1 - (float) (now - out) / (gone - out);
        }
        return 0;
    }

    /** The fades in ticks; none at all for a layer always shown, null if they can't be read. */
    private static int @Nullable [] fades(String[] text) {
        if (text[0] == null && text[1] == null && text[2] == null && text[3] == null) {
            return new int[] {-1, -1, -1, -1};
        }
        if (text[0] == null || text[1] == null || text[2] == null) {
            return null;
        }
        int[] fades = new int[4];
        for (int i = 0; i < 3; i++) {
            fades[i] = time(text[i]);
            if (fades[i] < 0) {
                return null;
            }
        }
        // OptiFine's default: the fade out takes as long as the fade in.
        fades[3] = text[3] == null ? wrap(fades[2] + fades[1] - fades[0]) : time(text[3]);
        return fades[3] < 0 ? null : fades;
    }

    /** {@code hh:mm} as ticks from dawn (6:00 is 0), or -1. */
    static int time(String text) {
        int colon = text.indexOf(':');
        if (colon < 0) {
            return -1;
        }
        try {
            int hours = Integer.parseInt(text.substring(0, colon).strip());
            int minutes = Integer.parseInt(text.substring(colon + 1).strip());
            if (hours < 0 || hours > 24 || minutes < 0 || minutes > 59) {
                return -1;
            }
            return wrap(hours * 1000 + minutes * 1000 / 60 - 6000);
        } catch (NumberFormatException e) {
            return -1;
        }
    }

    /**
     * A source as OptiFine resolves it: {@code ./} from the file's folder, {@code ~/} from
     * {@code optifine}, {@code assets/<namespace>/} or a namespace for that namespace, a bare name
     * in the file's folder, and any other path from {@code assets/minecraft}. Null if it isn't a
     * path the game can name.
     */
    static @Nullable String resolve(String source, String folder) {
        String path = source.strip();
        String namespace = "minecraft";
        if (path.startsWith("./")) {
            path = folder + "/" + path.substring(2);
        } else if (path.startsWith("~/")) {
            path = "optifine/" + path.substring(2);
        } else {
            if (path.startsWith("/")) {
                path = path.substring(1);
            }
            int colon = path.indexOf(':');
            if (colon > 0) {
                namespace = path.substring(0, colon);
                path = path.substring(colon + 1);
            } else if (path.startsWith("assets/") && path.indexOf('/', 7) > 7) {
                namespace = path.substring(7, path.indexOf('/', 7));
                path = path.substring(path.indexOf('/', 7) + 1);
            } else if (path.indexOf('/') < 0) {
                path = folder.isEmpty() ? path : folder + "/" + path;
            }
        }
        return validPath(namespace, path) ? namespace + ":" + path : null;
    }

    /** What the game takes for a resource's namespace and path. */
    private static boolean validPath(String namespace, String path) {
        if (namespace.isEmpty() || path.isEmpty() || path.contains("..")) {
            return false;
        }
        for (int i = 0; i < namespace.length(); i++) {
            if (!nameChar(namespace.charAt(i))) {
                return false;
            }
        }
        for (int i = 0; i < path.length(); i++) {
            if (path.charAt(i) != '/' && !nameChar(path.charAt(i))) {
                return false;
            }
        }
        return true;
    }

    private static boolean nameChar(char c) {
        return (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '_' || c == '-' || c == '.';
    }

    /**
     * Where a file comes in its folder's order: the number in {@code sky12.properties}; a file
     * without one goes last.
     */
    public static int order(String file) {
        String name = file.substring(file.lastIndexOf('/') + 1);
        int end = name.indexOf('.') < 0 ? name.length() : name.indexOf('.');
        int start = end;
        while (start > 0 && Character.isDigit(name.charAt(start - 1))) {
            start--;
        }
        if (start == end || end - start > 6) {
            return Integer.MAX_VALUE;
        }
        return Integer.parseInt(name.substring(start, end));
    }

    private static Blend blend(String text) {
        return switch (text.strip().toLowerCase(Locale.ROOT)) {
            case "alpha" -> Blend.ALPHA;
            case "subtract" -> Blend.SUBTRACT;
            case "multiply" -> Blend.MULTIPLY;
            case "dodge" -> Blend.DODGE;
            case "burn" -> Blend.BURN;
            case "screen" -> Blend.SCREEN;
            case "overlay" -> Blend.OVERLAY;
            case "replace" -> Blend.REPLACE;
            default -> Blend.ADD;
        };
    }

    private static float number(String text, float fallback) {
        try {
            float value = Float.parseFloat(text);
            return Float.isFinite(value) ? value : fallback;
        } catch (NumberFormatException e) {
            return fallback;
        }
    }

    /** Three numbers, made length 1; the sun's axis if they aren't. */
    private static float[] axis(String text) {
        String[] parts = text.strip().split("\\s+", -1);
        if (parts.length == 3) {
            float x = number(parts[0], Float.NaN);
            float y = number(parts[1], Float.NaN);
            float z = number(parts[2], Float.NaN);
            float length = (float) Math.sqrt(x * x + y * y + z * z);
            if (length > 1.0E-4F) {
                return new float[] {x / length, y / length, z / length};
            }
        }
        return new float[] {0, 0, 1};
    }

    private static int wrap(long ticks) {
        return (int) Math.floorMod(ticks, (long) DAY);
    }
}
