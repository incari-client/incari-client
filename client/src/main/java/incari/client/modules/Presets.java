package incari.client.modules;

import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * The module presets (ADR 0062): ready-made sets of modules for a way of playing. Applying one
 * switches to a profile of its name (made as a copy of the active one the first time) with exactly
 * its modules on; settings stay as the player had them but for the preset's few
 * ({@code ModuleSystem.applyPreset}). Every preset sprints on its own (Toggle Sprint on Always), and
 * has Fullbright, Freelook and Weather Changer.
 */
final class Presets {

    /**
     * A preset: its name (also its profile's), what it's for, the ids of the modules it turns on,
     * and the settings it sets ({@code "module.setting"} to its config text).
     */
    record Preset(String name, String description, Set<String> modules, Map<String, String> settings) {}

    /** What every preset adds, said in each one's description. */
    private static final String SHARED = "Also Fullbright, Freelook, Weather Changer and auto sprint.";

    /** Auto sprint: what every preset sets. */
    private static final Map<String, String> AUTO_SPRINT = Map.of("togglesprint.sprint", "always");

    static final List<Preset> ALL = List.of(
            new Preset(
                    "1.8 PvP",
                    "Old-school combat: 1.7 animations, old textures, short swords, keystrokes, CPS and a block outline. "
                            + SHARED,
                    Set.of(
                            "oldanimations",
                            "oldtextures",
                            "togglesprint",
                            "keystrokes",
                            "cps",
                            "lowfire",
                            "nohurtcam",
                            "shortswords",
                            "blockoutline",
                            "fullbright",
                            "freelook",
                            "weatherchanger"),
                    AUTO_SPRINT),
            new Preset(
                    "Modern PvP",
                    "1.9+ combat: shield and totem HUD, armor view, reach display and a block outline. " + SHARED,
                    Set.of(
                            "shieldtotem",
                            "lowshield",
                            "lowfire",
                            "armorpotionview",
                            "reachdisplay",
                            "togglesprint",
                            "blockoutline",
                            "fullbright",
                            "freelook",
                            "weatherchanger"),
                    AUTO_SPRINT),
            new Preset(
                    "QoL",
                    "Getting around: minimap, waypoints, coordinates, zoom, saturation, FPS and more. " + SHARED,
                    Set.of(
                            "minimap",
                            "waypoints",
                            "coordinates",
                            "fpsdisplay",
                            "shulkertooltips",
                            "clearchat",
                            "freelook",
                            "zoom",
                            "saturation",
                            "customscoreboard",
                            "togglesprint",
                            "fullbright",
                            "weatherchanger"),
                    AUTO_SPRINT));

    private Presets() {}
}
