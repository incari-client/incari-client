package incari.client;

import incari.client.cosmetics.Cosmetics;
import incari.client.eagler.EaglerVoice;
import incari.client.legacy.LegacyRules;
import incari.client.look.Looks;
import incari.client.modules.IncariModules;
import incari.client.net.ServerConnections;
import incari.client.online.IncariOnline;
import incari.client.profiler.Profiling;
import incari.client.recorder.ScreenRecorder;
import incari.client.render.mesher.ChestBaking;
import incari.client.singleplayer.QuickWorlds;
import net.minecraft.client.Minecraft;

/**
 * What Incari does once a client tick, from one hook in {@code Minecraft.tick} (after the GUI's
 * tick, before key bindings): the legacy behaviour layer follows the server's version, voice
 * chat runs, the profiler overlay's key is read, the benchmark's quick worlds are made and left, and
 * the Incari account's connection runs, and the modules tick (their keybinds, then their listeners). The first tick also puts on the client's
 * look. Game thread.
 */
public final class IncariTicks {

    private IncariTicks() {}

    public static void tick(Minecraft minecraft) {
        Looks.start(minecraft);
        // A join that reached a world: a kick from here on isn't a refusal of its version.
        ServerConnections.tick(minecraft);
        // First: the rest of the tick moves the player by the rules of the server it's on.
        LegacyRules.update(minecraft);
        // Push to talk works with no screen open, as other keys do, and while chat is closed.
        boolean pushToTalk = minecraft.gui.screen() == null && IncariKeys.PUSH_TO_TALK.isDown();
        EaglerVoice.tick(minecraft, pushToTalk);
        Profiling.tick();
        ScreenRecorder.tick(minecraft);
        ChestBaking.tick(minecraft.level);
        QuickWorlds.tick(minecraft);
        // Before the modules: they read who's an Incari player (ADR 0058).
        IncariOnline.tick(minecraft);
        // Cosmetics read the service's emotes and what Incari players wear (ADR 0061).
        Cosmetics.tick(minecraft);
        IncariModules.tick(minecraft);
    }
}
