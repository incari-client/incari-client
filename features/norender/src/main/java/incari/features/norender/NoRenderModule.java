package incari.features.norender;

import incari.framework.Category;
import incari.framework.Module;
import incari.framework.game.NoRender;
import incari.framework.game.NoRender.Items;
import incari.framework.setting.BooleanSetting;
import incari.framework.setting.ChoiceSetting;
import incari.framework.setting.Setting;

/**
 * No Render: leaves what costs frames out of the world drawn, on this client only. Items on the
 * ground hidden or lying still, a stack as one item, and XP orbs, falling blocks, fireworks and the
 * particles the player picks hidden. What's hidden is still there, to pick up or hit. The choice is
 * handed to the game once per change, not per tick: the client checks it as it draws.
 */
public final class NoRenderModule extends Module {

    final ChoiceSetting<Items> items = settings()
            .choice("items", "Items on the ground", Items.values(), Items.HIDDEN, Items::label)
            .describedAs("Still: no bobbing or spinning. Hidden: not drawn, but still picked up.");
    final BooleanSetting oneCopy = settings()
            .bool("one-copy", "One item per stack", true)
            .describedAs("Draws a stack on the ground as one item, not a pile of up to five.")
            .visibleWhen(() -> items.get() != Items.HIDDEN);
    final BooleanSetting orbs =
            settings().bool("orbs", "Hide XP orbs", false).describedAs("Experience orbs aren't drawn.");
    final BooleanSetting fallingBlocks = settings()
            .bool("falling-blocks", "Hide falling blocks", false)
            .describedAs("Falling sand, gravel and anvils aren't drawn until they land.");
    final BooleanSetting fireworks = settings()
            .bool("fireworks", "Hide fireworks", false)
            .describedAs("Firework rockets and their bursts aren't drawn.");
    final BooleanSetting growth = settings()
            .bool("growth", "Hide bone meal sparkles", true)
            .describedAs("The green sparkles of bone meal and villagers aren't drawn.");
    final BooleanSetting breaking = settings()
            .bool("breaking", "Hide block bits", false)
            .describedAs("The bits of blocks breaking, and the dust of running and landing.");
    final BooleanSetting explosions = settings()
            .bool("explosions", "Hide explosions", false)
            .describedAs("Explosions' smoke clouds aren't drawn.");
    final BooleanSetting allParticles = settings()
            .bool("all-particles", "Hide all particles", false)
            .describedAs("No particles at all: more frames, but no rain splashes, hits or potions.");

    /** No Render, off until the player turns it on. */
    public NoRenderModule() {
        super("norender", "No Render", "Hides items on the ground and other things that cost frames.", Category.VISUAL);
        for (Setting<?> setting : settings()) {
            setting.onChange(this::changed);
        }
    }

    private void changed() {
        if (isEnabled()) {
            system().game().overrideNoRender(choice());
        }
    }

    @Override
    protected void onEnable() {
        system().game().overrideNoRender(choice());
    }

    @Override
    protected void onDisable() {
        system().game().overrideNoRender(null);
    }

    /** What the settings say, as the game takes it. */
    NoRender choice() {
        return new NoRender(
                items.get(),
                oneCopy.get(),
                orbs.get(),
                fallingBlocks.get(),
                fireworks.get(),
                growth.get(),
                breaking.get(),
                explosions.get(),
                allParticles.get());
    }
}
