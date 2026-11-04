package incari.client.modules;

import incari.framework.game.NoRender;
import net.minecraft.client.particle.FireworkParticles;
import net.minecraft.client.particle.ItemPickupParticle;
import net.minecraft.client.particle.Particle;
import net.minecraft.client.particle.TerrainParticle;
import net.minecraft.client.renderer.entity.state.ItemEntityRenderState;
import net.minecraft.core.particles.ParticleType;
import net.minecraft.core.particles.ParticleTypes;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.ExperienceOrb;
import net.minecraft.world.entity.item.FallingBlockEntity;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.entity.projectile.FireworkRocketEntity;
import org.jspecify.annotations.Nullable;

/**
 * What a module leaves out of the world drawn ({@code Game.overrideNoRender}, No Render). Hidden
 * entities aren't extracted and hidden particles aren't made, so they cost nothing; the entities are
 * still in the world. Render thread, per entity and particle: hot paths, so the choice is flattened
 * into fields once per change, and while no module holds one every hook is a field read.
 */
public final class ModuleNoRender {

    /** Whether a module holds anything: the one field the hooks read while none does. */
    private static boolean held;

    private static boolean hideItems;
    private static boolean stillItems;
    private static boolean oneCopy;
    private static boolean hideOrbs;
    private static boolean hideFalling;
    private static boolean hideFireworks;
    private static boolean hideGrowth;
    private static boolean hideBreaking;
    private static boolean hideExplosions;
    private static boolean hideParticles;

    private ModuleNoRender() {}

    /** {@code Game.overrideNoRender}'s. */
    static void hold(@Nullable NoRender noRender) {
        if (noRender == null) {
            held = false;
            hideItems = stillItems = oneCopy = hideOrbs = hideFalling = hideFireworks = false;
            hideGrowth = hideBreaking = hideExplosions = hideParticles = false;
            return;
        }
        hideItems = noRender.items() == NoRender.Items.HIDDEN;
        stillItems = noRender.items() == NoRender.Items.STILL;
        oneCopy = noRender.oneCopy();
        hideOrbs = noRender.orbs();
        hideFalling = noRender.fallingBlocks();
        hideFireworks = noRender.fireworks();
        hideGrowth = noRender.growth();
        hideBreaking = noRender.breaking();
        hideExplosions = noRender.explosions();
        hideParticles = noRender.allParticles();
        held = true;
    }

    /** Hook ({@code EntityRenderDispatcher.shouldRender}): whether {@code entity} is left undrawn. */
    public static boolean hides(Entity entity) {
        if (!held) {
            return false;
        }
        return entity instanceof ItemEntity
                ? hideItems
                : entity instanceof ExperienceOrb
                        ? hideOrbs
                        : entity instanceof FallingBlockEntity
                                ? hideFalling
                                : entity instanceof FireworkRocketEntity && hideFireworks;
    }

    /**
     * Hook ({@code ItemEntityRenderer.extractRenderState}, at its end): an item lying still is drawn
     * at age 0, where its bob and spin stay put (its own random turn is kept); one copy per stack.
     */
    public static void item(ItemEntityRenderState state) {
        if (!held) {
            return;
        }
        if (stillItems) {
            state.ageInTicks = 0;
        }
        if (oneCopy && state.count > 1) {
            state.count = 1;
        }
    }

    /** Hook ({@code ParticleEngine.createParticle}, first): whether a particle of {@code type} isn't made. */
    @SuppressWarnings("ReferenceEquality") // the registry's one type object each, not equal ones
    public static boolean hides(ParticleType<?> type) {
        if (!held) {
            return false;
        }
        return hideParticles
                || hideGrowth && type == ParticleTypes.HAPPY_VILLAGER
                || hideExplosions && (type == ParticleTypes.EXPLOSION || type == ParticleTypes.EXPLOSION_EMITTER)
                || hideFireworks && (type == ParticleTypes.FIREWORK || type == ParticleTypes.FLASH);
    }

    /**
     * Hook ({@code ParticleEngine.add}, first): whether {@code particle} is dropped. Catches what the
     * game makes without a type: a breaking block's bits, a firework's burst, an item picked up.
     */
    public static boolean hides(Particle particle) {
        if (!held) {
            return false;
        }
        return hideParticles
                || hideBreaking && particle instanceof TerrainParticle
                || hideFireworks && particle instanceof FireworkParticles.Starter
                || hideItems && particle instanceof ItemPickupParticle;
    }
}
