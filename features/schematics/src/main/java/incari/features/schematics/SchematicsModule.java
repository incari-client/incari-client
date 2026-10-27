package incari.features.schematics;

import incari.features.schematics.format.BlockSpec;
import incari.features.schematics.format.Schematic;
import incari.features.schematics.format.SchematicException;
import incari.features.schematics.format.SchematicFiles;
import incari.framework.Category;
import incari.framework.Module;
import incari.framework.event.Events;
import incari.framework.game.Blocks;
import incari.framework.game.Game;
import incari.framework.game.GameKey;
import incari.framework.game.Mirror;
import incari.framework.game.PlayerView;
import incari.framework.hud.BlockGhosts;
import incari.framework.hud.WorldCanvas;
import incari.framework.setting.ActionSetting;
import incari.framework.setting.BooleanSetting;
import incari.framework.setting.ChoiceSetting;
import incari.framework.setting.ColorSetting;
import incari.framework.setting.IntSetting;
import incari.framework.setting.Key;
import incari.framework.setting.KeySetting;
import java.util.List;
import org.jspecify.annotations.Nullable;

/**
 * Schematics (ADR 0066): a structure from a file, placed in the world as see-through blocks to
 * build by. It's moved, turned and mirrored where it stands; what's still missing shows as ghosts,
 * the wrong blocks as boxes, and a list says what's left to place. Reads {@code .litematic}, Sponge
 * {@code .schem} and the game's structure {@code .nbt}. One schematic at a time, kept until another
 * is chosen or it's unloaded, whatever world the player goes to.
 */
public final class SchematicsModule extends Module {

    /** Where in the game directory the files are. */
    static final String FOLDER = "schematics";

    static final String TITLE = "Schematics";

    /** At most this many boxes a frame, nearest sections first: thousands would slow a Chromebook. */
    static final int MAX_BOXES = 1024;

    /** Ticks between the material list's counts. */
    static final int MATERIAL_TICKS = 10;

    /** What's shown as ghosts. */
    enum Ghosts {
        MISSING("Missing blocks"),
        ALL("Every block"),
        NONE("None");

        private final String label;

        Ghosts(String label) {
            this.label = label;
        }

        String label() {
            return label;
        }
    }

    /** Which of the schematic's layers are drawn. */
    enum Layers {
        ALL("All"),
        BELOW("Up to a layer"),
        ONE("One layer");

        private final String label;

        Layers(String label) {
            this.label = label;
        }

        String label() {
            return label;
        }
    }

    final ActionSetting choose = settings()
            .action("choose", "Schematic", "Choose...", this::chooseFile)
            .describedAs("Picks a schematic file to build: .litematic, .schem or .nbt.");
    final ActionSetting here = settings()
            .action("here", "Place", "Here", this::moveHere)
            .describedAs("Moves the schematic's corner to where you stand.")
            .visibleWhen(this::isLoaded);
    final ActionSetting turn = settings()
            .action("turn", "Turn", "90° right", () -> turn(1))
            .describedAs("Turns it a quarter clockwise where it stands.")
            .visibleWhen(this::isLoaded);
    final ActionSetting mirror = settings()
            .action("mirror", "Mirror", "Next", this::nextMirror)
            .describedAs("Mirrors it east-west, then north-south, then not.")
            .visibleWhen(this::isLoaded);
    final ActionSetting unload = settings()
            .action("unload", "Unload", "Unload", this::unloadSchematic)
            .describedAs("Takes the schematic out of the world.")
            .visibleWhen(this::isLoaded);
    final ChoiceSetting<Ghosts> ghosts = settings()
            .choice("ghosts", "Ghosts", Ghosts.values(), Ghosts.MISSING, Ghosts::label)
            .describedAs("Which of the schematic's blocks show as see-through ghosts.");
    final IntSetting opacity = settings()
            .integer("opacity", "Ghost opacity", 10, 90, 45)
            .formattedAs(value -> value + "%")
            .describedAs("How solid the ghosts are.");
    final IntSetting range = settings()
            .integer("range", "Range", 16, 128, 48)
            .formattedAs(value -> value + " blocks")
            .describedAs("How far from you ghosts and boxes show; nearer is faster.");
    final ChoiceSetting<Layers> layers = settings()
            .choice("layers", "Layers", Layers.values(), Layers.ALL, Layers::label)
            .describedAs("Shows the whole schematic, or one layer to build by.");
    final BooleanSetting outline =
            settings().bool("outline", "Show its box", true).describedAs("Outlines the schematic's whole box.");
    final ColorSetting outlineColor = settings()
            .color("outline-color", "Box color", 0x55AAFF)
            .describedAs("The outline's color.")
            .visibleWhen(outline::get);
    final BooleanSetting highlight = settings()
            .bool("highlight", "Mark wrong blocks", true)
            .describedAs("Boxes blocks that differ from the schematic.");
    final ColorSetting wrongColor = settings()
            .color("wrong-color", "Wrong block", 0xFF3333)
            .describedAs("A different block from the schematic's.")
            .visibleWhen(highlight::get);
    final ColorSetting stateColor = settings()
            .color("state-color", "Wrong way", 0xFFAA00)
            .describedAs("The right block, but facing or set another way.")
            .visibleWhen(highlight::get);
    final ColorSetting extraColor = settings()
            .color("extra-color", "Extra block", 0xFF55FF)
            .describedAs("A block where the schematic has air.")
            .visibleWhen(highlight::get);
    final KeySetting hereKey = settings()
            .key("here-key", "Place here key", Key.NONE)
            .describedAs("Moves the schematic's corner to where you stand.")
            .onPress(this::moveHere);
    final KeySetting turnKey = settings()
            .key("turn-key", "Turn key", Key.NONE)
            .describedAs("Turns it a quarter clockwise; anticlockwise while sneaking.")
            .onPress(() -> turn(sneaking() ? -1 : 1));
    final KeySetting mirrorKey = settings()
            .key("mirror-key", "Mirror key", Key.NONE)
            .describedAs("Mirrors it east-west, then north-south, then not.")
            .onPress(this::nextMirror);
    final KeySetting nudgeKey = settings()
            .key("nudge-key", "Nudge key", Key.NONE)
            .describedAs("Moves it a block the way you face; back while sneaking.")
            .onPress(this::nudge);
    final KeySetting raiseKey = settings()
            .key("raise-key", "Up key", Key.NONE)
            .describedAs("Moves it a block up; down while sneaking.")
            .onPress(() -> raise(sneaking() ? -1 : 1));
    final KeySetting layerKey = settings()
            .key("layer-key", "Layer key", Key.NONE)
            .describedAs("Shows the next layer up; down while sneaking.")
            .onPress(() -> moveLayer(sneaking() ? -1 : 1));

    private @Nullable Loaded loaded;

    /** The layer drawn, in world y, while {@link #layers} isn't All. */
    private int layer;

    /** Goes up whenever what the ghosts show might have changed everywhere. */
    private long ghostRevision = 1;

    private final GhostView ghostView = new GhostView();
    private long ticks;
    private final StatusPanel status = new StatusPanel(this);
    private final MaterialPanel materials = new MaterialPanel(this);

    /** A schematic loaded and placed: what's worked out from it once, and again as it moves. */
    static final class Loaded {

        final Schematic schematic;
        final Placement placement;

        /** The game's state for each palette index, as the file has it; UNKNOWN where the game has none. */
        final int[] baseStates;

        /** {@link #baseStates} mirrored and turned as placed. */
        final int[] states;

        final Materials materials;
        Verifier verifier;
        final int unknownKinds;

        Loaded(Schematic schematic, Placement placement, Blocks blocks) {
            this.schematic = schematic;
            this.placement = placement;
            List<BlockSpec> palette = schematic.palette();
            baseStates = new int[palette.size()];
            states = new int[palette.size()];
            int unknown = 0;
            for (int i = 0; i < baseStates.length; i++) {
                BlockSpec spec = palette.get(i);
                baseStates[i] = spec.isNothing()
                        ? Blocks.UNKNOWN
                        : blocks.state(spec.name(), spec.properties(), schematic.dataVersion());
                if (!spec.isNothing() && baseStates[i] == Blocks.UNKNOWN) {
                    unknown++;
                }
            }
            unknownKinds = unknown;
            materials = new Materials(schematic, baseStates, blocks);
            verifier = place(blocks);
        }

        /** Works the states out again for how it's placed now, and starts comparing afresh. */
        Verifier place(Blocks blocks) {
            for (int i = 0; i < states.length; i++) {
                int base = baseStates[i];
                states[i] = base == Blocks.UNKNOWN || blocks.isAir(base)
                        ? base
                        : blocks.transform(base, placement.mirror(), placement.turns());
            }
            verifier = new Verifier(placement, states, blocks);
            return verifier;
        }
    }

    /** Schematics, off until the player turns it on. */
    public SchematicsModule() {
        super(
                "schematics",
                "Schematics",
                "Places a schematic in the world as ghost blocks to build by.",
                Category.UTILITY);
        hud(status);
        hud(materials);
        on(Events.TICK, this::tick);
        on(Events.WORLD, this::draw);
        on(Events.LEAVE, this::left);
        ghosts.onChange(this::ghostsChanged);
        layers.onChange(this::layersChanged);
    }

    /** The game, for the HUD elements. */
    Game game() {
        return system().game();
    }

    /** Whether a schematic is loaded. */
    boolean isLoaded() {
        return loaded != null;
    }

    @Nullable
    Loaded loaded() {
        return loaded;
    }

    /** The layer drawn, in world y. */
    int layer() {
        return layer;
    }

    private void chooseFile() {
        system().game().chooseFile("Choose a schematic", FOLDER, SchematicFiles.EXTENSIONS, this::load);
    }

    /** Reads {@code bytes}, the file {@code fileName}, and places it where the player stands. */
    void load(String fileName, byte[] bytes) {
        Schematic schematic;
        try {
            schematic = SchematicFiles.read(fileName, bytes);
        } catch (SchematicException e) {
            system().game().showMessage(TITLE, "Couldn't load " + fileName + ": " + e.getMessage());
            return;
        }
        PlayerView player = system().game().player();
        int x = player == null ? 0 : floor(player.x());
        int y = player == null ? 64 : floor(player.y());
        int z = player == null ? 0 : floor(player.z());
        Loaded now = new Loaded(
                schematic, new Placement(schematic, x, y, z), system().game().blocks());
        loaded = now;
        layer = y;
        ghostRevision++;
        String size = schematic.sizeX() + " x " + schematic.sizeY() + " x " + schematic.sizeZ();
        String message = schematic.name() + ", " + size + ", placed where you stand.";
        if (now.unknownKinds > 0) {
            message += " " + now.unknownKinds + (now.unknownKinds == 1 ? " block isn't" : " blocks aren't")
                    + " in this version: left out.";
        }
        system().game().showMessage(TITLE, message);
        status.changed();
        materials.changed();
    }

    private void unloadSchematic() {
        loaded = null;
        ghostRevision++;
        status.changed();
        materials.changed();
    }

    private boolean sneaking() {
        return system().game().isDown(GameKey.SNEAK);
    }

    private void moveHere() {
        Loaded now = loaded;
        PlayerView player = system().game().player();
        if (now == null || player == null) {
            return;
        }
        now.placement.moveTo(floor(player.x()), floor(player.y()), floor(player.z()));
        placed(now);
    }

    /** Moves it a block the way the player faces, or back while they sneak. */
    private void nudge() {
        Loaded now = loaded;
        PlayerView player = system().game().player();
        if (now == null || player == null) {
            return;
        }
        // The heading's nearest of north, east, south, west.
        int facing = Math.floorMod(Math.round(player.heading() / 90.0F), 4);
        int step = sneaking() ? -1 : 1;
        int dx = facing == 1 ? step : facing == 3 ? -step : 0;
        int dz = facing == 2 ? step : facing == 0 ? -step : 0;
        now.placement.moveBy(dx, 0, dz);
        placed(now);
    }

    private void raise(int blocks) {
        Loaded now = loaded;
        if (now == null) {
            return;
        }
        now.placement.moveBy(0, blocks, 0);
        layer += blocks;
        placed(now);
    }

    void turn(int quarterTurns) {
        Loaded now = loaded;
        if (now == null) {
            return;
        }
        now.placement.turn(quarterTurns);
        placed(now);
    }

    void nextMirror() {
        Loaded now = loaded;
        if (now == null) {
            return;
        }
        now.placement.mirror(
                switch (now.placement.mirror()) {
                    case NONE -> Mirror.X;
                    case X -> Mirror.Z;
                    case Z -> Mirror.NONE;
                });
        placed(now);
    }

    private void moveLayer(int by) {
        Loaded now = loaded;
        if (now == null) {
            return;
        }
        if (layers.get() == Layers.ALL) {
            layers.set(Layers.ONE);
        }
        layer = Math.max(now.placement.minY(), Math.min(now.placement.maxY() - 1, layer + by));
        ghostRevision++;
        status.changed();
    }

    /** It moved, turned or mirrored: everything's worked out again. */
    private void placed(Loaded now) {
        now.place(system().game().blocks());
        layer = Math.max(now.placement.minY(), Math.min(now.placement.maxY() - 1, layer));
        ghostRevision++;
        status.changed();
        materials.changed();
    }

    private void ghostsChanged() {
        ghostRevision++;
    }

    private void layersChanged() {
        Loaded now = loaded;
        PlayerView player = system().game().player();
        if (now != null && layers.get() != Layers.ALL && player != null) {
            // Start at the player's feet: the layer they're likely building.
            layer = Math.max(now.placement.minY(), Math.min(now.placement.maxY() - 1, floor(player.y())));
        }
        ghostRevision++;
        status.changed();
    }

    /** Whether world layer {@code y} is drawn. */
    boolean shows(int y) {
        return switch (layers.get()) {
            case ALL -> true;
            case BELOW -> y <= layer;
            case ONE -> y == layer;
        };
    }

    /** The world changed under it: what was found is forgotten, and looked for again. */
    private void left() {
        Loaded now = loaded;
        if (now != null) {
            now.place(system().game().blocks());
            ghostRevision++;
            materials.changed();
        }
    }

    private void tick() {
        Loaded now = loaded;
        PlayerView player = system().game().player();
        if (now == null || player == null) {
            return;
        }
        now.verifier.tick(floor(player.x()), floor(player.y()), floor(player.z()), range.get());
        if (++ticks % MATERIAL_TICKS == 0) {
            now.materials.refresh(now.verifier);
            status.changed();
            materials.changed();
        }
    }

    private void draw(WorldCanvas canvas) {
        Loaded now = loaded;
        if (now == null) {
            return;
        }
        Placement placement = now.placement;
        if (ghosts.get() != Ghosts.NONE) {
            canvas.ghosts(ghostView, Math.round(opacity.get() * 255 / 100.0F), range.get());
        }
        if (outline.get()) {
            canvas.box(
                    placement.minX(),
                    placement.minY(),
                    placement.minZ(),
                    placement.maxX(),
                    placement.maxY(),
                    placement.maxZ(),
                    0xFF000000 | outlineColor.get(),
                    0);
        }
        if (highlight.get()) {
            drawProblems(canvas, now.verifier);
        }
    }

    /** The wrong and extra blocks near the camera, as boxes. */
    private void drawProblems(WorldCanvas canvas, Verifier verifier) {
        int cx = floor(canvas.cameraX());
        int cy = floor(canvas.cameraY());
        int cz = floor(canvas.cameraZ());
        int reach = range.get();
        int boxes = 0;
        // Ring by ring outward, so the cap leaves out the farthest.
        int rings = (reach >> 4) + 1;
        for (int ring = 0; ring <= rings && boxes < MAX_BOXES; ring++) {
            for (int sy = (cy >> 4) - ring; sy <= (cy >> 4) + ring && boxes < MAX_BOXES; sy++) {
                for (int sz = (cz >> 4) - ring; sz <= (cz >> 4) + ring && boxes < MAX_BOXES; sz++) {
                    for (int sx = (cx >> 4) - ring; sx <= (cx >> 4) + ring && boxes < MAX_BOXES; sx++) {
                        int edge = Math.max(
                                Math.abs(sx - (cx >> 4)), Math.max(Math.abs(sy - (cy >> 4)), Math.abs(sz - (cz >> 4))));
                        if (edge != ring) {
                            continue;
                        }
                        Verifier.Section section = verifier.section(sx, sy, sz);
                        if (section != null && section.issueCount > 0) {
                            boxes = drawSection(canvas, section, cx, cy, cz, reach, boxes);
                        }
                    }
                }
            }
        }
    }

    private int drawSection(
            WorldCanvas canvas, Verifier.Section section, int cx, int cy, int cz, int reach, int boxes) {
        long reachSquared = (long) reach * reach;
        int count = boxes;
        for (int i = 0; i < section.issueCount && count < MAX_BOXES; i++) {
            int issue = section.issue(i);
            int at = issue >>> 3;
            int x = (section.sectionX << 4) | (at & 15);
            int y = (section.sectionY << 4) | (at >> 8);
            int z = (section.sectionZ << 4) | ((at >> 4) & 15);
            long dx = (long) x - cx;
            long dy = (long) y - cy;
            long dz = (long) z - cz;
            if (!shows(y) || dx * dx + dy * dy + dz * dz > reachSquared) {
                continue;
            }
            int rgb =
                    switch (issue & 7) {
                        case Verifier.WRONG_BLOCK -> wrongColor.get();
                        case Verifier.WRONG_STATE -> stateColor.get();
                        default -> extraColor.get();
                    };
            canvas.box(x, y, z, x + 1, y + 1, z + 1, 0xFF000000 | rgb, 0x40000000 | rgb);
            count++;
        }
        return count;
    }

    static int floor(double value) {
        return (int) Math.floor(value);
    }

    /** What the client meshes as ghosts: the placed schematic's blocks still to build, or all of them. */
    private final class GhostView implements BlockGhosts {

        @Override
        public int minX() {
            Loaded now = loaded;
            return now == null ? 0 : now.placement.minX();
        }

        @Override
        public int minY() {
            Loaded now = loaded;
            return now == null ? 0 : now.placement.minY();
        }

        @Override
        public int minZ() {
            Loaded now = loaded;
            return now == null ? 0 : now.placement.minZ();
        }

        @Override
        public int maxX() {
            Loaded now = loaded;
            return now == null ? 0 : now.placement.maxX();
        }

        @Override
        public int maxY() {
            Loaded now = loaded;
            return now == null ? 0 : now.placement.maxY();
        }

        @Override
        public int maxZ() {
            Loaded now = loaded;
            return now == null ? 0 : now.placement.maxZ();
        }

        @Override
        public int ghostAt(int x, int y, int z) {
            Loaded now = loaded;
            if (now == null || !shows(y)) {
                return Blocks.AIR;
            }
            int index = now.placement.indexAt(x, y, z);
            if (index == 0) {
                return Blocks.AIR;
            }
            int want = now.states[index];
            Blocks blocks = system().game().blocks();
            if (want == Blocks.UNKNOWN || blocks.isAir(want)) {
                return Blocks.AIR;
            }
            if (ghosts.get() == Ghosts.ALL) {
                return want;
            }
            int have = blocks.at(x, y, z);
            return have != Blocks.UNKNOWN && blocks.isAir(have) ? want : Blocks.AIR;
        }

        @Override
        public long revision(int sectionX, int sectionY, int sectionZ) {
            Loaded now = loaded;
            Verifier.Section section = now == null ? null : now.verifier.section(sectionX, sectionY, sectionZ);
            return ghostRevision << 32 | (section == null ? 0 : section.revision & 0xFFFFFFFFL);
        }
    }
}
