package incari.client.modules;

import it.unimi.dsi.fastutil.ints.IntList;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.core.BlockPos;
import net.minecraft.util.Util;
import net.minecraft.world.attribute.EnvironmentAttributes;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.ai.attributes.Attributes;
import net.minecraft.world.entity.boss.enderdragon.EndCrystal;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;

/**
 * Breaks a module predicts ({@code Game.predictBreaks}, Fast Crystal): an end crystal the player
 * hits, and a charged respawn anchor they set off where it explodes, go from this client at once
 * instead of when the server's answer comes back a ping later. Prediction only, as the game's own
 * for a broken block: the hooks run after the player's own click has sent its one packet, and send
 * nothing. What the server keeps comes back: an anchor in the game's block prediction (the server's
 * acknowledgement restores its state), a crystal from {@link #tick} if the server hasn't removed it
 * in time. Game thread.
 */
public final class ModuleBreaks {

    /** The least and most a crystal waits for the server before coming back, in milliseconds. */
    private static final long MIN_WAIT = 600;

    private static final long MAX_WAIT = 3000;

    private static boolean crystals;
    private static boolean anchors;

    /** Crystals taken away here that the server hasn't removed yet: rarely more than a few. */
    private static final List<Pending> pending = new ArrayList<>();

    private ModuleBreaks() {}

    /** {@code Game.predictBreaks}'s. */
    static void hold(boolean crystals, boolean anchors) {
        ModuleBreaks.crystals = crystals;
        ModuleBreaks.anchors = anchors;
    }

    /**
     * Hook ({@code MultiPlayerGameMode.attack}, after the attack packet is sent): takes away a
     * crystal the hit breaks. Only where the server breaks it too: a hit doing no damage (weakness
     * with a bare hand) or an invulnerable crystal (the dragon fight's) stays.
     */
    public static void attacked(Player player, Entity entity) {
        if (!crystals
                || !(entity instanceof EndCrystal crystal)
                || !(crystal.level() instanceof ClientLevel level)
                || crystal.isRemoved()
                || crystal.isInvulnerable()
                || player.isSpectator()
                || player.getAttributeValue(Attributes.ATTACK_DAMAGE) <= 0) {
            return;
        }
        pending.add(new Pending(crystal, level, Util.getMillis() + wait(player)));
        level.removeEntity(crystal.getId(), Entity.RemovalReason.KILLED);
    }

    /** Hook ({@code ClientPacketListener.handleRemoveEntities}, first): the server removed these. */
    public static void removed(IntList ids) {
        if (pending.isEmpty()) {
            return;
        }
        for (int i = 0; i < ids.size(); i++) {
            int id = ids.getInt(i);
            pending.removeIf(p -> p.crystal.getId() == id);
        }
    }

    /**
     * Hook ({@code RespawnAnchorBlock.useWithoutItem}, client side, the anchor charged): takes the
     * anchor away where it explodes rather than setting the spawn. The use runs in the game's block
     * prediction, so the server's acknowledgement puts the anchor back if it stays.
     */
    public static void anchorUsed(Level level, BlockPos pos) {
        if (!anchors
                || !level.isClientSide()
                || level.environmentAttributes().getValue(EnvironmentAttributes.RESPAWN_ANCHOR_WORKS, pos)) {
            return;
        }
        level.setBlock(pos, Blocks.AIR.defaultBlockState(), Block.UPDATE_ALL_IMMEDIATE);
    }

    /** From {@code IncariModules.tick}: puts back crystals the server kept past their wait. */
    @SuppressWarnings("ReferenceEquality") // the same level object, not an equal one
    static void tick(Minecraft minecraft) {
        if (pending.isEmpty()) {
            return;
        }
        long now = Util.getMillis();
        for (int i = pending.size() - 1; i >= 0; i--) {
            Pending p = pending.get(i);
            if (now < p.deadline) {
                continue;
            }
            pending.remove(i);
            EndCrystal old = p.crystal;
            ClientLevel level = p.level;
            // Another world now, or the server reused the id: nothing to put back.
            if (minecraft.level != level
                    || level.getEntity(old.getId()) != null
                    || !level.isLoaded(old.blockPosition())) {
                continue;
            }
            EndCrystal back = new EndCrystal(level, old.getX(), old.getY(), old.getZ());
            back.syncPacketPositionCodec(old.getX(), old.getY(), old.getZ());
            back.setId(old.getId());
            back.setUUID(old.getUUID());
            back.setBeamTarget(old.getBeamTarget());
            back.setShowBottom(old.showsBottom());
            back.time = old.time;
            level.addEntity(back);
        }
    }

    /** From {@code IncariModules} on leaving a world: its crystals are gone with it. */
    static void leave() {
        pending.clear();
    }

    /** How long a crystal waits for the server: twice the ping and some, within bounds. */
    private static long wait(Player player) {
        ClientPacketListener connection = Minecraft.getInstance().getConnection();
        PlayerInfo info = connection == null ? null : connection.getPlayerInfo(player.getUUID());
        long ping = info == null ? 0 : info.getLatency();
        return Math.min(MAX_WAIT, Math.max(MIN_WAIT, 2 * ping + 300));
    }

    private record Pending(EndCrystal crystal, ClientLevel level, long deadline) {}
}
