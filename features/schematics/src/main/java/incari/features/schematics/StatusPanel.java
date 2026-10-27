package incari.features.schematics;

import incari.framework.game.Mirror;
import incari.framework.hud.Anchor;
import incari.framework.hud.HudCanvas;
import incari.framework.hud.HudElement;
import incari.framework.hud.TextElement;
import incari.framework.hud.Tone;
import java.util.ArrayList;
import java.util.List;

/**
 * The loaded schematic, how far it's built and how it's placed, in a few lines. The lines are made
 * again only when something they say changed ({@link #changed}), not each frame.
 */
final class StatusPanel extends HudElement {

    /** The widest a line may be before it's cut short. */
    static final int MAX_WIDTH = 170;

    private final SchematicsModule module;
    private final List<String> lines = new ArrayList<>();
    private final List<Tone> tones = new ArrayList<>();
    private boolean stale = true;
    private boolean editingLines;

    StatusPanel(SchematicsModule module) {
        super("status", "Schematic", Anchor.TOP_LEFT, 4, 60);
        this.module = module;
    }

    /** What it says may have changed: its lines are made again when next drawn. */
    void changed() {
        stale = true;
    }

    @Override
    public boolean isVisible() {
        return module.isLoaded();
    }

    private List<String> lines(HudCanvas canvas) {
        boolean editing = canvas.editing() && !module.isLoaded();
        if (stale || editing != editingLines) {
            stale = false;
            editingLines = editing;
            build(canvas, editing);
        }
        return lines;
    }

    private void build(HudCanvas canvas, boolean editing) {
        lines.clear();
        tones.clear();
        SchematicsModule.Loaded now = module.loaded();
        if (now == null) {
            if (editing) {
                add("Castle", Tone.ACCENT);
                add("1200 of 4000 placed", Tone.TEXT);
                add("3 wrong", Tone.WARNING);
            }
            return;
        }
        add(fit(canvas, now.schematic.name()), Tone.ACCENT);
        Verifier verifier = now.verifier;
        int total = now.schematic.blockCount();
        add(Math.min(verifier.right(), total) + " of " + total + " placed", Tone.TEXT);
        int wrong = verifier.wrongBlocks() + verifier.wrongStates();
        if (wrong > 0 || verifier.extra() > 0) {
            StringBuilder problems = new StringBuilder();
            if (wrong > 0) {
                problems.append(wrong).append(" wrong");
            }
            if (verifier.extra() > 0) {
                if (wrong > 0) {
                    problems.append(", ");
                }
                problems.append(verifier.extra()).append(" extra");
            }
            add(problems.toString(), Tone.WARNING);
        } else if (verifier.lookedAtAll() && verifier.right() >= total) {
            add("Finished", Tone.POSITIVE);
        }
        Placement placement = now.placement;
        String placed = placement.minX() + " " + placement.minY() + " " + placement.minZ();
        if (placement.turns() != 0) {
            placed += ", turned " + placement.turns() * 90 + "°";
        }
        if (placement.mirror() != Mirror.NONE) {
            placed += ", mirrored";
        }
        add(placed, Tone.MUTED);
        if (module.layers.get() != SchematicsModule.Layers.ALL) {
            add(
                    (module.layers.get() == SchematicsModule.Layers.ONE ? "Layer " : "Up to layer ") + module.layer(),
                    Tone.MUTED);
        }
    }

    private void add(String line, Tone tone) {
        lines.add(line);
        tones.add(tone);
    }

    /** {@code text} cut short with an ellipsis to fit {@link #MAX_WIDTH}. */
    static String fit(HudCanvas canvas, String text) {
        if (canvas.textWidth(text) <= MAX_WIDTH) {
            return text;
        }
        String cut = text;
        while (!cut.isEmpty() && canvas.textWidth(cut + "...") > MAX_WIDTH) {
            cut = cut.substring(0, cut.length() - 1);
        }
        return cut.strip() + "...";
    }

    @Override
    public int width(HudCanvas canvas) {
        int width = 0;
        for (String line : lines(canvas)) {
            width = Math.max(width, canvas.textWidth(line));
        }
        return width == 0 ? 0 : width + TextElement.PAD_X * 2;
    }

    @Override
    public int height(HudCanvas canvas) {
        int count = lines(canvas).size();
        return count == 0 ? 0 : count * canvas.lineHeight() + TextElement.PAD_Y * 2 - 1;
    }

    @Override
    public void draw(HudCanvas canvas) {
        List<String> text = lines(canvas);
        if (text.isEmpty()) {
            return;
        }
        canvas.fill(0, 0, width(canvas), height(canvas), Tone.BACKGROUND);
        for (int i = 0; i < text.size(); i++) {
            canvas.text(text.get(i), TextElement.PAD_X, TextElement.PAD_Y + i * canvas.lineHeight(), tones.get(i));
        }
    }
}
