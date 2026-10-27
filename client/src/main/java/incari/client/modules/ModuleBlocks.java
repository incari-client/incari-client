package incari.client.modules;

import com.mojang.logging.LogUtils;
import com.mojang.serialization.Dynamic;
import incari.framework.game.Blocks;
import incari.framework.game.Mirror;
import java.util.HashMap;
import java.util.Map;
import java.util.Optional;
import net.minecraft.SharedConstants;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.resources.Identifier;
import net.minecraft.util.datafix.fixes.References;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Rotation;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.StateDefinition;
import net.minecraft.world.level.block.state.properties.Property;
import net.minecraft.world.level.chunk.LevelChunk;
import net.minecraft.world.level.chunk.LevelChunkSection;
import net.minecraft.world.level.chunk.status.ChunkStatus;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;

/**
 * The game's block states and the world's blocks as modules see them ({@link Blocks}): a state is
 * its number in the game's state registry ({@code Block.getId}), air's being 0. Game and render
 * thread. Reading the world never loads a chunk: one the client doesn't hold is unknown.
 */
final class ModuleBlocks implements Blocks {

    private static final Logger LOGGER = LogUtils.getLogger();

    /** The data version from which the game names a state's fields {@code id} and {@code properties}. */
    private static final int NEW_STATE_NAMES = 5006;

    /** States looked up before, by data version and text. */
    private final Map<String, Integer> resolved = new HashMap<>();

    private final Map<Item, String> itemIds = new HashMap<>();

    @Override
    public int state(String name, Map<String, String> properties, int dataVersion) {
        String key = dataVersion + " " + name + properties;
        Integer known = resolved.get(key);
        if (known != null) {
            return known;
        }
        int state = parse(name, properties, true);
        int current = SharedConstants.getCurrentVersion().dataVersion().version();
        if (state == UNKNOWN && dataVersion > 0 && dataVersion < current) {
            // Renamed since (grass is short grass now) or a property changed (walls' sides): only then
            // the data fixer, which the browser build makes the first time it's needed (ADR 0025).
            state = upgrade(name, properties, dataVersion, current);
        }
        if (state == UNKNOWN) {
            // A property this version doesn't know: the block as near as can be, rather than none.
            state = parse(name, properties, false);
        }
        resolved.put(key, state);
        return state;
    }

    /**
     * The state with these properties; with {@code strict}, unknown if any property doesn't fit the
     * block, else those are left at the block's defaults.
     */
    private static int parse(String name, Map<String, String> properties, boolean strict) {
        Identifier id = Identifier.tryParse(name);
        Optional<Block> block = id == null ? Optional.empty() : BuiltInRegistries.BLOCK.getOptional(id);
        if (block.isEmpty()) {
            return UNKNOWN;
        }
        StateDefinition<Block, BlockState> definition = block.get().getStateDefinition();
        BlockState state = block.get().defaultBlockState();
        for (Map.Entry<String, String> entry : properties.entrySet()) {
            Property<?> property = definition.getProperty(entry.getKey());
            BlockState with = property == null ? null : with(state, property, entry.getValue());
            if (with == null) {
                if (strict) {
                    return UNKNOWN;
                }
                continue;
            }
            state = with;
        }
        return Block.getId(state);
    }

    private static <T extends Comparable<T>> @Nullable BlockState with(
            BlockState state, Property<T> property, String value) {
        Optional<T> parsed = property.getValue(value);
        return parsed.isPresent() ? state.setValue(property, parsed.get()) : null;
    }

    /** Runs the state through the game's data fixer from {@code dataVersion}; unknown if it fails. */
    private static int upgrade(String name, Map<String, String> properties, int dataVersion, int current) {
        boolean newNames = dataVersion >= NEW_STATE_NAMES;
        CompoundTag tag = new CompoundTag();
        tag.putString(newNames ? "id" : "Name", name);
        if (!properties.isEmpty()) {
            CompoundTag values = new CompoundTag();
            properties.forEach(values::putString);
            tag.put(newNames ? "properties" : "Properties", values);
        }
        try {
            Tag fixed = Minecraft.getInstance()
                    .getFixerUpper()
                    .update(References.BLOCK_STATE, new Dynamic<>(NbtOps.INSTANCE, tag), dataVersion, current)
                    .getValue();
            if (!(fixed instanceof CompoundTag out)) {
                return UNKNOWN;
            }
            String fixedName = out.getStringOr("id", out.getStringOr("Name", ""));
            Map<String, String> fixedProperties = new HashMap<>();
            CompoundTag values = out.getCompound("properties").orElseGet(() -> out.getCompoundOrEmpty("Properties"));
            for (String key : values.keySet()) {
                fixedProperties.put(key, values.getStringOr(key, ""));
            }
            return parse(fixedName, fixedProperties, false);
        } catch (RuntimeException e) {
            LOGGER.warn("Couldn't upgrade {}{} from data version {}: {}", name, properties, dataVersion, e.toString());
            return UNKNOWN;
        }
    }

    private static BlockState of(int state) {
        return Block.stateById(state);
    }

    @Override
    public int transform(int state, Mirror mirror, int clockwiseTurns) {
        if (state < 0) {
            return state;
        }
        BlockState turned = of(state)
                .mirror(
                        switch (mirror) {
                            case NONE -> net.minecraft.world.level.block.Mirror.NONE;
                            // Front to back flips x; left to right flips z.
                            case X -> net.minecraft.world.level.block.Mirror.FRONT_BACK;
                            case Z -> net.minecraft.world.level.block.Mirror.LEFT_RIGHT;
                        })
                .rotate(
                        switch (Math.floorMod(clockwiseTurns, 4)) {
                            case 1 -> Rotation.CLOCKWISE_90;
                            case 2 -> Rotation.CLOCKWISE_180;
                            case 3 -> Rotation.COUNTERCLOCKWISE_90;
                            default -> Rotation.NONE;
                        });
        return Block.getId(turned);
    }

    @Override
    public int at(int x, int y, int z) {
        ClientLevel level = Minecraft.getInstance().level;
        if (level == null || level.isOutsideBuildHeight(y)) {
            return UNKNOWN;
        }
        LevelChunk chunk = level.getChunkSource().getChunk(x >> 4, z >> 4, ChunkStatus.FULL, false);
        if (chunk == null) {
            return UNKNOWN;
        }
        LevelChunkSection section = chunk.getSection(chunk.getSectionIndex(y));
        // Hot path: the section's palette, not a position object.
        return section.hasOnlyAir() ? AIR : Block.getId(section.getBlockState(x & 15, y & 15, z & 15));
    }

    @Override
    public boolean isAir(int state) {
        return state >= 0 && of(state).isAir();
    }

    @Override
    public int block(int state) {
        return state < 0 ? state : Block.getId(of(state).getBlock().defaultBlockState());
    }

    @Override
    @SuppressWarnings("ReferenceEquality") // Items are one object each.
    public String itemId(int state) {
        if (state < 0) {
            return "";
        }
        Item item = of(state).getBlock().asItem();
        if (item == Items.AIR) {
            return "";
        }
        return itemIds.computeIfAbsent(
                item, i -> BuiltInRegistries.ITEM.getKey(i).toString());
    }

    @Override
    public String name(int state) {
        return state < 0 ? "" : of(state).getBlock().getName().getString();
    }
}
