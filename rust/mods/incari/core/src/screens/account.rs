//! Incari Account and the screens it leads to (ADR 0058; AccountScreen, SignInScreen,
//! CreateAccountScreen, ForgotPasswordScreen, ChangePasswordScreen, ConfirmPasswordScreen,
//! RecoveryCodesScreen, BlockedPlayersScreen and ReportScreen in Java), each on `Form`.
//!
//! Java's `show(parent)` is `Go::Back` here, `show(new X(parent))` is `Go::Replace`, and
//! `show(new X(this))` is `Go::Open`.
use super::form::{CONTENT_WIDTH, Form, Stack, Tone};
use crate::online::service::Block;
use crate::online::{self, Answer, ReportReason, Status, rules};
use crate::screen::{Cx, Go, Screen};

fn done_label(cx: &Cx) -> String {
    cx.host.translate("gui.done")
}

fn cancel_label(cx: &Cx) -> String {
    cx.host.translate("gui.cancel")
}

// -------------------------------------------------------------------------------------------------
// Incari Account
// -------------------------------------------------------------------------------------------------

/// What the account screen's request was.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AccountAction {
    Share,
    SignOut { everywhere: bool },
}

/// Incari Account, from the title screen's player card: signing in, creating an account and
/// recovering one when signed out; the account's settings, blocked players, password, recovery
/// codes, signing out and deleting it when signed in.
#[derive(Default)]
pub struct Account {
    form: Form,
    action: Option<AccountAction>,
}

impl Screen for Account {
    fn draw(&mut self, cx: &mut Cx) {
        // A new account's codes the player left Create Account before seeing: shown now, once.
        if let Some(codes) = online::with(|o| o.take_unsaved_codes()) {
            cx.open(RecoveryCodes::new(codes, true));
            return;
        }
        if let Some(outcome) = self.form.poll() {
            match self.action.take() {
                Some(AccountAction::Share) if self.form.answered(&outcome) => {
                    self.form.status("Saved.", Tone::Positive);
                }
                Some(AccountAction::SignOut { everywhere }) if self.form.answered(&outcome) => {
                    let said = if everywhere { "Signed out on every device." } else { "Signed out." };
                    self.form.status(said, Tone::Positive);
                }
                _ => {}
            }
        }
        let (status, name, shares) = online::with(|o| (o.status(), o.account_name().map(str::to_owned), o.shares_server()));
        let done = done_label(cx);
        let mut s = Stack::default();
        let mut ids = AccountIds::default();
        match status {
            Status::Off => {
                s.text("Incari accounts aren't available in this version.", Tone::Muted);
                ids.done = Some(s.button(&done));
            }
            Status::UpdateNeeded => {
                s.text("Update Incari Client to use your account. Everything else still works.", Tone::Warning);
                ids.done = Some(s.button(&done));
            }
            Status::Connecting => {
                match &name {
                    Some(name) => s.centred(format!("Signing in as {name}..."), Tone::Text),
                    None => s.centred("Connecting to Incari...", Tone::Text),
                }
                s.text(
                    "If this takes a while, your network may be blocking Incari. You can still play without an \
                     account.",
                    Tone::Muted,
                );
                let row = s.row(&["Try Again", &done]);
                ids.retry = Some(row);
                ids.done = Some(row + 1);
                if name.is_some() {
                    s.gap(2.0);
                    ids.sign_out_here = Some(s.button("Sign Out on This Device"));
                }
                s.status_line();
            }
            Status::SignedOut => {
                s.text(
                    "An Incari account is optional. It lets friends find you and others see your cosmetics on \
                     any server. You only need a username and a password, with no email or real name.",
                    Tone::Muted,
                );
                s.gap(2.0);
                ids.sign_in = Some(s.button("Sign In"));
                ids.create = Some(s.button("Create Account"));
                ids.forgot = Some(s.button("Forgot Password"));
                s.status_line();
                ids.done = Some(s.button(&done));
            }
            Status::SignedIn => {
                s.centred(format!("Signed in as {}", name.as_deref().unwrap_or("?")), Tone::Accent);
                s.gap(2.0);
                ids.share = Some(s.toggle("Show friends which server I'm on", shares));
                ids.blocked = Some(s.button("Blocked Players"));
                ids.password = Some(s.row(&["Change Password", "New Recovery Codes"]));
                ids.sign_out = Some(s.row(&["Sign Out", "Sign Out Everywhere"]));
                ids.delete = Some(s.button("Delete Account"));
                s.status_line();
                ids.done = Some(s.button(&done));
            }
        }
        let pressed = self.form.show(cx, "Incari Account", s).pressed;
        let Some(p) = pressed else {
            return;
        };
        let is = |id: Option<usize>, offset: usize| id.map(|i| i + offset) == Some(p);
        if is(ids.done, 0) {
            cx.go(Go::Back);
        } else if is(ids.retry, 0) {
            online::with(|o| o.retry());
        } else if is(ids.sign_out_here, 0) {
            let t = online::with(|o| o.sign_out(false));
            online::with(|o| o.forget(t));
        } else if is(ids.sign_in, 0) {
            cx.open(SignIn::default());
        } else if is(ids.create, 0) {
            cx.open(CreateAccount::default());
        } else if is(ids.forgot, 0) {
            cx.open(ForgotPassword::default());
        } else if is(ids.share, 0) {
            if !self.form.busy() {
                let t = online::with(|o| o.set_share_server(!shares));
                self.form.sending(t, "Saving...");
                self.action = Some(AccountAction::Share);
            }
        } else if is(ids.blocked, 0) {
            cx.open(BlockedPlayers::default());
        } else if is(ids.password, 0) {
            cx.open(ChangePassword::default());
        } else if is(ids.password, 1) {
            cx.open(ConfirmPassword::new_codes());
        } else if is(ids.sign_out, 0) || is(ids.sign_out, 1) {
            let everywhere = is(ids.sign_out, 1);
            if !self.form.busy() {
                let t = online::with(|o| o.sign_out(everywhere));
                self.form.sending(t, "Signing out...");
                self.action = Some(AccountAction::SignOut { everywhere });
            }
        } else if is(ids.delete, 0) {
            cx.open(ConfirmPassword::delete());
        }
    }

    fn closed(&mut self, _cx: &mut Cx) {
        self.form.close();
    }
}

#[derive(Default)]
struct AccountIds {
    done: Option<usize>,
    retry: Option<usize>,
    sign_out_here: Option<usize>,
    sign_in: Option<usize>,
    create: Option<usize>,
    forgot: Option<usize>,
    share: Option<usize>,
    blocked: Option<usize>,
    password: Option<usize>,
    sign_out: Option<usize>,
    delete: Option<usize>,
}

// -------------------------------------------------------------------------------------------------
// Sign In
// -------------------------------------------------------------------------------------------------

const USERNAME: &str = "Incari username";
const PASSWORD: &str = "Password";

/// Sign In: an Incari username and password. A failure never says which of the two was wrong.
#[derive(Default)]
pub struct SignIn {
    form: Form,
}

impl Screen for SignIn {
    fn draw(&mut self, cx: &mut Cx) {
        if let Some(outcome) = self.form.poll() {
            if self.form.answered(&outcome) {
                cx.go(Go::Back);
                return;
            }
            self.form.set_value(PASSWORD, "");
        }
        // Signed in meanwhile (a resumed session): nothing to do here.
        if !self.form.busy() && online::with(|o| o.signed_in()) {
            cx.go(Go::Back);
            return;
        }
        let cancel = cancel_label(cx);
        let mut s = Stack::default();
        s.username_field(USERNAME);
        s.password_field(PASSWORD);
        s.status_line();
        let row = s.row(&[&cancel, "Sign In"]);
        let forgot = s.button("Forgot Password");
        s.submit(row + 1);
        s.focus(if self.form.value(USERNAME).is_empty() { USERNAME } else { PASSWORD });
        match self.form.show(cx, "Sign In", s).pressed {
            Some(p) if p == row => cx.go(Go::Back),
            Some(p) if p == row + 1 => self.sign_in(),
            Some(p) if p == forgot => cx.go(Go::Replace(Box::new(ForgotPassword::default()))),
            _ => {}
        }
    }

    fn closed(&mut self, _cx: &mut Cx) {
        self.form.close();
    }
}

impl SignIn {
    fn sign_in(&mut self) {
        if self.form.busy() {
            return;
        }
        let (name, secret) = (self.form.value(USERNAME).to_owned(), self.form.value(PASSWORD).to_owned());
        if name.is_empty() || secret.is_empty() {
            self.form.problem("Type your username and password.");
            return;
        }
        let t = online::with(|o| o.sign_in(&name, &secret));
        self.form.sending(t, "Signing in...");
    }
}

// -------------------------------------------------------------------------------------------------
// Create Account
// -------------------------------------------------------------------------------------------------

const NEW_USERNAME: &str = "Incari username (3-16 letters, digits, _)";
const NEW_PASSWORD: &str = "Password (10 or more characters)";
const PASSWORD_AGAIN: &str = "Password again";

/// Create Account: a username, a password typed twice, and a plain warning that the account can't
/// be recovered without the password or a recovery code. Sending it solves the service's proof of
/// work first (a few seconds on a Chromebook), then shows the recovery codes.
#[derive(Default)]
pub struct CreateAccount {
    form: Form,
}

impl Screen for CreateAccount {
    fn draw(&mut self, cx: &mut Cx) {
        if let Some(outcome) = self.form.poll()
            && self.form.answered(&outcome)
            && let Ok(Answer::Codes(codes)) = outcome
        {
            cx.go(Go::Replace(Box::new(RecoveryCodes::new(codes, true))));
            return;
        }
        let cancel = cancel_label(cx);
        let mut s = Stack::default();
        s.text(
            "Your Incari username is separate from your Minecraft name, and only friends you add will see it. \
             Don't use your real name.",
            Tone::Muted,
        );
        s.username_field(NEW_USERNAME);
        s.password_field(NEW_PASSWORD);
        s.password_field(PASSWORD_AGAIN);
        s.text(
            "There's no email. If you forget your password, you'll need a recovery code. You'll get 10 on the \
             next screen.",
            Tone::Warning,
        );
        s.status_line();
        let row = s.row(&[&cancel, "Create Account"]);
        s.submit(row + 1);
        s.focus(NEW_USERNAME);
        match self.form.show(cx, "Create Account", s).pressed {
            Some(p) if p == row => self.escape(cx),
            Some(p) if p == row + 1 => self.create(),
            _ => {}
        }
    }

    fn escape(&mut self, cx: &mut Cx) {
        if self.form.busy() && !online::with(|o| o.cancel_sign_up()) {
            // The form is out and the account may already exist: its codes are moments away.
            self.form.status("Almost done. Your account is being created...", Tone::Muted);
            return;
        }
        cx.go(Go::Back);
    }

    fn closed(&mut self, _cx: &mut Cx) {
        if let Some(t) = self.form.ticket() {
            // Still solving the challenge: nothing is made. Past that, the account was made all the
            // same: its codes wait for the account screen.
            online::with(|o| if o.cancel_sign_up() { o.forget(t) } else { o.keep_codes(t) });
        }
    }
}

impl CreateAccount {
    fn create(&mut self) {
        if self.form.busy() {
            return;
        }
        let name = self.form.value(NEW_USERNAME).to_owned();
        let secret = self.form.value(NEW_PASSWORD).to_owned();
        if let Some(problem) = rules::username_problem(&name) {
            self.form.problem(&format!("{problem}."));
            return;
        }
        if let Some(problem) = rules::password_problem(&secret, &name) {
            self.form.problem(&format!("Password: {problem}."));
            return;
        }
        if secret != self.form.value(PASSWORD_AGAIN) {
            self.form.problem("The two passwords don't match.");
            return;
        }
        let t = online::with(|o| o.sign_up(&name, &secret));
        self.form.sending(t, "Creating your account (a few seconds)...");
    }
}

// -------------------------------------------------------------------------------------------------
// Forgot Password
// -------------------------------------------------------------------------------------------------

const CODE: &str = "Recovery code (XXXX-XXXX-XXXX)";
const NEW: &str = "New password";
const NEW_AGAIN: &str = "New password again";

/// Forgot Password: the username, one recovery code (used up by this) and a new password. Every
/// other session ends. Without a code there's no way back, and the screen says so.
#[derive(Default)]
pub struct ForgotPassword {
    form: Form,
}

impl Screen for ForgotPassword {
    fn draw(&mut self, cx: &mut Cx) {
        if let Some(outcome) = self.form.poll()
            && self.form.answered(&outcome)
        {
            cx.go(Go::Back);
            return;
        }
        let cancel = cancel_label(cx);
        let mut s = Stack::default();
        s.text(
            "Enter one of the recovery codes you saved when you made the account. Without one, the account \
             can't be recovered.",
            Tone::Muted,
        );
        s.username_field(USERNAME);
        s.field(CODE, 20);
        s.password_field(NEW);
        s.password_field(NEW_AGAIN);
        s.status_line();
        let row = s.row(&[&cancel, "Set Password"]);
        s.submit(row + 1);
        s.focus(USERNAME);
        match self.form.show(cx, "Forgot Password", s).pressed {
            Some(p) if p == row => cx.go(Go::Back),
            Some(p) if p == row + 1 => self.recover(),
            _ => {}
        }
    }

    fn closed(&mut self, _cx: &mut Cx) {
        self.form.close();
    }
}

impl ForgotPassword {
    fn recover(&mut self) {
        if self.form.busy() {
            return;
        }
        let name = self.form.value(USERNAME).to_owned();
        let code = self.form.value(CODE).to_owned();
        let secret = self.form.value(NEW).to_owned();
        if name.is_empty() {
            self.form.problem("Type your username.");
            return;
        }
        if rules::normalize_code(&code).is_none() {
            self.form.problem("A code is 12 letters and digits, like ABCD-EFGH-JKMN.");
            return;
        }
        if let Some(problem) = rules::password_problem(&secret, &name) {
            self.form.problem(&format!("Password: {problem}."));
            return;
        }
        if secret != self.form.value(NEW_AGAIN) {
            self.form.problem("The two passwords don't match.");
            return;
        }
        let t = online::with(|o| o.recover(&name, &code, &secret));
        self.form.sending(t, "Setting your new password...");
    }
}

// -------------------------------------------------------------------------------------------------
// Change Password
// -------------------------------------------------------------------------------------------------

const CURRENT: &str = "Current password";

/// Change Password: the current one and a new one. Every other session the account has ends.
#[derive(Default)]
pub struct ChangePassword {
    form: Form,
    changed: bool,
}

impl Screen for ChangePassword {
    fn draw(&mut self, cx: &mut Cx) {
        if let Some(outcome) = self.form.poll() {
            if self.form.answered(&outcome) {
                self.changed = true;
            } else {
                self.form.set_value(CURRENT, "");
            }
        }
        let mut s = Stack::default();
        if self.changed {
            s.text("Your password is changed. Every other device was signed out.", Tone::Positive);
            let done = s.button(&done_label(cx));
            s.submit(done);
            if self.form.show(cx, "Change Password", s).pressed == Some(done) {
                cx.go(Go::Back);
            }
            return;
        }
        let cancel = cancel_label(cx);
        s.password_field(CURRENT);
        s.password_field(NEW);
        s.password_field(NEW_AGAIN);
        s.status_line();
        let row = s.row(&[&cancel, "Change Password"]);
        s.submit(row + 1);
        s.focus(CURRENT);
        match self.form.show(cx, "Change Password", s).pressed {
            Some(p) if p == row => cx.go(Go::Back),
            Some(p) if p == row + 1 => self.change(),
            _ => {}
        }
    }

    fn closed(&mut self, _cx: &mut Cx) {
        self.form.close();
    }
}

impl ChangePassword {
    fn change(&mut self) {
        if self.form.busy() {
            return;
        }
        let secret = self.form.value(NEW).to_owned();
        let account = online::with(|o| o.account_name().unwrap_or("").to_owned());
        if let Some(problem) = rules::password_problem(&secret, &account) {
            self.form.problem(&format!("New password: {problem}."));
            return;
        }
        if secret != self.form.value(NEW_AGAIN) {
            self.form.problem("The two new passwords don't match.");
            return;
        }
        let old = self.form.value(CURRENT).to_owned();
        let t = online::with(|o| o.change_password(&old, &secret));
        self.form.sending(t, "Changing your password...");
    }
}

// -------------------------------------------------------------------------------------------------
// Confirm Password: new recovery codes, or deleting the account
// -------------------------------------------------------------------------------------------------

const YOUR_PASSWORD: &str = "Your password";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Confirming {
    NewCodes,
    Delete,
}

/// An account action that needs the password again. The warning says what it does before the
/// player types anything.
pub struct ConfirmPassword {
    form: Form,
    what: Confirming,
}

impl ConfirmPassword {
    /// New Recovery Codes: ten new ones, shown once; the old ten stop working.
    pub fn new_codes() -> Self {
        Self { form: Form::default(), what: Confirming::NewCodes }
    }

    /// Delete Account: the account and everything tied to it, at once and for good.
    pub fn delete() -> Self {
        Self { form: Form::default(), what: Confirming::Delete }
    }

    fn title(&self) -> &'static str {
        match self.what {
            Confirming::NewCodes => "New Recovery Codes",
            Confirming::Delete => "Delete Account",
        }
    }
}

impl Screen for ConfirmPassword {
    fn draw(&mut self, cx: &mut Cx) {
        if let Some(outcome) = self.form.poll()
            && self.form.answered(&outcome)
        {
            match (self.what, outcome) {
                (Confirming::NewCodes, Ok(Answer::Codes(codes))) => {
                    cx.go(Go::Replace(Box::new(RecoveryCodes::new(codes, false))));
                }
                _ => cx.go(Go::Back),
            }
            return;
        }
        let (warning, tone, confirm) = match self.what {
            Confirming::NewCodes => (
                "You'll get ten new recovery codes, and the ones you have now will stop working.",
                Tone::Muted,
                "Get New Codes",
            ),
            Confirming::Delete => (
                "This deletes your Incari account with its friends, blocks and cosmetics. It can't be undone, \
                 and someone else may take the username.",
                Tone::Negative,
                "Delete Account",
            ),
        };
        let cancel = cancel_label(cx);
        let mut s = Stack::default();
        s.text(warning, tone);
        s.password_field(YOUR_PASSWORD);
        s.status_line();
        let row = s.row(&[&cancel, confirm]);
        s.submit(row + 1);
        s.focus(YOUR_PASSWORD);
        let title = self.title();
        match self.form.show(cx, title, s).pressed {
            Some(p) if p == row => cx.go(Go::Back),
            Some(p) if p == row + 1 => self.run(),
            _ => {}
        }
    }

    fn closed(&mut self, _cx: &mut Cx) {
        self.form.close();
    }
}

impl ConfirmPassword {
    fn run(&mut self) {
        if self.form.busy() {
            return;
        }
        let password = self.form.value(YOUR_PASSWORD).to_owned();
        if password.is_empty() {
            self.form.problem("Type your password.");
            return;
        }
        let (t, waiting) = match self.what {
            Confirming::NewCodes => (online::with(|o| o.new_recovery_codes(&password)), "Making new codes..."),
            Confirming::Delete => (online::with(|o| o.delete_account(&password)), "Deleting your account..."),
        };
        self.form.sending(t, waiting);
        self.form.set_value(YOUR_PASSWORD, "");
    }
}

// -------------------------------------------------------------------------------------------------
// Your Recovery Codes
// -------------------------------------------------------------------------------------------------

pub const CODES_FILE: &str = "incari-recovery-codes.txt";
const CODE_ROWS: usize = 5;

/// The ten recovery codes, shown once, with Copy and Download, and Continue only once the player
/// says they've saved them. Escape doesn't close it: closed by mistake, the codes are gone.
pub struct RecoveryCodes {
    form: Form,
    codes: Vec<String>,
    new_account: bool,
    saved: bool,
}

impl RecoveryCodes {
    pub fn new(codes: Vec<String>, new_account: bool) -> Self {
        Self { form: Form::default(), codes, new_account, saved: false }
    }

    /// The file and the clipboard's text.
    pub fn as_text(&self) -> String {
        let mut text = String::from("Incari account recovery codes. Each works once.\n\n");
        for code in &self.codes {
            text.push_str(code);
            text.push('\n');
        }
        text
    }
}

impl Screen for RecoveryCodes {
    fn draw(&mut self, cx: &mut Cx) {
        let mut s = Stack::default();
        s.text(
            format!(
                "{}If you forget your password, use one of these codes to set a new one. Each works once. Save \
                 them somewhere other than this device.",
                if self.new_account { "Account created. " } else { "Your old codes no longer work. " }
            ),
            Tone::Muted,
        );
        // Room for the codes, in two columns.
        s.space(CODE_ROWS as f32 * 12.0 + 6.0);
        s.status_line();
        // Only the browser build reaches the service: the file is a download.
        let row = s.row(&["Copy", "Download"]);
        let saved = s.toggle("I saved my recovery codes", self.saved);
        let proceed = s.button_active("Continue", self.saved);
        let shown = self.form.show(cx, "Your Recovery Codes", s);
        if let Some(&[x, y]) = shown.spaces.first() {
            let th = cx.p.theme;
            cx.p.fill(x, y - 3.0, CONTENT_WIDTH, CODE_ROWS as f32 * 12.0 + 4.0, th.field);
            for (i, code) in self.codes.iter().enumerate() {
                let (column, row) = (i / CODE_ROWS, i % CODE_ROWS);
                let cx0 = x + 6.0 + column as f32 * (CONTENT_WIDTH / 2.0).floor();
                let cy = y + row as f32 * 12.0;
                cx.p.text(&format!("{}.", i + 1), cx0, cy, th.muted, false);
                cx.p.text(code, cx0 + 14.0, cy, th.text, false);
            }
        }
        match shown.pressed {
            Some(p) if p == row => {
                cx.ui.copy = Some(self.as_text());
                self.form.status("Copied.", Tone::Positive);
            }
            Some(p) if p == row + 1 => {
                online::download(CODES_FILE, self.as_text().into_bytes());
                self.form.status(&format!("Downloading {CODES_FILE}"), Tone::Positive);
            }
            Some(p) if p == saved => self.saved = !self.saved,
            Some(p) if p == proceed => cx.go(Go::Back),
            _ => {}
        }
    }

    fn escape(&mut self, cx: &mut Cx) {
        if self.saved {
            cx.go(Go::Back);
        }
    }
}

// -------------------------------------------------------------------------------------------------
// Blocked Players
// -------------------------------------------------------------------------------------------------

const PAGE: usize = 5;
const BLOCK_NAME: &str = "Block by Incari username";

#[derive(Clone, PartialEq, Eq)]
enum BlockAction {
    Block,
    Unblock(Block),
}

/// Blocked Players: the accounts the player blocked, to unblock, and a field to block or report
/// one by its Incari username. Players in the world are blocked and reported from the chat menu.
/// The service enforces blocks both ways, and never tells the blocked player.
#[derive(Default)]
pub struct BlockedPlayers {
    form: Form,
    blocks: Option<Vec<Block>>,
    loading: Option<online::Ticket>,
    page: usize,
    action: Option<BlockAction>,
}

impl Screen for BlockedPlayers {
    fn draw(&mut self, cx: &mut Cx) {
        if self.blocks.is_none() && self.loading.is_none() {
            self.load();
        }
        if let Some(t) = self.loading
            && let Some(outcome) = online::with(|o| o.take_result(t))
        {
            self.loading = None;
            match outcome {
                Ok(Answer::Blocks(list)) => self.blocks = Some(list),
                Ok(_) => self.blocks = Some(Vec::new()),
                Err(f) => {
                    self.blocks = Some(Vec::new());
                    self.form.failed(f);
                }
            }
        }
        if let Some(outcome) = self.form.poll() {
            let action = self.action.take();
            if self.form.answered(&outcome) {
                match action {
                    Some(BlockAction::Block) => {
                        self.form.set_value(BLOCK_NAME, "");
                        self.load();
                        // Said the same whether or not the account exists: the list shows it.
                        self.form.status("Done.", Tone::Positive);
                    }
                    Some(BlockAction::Unblock(block)) => {
                        if let Some(list) = self.blocks.as_mut() {
                            list.retain(|b| *b != block);
                        }
                        self.form.status(&format!("Unblocked {}.", block.label), Tone::Positive);
                    }
                    None => {}
                }
            }
        }
        let mut s = Stack::default();
        s.text(
            "Blocked players can't send you friend requests or messages, and you won't see each other as Incari \
             players. They aren't told.",
            Tone::Muted,
        );
        s.username_field(BLOCK_NAME);
        let row = s.row(&["Block", "Report"]);
        s.submit(row);
        s.focus(BLOCK_NAME);
        let mut unblocks: Vec<(usize, Block)> = Vec::new();
        let mut turn = None;
        let mut pages = 1;
        match &self.blocks {
            None => s.centred("Loading...", Tone::Muted),
            Some(list) if list.is_empty() => s.centred("You haven't blocked anyone.", Tone::Muted),
            Some(list) => {
                pages = list.len().div_ceil(PAGE);
                self.page = self.page.min(pages - 1);
                let heading =
                    if pages > 1 { format!("Blocked ({} of {pages})", self.page + 1) } else { "Blocked".to_owned() };
                s.centred(heading, Tone::Text);
                for block in list.iter().skip(self.page * PAGE).take(PAGE) {
                    unblocks.push((s.entry(&block.label, &["Unblock"]), block.clone()));
                }
                if pages > 1 {
                    turn = Some(s.row(&["Previous", "Next"]));
                }
            }
        }
        s.status_line();
        let done = s.button(&done_label(cx));
        let Some(p) = self.form.show(cx, "Blocked Players", s).pressed else {
            return;
        };
        if p == done {
            cx.go(Go::Back);
        } else if p == row {
            self.block_by_name();
        } else if p == row + 1 {
            let name = self.form.value(BLOCK_NAME).to_owned();
            if rules::username_problem(&name).is_some() {
                self.form.problem("Type the Incari username to report.");
            } else {
                cx.open(Report::username(&name));
            }
        } else if let Some(t) = turn.filter(|t| p == *t || p == *t + 1) {
            let by = if p == t { pages - 1 } else { 1 };
            self.page = (self.page + by) % pages;
        } else if let Some((_, block)) = unblocks.into_iter().find(|(id, _)| *id == p)
            && !self.form.busy()
        {
            let t = online::with(|o| o.unblock(&block.id));
            self.form.sending(t, "Unblocking...");
            self.action = Some(BlockAction::Unblock(block));
        }
    }

    fn closed(&mut self, _cx: &mut Cx) {
        self.form.close();
        if let Some(t) = self.loading.take() {
            online::with(|o| o.forget(t));
        }
    }
}

impl BlockedPlayers {
    /// Fetches the block list, leaving the status line to whatever asked for it.
    fn load(&mut self) {
        if let Some(t) = self.loading.take() {
            online::with(|o| o.forget(t));
        }
        self.loading = Some(online::with(|o| o.blocks()));
    }

    fn block_by_name(&mut self) {
        if self.form.busy() {
            return;
        }
        let name = self.form.value(BLOCK_NAME).to_owned();
        if rules::username_problem(&name).is_some() {
            self.form.problem("Type an Incari username.");
            return;
        }
        let t = online::with(|o| o.block_username(&name));
        self.form.sending(t, "Blocking...");
        self.action = Some(BlockAction::Block);
    }
}

// -------------------------------------------------------------------------------------------------
// Report
// -------------------------------------------------------------------------------------------------

const NOTE: &str = "More about it (optional)";

/// Who a report is about: the Incari player here by pid, or an account by username.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Reported {
    Player(String),
    Username(String),
}

/// Report a player: a reason from the service's fixed list and an optional note. The service keeps
/// the report with the reported account's recent messages to the player.
pub struct Report {
    form: Form,
    who: String,
    reported: Reported,
    reason: usize,
    sent: bool,
}

impl Report {
    /// Reports the Incari player called `name` in this world.
    pub fn player(name: &str, pid: &str) -> Self {
        Self::new(name, Reported::Player(pid.to_owned()))
    }

    /// Reports the account with this Incari username.
    pub fn username(username: &str) -> Self {
        Self::new(username, Reported::Username(username.to_owned()))
    }

    fn new(who: &str, reported: Reported) -> Self {
        Self { form: Form::default(), who: who.to_owned(), reported, reason: 0, sent: false }
    }
}

impl Screen for Report {
    fn draw(&mut self, cx: &mut Cx) {
        if let Some(outcome) = self.form.poll()
            && self.form.answered(&outcome)
        {
            self.sent = true;
        }
        let title = format!("Report {}", self.who);
        let mut s = Stack::default();
        if self.sent {
            s.text(format!("Report about {} sent. Block them to stop hearing from them.", self.who), Tone::Positive);
            let done = s.button(&done_label(cx));
            s.submit(done);
            if self.form.show(cx, &title, s).pressed == Some(done) {
                cx.go(Go::Back);
            }
            return;
        }
        let cancel = cancel_label(cx);
        s.centred("What's wrong?", Tone::Muted);
        let reason = s.choice(ReportReason::ALL[self.reason].label());
        s.field(NOTE, online::service::MAX_TEXT);
        s.status_line();
        let row = s.row(&[&cancel, "Send Report"]);
        s.submit(row + 1);
        let shown = self.form.show(cx, &title, s);
        match shown.pressed {
            Some(p) if p == reason => {
                let n = ReportReason::ALL.len();
                self.reason = if shown.back { (self.reason + n - 1) % n } else { (self.reason + 1) % n };
            }
            Some(p) if p == row => cx.go(Go::Back),
            Some(p) if p == row + 1 && !self.form.busy() => {
                let (reason, note) = (ReportReason::ALL[self.reason], self.form.value(NOTE).to_owned());
                let t = online::with(|o| match &self.reported {
                    Reported::Player(pid) => o.report_player(pid, reason, &note),
                    Reported::Username(name) => o.report_username(name, reason, &note),
                });
                self.form.sending(t, "Sending...");
            }
            _ => {}
        }
    }

    fn closed(&mut self, _cx: &mut Cx) {
        self.form.close();
    }
}
