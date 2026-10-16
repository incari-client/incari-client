package incari.client.cosmetics;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;

import java.util.Set;
import net.minecraft.ChatFormatting;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.Style;
import org.junit.jupiter.api.Test;

class BadgesTest {

    private static final Set<String> INCARI = Set.of("Steve", "Alex");

    @Test
    void chatWithNoIncariPlayerIsUnchanged() {
        Component message = Component.literal("<Notch> hello Steven");
        assertSame(message, Badges.badgedInChat(message, INCARI::contains));
    }

    @Test
    void badgeGoesBeforeEachIncariPlayersFirstMention() {
        Component message = Component.literal("<Steve> hi Alex, Steve here");
        assertEquals(
                "< Steve> hi  Alex, Steve here",
                Badges.badgedInChat(message, INCARI::contains).getString());
    }

    @Test
    void piecesKeepTheirStyleAndLegacyColours() {
        Style green = Style.EMPTY.withColor(ChatFormatting.GREEN);
        Component message = Component.literal("§7[§aVIP§7] Steve: hi").setStyle(green);
        Component badged = Badges.badgedInChat(message, INCARI::contains);
        assertEquals("§7[§aVIP§7]  §7§a§7Steve: hi", badged.getString());
        // The cut after the badge repeats the codes before it, so "Steve" stays grey (§7).
        StringBuilder pieces = new StringBuilder();
        badged.visit(
                (style, text) -> {
                    if (text.contains("Steve")) {
                        assertEquals(green.getColor(), style.getColor());
                        pieces.append(text);
                    }
                    return java.util.Optional.empty();
                },
                Style.EMPTY);
        assertEquals("§7§a§7Steve: hi", pieces.toString());
    }
}
