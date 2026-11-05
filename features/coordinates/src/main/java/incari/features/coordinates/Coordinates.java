package incari.features.coordinates;

import incari.framework.Category;
import incari.framework.Module;
import incari.framework.game.PlayerView;
import incari.framework.hud.Anchor;
import incari.framework.hud.HudCanvas;
import incari.framework.hud.HudElement;
import incari.framework.hud.TextElement;
import incari.framework.hud.Tone;
import incari.framework.setting.BooleanSetting;
import incari.framework.setting.ChoiceSetting;
import org.jspecify.annotations.Nullable;

/**
 * Coordinates: where the player is, block by block, which way they face, the biome they stand in,
 * and where the same spot is in the other dimension (a Nether portal's partner).
 */
public final class Coordinates extends Module {

    /** How the position is laid out. */
    public enum Layout {
        /** X, Y and Z on lines of their own. */
        VERTICAL("Lines"),
        /** "X 12  Y 64  Z -40" on one line. */
        LINE("One line");

        private final String label;

        Layout(String label) {
            this.label = label;
        }

        /** What the settings screen calls it. */
        public String label() {
            return label;
        }
    }

    final ChoiceSetting<Layout> layout = settings()
            .choice("layout", "Layout", Layout.values(), Layout.VERTICAL, Layout::label)
            .describedAs("X, Y and Z on lines of their own, or on one.");
    final BooleanSetting showFacing = settings()
            .bool("show-facing", "Facing", true)
            .describedAs("Which way you face, with the axis it runs along.");
    final BooleanSetting showBiome =
            settings().bool("show-biome", "Biome", true).describedAs("The biome you stand in.");
    final BooleanSetting showOther = settings()
            .bool("show-other", "Other dimension", true)
            .describedAs("In the Nether, where you'd be in the Overworld; in the Overworld, in the Nether.");
    final BooleanSetting background =
            settings().bool("background", "Background", true).describedAs("Draws a panel behind the text.");

    /** Coordinates, off until the player turns it on. */
    public Coordinates() {
        super("coordinates", "Coordinates", "Shows where you are, which way you face and the biome.", Category.HUD);
        hud(new Position());
    }

    /** The compass point for a bearing (0 north, 90 east), with the axis that way runs along. */
    static String facing(float heading) {
        int sector = Math.floorMod(Math.round(heading / 45.0F), 8);
        return switch (sector) {
            case 0 -> "North (-Z)";
            case 1 -> "Northeast";
            case 2 -> "East (+X)";
            case 3 -> "Southeast";
            case 4 -> "South (+Z)";
            case 5 -> "Southwest";
            case 6 -> "West (-X)";
            default -> "Northwest";
        };
    }

    /** The block a coordinate is in: rounded down, so -0.5 is block -1. */
    static int block(double coordinate) {
        return (int) Math.floor(coordinate);
    }

    /** The lines, label beside value, on the glass. */
    private final class Position extends HudElement {

        private static final int MAX_ROWS = 6;
        private static final int LABEL_GAP = 4;

        private final @Nullable String[] labels = new String[MAX_ROWS];
        private final String[] values = new String[MAX_ROWS];
        private int rows;

        // What the rows were made from: made again only when one of these changes.
        private int cachedX = Integer.MIN_VALUE;
        private int cachedY;
        private int cachedZ;
        private int cachedOverworldX;
        private int cachedOverworldZ;
        private int cachedSector = -1;
        private @Nullable String cachedBiome;
        private @Nullable String cachedDimension;
        private long cachedSettings = -1;

        Position() {
            super("position", "Coordinates", Anchor.TOP_LEFT, 2, 62);
            for (int i = 0; i < MAX_ROWS; i++) {
                values[i] = "";
            }
        }

        @Override
        public boolean isVisible() {
            return system().game().player() != null;
        }

        private void refresh(HudCanvas canvas) {
            PlayerView player = system().game().player();
            if (player == null) {
                player = canvas.editing() ? system().game().samplePlayer() : null;
            }
            if (player == null) {
                rows = 0;
                return;
            }
            int x = block(player.x());
            int y = block(player.y());
            int z = block(player.z());
            int sector = Math.floorMod(Math.round(player.heading() / 45.0F), 8);
            String biome = showBiome.get() ? player.biome() : "";
            String dimension = player.dimension();
            // The Overworld block matching where they stand in the Nether: from the exact position,
            // as a Nether block spans 8 Overworld ones (x 100.9 is 807, not 800).
            int overworldX = block(player.x() * 8);
            int overworldZ = block(player.z() * 8);
            long settingsKey = (layout.get() == Layout.LINE ? 1 : 0)
                    | (showFacing.get() ? 2 : 0)
                    | (showBiome.get() ? 4 : 0)
                    | (showOther.get() ? 8 : 0);
            if (x == cachedX
                    && y == cachedY
                    && overworldX == cachedOverworldX
                    && overworldZ == cachedOverworldZ
                    && z == cachedZ
                    && sector == cachedSector
                    && biome.equals(cachedBiome)
                    && dimension.equals(cachedDimension)
                    && settingsKey == cachedSettings) {
                return;
            }
            cachedX = x;
            cachedY = y;
            cachedOverworldX = overworldX;
            cachedOverworldZ = overworldZ;
            cachedZ = z;
            cachedSector = sector;
            cachedBiome = biome;
            cachedDimension = dimension;
            cachedSettings = settingsKey;
            rows = 0;
            if (layout.get() == Layout.LINE) {
                row(null, "X " + x + "  Y " + y + "  Z " + z);
            } else {
                row("X", Integer.toString(x));
                row("Y", Integer.toString(y));
                row("Z", Integer.toString(z));
            }
            if (showFacing.get()) {
                row(null, facing(player.heading()));
            }
            if (showBiome.get() && !biome.isEmpty()) {
                row(null, biome);
            }
            if (showOther.get()) {
                if (dimension.equals(PlayerView.NETHER)) {
                    row("Overworld", overworldX + ", " + overworldZ);
                } else if (dimension.equals(PlayerView.OVERWORLD)) {
                    row("Nether", Math.floorDiv(x, 8) + ", " + Math.floorDiv(z, 8));
                }
            }
        }

        private void row(@Nullable String label, String value) {
            labels[rows] = label;
            values[rows] = value;
            rows++;
        }

        private int pad() {
            return background.get() ? TextElement.PAD_X : 0;
        }

        private int padY() {
            return background.get() ? TextElement.PAD_Y : 0;
        }

        /** A row's label and the gap after it; 0 for a row without. */
        private int labelWidth(HudCanvas canvas, int row) {
            String label = labels[row];
            return label == null ? 0 : canvas.textWidth(label) + LABEL_GAP;
        }

        @Override
        public int width(HudCanvas canvas) {
            refresh(canvas);
            int width = 0;
            for (int i = 0; i < rows; i++) {
                width = Math.max(width, labelWidth(canvas, i) + canvas.textWidth(values[i]));
            }
            return rows == 0 ? 0 : width + pad() * 2;
        }

        @Override
        public int height(HudCanvas canvas) {
            refresh(canvas);
            return rows == 0 ? 0 : rows * canvas.lineHeight() + (background.get() ? padY() * 2 - 1 : -1);
        }

        @Override
        public void draw(HudCanvas canvas) {
            refresh(canvas);
            if (rows == 0) {
                return;
            }
            if (background.get()) {
                canvas.fill(0, 0, width(canvas), height(canvas), Tone.BACKGROUND);
            }
            int y = padY();
            for (int i = 0; i < rows; i++) {
                String label = labels[i];
                int x = pad();
                if (label != null) {
                    canvas.text(label, x, y, Tone.ACCENT);
                    x += labelWidth(canvas, i);
                }
                canvas.text(values[i], x, y, label == null && i >= 1 ? Tone.MUTED : Tone.TEXT);
                y += canvas.lineHeight();
            }
        }
    }
}
