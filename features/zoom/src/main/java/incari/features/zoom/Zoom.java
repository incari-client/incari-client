package incari.features.zoom;

import incari.framework.Category;
import incari.framework.Module;
import incari.framework.event.CameraAngles;
import incari.framework.event.Events;
import incari.framework.event.Turn;
import incari.framework.game.Game;
import incari.framework.setting.BooleanSetting;
import incari.framework.setting.ChoiceSetting;
import incari.framework.setting.FloatSetting;
import incari.framework.setting.Key;
import incari.framework.setting.KeySetting;

/**
 * Zoom, as OptiFine's: while its key is held (or after a press, as a toggle), the world's field of
 * view narrows and the view turns smoothly, so far things can be picked out. The hand and the HUD
 * stay as they are. Letting go gives the view back at once, or eased out.
 */
public final class Zoom extends Module {

    /** C's scancode: OptiFine's zoom key. */
    static final int KEY_C = 6;

    /** How long easing in or out takes, near enough: the share left shrinks by e every this many ms. */
    private static final float EASE_MS = 50;

    /** How the key works. */
    public enum Mode {
        HOLD("Hold"),
        TOGGLE("Toggle");

        private final String label;

        Mode(String label) {
            this.label = label;
        }

        /** What the settings screen calls it. */
        public String label() {
            return label;
        }
    }

    /** How the view turns while zoomed. */
    public enum Turning {
        SMOOTH("Smooth"),
        SLOWER("Slower"),
        USUAL("As usual");

        private final String label;

        Turning(String label) {
            this.label = label;
        }

        /** What the settings screen calls it. */
        public String label() {
            return label;
        }
    }

    final KeySetting key =
            settings().key("zoom", "Zoom key", Key.keyboard(KEY_C)).describedAs("Hold (or press) to zoom in.");
    final ChoiceSetting<Mode> mode = settings()
            .choice("mode", "Key", Mode.values(), Mode.HOLD, Mode::label)
            .describedAs("Hold the key to zoom, or press it once to zoom in and again to zoom out.");
    final FloatSetting amount = settings()
            .decimal("amount", "Zoom", 2, 10, 0.5F, 4)
            .formattedAs(value ->
                    (value == Math.floor(value) ? Integer.toString((int) value) : Double.toString(value)) + "x")
            .describedAs("How many times closer things look. OptiFine's is 4x.");
    final ChoiceSetting<Turning> turning = settings()
            .choice("turning", "Turning", Turning.values(), Turning.SMOOTH, Turning::label)
            .describedAs("How the view turns while zoomed: smoothly as OptiFine's, slower, or as usual.");
    final BooleanSetting ease = settings()
            .bool("ease", "Ease in and out", false)
            .describedAs("Zooms in and out gradually instead of at once.");

    private boolean toggled;
    private boolean active;

    /** The field of view now, as a share of the game's: 1 not zoomed. */
    private float shown = 1;

    /** When the view last eased, by the game's clock; 0 before the first frame. */
    private long eased;

    /** Zoom, off until the player turns it on. */
    public Zoom() {
        super("zoom", "Zoom", "Hold a key to zoom in.", Category.VISUAL);
        key.onPress(() -> toggled = !toggled);
        on(Events.TICK, this::tick);
        on(Events.TURN, this::onTurn);
        on(Events.CAMERA, this::onCamera);
    }

    /** Whether the view's zoomed now. */
    boolean zooming() {
        return active;
    }

    /** The field of view the module asks for, as a share of the game's. */
    float shown() {
        return shown;
    }

    private float target() {
        return active ? 1 / amount.get() : 1;
    }

    private void tick() {
        Game game = system().game();
        active = game.player() != null && (mode.get() == Mode.HOLD ? key.isDown() : toggled);
        if (!active) {
            toggled = false;
        }
        game.overrideSmoothCamera(active && turning.get() == Turning.SMOOTH);
        if (!ease.get()) {
            show(target());
        }
    }

    private void onTurn(Turn turn) {
        if (shown < 1 && turning.get() == Turning.SLOWER) {
            turn.scale(shown);
        }
    }

    private void onCamera(CameraAngles camera) {
        // Each frame, so easing is smooth between ticks.
        long now = system().game().millis();
        long since = eased == 0 ? 0 : Math.min(now - eased, 100);
        eased = now;
        if (!ease.get()) {
            return;
        }
        float target = target();
        float next = shown + (target - shown) * (1 - (float) Math.exp(-since / EASE_MS));
        show(Math.abs(target - next) < 0.002F ? target : next);
    }

    private void show(float scale) {
        if (scale != shown) {
            shown = scale;
            system().game().overrideFov(scale);
        }
    }

    @Override
    protected void onDisable() {
        active = false;
        toggled = false;
        eased = 0;
        shown = 1;
        Game game = system().game();
        game.overrideFov(1);
        game.overrideSmoothCamera(false);
    }
}
