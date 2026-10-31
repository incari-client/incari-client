package incari.client.modules;

import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.blaze3d.vertex.BufferBuilder;
import com.mojang.blaze3d.vertex.ByteBufferBuilder;
import com.mojang.blaze3d.vertex.DefaultVertexFormat;
import com.mojang.blaze3d.vertex.MeshData;
import com.mojang.logging.LogUtils;
import com.mojang.renderpearl.api.buffers.GpuBuffer;
import com.mojang.renderpearl.api.buffers.GpuBufferSlice;
import com.mojang.renderpearl.api.commands.RenderPass;
import com.mojang.renderpearl.api.pipeline.BlendFactor;
import com.mojang.renderpearl.api.pipeline.BlendFunction;
import com.mojang.renderpearl.api.pipeline.ColorTargetState;
import com.mojang.renderpearl.api.pipeline.PrimitiveTopology;
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import incari.features.customsky.SkyLayer;
import java.io.BufferedReader;
import java.io.IOException;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.EnumMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Optional;
import java.util.stream.Collectors;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.renderer.BindGroupLayouts;
import net.minecraft.client.renderer.SkyRenderer;
import net.minecraft.client.renderer.texture.AbstractTexture;
import net.minecraft.client.renderer.texture.TextureManager;
import net.minecraft.core.Holder;
import net.minecraft.resources.Identifier;
import net.minecraft.server.packs.resources.Resource;
import net.minecraft.server.packs.resources.ResourceManager;
import net.minecraft.world.clock.WorldClock;
import net.minecraft.world.level.Level;
import org.joml.Matrix4f;
import org.joml.Vector4f;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;

/**
 * Resource packs' custom skies ({@code Game.showPackSkies}, Custom Sky): OptiFine's sky layers
 * ({@link SkyLayer}) drawn as textured cubes in the sky pass, after the sunrise and before the sun,
 * moon and stars, as OptiFine draws them. Overworld only, as {@code world0} is.
 *
 * <p>Render thread. The files are read and the textures loaded the first frame the module's on
 * after a resource reload, and the textures released when it's off, so a pack's large sky costs
 * nothing until it's wanted. The hooks are a field read or two while it's off.
 */
public final class ModuleSkyLayers {

    private static final Logger LOGGER = LogUtils.getLogger();

    private static final String[] FOLDERS = {"optifine/sky/world0", "mcpatcher/sky/world0"};

    /** Each blend's pipeline: the end sky's, but for the blend. */
    private static final EnumMap<SkyLayer.Blend, RenderPipeline> PIPELINES = new EnumMap<>(SkyLayer.Blend.class);

    static {
        for (SkyLayer.Blend blend : SkyLayer.Blend.values()) {
            PIPELINES.put(
                    blend,
                    RenderPipeline.builder()
                            .withLocation(Identifier.fromNamespaceAndPath(
                                    "incari",
                                    "pipeline/sky_layer_" + blend.name().toLowerCase(Locale.ROOT)))
                            .withBindGroupLayout(BindGroupLayouts.GLOBALS)
                            .withBindGroupLayout(BindGroupLayouts.PROJECTION)
                            .withBindGroupLayout(BindGroupLayouts.DYNAMIC_TRANSFORMS)
                            // The end sky's stages: the page already has them translated.
                            .withVertexShader("core/position_tex_color")
                            .withFragmentShader("core/position_tex_color")
                            .withBindGroupLayout(BindGroupLayouts.SAMPLER0)
                            .withColorTargetState(colorTarget(blend))
                            .withVertexBinding(0, DefaultVertexFormat.POSITION_TEX_COLOR)
                            .withPrimitiveTopology(PrimitiveTopology.QUADS)
                            .build());
        }
    }

    /** The brightness asked for, 0 while off. */
    private static float brightness;

    /** Whether the layers must be read again: a resource reload, or never read. */
    private static boolean stale = true;

    /** The sky renderer of the last frame: the game makes a new one after a resource reload. */
    private static @Nullable SkyRenderer lastRenderer;

    private static List<Loaded> layers = List.of();

    /** This frame's sky, from {@link #extract}: whether to draw, the time, the weather as shown. */
    private static boolean shown;

    private static long timeOfDay;
    private static float rain;
    private static float thunder;

    private static @Nullable GpuBuffer cube;

    private static final Matrix4f POSE = new Matrix4f();
    private static final Vector4f COLOR = new Vector4f();

    private ModuleSkyLayers() {}

    /** {@code Game.showPackSkies}'s: 0 for none. */
    static void show(float level) {
        brightness = Math.max(0, Math.min(1, level));
    }

    /**
     * Hook ({@code SkyRenderer.extractRenderState}, the overworld's sky): reads the time and the
     * weather as shown (Weather Changer's, if it's on), and the layers if they're due. The game
     * makes {@code renderer} again after a resource reload, so a new one means the packs may have
     * changed.
     */
    @SuppressWarnings("ReferenceEquality") // A new renderer, not an equal one.
    public static void extract(SkyRenderer renderer, ClientLevel level, float partialTicks) {
        if (renderer != lastRenderer) {
            lastRenderer = renderer;
            stale = true;
        }
        if (brightness <= 0) {
            shown = false;
            if (!layers.isEmpty()) {
                release();
            }
            return;
        }
        if (stale) {
            release();
            stale = false;
            layers = load();
        }
        shown = !layers.isEmpty() && level.dimension().equals(Level.OVERWORLD);
        if (!shown) {
            return;
        }
        Optional<Holder<WorldClock>> clock = level.dimensionType().defaultClock();
        // A level without a clock stands at noon.
        timeOfDay = clock.isPresent()
                ? level.clockManager().getInstance(clock.get()).totalTicks()
                : 6000;
        rain = ModuleSky.shownRain(level, partialTicks);
        thunder = ModuleSky.shownThunder(level, partialTicks);
    }

    /**
     * Hook ({@code SkyRenderer.render}, after the sunrise and before the sun): draws the layers,
     * turned with the sun at {@code sunAngle} (radians).
     */
    public static void render(RenderPass pass, float sunAngle) {
        if (!shown) {
            return;
        }
        GpuBuffer vertices = cube();
        RenderSystem.AutoStorageIndexBuffer indices = RenderSystem.getSequentialBuffer(PrimitiveTopology.QUADS);
        GpuBuffer indexBuffer = indices.getBuffer(36);
        for (int i = 0; i < layers.size(); i++) {
            Loaded loaded = layers.get(i);
            SkyLayer layer = loaded.layer;
            float alpha = layer.brightness(timeOfDay, rain, thunder) * brightness;
            if (alpha <= 1.0E-4F) {
                continue;
            }
            POSE.set(RenderSystem.getModelViewStack());
            float angle = layer.angle(sunAngle);
            if (angle != 0) {
                POSE.rotate(angle, layer.axisX(), layer.axisY(), layer.axisZ());
            }
            color(layer.blend(), alpha);
            GpuBufferSlice transforms = RenderSystem.getDynamicUniforms().writeTransform(POSE, COLOR);
            pass.pushDebugGroup(() -> "Custom sky layer");
            pass.setPipeline(RenderSystem.getCompiledPipeline(loaded.pipeline));
            RenderSystem.bindDefaultUniforms(pass);
            pass.setUniform("DynamicTransforms", transforms);
            pass.setUniform("Sampler0", loaded.texture.getTextureView(), loaded.texture.getSampler());
            pass.setVertexBuffer(0, vertices.slice());
            pass.setIndexBuffer(indexBuffer, indices.type());
            pass.drawIndexed(36, 1, 0, 0, 0);
            pass.popDebugGroup();
        }
    }

    /**
     * OptiFine's blends: the source and destination factors it sets for each, and which of the
     * color's channels carry the layer's brightness.
     */
    private static ColorTargetState colorTarget(SkyLayer.Blend blend) {
        BlendFunction function =
                switch (blend) {
                    case ALPHA -> BlendFunction.TRANSLUCENT;
                    case ADD -> new BlendFunction(BlendFactor.SRC_ALPHA, BlendFactor.ONE);
                    case SUBTRACT -> new BlendFunction(BlendFactor.ONE_MINUS_DST_COLOR, BlendFactor.ZERO);
                    case MULTIPLY -> new BlendFunction(BlendFactor.DST_COLOR, BlendFactor.ONE_MINUS_SRC_ALPHA);
                    case DODGE -> new BlendFunction(BlendFactor.ONE, BlendFactor.ONE);
                    case BURN -> new BlendFunction(BlendFactor.ZERO, BlendFactor.ONE_MINUS_SRC_COLOR);
                    case SCREEN -> new BlendFunction(BlendFactor.ONE, BlendFactor.ONE_MINUS_SRC_COLOR);
                    case OVERLAY -> new BlendFunction(BlendFactor.DST_COLOR, BlendFactor.SRC_COLOR);
                    case REPLACE -> null;
                };
        return function == null ? ColorTargetState.DEFAULT : new ColorTargetState(function);
    }

    private static void color(SkyLayer.Blend blend, float alpha) {
        switch (blend) {
            case ALPHA, ADD, REPLACE -> COLOR.set(1, 1, 1, alpha);
            case MULTIPLY -> COLOR.set(alpha, alpha, alpha, alpha);
            default -> COLOR.set(alpha, alpha, alpha, 1);
        }
    }

    /** Every pack's layers in the first folder that has any, in their files' order. */
    private static List<Loaded> load() {
        Minecraft minecraft = Minecraft.getInstance();
        ResourceManager resources = minecraft.getResourceManager();
        TextureManager textures = minecraft.getTextureManager();
        for (String folder : FOLDERS) {
            Map<Identifier, Resource> files = resources.listResources(
                    folder,
                    id -> id.getNamespace().equals("minecraft") && id.getPath().endsWith(".properties"));
            List<Identifier> ordered = files.keySet().stream()
                    .filter(id -> id.getPath().lastIndexOf('/') == folder.length())
                    .sorted(Comparator.comparingInt((Identifier id) -> SkyLayer.order(id.getPath()))
                            .thenComparing(Identifier::getPath))
                    .toList();
            List<Loaded> loaded = new ArrayList<>();
            for (Identifier file : ordered) {
                Resource resource = files.get(file);
                if (resource == null || SkyLayer.order(file.getPath()) == Integer.MAX_VALUE) {
                    // sun.properties, moon_phases.properties: not layers.
                    continue;
                }
                SkyLayer layer = read(resource, file);
                if (layer == null) {
                    continue;
                }
                Identifier source = Identifier.parse(layer.source());
                if (resources.getResource(source).isEmpty()) {
                    LOGGER.warn("Custom sky {}: no texture {}", file, source);
                    continue;
                }
                RenderPipeline pipeline = PIPELINES.get(layer.blend());
                if (pipeline != null) {
                    loaded.add(new Loaded(layer, source, textures.getTexture(source), pipeline));
                }
            }
            if (!loaded.isEmpty()) {
                LOGGER.info("Custom sky: {} layers from {}", loaded.size(), folder);
                return List.copyOf(loaded);
            }
        }
        return List.of();
    }

    private static @Nullable SkyLayer read(Resource resource, Identifier file) {
        try (BufferedReader reader = resource.openAsReader()) {
            SkyLayer layer = SkyLayer.parse(reader.lines().collect(Collectors.joining("\n")), file.getPath());
            if (layer == null) {
                LOGGER.warn("Custom sky {}: can't be drawn (fade times or source)", file);
            }
            return layer;
        } catch (IOException | RuntimeException e) {
            LOGGER.warn("Custom sky {}: can't be read: {}", file, e.toString());
            return null;
        }
    }

    private static void release() {
        TextureManager textures = Minecraft.getInstance().getTextureManager();
        for (Loaded loaded : layers) {
            textures.release(loaded.source);
        }
        layers = List.of();
        shown = false;
        // Read again when next wanted: the packs may change while it's off.
        stale = true;
    }

    /**
     * A cube 200 across around the camera, each face a sixth of the texture: OptiFine's layout, the
     * bottom face from the grid's middle of the lower row, and its faces in OptiFine's order.
     */
    private static GpuBuffer cube() {
        GpuBuffer built = cube;
        if (built != null) {
            return built;
        }
        try (ByteBufferBuilder bytes =
                ByteBufferBuilder.exactlySized(24 * DefaultVertexFormat.POSITION_TEX_COLOR.getVertexSize())) {
            BufferBuilder builder =
                    new BufferBuilder(bytes, PrimitiveTopology.QUADS, DefaultVertexFormat.POSITION_TEX_COLOR);
            Matrix4f pose = new Matrix4f();
            face(builder, pose, 4);
            face(builder, pose.rotationX((float) (Math.PI / 2)), 1);
            face(builder, pose.rotationX((float) (-Math.PI / 2)), 0);
            face(builder, pose.rotationZ((float) (Math.PI / 2)), 5);
            face(builder, pose.rotationZ((float) Math.PI), 2);
            face(builder, pose.rotationZ((float) (Math.PI * 1.5)), 3);
            try (MeshData mesh = builder.buildOrThrow()) {
                built = RenderSystem.getDevice().createBuffer(() -> "Custom sky cube", 40, mesh.vertexBuffer());
            }
        }
        cube = built;
        return built;
    }

    private static void face(BufferBuilder builder, Matrix4f pose, int side) {
        float u = side % 3 / 3.0F;
        int row = side / 3;
        float v = row * 0.5F;
        float u1 = u + 1 / 3.0F;
        float v1 = v + 0.5F;
        builder.addVertex(pose, -100.0F, -100.0F, -100.0F).setUv(u, v).setColor(-1);
        builder.addVertex(pose, -100.0F, -100.0F, 100.0F).setUv(u, v1).setColor(-1);
        builder.addVertex(pose, 100.0F, -100.0F, 100.0F).setUv(u1, v1).setColor(-1);
        builder.addVertex(pose, 100.0F, -100.0F, -100.0F).setUv(u1, v).setColor(-1);
    }

    private record Loaded(SkyLayer layer, Identifier source, AbstractTexture texture, RenderPipeline pipeline) {}
}
