package incari.features.schematics;

import incari.framework.game.ItemView;
import incari.framework.game.PlayerView;
import incari.framework.hud.Anchor;
import incari.framework.hud.HudCanvas;
import incari.framework.hud.HudElement;
import incari.framework.hud.TextElement;
import incari.framework.hud.Tone;
import incari.framework.setting.IntSetting;
import java.util.ArrayList;
import java.util.List;
import org.jspecify.annotations.Nullable;

/**
 * The material list: the items the schematic still needs, most first, each with its icon and how
 * many. The count is green when the player carries enough, amber when not. The rows are made again
 * when the counts change ({@link #changed}), every half second at most.
 */
final class MaterialPanel extends HudElement {

    private static final int ICON = 16;
    private static final int GAP = 3;
    private static final int NAME_WIDTH = 110;

    final IntSetting rows =
            settings().integer("rows", "Rows", 3, 20, 8).describedAs("How many items it lists at most.");

    private final SchematicsModule module;
    private final List<Row> shown = new ArrayList<>();
    private String header = "";
    private boolean stale = true;
    private boolean editingRows;
    private int countWidth;

    /** One listed item. */
    private record Row(ItemView item, String id, String name, String count, int needed) {}

    MaterialPanel(SchematicsModule module) {
        super("materials", "Material list", Anchor.TOP_RIGHT, 4, 60);
        this.module = module;
        rows.onChange(this::changed);
    }

    /** The counts may have changed: the rows are made again when next drawn. */
    void changed() {
        stale = true;
    }

    @Override
    public boolean isVisible() {
        return module.isLoaded();
    }

    private List<Row> rows(HudCanvas canvas) {
        boolean editing = canvas.editing() && !module.isLoaded();
        if (stale || editing != editingRows) {
            stale = false;
            editingRows = editing;
            build(canvas, editing);
        }
        return shown;
    }

    private void build(HudCanvas canvas, boolean editing) {
        shown.clear();
        countWidth = 0;
        SchematicsModule.Loaded now = module.loaded();
        if (now == null) {
            if (editing) {
                header = "Materials: 2 left";
                add(canvas, "minecraft:stone_bricks", 320);
                add(canvas, "minecraft:oak_planks", 64);
            } else {
                header = "";
            }
            return;
        }
        Materials materials = now.materials;
        int left = materials.kindsNeeded();
        header = left == 0 ? "Materials: none left" : "Materials: " + left + " left";
        for (int rank = 0; rank < materials.kinds() && shown.size() < rows.get(); rank++) {
            int item = materials.ranked(rank);
            int needed = materials.needed(item);
            if (needed <= 0) {
                break;
            }
            add(canvas, materials.id(item), needed);
        }
    }

    private void add(HudCanvas canvas, String id, int needed) {
        ItemView item = module.game().item(id);
        String name = item.name();
        Row row = new Row(item, id, fit(canvas, name.isEmpty() ? id : name), Integer.toString(needed), needed);
        shown.add(row);
        countWidth = Math.max(countWidth, canvas.textWidth(row.count));
    }

    private static String fit(HudCanvas canvas, String text) {
        if (canvas.textWidth(text) <= NAME_WIDTH) {
            return text;
        }
        String cut = text;
        while (!cut.isEmpty() && canvas.textWidth(cut + "...") > NAME_WIDTH) {
            cut = cut.substring(0, cut.length() - 1);
        }
        return cut.strip() + "...";
    }

    @Override
    public int width(HudCanvas canvas) {
        List<Row> list = rows(canvas);
        if (header.isEmpty()) {
            return 0;
        }
        int widest = canvas.textWidth(header);
        for (Row row : list) {
            widest = Math.max(widest, ICON + GAP + canvas.textWidth(row.name) + GAP * 2 + countWidth);
        }
        return widest + TextElement.PAD_X * 2;
    }

    @Override
    public int height(HudCanvas canvas) {
        List<Row> list = rows(canvas);
        if (header.isEmpty()) {
            return 0;
        }
        return TextElement.PAD_Y * 2 + canvas.lineHeight() + list.size() * (ICON + 1);
    }

    @Override
    public void draw(HudCanvas canvas) {
        List<Row> list = rows(canvas);
        if (header.isEmpty()) {
            return;
        }
        int width = width(canvas);
        canvas.fill(0, 0, width, height(canvas), Tone.BACKGROUND);
        canvas.text(header, TextElement.PAD_X, TextElement.PAD_Y, Tone.ACCENT);
        PlayerView player = module.game().player();
        int y = TextElement.PAD_Y + canvas.lineHeight();
        int textY = (ICON - canvas.lineHeight()) / 2 + 1;
        for (Row row : list) {
            canvas.item(row.item, TextElement.PAD_X, y, false);
            canvas.text(row.name, TextElement.PAD_X + ICON + GAP, y + textY, Tone.TEXT);
            int countX = width - TextElement.PAD_X - canvas.textWidth(row.count);
            canvas.text(row.count, countX, y + textY, tone(player, row));
            y += ICON + 1;
        }
    }

    /** Green when the player carries enough of it, amber when not. */
    private static Tone tone(@Nullable PlayerView player, Row row) {
        if (player == null) {
            return Tone.TEXT;
        }
        return player.count(row.id) >= row.needed ? Tone.POSITIVE : Tone.WARNING;
    }
}
