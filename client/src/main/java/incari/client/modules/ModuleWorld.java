package incari.client.modules;

import com.mojang.blaze3d.vertex.PoseStack;
import incari.framework.ModuleSystem;
import incari.framework.event.Events;
import incari.framework.hud.BlockGhosts;
import incari.framework.hud.WorldCanvas;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.blockentity.BeaconRenderer;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.gizmos.GizmoStyle;
import net.minecraft.gizmos.Gizmos;
import net.minecraft.util.ARGB;
import net.minecraft.world.phys.AABB;
import org.jspecify.annotations.Nullable;

/**
 * What modules add to the world itself ({@link Events#WORLD}, ADR 0051): waypoints' beams, drawn as
 * a beacon's are, with the game's beam texture and render types, so they look like one and sort with
 * the world's other translucent things; a schematic's ghost blocks ({@link ModuleGhosts}) and boxes,
 * as the game's debug shapes. Render thread, while the frame's world is submitted.
 */
public final class ModuleWorld implements WorldCanvas {

    private static final ModuleWorld CANVAS = new ModuleWorld();

    /** A beam's height above its base: past the top of any world. */
    private static final int BEAM_HEIGHT = BeaconRenderer.MAX_RENDER_Y;

    /** Within this share of the fog's start, a beam is drawn where it is; beyond it, moved in. */
    private static final float NEAR_SHARE = 0.85F;

    /** How far a box stands off its blocks. */
    private static final double BOX_MARGIN = 0.002;

    /** A box's lines: the game's debug line width. */
    private static final float BOX_WIDTH = 2.5F;

    private @Nullable PoseStack poseStack;
    private @Nullable SubmitNodeCollector collector;
    private @Nullable CameraRenderState camera;
    private float animationTime;

    private ModuleWorld() {}

    /** Hook ({@code LevelRenderer.submitFeatures}, after particles): the modules' part of the world. */
    public static void submit(PoseStack poseStack, SubmitNodeCollector collector, CameraRenderState camera) {
        ModuleSystem system = IncariModules.current();
        ClientLevel level = Minecraft.getInstance().level;
        if (system == null || level == null || system.events().count(Events.WORLD) == 0) {
            return;
        }
        float partial = Minecraft.getInstance().getDeltaTracker().getGameTimeDeltaPartialTick(false);
        CANVAS.animationTime = Math.floorMod(level.getGameTime(), 40) + partial;
        CANVAS.poseStack = poseStack;
        CANVAS.collector = collector;
        CANVAS.camera = camera;
        // Boxes are the game's gizmos: collected with the frame's own, drawn after this.
        try (Gizmos.TemporaryCollection _ =
                Minecraft.getInstance().levelRenderer.collectPerFrameRenderThreadGizmos()) {
            system.events().fireWorld(CANVAS);
        } finally {
            CANVAS.poseStack = null;
            CANVAS.collector = null;
            CANVAS.camera = null;
        }
    }

    private CameraRenderState camera() {
        CameraRenderState current = camera;
        if (current == null) {
            throw new IllegalStateException("The world is drawn on only while it's submitted");
        }
        return current;
    }

    @Override
    public void beam(int x, int y, int z, int rgb) {
        PoseStack poses = poseStack;
        SubmitNodeCollector submit = collector;
        CameraRenderState view = camera();
        if (poses == null || submit == null) {
            return;
        }
        double dx = x + 0.5 - view.pos.x;
        double dy = y - view.pos.y;
        double dz = z + 0.5 - view.pos.z;
        double distance = Math.sqrt(dx * dx + dz * dz);
        // Beyond the fog the beam would be fogged out or cut by the far plane: it's moved in along
        // the line of sight and shrunk alike, so it looks the same from here.
        float near = Math.max(16.0F, view.fogData.renderDistanceStart * NEAR_SHARE);
        float shrink = distance > near ? (float) (near / distance) : 1.0F;
        // As a beacon's beam, wider far off so it stays a few pixels wide (before the shrink, which
        // the pose applies to the beam whole).
        float widen = Math.max(1.0F, (float) distance / 96.0F);
        poses.pushPose();
        // The block's corner, as a beacon's pose has it, moved in toward the camera.
        poses.translate((dx - 0.5) * shrink, dy * shrink, (dz - 0.5) * shrink);
        poses.scale(shrink, shrink, shrink);
        BeaconRenderer.submitBeaconBeam(
                poses,
                submit,
                BeaconRenderer.BEAM_LOCATION,
                1.0F,
                animationTime,
                0,
                BEAM_HEIGHT,
                ARGB.opaque(rgb),
                BeaconRenderer.SOLID_BEAM_RADIUS * widen,
                BeaconRenderer.BEAM_GLOW_RADIUS * widen);
        poses.popPose();
    }

    @Override
    public void ghosts(BlockGhosts ghosts, int alpha, int range) {
        PoseStack poses = poseStack;
        SubmitNodeCollector submit = collector;
        CameraRenderState view = camera();
        if (poses != null && submit != null) {
            ModuleGhosts.draw(ghosts, Math.max(0, Math.min(255, alpha)), range, poses, submit, view);
        }
    }

    @Override
    public void box(int minX, int minY, int minZ, int maxX, int maxY, int maxZ, int argb, int fillArgb) {
        camera();
        // A hair outside the blocks, so the edges aren't lost in the faces they lie on.
        AABB bounds = new AABB(minX, minY, minZ, maxX, maxY, maxZ).inflate(BOX_MARGIN);
        Gizmos.cuboid(bounds, GizmoStyle.strokeAndFill(argb, BOX_WIDTH, fillArgb));
    }

    @Override
    public double cameraX() {
        return camera().pos.x;
    }

    @Override
    public double cameraY() {
        return camera().pos.y;
    }

    @Override
    public double cameraZ() {
        return camera().pos.z;
    }
}
