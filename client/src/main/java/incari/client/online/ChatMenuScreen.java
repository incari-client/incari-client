package incari.client.online;

import incari.client.ui.Icons;
import incari.client.ui.RoundRects;
import incari.client.ui.ScreenPanels;
import incari.client.ui.Theme;
import incari.client.ui.Ui;
import incari.framework.online.IncariPlayer;
import incari.framework.online.Online.Status;
import incari.online.Credentials;
import incari.online.Failure;
import incari.online.OnlineService;
import incari.online.Result;
import incari.online.Social;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.TimeZone;
import java.util.function.Consumer;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractButton;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.client.gui.components.Tooltip;
import net.minecraft.client.gui.narration.NarrationElementOutput;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.InputWithModifiers;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.Component;
import net.minecraft.util.ARGB;
import net.minecraft.util.FormattedCharSequence;
import net.minecraft.world.entity.player.PlayerSkin;
import org.jspecify.annotations.Nullable;

/**
 * The chat menu (ADR 0059), laid out like Discord, which most players know: people down the left,
 * the conversation on the right. Friends are listed first, with a presence dot; other Incari players
 * on this server show their skin's face and a server tag. Friend requests sit at the top with
 * Accept and Decline on the row. The header has Add Friend, Block and Report; the gear opens chat
 * settings (who can message the player, and send them friend requests).
 *
 * <p>Opened by the Friends key (O) or the button under the chat box, over the game or a menu, as
 * the module menu is. Game thread; it reads {@link OnlineService} and redraws as it changes.
 */
public final class ChatMenuScreen extends Screen {

    private static final int HEADER = 34;
    private static final int ROW = 22;
    private static final int SECTION = 16;
    private static final int AVATAR = 16;
    private static final int PAD = 8;
    private static final int SETTING_CONTROL_WIDTH = 150;
    private static final int STRIP = 32;
    private static final int INPUT = 20;
    /** The chat header keeps at least this for the name before its actions lose their labels. */
    private static final int MIN_NAME = 64;
    /** Messages within this long of the last by the same person join its group, as in Discord. */
    private static final long GROUP_MILLIS = 5 * 60_000;

    private static final int SIDEBAR_SHADE = 0x24000000;
    private static final int LINE = 0x1AFFFFFF;

    /** What the right side shows. */
    private enum View {
        /** Nobody chosen yet: a welcome. */
        HOME,
        CHAT,
        ADD_FRIEND,
        SETTINGS
    }

    /** A line in the people list. */
    private record Row(
            boolean header,
            String label,
            String detail,
            Social.@Nullable Peer peer,
            Social.@Nullable Request request,
            boolean friend,
            boolean online,
            int unread) {}

    private final @Nullable Screen parent;
    private final OnlineService online = IncariOnline.service();
    private final Social social = online.social();

    private View view = View.HOME;
    private Social.@Nullable Peer selected;
    /** What the player was typing to each person, kept while the menu is laid out again. */
    private final Map<String, String> drafts = new HashMap<>();
    /** What the player was typing in Add Friend, kept the same way. */
    private String friendDraft = "";

    private final List<Row> rows = new ArrayList<>();
    private long rowsRevision = -1;
    private double listScroll;
    private double chatScroll;

    private String status = "";
    private int statusColor = Theme.MUTED;
    private long statusUntil;
    /** The header's Block, armed by a first click: a second within a few seconds blocks. */
    private long blockArmedUntil;

    private @Nullable EditBox input;
    /** What {@link #input} was made for: its view, and in a conversation, whose it is. */
    private @Nullable View inputView;

    private Social.@Nullable Peer inputPeer;
    private @Nullable Object shownShape;

    // Layout.
    private int px;
    private int py;
    private int pw;
    private int ph;
    private int side;
    private int mainX;
    private int mainWidth;
    private int listTop;
    private int listBottom;
    private int chatTop;
    private int chatBottom;
    /** The left edge of the chat header's action buttons: the name and its line stop short of it. */
    private int actionsLeft;

    // The conversation, wrapped: rebuilt when it or the width changes.
    private final List<ChatLine> lines = new ArrayList<>();
    private long linesRevision = -1;
    private int linesWidth = -1;
    private Social.@Nullable Peer linesPeer;
    private int linesHeight;

    /** One drawn line of the conversation: a group's header (avatar, name, time) or a line of text. */
    private record ChatLine(
            boolean head, String name, boolean mine, String time, FormattedCharSequence text, int color, int y) {}

    public ChatMenuScreen(@Nullable Screen parent) {
        super(Component.literal("Incari Chat"));
        this.parent = parent;
    }

    /** Opened on the conversation with {@code peer} (the smoke test's pictures, a toast's click later). */
    public ChatMenuScreen(@Nullable Screen parent, Social.Peer peer) {
        this(parent);
        select(peer);
    }

    // ---------------------------------------------------------------------------------------------
    // Layout
    // ---------------------------------------------------------------------------------------------

    @Override
    protected void init() {
        keepDraft();
        pw = Math.min(width - 20, 620);
        ph = Math.min(height - 16, 352);
        px = (width - pw) / 2;
        py = (height - ph) / 2;
        side = Math.clamp(Math.round(pw * 0.27F), 118, 150);
        mainX = px + side;
        mainWidth = pw - side;
        listTop = py + PAD + Theme.CONTROL_HEIGHT + 6;
        listBottom = py + ph - STRIP;
        chatTop = py + HEADER;
        chatBottom = py + ph - PAD - INPUT - 6;
        ScreenPanels.clear(this);
        ScreenPanels.add(this, px, py, pw, ph);
        shownShape = shape();
        input = null;
        inputView = null;
        inputPeer = null;

        // Sidebar: Add Friend on top, the player's strip with its gear at the bottom.
        if (online.signedIn()) {
            Button add = Ui.menuButton(
                    Component.literal("Add Friend"), Icons.PERSON_ADD, side - 2 * PAD, () -> show(View.ADD_FRIEND));
            add.setPosition(px + PAD, py + PAD);
            addRenderableWidget(add);
            IconButton gear = new IconButton(Icons.GEAR, "Chat settings", () -> show(View.SETTINGS));
            gear.setPosition(px + side - PAD - IconButton.SIZE, py + ph - STRIP + (STRIP - IconButton.SIZE) / 2);
            addRenderableWidget(gear);
        }
        IconButton close = new IconButton(Icons.CLOSE, "Close", this::onClose);
        close.setPosition(px + pw - PAD - IconButton.SIZE, py + (HEADER - IconButton.SIZE) / 2);
        addRenderableWidget(close);

        if (!online.signedIn()) {
            initSignedOut();
            return;
        }
        switch (view) {
            case CHAT -> initChat();
            case ADD_FRIEND -> initAddFriend();
            case SETTINGS -> initSettings();
            case HOME -> {}
        }
    }

    /** What decides which widgets there are: they're made again when it changes. */
    private Object shape() {
        Social.Peer peer = selected;
        String relation = "";
        if (peer != null) {
            relation = relation(peer).name() + "/" + (social.sentTo(peer) != null);
        }
        return List.of(
                online.status(),
                view,
                peer == null ? "" : peer.key(),
                relation,
                Objects.requireNonNullElse(social.available(), "?"),
                social.messagesFrom(),
                social.friendRequests(),
                online.sharesServer(),
                blockArmedUntil > 0);
    }

    private void initSignedOut() {
        int buttonWidth = 120;
        int x = mainX + (mainWidth - buttonWidth) / 2;
        int y = signedOutTop() + 46 + signedOutLines().size() * 10 + 6;
        if (online.status() == Status.SIGNED_OUT) {
            Button signIn = Ui.button(
                    Component.literal("Sign In"), buttonWidth, () -> minecraft.gui.setScreen(new AccountScreen(this)));
            signIn.setPosition(x, y);
            addRenderableWidget(signIn);
        } else if (online.status() == Status.CONNECTING) {
            Button retry = Ui.button(Component.literal("Try Again"), buttonWidth, online::retry);
            retry.setPosition(x, y);
            addRenderableWidget(retry);
        }
    }

    private void initChat() {
        Social.Peer peer = Objects.requireNonNull(selected);
        // Back from Report, Blocked Players or the account screen (removed() let it go): the
        // conversation shown is the one whose messages arrive read again.
        online.viewing(peer);
        // The header's actions, right to left from the close button: Report, Block, then the friendship's.
        List<Action> actions = new ArrayList<>();
        actions.add(new Action(Icons.FLAG, "Report", this::report, false));
        boolean armed = blockArmedUntil > 0;
        actions.add(new Action(Icons.BLOCK, armed ? "Click to Block" : "Block", this::block, armed));
        switch (relation(peer)) {
            case FRIEND -> actions.add(new Action(Icons.CLOSE, "Unfriend", this::unfriend, false));
            case PLAYER -> {
                // Only a sent request: a received one is answered on its row, since its sender's
                // Incari username can't be tied to a player here. Add Friend to someone who already
                // asked makes them friends at once.
                Social.Request sent = social.sentTo(peer);
                if (sent == null) {
                    actions.add(new Action(Icons.PERSON_ADD, "Add Friend", this::befriendSelected, false));
                } else {
                    actions.add(new Action(Icons.CLOSE, "Cancel Request", () -> decline(sent), false));
                }
            }
            case GONE -> {}
        }
        int right = px + pw - PAD - IconButton.SIZE - 6;
        int labelled = 0;
        for (Action each : actions) {
            labelled += actionWidth(each.label()) + 4;
        }
        // Labels while the name has room; on a narrow panel (small window, large GUI scale) icons
        // with tooltips, so nothing spills over the people list.
        boolean iconsOnly = right - labelled < mainX + PAD + 2 + AVATAR + 7 + MIN_NAME;
        for (Action each : actions) {
            right = iconsOnly ? iconAction(right, each) : action(right, each);
        }
        actionsLeft = right;

        EditBox box = Ui.textField(
                font, Component.literal("Message"), mainWidth - 2 * PAD - IconButton.SIZE - 6, OnlineService.MAX_TEXT);
        box.setPosition(mainX + PAD, py + ph - PAD - INPUT);
        box.setHint(Component.literal("Message @" + peer.label()).withColor(Theme.MUTED));
        box.setValue(drafts.getOrDefault(peer.key(), ""));
        addRenderableWidget(box);
        input = box;
        inputView = View.CHAT;
        inputPeer = peer;
        IconButton send = new IconButton(Icons.SEND, "Send", this::send);
        send.setPosition(
                mainX + mainWidth - PAD - IconButton.SIZE, py + ph - PAD - INPUT + (INPUT - IconButton.SIZE) / 2);
        addRenderableWidget(send);
        setInitialFocus(box);
    }

    /** One of the chat header's actions; {@code warn} for Block's armed second click. */
    private record Action(Icons icon, String label, Runnable run, boolean warn) {}

    private int actionWidth(String label) {
        return 24 + font.width(label) + 8;
    }

    /** A labelled action in the header, placed to the left of {@code right}; returns the next one's right edge. */
    private int action(int right, Action action) {
        int buttonWidth = actionWidth(action.label());
        Button button = Ui.menuButton(Component.literal(action.label()), action.icon(), buttonWidth, action.run());
        button.setPosition(right - buttonWidth, py + (HEADER - Theme.CONTROL_HEIGHT) / 2);
        addRenderableWidget(button);
        return right - buttonWidth - 4;
    }

    /** An action as an icon alone, named by its tooltip, for a header too narrow for labels. */
    private int iconAction(int right, Action action) {
        IconButton button = new IconButton(action.icon(), action.label(), action.run());
        if (action.warn()) {
            button.tint = Theme.NEGATIVE;
        }
        button.setPosition(right - IconButton.SIZE, py + (HEADER - IconButton.SIZE) / 2);
        addRenderableWidget(button);
        return right - IconButton.SIZE - 4;
    }

    private void initAddFriend() {
        int formWidth = Math.min(mainWidth - 4 * PAD, 260);
        int x = mainX + (mainWidth - formWidth) / 2;
        int y = py + HEADER + 54;
        EditBox name = new EditBox(font, x, y, formWidth, Theme.CONTROL_HEIGHT, Component.literal("Incari username")) {
            @Override
            public void insertText(String typed) {
                StringBuilder allowed = new StringBuilder();
                for (int i = 0; i < typed.length(); i++) {
                    if (Credentials.usernameCharacter(typed.charAt(i))) {
                        allowed.append(typed.charAt(i));
                    }
                }
                super.insertText(allowed.toString());
            }
        };
        name.setMaxLength(Credentials.MAX_USERNAME);
        name.setHint(Component.literal("Their Incari username").withColor(Theme.MUTED));
        // Laid out again whenever the menu's shape changes: what was typed stays.
        name.setValue(friendDraft);
        addRenderableWidget(name);
        input = name;
        inputView = View.ADD_FRIEND;
        inputPeer = null;
        Button send = Ui.menuButton(
                Component.literal("Send Friend Request"), Icons.PERSON_ADD, formWidth, () -> befriendByName(name));
        send.setPosition(x, y + Theme.CONTROL_HEIGHT + 6);
        addRenderableWidget(send);
        setInitialFocus(name);
    }

    private void initSettings() {
        int controlWidth = SETTING_CONTROL_WIDTH;
        int x = mainX + mainWidth - PAD * 2 - controlWidth;
        int y = py + HEADER + 30;
        addRenderableWidget(place(
                Ui.choice(
                        Component.literal("Who can message you"),
                        List.of(Social.MessagesFrom.values()),
                        social.messagesFrom(),
                        from -> Component.literal(from.label()),
                        controlWidth,
                        from -> online.setMessagesFrom(from, saved("Saved."))),
                x,
                y));
        addRenderableWidget(place(
                Ui.toggle(
                        Component.literal("Friend requests"),
                        social.friendRequests(),
                        controlWidth,
                        on -> online.setFriendRequests(on, saved("Saved."))),
                x,
                y + 40));
        addRenderableWidget(place(
                Ui.toggle(
                        Component.literal("Show friends your server"),
                        online.sharesServer(),
                        controlWidth,
                        on -> online.setShareServer(on, saved("Saved."))),
                x,
                y + 80));
        int half = (mainWidth - 4 * PAD - 6) / 2;
        Button blocked = Ui.menuButton(
                Component.literal("Blocked Players"),
                Icons.BLOCK,
                half,
                () -> minecraft.gui.setScreen(new BlockedPlayersScreen(this)));
        addRenderableWidget(place(blocked, mainX + 2 * PAD, y + 128));
        Button account = Ui.menuButton(
                Component.literal("Account"),
                Icons.PERSON,
                half,
                () -> minecraft.gui.setScreen(new AccountScreen(this)));
        addRenderableWidget(place(account, mainX + 2 * PAD + half + 6, y + 128));
    }

    private static <T extends net.minecraft.client.gui.components.AbstractWidget> T place(T widget, int x, int y) {
        widget.setPosition(x, y);
        return widget;
    }

    // ---------------------------------------------------------------------------------------------
    // What the player does
    // ---------------------------------------------------------------------------------------------

    private void show(View next) {
        keepDraft();
        view = next;
        if (next != View.CHAT) {
            select(null);
        }
        rebuildWidgets();
    }

    private void select(Social.@Nullable Peer peer) {
        keepDraft();
        selected = peer;
        blockArmedUntil = 0;
        chatScroll = 0;
        online.viewing(peer);
        if (peer != null) {
            view = View.CHAT;
            online.loadOlder(peer);
        }
    }

    /** Keeps what's typed in the text field, for what the field was made for (not what's chosen now). */
    private void keepDraft() {
        Social.Peer peer = inputPeer;
        EditBox box = input;
        if (box == null) {
            return;
        }
        if (inputView == View.CHAT && peer != null) {
            drafts.put(peer.key(), box.getValue());
        } else if (inputView == View.ADD_FRIEND) {
            friendDraft = box.getValue();
        }
    }

    private void send() {
        Social.Peer peer = selected;
        EditBox box = input;
        if (peer == null || box == null || box.getValue().isBlank()) {
            return;
        }
        String text = box.getValue();
        box.setValue("");
        drafts.remove(peer.key());
        chatScroll = 0;
        online.sendMessage(peer, text, result -> {
            if (!result.ok()) {
                say(Objects.requireNonNull(result.failure()).message(), Theme.NEGATIVE);
            }
        });
    }

    private void befriendSelected() {
        Social.Peer peer = selected;
        IncariPlayer player = peer == null ? null : online.worldPlayers().get(peer.label());
        if (peer == null || player == null) {
            say("They've left this server.", Theme.NEGATIVE);
            return;
        }
        online.befriend(peer.label(), player, done("Friend request sent to " + peer.label() + "."));
    }

    private void befriendByName(EditBox name) {
        String username = name.getValue();
        if (Credentials.usernameProblem(username) != null) {
            say("Type an Incari username: 3 to 16 letters, digits or _.", Theme.NEGATIVE);
            return;
        }
        online.befriend(username, result -> {
            if (result.ok()) {
                friendDraft = "";
                // The field shown now: the menu may have been laid out again since.
                EditBox shown = input;
                if (inputView == View.ADD_FRIEND
                        && shown != null
                        && shown.getValue().equals(username)) {
                    shown.setValue("");
                }
                say("Friend request sent to " + username + ".", Theme.POSITIVE);
            } else {
                say(
                        result.failure() == Failure.REJECTED ? "No Incari account has that username." : message(result),
                        Theme.NEGATIVE);
            }
        });
    }

    private void accept(Social.Request request) {
        online.accept(request, done("You and " + request.label() + " are friends now."));
    }

    private void decline(Social.Request request) {
        online.decline(request, done(request.incoming() ? "Request declined." : "Request cancelled."));
    }

    private void unfriend() {
        Social.Peer peer = selected;
        Social.Friend friend = peer == null ? null : social.friend(peer.id());
        if (friend == null) {
            return;
        }
        online.unfriend(friend, result -> {
            if (result.ok()) {
                select(null);
                view = View.HOME;
                say("Removed " + friend.username() + " from your friends.", Theme.MUTED);
            } else {
                say(message(result), Theme.NEGATIVE);
            }
        });
    }

    /** Blocks the person shown, on a second click: the first arms it. */
    private void block() {
        Social.Peer peer = selected;
        if (peer == null) {
            return;
        }
        if (blockArmedUntil == 0) {
            blockArmedUntil = System.currentTimeMillis() + 4000;
            rebuildWidgets();
            return;
        }
        blockArmedUntil = 0;
        Consumer<Result<Void>> after = result -> {
            if (result.ok()) {
                select(null);
                view = View.HOME;
                say("Blocked " + peer.label() + ". Unblock them in Chat Settings.", Theme.MUTED);
            } else {
                say(message(result), Theme.NEGATIVE);
            }
        };
        if (peer.friend()) {
            Social.Friend friend = social.friend(peer.id());
            if (friend != null) {
                online.blockFriend(friend, after);
            }
        } else {
            online.blockPlayer(peer.label(), new IncariPlayer(peer.id(), false, List.of()), after);
        }
    }

    private void report() {
        Social.Peer peer = selected;
        if (peer == null) {
            return;
        }
        Social.Friend friend = peer.friend() ? social.friend(peer.id()) : null;
        minecraft.gui.setScreen(
                friend != null
                        ? ReportScreen.username(this, friend.username())
                        : ReportScreen.player(this, peer.label(), new IncariPlayer(peer.id(), false, List.of())));
    }

    private Consumer<Result<Void>> done(String success) {
        return result -> say(result.ok() ? success : message(result), result.ok() ? Theme.POSITIVE : Theme.NEGATIVE);
    }

    private Consumer<Result<Void>> saved(String success) {
        return result -> {
            say(result.ok() ? success : message(result), result.ok() ? Theme.POSITIVE : Theme.NEGATIVE);
            rebuildWidgets();
        };
    }

    private static String message(Result<?> result) {
        Failure failure = result.failure();
        return failure == null ? "" : failure.message();
    }

    private void say(String text, int color) {
        status = text;
        statusColor = color;
        statusUntil = System.currentTimeMillis() + 5000;
    }

    // ---------------------------------------------------------------------------------------------
    // The people list
    // ---------------------------------------------------------------------------------------------

    /** How the player stands with a peer, for the header's actions. */
    private enum Relation {
        FRIEND,
        /** An Incari player on this server. */
        PLAYER,
        /** A player who messaged from this server and has left it. */
        GONE
    }

    private Relation relation(Social.Peer peer) {
        if (peer.friend()) {
            return social.friend(peer.id()) != null ? Relation.FRIEND : Relation.GONE;
        }
        return online.worldPlayers().containsKey(peer.label()) ? Relation.PLAYER : Relation.GONE;
    }

    /** The list, made again when anything online changes: requests, friends, then players here. */
    private void buildRows() {
        long revision = online.revision();
        if (revision == rowsRevision) {
            return;
        }
        rowsRevision = revision;
        rows.clear();
        if (!online.signedIn()) {
            return;
        }
        List<Social.Request> requests = social.requests();
        if (!requests.isEmpty()) {
            rows.add(header("Friend Requests", requests.size()));
            for (Social.Request request : requests) {
                rows.add(new Row(
                        false,
                        request.label(),
                        request.incoming() ? "Wants to be friends" : "Request sent",
                        null,
                        request,
                        false,
                        false,
                        0));
            }
        }
        List<Social.Friend> friends = social.friends();
        int on = 0;
        for (Social.Friend friend : friends) {
            on += friend.online() ? 1 : 0;
        }
        rows.add(new Row(
                true, "Friends", friends.isEmpty() ? "" : on + "/" + friends.size(), null, null, true, false, 0));
        if (friends.isEmpty()) {
            rows.add(new Row(true, "No friends yet", "", null, null, false, false, -1));
        }
        for (Social.Friend friend : friends) {
            Social.Peer peer = new Social.Peer(true, friend.fid(), friend.username());
            Social.Conversation conversation = social.existing(peer);
            String detail = !friend.online() ? "Offline" : friend.server() != null ? friend.server() : "Online";
            rows.add(new Row(
                    false,
                    friend.username(),
                    detail,
                    peer,
                    null,
                    true,
                    friend.online(),
                    conversation == null ? 0 : conversation.unread()));
        }
        // Everyone else here who's an Incari player, below every friend.
        Map<String, IncariPlayer> here = online.worldPlayers();
        List<String> names = new ArrayList<>();
        for (Map.Entry<String, IncariPlayer> each : here.entrySet()) {
            if (!each.getValue().friend()) {
                names.add(each.getKey());
            }
        }
        Map<String, String> messaged = social.playersMessaged();
        for (Map.Entry<String, String> each : messaged.entrySet()) {
            if (!here.containsKey(each.getValue()) && !names.contains(each.getValue())) {
                names.add(each.getValue());
            }
        }
        names.sort(String.CASE_INSENSITIVE_ORDER);
        if (!names.isEmpty() || online.inWorld()) {
            rows.add(header("On This Server", names.size()));
        }
        if (names.isEmpty() && online.inWorld()) {
            rows.add(new Row(true, "No other Incari players", "", null, null, false, false, -1));
        }
        for (String name : names) {
            IncariPlayer player = here.get(name);
            String pid = player != null ? player.pid() : pidOf(messaged, name);
            Social.Peer peer = new Social.Peer(false, pid, name);
            Social.Conversation conversation = social.existing(peer);
            rows.add(new Row(
                    false,
                    name,
                    player != null ? "Incari player" : "Left the server",
                    peer,
                    null,
                    false,
                    player != null,
                    conversation == null ? 0 : conversation.unread()));
        }
    }

    private static String pidOf(Map<String, String> messaged, String name) {
        for (Map.Entry<String, String> each : messaged.entrySet()) {
            if (each.getValue().equals(name)) {
                return each.getKey();
            }
        }
        return "";
    }

    private static Row header(String label, int count) {
        return new Row(true, label, count == 0 ? "" : String.valueOf(count), null, null, false, false, 0);
    }

    private int listHeight() {
        int total = 0;
        for (Row row : rows) {
            total += row.header() ? SECTION : ROW;
        }
        return total;
    }

    // ---------------------------------------------------------------------------------------------
    // Input
    // ---------------------------------------------------------------------------------------------

    @Override
    public boolean mouseClicked(MouseButtonEvent event, boolean doubleClick) {
        if (super.mouseClicked(event, doubleClick)) {
            keepTyping();
            return true;
        }
        double mx = event.x();
        double my = event.y();
        if (mx < px || mx >= px + side || my < listTop || my >= listBottom) {
            return false;
        }
        int y = listTop - (int) listScroll;
        for (Row row : rows) {
            int h = row.header() ? SECTION : ROW;
            if (!row.header() && my >= y && my < y + h) {
                clickRow(row, mx);
                return true;
            }
            y += h;
        }
        return false;
    }

    /**
     * A click on a button (Send, Block, Add Friend...) gives the text field its focus back: with no
     * field focused, the Friends key (O) would close the menu as the player went on typing.
     */
    @SuppressWarnings("ReferenceEquality") // This very field, not an equal one.
    private void keepTyping() {
        EditBox box = input;
        if (box != null && getFocused() != box && children().contains(box)) {
            setFocused(box);
        }
    }

    private void clickRow(Row row, double mx) {
        Social.Request request = row.request();
        if (request != null) {
            int right = px + side - PAD;
            if (mx >= right - 14) {
                decline(request);
            } else if (request.incoming() && mx >= right - 30) {
                accept(request);
            }
            return;
        }
        Social.Peer peer = row.peer();
        if (peer != null) {
            select(peer);
            rebuildWidgets();
        }
    }

    @Override
    public boolean mouseScrolled(double mx, double my, double scrollX, double scrollY) {
        if (mx >= px && mx < px + side && my >= listTop && my < listBottom) {
            int max = Math.max(0, listHeight() - (listBottom - listTop));
            listScroll = Math.clamp(listScroll - scrollY * 20, 0, max);
            return true;
        }
        if (view == View.CHAT && mx >= mainX && my >= chatTop && my < chatBottom) {
            int max = Math.max(0, linesHeight - (chatBottom - chatTop - 8));
            chatScroll = Math.clamp(chatScroll + scrollY * 20, 0, max);
            Social.Peer peer = selected;
            if (peer != null && chatScroll >= max - 4) {
                // At the top: the page before, if there is one.
                online.loadOlder(peer);
            }
            return true;
        }
        return super.mouseScrolled(mx, my, scrollX, scrollY);
    }

    @Override
    public boolean keyPressed(KeyEvent event) {
        if (event.isConfirmation() && view == View.CHAT && input != null && input.isFocused()) {
            send();
            return true;
        }
        if (event.isConfirmation() && view == View.ADD_FRIEND && input != null) {
            befriendByName(input);
            return true;
        }
        return super.keyPressed(event);
    }

    @Override
    public void tick() {
        super.tick();
        if (blockArmedUntil > 0 && System.currentTimeMillis() > blockArmedUntil) {
            blockArmedUntil = 0;
        }
        Social.Peer peer = selected;
        if (peer != null && relation(peer) == Relation.GONE && peer.friend()) {
            // Unfriended (or blocked) from the other side.
            select(null);
            view = View.HOME;
        }
        if (!Objects.equals(shape(), shownShape)) {
            rebuildWidgets();
        }
    }

    // ---------------------------------------------------------------------------------------------
    // Drawing
    // ---------------------------------------------------------------------------------------------

    @Override
    public void extractRenderState(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
        buildRows();
        // The sidebar's shade and the lines between the parts.
        RoundRects.fill(graphics, px + 1, py + 1, side - 1, ph - 2, Theme.PANEL_RADIUS, SIDEBAR_SHADE);
        graphics.fill(px + side, py + 6, px + side + 1, py + ph - 6, LINE);
        graphics.fill(mainX + 6, py + HEADER, px + pw - 6, py + HEADER + 1, LINE);
        graphics.fill(px + 6, listBottom, px + side - 6, listBottom + 1, LINE);

        drawList(graphics, mouseX, mouseY);
        drawStrip(graphics);
        if (!online.signedIn()) {
            drawSignedOut(graphics);
        } else {
            switch (view) {
                case HOME -> drawHome(graphics);
                case CHAT -> drawChat(graphics);
                case ADD_FRIEND -> drawAddFriend(graphics);
                case SETTINGS -> drawSettings(graphics);
            }
        }
        super.extractRenderState(graphics, mouseX, mouseY, a);
        if (!status.isEmpty() && System.currentTimeMillis() < statusUntil) {
            int textWidth = font.width(status);
            int x = mainX + (mainWidth - textWidth) / 2;
            int y = view == View.CHAT ? chatBottom - 14 : py + ph - PAD - 12;
            RoundRects.fill(graphics, x - 6, y - 3, textWidth + 12, 13, 6, Theme.FLOATING);
            graphics.text(font, status, x, y, statusColor, false);
        }
    }

    private void drawList(GuiGraphicsExtractor graphics, int mouseX, int mouseY) {
        graphics.enableScissor(px, listTop, px + side, listBottom);
        int y = listTop - (int) listScroll;
        Social.Peer chosen = selected;
        for (Row row : rows) {
            if (row.header() && row.unread() < 0) {
                // An empty section's note, a section's height.
                graphics.text(font, row.label(), px + PAD + 2, y + 4, ARGB.multiplyAlpha(Theme.MUTED, 0.6F), false);
                y += SECTION;
                continue;
            }
            if (row.header()) {
                if (y + SECTION > listTop && y < listBottom) {
                    String label = row.label().toUpperCase(java.util.Locale.ROOT);
                    graphics.text(font, label, px + PAD + 2, y + 6, Theme.MUTED, false);
                    if (!row.detail().isEmpty()) {
                        graphics.text(
                                font,
                                row.detail(),
                                px + side - PAD - font.width(row.detail()),
                                y + 6,
                                ARGB.multiplyAlpha(Theme.MUTED, 0.7F),
                                false);
                    }
                }
                y += SECTION;
                continue;
            }
            if (y + ROW > listTop && y < listBottom) {
                drawRow(graphics, row, y, mouseX, mouseY, chosen);
            }
            y += ROW;
        }
        graphics.disableScissor();
    }

    private void drawRow(
            GuiGraphicsExtractor graphics, Row row, int y, int mouseX, int mouseY, Social.@Nullable Peer chosen) {
        int x = px + 5;
        int w = side - 10;
        boolean hot = mouseX >= x
                && mouseX < x + w
                && mouseY >= y
                && mouseY < y + ROW
                && mouseY >= listTop
                && mouseY < listBottom;
        Social.Peer peer = row.peer();
        boolean on = peer != null && chosen != null && chosen.key().equals(peer.key());
        if (on || hot) {
            RoundRects.fill(graphics, x, y + 1, w, ROW - 2, Theme.RADIUS, on ? Theme.BUTTON_HOT : Theme.BUTTON);
        }
        int avatarY = y + (ROW - AVATAR) / 2;
        PlayerSkin skin = row.friend() || row.request() != null ? null : Avatars.skin(row.label());
        Avatars.draw(graphics, font, x + 3, avatarY, AVATAR, row.label(), skin);
        if (row.friend()) {
            Avatars.status(graphics, x + 3, avatarY, AVATAR, row.online(), on || hot ? 0xFF2A333D : 0xFF20272F);
        }
        int textX = x + 3 + AVATAR + 6;
        int rightEdge = x + w - 4;
        Social.Request request = row.request();
        if (request != null) {
            // Accept and decline right on the row: one click, no screen between.
            int buttonY = y + (ROW - 12) / 2;
            rowButton(graphics, Icons.CLOSE, rightEdge - 12, buttonY, mouseX, mouseY, Theme.NEGATIVE);
            if (request.incoming()) {
                rowButton(graphics, Icons.CHECK, rightEdge - 28, buttonY, mouseX, mouseY, Avatars.ONLINE);
                rightEdge -= 30;
            } else {
                rightEdge -= 14;
            }
        } else if (row.unread() > 0) {
            String count = row.unread() > 99 ? "99+" : String.valueOf(row.unread());
            int badge = Math.max(12, font.width(count) + 6);
            RoundRects.fill(graphics, rightEdge - badge, y + (ROW - 11) / 2, badge, 11, 5, 0xFFE0565B);
            graphics.text(
                    font,
                    count,
                    rightEdge - badge + (badge - font.width(count)) / 2 + 1,
                    y + (ROW - 11) / 2 + 2,
                    Theme.TEXT,
                    false);
            rightEdge -= badge + 4;
        }
        int nameWidth = rightEdge - textX;
        boolean dim = !row.friend() && request == null && !row.online();
        int nameColor = on || row.unread() > 0 ? Theme.TEXT : dim ? ARGB.multiplyAlpha(Theme.MUTED, 0.7F) : Theme.MUTED;
        if (row.friend() && row.online()) {
            nameColor = on ? Theme.TEXT : 0xFFE6EBF0;
        }
        graphics.text(font, font.plainSubstrByWidth(row.label(), nameWidth), textX, y + 3, nameColor, false);
        String detail = row.detail();
        int detailColor = ARGB.multiplyAlpha(Theme.MUTED, 0.75F);
        if (!row.friend() && request == null && row.online()) {
            // A player here, not a friend: a small tag says where they're from.
            detail = "On this server";
            detailColor = ARGB.multiplyAlpha(Theme.ACCENT, 0.8F);
        }
        graphics.pose().pushMatrix();
        graphics.pose().translate(textX, y + 13);
        graphics.pose().scale(0.75F);
        graphics.text(font, font.plainSubstrByWidth(detail, (int) (nameWidth / 0.75F)), 0, 0, detailColor, false);
        graphics.pose().popMatrix();
    }

    private void rowButton(
            GuiGraphicsExtractor graphics, Icons icon, int x, int y, int mouseX, int mouseY, int hotColor) {
        boolean hot = mouseX >= x && mouseX < x + 12 && mouseY >= y && mouseY < y + 12;
        RoundRects.fill(graphics, x, y, 12, 12, 6, hot ? ARGB.multiplyAlpha(hotColor, 0.35F) : 0x2EFFFFFF);
        icon.draw(graphics, x + 2, y + 2, 8, hot ? hotColor : Theme.TEXT);
    }

    /** The player's own strip at the sidebar's foot: their account and whether they're connected. */
    private void drawStrip(GuiGraphicsExtractor graphics) {
        int y = py + ph - STRIP + (STRIP - AVATAR) / 2;
        int x = px + PAD;
        String name = online.accountName();
        String label = name == null ? "Not signed in" : name;
        Avatars.draw(graphics, font, x, y, AVATAR, label, null);
        boolean connected = online.signedIn();
        Avatars.status(graphics, x, y, AVATAR, connected, 0xFF1C2229);
        int textWidth = side - 2 * PAD - AVATAR - 6 - IconButton.SIZE - 4;
        graphics.text(font, font.plainSubstrByWidth(label, textWidth), x + AVATAR + 6, y - 1, Theme.TEXT, false);
        String state =
                switch (online.status()) {
                    case SIGNED_IN -> social.messagesFrom() == Social.MessagesFrom.NOBODY ? "Messages off" : "Online";
                    case CONNECTING -> "Connecting...";
                    case SIGNED_OUT -> "Signed out";
                    case UPDATE_NEEDED -> "Update needed";
                    case OFF -> "Unavailable";
                };
        graphics.pose().pushMatrix();
        graphics.pose().translate(x + AVATAR + 6, y + 10);
        graphics.pose().scale(0.75F);
        graphics.text(font, state, 0, 0, ARGB.multiplyAlpha(Theme.MUTED, 0.85F), false);
        graphics.pose().popMatrix();
    }

    private void drawSignedOut(GuiGraphicsExtractor graphics) {
        int cx = mainX + mainWidth / 2;
        int y = signedOutTop();
        Icons.CHAT.draw(graphics, cx - 12, y, 24, ARGB.multiplyAlpha(Theme.ACCENT, 0.9F));
        graphics.centeredText(font, signedOutText()[0], cx, y + 32, Theme.TEXT);
        int lineY = y + 46;
        for (FormattedCharSequence line : signedOutLines()) {
            graphics.centeredText(font, line, cx, lineY, Theme.MUTED);
            lineY += 10;
        }
    }

    /**
     * Where the signed-out panel's icon goes: the icon, title, text and button (if any) centred as
     * one, so however many lines the text wraps to, the button stays below it.
     */
    private int signedOutTop() {
        boolean button = online.status() == Status.SIGNED_OUT || online.status() == Status.CONNECTING;
        int height = 46 + signedOutLines().size() * 10 + (button ? 6 + Theme.CONTROL_HEIGHT : 0);
        return py + Math.max(4, (ph - height) / 2);
    }

    private List<FormattedCharSequence> signedOutLines() {
        return font.split(Component.literal(signedOutText()[1]), Math.min(mainWidth - 40, 260));
    }

    /** The signed-out panel's title and text, for the service's state. */
    private String[] signedOutText() {
        return switch (online.status()) {
            case OFF -> new String[] {"Chat isn't available", "Incari accounts aren't available in this version."};
            case UPDATE_NEEDED -> new String[] {"Update required", "Update Incari Client to use chat."};
            case CONNECTING ->
                new String[] {
                    "Connecting...", "Your network may be blocking Incari. The game works the same without it."
                };
            default -> new String[] {"Incari Chat", "Sign in to add friends and message them on any server."};
        };
    }

    private void drawHome(GuiGraphicsExtractor graphics) {
        graphics.text(font, "Incari Chat", mainX + PAD + 4, py + (HEADER - 8) / 2, Theme.TEXT, false);
        int cx = mainX + mainWidth / 2;
        int y = py + HEADER + (ph - HEADER) / 2 - 40;
        Icons.CHAT.draw(graphics, cx - 14, y, 28, ARGB.multiplyAlpha(Theme.ACCENT, 0.9F));
        Boolean available = social.available();
        String body;
        if (Boolean.FALSE.equals(available)) {
            body = "Friends and messages aren't available yet. You can still block and report players.";
        } else if (social.friends().isEmpty() && social.requests().isEmpty()) {
            body = "No friends yet. Add one by their Incari username, or pick an Incari player on this server.";
        } else {
            body = "Pick someone on the left to chat.";
        }
        int lineY = y + 38;
        for (FormattedCharSequence line : font.split(Component.literal(body), Math.min(mainWidth - 40, 280))) {
            graphics.centeredText(font, line, cx, lineY, Theme.MUTED);
            lineY += 10;
        }
        if (social.messagesFrom() == Social.MessagesFrom.NOBODY) {
            graphics.centeredText(
                    font, "Messages are off. Turn them on in Chat Settings.", cx, lineY + 8, Theme.WARNING);
        }
    }

    private void drawAddFriend(GuiGraphicsExtractor graphics) {
        graphics.text(font, "Add Friend", mainX + PAD + 4, py + (HEADER - 8) / 2, Theme.TEXT, false);
        int formWidth = Math.min(mainWidth - 4 * PAD, 260);
        int x = mainX + (mainWidth - formWidth) / 2;
        int y = py + HEADER + 16;
        for (FormattedCharSequence line : font.split(
                Component.literal(
                        "Ask your friend for their Incari username, or pick a player on this server from the left."),
                formWidth)) {
            graphics.text(font, line, x, y, Theme.MUTED, false);
            y += 10;
        }
        if (!social.friendRequests()) {
            graphics.text(
                    font,
                    "Your friend requests are off, so nobody can add you.",
                    x,
                    py + HEADER + 112,
                    Theme.WARNING,
                    false);
        }
    }

    private void drawSettings(GuiGraphicsExtractor graphics) {
        graphics.text(font, "Chat Settings", mainX + PAD + 4, py + (HEADER - 8) / 2, Theme.TEXT, false);
        int x = mainX + 2 * PAD;
        int y = py + HEADER + 30;
        setting(graphics, x, y, "Who can message you", "Choose who can send you messages.");
        setting(graphics, x, y + 40, "Friend requests", "Off: nobody can send you one. You can still send them.");
        setting(graphics, x, y + 80, "Show friends your server", "Friends can see which server you're on.");
    }

    private void setting(GuiGraphicsExtractor graphics, int x, int y, String name, String help) {
        // The column left of the 150-wide controls (initSettings); narrow panels wrap the help.
        int column = mainX + mainWidth - PAD * 2 - SETTING_CONTROL_WIDTH - 6 - x;
        graphics.text(font, Ui.fit(font, name, column), x, y + 2, Theme.TEXT, false);
        graphics.pose().pushMatrix();
        graphics.pose().translate(x, y + 13);
        graphics.pose().scale(0.75F);
        int lineY = 0;
        for (FormattedCharSequence line : Ui.wrap(font, help, (int) (column / 0.75F), 2)) {
            graphics.text(font, line, 0, lineY, Theme.MUTED, false);
            lineY += font.lineHeight + 1;
        }
        graphics.pose().popMatrix();
    }

    private void drawChat(GuiGraphicsExtractor graphics) {
        Social.Peer peer = Objects.requireNonNull(selected);
        // The header: who, and how the player stands with them.
        int avatarY = py + (HEADER - AVATAR) / 2;
        Relation relation = relation(peer);
        PlayerSkin skin = peer.friend() ? null : Avatars.skin(peer.label());
        Avatars.draw(graphics, font, mainX + PAD + 2, avatarY, AVATAR, peer.label(), skin);
        Social.Friend friend = peer.friend() ? social.friend(peer.id()) : null;
        if (friend != null) {
            Avatars.status(graphics, mainX + PAD + 2, avatarY, AVATAR, friend.online(), 0xFF262E37);
        }
        int textX = mainX + PAD + 2 + AVATAR + 7;
        // On a narrow panel the name and its line are cut short of the header's buttons.
        int textWidth = Math.max(0, actionsLeft - 4 - textX);
        graphics.text(font, Ui.fit(font, peer.label(), textWidth), textX, avatarY - 1, Theme.TEXT, false);
        String about =
                switch (relation) {
                    case FRIEND ->
                        friend != null && friend.online()
                                ? "Friend · " + (friend.server() != null ? "on " + friend.server() : "online")
                                : "Friend · offline";
                    case PLAYER -> social.sentTo(peer) != null ? "Friend request sent" : "Incari player on this server";
                    case GONE -> "Left the server";
                };
        graphics.pose().pushMatrix();
        graphics.pose().translate(textX, avatarY + 10);
        graphics.pose().scale(0.75F);
        graphics.text(
                font,
                Ui.fit(font, about, (int) (textWidth / 0.75F)),
                0,
                0,
                relation == Relation.FRIEND ? ARGB.multiplyAlpha(Theme.ACCENT, 0.9F) : Theme.MUTED,
                false);
        graphics.pose().popMatrix();

        Social.Conversation conversation = social.conversation(peer);
        layOutLines(peer, conversation);
        int areaTop = chatTop + 4;
        int areaBottom = chatBottom - 2;
        graphics.enableScissor(mainX, areaTop, px + pw, areaBottom);
        int base = areaBottom - linesHeight + (int) chatScroll;
        if (linesHeight < areaBottom - areaTop) {
            base = areaBottom - linesHeight;
        }
        if (!conversation.hasMore() || !peer.friend()) {
            // The start of the conversation, as Discord marks it.
            int startY = base - 44;
            if (startY > areaTop - 44) {
                Avatars.draw(graphics, font, mainX + PAD + 4, startY, 22, peer.label(), skin);
                graphics.text(font, peer.label(), mainX + PAD + 32, startY + 2, Theme.TEXT, false);
                String start = peer.friend()
                        ? "This is the start of your messages with " + peer.label() + "."
                        : "Messages with players here end when one of you leaves.";
                graphics.text(
                        font,
                        font.plainSubstrByWidth(start, mainWidth - 50),
                        mainX + PAD + 32,
                        startY + 13,
                        Theme.MUTED,
                        false);
            }
        } else if (conversation.loading()) {
            graphics.centeredText(font, "Loading...", mainX + mainWidth / 2, base - 14, Theme.MUTED);
        }
        int textX0 = mainX + PAD + 4;
        for (ChatLine line : lines) {
            int y = base + line.y();
            if (y + 12 < areaTop || y > areaBottom) {
                continue;
            }
            if (line.head()) {
                PlayerSkin face = line.mine() || peer.friend() ? null : skin;
                Avatars.draw(graphics, font, textX0, y + 1, AVATAR, line.name(), face);
                graphics.text(
                        font,
                        line.name(),
                        textX0 + AVATAR + 7,
                        y,
                        line.mine() ? 0xFFE6EBF0 : Avatars.lighter(line.name()),
                        false);
                int nameEnd = textX0 + AVATAR + 7 + font.width(line.name()) + 6;
                graphics.pose().pushMatrix();
                graphics.pose().translate(nameEnd, y + 2);
                graphics.pose().scale(0.75F);
                graphics.text(font, line.time(), 0, 0, ARGB.multiplyAlpha(Theme.MUTED, 0.7F), false);
                graphics.pose().popMatrix();
            } else {
                graphics.text(font, line.text(), textX0 + AVATAR + 7, y, line.color(), false);
            }
        }
        graphics.disableScissor();
        // The input's frame: the field sits in a rounded well, as Discord's does.
        RoundRects.fill(
                graphics,
                mainX + PAD - 3,
                py + ph - PAD - INPUT - 3,
                mainWidth - 2 * PAD + 6,
                INPUT + 6,
                6,
                0x24000000);
    }

    /** Wraps the conversation into lines, grouping each run of one person's messages under one head. */
    private void layOutLines(Social.Peer peer, Social.Conversation conversation) {
        int textWidth = mainWidth - 2 * PAD - 4 - AVATAR - 7 - 6;
        long revision = social.revision();
        if (revision == linesRevision && textWidth == linesWidth && peer.equals(linesPeer)) {
            return;
        }
        linesRevision = revision;
        linesWidth = textWidth;
        linesPeer = peer;
        lines.clear();
        String me = Objects.requireNonNullElse(online.accountName(), "You");
        int y = 0;
        Social.Message last = null;
        for (Social.Message message : conversation.messages()) {
            boolean grouped = last != null && last.mine() == message.mine() && message.at() - last.at() < GROUP_MILLIS;
            if (!grouped) {
                if (last != null) {
                    y += 6;
                }
                String name = message.mine() ? me : peer.label();
                lines.add(new ChatLine(
                        true, name, message.mine(), time(message.at()), FormattedCharSequence.EMPTY, 0, y));
                y += 11;
            }
            int color =
                    switch (message.delivery()) {
                        case SENT -> 0xFFDCE2E8;
                        case SENDING -> ARGB.multiplyAlpha(Theme.MUTED, 0.8F);
                        case FAILED -> Theme.NEGATIVE;
                    };
            for (FormattedCharSequence wrapped : font.split(Component.literal(message.text()), textWidth)) {
                lines.add(new ChatLine(false, "", message.mine(), "", wrapped, color, y));
                y += 10;
            }
            if (message.delivery() == Social.Delivery.FAILED) {
                lines.add(new ChatLine(
                        false,
                        "",
                        true,
                        "",
                        Component.literal("Not delivered").getVisualOrderText(),
                        ARGB.multiplyAlpha(Theme.NEGATIVE, 0.7F),
                        y));
                y += 10;
            }
            last = message;
        }
        linesHeight = y;
    }

    /** "Today at 14:02", "Yesterday at ...", or the date: in the player's own time zone. */
    private static String time(long at) {
        long now = System.currentTimeMillis();
        TimeZone zone = TimeZone.getDefault();
        long localAt = at + zone.getOffset(at);
        long localNow = now + zone.getOffset(now);
        long day = Math.floorDiv(localAt, 86_400_000L);
        long today = Math.floorDiv(localNow, 86_400_000L);
        long minutes = Math.floorMod(localAt, 86_400_000L) / 60_000L;
        String clock = two(minutes / 60) + ":" + two(minutes % 60);
        if (day == today) {
            return "Today at " + clock;
        }
        if (day == today - 1) {
            return "Yesterday at " + clock;
        }
        // Days since 1970 to a calendar date, without java.time (TeaVM has none).
        java.util.GregorianCalendar calendar = new java.util.GregorianCalendar(TimeZone.getTimeZone("UTC"));
        calendar.setTimeInMillis(day * 86_400_000L);
        return (calendar.get(java.util.Calendar.MONTH) + 1)
                + "/"
                + calendar.get(java.util.Calendar.DAY_OF_MONTH)
                + "/"
                + calendar.get(java.util.Calendar.YEAR)
                + " "
                + clock;
    }

    private static String two(long value) {
        return value < 10 ? "0" + value : Long.toString(value);
    }

    @Override
    public void removed() {
        keepDraft();
        online.viewing(null);
    }

    @Override
    public void onClose() {
        minecraft.gui.setScreen(parent);
    }

    @Override
    public boolean isPauseScreen() {
        // Like the module menu: singleplayer pauses while it's open.
        return true;
    }

    /** A small round button with an icon, and a tooltip naming it. */
    private static final class IconButton extends AbstractButton {

        static final int SIZE = 16;

        private final Icons icon;
        private final Runnable action;
        /** The icon's color whatever the pointer does, for a warning (Block armed); 0 for none. */
        int tint;

        IconButton(Icons icon, String name, Runnable action) {
            super(0, 0, SIZE, SIZE, Component.literal(name));
            this.icon = icon;
            this.action = action;
            setTooltip(Tooltip.create(Component.literal(name)));
        }

        @Override
        public void onPress(InputWithModifiers input) {
            action.run();
        }

        @Override
        protected void extractContents(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
            boolean hot = isHoveredOrFocused();
            if (hot) {
                RoundRects.fill(graphics, getX(), getY(), SIZE, SIZE, SIZE / 2, Theme.BUTTON_HOT);
            }
            icon.draw(graphics, getX() + 3, getY() + 3, SIZE - 6, tint != 0 ? tint : hot ? Theme.TEXT : Theme.MUTED);
        }

        @Override
        protected void updateWidgetNarration(NarrationElementOutput output) {
            defaultButtonNarrationText(output);
        }
    }
}
