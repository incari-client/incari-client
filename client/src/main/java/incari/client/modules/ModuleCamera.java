package incari.client.modules;

import incari.framework.ModuleSystem;
import incari.framework.event.CameraAngles;
import incari.framework.event.Events;
import incari.framework.event.Turn;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Options;
import net.minecraft.client.player.LocalPlayer;

/**
 * The turn and camera events ({@link Events#TURN}, {@link Events#CAMERA}): the mouse's turns,
 * which a module may keep from the player, and the camera's angles each frame, which a module may
 * change without turning the player (Freelook); and what Zoom holds: the field of view and the
 * game's smooth camera. Game and render thread (one thread).
 */
public final class ModuleCamera {

    /** The game's degrees per unit of mouse turn ({@code Entity.turn}). */
    private static final float DEGREES = 0.15F;

    /** One object each for every turn and frame: both events are hot. */
    private static final Turn TURN = new Turn();

    private static final CameraAngles ANGLES = new CameraAngles();

    /** The world's field of view as a share of the game's ({@code Game.overrideFov}). */
    private static float fovScale = 1;

    /** Whether a module holds the smooth camera on, and the player's own while it does. */
    private static boolean smoothHeld;

    private static boolean smoothBefore;

    private ModuleCamera() {}

    /**
     * Hook ({@code MouseHandler.turnPlayer}, before the player turns): the turn handed to modules.
     *
     * @return whether a module took it, or turned the player itself, so the game doesn't
     */
    public static boolean turn(double x, double y) {
        ModuleSystem system = IncariModules.current();
        if (system == null || system.events().count(Events.TURN) == 0) {
            return false;
        }
        Options options = Minecraft.getInstance().options;
        double yaw = options.invertMouseX().get() ? -x : x;
        double pitch = options.invertMouseY().get() ? -y : y;
        TURN.reset((float) yaw * DEGREES, (float) pitch * DEGREES);
        system.events().fireTurn(TURN);
        if (TURN.consumed()) {
            return true;
        }
        LocalPlayer player = Minecraft.getInstance().player;
        if (!TURN.scaled() || player == null) {
            return false;
        }
        // Already inverted, in degrees: back to the game's units.
        player.turn(TURN.yaw() / DEGREES, TURN.pitch() / DEGREES);
        return true;
    }

    /** Hook ({@code Camera.calculateFov}): the world's field of view as a module leaves it. */
    public static float fov(float fov) {
        return fov * fovScale;
    }

    /** {@code Game.overrideFov}'s. */
    static void fovScale(float scale) {
        fovScale = scale;
    }

    /** {@code Game.overrideSmoothCamera}'s: F8's smooth camera, the player's own put back after. */
    static void smooth(boolean on) {
        if (on == smoothHeld) {
            return;
        }
        Options options = Minecraft.getInstance().options;
        if (on) {
            smoothBefore = options.smoothCamera;
            options.smoothCamera = true;
        } else {
            options.smoothCamera = smoothBefore;
        }
        smoothHeld = on;
    }

    /**
     * Hook ({@code Camera.alignWithEntity}, with the player's view): the camera's angles handed to
     * modules.
     *
     * @return whether a module turned the camera: {@link #yaw()} and {@link #pitch()} then say where
     */
    public static boolean camera(float yaw, float pitch) {
        ModuleSystem system = IncariModules.current();
        if (system == null || system.events().count(Events.CAMERA) == 0) {
            return false;
        }
        ANGLES.reset(yaw, pitch);
        system.events().fireCamera(ANGLES);
        return ANGLES.changed();
    }

    /** The camera's yaw a module chose, after {@link #camera} said one did. */
    public static float yaw() {
        return ANGLES.yaw();
    }

    /** The camera's pitch a module chose. */
    public static float pitch() {
        return ANGLES.pitch();
    }
}
