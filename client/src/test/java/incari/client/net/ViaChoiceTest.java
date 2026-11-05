package incari.client.net;

import static org.assertj.core.api.Assertions.assertThat;

import net.minecraft.client.multiplayer.ServerData;
import net.minecraft.nbt.CompoundTag;
import org.junit.jupiter.api.Test;

/** Via is off unless the player turns it on (ADR 0071), and the disconnection screen says how. */
class ViaChoiceTest {

    private static ServerData server(int choice) {
        ServerData server = new ServerData("Test", "mc.example.com", ServerData.Type.OTHER);
        server.incariVersion = choice;
        return server;
    }

    @Test
    void aNewServerHasViaOff() {
        assertThat(ServerVersions.usesVia(new ServerData("Test", "mc.example.com", ServerData.Type.OTHER)))
                .isFalse();
    }

    @Test
    void aServerSavedByAnEarlierBuildLoadsWithViaOff() {
        CompoundTag saved = new CompoundTag();
        saved.putInt("incariVersion", 47);
        ServerData server = server(ServerVersions.AUTO);
        ServerVersions.read(server, saved);
        assertThat(server.incariVersion).isEqualTo(ServerVersions.OFF);
    }

    @Test
    void theChoiceIsSavedAndReadBack() {
        for (int choice : new int[] {ServerVersions.AUTO, 47}) {
            CompoundTag tag = new CompoundTag();
            ServerVersions.write(server(choice), tag);
            ServerData read = server(ServerVersions.OFF);
            ServerVersions.read(read, tag);
            assertThat(read.incariVersion).isEqualTo(choice);
        }
        CompoundTag off = new CompoundTag();
        ServerVersions.write(server(ServerVersions.OFF), off);
        assertThat(off.isEmpty()).isTrue();
    }

    @Test
    void hintsAtAVersionRefusalWithViaOff() {
        assertThat(ViaHint.applies(server(ServerVersions.OFF), false, true, false))
                .isTrue();
        assertThat(ViaHint.applies(server(ServerVersions.OFF), false, false, true))
                .isTrue();
    }

    @Test
    void noHintWithViaOnOrForAnotherKick() {
        assertThat(ViaHint.applies(server(ServerVersions.AUTO), false, true, false))
                .isFalse();
        assertThat(ViaHint.applies(server(ServerVersions.OFF), false, false, false))
                .isFalse();
        assertThat(ViaHint.applies(null, false, true, true)).isFalse();
    }

    @Test
    void readsTheUsualRefusals() {
        assertThat(VersionRefusals.isAboutVersions("Outdated server! I'm still on 1.8.8"))
                .isTrue();
        assertThat(VersionRefusals.isAboutVersions("Outdated client! Please use 1.8.8"))
                .isTrue();
        assertThat(VersionRefusals.isAboutVersions("multiplayer.disconnect.incompatible 1.20.4"))
                .isTrue();
        assertThat(VersionRefusals.isAboutVersions("You were kicked for flying"))
                .isFalse();
    }

    @Test
    void readsThePagesViaChoice() {
        assertThat(ServerVersions.parseChoice("auto")).isEqualTo(ServerVersions.AUTO);
        assertThat(ServerVersions.parseChoice("Off")).isEqualTo(ServerVersions.OFF);
        assertThat(ServerVersions.parseChoice("1.8.9")).isEqualTo(47);
        assertThat(ServerVersions.parseChoice("banana")).isEqualTo(ServerVersions.UNKNOWN);
    }

    @Test
    void aQuickPlayJoinGetsThePagesChoiceOnce() {
        assertThat(ServerVersions.setQuickPlay("ws://127.0.0.1:8801", "auto")).isTrue();
        ServerData other = new ServerData("Other", "ws://127.0.0.1:9999", ServerData.Type.OTHER);
        ServerVersions.applyQuickPlay(other);
        assertThat(ServerVersions.usesVia(other)).isFalse();
        ServerData quick = new ServerData("Minecraft Server", "ws://127.0.0.1:8801", ServerData.Type.OTHER);
        ServerVersions.applyQuickPlay(quick);
        assertThat(quick.incariVersion).isEqualTo(ServerVersions.AUTO);
        ServerData again = new ServerData("Minecraft Server", "ws://127.0.0.1:8801", ServerData.Type.OTHER);
        ServerVersions.applyQuickPlay(again);
        assertThat(ServerVersions.usesVia(again)).isFalse();
    }

    @Test
    void anEaglerServerFoundToRunAnotherVersionNeedsViaWhileItsOff() {
        ServerData off = server(ServerVersions.OFF);
        off.protocol = ServerVersions.UNKNOWN;
        assertThat(ServerVersions.isCompatible(off, () -> 47)).isFalse();
        assertThat(ServerVersions.isCompatible(off, () -> -1)).isTrue();
        assertThat(ServerVersions.isCompatible(off, ServerVersions::gameProtocol))
                .isTrue();
        ServerData auto = server(ServerVersions.AUTO);
        auto.protocol = ServerVersions.UNKNOWN;
        assertThat(ServerVersions.isCompatible(auto, () -> 47)).isTrue();

        assertThat(EaglerPings.versionLabel(47, false)).startsWith("Needs Via");
        assertThat(EaglerPings.versionLabel(47, true)).startsWith("Eagler ");
        assertThat(EaglerPings.versionLabel(-1, false)).isEqualTo("Eagler");
        assertThat(EaglerPings.versionLabel(ServerVersions.gameProtocol(), false))
                .startsWith("Eagler ");
    }

    @Test
    void noEditHintForAFeaturedServer() {
        assertThat(ViaHint.applies(server(ServerVersions.OFF), true, true, true))
                .isFalse();
    }
}
