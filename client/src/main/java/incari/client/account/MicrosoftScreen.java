package incari.client.account;

import incari.client.ui.Theme;
import incari.net.msa.DeviceCode;
import incari.net.msa.MicrosoftAccount;
import incari.net.msa.MicrosoftSignIn;
import incari.net.msa.SignInException;
import java.net.URI;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import java.util.function.Consumer;
import net.minecraft.ChatFormatting;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.MultiLineTextWidget;
import net.minecraft.client.gui.components.StringWidget;
import net.minecraft.client.gui.layouts.HeaderAndFooterLayout;
import net.minecraft.client.gui.layouts.LinearLayout;
import net.minecraft.client.gui.screens.ConfirmLinkScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.CommonComponents;
import net.minecraft.network.chat.Component;
import org.jspecify.annotations.Nullable;

/**
 * Microsoft sign-in (ADR 0070), opened from Edit Profile: the device-code flow, so the password is
 * typed on Microsoft's own page, on any device, never into the game. Shows the code, waits for the
 * player to enter it, then signs in to Minecraft; or shows who's signed in, with Sign Out.
 *
 * <p>English only for now; Incari's screens don't have language files yet.
 */
public final class MicrosoftScreen extends Screen {

    private static final int TEXT_WIDTH = 300;
    private static final int BUTTON_WIDTH = 150;

    private enum State {
        IDLE,
        REQUESTING,
        CODE,
        SIGNING_IN
    }

    private final Screen parent;
    /** Made again on each init: the screen rebuilds itself as the sign-in moves on. */
    private HeaderAndFooterLayout layout = new HeaderAndFooterLayout(this);

    private State state = State.IDLE;
    private Component message = CommonComponents.EMPTY;
    private @Nullable DeviceCode code;
    private @Nullable MicrosoftSignIn signIn;
    /** Bumped when a sign-in is started or abandoned, so a late answer for an old one is dropped. */
    private int attempt;

    private boolean polling;
    private long nextPollMillis;
    private int intervalSeconds;

    public MicrosoftScreen(Screen parent) {
        super(Component.literal("Microsoft Account"));
        this.parent = parent;
    }

    @Override
    protected void init() {
        layout = new HeaderAndFooterLayout(this);
        layout.addTitleHeader(this.title, this.font);
        LinearLayout contents = layout.addToContents(LinearLayout.vertical().spacing(Theme.GAP));
        contents.defaultCellSetting().alignHorizontallyCenter();
        LinearLayout footer = layout.addToFooter(LinearLayout.horizontal().spacing(8));
        MicrosoftAccount account = MicrosoftAccounts.account();
        boolean inWorld = this.minecraft.level != null;

        switch (state) {
            case IDLE -> {
                if (account != null) {
                    contents.addChild(text(Component.literal("Signed in as ")
                            .append(Component.literal(account.name()).withStyle(ChatFormatting.YELLOW))));
                    contents.addChild(
                            text(
                                    Component.literal(
                                            "Servers that check accounts see you as this player. Your password never passes through Incari.")));
                    Button signOut = footer.addChild(Button.builder(Component.literal("Sign Out"), button -> signOut())
                            .build());
                    signOut.active = !inWorld;
                } else {
                    contents.addChild(text(Component.literal(
                            "Sign in to join servers that check Minecraft accounts (online mode). You sign in on"
                                    + " Microsoft's own page, on this or any other device.")));
                    contents.addChild(text(
                            Component.literal("Sign-in goes " + MicrosoftAccounts.route() + ", encrypted end to end.")
                                    .withStyle(ChatFormatting.GRAY)));
                    Button start = footer.addChild(Button.builder(Component.literal("Sign In"), button -> start())
                            .build());
                    start.active = !inWorld;
                }
                if (inWorld) {
                    contents.addChild(text(Component.literal("Leave the world to change accounts.")
                            .withStyle(ChatFormatting.GRAY)));
                }
                footer.addChild(Button.builder(CommonComponents.GUI_DONE, button -> onClose())
                        .build());
            }
            case REQUESTING -> {
                contents.addChild(text(Component.literal("Asking Microsoft for a sign-in code...")));
                footer.addChild(Button.builder(CommonComponents.GUI_CANCEL, button -> cancel())
                        .build());
            }
            case CODE -> {
                DeviceCode current = code;
                if (current != null) {
                    contents.addChild(
                            text(Component.literal("On any device, open microsoft.com/link and enter this code:")));
                    contents.addChild(new StringWidget(
                            Component.literal(current.userCode()).withStyle(ChatFormatting.YELLOW, ChatFormatting.BOLD),
                            this.font));
                    LinearLayout actions =
                            contents.addChild(LinearLayout.horizontal().spacing(8));
                    actions.addChild(Button.builder(
                                    Component.literal("Open Page"),
                                    ConfirmLinkScreen.confirmLink(this, URI.create(current.pageWithCode())))
                            .width(BUTTON_WIDTH / 2 + 20)
                            .build());
                    actions.addChild(Button.builder(Component.literal("Copy Code"), button -> {
                                this.minecraft.keyboardHandler.setClipboard(current.userCode());
                                message = Component.literal("Code copied.");
                                rebuildWidgets();
                            })
                            .width(BUTTON_WIDTH / 2 + 20)
                            .build());
                    contents.addChild(text(
                            Component.literal("Waiting for you to sign in...").withStyle(ChatFormatting.GRAY)));
                }
                footer.addChild(Button.builder(CommonComponents.GUI_CANCEL, button -> cancel())
                        .build());
            }
            case SIGNING_IN -> {
                contents.addChild(text(Component.literal("Signing in to Minecraft...")));
                footer.addChild(Button.builder(CommonComponents.GUI_CANCEL, button -> cancel())
                        .build());
            }
        }
        if (!message.equals(CommonComponents.EMPTY)) {
            contents.addChild(text(message));
        }

        layout.visitWidgets(this::addRenderableWidget);
        repositionElements();
    }

    private MultiLineTextWidget text(Component text) {
        return new MultiLineTextWidget(text, this.font)
                .setMaxWidth(Math.min(TEXT_WIDTH, this.width - 20))
                .setMaxRows(4)
                .setCentered(true);
    }

    @Override
    protected void repositionElements() {
        layout.arrangeElements();
    }

    // --- The flow ------------------------------------------------------------------------------

    private void start() {
        int mine = ++attempt;
        MicrosoftSignIn client = MicrosoftAccounts.signIn();
        signIn = client;
        message = CommonComponents.EMPTY;
        show(State.REQUESTING);
        whenDone(client.requestCode(), mine, requested -> {
            code = requested;
            intervalSeconds = requested.intervalSeconds();
            nextPollMillis = System.currentTimeMillis() + intervalSeconds * 1000L;
            show(State.CODE);
        });
    }

    @Override
    public void tick() {
        DeviceCode current = code;
        MicrosoftSignIn client = signIn;
        if (state != State.CODE || current == null || client == null || polling) {
            return;
        }
        if (System.currentTimeMillis() < nextPollMillis) {
            return;
        }
        polling = true;
        int mine = attempt;
        whenDone(client.poll(current), mine, poll -> {
            polling = false;
            switch (poll.state()) {
                case WAITING -> nextPollMillis = System.currentTimeMillis() + intervalSeconds * 1000L;
                case SLOW_DOWN -> {
                    intervalSeconds += 5;
                    nextPollMillis = System.currentTimeMillis() + intervalSeconds * 1000L;
                }
                case DONE -> {
                    MicrosoftSignIn.MicrosoftTokens tokens = poll.tokens();
                    if (tokens != null) {
                        show(State.SIGNING_IN);
                        whenDone(client.signIn(tokens), mine, this::signedIn);
                    }
                }
            }
        });
    }

    private void signedIn(MicrosoftAccount account) {
        MicrosoftAccounts.signedIn(account);
        code = null;
        message = Component.literal("Signed in.").withStyle(ChatFormatting.GREEN);
        show(State.IDLE);
    }

    private void signOut() {
        MicrosoftAccounts.signOut();
        message = Component.literal("Signed out. Your saved sign-in was deleted.");
        show(State.IDLE);
    }

    private void cancel() {
        attempt++;
        code = null;
        polling = false;
        message = CommonComponents.EMPTY;
        show(State.IDLE);
    }

    /** Runs {@code then} on the game thread once {@code future} succeeds, unless the attempt was abandoned. */
    private <T> void whenDone(CompletableFuture<T> future, int mine, Consumer<T> then) {
        // Answers arrive on the transport's thread; the screen changes on the game's.
        var _ = future.handle((value, failure) -> {
            this.minecraft.execute(() -> {
                if (mine != attempt) {
                    return;
                }
                if (failure != null) {
                    failed(failure);
                } else {
                    then.accept(value);
                }
            });
            return null;
        });
    }

    private void failed(Throwable failure) {
        Throwable cause = failure;
        while (cause instanceof CompletionException && cause.getCause() != null) {
            cause = cause.getCause();
        }
        String reason = cause instanceof SignInException && cause.getMessage() != null
                ? cause.getMessage()
                : "Sign-in failed. Try again.";
        attempt++;
        code = null;
        polling = false;
        message = Component.literal(reason).withStyle(ChatFormatting.RED);
        show(State.IDLE);
    }

    private void show(State next) {
        state = next;
        rebuildWidgets();
    }

    @Override
    public void onClose() {
        attempt++;
        this.minecraft.gui.setScreen(parent);
    }
}
