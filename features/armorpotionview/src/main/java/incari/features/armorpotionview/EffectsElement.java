package incari.features.armorpotionview;

import incari.features.armorpotionview.ArmorPotionView.Side;
import incari.framework.game.EffectCategory;
import incari.framework.game.EffectView;
import incari.framework.game.PlayerView;
import incari.framework.hud.Anchor;
import incari.framework.hud.HudCanvas;
import incari.framework.hud.HudElement;
import incari.framework.hud.Tone;
import java.util.Arrays;
import org.jspecify.annotations.Nullable;

/**
 * The player's active effects, one row each in the game's order: the effect's icon, its name and
 * level, and the time it has left (amber in its last ten seconds).
 */
final class EffectsElement extends HudElement {

    private static final int ICON = 18;
    private static final int TEXT_GAP = 3;
    private static final int ROW_GAP = 2;
    /** Ticks left from which the time turns amber: ten seconds. */
    private static final int ENDING = 200;
    /** Rows with their text kept; more effects than this make their text each draw. */
    private static final int CACHED = 32;

    private static final String[] NUMERALS = {"", " II", " III", " IV", " V", " VI", " VII", " VIII", " IX", " X"};

    private final ArmorPotionView module;

    // Each row's text, made again only when what it says changes.
    private final @Nullable String[] cachedNameOf = new String[CACHED];
    private final int[] cachedAmplifier = new int[CACHED];
    private final @Nullable String[] cachedName = new String[CACHED];
    private final int[] cachedSeconds = new int[CACHED];
    private final @Nullable String[] cachedTime = new String[CACHED];

    EffectsElement(ArmorPotionView module) {
        // Top left, below Toggle Sprint's line and the FPS, CPS and reach readouts, so a long list
        // grows away from them.
        super("effects", "Effects", Anchor.TOP_LEFT, 2, 62);
        this.module = module;
        Arrays.fill(cachedSeconds, Integer.MIN_VALUE);
    }

    @Override
    public boolean isVisible() {
        PlayerView player = module.player();
        return module.showEffects.get() && player != null && player.effectCount() > 0;
    }

    @Override
    public boolean isPlaceable() {
        return module.showEffects.get();
    }

    @Override
    public int width(HudCanvas canvas) {
        PlayerView player = player(canvas);
        int count = player.effectCount();
        int text = 0;
        boolean names = module.effectNames.get();
        for (int i = 0; i < count; i++) {
            EffectView effect = player.effect(i);
            if (names) {
                text = Math.max(text, canvas.textWidth(name(i, effect)));
            }
            text = Math.max(text, canvas.textWidth(time(i, effect)));
        }
        return count == 0 ? 0 : ICON + TEXT_GAP + text;
    }

    @Override
    public int height(HudCanvas canvas) {
        int count = player(canvas).effectCount();
        return count == 0 ? 0 : count * (ICON + ROW_GAP) - ROW_GAP;
    }

    @Override
    public void draw(HudCanvas canvas) {
        PlayerView player = player(canvas);
        int count = player.effectCount();
        boolean right = module.side.get() == Side.RIGHT;
        boolean names = module.effectNames.get();
        int width = right ? width(canvas) : 0;
        int line = canvas.lineHeight();
        for (int i = 0; i < count; i++) {
            EffectView effect = player.effect(i);
            int y = i * (ICON + ROW_GAP);
            canvas.effect(effect, right ? width - ICON : 0, y);
            String time = time(i, effect);
            Tone timeTone = !effect.isInfinite() && effect.duration() < ENDING ? Tone.WARNING : Tone.MUTED;
            if (names) {
                String name = name(i, effect);
                text(canvas, name, right, width, y, nameTone(effect));
                text(canvas, time, right, width, y + line, timeTone);
            } else {
                text(canvas, time, right, width, y + (ICON - line) / 2 + 1, timeTone);
            }
        }
    }

    private static void text(HudCanvas canvas, String text, boolean right, int width, int y, Tone tone) {
        int x = right ? width - ICON - TEXT_GAP - canvas.textWidth(text) : ICON + TEXT_GAP;
        canvas.text(text, x, y, tone);
    }

    private Tone nameTone(EffectView effect) {
        if (!module.kindColors.get()) {
            return Tone.TEXT;
        }
        return effect.category() == EffectCategory.BENEFICIAL
                ? Tone.POSITIVE
                : effect.category() == EffectCategory.HARMFUL ? Tone.NEGATIVE : Tone.TEXT;
    }

    /** The real player, or in the HUD editor the sample when the real one has no effects. */
    private PlayerView player(HudCanvas canvas) {
        PlayerView player = module.player();
        if (player != null && player.effectCount() > 0) {
            return player;
        }
        return canvas.editing() ? module.samplePlayer() : PlayerView.EMPTY;
    }

    /** "Speed II", made again only when the effect in this row changes. */
    @SuppressWarnings("ReferenceEquality") // The client hands out the same name string each time.
    private String name(int row, EffectView effect) {
        String name = effect.name();
        int amplifier = effect.amplifier();
        if (row < CACHED && cachedNameOf[row] == name && cachedAmplifier[row] == amplifier) {
            String cached = cachedName[row];
            if (cached != null) {
                return cached;
            }
        }
        String text = name + level(amplifier);
        if (row < CACHED) {
            cachedNameOf[row] = name;
            cachedAmplifier[row] = amplifier;
            cachedName[row] = text;
        }
        return text;
    }

    /** "1:23", made again only when the second changes. */
    private String time(int row, EffectView effect) {
        int seconds = effect.isInfinite() ? -1 : effect.duration() / 20;
        if (row < CACHED && cachedSeconds[row] == seconds) {
            String cached = cachedTime[row];
            if (cached != null) {
                return cached;
            }
        }
        String text = seconds < 0 ? "∞" : clock(seconds);
        if (row < CACHED) {
            cachedSeconds[row] = seconds;
            cachedTime[row] = text;
        }
        return text;
    }

    /** " II" for amplifier 1; nothing for level I; numbers past X. */
    static String level(int amplifier) {
        if (amplifier >= 0 && amplifier < NUMERALS.length) {
            return NUMERALS[amplifier];
        }
        return " " + (amplifier + 1);
    }

    /** "0:08", "1:23", "1:02:03". */
    static String clock(int seconds) {
        int hours = seconds / 3600;
        int minutes = seconds / 60 % 60;
        int secs = seconds % 60;
        String tail = (secs < 10 ? ":0" : ":") + secs;
        if (hours > 0) {
            return hours + (minutes < 10 ? ":0" : ":") + minutes + tail;
        }
        return minutes + tail;
    }
}
