package incari.client.modules;

import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.math.Axis;
import incari.framework.ModuleSystem;
import incari.framework.event.Events;
import incari.framework.event.HeldItem;
import net.minecraft.client.Minecraft;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.resources.model.cuboid.ItemTransform;
import net.minecraft.tags.ItemTags;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.ShieldItem;
import net.minecraft.world.phys.HitResult;
import org.joml.Vector3fc;

/**
 * The held item event ({@link Events#HELD_ITEM}): how each hand's item is drawn in first person.
 * Low Shield moves a shield down; Short Items draws held items smaller; 1.7 Animations holds flat items as 1.7 did, flips the fishing rod,
 * gives a blocking sword 1.7's pose and swings the arm while an item's in use (the block-hit). Render
 * thread, for each hand each frame; nothing here allocates.
 *
 * <p>1.7's poses are 1.7's own item transform (its {@code ItemRenderer}'s scale, turn and offset
 * for a flat item) applied after undoing the item model's own first-person display transform, so
 * that transform then leaves the item where 1.7 put it. Only for models held as flat and handheld
 * items are (turned 0, -90, 25, wherever they're moved or however scaled: a mace, a resource
 * pack's sword) and rods; a model held otherwise (a spear) keeps today's pose.
 */
public final class ModuleHands {

    /** One object for every hand and frame: the event is hot. */
    private static final HeldItem ITEM = new HeldItem();

    /** Whether this hand took 1.7's blocking pose, which its item's 1.7 pose then finishes. */
    private static boolean blockPosed;

    /** The last main hand's answer, for the tick's block-hit swing. */
    private static boolean mainSwingsWhileUsing;

    /** 25 degrees: the tilt of 1.7's flat items. */
    private static final double TILT = Math.toRadians(25);

    private ModuleHands() {}

    /**
     * Hook ({@code FirstPersonHandsAndItemsRenderer.submitArmWithItem}, before the hand is placed):
     * fires the event for this hand and moves it down as the listeners said.
     */
    public static void begin(PoseStack pose, boolean mainHand, ItemStack stack, boolean using) {
        blockPosed = false;
        ModuleSystem system = IncariModules.current();
        if (system == null || system.events().count(Events.HELD_ITEM) == 0) {
            ITEM.reset(mainHand, false, using);
            if (mainHand) {
                mainSwingsWhileUsing = false;
            }
            return;
        }
        ITEM.reset(mainHand, stack.getItem() instanceof ShieldItem, stack.is(ItemTags.SWORDS), using);
        system.events().fireHeldItem(ITEM);
        if (mainHand) {
            mainSwingsWhileUsing = ITEM.swingWhileUsing();
        }
        float lower = ITEM.lower();
        if (lower != 0) {
            pose.translate(0.0F, -lower, 0.0F);
        }
    }

    /** Hook (an item in use, after the arm is placed): whether the arm swings as it would unused. */
    public static boolean swingWhileUsing() {
        return ITEM.swingWhileUsing();
    }

    /**
     * Hook (a blocking sword): 1.7's blocking pose in place of today's.
     *
     * @return whether it took the pose, so today's is skipped
     */
    public static boolean legacyBlock(PoseStack pose, int invert) {
        if (!ITEM.legacyBlock()) {
            return false;
        }
        // 1.7's block turn, in the frame its item renderer worked in (turned 45 degrees, at 0.4 scale).
        pose.rotateDegrees(Axis.YP, invert * 45.0F);
        pose.scale(0.4F, 0.4F, 0.4F);
        pose.translate(invert * -0.5F, 0.2F, 0.0F);
        pose.rotateDegrees(Axis.YP, invert * 30.0F);
        pose.rotateDegrees(Axis.XP, -80.0F);
        pose.rotateDegrees(Axis.YP, invert * 60.0F);
        pose.scale(2.5F, 2.5F, 2.5F);
        pose.rotateDegrees(Axis.YP, invert * -45.0F);
        blockPosed = true;
        return true;
    }

    /**
     * Hook (just before the item is drawn): 1.7's rod and flat items, if the listeners asked, and
     * the item's size (Short Items), scaled toward the hand.
     */
    public static void beforeItem(PoseStack pose, ItemStack stack, int invert, ItemTransform display) {
        float scale = ITEM.scale();
        if (scale != 1) {
            pose.scale(scale, scale, scale);
        }
        if (ITEM.legacyRod() && stack.is(Items.FISHING_ROD)) {
            pose.rotateDegrees(Axis.YP, invert * 180.0F);
        }
        if ((ITEM.legacyItems() || blockPosed) && isFlat(stack)) {
            // The model's own first-person transform, as the left hand mirrors it.
            boolean left = invert < 0;
            Vector3fc rotation = display.rotation();
            float turnY = left ? -rotation.y() : rotation.y();
            float turnZ = left ? -rotation.z() : rotation.z();
            boolean held = near(rotation.x(), 0) && near(turnZ, 25);
            if (held && near(turnY, -90) && invertible(display.scale())) {
                // Held as flat and handheld items are, wherever the model (a mace, a resource
                // pack's sword) moves or scales it: exactly undone.
                legacyItem(pose, invert);
                undoDisplay(pose, display, left);
            } else if (held && near(turnY, 90)) {
                // Turned the other way, as rods are: undone as before, the rod's flip with it.
                legacyItem(pose, invert);
                undoFlatDisplay(pose, invert);
            }
            // Anything else is held its own way (spears, a pack's 3D models): today's pose stays.
        }
    }

    /** Whether 1.7 drew it as a flat item: not a block, and not one of today's own models. */
    private static boolean isFlat(ItemStack stack) {
        return !(stack.getItem() instanceof BlockItem)
                && !(stack.getItem() instanceof ShieldItem)
                && !stack.is(Items.TRIDENT)
                && !stack.is(Items.CROSSBOW)
                && !stack.is(Items.SPYGLASS);
    }

    /** 1.7's flat item, from the arm: smaller, turned edge-on and tilted, its grip at the hand. */
    private static void legacyItem(PoseStack pose, int invert) {
        pose.scale(0.6F, 0.6F, 0.6F);
        pose.rotateDegrees(Axis.YP, invert * 275.0F);
        pose.rotateDegrees(Axis.ZP, invert * 25.0F);
        pose.translate(
                invert * (float) (-0.2 * Math.sin(TILT) + 0.4375), (float) (-0.2 * Math.cos(TILT) + 0.4375), 0.03125F);
    }

    /**
     * The inverse of the model's first-person display transform ({@code ItemTransform.apply}:
     * moved, turned X then Y then Z, scaled), with the left hand's sprite seen from behind, as
     * 1.7's pose mirrored for that hand wants it.
     */
    private static void undoDisplay(PoseStack pose, ItemTransform display, boolean left) {
        Vector3fc rotation = display.rotation();
        Vector3fc translation = display.translation();
        Vector3fc size = display.scale();
        float sign = left ? -1 : 1;
        if (left) {
            pose.rotateDegrees(Axis.YP, 180.0F);
        }
        pose.scale(1 / size.x(), 1 / size.y(), 1 / size.z());
        pose.rotateDegrees(Axis.ZP, -sign * rotation.z());
        pose.rotateDegrees(Axis.YP, -sign * rotation.y());
        pose.rotateDegrees(Axis.XP, -rotation.x());
        pose.translate(-sign * translation.x(), -translation.y(), -translation.z());
    }

    private static boolean near(float value, float expected) {
        return Math.abs(value - expected) < 0.01F;
    }

    private static boolean invertible(Vector3fc scale) {
        return Math.abs(scale.x()) > 1e-4F && Math.abs(scale.y()) > 1e-4F && Math.abs(scale.z()) > 1e-4F;
    }

    /** The inverse of the first-person display transform vanilla gives flat items. */
    private static void undoFlatDisplay(PoseStack pose, int invert) {
        float scale = 1 / 0.68F;
        pose.scale(scale, scale, scale);
        pose.rotateDegrees(Axis.ZP, invert * -25.0F);
        pose.rotateDegrees(Axis.YP, invert * 90.0F);
        pose.translate(invert * -1.13F / 16, -3.2F / 16, -1.13F / 16);
    }

    /**
     * From each client tick: 1.7's block-hit. With the attack key held while an item's in use and
     * the crosshair on a block, the arm swings, drawn only (nothing is sent: today's servers would
     * take a swing for an attack).
     */
    static void tick(Minecraft minecraft) {
        LocalPlayer player = minecraft.player;
        if (!mainSwingsWhileUsing || player == null || minecraft.gui.screen() != null) {
            return;
        }
        HitResult hit = minecraft.hitResult;
        if (player.isUsingItem()
                && minecraft.options.keyAttack.isDown()
                && hit != null
                && hit.getType() == HitResult.Type.BLOCK) {
            // LivingEntity.swing only sends from a server; on the client it's the animation.
            player.swing(InteractionHand.MAIN_HAND, player.getMainHandItem().getAttackAnimation(), false);
        }
    }
}
