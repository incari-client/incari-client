package incari.client.net;

import static org.assertj.core.api.Assertions.assertThat;

import incari.client.testing.TestPlatform;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.SharedConstants;
import net.minecraft.client.multiplayer.ServerData;
import net.minecraft.nbt.ListTag;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;

class FeaturedServersTest {

    @BeforeAll
    static void gameVersion() {
        // A ServerData asks for the game's version, set once per run: not left to whichever test ran first.
        TestPlatform.install();
        SharedConstants.tryDetectVersion();
    }

    @Test
    void readsTheBuiltInList() {
        assertThat(FeaturedServers.parse("Carrotcraft|wss://eagler.carrot-craft.org"))
                .containsExactly(new FeaturedServers.Entry("Carrotcraft", "wss://eagler.carrot-craft.org"));
        assertThat(FeaturedServers.parse(" A | a.example ;B|b.example;"))
                .containsExactly(
                        new FeaturedServers.Entry("A", "a.example"), new FeaturedServers.Entry("B", "b.example"));
    }

    private static final List<FeaturedServers.Entry> CARROT =
            List.of(new FeaturedServers.Entry("Carrotcraft", "wss://eagler.carrot-craft.org"));

    @Test
    void anotherListLoadingKeepsTheShownListsEntryFeatured() {
        // The multiplayer screen's list, then the one a join loads to save the server it joined.
        List<ServerData> shown = new ArrayList<>();
        FeaturedServers.addTo(shown, CARROT);
        List<ServerData> joined = new ArrayList<>();
        FeaturedServers.addTo(joined, CARROT);

        assertThat(FeaturedServers.is(shown.getFirst())).isTrue();
        assertThat(FeaturedServers.is(joined.getFirst())).isTrue();
    }

    @Test
    void aSavedCopyIsToldApartFromThePlayersOwnEntry() {
        assertThat(FeaturedServers.isSavedCopy(
                        new ServerData("Carrotcraft", "WSS://eagler.carrot-craft.org", ServerData.Type.OTHER), CARROT))
                .isTrue();
        assertThat(FeaturedServers.isSavedCopy(
                        new ServerData("My carrots", "wss://eagler.carrot-craft.org", ServerData.Type.OTHER), CARROT))
                .isFalse();
        assertThat(FeaturedServers.is(
                        new ServerData("Carrotcraft", "wss://eagler.carrot-craft.org", ServerData.Type.OTHER)))
                .isFalse();
    }

    @Test
    void thePlayersOwnEntryWithAFeaturedNameIsKept() {
        ServerData own = new ServerData("Carrotcraft", "wss://eagler.carrot-craft.org", ServerData.Type.OTHER);
        // Written by an older build: a copy of the featured entry, left out.
        assertThat(FeaturedServers.isSavedCopy(own.write(), own, CARROT)).isTrue();
        // Written by this one: the player's, kept.
        ListTag saved = new ListTag();
        saved.add(own.write());
        FeaturedServers.markOwn(saved);
        assertThat(FeaturedServers.isSavedCopy(saved.getCompoundOrEmpty(0), own, CARROT))
                .isFalse();
    }

    @Test
    void leavesOutMalformedEntries() {
        assertThat(FeaturedServers.parse("")).isEmpty();
        assertThat(FeaturedServers.parse("no-bar;|address;name|")).isEmpty();
    }
}
