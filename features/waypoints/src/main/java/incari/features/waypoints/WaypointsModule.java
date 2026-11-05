package incari.features.waypoints;

import incari.framework.Category;
import incari.framework.Module;
import incari.framework.event.Events;
import incari.framework.game.PlayerView;
import incari.framework.hud.Shape;
import incari.framework.hud.WorldCanvas;
import incari.framework.setting.ActionSetting;
import incari.framework.setting.BooleanSetting;
import incari.framework.setting.ChoiceSetting;
import incari.framework.setting.FloatSetting;
import incari.framework.setting.Key;
import incari.framework.setting.KeySetting;
import incari.framework.waypoint.Waypoint;
import incari.framework.waypoint.Waypoints;
import java.util.List;
import org.jspecify.annotations.Nullable;

/**
 * Waypoints: places the player marks in a world, each with a name, a color and an icon. Over the
 * world each shows its icon, its name and how far it is, and a beacon's beam rises from it; one off
 * screen is pointed to from the screen's edge. A key marks where the player stands, another lists
 * the world's waypoints, and where the player dies is marked for them.
 *
 * <p>The waypoints themselves are the framework's ({@code ModuleSystem.waypoints()}), kept for each
 * world; the minimap shows them too. This module draws them over the world and makes new ones.
 */
public final class WaypointsModule extends Module {

    /** When a waypoint's name and distance are written beside its icon. */
    public enum Labels {
        /** Every waypoint's. */
        ALWAYS("Always"),
        /** Only the one near the crosshair: the others show their icons. */
        LOOKED_AT("When looked at"),
        /** Never: icons only. */
        NEVER("Never");

        private final String label;

        Labels(String label) {
            this.label = label;
        }

        /** What the settings screen calls it. */
        public String label() {
            return label;
        }
    }

    /** How {@code Game.world()} names a server's world. */
    static final String SERVER_PREFIX = "server/";

    /** The name a death's waypoint gets. */
    static final String DEATH_NAME = "Death";

    /** A death's waypoint's color: the palette's red. */
    static final int DEATH_COLOR = Waypoint.COLORS.get(0);

    /** N: a New waypoint (B is voice chat's push to talk). */
    private static final int KEY_N = 17;

    /** U, as other clients' waypoint lists. */
    private static final int KEY_U = 24;

    final ActionSetting manage = settings()
            .action("manage", "This world's waypoints", "Manage...", this::openList)
            .describedAs("Lists this world's waypoints to add, change, hide or remove them.");
    final KeySetting newKey = settings()
            .key("new", "New waypoint key", Key.keyboard(KEY_N))
            .describedAs("Marks where you stand: opens a new waypoint to name.")
            .onPress(this::newWaypoint);
    final KeySetting listKey = settings()
            .key("list", "Waypoint list key", Key.keyboard(KEY_U))
            .describedAs("Opens this world's waypoints.")
            .onPress(this::openList);
    final ChoiceSetting<Labels> labels = settings()
            .choice("labels", "Names", Labels.values(), Labels.ALWAYS, Labels::label)
            .describedAs("When a waypoint's name shows above its icon.");
    final BooleanSetting distance =
            settings().bool("distance", "Show distance", true).describedAs("Shows the distance beside each name.");
    final BooleanSetting beams = settings()
            .bool("beams", "Beams", true)
            .describedAs("Shows a beacon beam on waypoints that have one turned on.");
    final BooleanSetting arrows = settings()
            .bool("arrows", "Arrows at the edge", true)
            .describedAs("Points to waypoints off the screen from its edge.");
    final FloatSetting size = settings()
            .decimal("size", "Size", 0.5F, 2.0F, 0.25F, 1.0F)
            .describedAs("How big icons and names are drawn over the world.");
    final BooleanSetting deaths = settings()
            .bool("deaths", "Mark deaths", true)
            .describedAs("Adds a waypoint where you die, replacing the last one; on a server, until you leave.");
    final BooleanSetting clearDeaths = settings()
            .bool("clear-deaths", "Clear deaths on arrival", true)
            .describedAs("Removes a death's waypoint once you get back to it.")
            .visibleWhen(deaths::get);

    /** Within this many blocks of a death's waypoint, the player has got back to it. */
    static final double ARRIVED = 4;

    /**
     * The player must first get this far away, so a respawn beside the place of death isn't an
     * arrival, but walking away and back is.
     */
    static final double AWAY = 8;

    private final WaypointOverlay overlay = new WaypointOverlay(this);
    private boolean wasDead;
    /** Whether the player has been {@link #AWAY} from the death's waypoint since it was made. */
    private boolean leftDeath;

    /** Waypoints, off until the player turns it on. */
    public WaypointsModule() {
        super(
                "waypoints",
                "Waypoints",
                "Marks places in the world, shown with their distance and a beacon beam.",
                Category.UTILITY);
        on(Events.OVERLAY, overlay);
        on(Events.WORLD, this::drawBeams);
        on(Events.TICK, this::watchForDeath);
    }

    /** The world's waypoints. */
    Waypoints store() {
        return system().waypoints();
    }

    /** The player, or null outside a world. */
    @Nullable
    PlayerView player() {
        return system().game().player();
    }

    /** Opens the waypoint list. */
    void openList() {
        if (store().world() != null) {
            system().game().openWaypoints();
        }
    }

    /**
     * Opens the editor on a new waypoint where the player stands: the next free "Waypoint N", the
     * palette's next color. It's added only if the player saves it.
     */
    void newWaypoint() {
        PlayerView player = player();
        Waypoints store = store();
        if (player == null || store.world() == null) {
            return;
        }
        Waypoint draft = new Waypoint(
                nextName(store.all()),
                floor(player.x()),
                floor(player.y()),
                floor(player.z()),
                player.dimension(),
                Waypoint.COLORS.get(store.all().size() % Waypoint.COLORS.size()),
                Shape.CIRCLE);
        system().game().editWaypoint(draft, true);
    }

    /** "Waypoint 1", or the first number after it no waypoint is called. */
    static String nextName(List<Waypoint> waypoints) {
        for (int n = waypoints.size() + 1; ; n++) {
            String name = "Waypoint " + n;
            boolean taken = false;
            for (Waypoint waypoint : waypoints) {
                if (waypoint.name().equals(name)) {
                    taken = true;
                    break;
                }
            }
            if (!taken) {
                return name;
            }
        }
    }

    /** Whether {@code waypoint} is shown to the player now: visible and in their dimension. */
    static boolean shown(Waypoint waypoint, PlayerView player) {
        return waypoint.isVisible() && waypoint.dimension().equals(player.dimension());
    }

    private void drawBeams(WorldCanvas canvas) {
        PlayerView player = player();
        if (!beams.get() || player == null) {
            return;
        }
        List<Waypoint> all = store().all();
        for (int i = 0; i < all.size(); i++) {
            Waypoint waypoint = all.get(i);
            if (waypoint.hasBeam() && shown(waypoint, player)) {
                canvas.beam(waypoint.x(), waypoint.y(), waypoint.z(), waypoint.rgb());
            }
        }
    }

    /** Once a tick: a death just now leaves a waypoint where it happened. */
    private void watchForDeath() {
        PlayerView player = player();
        boolean dead = player != null && player.isDead();
        if (player != null && dead && !wasDead && deaths.get()) {
            markDeath(player);
            leftDeath = false;
        }
        wasDead = dead;
        if (player != null && !dead && clearDeaths.get()) {
            clearOnArrival(player);
        }
    }

    /** Removes the death's waypoint once the player, having been away from it, gets back to it. */
    private void clearOnArrival(PlayerView player) {
        Waypoints store = store();
        Waypoint death = store.world() == null ? null : store.death();
        if (death == null || !death.dimension().equals(player.dimension())) {
            return;
        }
        double dx = player.x() - (death.x() + 0.5);
        double dy = player.y() - death.y();
        double dz = player.z() - (death.z() + 0.5);
        double distanceSquared = dx * dx + dy * dy + dz * dz;
        if (distanceSquared > AWAY * AWAY) {
            leftDeath = true;
        } else if (leftDeath && distanceSquared <= ARRIVED * ARRIVED) {
            store.remove(death);
            leftDeath = false;
        }
    }

    private void markDeath(PlayerView player) {
        Waypoints store = store();
        String world = store.world();
        if (world == null) {
            return;
        }
        Waypoint last = store.death();
        if (last != null) {
            store.remove(last);
        }
        Waypoint death = new Waypoint(
                DEATH_NAME,
                floor(player.x()),
                floor(player.y()),
                floor(player.z()),
                player.dimension(),
                DEATH_COLOR,
                Shape.CROSS);
        death.setDeath(true);
        // A server's address can be many maps (a network's lobby and its games, each with its own
        // places at the same coordinates): there a death is marked for this visit only, never kept
        // in the server's file to show up on the next map or the next day.
        death.setTemporary(onServer(world));
        store.add(death);
    }

    /** Whether {@code world} ({@code Game.world()}) is a server's, rather than singleplayer's. */
    static boolean onServer(String world) {
        return world.startsWith(SERVER_PREFIX);
    }

    private static int floor(double value) {
        return (int) Math.floor(value);
    }
}
