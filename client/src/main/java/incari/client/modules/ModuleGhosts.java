package incari.client.modules;

import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.QuadInstance;
import com.mojang.blaze3d.vertex.VertexConsumer;
import incari.framework.game.Blocks;
import incari.framework.hud.BlockGhosts;
import it.unimi.dsi.fastutil.longs.Long2ObjectMap;
import it.unimi.dsi.fastutil.longs.Long2ObjectOpenHashMap;
import it.unimi.dsi.fastutil.objects.ObjectIterator;
import java.util.Arrays;
import net.minecraft.client.Minecraft;
import net.minecraft.client.model.geom.builders.UVPair;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.block.BlockAndTintGetter;
import net.minecraft.client.renderer.block.BlockQuadOutput;
import net.minecraft.client.renderer.block.BlockStateModelSet;
import net.minecraft.client.renderer.block.ModelBlockRenderer;
import net.minecraft.client.renderer.rendertype.RenderTypes;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.client.renderer.texture.OverlayTexture;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.SectionPos;
import net.minecraft.gizmos.GizmoStyle;
import net.minecraft.gizmos.Gizmos;
import net.minecraft.util.ARGB;
import net.minecraft.util.LightCoordsUtil;
import net.minecraft.world.level.CardinalLighting;
import net.minecraft.world.level.ColorResolver;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.RenderShape;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.lighting.LevelLightEngine;
import net.minecraft.world.level.material.FluidState;
import org.joml.Matrix4f;
import org.joml.Vector3f;
import org.joml.Vector3fc;
import org.jspecify.annotations.Nullable;

/**
 * A module's ghost blocks ({@code WorldCanvas.ghosts}, Schematics): each 16-block section meshed
 * once with the game's own block models, faces against other ghosts and solid blocks culled, and
 * kept until the module says the section changed. A frame then only copies each kept quad into the
 * game's translucent moving-block buffer, fully lit, at the module's opacity: it sorts with the
 * world's other see-through things and goes through {@code GpuDevice} as falling blocks do. Blocks
 * without a model (chests, signs: drawn by their block entity) show as a tinted box. Render thread.
 */
public final class ModuleGhosts {

    /** Sections meshed in a frame at most, nearest first: each is up to 4096 blocks. */
    private static final int MESHES_PER_FRAME = 2;

    /** Quads kept over every section at most, about 10 MB: past it, farther sections wait. */
    private static final int MAX_QUADS = 1 << 16;

    /** Frames a section may go undrawn before its mesh is let go. */
    private static final int KEEP_FRAMES = 60;

    /** A modelless block's box color, before the opacity: a pale blue. */
    private static final int BOX_RGB = 0x88CCFF;

    private static final Direction[] DIRECTIONS = Direction.values();

    private static final ModuleGhosts INSTANCE = new ModuleGhosts();

    private final Long2ObjectOpenHashMap<Mesh> meshes = new Long2ObjectOpenHashMap<>();
    private @Nullable BlockGhosts source;

    /** The world and models the meshes were made in: a new world or a resource reload (the atlas moves) drops them. */
    private @Nullable ClientLevel meshedLevel;

    private @Nullable BlockStateModelSet meshedModels;

    private long frame;
    private int quads;

    /** What a section is meshed from: the section's ghosts, the world everywhere else. */
    private final Region region = new Region();

    private final QuadSink sink = new QuadSink();
    private final BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();

    private ModuleGhosts() {}

    /** One section's ghosts, as quads relative to the section's corner. */
    private static final class Mesh implements SubmitNodeCollector.CustomGeometryRenderer {

        long revision;
        long drawnAt;

        /** Per quad: 4 corners of x, y, z; 4 of u, v; 4 colors; one normal's direction. */
        float[] positions = new float[0];

        float[] uvs = new float[0];
        int[] colors = new int[0];
        byte[] normals = new byte[0];
        int count;

        /** Blocks with no model to draw: their places in the section, {@code y << 8 | z << 4 | x}. */
        int[] boxes = new int[0];

        int boxCount;

        /** This frame's opacity, set as it's submitted and read when it's drawn, later in the frame. */
        int alpha;

        private final Vector3f scratch = new Vector3f();
        private final Vector3f normal = new Vector3f();

        @Override
        public void render(PoseStack.Pose pose, VertexConsumer buffer) {
            Matrix4f matrix = pose.pose();
            int a = alpha;
            for (int q = 0; q < count; q++) {
                pose.transformNormal(Direction.from3DDataValue(normals[q]).getUnitVec3f(), normal);
                for (int v = 0; v < 4; v++) {
                    int p = q * 12 + v * 3;
                    matrix.transformPosition(positions[p], positions[p + 1], positions[p + 2], scratch);
                    int t = q * 8 + v * 2;
                    buffer.addVertex(
                            scratch.x(),
                            scratch.y(),
                            scratch.z(),
                            ARGB.color(a, colors[q * 4 + v]),
                            uvs[t],
                            uvs[t + 1],
                            OverlayTexture.NO_OVERLAY,
                            LightCoordsUtil.FULL_BRIGHT,
                            normal.x(),
                            normal.y(),
                            normal.z());
                }
            }
        }
    }

    /** {@code WorldCanvas.ghosts}: draws {@code ghosts} within {@code range} blocks of the camera. */
    static void draw(
            BlockGhosts ghosts,
            int alpha,
            int range,
            PoseStack poseStack,
            SubmitNodeCollector collector,
            CameraRenderState camera) {
        INSTANCE.drawFrame(ghosts, alpha, range, poseStack, collector, camera);
    }

    /** How many quads the kept meshes hold now, every section's together: the smoke test's check. */
    public static int quads() {
        return INSTANCE.quads;
    }

    /** Lets every mesh go: the world is left, or resources reload (the atlas moves). */
    static void clear() {
        INSTANCE.meshes.clear();
        INSTANCE.quads = 0;
        INSTANCE.source = null;
    }

    @SuppressWarnings("ReferenceEquality") // the module's object, the world and the models, not equal ones
    private void drawFrame(
            BlockGhosts ghosts,
            int alpha,
            int range,
            PoseStack poseStack,
            SubmitNodeCollector collector,
            CameraRenderState camera) {
        ClientLevel level = Minecraft.getInstance().level;
        if (level == null) {
            return;
        }
        BlockStateModelSet models = Minecraft.getInstance().getModelManager().getBlockStateModelSet();
        if (ghosts != source || level != meshedLevel || models != meshedModels) {
            clear();
            source = ghosts;
            meshedLevel = level;
            meshedModels = models;
        }
        frame++;
        double camX = camera.pos.x;
        double camY = camera.pos.y;
        double camZ = camera.pos.z;
        int cx = SectionPos.blockToSectionCoord(camX);
        int cy = SectionPos.blockToSectionCoord(camY);
        int cz = SectionPos.blockToSectionCoord(camZ);
        int minSX = Math.max(SectionPos.blockToSectionCoord(ghosts.minX()), cx - (range >> 4) - 1);
        int minSY = Math.max(SectionPos.blockToSectionCoord(ghosts.minY()), cy - (range >> 4) - 1);
        int minSZ = Math.max(SectionPos.blockToSectionCoord(ghosts.minZ()), cz - (range >> 4) - 1);
        int maxSX = Math.min(SectionPos.blockToSectionCoord(ghosts.maxX() - 1), cx + (range >> 4) + 1);
        int maxSY = Math.min(SectionPos.blockToSectionCoord(ghosts.maxY() - 1), cy + (range >> 4) + 1);
        int maxSZ = Math.min(SectionPos.blockToSectionCoord(ghosts.maxZ() - 1), cz + (range >> 4) + 1);
        int meshed = 0;
        double rangeSquared = (double) range * range;
        int rings = Math.max(
                Math.max(Math.abs(cx - minSX), Math.abs(maxSX - cx)),
                Math.max(
                        Math.max(Math.abs(cy - minSY), Math.abs(maxSY - cy)),
                        Math.max(Math.abs(cz - minSZ), Math.abs(maxSZ - cz))));
        GizmoStyle boxStyle = GizmoStyle.fill(ARGB.color(alpha / 2, BOX_RGB));
        // Ring by ring outward, so the nearest sections are meshed first.
        for (int ring = 0; ring <= rings; ring++) {
            for (int sy = Math.max(minSY, cy - ring); sy <= Math.min(maxSY, cy + ring); sy++) {
                for (int sz = Math.max(minSZ, cz - ring); sz <= Math.min(maxSZ, cz + ring); sz++) {
                    for (int sx = Math.max(minSX, cx - ring); sx <= Math.min(maxSX, cx + ring); sx++) {
                        if (Math.max(Math.abs(sx - cx), Math.max(Math.abs(sy - cy), Math.abs(sz - cz))) != ring) {
                            continue;
                        }
                        if (distanceSquared(sx, sy, sz, camX, camY, camZ) > rangeSquared) {
                            continue;
                        }
                        long key = SectionPos.asLong(sx, sy, sz);
                        long revision = ghosts.revision(sx, sy, sz);
                        Mesh mesh = meshes.get(key);
                        if ((mesh == null || mesh.revision != revision)
                                && meshed < MESHES_PER_FRAME
                                && quads < MAX_QUADS) {
                            mesh = mesh(level, ghosts, sx, sy, sz, mesh);
                            mesh.revision = revision;
                            meshes.put(key, mesh);
                            meshed++;
                        }
                        if (mesh == null) {
                            continue;
                        }
                        mesh.drawnAt = frame;
                        int ox = SectionPos.sectionToBlockCoord(sx);
                        int oy = SectionPos.sectionToBlockCoord(sy);
                        int oz = SectionPos.sectionToBlockCoord(sz);
                        if (mesh.count > 0) {
                            mesh.alpha = alpha;
                            poseStack.pushPose();
                            poseStack.translate(ox - camX, oy - camY, oz - camZ);
                            collector.submitCustomGeometry(poseStack, RenderTypes.translucentMovingBlock(), mesh);
                            poseStack.popPose();
                        }
                        for (int i = 0; i < mesh.boxCount; i++) {
                            int at = mesh.boxes[i];
                            pos.set(ox + (at & 15), oy + (at >> 8), oz + ((at >> 4) & 15));
                            Gizmos.cuboid(pos, boxStyle);
                        }
                    }
                }
            }
        }
        if (frame % KEEP_FRAMES == 0) {
            evict();
        }
    }

    private static double distanceSquared(int sx, int sy, int sz, double x, double y, double z) {
        double dx = Math.max(0, Math.max((sx << 4) - x, x - ((sx << 4) + 16)));
        double dy = Math.max(0, Math.max((sy << 4) - y, y - ((sy << 4) + 16)));
        double dz = Math.max(0, Math.max((sz << 4) - z, z - ((sz << 4) + 16)));
        return dx * dx + dy * dy + dz * dz;
    }

    /** Lets go of the meshes not drawn lately: the player went away from them. */
    private void evict() {
        ObjectIterator<Long2ObjectMap.Entry<Mesh>> iterator =
                meshes.long2ObjectEntrySet().fastIterator();
        while (iterator.hasNext()) {
            Mesh mesh = iterator.next().getValue();
            if (frame - mesh.drawnAt > KEEP_FRAMES) {
                quads -= mesh.count;
                iterator.remove();
            }
        }
    }

    /** Meshes one section's ghosts, into {@code reuse} if there is one. */
    private Mesh mesh(ClientLevel level, BlockGhosts ghosts, int sx, int sy, int sz, @Nullable Mesh reuse) {
        Mesh mesh = reuse == null ? new Mesh() : reuse;
        quads -= mesh.count;
        mesh.count = 0;
        mesh.boxCount = 0;
        int ox = SectionPos.sectionToBlockCoord(sx);
        int oy = SectionPos.sectionToBlockCoord(sy);
        int oz = SectionPos.sectionToBlockCoord(sz);
        region.begin(level, ghosts, ox, oy, oz);
        boolean any = false;
        for (int i = 0; i < 4096; i++) {
            int x = ox + (i & 15);
            int y = oy + (i >> 8);
            int z = oz + ((i >> 4) & 15);
            boolean inside = x >= ghosts.minX()
                    && y >= ghosts.minY()
                    && z >= ghosts.minZ()
                    && x < ghosts.maxX()
                    && y < ghosts.maxY()
                    && z < ghosts.maxZ();
            int state = inside ? ghosts.ghostAt(x, y, z) : Blocks.AIR;
            region.ghosts[i] = state > 0 ? state : 0;
            any |= state > 0;
        }
        if (!any) {
            region.end();
            return mesh;
        }
        Minecraft minecraft = Minecraft.getInstance();
        BlockStateModelSet models = minecraft.getModelManager().getBlockStateModelSet();
        ModelBlockRenderer renderer = new ModelBlockRenderer(false, true, minecraft.getBlockColors());
        sink.mesh = mesh;
        for (int i = 0; i < 4096; i++) {
            int id = region.ghosts[i];
            if (id == 0) {
                continue;
            }
            BlockState state = Block.stateById(id);
            int lx = i & 15;
            int ly = i >> 8;
            int lz = (i >> 4) & 15;
            int before = mesh.count;
            if (state.getRenderShape() == RenderShape.MODEL) {
                pos.set(ox + lx, oy + ly, oz + lz);
                renderer.tesselateBlock(sink, lx, ly, lz, region, pos, state, models.get(state), state.getSeed(pos));
            }
            if (mesh.count == before) {
                // Nothing drawn for it: a block entity's model or a fluid, or hidden on every side.
                if (!fullyHidden(region, ox + lx, oy + ly, oz + lz)) {
                    if (mesh.boxCount == mesh.boxes.length) {
                        mesh.boxes = Arrays.copyOf(mesh.boxes, Math.max(16, mesh.boxes.length * 2));
                    }
                    mesh.boxes[mesh.boxCount++] = ly << 8 | lz << 4 | lx;
                }
            }
        }
        sink.mesh = null;
        region.end();
        quads += mesh.count;
        return mesh;
    }

    /** Whether every side of the block at {@code x, y, z} touches another ghost or a solid block. */
    private boolean fullyHidden(Region region, int x, int y, int z) {
        for (Direction direction : DIRECTIONS) {
            pos.set(x + direction.getStepX(), y + direction.getStepY(), z + direction.getStepZ());
            if (!region.getBlockState(pos).isSolidRender()) {
                return false;
            }
        }
        return true;
    }

    /** Takes the quads the block renderer gives into the mesh being made. */
    private static final class QuadSink implements BlockQuadOutput {

        @Nullable
        Mesh mesh;

        @Override
        public void put(float x, float y, float z, BakedQuad quad, QuadInstance instance) {
            Mesh into = mesh;
            if (into == null) {
                return;
            }
            int q = into.count;
            if (q == into.normals.length) {
                int grown = Math.max(64, q * 2);
                into.positions = Arrays.copyOf(into.positions, grown * 12);
                into.uvs = Arrays.copyOf(into.uvs, grown * 8);
                into.colors = Arrays.copyOf(into.colors, grown * 4);
                into.normals = Arrays.copyOf(into.normals, grown);
            }
            for (int v = 0; v < 4; v++) {
                Vector3fc corner = quad.position(v);
                int p = q * 12 + v * 3;
                into.positions[p] = corner.x() + x;
                into.positions[p + 1] = corner.y() + y;
                into.positions[p + 2] = corner.z() + z;
                long uv = quad.packedUV(v);
                into.uvs[q * 8 + v * 2] = UVPair.unpackU(uv);
                into.uvs[q * 8 + v * 2 + 1] = UVPair.unpackV(uv);
                into.colors[q * 4 + v] = instance.getColor(v) & 0xFFFFFF;
            }
            into.normals[q] = (byte) quad.direction().get3DDataValue();
            into.count = q + 1;
        }
    }

    /**
     * The blocks a section is meshed against: its ghosts where it has them, the world's blocks
     * elsewhere (so a ghost beside a built wall hides the face it shares), and the world's tints.
     */
    private static final class Region implements BlockAndTintGetter {

        final int[] ghosts = new int[4096];
        private @Nullable ClientLevel level;
        private @Nullable BlockGhosts source;
        private int ox;
        private int oy;
        private int oz;

        void begin(ClientLevel level, BlockGhosts source, int ox, int oy, int oz) {
            this.level = level;
            this.source = source;
            this.ox = ox;
            this.oy = oy;
            this.oz = oz;
        }

        void end() {
            level = null;
            source = null;
        }

        private ClientLevel level() {
            ClientLevel current = level;
            if (current == null) {
                throw new IllegalStateException("Read outside a mesh");
            }
            return current;
        }

        @Override
        public BlockState getBlockState(BlockPos at) {
            int lx = at.getX() - ox;
            int ly = at.getY() - oy;
            int lz = at.getZ() - oz;
            int id;
            if ((lx | ly | lz) >= 0 && lx < 16 && ly < 16 && lz < 16) {
                id = ghosts[ly << 8 | lz << 4 | lx];
            } else {
                BlockGhosts from = source;
                id = from != null && inside(from, at) ? from.ghostAt(at.getX(), at.getY(), at.getZ()) : 0;
            }
            return id > 0 ? Block.stateById(id) : level().getBlockState(at);
        }

        private static boolean inside(BlockGhosts box, BlockPos at) {
            return at.getX() >= box.minX()
                    && at.getY() >= box.minY()
                    && at.getZ() >= box.minZ()
                    && at.getX() < box.maxX()
                    && at.getY() < box.maxY()
                    && at.getZ() < box.maxZ();
        }

        @Override
        public FluidState getFluidState(BlockPos at) {
            return getBlockState(at).getFluidState();
        }

        @Override
        public @Nullable BlockEntity getBlockEntity(BlockPos at) {
            return null;
        }

        @Override
        public CardinalLighting cardinalLighting() {
            return level().cardinalLighting();
        }

        @Override
        public int getBlockTint(BlockPos at, ColorResolver color) {
            return level().getBlockTint(at, color);
        }

        @Override
        public LevelLightEngine getLightEngine() {
            return level().getLightEngine();
        }

        @Override
        public int getHeight() {
            return level().getHeight();
        }

        @Override
        public int getMinY() {
            return level().getMinY();
        }
    }
}
