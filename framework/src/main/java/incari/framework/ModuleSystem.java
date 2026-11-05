package incari.framework;

import incari.framework.config.ConfigStorage;
import incari.framework.config.ProfileCodec;
import incari.framework.config.Profiles;
import incari.framework.event.Channel;
import incari.framework.event.EventBus;
import incari.framework.event.Events;
import incari.framework.game.Game;
import incari.framework.game.GamePart;
import incari.framework.hud.GamePartElement;
import incari.framework.hud.HudCanvas;
import incari.framework.hud.HudElement;
import incari.framework.online.Online;
import incari.framework.setting.KeySetting;
import incari.framework.setting.Setting;
import incari.framework.waypoint.Waypoints;
import java.io.IOException;
import java.util.ArrayList;
import java.util.EnumSet;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.jspecify.annotations.Nullable;

/**
 * Every module, the events they listen to, their keybinds, their HUD and the profiles that store
 * them. The client makes one at start-up with the build's modules and calls in from the game's
 * hooks; everything else (screens, the HUD editor, server feature control) goes through it.
 *
 * <p>Game thread only. Changes are saved a second after the last one ({@link #tick}), or at once
 * by {@link #flush()}; the browser's files are in memory and stored behind (ADR 0011), so a save
 * never waits on the disk.
 */
public final class ModuleSystem {

    /** Where the system reports problems: the client's log. */
    public interface Log {

        /** Something went wrong; {@code error} is null if there's no exception. */
        void warn(String message, @Nullable Throwable error);
    }

    /** Ticks from the last change to the save: a slider's drag is one save, not dozens. */
    static final int SAVE_DELAY_TICKS = 20;

    /**
     * The game's effect icons in the top right: four 25-unit boxes in from the right edge, and two
     * rows (helpful, then harmful) 52 units tall from the top.
     */
    static final int EFFECT_ICONS_WIDTH = 100;

    static final int EFFECT_ICONS_HEIGHT = 52;

    /** {@link #effectIconsShift}'s scratch: an element's bounds. */
    private final int[] iconBounds = new int[4];

    private final List<Module> modules;
    private final EventBus events;
    private final Profiles profiles;
    private final Log log;
    private final Game game;
    private final Online online;
    private final Waypoints waypoints;

    /** Every keybind, with the module it belongs to at the same index. */
    private final KeySetting[] keys;

    private final Module[] keyOwners;

    /** Whether each keybind is its module's toggle key, which fires while the module is off. */
    private final boolean[] toggleKeys;

    /** The HUD elements of enabled modules drawn with the HUD: the only ones drawn there. */
    private HudElement[] activeHud = new HudElement[0];

    private Module[] activeHudOwners = new Module[0];

    /** Those drawn every frame ({@link HudElement#isLive()}). */
    private HudElement[] liveHud = new HudElement[0];

    private Module[] liveHudOwners = new Module[0];

    /** The parts of the game's HUD an enabled module's element draws. */
    private final EnumSet<GamePart> heldParts = EnumSet.noneOf(GamePart.class);

    /** The modules that are switches for resource packs ({@link Module#followPack}). */
    private final Module[] packSwitches;

    private long revision;
    private int saveCountdown = -1;
    private boolean loading;

    /**
     * True while {@link #load} applies the profile the client started with: only then is what the
     * profile says of the game's state (a held option's player value) current.
     */
    private boolean startingUp;

    /**
     * True while the profile first loads and while a module follows the game's packs: the module's
     * state comes from the game's, not the other way round.
     */
    private boolean followingGame;

    /** With no game and no online service; see the full constructor. */
    public ModuleSystem(List<? extends Module> modules, ConfigStorage storage, Log log) {
        this(modules, storage, log, Game.NONE);
    }

    /** With no online service; see the full constructor. */
    public ModuleSystem(List<? extends Module> modules, ConfigStorage storage, Log log, Game game) {
        this(modules, storage, log, game, Online.NONE);
    }

    /**
     * @param modules every module of the build, in menu order within a category; each in no other
     *     system
     * @param storage where profiles are kept
     * @param log where problems go
     * @param game what modules can read of the game
     * @param online the player's Incari account and the Incari players around them (ADR 0058)
     */
    public ModuleSystem(List<? extends Module> modules, ConfigStorage storage, Log log, Game game, Online online) {
        this.modules = List.copyOf(modules);
        this.log = log;
        this.game = game;
        this.online = online;
        this.events = new EventBus(this::listenerFailed);
        this.profiles = new Profiles(storage, this.modules, message -> log.warn(message, null));
        this.waypoints = new Waypoints(storage, message -> log.warn(message, null));
        List<KeySetting> keyList = new ArrayList<>();
        List<Module> ownerList = new ArrayList<>();
        List<Module> switches = new ArrayList<>();
        Set<String> ids = new HashSet<>();
        for (Module module : this.modules) {
            if (!ids.add(module.id())) {
                throw new IllegalArgumentException("Two modules with the id " + module.id());
            }
            module.register(this);
            module.settings().attach(this::changed);
            if (module.pack() != null) {
                switches.add(module);
            }
            for (HudElement element : module.hudElements()) {
                element.settings().attach(this::changed);
            }
            for (Setting<?> setting : module.settings()) {
                if (setting instanceof KeySetting key) {
                    keyList.add(key);
                    ownerList.add(module);
                }
            }
        }
        this.packSwitches = switches.toArray(new Module[0]);
        this.keys = keyList.toArray(new KeySetting[0]);
        this.keyOwners = ownerList.toArray(new Module[0]);
        this.toggleKeys = new boolean[keys.length];
        for (int i = 0; i < keys.length; i++) {
            toggleKeys[i] = keys[i].equals(keyOwners[i].toggleKey());
        }
    }

    /** Every module, in the order given. */
    public List<Module> modules() {
        return modules;
    }

    /** The module with this id, or null. */
    public @Nullable Module module(String id) {
        for (Module module : modules) {
            if (module.id().equals(id)) {
                return module;
            }
        }
        return null;
    }

    /** The events, for the game's hooks to fire. */
    public EventBus events() {
        return events;
    }

    /** What modules can read of the game. */
    public Game game() {
        return game;
    }

    /**
     * The player's Incari account and which players in the world are Incari players (ADR 0058).
     * Accounts aren't a module: they're there, or not, whatever the profile says.
     */
    public Online online() {
        return online;
    }

    /**
     * The waypoints of the world the player is in. The client opens a world's as the player joins
     * it and closes them as they leave.
     */
    public Waypoints waypoints() {
        return waypoints;
    }

    /**
     * Whether an enabled module draws the game's {@code part} itself: the game then leaves it out
     * where it would draw it. A field read; the HUD asks every frame.
     */
    public boolean holds(GamePart part) {
        return heldParts.contains(part);
    }

    /**
     * A number that goes up whenever a module is turned on or off, a setting changes or a profile is
     * applied: a screen compares it to know when to refresh.
     */
    public long revision() {
        return revision;
    }

    /**
     * Applies the active profile, before the first tick; a failure leaves every default. A resource
     * pack's switch follows the pack, not the profile: the resources already loaded with the packs,
     * and changing them would reload them all.
     */
    public void load() {
        loading = true;
        startingUp = true;
        followingGame = true;
        try {
            profiles.load();
        } catch (IOException | RuntimeException e) {
            log.warn("Couldn't load the module profiles", e);
        }
        try {
            followPacks();
            // Always-on modules are on even when the profile couldn't be read.
            for (Module module : modules) {
                if (module.isAlwaysOn()) {
                    module.setEnabled(true);
                }
            }
        } finally {
            loading = false;
            startingUp = false;
            followingGame = false;
        }
        revision++;
    }

    /**
     * A client tick: keybinds fire, then {@link incari.framework.event.Events#TICK}'s listeners, then
     * a due save.
     */
    public void tick(KeyInput input) {
        followPacks();
        boolean toggled = false;
        for (int i = 0; i < keys.length; i++) {
            KeySetting key = keys[i];
            Module owner = keyOwners[i];
            // Every key's presses are taken, so a module turned on later doesn't see stale ones.
            // An unbound key is never pressed, whatever the input says of "no key".
            boolean bound = key.get().isBound();
            int presses = bound ? input.presses(key) : 0;
            boolean live = owner.isEnabled() || toggleKeys[i];
            key.update(bound && live && input.isDown(key));
            for (int p = 0; p < presses && live; p++) {
                try {
                    if (toggleKeys[i]) {
                        owner.toggle();
                        toggled = true;
                    } else {
                        key.press();
                    }
                } catch (RuntimeException e) {
                    failed(owner, "its key " + key.id(), e);
                }
                live = owner.isEnabled() || toggleKeys[i];
            }
        }
        events.fireTick();
        if (toggled) {
            // A key's toggle is one change, not a drag: saved now, as the tab may close next.
            flush();
        } else if (saveCountdown > 0 && --saveCountdown == 0) {
            flush();
        }
        waypoints.tick();
    }

    /** The menu buttons of every enabled module, in module order: the title's and pause menu's. */
    public List<MenuButton> menuButtons() {
        List<MenuButton> buttons = new ArrayList<>();
        for (Module module : modules) {
            if (module.isEnabled()) {
                buttons.addAll(module.menuButtons());
            }
        }
        return buttons;
    }

    /** Whether any HUD element could draw: false lets the game skip the HUD hook's setup. */
    public boolean hasHud() {
        return activeHud.length > 0;
    }

    /**
     * Whether anything is drawn every frame: a live HUD element ({@link HudElement#isLive()}) or a
     * listener to {@link incari.framework.event.Events#OVERLAY}.
     */
    public boolean hasLiveHud() {
        return liveHud.length > 0 || events.count(Events.OVERLAY) > 0;
    }

    /**
     * How far down the game draws its effect icons, in GUI units, so they clear the HUD elements in
     * their place in the top right (the minimap): below the lowest in the way, but never off the
     * bottom of the screen. 0 leaves them where the game puts them. Render thread, as the game's HUD
     * draws them; reads where each element was last drawn, allocating nothing.
     */
    public int effectIconsShift(int screenWidth, int screenHeight) {
        int shift = 0;
        int lowest = Math.max(0, screenHeight - EFFECT_ICONS_HEIGHT);
        // Each element in the way moves them below it, until none is: a handful of elements.
        for (int pass = 0; pass <= activeHud.length + liveHud.length; pass++) {
            int below = Math.max(below(activeHud, shift, screenWidth), below(liveHud, shift, screenWidth));
            if (below <= shift) {
                break;
            }
            if (below > lowest) {
                return lowest;
            }
            shift = below;
        }
        return shift;
    }

    /** The lowest bottom of the {@code elements} over the icons' place {@code shift} down, plus a gap. */
    private int below(HudElement[] elements, int shift, int screenWidth) {
        int[] b = iconBounds;
        int lowest = shift;
        for (HudElement element : elements) {
            if (element.lastDrawn(b)
                    && b[0] + b[2] > screenWidth - EFFECT_ICONS_WIDTH
                    && b[1] < shift + EFFECT_ICONS_HEIGHT
                    && b[1] + b[3] > shift) {
                lowest = Math.max(lowest, b[1] + b[3] + 1);
            }
        }
        return lowest;
    }

    /**
     * Draws every enabled module's visible HUD elements, but for the live ones. Render thread, during
     * the HUD's extraction.
     */
    public void drawHud(HudCanvas canvas) {
        draw(canvas, activeHud, activeHudOwners);
    }

    /** Draws every enabled module's visible live HUD elements. Render thread, every frame. */
    public void drawLiveHud(HudCanvas canvas) {
        draw(canvas, liveHud, liveHudOwners);
    }

    private void draw(HudCanvas canvas, HudElement[] elements, Module[] owners) {
        for (int i = 0; i < elements.length; i++) {
            try {
                elements[i].render(canvas);
            } catch (RuntimeException e) {
                failed(owners[i], "its HUD element " + elements[i].id(), e);
            }
        }
    }

    /**
     * Draws one of {@code owner}'s HUD elements, as {@link #drawHud} does: the HUD editor's, which
     * draws each element over its own frame. A throw turns the module off, as there.
     */
    public void drawHud(HudCanvas canvas, Module owner, HudElement element) {
        try {
            element.render(canvas);
        } catch (RuntimeException e) {
            failed(owner, "its HUD element " + element.id(), e);
        }
    }

    /** The active profile's name. */
    public String activeProfile() {
        return profiles.active();
    }

    /** Every profile's name. */
    public List<String> profiles() {
        return profiles.names();
    }

    /** Saves the active profile, then applies {@code name}. False if there's no such profile. */
    public boolean switchProfile(String name) {
        return profileChange("switch to profile " + name, () -> profiles.switchTo(name));
    }

    /** Makes a copy of the active profile called {@code name} and switches to it. False if it can't. */
    public boolean createProfile(String name) {
        return profileChange("create profile " + name, () -> profiles.create(name));
    }

    /** Deletes a profile; the first left becomes active if it was. False if it's the only one. */
    public boolean deleteProfile(String name) {
        return profileChange("delete profile " + name, () -> profiles.delete(name));
    }

    /**
     * Applies a preset (1.8 PvP, Modern PvP, QoL): switches to the profile {@code name} (copying the
     * active one if it doesn't exist), then turns on exactly the modules in {@code on} and the rest
     * off. Always-on modules aren't switched, and unknown ids are skipped. Settings stay as they were
     * except those in {@code settings}, which maps {@code "module.setting"} to config text (e.g.
     * {@code "togglesprint.sprint"} to {@code "always"}). False if the profile can't be made.
     */
    public boolean applyPreset(String name, Set<String> on, Map<String, String> settings) {
        boolean ready =
                profiles.names().stream().anyMatch(name::equalsIgnoreCase) ? switchProfile(name) : createProfile(name);
        if (!ready) {
            return false;
        }
        for (Module module : modules) {
            if (!module.isAlwaysOn()) {
                module.setEnabled(on.contains(module.id()));
            }
            for (Setting<?> setting : module.settings()) {
                String text = settings.get(module.id() + "." + setting.id());
                if (text != null && !ProfileCodec.setFromText(setting, text)) {
                    log.warn(
                            "Preset " + name + " has a value " + module.id() + "." + setting.id() + " can't take: "
                                    + text,
                            null);
                }
            }
        }
        // The module states are the change: saved like any other.
        changed();
        return true;
    }

    /** {@link #applyPreset(String, Set, Map)} with no settings changed. */
    public boolean applyPreset(String name, Set<String> on) {
        return applyPreset(name, on, Map.of());
    }

    /** Saves the profile and the waypoints now if anything changed since the last save. */
    public void flush() {
        waypoints.flush();
        if (saveCountdown < 0) {
            return;
        }
        saveCountdown = -1;
        try {
            profiles.save();
        } catch (IOException | RuntimeException e) {
            log.warn("Couldn't save module profile " + profiles.active(), e);
        }
    }

    /**
     * Turns each pack's switch on or off as the game has its pack: the player may have changed it
     * in the Resource Packs screen, or a pack asked for may not be there. A few list lookups a tick.
     */
    private void followPacks() {
        boolean was = followingGame;
        followingGame = true;
        try {
            for (Module module : packSwitches) {
                String pack = module.pack();
                if (pack != null) {
                    module.setEnabled(game.packSelected(pack));
                }
            }
        } finally {
            followingGame = was;
        }
    }

    /** Whether the profile the client started with is being applied ({@link #load}). */
    boolean startingUp() {
        return startingUp;
    }

    /** {@link Module#setEnabled}'s report for a pack's switch: the pack goes with it. */
    void selectPack(Module module, String pack, boolean on) {
        if (followingGame || game.packSelected(pack) == on) {
            return;
        }
        if (!game.selectPack(pack, on)) {
            // The next tick turns the module back as the pack is.
            log.warn(
                    "Module " + module.id() + " couldn't turn the resource pack " + pack + (on ? " on" : " off"), null);
        }
    }

    /** {@link Module#setEnabled}'s report. */
    void enabledChanged(Module module) {
        List<HudElement> elements = new ArrayList<>();
        List<Module> owners = new ArrayList<>();
        List<HudElement> live = new ArrayList<>();
        List<Module> liveOwners = new ArrayList<>();
        heldParts.clear();
        for (Module m : modules) {
            if (m.isEnabled()) {
                for (HudElement element : m.hudElements()) {
                    (element.isLive() ? live : elements).add(element);
                    (element.isLive() ? liveOwners : owners).add(m);
                    if (element instanceof GamePartElement part) {
                        heldParts.add(part.part());
                    }
                }
            }
        }
        activeHud = elements.toArray(new HudElement[0]);
        activeHudOwners = owners.toArray(new Module[0]);
        liveHud = live.toArray(new HudElement[0]);
        liveHudOwners = liveOwners.toArray(new Module[0]);
        changed();
    }

    private void changed() {
        revision++;
        if (!loading) {
            saveCountdown = SAVE_DELAY_TICKS;
        }
    }

    private interface ProfileAction {
        boolean run() throws IOException;
    }

    private boolean profileChange(String what, ProfileAction action) {
        loading = true;
        try {
            // What the action applies isn't a change to save; a change pending from before still is.
            return action.run();
        } catch (IOException | RuntimeException e) {
            log.warn("Couldn't " + what, e);
            return false;
        } finally {
            loading = false;
            revision++;
        }
    }

    private void listenerFailed(Object owner, Channel<?> channel, RuntimeException error) {
        failed((Module) owner, "a " + channel.name() + " event", error);
    }

    /**
     * A module threw: it's turned off (it would likely throw again every tick), the error logged,
     * and the player told, so it isn't left off without their knowing.
     */
    void failed(Module module, String where, RuntimeException error) {
        log.warn("Module " + module.id() + " failed in " + where + " and was turned off", error);
        boolean wasOn = module.isEnabled();
        try {
            module.disableAfterFailure();
        } catch (RuntimeException again) {
            log.warn("Module " + module.id() + " failed again while turning off", again);
        }
        if (wasOn) {
            try {
                game.showMessage(module.name() + " was turned off", "It hit a problem. You can turn it on again.");
            } catch (RuntimeException e) {
                log.warn("Couldn't tell the player module " + module.id() + " was turned off", e);
            }
        }
    }
}
