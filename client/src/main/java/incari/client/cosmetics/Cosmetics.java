package incari.client.cosmetics;

import incari.client.IncariKeys;
import incari.client.online.IncariOnline;
import incari.client.settings.ClientSettings;
import incari.framework.online.IncariPlayer;
import incari.framework.online.Online;
import incari.online.Cosmetic;
import incari.online.Loadout;
import incari.online.OnlineService;
import incari.online.Wardrobe;
import java.util.HashMap;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import net.minecraft.client.CameraType;
import net.minecraft.client.Minecraft;
import net.minecraft.client.model.player.PlayerModel;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.client.player.AbstractClientPlayer;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.renderer.entity.RenderLayerParent;
import net.minecraft.client.renderer.entity.layers.RenderLayer;
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
import net.minecraft.core.ClientAsset;
import net.minecraft.core.particles.ParticleTypes;
import net.minecraft.network.chat.Component;
import net.minecraft.util.RandomSource;
import net.minecraft.world.entity.Avatar;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.entity.player.PlayerSkin;
import org.jspecify.annotations.Nullable;

/**
 * Cosmetics in the world (ADR 0061): what each player wears and plays, from the player's own
 * wardrobe or, for others, the online service, put on each player's render state for the layers to
 * draw. Others' are drawn only for Incari players the service named, and only while
 * {@link Wardrobe#showOthers} is on.
 *
 * <p>The game calls in from six hooks: the player renderer's layers ({@link #layer}) and its
 * render state ({@link #extract}), the player model's pose ({@link #pose}), the tab list's names
 * ({@link #tabListName}), chat ({@link #chatMessage}), and once a tick ({@link #tick}): emotes start and end, trails are left,
 * moving capes are painted.
 *
 * <p>Render thread (the game's thread in Incari's builds). {@link #extract} runs for every player
 * every frame: a lookup, and nothing made unless what someone wears changed.
 */
public final class Cosmetics {

    /** Trails are left only by players this near. */
    private static final double TRAIL_DISTANCE = 32;

    /** At most this many trail particles a tick, however many players are about. */
    private static final int TRAIL_BUDGET = 12;

    /**
     * An emote playing: which, the player's tick it started on (its pose counts from it), and the
     * client tick it ends on. The end is in client ticks because a respawn or dimension change makes
     * a new player, whose ticks restart at 0.
     */
    private record Play(Cosmetic emote, int startTick, long endsAt) {
        Play(Cosmetic emote, int startTick) {
            this(emote, startTick, ticks + Emotes.duration(emote));
        }
    }

    /** What another player was last seen wearing and playing, and what that came to. */
    private record Seen(List<String> source, @Nullable Play play, Worn worn) {}

    private static final Map<UUID, Seen> OTHERS = new HashMap<>();
    private static final Map<UUID, Play> PLAYS = new HashMap<>();

    /** Skins with a cosmetic cape, by cape and by the skin they were made from. */
    private static final Map<Cosmetic, Map<PlayerSkin, PlayerSkin>> DRESSED = new HashMap<>();

    private static Worn self = Worn.NONE;
    private static Loadout selfLoadout = Loadout.EMPTY;
    private static @Nullable Play selfPlay;

    /** The camera to go back to once the player's own emote ends; null when it wasn't moved. */
    private static @Nullable CameraType cameraBefore;

    private static long ticks;

    private Cosmetics() {}

    // ---------------------------------------------------------------------------------------------
    // The game's hooks
    // ---------------------------------------------------------------------------------------------

    /** {@code AvatarRenderer}'s constructor: the layer that draws hats and back cosmetics. */
    public static RenderLayer<AvatarRenderState, PlayerModel> layer(
            RenderLayerParent<AvatarRenderState, PlayerModel> parent) {
        return new CosmeticsLayer(parent);
    }

    /** {@code AvatarRenderer.extractRenderState}'s end: what this player wears, on their state. */
    public static void extract(Avatar avatar, AvatarRenderState state) {
        if (avatar instanceof AbstractClientPlayer player) {
            Worn worn = wornBy(player);
            if (worn != Worn.NONE) {
                dress(state, worn, player.getUUID());
            }
            // Incari's badge (ADR 0062): other Incari players who show it, while the player wants it.
            if (state.nameTag != null && !(player instanceof LocalPlayer) && ClientSettings.SHOW_BADGES.get()) {
                IncariPlayer incari = IncariOnline.service().player(player.getUUID());
                if (incari != null && incari.badge()) {
                    state.nameTag = Badges.badged(player.getUUID(), state.nameTag);
                }
            }
        }
    }

    /**
     * {@code PlayerTabOverlay}'s list: the player {@code id}'s {@code name} with Incari's badge before
     * it, as their name tag has it (ADR 0062), or {@code name}. The player's own name has it too while
     * they show others they use Incari.
     */
    public static Component tabListName(UUID id, Component name) {
        OnlineService online = IncariOnline.service();
        if (online.status() != Online.Status.SIGNED_IN) {
            return name;
        }
        return showsBadge(online, id) ? Badges.badgedInTabList(id, name) : name;
    }

    /**
     * {@code ChatComponent.addMessage}: {@code message} with Incari's badge before the first mention
     * of each Incari player in it, as the tab list has it, or {@code message}. Players are found by
     * their name in the tab list, since most servers send chat as plain text with no sender.
     */
    public static Component chatMessage(Component message) {
        OnlineService online = IncariOnline.service();
        ClientPacketListener connection = Minecraft.getInstance().getConnection();
        if (online.status() != Online.Status.SIGNED_IN || connection == null) {
            return message;
        }
        return Badges.badgedInChat(message, name -> {
            PlayerInfo info = connection.getPlayerInfo(name);
            return info != null && showsBadge(online, info.getProfile().id());
        });
    }

    /** Whether the player {@code id}'s badge shows: theirs if they let it, the player's own if they do. */
    private static boolean showsBadge(OnlineService online, UUID id) {
        LocalPlayer me = Minecraft.getInstance().player;
        if (me != null && me.getUUID().equals(id)) {
            return online.showsBadge();
        }
        IncariPlayer incari = online.player(id);
        return incari != null && incari.badge() && ClientSettings.SHOW_BADGES.get();
    }

    /** {@code PlayerModel.setupAnim}'s end: an emote's pose over the game's. */
    public static void pose(PlayerModel model, AvatarRenderState state) {
        Emotes.pose(model, state);
    }

    /** Whether the player wears any cosmetic: the title screen's player then shows them. */
    public static boolean wearsAny() {
        return !IncariOnline.service().wardrobe().worn().isEmpty();
    }

    /** What the player wears. */
    public static Loadout ownLoadout() {
        return IncariOnline.service().wardrobe().worn();
    }

    /** The player's {@code name} in their name tag cosmetic, or plain. */
    public static Component ownName(String name) {
        Cosmetic nametag = ownLoadout().in(Cosmetic.Slot.NAMETAG);
        return nametag == null ? Component.literal(name) : NameTags.styled(name, nametag);
    }

    /** The emote {@code state}'s player is playing, or null (the smoke tests' check). */
    public static @Nullable Cosmetic emoteOf(AvatarRenderState state) {
        return state.incariWorn.emote;
    }

    /** What {@code state} carries. */
    static Worn worn(AvatarRenderState state) {
        return state.incariWorn;
    }

    /**
     * Puts {@code worn} on {@code state}: for the layers, the cape as the skin's own, and the name tag
     * styled. The wardrobe's preview dresses its state the same way.
     */
    static void dress(AvatarRenderState state, Worn worn, UUID id) {
        state.incariWorn = worn;
        Cosmetic cape = worn.cape;
        if (cape != null) {
            state.skin = withCape(state.skin, cape);
        }
        Cosmetic back = worn.back;
        if (back != null && back.id().equals("back.backpack")) {
            // A backpack and a cape would go through each other.
            state.showCape = false;
        }
        Cosmetic nametag = worn.nametag;
        if (nametag != null && state.nameTag != null) {
            state.nameTag = NameTags.style(id, state.nameTag, nametag);
        }
    }

    private static PlayerSkin withCape(PlayerSkin skin, Cosmetic cape) {
        ClientAsset.Texture texture = CosmeticTextures.cape(cape);
        Map<PlayerSkin, PlayerSkin> bySkin = DRESSED.computeIfAbsent(cape, key -> new HashMap<>());
        PlayerSkin dressed = bySkin.get(skin);
        if (dressed == null) {
            if (bySkin.size() > 256) {
                bySkin.clear();
            }
            // The elytra matches the cape, as it does with the game's capes.
            dressed = new PlayerSkin(skin.body(), texture, texture, skin.model(), skin.secure());
            bySkin.put(skin, dressed);
        }
        return dressed;
    }

    // ---------------------------------------------------------------------------------------------
    // Who wears what
    // ---------------------------------------------------------------------------------------------

    @SuppressWarnings("ReferenceEquality") // the same loadout, list and play objects, not equal ones
    private static Worn wornBy(AbstractClientPlayer player) {
        UUID id = player.getUUID();
        OnlineService online = IncariOnline.service();
        Wardrobe wardrobe = online.wardrobe();
        Play play = PLAYS.get(id);
        if (player instanceof LocalPlayer) {
            Loadout loadout = wardrobe.worn();
            if (loadout != selfLoadout || play != selfPlay) {
                selfLoadout = loadout;
                selfPlay = play;
                self = loadout.isEmpty() && play == null
                        ? Worn.NONE
                        : new Worn(loadout, play == null ? null : play.emote(), play == null ? 0 : play.startTick());
            }
            return self;
        }
        if (!wardrobe.showOthers()) {
            return Worn.NONE;
        }
        IncariPlayer incari = online.player(id);
        if (incari == null && play == null) {
            return Worn.NONE;
        }
        List<String> source = incari == null ? List.of() : incari.cosmetics();
        Seen seen = OTHERS.get(id);
        if (seen == null || seen.source() != source || seen.play() != play) {
            Loadout loadout = Loadout.of(source);
            Worn worn = loadout.isEmpty() && play == null
                    ? Worn.NONE
                    : new Worn(loadout, play == null ? null : play.emote(), play == null ? 0 : play.startTick());
            seen = new Seen(source, play, worn);
            OTHERS.put(id, seen);
        }
        return seen.worn();
    }

    // ---------------------------------------------------------------------------------------------
    // Emotes
    // ---------------------------------------------------------------------------------------------

    /**
     * The player plays {@code emote} (the emote wheel): the camera turns to face them while it plays,
     * and Incari players on the server see it too. False if it can't play now (one every two
     * seconds, and only in a world).
     */
    public static boolean playEmote(Cosmetic emote) {
        Minecraft minecraft = Minecraft.getInstance();
        LocalPlayer player = minecraft.player;
        if (player == null || !IncariOnline.service().playEmote(emote)) {
            return false;
        }
        PLAYS.put(player.getUUID(), new Play(emote, player.tickCount));
        if (minecraft.options.getCameraType() == CameraType.FIRST_PERSON) {
            cameraBefore = CameraType.FIRST_PERSON;
            minecraft.options.setCameraType(CameraType.THIRD_PERSON_FRONT);
        }
        return true;
    }

    private static void emotesEnded(Minecraft minecraft) {
        CameraType before = cameraBefore;
        cameraBefore = null;
        // Unless the player moved the camera themselves meanwhile.
        if (before != null && minecraft.options.getCameraType() == CameraType.THIRD_PERSON_FRONT) {
            minecraft.options.setCameraType(before);
        }
    }

    // ---------------------------------------------------------------------------------------------
    // Each tick
    // ---------------------------------------------------------------------------------------------

    /** From {@code IncariTicks}, after the online service's tick. */
    public static void tick(Minecraft minecraft) {
        ticks++;
        CosmeticTextures.tick(ticks);
        OnlineService online = IncariOnline.service();
        List<OnlineService.PlayedEmote> played = online.takeEmotes();
        ClientLevel level = minecraft.level;
        if (level == null) {
            if (!PLAYS.isEmpty() || !OTHERS.isEmpty()) {
                PLAYS.clear();
                OTHERS.clear();
                NameTags.clear();
                Badges.clear();
            }
            // Left the world mid-emote: the camera goes back now, not with the next world.
            emotesEnded(minecraft);
            return;
        }
        // The wheel opens with its key over the game, not over another screen.
        while (IncariKeys.EMOTE_WHEEL.consumeClick()) {
            if (minecraft.gui.screen() == null && minecraft.player != null) {
                minecraft.gui.setScreen(new EmoteWheelScreen());
            }
        }
        boolean showOthers = online.wardrobe().showOthers();
        for (OnlineService.PlayedEmote each : played) {
            Player player = level.getPlayerByUUID(each.player());
            if (player != null && showOthers) {
                PLAYS.put(each.player(), new Play(each.emote(), player.tickCount));
            }
        }
        endEmotes(minecraft, level, showOthers);
        trails(minecraft, level);
        if (ticks % 200 == 0) {
            OTHERS.keySet().removeIf(id -> level.getPlayerByUUID(id) == null);
        }
    }

    private static void endEmotes(Minecraft minecraft, ClientLevel level, boolean showOthers) {
        LocalPlayer me = minecraft.player;
        boolean mineEnded = false;
        for (Iterator<Map.Entry<UUID, Play>> each = PLAYS.entrySet().iterator(); each.hasNext(); ) {
            Map.Entry<UUID, Play> entry = each.next();
            Player player = level.getPlayerByUUID(entry.getKey());
            Play play = entry.getValue();
            boolean mine = me != null && entry.getKey().equals(me.getUUID());
            boolean over = player == null
                    || ticks >= play.endsAt()
                    || (!mine && !showOthers)
                    // Moving cuts an emote short: the player's own by their keys, others' by their legs.
                    || ((me != null && mine) ? moving(me) : player.walkAnimation.speed() > 0.35F);
            if (over) {
                each.remove();
                mineEnded |= mine;
            }
        }
        if (mineEnded || (cameraBefore != null && (me == null || !PLAYS.containsKey(me.getUUID())))) {
            emotesEnded(minecraft);
        }
    }

    private static boolean moving(LocalPlayer player) {
        return player.input.getMoveVector().lengthSquared() > 0
                || player.input.keyPresses.jump()
                || player.isSwinging();
    }

    private static void trails(Minecraft minecraft, ClientLevel level) {
        if ((ticks & 1) != 0) {
            return;
        }
        LocalPlayer me = minecraft.player;
        if (me == null) {
            return;
        }
        int left = TRAIL_BUDGET;
        RandomSource random = level.getRandom();
        for (AbstractClientPlayer player : level.players()) {
            if (left <= 0) {
                break;
            }
            Cosmetic trail = wornBy(player).trail;
            if (trail == null || player.isInvisible() || player.isSpectator()) {
                continue;
            }
            if (player.distanceToSqr(me) > TRAIL_DISTANCE * TRAIL_DISTANCE) {
                continue;
            }
            double dx = player.getX() - player.xo;
            double dy = player.getY() - player.yo;
            double dz = player.getZ() - player.zo;
            if (dx * dx + dy * dy + dz * dz < 0.0004) {
                continue;
            }
            // Hearts are big: fewer of them.
            if (trail.id().equals("trail.hearts") && (ticks / 2 + player.getId()) % 3 != 0) {
                continue;
            }
            double x = player.getX() + (random.nextDouble() - 0.5) * 0.5;
            double y = player.getY() + 0.1 + random.nextDouble() * 0.2;
            double z = player.getZ() + (random.nextDouble() - 0.5) * 0.5;
            switch (trail.id()) {
                case "trail.snow" -> level.addParticle(ParticleTypes.SNOWFLAKE, x, y, z, 0, 0.02, 0);
                case "trail.hearts" -> level.addParticle(ParticleTypes.HEART, x, y + 0.3, z, 0, 0, 0);
                case "trail.sparkles" ->
                    level.addParticle(
                            ParticleTypes.END_ROD,
                            x,
                            y + 0.2,
                            z,
                            (random.nextDouble() - 0.5) * 0.02,
                            0.02,
                            (random.nextDouble() - 0.5) * 0.02);
                // A note's color is its first speed (a pitch, 0 to 1).
                case "trail.notes" -> level.addParticle(ParticleTypes.NOTE, x, y + 0.4, z, random.nextDouble(), 0, 0);
                default -> {}
            }
            left--;
        }
    }
}
