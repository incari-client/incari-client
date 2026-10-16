package incari.client.cosmetics;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.UUID;
import java.util.function.Predicate;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.FontDescription;
import net.minecraft.network.chat.MutableComponent;
import net.minecraft.network.chat.Style;
import net.minecraft.resources.Identifier;

/**
 * Incari's badge beside an Incari player's name tag, their name in the tab list and their name in chat (ADR 0062), so
 * Incari players know each other without asking. The badge is one glyph of Incari's pack's {@code incari:badge} font (Incari's
 * mark at 32 pixels, white so the glyph keeps its own colours), so the game's own name tag and tab list drawing carry it:
 * nothing new is drawn. Shown for players whose account lets it show, while the player's
 * "Show Incari badges" switch is on.
 *
 * <p>Render thread. A badged name is made once per player and name, not each frame; name tags and
 * the tab list keep their own, as the two names differ.
 */
final class Badges {

    private static final Component BADGE = Component.literal("")
            .withStyle(Style.EMPTY
                    .withFont(new FontDescription.Resource(Identifier.fromNamespaceAndPath("incari", "badge")))
                    .withColor(0xFFFFFF));

    private record Badged(Component from, Component to) {}

    private static final Map<UUID, Badged> NAME_TAGS = new HashMap<>();
    private static final Map<UUID, Badged> TAB_LIST = new HashMap<>();

    private Badges() {}

    /** The name tag {@code tag} with the badge before it, for the player {@code id}. */
    static Component badged(UUID id, Component tag) {
        return badged(NAME_TAGS, id, tag);
    }

    /** The tab list's {@code name} with the badge before it, for the player {@code id}. */
    static Component badgedInTabList(UUID id, Component name) {
        return badged(TAB_LIST, id, name);
    }

    /**
     * Chat's {@code message} with the badge before the first mention of each name {@code badged}
     * takes, or {@code message} when it names none. Names are runs of name characters in the
     * message's text; legacy {@code §} codes (1.8 servers' chat) are skipped over and carried past
     * the badge, so a coloured name keeps its colour. Once per chat line, not per frame.
     */
    static Component badgedInChat(Component message, Predicate<String> badged) {
        String text = message.getString();
        List<Integer> at = new ArrayList<>();
        Set<String> seen = new HashSet<>();
        int n = text.length();
        for (int i = 0; i < n; ) {
            char c = text.charAt(i);
            if (c == '§') {
                i += 2;
            } else if (!isNameChar(c)) {
                i++;
            } else {
                int start = i;
                while (i < n && isNameChar(text.charAt(i))) {
                    i++;
                }
                String name = text.substring(start, i);
                if (i - start <= 16 && seen.add(name) && badged.test(name)) {
                    at.add(start);
                }
            }
        }
        if (at.isEmpty()) {
            return message;
        }
        // The message rebuilt piece by piece in the same styles, each piece cut where a badge goes.
        MutableComponent out = Component.empty();
        int[] offset = {0};
        int[] next = {0};
        message.visit(
                (style, piece) -> {
                    int end = offset[0] + piece.length();
                    int from = 0;
                    String carried = "";
                    while (next[0] < at.size() && at.get(next[0]) < end) {
                        int cut = at.get(next[0]++) - offset[0];
                        String before = piece.substring(from, cut);
                        if (!before.isEmpty()) {
                            out.append(Component.literal(carried + before).setStyle(style));
                        }
                        carried += legacyCodes(before);
                        out.append(BADGE).append(" ");
                        from = cut;
                    }
                    out.append(
                            Component.literal(carried + piece.substring(from)).setStyle(style));
                    offset[0] = end;
                    return Optional.empty();
                },
                Style.EMPTY);
        return out;
    }

    private static boolean isNameChar(char c) {
        return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '_';
    }

    /** The {@code §} codes in {@code text}, in order: what a later cut of the same text must repeat. */
    private static String legacyCodes(String text) {
        StringBuilder codes = new StringBuilder();
        for (int i = text.indexOf('§'); i >= 0 && i + 1 < text.length(); i = text.indexOf('§', i + 2)) {
            codes.append(text, i, i + 2);
        }
        return codes.toString();
    }

    private static Component badged(Map<UUID, Badged> made, UUID id, Component name) {
        Badged last = made.get(id);
        if (last != null && last.from().equals(name)) {
            return last.to();
        }
        MutableComponent badged = Component.empty().append(BADGE).append(" ").append(name);
        if (made.size() > 512) {
            made.clear();
        }
        made.put(id, new Badged(name, badged));
        return badged;
    }

    /** Players who left, forgotten (on leaving a world). */
    static void clear() {
        NAME_TAGS.clear();
        TAB_LIST.clear();
    }
}
