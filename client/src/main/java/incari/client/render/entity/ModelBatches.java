package incari.client.render.entity;

import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.logging.LogUtils;
import com.mojang.renderpearl.api.buffers.GpuBuffer;
import com.mojang.renderpearl.api.commands.CommandEncoder;
import com.mojang.renderpearl.api.commands.RenderPass;
import com.mojang.renderpearl.api.pipeline.PrimitiveTopology;
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import net.minecraft.client.model.Model;
import net.minecraft.client.renderer.feature.ItemFeatureRenderer;
import net.minecraft.client.renderer.feature.ModelFeatureRenderer;
import net.minecraft.client.renderer.item.ItemStackRenderState;
import net.minecraft.client.renderer.oit.OitStage;
import net.minecraft.client.renderer.rendertype.PreparedRenderType;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.minecraft.client.renderer.rendertype.RenderTypes;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;

/**
 * Entity models drawn instanced (ADR 0024): one draw per render type and model in a group,
 * instead of every entity's every vertex transformed and written on the CPU. The game's
 * {@code ModelFeatureRenderer} owns one of these; its hooks hand each model submit here first
 * ({@link #take}), and whatever is taken is drawn by {@link #draw} with the group the game's own
 * vertices draw in, before them: a block entity's crumbling overlay, drawn the game's way, writes no
 * depth, so a model drawn after it would cover it.
 *
 * <p>Per instance the CPU runs the model's {@code setupAnim} and writes one matrix per part
 * ({@link ModelMesh#writeRows}); the vertices are baked once. Taken: opaque and cutout models (the
 * solid phase, where order doesn't show) on the game's entity pipeline, with no texture remap or
 * decal. The rest (translucent models, outlines, crumbling, armor trims) goes the game's way.
 *
 * <p>Translucent models whose texture is all-or-nothing alpha, which draw exactly as opaque ones
 * ({@link OpaqueSkins}: most player skins), are in the solid phase too, and taken the same way.
 *
 * <p>Items are taken the same way ({@link #takeItem}, ADR 0027), by an instance the game's
 * {@code ItemFeatureRenderer} owns: an item model's quads baked once as one part, placed by the
 * item's pose. Only solid-phase items in one color: one tint index for all quads (generated items
 * give each layer's quads the layer's index, tinted or not), no light emission, glint or outline.
 * The rest (potions and other multi-tint items, enchanted items) goes the game's way.
 *
 * <p>The WebGL2 backend draws it; the desktop's backends compile shaders from source and don't
 * know the rewrite, so it's only on when the page turns it on ({@link #enable}).
 *
 * <p>Render thread only.
 */
public final class ModelBatches {

    private static final Logger LOGGER = LogUtils.getLogger();

    private static boolean enabled;
    /** Items too ({@link #takeItem}); the page's {@code ?items=game} leaves them the game's way. */
    private static boolean items = true;
    /** Whether the page has been told that models are drawn instanced (once). */
    private static boolean announced;

    /** Meshes, shared by every renderer: a model is baked once. */
    private static final IdentityHashMap<Model<?>, @Nullable ModelMesh> MESHES = new IdentityHashMap<>();

    /**
     * Item meshes, by the quad list an item model baked once ({@code ItemQuads}): null for one
     * that can't be drawn this way.
     */
    private static final IdentityHashMap<List<BakedQuad>, @Nullable ModelMesh> ITEM_MESHES = new IdentityHashMap<>();

    /** The tint index every quad of an item mesh's list shares (-1: none), by the same list. */
    private static final IdentityHashMap<List<BakedQuad>, Integer> ITEM_TINTS = new IdentityHashMap<>();

    /** Quads whose tint indexes differ: no one color fits them all. */
    private static final int MIXED_TINTS = Integer.MIN_VALUE;

    /** Meshes unused for this many frames are closed (their models went with a resource reload). */
    private static final int MESH_IDLE_FRAMES = 600;

    /** Frames drawn, counted by every instance's {@link #endFrame}; {@link OpaqueSkins} keys its answers by it. */
    static long frame;

    /** One draw's instances: a render type, a model, and at most a block's worth of rows. */
    private static final class Batch {
        @Nullable
        PreparedRenderType type;

        @Nullable
        RenderPipeline pipeline;

        @Nullable
        ModelMesh mesh;

        int capacity;
        int count;
        final float[] rows = new float[InstancedPipelines.ROWS * 4];
        /** Four ints an instance: color (RGBA bytes), overlay, light, first row. */
        final int[] instances = new int[InstancedPipelines.ROWS / 3 * 4];

        long rowsOffset;
        long instancesOffset;
    }

    private final List<Batch> pool = new ArrayList<>();
    private int used;
    /** The batches of each group, by the group's index among this renderer's groups this frame. */
    private final List<List<Batch>> groups = new ArrayList<>();

    private @Nullable List<Batch> group;
    /** The current group's open batch per render type and model. */
    private final IdentityHashMap<RenderType, IdentityHashMap<Object, Batch>> open = new IdentityHashMap<>();
    /** Render types prepared this frame (their uniforms written once, as the game writes one per draw). */
    private final IdentityHashMap<RenderType, PreparedRenderType> prepared = new IdentityHashMap<>();

    private final IdentityHashMap<RenderType, @Nullable RenderPipeline> pipelines = new IdentityHashMap<>();

    private @Nullable ByteBuffer rowData;
    private @Nullable ByteBuffer instanceData;
    private @Nullable GpuBuffer rowBuffer;
    private @Nullable GpuBuffer instanceBuffer;
    private int maxQuads;

    /** Draws items with the game's way instead ({@code ?items=game}), to compare. */
    public static void setItemsEnabled(boolean on) {
        items = on;
    }

    /** Draws entity models instanced from now on. The page calls it before the game starts. */
    public static void enable() {
        enabled = true;
    }

    public static boolean enabled() {
        return enabled;
    }

    /** Hook: a group of model submits begins (the game's {@code buildGroup}). */
    public void beginGroup() {
        group = new ArrayList<>();
        groups.add(group);
        open.clear();
    }

    /**
     * Hook: takes a model submit to draw instanced, instead of the game writing its vertices.
     *
     * @return false if it isn't one to draw instanced: the game draws it
     */
    public <S> boolean take(ModelFeatureRenderer.Submit<S> submit) {
        List<Batch> current = group;
        if (!enabled || current == null || submit.uvMapping() != null || submit.sheetedDecalPose() != null) {
            return false;
        }
        RenderType renderType = submit.renderType();
        // A translucent model that draws as an opaque one is in the solid phase too (ADR 0027).
        boolean opaqueSkin = renderType.hasBlending() && OpaqueSkins.drawsAsSolid(renderType, submit.tintedColor());
        RenderPipeline pipeline = opaqueSkin ? opaqueSkinPipeline(renderType) : pipeline(renderType);
        if (pipeline == null) {
            return false;
        }
        Model<? super S> model = submit.model();
        ModelMesh mesh = mesh(model);
        if (mesh == null) {
            return false;
        }
        IdentityHashMap<Object, Batch> byModel = open.computeIfAbsent(renderType, type -> new IdentityHashMap<>());
        Batch batch = byModel.get(model);
        if (batch == null || batch.count == batch.capacity) {
            batch = batch(renderType, pipeline, mesh);
            current.add(batch);
            byModel.put(model, batch);
        }
        model.setupAnim(submit.state());
        int instance = batch.count++;
        int firstRow = instance * mesh.rows();
        mesh.writeRows(submit.pose(), batch.rows, firstRow * 4);
        int at = instance * 4;
        int color = submit.tintedColor();
        // The game's vertex color bytes: red, green, blue, alpha, from its ARGB.
        batch.instances[at] =
                (color >> 16 & 0xFF) | (color >> 8 & 0xFF) << 8 | (color & 0xFF) << 16 | (color >>> 24) << 24;
        batch.instances[at + 1] = packShorts(submit.overlayCoords());
        batch.instances[at + 2] = packShorts(submit.lightCoords());
        batch.instances[at + 3] = firstRow;
        return true;
    }

    /**
     * Hook: takes an item submit to draw instanced, instead of the game writing its quads
     * ({@code ItemFeatureRenderer.prepareSubmit}).
     *
     * @return false if it isn't one to draw instanced: the game draws it
     */
    public boolean takeItem(ItemFeatureRenderer.Submit submit) {
        List<Batch> current = group;
        if (!enabled
                || !items
                || current == null
                || submit.outlineColor() != 0
                || submit.foilType() != ItemStackRenderState.FoilType.NONE) {
            return false;
        }
        List<BakedQuad> quads = submit.quads();
        ModelMesh mesh = itemMesh(quads);
        if (mesh == null) {
            return false;
        }
        RenderType renderType = quads.getFirst().materialInfo().itemRenderType();
        RenderPipeline pipeline = pipeline(renderType);
        if (pipeline == null) {
            return false;
        }
        IdentityHashMap<Object, Batch> byQuads = open.computeIfAbsent(renderType, type -> new IdentityHashMap<>());
        Batch batch = byQuads.get(quads);
        if (batch == null || batch.count == batch.capacity) {
            batch = batch(renderType, pipeline, mesh);
            current.add(batch);
            byQuads.put(quads, batch);
        }
        int instance = batch.count++;
        int firstRow = instance * mesh.rows();
        mesh.writeRows(submit.pose(), batch.rows, firstRow * 4);
        int at = instance * 4;
        // The quads' color, its light the submit's (no emission), as the game writes them.
        int color = itemColor(submit.tintLayers(), ITEM_TINTS.getOrDefault(quads, -1));
        batch.instances[at] =
                (color >> 16 & 0xFF) | (color >> 8 & 0xFF) << 8 | (color & 0xFF) << 16 | (color >>> 24) << 24;
        batch.instances[at + 1] = packShorts(submit.overlayCoords());
        batch.instances[at + 2] = packShorts(submit.lightCoords());
        batch.instances[at + 3] = firstRow;
        return true;
    }

    /**
     * The mesh for an item's quads, baked the first time. Null when they can't be drawn this way:
     * more than one tint index or render type, an emissive or blending quad (the translucent
     * phase), or an animated sprite (the game's way marks the sprite drawn, so it keeps animating;
     * ADR 0030).
     */
    private static @Nullable ModelMesh itemMesh(List<BakedQuad> quads) {
        ModelMesh mesh;
        if (ITEM_MESHES.containsKey(quads)) {
            mesh = ITEM_MESHES.get(quads);
        } else {
            int tint = sharedTint(quads);
            mesh = tint != MIXED_TINTS && plainQuads(quads) ? ModelMesh.ofQuads(quads, "item") : null;
            if (mesh != null && mesh.rows() > InstancedPipelines.ROWS) {
                mesh.close();
                mesh = null;
            }
            ITEM_MESHES.put(quads, mesh);
            ITEM_TINTS.put(quads, tint);
        }
        if (mesh != null) {
            mesh.lastUsedFrame = frame;
        }
        return mesh;
    }

    @SuppressWarnings("ReferenceEquality") // render types are the game's shared instances
    private static boolean plainQuads(List<BakedQuad> quads) {
        if (quads.isEmpty()) {
            return false;
        }
        RenderType renderType = quads.getFirst().materialInfo().itemRenderType();
        if (renderType.hasBlending()) {
            return false;
        }
        for (BakedQuad quad : quads) {
            BakedQuad.MaterialInfo material = quad.materialInfo();
            if (material.lightEmission() != 0
                    || material.itemRenderType() != renderType
                    || material.sprite().isAnimated()) {
                return false;
            }
        }
        return true;
    }

    /** The tint index all of {@code quads} share, -1 for none, or {@link #MIXED_TINTS}. */
    static int sharedTint(List<BakedQuad> quads) {
        int tint = quads.isEmpty() ? -1 : quads.getFirst().materialInfo().tintIndex();
        for (BakedQuad quad : quads) {
            if (quad.materialInfo().tintIndex() != tint) {
                return MIXED_TINTS;
            }
        }
        // Any negative index draws untinted, as the game's -1 does.
        return tint < 0 ? -1 : tint;
    }

    /**
     * The color the game gives a quad of tint index {@code tint} ({@code ItemFeatureRenderer}'s
     * {@code getLayerColorSafe}): the item's color for that layer, white for an untinted quad or a
     * layer the item has no color for (most generated items: their quads carry the layer's index).
     */
    static int itemColor(int[] tintLayers, int tint) {
        return tint >= 0 && tint < tintLayers.length ? tintLayers[tint] : -1;
    }

    /** The game's overlay and light: two 16-bit halves, as its vertices hold them. */
    private static int packShorts(int packed) {
        return (packed & 0xFFFF) | (packed >> 16 & 0xFFFF) << 16;
    }

    private @Nullable RenderPipeline pipeline(RenderType renderType) {
        if (pipelines.containsKey(renderType)) {
            return pipelines.get(renderType);
        }
        // The solid phase only: the game puts a model there when it doesn't blend, and draws those
        // with depth, so the order of its draws doesn't show.
        RenderPipeline pipeline = renderType.hasBlending() || renderType.isOutline() || isWaterMask(renderType)
                ? null
                : InstancedPipelines.of(renderType.pipeline());
        if (pipeline != null && RenderSystem.getCompiledPipelineNullable(pipeline) == null) {
            // The backend can't draw it (it refuses the copy rather than skip its draws).
            LOGGER.warn(
                    "Entity models drawn with {} go the game's way: the backend has no instanced pipeline for it",
                    renderType.pipeline().getLocation());
            pipeline = null;
        }
        pipelines.put(renderType, pipeline);
        return pipeline;
    }

    /** The instanced copy of a translucent pipeline whose model draws as an opaque one. */
    private @Nullable RenderPipeline opaqueSkinPipeline(RenderType renderType) {
        RenderPipeline pipeline = InstancedPipelines.of(renderType.pipeline());
        if (pipeline != null && RenderSystem.getCompiledPipelineNullable(pipeline) == null) {
            return null;
        }
        return pipeline;
    }

    @SuppressWarnings("ReferenceEquality") // The game tells the water mask apart by identity too.
    private static boolean isWaterMask(RenderType renderType) {
        return renderType == RenderTypes.waterMask();
    }

    private static @Nullable ModelMesh mesh(Model<?> model) {
        ModelMesh mesh;
        if (MESHES.containsKey(model)) {
            mesh = MESHES.get(model);
        } else {
            mesh = ModelMesh.bake(model);
            if (mesh != null && mesh.rows() > InstancedPipelines.ROWS) {
                // More parts than a block holds: the game draws it.
                mesh.close();
                mesh = null;
            }
            MESHES.put(model, mesh);
        }
        if (mesh != null) {
            mesh.lastUsedFrame = frame;
        }
        return mesh;
    }

    private Batch batch(RenderType renderType, RenderPipeline pipeline, ModelMesh mesh) {
        if (used == pool.size()) {
            pool.add(new Batch());
        }
        Batch batch = pool.get(used++);
        batch.type = prepared.computeIfAbsent(renderType, RenderType::prepare);
        batch.pipeline = pipeline;
        batch.mesh = mesh;
        batch.capacity = InstancedPipelines.ROWS / mesh.rows();
        batch.count = 0;
        maxQuads = Math.max(maxQuads, mesh.quads());
        return batch;
    }

    /**
     * Hook: the groups are prepared ({@code finishPrepare}): lays the batches out in two buffers,
     * each batch's rows where a uniform block may start, and uploads them.
     */
    public void upload() {
        if (used == 0) {
            return;
        }
        int alignment = Math.max(
                ModelMesh.ROW_BYTES,
                RenderSystem.getDevice().getDeviceInfo().limits().minUniformOffsetAlignment());
        long rowBytes = 0;
        long instanceBytes = 0;
        for (int i = 0; i < used; i++) {
            Batch batch = pool.get(i);
            ModelMesh mesh = Objects.requireNonNull(batch.mesh);
            mesh.upload();
            rowBytes = (rowBytes + alignment - 1) / alignment * alignment;
            batch.rowsOffset = rowBytes;
            rowBytes += (long) batch.count * mesh.rows() * ModelMesh.ROW_BYTES;
            batch.instancesOffset = instanceBytes;
            instanceBytes += (long) batch.count * InstancedPipelines.INSTANCE.getVertexSize();
        }
        ByteBuffer rows = room(rowData, rowBytes);
        ByteBuffer instances = room(instanceData, instanceBytes);
        rowData = rows;
        instanceData = instances;
        for (int i = 0; i < used; i++) {
            Batch batch = pool.get(i);
            int floats = batch.count * Objects.requireNonNull(batch.mesh).rows() * 4;
            rows.position((int) batch.rowsOffset);
            rows.asFloatBuffer().put(batch.rows, 0, floats);
            instances.position((int) batch.instancesOffset);
            instances.asIntBuffer().put(batch.instances, 0, batch.count * 4);
        }
        // The backend binds a whole block's worth from where a batch's rows start: room past the end.
        long rowCapacity = rowBytes + (long) InstancedPipelines.ROWS * ModelMesh.ROW_BYTES;
        GpuBuffer rowTarget =
                fit(rowBuffer, rowCapacity, GpuBuffer.USAGE_UNIFORM | GpuBuffer.USAGE_COPY_DST, "part rows");
        GpuBuffer instanceTarget =
                fit(instanceBuffer, instanceBytes, GpuBuffer.USAGE_VERTEX | GpuBuffer.USAGE_COPY_DST, "instances");
        rowBuffer = rowTarget;
        instanceBuffer = instanceTarget;
        CommandEncoder encoder = RenderSystem.getDevice().createCommandEncoder();
        encoder.writeToBuffer(rowTarget.slice(0, rowBytes), rows.slice(0, (int) rowBytes));
        encoder.writeToBuffer(instanceTarget.slice(0, instanceBytes), instances.slice(0, (int) instanceBytes));
        RenderSystem.getSequentialBuffer(PrimitiveTopology.QUADS).getBuffer(maxQuads * 6);
    }

    private static ByteBuffer room(@Nullable ByteBuffer buffer, long bytes) {
        if (buffer != null && buffer.capacity() >= bytes) {
            buffer.clear();
            return buffer;
        }
        int capacity = Integer.highestOneBit((int) Math.max(bytes, 1024) - 1) << 1;
        return ByteBuffer.allocateDirect(capacity).order(ByteOrder.nativeOrder());
    }

    private static GpuBuffer fit(@Nullable GpuBuffer buffer, long bytes, int usage, String name) {
        if (buffer != null && buffer.size() >= bytes) {
            return buffer;
        }
        if (buffer != null) {
            buffer.close();
        }
        long size = Long.highestOneBit(Math.max(bytes, 4096) - 1) << 1;
        return RenderSystem.getDevice().createBuffer(() -> "Incari instanced models: " + name, usage, size);
    }

    /** Hook: after the game's draws of a group ({@code executeGroup}), this group's instanced draws. */
    public void draw(int groupIndex, @Nullable OitStage stage, RenderPass renderPass) {
        GpuBuffer rowSource = rowBuffer;
        GpuBuffer instanceSource = instanceBuffer;
        if (stage != null || groupIndex >= groups.size() || rowSource == null || instanceSource == null) {
            return;
        }
        RenderSystem.AutoStorageIndexBuffer indices = RenderSystem.getSequentialBuffer(PrimitiveTopology.QUADS);
        for (Batch batch : groups.get(groupIndex)) {
            PreparedRenderType type = batch.type;
            ModelMesh mesh = batch.mesh;
            RenderPipeline pipeline = batch.pipeline;
            GpuBuffer vertices = mesh == null ? null : mesh.vertices();
            if (batch.count == 0 || type == null || mesh == null || pipeline == null || vertices == null) {
                continue;
            }
            renderPass.pushDebugGroup(() -> "Instanced " + type.name());
            renderPass.setPipeline(RenderSystem.getCompiledPipeline(pipeline));
            if (type.scissorState().enabled()) {
                renderPass.enableScissor(
                        type.scissorState().x(),
                        type.scissorState().y(),
                        type.scissorState().width(),
                        type.scissorState().height());
            }
            RenderSystem.bindDefaultUniforms(renderPass);
            renderPass.setUniform("DynamicTransforms", type.dynamicTransforms());
            renderPass.setUniform(
                    InstancedPipelines.BLOCK,
                    rowSource.slice(batch.rowsOffset, (long) batch.count * mesh.rows() * ModelMesh.ROW_BYTES));
            renderPass.setVertexBuffer(0, vertices.slice());
            renderPass.setVertexBuffer(
                    1,
                    instanceSource.slice(
                            batch.instancesOffset, (long) batch.count * InstancedPipelines.INSTANCE.getVertexSize()));
            for (PreparedRenderType.Texture texture : type.textures()) {
                renderPass.setUniform(texture.name(), texture.textureView(), texture.sampler());
            }
            renderPass.setIndexBuffer(indices.getBuffer(), indices.type());
            renderPass.drawIndexed(mesh.quads() * 6, batch.count, 0, 0, 0);
            renderPass.popDebugGroup();
            if (!announced) {
                announced = true;
                LOGGER.info("Entity models are drawn instanced (first: {} of {})", batch.count, type.name());
            }
        }
    }

    /** Hook: the frame is drawn ({@code finishExecute}). */
    public void endFrame() {
        groups.clear();
        group = null;
        open.clear();
        prepared.clear();
        for (int i = 0; i < used; i++) {
            Batch batch = pool.get(i);
            batch.type = null;
            batch.pipeline = null;
            batch.mesh = null;
        }
        used = 0;
        maxQuads = 0;
        if (++frame % MESH_IDLE_FRAMES == 0) {
            for (Iterator<Map.Entry<List<BakedQuad>, @Nullable ModelMesh>> it =
                            ITEM_MESHES.entrySet().iterator();
                    it.hasNext(); ) {
                ModelMesh mesh = it.next().getValue();
                if (mesh != null && frame - mesh.lastUsedFrame > MESH_IDLE_FRAMES) {
                    mesh.close();
                    it.remove();
                }
            }
            for (Iterator<Map.Entry<Model<?>, @Nullable ModelMesh>> it =
                            MESHES.entrySet().iterator();
                    it.hasNext(); ) {
                ModelMesh mesh = it.next().getValue();
                if (mesh != null && frame - mesh.lastUsedFrame > MESH_IDLE_FRAMES) {
                    mesh.close();
                    it.remove();
                }
            }
        }
    }

    /** Hook: the renderer closes. */
    public void close() {
        if (rowBuffer != null) {
            rowBuffer.close();
        }
        if (instanceBuffer != null) {
            instanceBuffer.close();
        }
    }
}
