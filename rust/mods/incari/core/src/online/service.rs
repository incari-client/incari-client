//! The client's side of Incari's online service (ADR 0058; OnlineService.java) as a state machine
//! with no I/O: the game side hands it what the connection did (`opened`, `received`, `closed`)
//! and the clock (`tick`), and carries out what it asks (`take_effects`: open or close the
//! WebSocket, send a text frame, save or delete the session). The wire format is
//! `docs/online-protocol.md`.
//!
//! Nothing waits. A request returns a `Ticket` at once; its outcome is taken later
//! (`take_result`), when the answer has come, the request timed out, or the connection dropped
//! (`Failure::Offline`). Without a service the game carries on as if accounts didn't exist.
//!
//! The window's thread only.
use super::rules;
use super::social::{Delivery, Friend, Message, MessagesFrom, Notice, Peer, Request, Social};
use super::world::{IncariPlayer, Uuid, WorldPlayers};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};

/// The protocol version this client speaks.
pub const PROTOCOL: u32 = 1;
/// How long a request may wait for its answer before it fails as offline.
pub const REQUEST_TIMEOUT_MILLIS: f64 = 20_000.0;
/// How long opening the connection and saying hello may take before it's tried again.
pub const OPEN_TIMEOUT_MILLIS: f64 = 15_000.0;
/// The service's limit on a message; the client never sends one larger.
pub const MAX_MESSAGE: usize = 16 * 1024;
/// The longest message text the service takes.
pub const MAX_TEXT: usize = 500;
/// Nonces tried a tick by default: a few milliseconds' work on the reference Chromebook.
pub const POW_STEP: u32 = 4096;
/// How long the service keeps the player's place after they leave a world: a proxy's server
/// switch rejoins within it, so it isn't a new server (the service allows about 30 an hour).
pub const PRESENCE_GRACE_MILLIS: f64 = 10_000.0;
/// The first wait before a presence the service turned away goes again; doubling up to the max.
pub const PRESENCE_RETRY_MILLIS: f64 = 60_000.0;
pub const PRESENCE_RETRY_MAX_MILLIS: f64 = 15.0 * 60_000.0;
/// At least this long between two emotes sent; the service limits them too.
pub const EMOTE_INTERVAL_MILLIS: f64 = 2_000.0;

/// Where the client stands with the service (`Online.Status`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// This build has no service (no URL).
    Off,
    /// Opening the connection, or waiting to try again.
    Connecting,
    /// Connected, not signed in.
    SignedOut,
    /// Connected and signed in.
    SignedIn,
    /// The service no longer speaks this protocol; nothing more is tried.
    UpdateNeeded,
}

/// Why a request didn't work, with what the screens say about it. Deliberately general (ADR
/// 0058): a failed sign-in never says whether the username exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    WrongCredentials,
    WrongPassword,
    WrongCode,
    Taken,
    Rejected,
    RateLimited,
    SignedOut,
    Offline,
    Closed,
    Update,
    Unavailable,
}

impl Failure {
    /// What to tell the player.
    pub fn message(self) -> &'static str {
        match self {
            Failure::WrongCredentials => "Wrong username or password.",
            Failure::WrongPassword => "That isn't your password.",
            Failure::WrongCode => "Wrong username or recovery code.",
            Failure::Taken => "That username is taken.",
            Failure::Rejected => "That wasn't accepted. Check what you typed.",
            Failure::RateLimited => "Too many tries. Wait a few minutes and try again.",
            Failure::SignedOut => "You were signed out. Sign in again.",
            Failure::Offline => "Can't reach Incari. Try again later.",
            Failure::Closed => "They aren't taking messages from you.",
            Failure::Update => "Update Incari Client to sign in.",
            Failure::Unavailable => "Incari is having trouble. Try again later.",
        }
    }
}

/// A blocked account: an id to unblock it by, and what to call it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub id: String,
    pub label: String,
}

/// What a request gives back when it works.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    Done,
    /// Ten recovery codes, in their written form: shown once.
    Codes(Vec<String>),
    Blocks(Vec<Block>),
}

pub type Outcome = Result<Answer, Failure>;

/// A request's handle: its outcome is taken by it once it's in.
pub type Ticket = u32;

/// The reasons a report may give: the service takes these and no others.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportReason {
    Harassment,
    InappropriateName,
    Spam,
    Impersonation,
    Other,
}

impl ReportReason {
    pub const ALL: [ReportReason; 5] = [
        ReportReason::Harassment,
        ReportReason::InappropriateName,
        ReportReason::Spam,
        ReportReason::Impersonation,
        ReportReason::Other,
    ];

    pub fn wire(self) -> &'static str {
        match self {
            ReportReason::Harassment => "harassment",
            ReportReason::InappropriateName => "inappropriate_name",
            ReportReason::Spam => "spam",
            ReportReason::Impersonation => "impersonation",
            ReportReason::Other => "other",
        }
    }

    /// What the screens call it.
    pub fn label(self) -> &'static str {
        match self {
            ReportReason::Harassment => "Harassment or bullying",
            ReportReason::InappropriateName => "Inappropriate name",
            ReportReason::Spam => "Spam or scams",
            ReportReason::Impersonation => "Pretending to be someone",
            ReportReason::Other => "Something else",
        }
    }
}

/// A server pinned atop the server list, marked Sponsored (ADR 0062).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Featured {
    pub name: String,
    pub address: String,
}

/// An Incari player in the world played an emote (ADR 0061).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayedEmote {
    pub player: Uuid,
    pub emote: String,
}

/// What the game side does for the service. `connection` names the socket: events from an older
/// one are dropped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Open a WebSocket to `url`; report `opened`, then `received` frames, then `closed`.
    Connect { connection: u32, url: String },
    /// Send one text frame on the socket.
    Send { connection: u32, text: String },
    /// Close the socket.
    Close { connection: u32 },
    /// Write `incari/online/session.json` (the token, never the password).
    SaveSession(String),
    /// Delete `incari/online/session.json`.
    DeleteSession,
}

/// The saved session: the token, and the username to show while it resumes.
#[derive(Clone, PartialEq, Eq)]
struct Saved {
    username: String,
    token: String,
}

impl Saved {
    fn read(text: &str) -> Option<Self> {
        let json: Value = serde_json::from_str(text).ok()?;
        let username = json
            .get("username")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let token = json
            .get("token")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        (!token.is_empty()).then_some(Self { username, token })
    }
}

/// How long to wait before trying again (Backoff.java): 1 s doubling to 5 minutes, each wait
/// between half and all of that; a connection that lasts a minute starts over.
struct Backoff {
    failures: u32,
    connected_at: Option<f64>,
    seed: u64,
}

impl Backoff {
    const FIRST: f64 = 1_000.0;
    const MAX: f64 = 5.0 * 60_000.0;
    const STEADY: f64 = 60_000.0;

    fn random(&mut self) -> f64 {
        // xorshift64*: jitter only.
        self.seed ^= self.seed >> 12;
        self.seed ^= self.seed << 25;
        self.seed ^= self.seed >> 27;
        (self.seed.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
    }

    fn failed(&mut self, now: f64) -> f64 {
        if self.connected_at.is_some_and(|at| now - at >= Self::STEADY) {
            self.failures = 0;
        }
        self.connected_at = None;
        let ceiling = if self.failures >= 20 {
            Self::MAX
        } else {
            Self::MAX.min(Self::FIRST * f64::from(1u32 << self.failures))
        };
        self.failures += 1;
        let half = (ceiling / 2.0).floor();
        half + (self.random() * (ceiling - half)).floor()
    }
}

type Callback = Box<dyn FnOnce(&mut Online, &Value)>;

/// A request waiting for its answer. `signs_in` marks those that sign the connection in: if one
/// times out, the service may have signed in without the client knowing.
struct Pending {
    deadline: f64,
    answer: Callback,
    signs_in: bool,
}

/// A sign-up's proof of work, solved a step a tick.
struct Solving {
    run: u32,
    work: rules::ProofOfWork,
    challenge: String,
    username: String,
    password: String,
    ticket: Ticket,
}

pub struct Online {
    url: String,
    build: String,
    status: Status,
    started: bool,
    /// The socket asked for, by its number; None when there's none.
    link: Option<u32>,
    generation: u32,
    greeted: bool,
    open_deadline: Option<f64>,
    reconnect_at: Option<f64>,
    next_id: u64,
    pending: BTreeMap<u64, Pending>,
    backoff: Backoff,
    saved: Option<Saved>,
    account_name: Option<String>,
    featured: Option<Vec<Featured>>,
    latest_release: Option<String>,
    release_notes: Vec<String>,
    share_server: bool,
    badge: bool,
    revision: u64,
    /// Counts sign-ups and local sign-outs: an answer that finds it changed is for something the
    /// player cancelled.
    sign_up_run: u32,
    /// The sign-up's form is out: from then on it can't be cancelled.
    sign_up_sent: bool,
    solving: Option<Solving>,
    /// Nonces tried a tick.
    pub pow_tries_per_tick: u32,
    presence_held: String,
    presence_out: bool,
    presence_retry_at: f64,
    presence_retry_delay: f64,
    presence_grace_until: Option<f64>,
    world: WorldPlayers,
    social: Social,
    viewing: Option<Peer>,
    local_ids: u32,
    wardrobe: Option<(Vec<String>, Vec<String>)>,
    /// Counts the account's wardrobes that arrived (an answer to `wardrobe`, or its event).
    wardrobe_serial: u64,
    emotes: Vec<PlayedEmote>,
    next_emote: f64,
    now: f64,
    epoch: i64,
    effects: Vec<Effect>,
    warnings: Vec<String>,
    notices: Vec<Notice>,
    next_ticket: Ticket,
    results: HashMap<Ticket, Outcome>,
    abandoned: HashSet<Ticket>,
    keeping_codes: HashSet<Ticket>,
    unsaved_codes: Option<Vec<String>>,
}

impl Online {
    /// `url`: the service's `wss://` URL, empty for a build without one (`Status::Off`). `build`:
    /// `incari/<game version>`, for `hello`. `session`: `session.json`'s text, if there is one.
    /// `seed`: any number, for the back-off's jitter.
    pub fn new(url: &str, build: &str, session: Option<&str>, seed: u64) -> Self {
        let url = url.trim().to_owned();
        let off = url.is_empty();
        let saved = if off {
            None
        } else {
            session.and_then(Saved::read)
        };
        Self {
            url,
            build: build.to_owned(),
            status: if off { Status::Off } else { Status::Connecting },
            started: false,
            link: None,
            generation: 0,
            greeted: false,
            open_deadline: None,
            reconnect_at: None,
            next_id: 1,
            pending: BTreeMap::new(),
            backoff: Backoff {
                failures: 0,
                connected_at: None,
                seed: seed | 1,
            },
            account_name: saved.as_ref().map(|s| s.username.clone()),
            saved,
            featured: None,
            latest_release: None,
            release_notes: Vec::new(),
            share_server: true,
            badge: true,
            revision: 0,
            sign_up_run: 0,
            sign_up_sent: false,
            solving: None,
            pow_tries_per_tick: POW_STEP,
            presence_held: String::new(),
            presence_out: false,
            presence_retry_at: 0.0,
            presence_retry_delay: PRESENCE_RETRY_MILLIS,
            presence_grace_until: None,
            world: WorldPlayers::default(),
            social: Social::default(),
            viewing: None,
            local_ids: 0,
            wardrobe: None,
            wardrobe_serial: 0,
            emotes: Vec::new(),
            next_emote: 0.0,
            now: 0.0,
            epoch: 0,
            effects: Vec::new(),
            warnings: Vec::new(),
            notices: Vec::new(),
            next_ticket: 1,
            results: HashMap::new(),
            abandoned: HashSet::new(),
            keeping_codes: HashSet::new(),
            unsaved_codes: None,
        }
    }

    // ---------------------------------------------------------------------------------------------
    // What the game side calls
    // ---------------------------------------------------------------------------------------------

    /// Opens the connection, once: after the title screen shows, never while the game loads.
    pub fn start(&mut self) {
        if self.started || self.status == Status::Off {
            return;
        }
        self.started = true;
        self.connect();
    }

    pub fn started(&self) -> bool {
        self.started
    }

    /// Once a frame or tick: `now` steadily rising ms, `epoch` ms since 1970 (for message times).
    /// Retries, timeouts, a step of proof of work, presence and lookups.
    pub fn tick(&mut self, now: f64, epoch: i64) {
        self.now = now;
        self.epoch = epoch;
        if !self.started || matches!(self.status, Status::Off | Status::UpdateNeeded) {
            return;
        }
        if self.link.is_none() && self.reconnect_at.is_some_and(|at| now >= at) {
            self.connect();
        }
        if self.link.is_some() && !self.greeted && self.open_deadline.is_some_and(|at| now >= at) {
            self.warn("The Incari service didn't answer in time; trying again later");
            self.drop_connection(None);
        }
        if !self.pending.is_empty() {
            self.expire_requests(now);
        }
        self.step_proof_of_work();
        if self.status == Status::SignedIn {
            self.sync_presence();
            let batch = self.world.take_lookup(now);
            if !batch.is_empty() {
                self.lookup(batch);
            }
        }
    }

    /// The socket `connection` opened.
    pub fn opened(&mut self, connection: u32) {
        if self.current(connection) {
            self.say_hello();
        }
    }

    /// A text frame arrived on `connection` (a binary one, as its UTF-8).
    pub fn received(&mut self, connection: u32, text: &str) {
        if self.current(connection) {
            self.receive(text);
        }
    }

    /// The socket `connection` closed, or never opened (`why` for the log).
    pub fn closed(&mut self, connection: u32, why: &str) {
        if self.current(connection) {
            self.drop_connection(Some(why));
        }
    }

    fn current(&self, connection: u32) -> bool {
        self.link == Some(connection)
    }

    /// What to do: take them after each call and do them in order.
    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    /// Problems for the client's log; never a password, token or code.
    pub fn take_warnings(&mut self) -> Vec<String> {
        std::mem::take(&mut self.warnings)
    }

    /// Tries again now instead of waiting out the back-off: the account screen's Try Again.
    pub fn retry(&mut self) {
        if self.started && self.link.is_none() && self.status == Status::Connecting {
            self.backoff.failures = 0;
            self.connect();
        }
    }

    /// Joined a server at `address` as `name` (never singleplayer or the player's own LAN world).
    pub fn joined(&mut self, address: &str, name: &str) {
        self.world.join(address, name);
        self.presence_grace_until = None;
        self.revision += 1;
        self.sync_presence();
    }

    /// Left the world: the service keeps the place for `PRESENCE_GRACE_MILLIS`.
    pub fn left(&mut self) {
        let was_in = self.world.place().is_some();
        self.world.leave();
        self.social.left_server();
        self.revision += 1;
        if was_in {
            self.presence_grace_until = Some(self.now + PRESENCE_GRACE_MILLIS);
        }
    }

    /// The tab list's players by UUID, about once a second while in a world.
    pub fn tab_list(&mut self, players: &HashMap<Uuid, String>) {
        self.world.tab_list(players);
    }

    // ---------------------------------------------------------------------------------------------
    // What screens and features read
    // ---------------------------------------------------------------------------------------------

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn signed_in(&self) -> bool {
        self.status == Status::SignedIn
    }

    /// The account's username; while reconnecting, the one last signed in here.
    pub fn account_name(&self) -> Option<&str> {
        self.account_name.as_deref()
    }

    /// Whether a session is saved here: the account screen says "Signing in" while it resumes.
    pub fn has_saved_session(&self) -> bool {
        self.account_name.is_some()
    }

    /// The Incari player with this UUID in the world, while signed in.
    pub fn player(&self, id: Uuid) -> Option<&IncariPlayer> {
        if self.signed_in() {
            self.world.player(id)
        } else {
            None
        }
    }

    /// The Incari players in the world by their name there.
    pub fn world_players(&self) -> &HashMap<String, IncariPlayer> {
        self.world.players()
    }

    /// Whether the service hears where the player is.
    pub fn in_world(&self) -> bool {
        self.world.place().is_some()
    }

    pub fn shares_server(&self) -> bool {
        self.share_server
    }

    pub fn shows_badge(&self) -> bool {
        self.badge
    }

    /// Goes up whenever anything here changes: screens compare it to refresh.
    pub fn revision(&self) -> u64 {
        self.revision + self.world.revision() + self.social.revision()
    }

    /// The latest release the service named in its last `hello`.
    pub fn latest_release(&self) -> Option<&str> {
        self.latest_release.as_deref()
    }

    /// That release's changelog, one change a line.
    pub fn release_notes(&self) -> &[String] {
        &self.release_notes
    }

    /// The featured servers the last `hello` named, which replace the built-in ones; None if none.
    pub fn featured_servers(&self) -> Option<&[Featured]> {
        self.featured.as_deref()
    }

    /// Whether the service answered `hello` on the open connection.
    pub fn greeted(&self) -> bool {
        self.link.is_some() && self.greeted
    }

    pub fn social(&self) -> &Social {
        &self.social
    }

    /// What arrived while the chat menu was closed (messages, requests, an account made after its
    /// form closed), for toasts.
    pub fn take_notices(&mut self) -> Vec<Notice> {
        let mut all = std::mem::take(&mut self.notices);
        all.extend(self.social.take_notices());
        all
    }

    /// A request's outcome, once it's in; taken.
    pub fn take_result(&mut self, ticket: Ticket) -> Option<Outcome> {
        self.results.remove(&ticket)
    }

    /// Nobody waits for `ticket` any more (its screen closed): its outcome is dropped.
    pub fn forget(&mut self, ticket: Ticket) {
        if self.results.remove(&ticket).is_none() {
            self.abandoned.insert(ticket);
        }
    }

    /// A sign-up whose screen closed once its form was out: the account may be made all the
    /// same, so its codes wait for the account screen (`take_unsaved_codes`) and a toast says so.
    pub fn keep_codes(&mut self, ticket: Ticket) {
        match self.results.remove(&ticket) {
            Some(outcome) => self.keep(outcome),
            None => {
                self.keeping_codes.insert(ticket);
            }
        }
    }

    fn keep(&mut self, outcome: Outcome) {
        if let Ok(Answer::Codes(codes)) = outcome {
            self.unsaved_codes = Some(codes);
            self.notices.push(Notice {
                who: "Incari".into(),
                request: false,
                title: "Account created".into(),
                text: "Open Incari Account to save your recovery codes.".into(),
            });
        }
    }

    /// A new account's codes not shown yet, taken.
    pub fn take_unsaved_codes(&mut self) -> Option<Vec<String>> {
        self.unsaved_codes.take()
    }

    fn ticket(&mut self) -> Ticket {
        let t = self.next_ticket;
        self.next_ticket = self.next_ticket.wrapping_add(1).max(1);
        t
    }

    fn finish(&mut self, ticket: Ticket, outcome: Outcome) {
        if self.keeping_codes.remove(&ticket) {
            self.keep(outcome);
            return;
        }
        if self.abandoned.remove(&ticket) {
            return;
        }
        self.results.insert(ticket, outcome);
    }

    fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }

    // ---------------------------------------------------------------------------------------------
    // Accounts
    // ---------------------------------------------------------------------------------------------

    /// Creates an account and signs in: asks for a challenge, solves it a step a tick, then sends
    /// the form. Answers with the ten recovery codes.
    pub fn sign_up(&mut self, username: &str, password: &str) -> Ticket {
        let t = self.ticket();
        if !self.signed_out_and_connected(t) {
            return t;
        }
        self.sign_up_run += 1;
        let run = self.sign_up_run;
        self.sign_up_sent = false;
        let (username, password) = (username.to_owned(), password.to_owned());
        let form = self.request("challenge");
        self.send(form, false, move |o, answer| {
            if run != o.sign_up_run {
                // Cancelled before anything was made: nobody is waiting any more.
                return;
            }
            if let Some(f) = o.failure(answer, Failure::WrongCredentials) {
                o.finish(t, Err(f));
                return;
            }
            let text = answer
                .get("challenge")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let bits = answer
                .get("bits")
                .and_then(Value::as_u64)
                .unwrap_or(u64::MAX);
            let work = rules::base64_url(&text)
                .filter(|c| c.len() == 16 && bits <= 40)
                .and_then(|c| rules::ProofOfWork::new(&c, bits as u32));
            let Some(work) = work else {
                o.warn("The Incari service sent a challenge this client can't use");
                o.finish(t, Err(Failure::Unavailable));
                return;
            };
            o.solving = Some(Solving {
                run,
                work,
                challenge: text,
                username,
                password,
                ticket: t,
            });
        });
        t
    }

    fn step_proof_of_work(&mut self) {
        let Some(mut s) = self.solving.take() else {
            return;
        };
        if s.run != self.sign_up_run {
            return;
        }
        match s.work.step(self.pow_tries_per_tick) {
            Some(nonce) => self.solved(s, nonce),
            None if s.work.exhausted() => {
                self.warn("Sign-up's proof of work failed");
                self.finish(s.ticket, Err(Failure::Unavailable));
            }
            None => self.solving = Some(s),
        }
    }

    fn solved(&mut self, s: Solving, nonce: u64) {
        let mut form = self.request("signup");
        form.insert("username".into(), json!(s.username));
        form.insert("password".into(), json!(s.password));
        form.insert("challenge".into(), json!(s.challenge));
        form.insert("nonce".into(), json!(nonce));
        // Can't be cancelled from here: the account may exist before the answer comes, and the
        // player must get its recovery codes.
        self.sign_up_sent = true;
        let (t, username) = (s.ticket, s.username);
        self.send(form, true, move |o, answer| {
            o.sign_up_sent = false;
            if let Some(f) = o.failure(answer, Failure::Rejected) {
                o.finish(t, Err(f));
                return;
            }
            let codes = codes(answer);
            if !o.signed_in_with(answer, &username) && codes.is_empty() {
                o.finish(t, Err(Failure::Unavailable));
                return;
            }
            // A malformed answer doesn't sign in, but the account exists: the player still gets
            // its codes and can sign in with the password.
            o.finish(t, Ok(Answer::Codes(codes)));
        });
    }

    /// Cancels the sign-up if its form isn't out yet: no account is made and no outcome comes.
    /// False once the form is out, as the account may exist.
    pub fn cancel_sign_up(&mut self) -> bool {
        if self.sign_up_sent {
            return false;
        }
        self.sign_up_run += 1;
        self.solving = None;
        true
    }

    /// Signs in. A wrong username and a wrong password fail the same way, on purpose.
    pub fn sign_in(&mut self, username: &str, password: &str) -> Ticket {
        let t = self.ticket();
        if !self.signed_out_and_connected(t) {
            return t;
        }
        let mut form = self.request("signin");
        form.insert("username".into(), json!(username));
        form.insert("password".into(), json!(password));
        let username = username.to_owned();
        self.send(form, true, move |o, answer| {
            let failure = o.failure(answer, Failure::WrongCredentials).map(|f| {
                // A limited sign-in reads as a wrong one: the service means it to.
                if f == Failure::RateLimited {
                    Failure::WrongCredentials
                } else {
                    f
                }
            });
            let outcome = match failure {
                Some(f) => Err(f),
                None if o.signed_in_with(answer, &username) => Ok(Answer::Done),
                None => Err(Failure::Unavailable),
            };
            o.finish(t, outcome);
        });
        t
    }

    /// Sets a new password with a recovery code (used up), and signs in. Every other session ends.
    pub fn recover(&mut self, username: &str, code: &str, new_password: &str) -> Ticket {
        let t = self.ticket();
        if !self.signed_out_and_connected(t) {
            return t;
        }
        let Some(normal) = rules::normalize_code(code) else {
            self.finish(t, Err(Failure::WrongCode));
            return t;
        };
        let mut form = self.request("recover");
        form.insert("username".into(), json!(username));
        form.insert("code".into(), json!(normal));
        form.insert("newPassword".into(), json!(new_password));
        let username = username.to_owned();
        self.send(form, true, move |o, answer| {
            let outcome = match o.failure(answer, Failure::WrongCode) {
                Some(f) => Err(f),
                None if o.signed_in_with(answer, &username) => Ok(Answer::Done),
                None => Err(Failure::Unavailable),
            };
            o.finish(t, outcome);
        });
        t
    }

    /// Changes the password. Every other session ends; this one stays.
    pub fn change_password(&mut self, old: &str, new: &str) -> Ticket {
        let mut form = self.request("password");
        form.insert("old".into(), json!(old));
        form.insert("new".into(), json!(new));
        self.simple(form, Failure::WrongPassword, |_| {})
    }

    /// Ten new recovery codes; the old ten stop working.
    pub fn new_recovery_codes(&mut self, password: &str) -> Ticket {
        let t = self.ticket();
        if !self.signed_in_and_connected(t) {
            return t;
        }
        let mut form = self.request("codes");
        form.insert("password".into(), json!(password));
        self.send(form, false, move |o, answer| {
            let outcome = match o.failure(answer, Failure::WrongPassword) {
                Some(f) => Err(f),
                None => match codes(answer) {
                    codes if codes.is_empty() => Err(Failure::Unavailable),
                    codes => Ok(Answer::Codes(codes)),
                },
            };
            o.finish(t, outcome);
        });
        t
    }

    /// Signs out here, at once and whatever the service says. With `everywhere`, the service also
    /// ends every other session; that needs the connection.
    pub fn sign_out(&mut self, everywhere: bool) -> Ticket {
        let t = self.ticket();
        if everywhere && !self.signed_in_and_connected(t) {
            return t;
        }
        if self.status == Status::SignedIn && self.greeted() {
            let mut form = self.request("signout");
            form.insert("everywhere".into(), json!(everywhere));
            self.send(form, false, move |o, answer| {
                let failure = o.failure(answer, Failure::Unavailable);
                o.signed_out_here();
                o.finish(t, failure.map_or(Ok(Answer::Done), Err));
            });
        } else {
            // Offline: the token is forgotten here; the service lets it lapse unused.
            self.signed_out_here();
            self.finish(t, Ok(Answer::Done));
        }
        t
    }

    /// Deletes the account and everything tied to it, then signs out.
    pub fn delete_account(&mut self, password: &str) -> Ticket {
        let mut form = self.request("delete");
        form.insert("password".into(), json!(password));
        self.simple(form, Failure::WrongPassword, |o| o.signed_out_here())
    }

    /// Whether friends see which server the player is on; kept by the service.
    pub fn set_share_server(&mut self, share: bool) -> Ticket {
        let mut form = self.request("settings");
        form.insert("shareServer".into(), json!(share));
        self.simple(form, Failure::Rejected, move |o| {
            o.share_server = share;
            o.revision += 1;
            // The address goes, or comes, with the next presence.
            o.sync_presence();
        })
    }

    /// Shows or hides the player's Incari badge to others (ADR 0062); kept by the service.
    pub fn set_show_badge(&mut self, show: bool) -> Ticket {
        let mut form = self.request("settings");
        form.insert("badge".into(), json!(show));
        self.simple(form, Failure::Rejected, move |o| {
            o.badge = show;
            o.revision += 1;
        })
    }

    /// Tells the service this install ran today (ADR 0062's usage counts); fails offline without
    /// trying. The caller sends it at most once a UTC day.
    pub fn count(&mut self, install: &str, version: &str, kind: &str) -> Ticket {
        let t = self.ticket();
        if !self.greeted() {
            self.finish(t, Err(Failure::Offline));
            return t;
        }
        let mut form = self.request("count");
        form.insert("install".into(), json!(install));
        form.insert("version".into(), json!(version));
        form.insert("kind".into(), json!(kind));
        self.send(form, false, move |o, answer| {
            let f = o.failure(answer, Failure::Rejected);
            o.finish(t, f.map_or(Ok(Answer::Done), Err));
        });
        t
    }

    /// A signed-in request that gives nothing back: `then` runs when it worked.
    fn simple(
        &mut self,
        form: Map<String, Value>,
        unauthorized: Failure,
        then: impl FnOnce(&mut Online) + 'static,
    ) -> Ticket {
        let t = self.ticket();
        if !self.signed_in_and_connected(t) {
            return t;
        }
        self.send(form, false, move |o, answer| {
            let outcome = match o.failure(answer, unauthorized) {
                Some(f) => Err(f),
                None => {
                    then(o);
                    Ok(Answer::Done)
                }
            };
            o.finish(t, outcome);
        });
        t
    }

    // ---------------------------------------------------------------------------------------------
    // Blocking and reporting
    // ---------------------------------------------------------------------------------------------

    /// Blocks the Incari player called `name` here, by pid. The service keeps `name` as the block
    /// list's label: it never tells the player the account's username.
    pub fn block_player(&mut self, name: &str, pid: &str) -> Ticket {
        let mut form = self.request("block");
        form.insert("pid".into(), json!(pid));
        form.insert("name".into(), json!(name));
        let pid = pid.to_owned();
        self.simple(form, Failure::Rejected, move |o| o.world.gone(&pid))
    }

    /// Blocks the account with this Incari username.
    pub fn block_username(&mut self, username: &str) -> Ticket {
        let mut form = self.request("block");
        form.insert("username".into(), json!(username));
        self.simple(form, Failure::Rejected, |_| {})
    }

    /// Blocks a friend by their username; the service ends the friendship with it (ADR 0059).
    pub fn block_friend(&mut self, fid: &str, username: &str) -> Ticket {
        let mut form = self.request("block");
        form.insert("username".into(), json!(username));
        let fid = fid.to_owned();
        self.simple(form, Failure::Rejected, move |o| o.social.unfriended(&fid))
    }

    /// Unblocks an account from the block list.
    pub fn unblock(&mut self, block_id: &str) -> Ticket {
        let mut form = self.request("unblock");
        form.insert("bid".into(), json!(block_id));
        // Whoever it was may be an Incari player here again.
        self.simple(form, Failure::Rejected, |o| o.world.ask_all())
    }

    /// The accounts the player has blocked.
    pub fn blocks(&mut self) -> Ticket {
        let t = self.ticket();
        if !self.signed_in_and_connected(t) {
            return t;
        }
        let form = self.request("blocks");
        self.send(form, false, move |o, answer| {
            if let Some(f) = o.failure(answer, Failure::Unavailable) {
                o.finish(t, Err(f));
                return;
            }
            let blocks = answer
                .get("blocks")
                .and_then(Value::as_array)
                .and_then(|list| {
                    list.iter()
                        .map(|b| {
                            Some(Block {
                                id: b.get("bid")?.as_str()?.to_owned(),
                                label: b.get("label")?.as_str()?.to_owned(),
                            })
                        })
                        .collect::<Option<Vec<_>>>()
                });
            match blocks {
                Some(blocks) => o.finish(t, Ok(Answer::Blocks(blocks))),
                None => {
                    o.warn("The Incari service sent a block list this client can't read");
                    o.finish(t, Err(Failure::Unavailable));
                }
            }
        });
        t
    }

    /// Reports an Incari player here, by pid.
    pub fn report_player(&mut self, pid: &str, reason: ReportReason, note: &str) -> Ticket {
        let mut form = self.request("report");
        form.insert("pid".into(), json!(pid));
        self.report(form, reason, note)
    }

    /// Reports the account with this Incari username.
    pub fn report_username(&mut self, username: &str, reason: ReportReason, note: &str) -> Ticket {
        let mut form = self.request("report");
        form.insert("username".into(), json!(username));
        self.report(form, reason, note)
    }

    fn report(&mut self, mut form: Map<String, Value>, reason: ReportReason, note: &str) -> Ticket {
        form.insert("reason".into(), json!(reason.wire()));
        form.insert("note".into(), json!(cut(note, MAX_TEXT)));
        self.simple(form, Failure::Rejected, |_| {})
    }

    // ---------------------------------------------------------------------------------------------
    // Cosmetics (ADR 0061): what the account owns and wears, for the Wardrobe's port
    // ---------------------------------------------------------------------------------------------

    /// The account's wardrobe as the service last said: (owned, equipped). None before it answered,
    /// or from a service without cosmetics.
    pub fn wardrobe(&self) -> Option<&(Vec<String>, Vec<String>)> {
        self.wardrobe.as_ref()
    }

    /// Goes up each time the account's wardrobe arrives (the answer to `wardrobe` after a sign-in
    /// or reconnect, or a `wardrobe` event), not when an `equip` is answered: the device's
    /// wardrobe (`cosmetics::wardrobe`) takes the account's then.
    pub fn wardrobe_serial(&self) -> u64 {
        self.wardrobe_serial
    }

    /// Replaces what the account wears. The Wardrobe sends at most one a second after changes stop.
    pub fn equip(&mut self, ids: &[String]) -> Ticket {
        let t = self.ticket();
        if !self.signed_in_and_connected(t) {
            return t;
        }
        let mut form = self.request("equip");
        form.insert("cosmetics".into(), json!(ids));
        self.send(form, false, move |o, answer| {
            match error(answer).as_deref() {
                None => {
                    let equipped = strings(answer, "equipped");
                    if let Some(w) = o.wardrobe.as_mut() {
                        w.1 = equipped;
                    }
                    o.revision += 1;
                    o.finish(t, Ok(Answer::Done));
                }
                Some("not_owned" | "bad_request") => {
                    // This client and the service disagree on what the account owns: take the service's.
                    o.warn("The Incari service turned down what's worn");
                    o.load_wardrobe();
                    o.finish(t, Err(Failure::Rejected));
                }
                Some(_) => {
                    let f = o
                        .failure(answer, Failure::Rejected)
                        .unwrap_or(Failure::Unavailable);
                    o.finish(t, Err(f));
                }
            }
        });
        t
    }

    /// The player plays emote `id` (`emote.<name>`): Incari players on the same server hear of it.
    /// False if it can't go yet (one every `EMOTE_INTERVAL_MILLIS`) or isn't an owned emote.
    pub fn play_emote(&mut self, id: &str) -> bool {
        let owned = self
            .wardrobe
            .as_ref()
            .is_none_or(|w| w.0.iter().any(|o| o == id));
        if !id.starts_with("emote.") || !owned || self.now < self.next_emote {
            return false;
        }
        self.next_emote = self.now + EMOTE_INTERVAL_MILLIS;
        if self.status == Status::SignedIn && self.greeted() && self.world.place().is_some() {
            let mut form = self.request("emote");
            form.insert("emote".into(), json!(id));
            self.send(form, false, |o, answer| {
                if let Some(e) = error(answer)
                    && e != "offline"
                    && e != "rate_limited"
                {
                    o.warn(format!("The Incari service didn't take an emote ({e})"));
                }
            });
        }
        true
    }

    /// The emotes Incari players in the world played since the last call, oldest first.
    pub fn take_emotes(&mut self) -> Vec<PlayedEmote> {
        std::mem::take(&mut self.emotes)
    }

    fn load_wardrobe(&mut self) {
        let form = self.request("wardrobe");
        self.send(form, false, |o, answer| {
            if let Some(e) = error(answer) {
                // "bad_request": a service without cosmetics yet; what's worn stays this device's.
                if e != "bad_request" && e != "offline" {
                    o.warn(format!(
                        "The Incari service couldn't send the wardrobe ({e})"
                    ));
                }
                return;
            }
            o.wardrobe = Some((strings(answer, "owned"), strings(answer, "equipped")));
            o.wardrobe_serial += 1;
            o.revision += 1;
        });
    }

    // ---------------------------------------------------------------------------------------------
    // Friends and messages (ADR 0059)
    // ---------------------------------------------------------------------------------------------

    /// The conversation the chat menu shows, or None: its messages arrive read and raise no toast.
    /// Setting one marks it read.
    pub fn set_viewing(&mut self, peer: Option<&Peer>) {
        self.viewing = peer.cloned();
        if let Some(peer) = peer {
            self.mark_read(peer);
        }
    }

    /// The conversation the chat menu shows.
    pub fn viewing(&self) -> Option<&Peer> {
        self.viewing.as_ref()
    }

    /// Ms since 1970 as of the last tick, for "Today at 14:02".
    pub fn epoch(&self) -> i64 {
        self.epoch
    }

    /// Who may message the player; kept by the service.
    pub fn set_messages_from(&mut self, from: MessagesFrom) -> Ticket {
        let mut form = self.request("settings");
        form.insert("messages".into(), json!(from.wire()));
        self.simple(form, Failure::Rejected, move |o| {
            let requests = o.social.friend_requests();
            o.social.settings(from, requests);
        })
    }

    /// Whether others can send the player friend requests; kept by the service.
    pub fn set_friend_requests(&mut self, allowed: bool) -> Ticket {
        let mut form = self.request("settings");
        form.insert("friendRequests".into(), json!(allowed));
        self.simple(form, Failure::Rejected, move |o| {
            let from = o.social.messages_from();
            o.social.settings(from, allowed);
        })
    }

    /// A friend request to the Incari player called `name` on this server.
    pub fn befriend_player(&mut self, name: &str, pid: &str) -> Ticket {
        let mut form = self.request("befriend");
        form.insert("pid".into(), json!(pid));
        form.insert("name".into(), json!(name));
        self.send_request(form, Some(pid.to_owned()))
    }

    /// A friend request to the account with this Incari username.
    pub fn befriend(&mut self, username: &str) -> Ticket {
        let mut form = self.request("befriend");
        form.insert("username".into(), json!(username));
        self.send_request(form, None)
    }

    fn send_request(&mut self, form: Map<String, Value>, pid: Option<String>) -> Ticket {
        let t = self.ticket();
        if !self.signed_in_and_connected(t) {
            return t;
        }
        self.send(form, false, move |o, answer| {
            if let Some(f) = o.failure(answer, Failure::Rejected) {
                o.finish(t, Err(f));
                return;
            }
            if let Some(sent) = answer.get("request").and_then(request) {
                let rid = sent.rid.clone();
                o.social.request(sent);
                if let Some(pid) = &pid {
                    o.social.request_sent_to(&rid, pid);
                }
            }
            if let Some(friend) = answer.get("friend").and_then(friend) {
                // They had asked already: friends at once.
                o.social.friend_changed(friend, false);
            }
            o.finish(t, Ok(Answer::Done));
        });
        t
    }

    /// Accepts a friend request received.
    pub fn accept(&mut self, rid: &str) -> Ticket {
        self.answer_request("accept", rid)
    }

    /// Declines a request received, or withdraws one sent.
    pub fn decline(&mut self, rid: &str) -> Ticket {
        self.answer_request("decline", rid)
    }

    fn answer_request(&mut self, kind: &str, rid: &str) -> Ticket {
        let t = self.ticket();
        if !self.signed_in_and_connected(t) {
            return t;
        }
        let mut form = self.request(kind);
        form.insert("rid".into(), json!(rid));
        let rid = rid.to_owned();
        self.send(form, false, move |o, answer| {
            if let Some(f) = o.failure(answer, Failure::Rejected) {
                o.finish(t, Err(f));
                return;
            }
            o.social.request_gone(&rid);
            if let Some(friend) = answer.get("friend").and_then(friend) {
                o.social.friend_changed(friend, false);
            }
            o.finish(t, Ok(Answer::Done));
        });
        t
    }

    /// Ends a friendship; the conversation goes with it.
    pub fn unfriend(&mut self, fid: &str) -> Ticket {
        let mut form = self.request("unfriend");
        form.insert("fid".into(), json!(fid));
        let fid = fid.to_owned();
        self.simple(form, Failure::Rejected, move |o| o.social.unfriended(&fid))
    }

    /// Sends a message: it shows at once as sending, then as sent, or failed if the service won't
    /// deliver it.
    pub fn send_message(&mut self, peer: &Peer, text: &str) -> Ticket {
        let t = self.ticket();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            self.finish(t, Err(Failure::Rejected));
            return t;
        }
        let body = cut(trimmed, MAX_TEXT).to_owned();
        if !self.signed_in_and_connected(t) {
            return t;
        }
        self.local_ids += 1;
        let local = format!("local-{}", self.local_ids);
        let sending = Message {
            mid: local.clone(),
            mine: true,
            text: body.clone(),
            at: self.epoch,
            delivery: Delivery::Sending,
        };
        self.social.message(peer, sending, true);
        let mut form = self.request("send");
        form.insert(
            if peer.friend { "fid" } else { "pid" }.into(),
            json!(peer.id),
        );
        form.insert("text".into(), json!(body));
        let peer = peer.clone();
        self.send(form, false, move |o, answer| {
            match o.failure(answer, Failure::Rejected) {
                None => {
                    let mid = answer
                        .get("mid")
                        .and_then(Value::as_str)
                        .map_or(local.clone(), str::to_owned);
                    let at = answer.get("at").and_then(Value::as_i64).unwrap_or(o.epoch);
                    o.social.replace(
                        &peer,
                        &local,
                        Message {
                            mid,
                            mine: true,
                            text: body,
                            at,
                            delivery: Delivery::Sent,
                        },
                    );
                    o.finish(t, Ok(Answer::Done));
                }
                Some(f) => {
                    let failed = Message {
                        mid: local.clone(),
                        mine: true,
                        text: body,
                        at: o.epoch,
                        delivery: Delivery::Failed,
                    };
                    o.social.replace(&peer, &local, failed);
                    let closed = error(answer).as_deref() == Some("closed");
                    o.finish(t, Err(if closed { Failure::Closed } else { f }));
                }
            }
        });
        t
    }

    /// Loads the page before the oldest message held, for a friend's conversation: the first time
    /// it's shown and as the player scrolls back. After a reconnect (stale), the newest page.
    /// Players on this server have no history.
    pub fn load_older(&mut self, peer: &Peer) {
        let connected = self.status == Status::SignedIn && self.greeted();
        let conversation = self.social.conversation(peer);
        if !peer.friend || conversation.loading() {
            return;
        }
        let newest = conversation.stale();
        if (!newest && conversation.loaded() && !conversation.has_more()) || !connected {
            return;
        }
        let before = if newest {
            None
        } else {
            conversation
                .messages()
                .iter()
                .find(|m| m.delivery == Delivery::Sent)
                .map(|m| m.mid.clone())
        };
        conversation.start_loading();
        let mut form = self.request("history");
        form.insert("fid".into(), json!(peer.id));
        if let Some(before) = before {
            form.insert("before".into(), json!(before));
        }
        let peer = peer.clone();
        self.send(form, false, move |o, answer| {
            if error(answer).is_some() {
                o.social.load_failed(&peer);
                return;
            }
            let page = answer
                .get("messages")
                .and_then(Value::as_array)
                .and_then(|list| list.iter().map(message).collect::<Option<Vec<_>>>());
            let Some(mut page) = page else {
                o.warn("The Incari service sent a history this client can't read");
                o.social.load_failed(&peer);
                return;
            };
            page.sort_by_key(|m| m.at);
            let more = answer.get("more").and_then(Value::as_bool).unwrap_or(false);
            if newest {
                o.social.newest(&peer, page, more);
            } else {
                o.social.older(&peer, page, more);
            }
            if newest
                && o.viewing
                    .as_ref()
                    .is_some_and(|v| Social::same_peer(v, &peer))
            {
                // Read up to the newest now that it's here (mark_read held it back while stale).
                o.send_read(&peer);
            }
        });
    }

    fn mark_read(&mut self, peer: &Peer) {
        let (exists, unread, stale) = match self.social.existing(peer) {
            Some(c) => (true, c.unread() > 0, c.stale()),
            None => (false, false, false),
        };
        self.social.read(peer);
        if !exists || !unread || !peer.friend {
            return;
        }
        if stale {
            // What's held may stop short of the newest: the read goes once the newest page is in,
            // or the service would count unseen messages read.
            self.load_older(peer);
            return;
        }
        self.send_read(peer);
    }

    /// Tells the service the player read the conversation up to the newest message held.
    fn send_read(&mut self, peer: &Peer) {
        if self.status != Status::SignedIn || self.link.is_none() {
            return;
        }
        let last = self.social.existing(peer).and_then(|c| {
            c.messages()
                .iter()
                .rev()
                .find(|m| !m.mine)
                .map(|m| m.mid.clone())
        });
        let mut form = self.request("read");
        form.insert("fid".into(), json!(peer.id));
        if let Some(last) = last {
            form.insert("mid".into(), json!(last));
        }
        self.send(form, false, |_, _| {});
    }

    /// After signing in: the friend list, requests and unread counts.
    fn load_friends(&mut self) {
        let form = self.request("friends");
        self.send(form, false, |o, answer| {
            match error(answer).as_deref() {
                // A service that doesn't speak friends yet.
                Some("bad_request") => return o.social.set_available(false),
                Some(_) => return,
                None => {}
            }
            let list = answer
                .get("friends")
                .and_then(Value::as_array)
                .and_then(|list| {
                    list.iter()
                        .map(|json| {
                            let f = friend(json)?;
                            let unread =
                                json.get("unread").and_then(Value::as_u64).unwrap_or(0) as u32;
                            Some((f, unread))
                        })
                        .collect::<Option<Vec<_>>>()
                });
            let requests = match answer.get("requests") {
                None => Some(Vec::new()),
                Some(r) => r
                    .as_array()
                    .and_then(|list| list.iter().map(request).collect::<Option<Vec<_>>>()),
            };
            let (Some(list), Some(requests)) = (list, requests) else {
                o.warn("The Incari service sent a friend list this client can't read");
                return;
            };
            let unread: Vec<(String, u32)> = list
                .iter()
                .filter(|(_, n)| *n > 0)
                .map(|(f, n)| (f.fid.clone(), *n))
                .collect();
            o.social
                .set_loaded(list.into_iter().map(|(f, _)| f).collect(), requests, unread);
            o.social.set_available(true);
            if let Some(open) = o.viewing.clone().filter(|p| p.friend) {
                // The conversation on screen catches up with what came while disconnected.
                o.load_older(&open);
                o.mark_read(&open);
            }
        });
    }

    fn message_event(&mut self, json: &Value) -> Option<()> {
        let body = Message {
            mid: json.get("mid")?.as_str()?.to_owned(),
            mine: false,
            text: json.get("text")?.as_str()?.to_owned(),
            at: json.get("at").and_then(Value::as_i64).unwrap_or(self.epoch),
            delivery: Delivery::Sent,
        };
        let peer = match json.get("fid").and_then(Value::as_str) {
            Some(fid) => {
                let label = self
                    .social
                    .friend(fid)
                    .map_or("A friend".to_owned(), |f| f.username.clone());
                Peer::new(true, fid, label)
            }
            None => Peer::new(
                false,
                json.get("pid")?.as_str()?,
                json.get("name")?.as_str()?,
            ),
        };
        let shown = self
            .viewing
            .as_ref()
            .is_some_and(|v| Social::same_peer(v, &peer));
        self.social.message(&peer, body, shown);
        if shown {
            self.mark_read(&peer);
        }
        Some(())
    }

    // ---------------------------------------------------------------------------------------------
    // The connection
    // ---------------------------------------------------------------------------------------------

    fn connect(&mut self) {
        self.reconnect_at = None;
        self.greeted = false;
        self.set_status(Status::Connecting);
        self.generation = self.generation.wrapping_add(1);
        self.link = Some(self.generation);
        // `id` counts from 1 on each connection.
        self.next_id = 1;
        self.open_deadline = Some(self.now + OPEN_TIMEOUT_MILLIS);
        self.effects.push(Effect::Connect {
            connection: self.generation,
            url: self.url.clone(),
        });
    }

    fn say_hello(&mut self) {
        let mut hello = self.request("hello");
        hello.insert("v".into(), json!(PROTOCOL));
        hello.insert("client".into(), json!(self.build));
        self.send(hello, false, |o, answer| {
            match error(answer).as_deref() {
                Some("update") => {
                    o.warn(format!(
                        "The Incari service no longer speaks protocol {PROTOCOL}; update the client"
                    ));
                    o.close_connection();
                    o.set_status(Status::UpdateNeeded);
                    return;
                }
                Some(e) => {
                    // Offline with no link: the connection went already, and its back-off is set.
                    if o.link.is_some() {
                        o.warn(format!("The Incari service refused hello ({e})"));
                        o.drop_connection(None);
                    }
                    return;
                }
                None => {}
            }
            o.greeted = true;
            o.open_deadline = None;
            o.read_hello(answer);
            o.backoff.connected_at = Some(o.now);
            let Some(saved) = o.saved.clone() else {
                o.account_name = None;
                o.set_status(Status::SignedOut);
                return;
            };
            let mut resume = o.request("resume");
            resume.insert("token".into(), json!(saved.token));
            let run = o.sign_up_run;
            o.send(resume, true, move |o, resumed| {
                if run != o.sign_up_run {
                    // Signed out here while the resume was out: it stays signed out.
                    return;
                }
                let e = error(resumed);
                if e.is_none() && o.signed_in_with(resumed, &saved.username) {
                    return;
                }
                match e.as_deref() {
                    Some("unauthorized" | "no_session") => {
                        // The session ended elsewhere (Sign Out Everywhere, a new password, unused).
                        o.clear_session();
                        o.account_name = None;
                        o.set_status(Status::SignedOut);
                    }
                    Some(e) if o.link.is_some() => {
                        o.warn(format!(
                            "The Incari service couldn't resume the session ({e})"
                        ));
                        o.drop_connection(None);
                    }
                    // The connection went already (its requests fail as it closes).
                    Some(_) => {}
                    None => {}
                }
            });
        });
    }

    /// `hello`'s `release` and `featured`.
    fn read_hello(&mut self, answer: &Value) {
        if let Some(named) = answer.get("release").and_then(Value::as_object)
            && let Some(version) = named
                .get("version")
                .and_then(Value::as_str)
                .and_then(crate::release::valid)
        {
            let notes: Vec<String> = named
                .get("notes")
                .and_then(Value::as_array)
                .map(|list| {
                    list.iter()
                        .filter_map(Value::as_str)
                        .filter(|n| crate::release::note(n))
                        .take(crate::release::MAX_NOTES)
                        .map(|n| n.trim().to_owned())
                        .collect()
                })
                .unwrap_or_default();
            if self.latest_release.as_deref() != Some(version) || notes != self.release_notes {
                self.latest_release = Some(version.to_owned());
                self.release_notes = notes;
                self.revision += 1;
            }
        }
        if let Some(list) = answer.get("featured").and_then(Value::as_array) {
            let servers: Vec<Featured> = list
                .iter()
                .filter_map(|s| {
                    let name = s.get("name")?.as_str()?;
                    let address = s.get("address")?.as_str()?;
                    rules::valid_featured(name, address).then(|| Featured {
                        name: name.trim().to_owned(),
                        address: address.trim().to_owned(),
                    })
                })
                .take(4)
                .collect();
            if self.featured.as_ref() != Some(&servers) {
                self.featured = Some(servers);
                self.revision += 1;
            }
        }
    }

    fn receive(&mut self, text: &str) {
        let Ok(message) = serde_json::from_str::<Value>(text) else {
            self.warn("The Incari service sent a message this client can't read");
            return;
        };
        if !message.is_object() {
            self.warn("The Incari service sent a message this client can't read");
            return;
        }
        match message.get("id") {
            None | Some(Value::Null) => {
                if self.event(&message).is_none() {
                    self.warn("The Incari service sent an event this client can't read");
                }
            }
            Some(id) => {
                let Some(id) = id.as_u64() else {
                    self.warn("The Incari service sent an answer to no request");
                    return;
                };
                if let Some(waiting) = self.pending.remove(&id) {
                    (waiting.answer)(self, &message);
                }
            }
        }
    }

    /// Something the service says of its own accord. Unknown types are ignored; None for one this
    /// client can't read.
    fn event(&mut self, m: &Value) -> Option<()> {
        let s = |key: &str| m.get(key).and_then(Value::as_str);
        match s("t").unwrap_or("") {
            "seen" => {
                let player = incari_player(m)?;
                self.world.seen(s("name")?, player);
            }
            "gone" => self.world.gone(s("pid")?),
            "cosmetics" => self.world.cosmetics(s("pid")?, strings(m, "cosmetics")),
            "badge" => self.world.badge(s("pid")?, m.get("badge")?.as_bool()?),
            "request" => self.social.request(request(m)?),
            "requestgone" => self.social.request_gone(s("rid")?),
            "friend" => {
                let f = friend(m.get("friend")?)?;
                let announce = self.social.loaded();
                self.social.friend_changed(f, announce);
            }
            "unfriended" => self.social.unfriended(s("fid")?),
            "message" => self.message_event(m)?,
            "wardrobe" => {
                self.wardrobe = Some((strings(m, "owned"), strings(m, "equipped")));
                self.wardrobe_serial += 1;
                self.revision += 1;
            }
            "emote" => {
                let (pid, emote) = (s("pid")?, s("emote")?);
                if let Some(player) = self.world.uuid(pid)
                    && emote.starts_with("emote.")
                {
                    self.emotes.push(PlayedEmote {
                        player,
                        emote: emote.to_owned(),
                    });
                }
            }
            // Ended elsewhere: Sign Out Everywhere, a new password, the account deleted or banned.
            "signedout" if self.status == Status::SignedIn => self.signed_out_here(),
            // Later protocol additions.
            _ => {}
        }
        Some(())
    }

    /// The connection is gone (or never opened): every waiting request fails, and a retry is set.
    fn drop_connection(&mut self, cause: Option<&str>) {
        let signed_in = self.status == Status::SignedIn;
        self.close_connection();
        if matches!(self.status, Status::UpdateNeeded | Status::Off) {
            return;
        }
        if let Some(cause) = cause
            && !signed_in
        {
            self.warn(format!("Can't reach the Incari service: {cause}"));
        }
        let wait = self.backoff.failed(self.now);
        self.reconnect_at = Some(self.now + wait);
        self.set_status(Status::Connecting);
        self.emotes.clear();
        if signed_in {
            // Back on the next connection; the service forgot the world with this one.
            self.world.ask_all();
        }
    }

    fn close_connection(&mut self) {
        if let Some(connection) = self.link.take() {
            self.effects.push(Effect::Close { connection });
        }
        self.greeted = false;
        self.open_deadline = None;
        self.generation = self.generation.wrapping_add(1);
        self.fail_pending();
        // The service forgets a connection's place when it goes.
        self.forget_presence();
    }

    fn fail_pending(&mut self) {
        let failed = std::mem::take(&mut self.pending);
        let offline = offline();
        for (_, each) in failed {
            (each.answer)(self, &offline);
        }
    }

    fn expire_requests(&mut self, now: f64) {
        let expired: Vec<u64> = self
            .pending
            .iter()
            .filter(|(_, p)| now >= p.deadline)
            .map(|(id, _)| *id)
            .collect();
        if expired.is_empty() {
            return;
        }
        let offline = offline();
        let ours = self.generation;
        let mut signing_in = false;
        for id in expired {
            if let Some(each) = self.pending.remove(&id) {
                signing_in |= each.signs_in;
                (each.answer)(self, &offline);
            }
        }
        if signing_in && ours == self.generation && self.link.is_some() {
            // The service may have signed this connection in after all, and the client doesn't
            // know the account or its token: a fresh connection starts both sides signed out.
            self.warn("The Incari service didn't answer a sign-in in time; connecting again");
            self.drop_connection(None);
        }
    }

    fn request(&mut self, kind: &str) -> Map<String, Value> {
        let mut m = Map::new();
        m.insert("t".into(), json!(kind));
        m.insert("id".into(), json!(self.next_id));
        self.next_id += 1;
        m
    }

    /// Sends a request; `answer` hears the service's answer, or an `offline` error.
    fn send(
        &mut self,
        message: Map<String, Value>,
        signs_in: bool,
        answer: impl FnOnce(&mut Online, &Value) + 'static,
    ) {
        let id = message.get("id").and_then(Value::as_u64).unwrap_or(0);
        let text = Value::Object(message).to_string();
        let Some(connection) = self.link.filter(|_| text.len() <= MAX_MESSAGE) else {
            answer(self, &offline());
            return;
        };
        self.pending.insert(
            id,
            Pending {
                deadline: self.now + REQUEST_TIMEOUT_MILLIS,
                answer: Box::new(answer),
                signs_in,
            },
        );
        self.effects.push(Effect::Send { connection, text });
    }

    // ---------------------------------------------------------------------------------------------
    // Signing in and out
    // ---------------------------------------------------------------------------------------------

    /// A sign-in's answer (`signup`, `signin`, `resume`, `recover`): keeps the token, if there's
    /// a new one, and the account's settings. False if it's malformed.
    fn signed_in_with(&mut self, answer: &Value, username: &str) -> bool {
        let parsed = (|| {
            let mut name = username.to_owned();
            let mut settings = None;
            if let Some(account) = answer.get("account") {
                let account = account.as_object()?;
                if let Some(u) = account.get("username") {
                    name = u.as_str()?.to_owned();
                }
                if let Some(s) = account.get("settings") {
                    settings = Some(read_settings(s)?);
                }
            }
            let token = match answer.get("token") {
                None => None,
                Some(t) => Some(t.as_str()?.to_owned()),
            };
            Some((name, settings, token))
        })();
        let Some((name, settings, token)) = parsed else {
            self.warn("The Incari service sent a sign-in this client can't read");
            return false;
        };
        if let Some((share, badge, from, requests)) = settings {
            if let Some(share) = share {
                self.share_server = share;
            }
            // A service before ADR 0062 leaves it out: shown.
            self.badge = badge.unwrap_or(true);
            let from = from.unwrap_or(self.social.messages_from());
            let requests = requests.unwrap_or(self.social.friend_requests());
            self.social.settings(from, requests);
        }
        if let Some(token) = token {
            let saved = Saved {
                username: name.clone(),
                token,
            };
            self.effects.push(Effect::SaveSession(
                json!({"username": saved.username, "token": saved.token}).to_string(),
            ));
            self.saved = Some(saved);
        }
        self.account_name = Some(name);
        self.set_status(Status::SignedIn);
        self.world.ask_all();
        self.forget_presence();
        self.sync_presence();
        self.load_friends();
        self.load_wardrobe();
        true
    }

    fn clear_session(&mut self) {
        self.saved = None;
        self.effects.push(Effect::DeleteSession);
    }

    fn signed_out_here(&mut self) {
        self.sign_up_run += 1;
        self.solving = None;
        self.social.clear();
        self.clear_session();
        self.wardrobe = None;
        self.emotes.clear();
        self.account_name = None;
        self.share_server = true;
        self.badge = true;
        self.world.ask_all();
        if self.greeted() {
            self.set_status(Status::SignedOut);
        } else {
            self.revision += 1;
        }
    }

    /// Brings the service's idea of where the player is in line with the world: one at a time; one
    /// turned away goes again later, each wait twice the last.
    fn sync_presence(&mut self) {
        if self.status != Status::SignedIn || !self.greeted() || self.presence_out {
            return;
        }
        let now = self.now;
        if now < self.presence_retry_at {
            return;
        }
        let place = self.world.place().cloned();
        if place.is_none()
            && let Some(until) = self.presence_grace_until
        {
            if now < until {
                // Left moments ago: maybe a server switch, which joins the same place again.
                return;
            }
            self.presence_grace_until = None;
        }
        let wanted = place.as_ref().map_or(String::new(), |p| {
            format!(
                "{}\n{}\n{}",
                p.server_hash,
                p.name,
                if self.share_server {
                    p.address.as_str()
                } else {
                    ""
                }
            )
        });
        if wanted == self.presence_held {
            return;
        }
        let mut presence = self.request("presence");
        match &place {
            None => {
                presence.insert("serverHash".into(), Value::Null);
            }
            Some(p) => {
                presence.insert("serverHash".into(), json!(p.server_hash));
                presence.insert("name".into(), json!(p.name));
                if self.share_server {
                    // Only friends ever see it, and the service keeps it in memory only.
                    presence.insert("server".into(), json!(p.address));
                }
            }
        }
        self.presence_out = true;
        let ours = self.generation;
        self.send(presence, false, move |o, answer| {
            if ours != o.generation {
                // The connection it went on is gone, and the service forgot the place with it.
                return;
            }
            o.presence_out = false;
            let Some(e) = error(answer) else {
                o.presence_held = wanted;
                o.presence_retry_delay = PRESENCE_RETRY_MILLIS;
                // The world may have changed while this was out.
                o.sync_presence();
                return;
            };
            match o.failure(answer, Failure::Unavailable) {
                Some(Failure::Rejected) => {
                    // Sending it again won't change the answer: wait for the next place.
                    o.warn(format!(
                        "The Incari service turned down the player's server ({e})"
                    ));
                    o.presence_held = wanted;
                }
                Some(Failure::SignedOut | Failure::Update) => {}
                f => {
                    if !matches!(f, Some(Failure::RateLimited | Failure::Offline)) {
                        o.warn(format!(
                            "The Incari service didn't take the player's server ({e})"
                        ));
                    }
                    o.presence_retry_at = o.now + o.presence_retry_delay;
                    o.presence_retry_delay =
                        (o.presence_retry_delay * 2.0).min(PRESENCE_RETRY_MAX_MILLIS);
                }
            }
        });
    }

    /// A new connection or account: the service holds no place for it yet.
    fn forget_presence(&mut self) {
        self.presence_held.clear();
        self.presence_out = false;
        self.presence_retry_at = 0.0;
        self.presence_retry_delay = PRESENCE_RETRY_MILLIS;
    }

    fn lookup(&mut self, names: Vec<String>) {
        let mut form = self.request("lookup");
        form.insert("names".into(), json!(names));
        self.send(form, false, move |o, answer| {
            if error(answer).is_some() {
                o.world.ask_again(&names);
                return;
            }
            let Some(players) = answer.get("players").and_then(Value::as_array) else {
                o.warn("The Incari service sent a lookup answer this client can't read");
                return;
            };
            for p in players {
                match (p.get("name").and_then(Value::as_str), incari_player(p)) {
                    (Some(name), Some(player)) => o.world.seen(name, player),
                    _ => o.warn("The Incari service sent a lookup answer this client can't read"),
                }
            }
        });
    }

    // ---------------------------------------------------------------------------------------------
    // Answers
    // ---------------------------------------------------------------------------------------------

    fn signed_out_and_connected(&mut self, t: Ticket) -> bool {
        if self.status == Status::SignedOut {
            return true;
        }
        let f = if self.status == Status::UpdateNeeded {
            Failure::Update
        } else {
            Failure::Offline
        };
        self.finish(t, Err(f));
        false
    }

    fn signed_in_and_connected(&mut self, t: Ticket) -> bool {
        if self.status == Status::SignedIn && self.greeted() {
            return true;
        }
        let f = match self.status {
            Status::SignedOut => Failure::SignedOut,
            Status::UpdateNeeded => Failure::Update,
            _ => Failure::Offline,
        };
        self.finish(t, Err(f));
        false
    }

    /// Why the answer failed, or None; `unauthorized` is what that code means for this request.
    /// `no_session` signs out here, `update` stops trying.
    fn failure(&mut self, answer: &Value, unauthorized: Failure) -> Option<Failure> {
        let e = error(answer)?;
        Some(match e.as_str() {
            "unauthorized" => unauthorized,
            "no_session" => {
                if self.status == Status::SignedIn {
                    self.signed_out_here();
                }
                Failure::SignedOut
            }
            "taken" => Failure::Taken,
            "bad_request" | "weak_password" => Failure::Rejected,
            "rate_limited" => Failure::RateLimited,
            "offline" => Failure::Offline,
            "update" => {
                self.close_connection();
                self.set_status(Status::UpdateNeeded);
                Failure::Update
            }
            _ => Failure::Unavailable,
        })
    }

    fn set_status(&mut self, next: Status) {
        if self.status != next {
            self.status = next;
            self.revision += 1;
        }
    }
}

/// The answer's error code, or None for `ok`.
fn error(answer: &Value) -> Option<String> {
    if answer.get("t").and_then(Value::as_str) == Some("ok") {
        return None;
    }
    Some(
        answer
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or("unavailable")
            .to_owned(),
    )
}

/// What a request that never got its answer reads as.
fn offline() -> Value {
    json!({"t": "err", "code": "offline"})
}

fn strings(json: &Value, key: &str) -> Vec<String> {
    json.get(key)
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn codes(answer: &Value) -> Vec<String> {
    strings(answer, "recoveryCodes")
        .iter()
        .map(|c| rules::display_code(c))
        .collect()
}

/// `text` cut to `max` characters.
fn cut(text: &str, max: usize) -> &str {
    match text.char_indices().nth(max) {
        Some((at, _)) => &text[..at],
        None => text,
    }
}

type Settings = (
    Option<bool>,
    Option<bool>,
    Option<MessagesFrom>,
    Option<bool>,
);

/// `account.settings`: shareServer, badge, messages, friendRequests, each optional (older
/// services leave some out); None if one is the wrong type.
fn read_settings(s: &Value) -> Option<Settings> {
    let s = s.as_object()?;
    let flag = |key: &str| -> Option<Option<bool>> {
        match s.get(key) {
            None => Some(None),
            Some(v) => Some(Some(v.as_bool()?)),
        }
    };
    let messages = match s.get("messages") {
        None => None,
        Some(v) => Some(MessagesFrom::of(v.as_str()?)),
    };
    Some((
        flag("shareServer")?,
        flag("badge")?,
        messages,
        flag("friendRequests")?,
    ))
}

fn friend(json: &Value) -> Option<Friend> {
    Some(Friend {
        fid: json.get("fid")?.as_str()?.to_owned(),
        username: json.get("username")?.as_str()?.to_owned(),
        online: json.get("online").and_then(Value::as_bool).unwrap_or(false),
        server: json
            .get("server")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

fn request(json: &Value) -> Option<Request> {
    Some(Request {
        rid: json.get("rid")?.as_str()?.to_owned(),
        incoming: json.get("incoming")?.as_bool()?,
        label: json.get("label")?.as_str()?.to_owned(),
    })
}

fn message(json: &Value) -> Option<Message> {
    Some(Message {
        mid: json.get("mid")?.as_str()?.to_owned(),
        mine: json.get("from")?.as_str()? == "me",
        text: json.get("text")?.as_str()?.to_owned(),
        at: json.get("at")?.as_i64()?,
        delivery: Delivery::Sent,
    })
}

fn incari_player(json: &Value) -> Option<IncariPlayer> {
    Some(IncariPlayer {
        pid: json.get("pid")?.as_str()?.to_owned(),
        friend: json.get("friend").and_then(Value::as_bool).unwrap_or(false),
        cosmetics: strings(json, "cosmetics"),
        // A service before ADR 0062 leaves the badge out: every account shows it.
        badge: json.get("badge").and_then(Value::as_bool).unwrap_or(true),
    })
}
