package incari.framework.waypoint;

import incari.framework.config.ConfigStorage;
import incari.framework.config.WaypointCodec;
import java.io.IOException;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Locale;
import java.util.function.Consumer;
import org.jspecify.annotations.Nullable;

/**
 * The waypoints of the world the player is in, every dimension's, kept in a file of the world's
 * own ({@code waypoints/<world>.json} beside the profiles): opened when the player joins a world,
 * closed when they leave. Waypoints aren't part of a profile, so switching profiles keeps them.
 *
 * <p>Game thread. Changes are saved a second after the last one, as profiles are, or at once by
 * {@link #flush()} and on {@link #close()}.
 */
public final class Waypoints {

    /** Ticks from the last change to the save: dragging through a color is one save. */
    static final int SAVE_DELAY_TICKS = 20;

    private final ConfigStorage storage;
    private final Consumer<String> log;
    private final List<Waypoint> list = new ArrayList<>();
    private final List<Waypoint> view = Collections.unmodifiableList(list);

    private @Nullable String world;
    private long revision;
    private int saveCountdown = -1;

    /**
     * The module system's.
     *
     * @param storage where the files are kept
     * @param log where problems go
     */
    public Waypoints(ConfigStorage storage, Consumer<String> log) {
        this.storage = storage;
        this.log = log;
    }

    /** The world whose waypoints these are ({@code Game.world()}), or null outside a world. */
    public @Nullable String world() {
        return world;
    }

    /**
     * Every waypoint of the world, in every dimension, in the order they were made. Unmodifiable;
     * changes go through {@link #add} and {@link #remove}.
     */
    public List<Waypoint> all() {
        return view;
    }

    /**
     * A number that goes up whenever a waypoint is added, removed or changed, or another world's
     * are opened: what draws them compares it to know when to refresh.
     */
    public long revision() {
        return revision;
    }

    /** Adds {@code waypoint} at the end; nothing outside a world. */
    public void add(Waypoint waypoint) {
        if (world == null || waypoint.owner() == this) {
            return;
        }
        Waypoints previous = waypoint.owner();
        if (previous != null) {
            previous.remove(waypoint);
        }
        list.add(waypoint);
        waypoint.attach(this);
        changed(waypoint.isTemporary());
    }

    /** Removes {@code waypoint}; nothing if it isn't here. */
    public void remove(Waypoint waypoint) {
        if (list.remove(waypoint)) {
            waypoint.attach(null);
            changed(waypoint.isTemporary());
        }
    }

    /** The waypoint marking the player's last death in this world, or null. */
    public @Nullable Waypoint death() {
        for (Waypoint waypoint : list) {
            if (waypoint.isDeath()) {
                return waypoint;
            }
        }
        return null;
    }

    /**
     * Opens {@code name}'s waypoints ({@code Game.world()}), saving and closing any open ones first.
     * A missing or unreadable file reads as none; an unreadable one isn't overwritten until
     * something changes.
     */
    public void open(String name) {
        if (name.equals(world)) {
            return;
        }
        close();
        world = name;
        try {
            String text = storage.read(path(name));
            if (text != null) {
                for (Waypoint waypoint : WaypointCodec.read(text, message -> log.accept(name + ": " + message))) {
                    list.add(waypoint);
                    waypoint.attach(this);
                }
            }
        } catch (IOException | RuntimeException e) {
            log.accept("Couldn't read the waypoints of " + name + ": " + e);
        }
        revision++;
    }

    /** Saves any change and forgets the world's waypoints: the player left it. */
    public void close() {
        flush();
        for (Waypoint waypoint : list) {
            waypoint.attach(null);
        }
        list.clear();
        world = null;
        saveCountdown = -1;
        revision++;
    }

    /**
     * Deletes {@code name}'s waypoints: the game deleted the world, and a new one with its name
     * mustn't find them. Nothing for the world open now.
     */
    public void delete(String name) {
        if (name.equals(world)) {
            return;
        }
        try {
            storage.delete(path(name));
        } catch (IOException | RuntimeException e) {
            log.accept("Couldn't delete the waypoints of " + name + ": " + e);
        }
    }

    /** Once a client tick: a due save. */
    public void tick() {
        if (saveCountdown > 0 && --saveCountdown == 0) {
            flush();
        }
    }

    /** Saves now if anything changed since the last save; {@linkplain Waypoint#isTemporary() temporary} ones aren't. */
    public void flush() {
        String name = world;
        if (saveCountdown < 0 || name == null) {
            return;
        }
        saveCountdown = -1;
        // A temporary waypoint is the visit's: never in the file.
        List<Waypoint> kept = new ArrayList<>(list.size());
        for (Waypoint waypoint : list) {
            if (!waypoint.isTemporary()) {
                kept.add(waypoint);
            }
        }
        try {
            if (kept.isEmpty()) {
                storage.delete(path(name));
            } else {
                storage.write(path(name), WaypointCodec.write(kept));
            }
        } catch (IOException | RuntimeException e) {
            log.accept("Couldn't save the waypoints of " + name + ": " + e);
        }
    }

    /**
     * A waypoint in the list changed, or the list did. A {@linkplain Waypoint#isTemporary()
     * temporary} one's change needs no save: the file never has it (and an unreadable file isn't
     * overwritten for it).
     */
    void changed(boolean temporary) {
        revision++;
        if (!temporary) {
            saveCountdown = SAVE_DELAY_TICKS;
        }
    }

    /**
     * The file a world's waypoints are kept in: its name with anything but letters, digits, dots
     * and dashes made an underscore, and a hash of it, so two names that read alike stay apart.
     */
    static String path(String world) {
        StringBuilder file = new StringBuilder(world.length() + 16).append("waypoints/");
        for (int i = 0; i < world.length() && i < 64; i++) {
            char c = world.charAt(i);
            boolean plain =
                    (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '.' || c == '-';
            file.append(plain ? c : '_');
        }
        return file.append('-')
                .append(Integer.toHexString(world.hashCode()).toLowerCase(Locale.ROOT))
                .append(".json")
                .toString();
    }
}
