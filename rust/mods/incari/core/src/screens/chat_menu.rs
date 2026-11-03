//! The chat menu (ChatMenuScreen.java, ADR 0059), laid out like Discord: people down the left, the
//! conversation on the right. Friend requests sit at the top with Accept and Decline on the row,
//! then friends (online first, a presence dot), then other Incari players on this server. The
//! conversation's header has Unfriend or Add Friend, Block (a second click within 4 s) and Report;
//! the gear opens Chat Settings.
//!
//! Opened by the Friends key (O) or the button under the chat box, over the game or a menu.
use super::account::{Account, BlockedPlayers, Report};
use super::form::{self, FieldKind};
use crate::color::alpha;
use crate::draw::Rect;
use crate::icons::Icon;
use crate::input::keys;
use crate::online::social::{Delivery, MessagesFrom, Peer, Request};
use crate::online::{self, Failure, Outcome, Status, Ticket, avatars, rules, toast};
use crate::paint::Painter;
use crate::panels;
use crate::screen::{Cx, Go, Screen};
use crate::theme::CONTROL_HEIGHT;
use crate::ui;
use std::collections::HashMap;

const HEADER: f32 = 34.0;
const ROW: f32 = 22.0;
const SECTION: f32 = 16.0;
const AVATAR: f32 = 16.0;
const PAD: f32 = 8.0;
const SETTING_CONTROL_WIDTH: f32 = 150.0;
const STRIP: f32 = 32.0;
const INPUT: f32 = 20.0;
const ICON_BUTTON: f32 = 16.0;
/// The header keeps at least this for the name before its actions lose their labels.
const MIN_NAME: f32 = 64.0;
/// Messages within this long of the last by the same person join its group, as in Discord.
const GROUP_MILLIS: i64 = 5 * 60_000;
const SIDEBAR_SHADE: u32 = 0x2400_0000;
const LINE: u32 = 0x1AFF_FFFF;
const BADGE_RED: u32 = 0xFFE0_565B;

/// What the right side shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    /// Nobody chosen yet: a welcome.
    Home,
    Chat,
    AddFriend,
    Settings,
}

/// How the player stands with a peer, for the header's actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Relation {
    Friend,
    /// An Incari player on this server.
    Player,
    /// A player who messaged from this server and left it.
    Gone,
}

/// A line in the people list.
#[derive(Clone, Debug)]
struct Row {
    header: bool,
    label: String,
    detail: String,
    peer: Option<Peer>,
    request: Option<Request>,
    friend: bool,
    online: bool,
    /// -1 on a header: an empty section's note.
    unread: i64,
}

/// One drawn line of the conversation: a group's head (avatar, name, time) or a line of text.
struct ChatLine {
    head: bool,
    name: String,
    mine: bool,
    time: String,
    text: String,
    color: u32,
    y: f32,
}

/// What to do with a request's outcome.
enum Then {
    /// Say this when it worked, the failure when not.
    Say(String),
    /// Settings: say "Saved.".
    Saved,
    BefriendByName(String),
    Unfriend(String),
    Block(String),
    /// A message: only a failure is said.
    Send,
}

/// One of the chat header's actions; `warn` for Block's armed second click.
struct Action {
    icon: Icon,
    label: &'static str,
    kind: ActionKind,
    warn: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActionKind {
    Report,
    Block,
    Unfriend,
    AddFriend,
    CancelRequest,
}

pub struct ChatMenu {
    view: View,
    selected: Option<Peer>,
    /// What the player was typing to each person, by conversation.
    drafts: HashMap<String, String>,
    /// What the player was typing in Add Friend.
    friend_draft: String,
    rows: Vec<Row>,
    rows_revision: u64,
    list_scroll: f32,
    chat_scroll: f32,
    status: String,
    status_color: u32,
    status_until: f64,
    /// The header's Block, armed by a first click: a second within a few seconds blocks.
    block_armed_until: f64,
    pending: Vec<(Ticket, Then)>,
    lines: Vec<ChatLine>,
    lines_key: Option<(u64, i32, String)>,
    lines_height: f32,
    /// The field to focus on the next frame (a view or conversation was just chosen).
    focus_next: bool,
}

impl Default for ChatMenu {
    fn default() -> Self {
        Self {
            view: View::Home,
            selected: None,
            drafts: HashMap::new(),
            friend_draft: String::new(),
            rows: Vec::new(),
            rows_revision: u64::MAX,
            list_scroll: 0.0,
            chat_scroll: 0.0,
            status: String::new(),
            status_color: 0,
            status_until: 0.0,
            block_armed_until: 0.0,
            pending: Vec::new(),
            lines: Vec::new(),
            lines_key: None,
            lines_height: 0.0,
            focus_next: false,
        }
    }
}

/// The menu's frame: where its parts are.
struct Layout {
    px: f32,
    py: f32,
    pw: f32,
    ph: f32,
    side: f32,
    main_x: f32,
    main_width: f32,
    list_top: f32,
    list_bottom: f32,
    chat_top: f32,
    chat_bottom: f32,
}

impl Layout {
    fn of([width, height]: [f32; 2]) -> Self {
        let pw = (width - 20.0).min(620.0);
        let ph = (height - 16.0).min(352.0);
        let px = ((width - pw) / 2.0).floor();
        let py = ((height - ph) / 2.0).floor();
        let side = (pw * 0.27).round().clamp(118.0, 150.0);
        Self {
            px,
            py,
            pw,
            ph,
            side,
            main_x: px + side,
            main_width: pw - side,
            list_top: py + PAD + CONTROL_HEIGHT + 6.0,
            list_bottom: py + ph - STRIP,
            chat_top: py + HEADER,
            chat_bottom: py + ph - PAD - INPUT - 6.0,
        }
    }
}

/// The menu's text field, for `Ui::focus`.
const INPUT_ID: u64 = 0x0001_C4A7_0001;

impl ChatMenu {
    /// Opened on the conversation with `peer` (a toast's click, later).
    pub fn with_peer(peer: Peer) -> Self {
        let mut menu = Self::default();
        menu.select(Some(peer));
        menu
    }

    fn select(&mut self, peer: Option<Peer>) {
        self.selected = peer.clone();
        self.block_armed_until = 0.0;
        self.chat_scroll = 0.0;
        self.focus_next = true;
        online::with(|o| {
            o.set_viewing(peer.as_ref());
            if let Some(peer) = &peer {
                o.load_older(peer);
            }
        });
        if peer.is_some() {
            self.view = View::Chat;
        }
    }

    fn show(&mut self, next: View) {
        self.view = next;
        self.focus_next = true;
        if next != View::Chat {
            self.select(None);
        }
    }

    fn say(&mut self, text: &str, color: u32, now: f64) {
        self.status = text.to_owned();
        self.status_color = color;
        self.status_until = now + 5000.0;
    }

    fn relation(&self, peer: &Peer) -> Relation {
        online::with(|o| {
            if peer.friend {
                if o.social().friend(&peer.id).is_some() {
                    Relation::Friend
                } else {
                    Relation::Gone
                }
            } else if o.world_players().contains_key(&peer.label) {
                Relation::Player
            } else {
                Relation::Gone
            }
        })
    }

    /// The list, made again when anything online changes: requests, friends, then players here.
    fn build_rows(&mut self) {
        let revision = online::with(|o| o.revision());
        if revision == self.rows_revision {
            return;
        }
        self.rows_revision = revision;
        self.rows = online::with(|o| rows(o));
    }

    fn list_height(&self) -> f32 {
        self.rows
            .iter()
            .map(|r| if r.header { SECTION } else { ROW })
            .sum()
    }

    /// Takes the outcomes that came in, and says what they came to.
    fn poll(&mut self, now: f64, th: crate::theme::Theme) {
        let mut i = 0;
        while i < self.pending.len() {
            let ticket = self.pending[i].0;
            let Some(outcome) = online::with(|o| o.take_result(ticket)) else {
                i += 1;
                continue;
            };
            let (_, then) = self.pending.remove(i);
            self.answered(then, outcome, now, th);
        }
    }

    fn answered(&mut self, then: Then, outcome: Outcome, now: f64, th: crate::theme::Theme) {
        let failure = outcome.as_ref().err().copied();
        let message = |f: Option<Failure>| f.map_or("", Failure::message).to_owned();
        match then {
            Then::Say(success) => match failure {
                None => self.say(&success, th.positive, now),
                Some(f) => self.say(f.message(), th.negative, now),
            },
            Then::Saved => match failure {
                None => self.say("Saved.", th.positive, now),
                Some(f) => self.say(f.message(), th.negative, now),
            },
            Then::BefriendByName(username) => match failure {
                None => {
                    if self.friend_draft == username {
                        self.friend_draft.clear();
                    }
                    self.say(
                        &format!("Friend request sent to {username}."),
                        th.positive,
                        now,
                    );
                }
                Some(Failure::Rejected) => {
                    self.say("No Incari account has that username.", th.negative, now)
                }
                Some(f) => self.say(f.message(), th.negative, now),
            },
            Then::Unfriend(username) => match failure {
                None => {
                    self.select(None);
                    self.view = View::Home;
                    self.say(
                        &format!("Removed {username} from your friends."),
                        th.muted,
                        now,
                    );
                }
                f => self.say(&message(f), th.negative, now),
            },
            Then::Block(label) => match failure {
                None => {
                    self.select(None);
                    self.view = View::Home;
                    self.say(
                        &format!("Blocked {label}. Unblock them in Chat Settings."),
                        th.muted,
                        now,
                    );
                }
                f => self.say(&message(f), th.negative, now),
            },
            Then::Send => {
                if let Some(f) = failure {
                    self.say(f.message(), th.negative, now);
                }
            }
        }
    }

    fn send(&mut self) {
        let Some(peer) = self.selected.clone() else {
            return;
        };
        let text = self.drafts.get(&peer.key()).cloned().unwrap_or_default();
        if text.trim().is_empty() {
            return;
        }
        self.drafts.remove(&peer.key());
        self.chat_scroll = 0.0;
        let t = online::with(|o| o.send_message(&peer, &text));
        self.pending.push((t, Then::Send));
    }

    fn befriend_by_name(&mut self, now: f64, th: crate::theme::Theme) {
        let username = self.friend_draft.clone();
        if rules::username_problem(&username).is_some() {
            self.say(
                "Type an Incari username: 3 to 16 letters, digits or _.",
                th.negative,
                now,
            );
            return;
        }
        let t = online::with(|o| o.befriend(&username));
        self.pending.push((t, Then::BefriendByName(username)));
    }

    fn act(&mut self, cx: &mut Cx, kind: ActionKind, now: f64) {
        let th = cx.p.theme;
        let Some(peer) = self.selected.clone() else {
            return;
        };
        match kind {
            ActionKind::Report => {
                let username = if peer.friend {
                    online::with(|o| o.social().friend(&peer.id).map(|f| f.username.clone()))
                } else {
                    None
                };
                match username {
                    Some(name) => cx.open(Report::username(&name)),
                    None => cx.open(Report::player(&peer.label, &peer.id)),
                }
            }
            ActionKind::Block => {
                if self.block_armed_until == 0.0 {
                    self.block_armed_until = now + 4000.0;
                    return;
                }
                self.block_armed_until = 0.0;
                let t = online::with(|o| {
                    if peer.friend {
                        let username = o.social().friend(&peer.id).map(|f| f.username.clone())?;
                        Some(o.block_friend(&peer.id, &username))
                    } else {
                        Some(o.block_player(&peer.label, &peer.id))
                    }
                });
                if let Some(t) = t {
                    self.pending.push((t, Then::Block(peer.label.clone())));
                }
            }
            ActionKind::Unfriend => {
                let t = online::with(|o| {
                    let username = o.social().friend(&peer.id).map(|f| f.username.clone())?;
                    Some((o.unfriend(&peer.id), username))
                });
                if let Some((t, username)) = t {
                    self.pending.push((t, Then::Unfriend(username)));
                }
            }
            ActionKind::AddFriend => {
                let pid =
                    online::with(|o| o.world_players().get(&peer.label).map(|p| p.pid.clone()));
                let Some(pid) = pid else {
                    self.say("They've left this server.", th.negative, now);
                    return;
                };
                let t = online::with(|o| o.befriend_player(&peer.label, &pid));
                self.pending.push((
                    t,
                    Then::Say(format!("Friend request sent to {}.", peer.label)),
                ));
            }
            ActionKind::CancelRequest => {
                if let Some(sent) = online::with(|o| o.social().sent_to(&peer).cloned()) {
                    self.decline(&sent);
                }
            }
        }
    }

    fn accept(&mut self, request: &Request) {
        let t = online::with(|o| o.accept(&request.rid));
        self.pending.push((
            t,
            Then::Say(format!("You and {} are friends now.", request.label)),
        ));
    }

    fn decline(&mut self, request: &Request) {
        let t = online::with(|o| o.decline(&request.rid));
        let said = if request.incoming {
            "Request declined."
        } else {
            "Request cancelled."
        };
        self.pending.push((t, Then::Say(said.into())));
    }
}

/// The people list from what the service holds.
fn rows(o: &online::Online) -> Vec<Row> {
    let mut rows = Vec::new();
    if !o.signed_in() {
        return rows;
    }
    let social = o.social();
    let header = |label: &str, count: usize| Row {
        header: true,
        label: label.into(),
        detail: if count == 0 {
            String::new()
        } else {
            count.to_string()
        },
        peer: None,
        request: None,
        friend: false,
        online: false,
        unread: 0,
    };
    let note = |label: &str| Row {
        unread: -1,
        ..header(label, 0)
    };
    let requests = social.requests();
    if !requests.is_empty() {
        rows.push(header("Friend Requests", requests.len()));
        for request in requests {
            rows.push(Row {
                header: false,
                label: request.label.clone(),
                detail: if request.incoming {
                    "Wants to be friends"
                } else {
                    "Request sent"
                }
                .into(),
                peer: None,
                request: Some(request),
                friend: false,
                online: false,
                unread: 0,
            });
        }
    }
    let friends = social.friends();
    let on = friends.iter().filter(|f| f.online).count();
    rows.push(Row {
        friend: true,
        detail: if friends.is_empty() {
            String::new()
        } else {
            format!("{on}/{}", friends.len())
        },
        ..header("Friends", 0)
    });
    if friends.is_empty() {
        rows.push(note("No friends yet"));
    }
    for friend in &friends {
        let peer = Peer::new(true, &friend.fid, &friend.username);
        let unread = social.existing(&peer).map_or(0, |c| c.unread());
        let detail = if !friend.online {
            "Offline".to_owned()
        } else {
            friend.server.clone().unwrap_or_else(|| "Online".into())
        };
        rows.push(Row {
            header: false,
            label: friend.username.clone(),
            detail,
            peer: Some(peer),
            request: None,
            friend: true,
            online: friend.online,
            unread: i64::from(unread),
        });
    }
    // Everyone else here who's an Incari player, below every friend.
    let here = o.world_players();
    let mut names: Vec<String> = here
        .iter()
        .filter(|(_, p)| !p.friend)
        .map(|(n, _)| n.clone())
        .collect();
    let messaged = social.players_messaged();
    for (_, name) in messaged {
        if !here.contains_key(name) && !names.contains(name) {
            names.push(name.clone());
        }
    }
    names.sort_by(|a, b| online::social::case_insensitive(a, b));
    if !names.is_empty() || o.in_world() {
        rows.push(header("On This Server", names.len()));
    }
    if names.is_empty() && o.in_world() {
        rows.push(note("No other Incari players"));
    }
    for name in names {
        let player = here.get(&name);
        let pid = player.map(|p| p.pid.clone()).unwrap_or_else(|| {
            messaged
                .iter()
                .find(|(_, n)| *n == name)
                .map(|(pid, _)| pid.clone())
                .unwrap_or_default()
        });
        let peer = Peer::new(false, pid, &name);
        let unread = social.existing(&peer).map_or(0, |c| c.unread());
        rows.push(Row {
            header: false,
            label: name,
            detail: if player.is_some() {
                "Incari player"
            } else {
                "Left the server"
            }
            .into(),
            peer: Some(peer),
            request: None,
            friend: false,
            online: player.is_some(),
            unread: i64::from(unread),
        });
    }
    rows
}

/// A small round button with an icon, and a tooltip naming it.
fn icon_button(cx: &mut Cx, x: f32, y: f32, icon: Icon, name: &str, tint: Option<u32>) -> bool {
    let r = [x, y, ICON_BUTTON, ICON_BUTTON];
    let hot = cx.ui.over(r);
    let th = cx.p.theme;
    if hot {
        cx.p.round(
            x,
            y,
            ICON_BUTTON,
            ICON_BUTTON,
            ICON_BUTTON / 2.0,
            th.button_hot,
        );
        cx.ui.tooltip(name);
    }
    let color = tint.unwrap_or(if hot { th.text } else { th.muted });
    cx.p.icon(icon, x + 3.0, y + 3.0, ICON_BUTTON - 6.0, color);
    cx.ui.clicked(r)
}

impl Screen for ChatMenu {
    fn draw(&mut self, cx: &mut Cx) {
        let now = cx.host.now_ms();
        toast::menu_shown(now);
        let th = cx.p.theme;
        self.poll(now, th);
        if self.block_armed_until > 0.0 && now > self.block_armed_until {
            self.block_armed_until = 0.0;
        }
        if let Some(peer) = self.selected.clone() {
            if peer.friend && self.relation(&peer) == Relation::Gone {
                // Unfriended (or blocked) from the other side.
                self.select(None);
                self.view = View::Home;
            } else if online::with(|o| o.viewing() != Some(&peer)) {
                // Back from Report, Blocked Players or the account screen: the conversation shown is
                // the one whose messages arrive read again.
                online::with(|o| o.set_viewing(Some(&peer)));
            }
        }
        self.build_rows();
        let l = Layout::of(cx.size);
        let signed_in = online::with(|o| o.signed_in());

        panels::panel(cx.p, l.px, l.py, l.pw, l.ph, 1.0);
        cx.p.round(
            l.px + 1.0,
            l.py + 1.0,
            l.side - 1.0,
            l.ph - 2.0,
            th.panel_radius,
            SIDEBAR_SHADE,
        );
        cx.p.fill(l.px + l.side, l.py + 6.0, 1.0, l.ph - 12.0, LINE);
        cx.p.fill(
            l.main_x + 6.0,
            l.py + HEADER,
            l.pw - l.side - 12.0,
            1.0,
            LINE,
        );
        cx.p.fill(l.px + 6.0, l.list_bottom, l.side - 12.0, 1.0, LINE);

        // The sidebar: Add Friend on top, the player's strip with its gear at the bottom.
        if signed_in {
            let r = [l.px + PAD, l.py + PAD, l.side - 2.0 * PAD, CONTROL_HEIGHT];
            if ui::menu_button(
                cx.p,
                cx.ui,
                r,
                "Add Friend",
                Some(Icon::PersonAdd),
                "",
                true,
            ) {
                self.show(View::AddFriend);
            }
            let gy = l.py + l.ph - STRIP + ((STRIP - ICON_BUTTON) / 2.0).floor();
            if icon_button(
                cx,
                l.px + l.side - PAD - ICON_BUTTON,
                gy,
                Icon::Gear,
                "Chat settings",
                None,
            ) {
                self.show(View::Settings);
            }
        }
        let close_y = l.py + ((HEADER - ICON_BUTTON) / 2.0).floor();
        if icon_button(
            cx,
            l.px + l.pw - PAD - ICON_BUTTON,
            close_y,
            Icon::Close,
            "Close",
            None,
        ) {
            cx.go(Go::Back);
        }
        self.draw_list(cx, &l);
        draw_strip(cx.p, &l);
        if !signed_in {
            self.draw_signed_out(cx, &l);
        } else {
            match self.view {
                View::Home => self.draw_home(cx.p, &l),
                View::Chat => self.draw_chat(cx, &l, now),
                View::AddFriend => self.draw_add_friend(cx, &l, now),
                View::Settings => self.draw_settings(cx, &l),
            }
        }
        if !self.status.is_empty() && now < self.status_until {
            let tw = cx.p.width(&self.status);
            let x = (l.main_x + (l.main_width - tw) / 2.0).floor();
            let y = if self.view == View::Chat {
                l.chat_bottom - 14.0
            } else {
                l.py + l.ph - PAD - 12.0
            };
            cx.p.round(x - 6.0, y - 3.0, tw + 12.0, 13.0, 6.0, th.floating);
            let status = self.status.clone();
            cx.p.text(&status, x, y, self.status_color, false);
        }
        // The Friends key closes the menu, unless the player is typing.
        if cx.ui.focus.is_none()
            && let crate::input::Key::Keyboard(friends) = cx.host.friends_key()
            && cx.ui.key(friends)
        {
            cx.go(Go::Back);
        }
    }

    fn closed(&mut self, _cx: &mut Cx) {
        online::with(|o| {
            o.set_viewing(None);
            for (t, _) in self.pending.drain(..) {
                o.forget(t);
            }
        });
    }
}

impl ChatMenu {
    fn draw_list(&mut self, cx: &mut Cx, l: &Layout) {
        let th = cx.p.theme;
        let area = [l.px, l.list_top, l.side, l.list_bottom - l.list_top];
        let max = (self.list_height() - area[3]).max(0.0);
        let steps = cx.ui.scrolled(area);
        self.list_scroll = (self.list_scroll - steps * 20.0).clamp(0.0, max);
        cx.p.clip_start(area);
        let mut y = l.list_top - self.list_scroll.floor();
        let chosen = self.selected.clone();
        let mut clicked: Option<(Row, f32)> = None;
        for row in &self.rows {
            if row.header && row.unread < 0 {
                // An empty section's note, a section's height.
                cx.p.text(
                    &row.label,
                    l.px + PAD + 2.0,
                    y + 4.0,
                    alpha(th.muted, 0.6),
                    false,
                );
                y += SECTION;
                continue;
            }
            if row.header {
                if y + SECTION > l.list_top && y < l.list_bottom {
                    cx.p.text(
                        &row.label.to_uppercase(),
                        l.px + PAD + 2.0,
                        y + 6.0,
                        th.muted,
                        false,
                    );
                    if !row.detail.is_empty() {
                        let w = cx.p.width(&row.detail);
                        cx.p.text(
                            &row.detail,
                            l.px + l.side - PAD - w,
                            y + 6.0,
                            alpha(th.muted, 0.7),
                            false,
                        );
                    }
                }
                y += SECTION;
                continue;
            }
            if y + ROW > l.list_top && y < l.list_bottom {
                draw_row(cx, l, row, y, chosen.as_ref());
                let hit = [
                    l.px,
                    y.max(l.list_top),
                    l.side,
                    (y + ROW).min(l.list_bottom) - y.max(l.list_top),
                ];
                if hit[3] > 0.0 && cx.ui.clicked(hit) {
                    clicked = Some((row.clone(), cx.ui.mouse[0]));
                }
            }
            y += ROW;
        }
        cx.p.clip_end();
        if let Some((row, mx)) = clicked {
            if let Some(request) = &row.request {
                let right = l.px + l.side - PAD;
                if mx >= right - 14.0 {
                    self.decline(request);
                } else if request.incoming && mx >= right - 30.0 {
                    self.accept(request);
                }
            } else if let Some(peer) = row.peer {
                self.select(Some(peer));
            }
        }
    }

    fn draw_signed_out(&mut self, cx: &mut Cx, l: &Layout) {
        let th = cx.p.theme;
        let status = online::with(|o| o.status());
        let (title, text) = match status {
            Status::Off => (
                "Chat isn't available",
                "Incari accounts aren't available in this version.",
            ),
            Status::UpdateNeeded => ("Update required", "Update Incari Client to use chat."),
            Status::Connecting => (
                "Connecting...",
                "Your network may be blocking Incari. The game works the same without it.",
            ),
            _ => (
                "Incari Chat",
                "Sign in to add friends and message them on any server.",
            ),
        };
        let lines = cx.p.wrap(text, (l.main_width - 40.0).min(260.0), 64);
        let button = matches!(status, Status::SignedOut | Status::Connecting);
        let height =
            46.0 + lines.len() as f32 * 10.0 + if button { 6.0 + CONTROL_HEIGHT } else { 0.0 };
        let top = l.py + ((l.ph - height) / 2.0).floor().max(4.0);
        let center = l.main_x + l.main_width / 2.0;
        cx.p.icon(Icon::Chat, center - 12.0, top, 24.0, alpha(th.accent, 0.9));
        cx.p.centered(title, center, top + 32.0, th.text, false);
        let mut y = top + 46.0;
        for line in &lines {
            cx.p.centered(line, center, y, th.muted, false);
            y += 10.0;
        }
        let r = [(center - 60.0).floor(), y + 6.0, 120.0, CONTROL_HEIGHT];
        match status {
            Status::SignedOut if ui::button(cx.p, cx.ui, r, "Sign In", true) => {
                cx.open(Account::default())
            }
            Status::Connecting if ui::button(cx.p, cx.ui, r, "Try Again", true) => {
                online::with(|o| o.retry())
            }
            _ => {}
        }
    }

    fn draw_home(&self, p: &mut Painter, l: &Layout) {
        let th = p.theme;
        p.text(
            "Incari Chat",
            l.main_x + PAD + 4.0,
            l.py + ((HEADER - 8.0) / 2.0).floor(),
            th.text,
            false,
        );
        let center = l.main_x + l.main_width / 2.0;
        let y = l.py + HEADER + ((l.ph - HEADER) / 2.0).floor() - 40.0;
        p.icon(Icon::Chat, center - 14.0, y, 28.0, alpha(th.accent, 0.9));
        let (available, empty, nobody) = online::with(|o| {
            let s = o.social();
            (
                s.available(),
                s.friends().is_empty() && s.requests().is_empty(),
                s.messages_from() == MessagesFrom::Nobody,
            )
        });
        let body = if available == Some(false) {
            "Friends and messages aren't available yet. You can still block and report players."
        } else if empty {
            "No friends yet. Add one by their Incari username, or pick an Incari player on this server."
        } else {
            "Pick someone on the left to chat."
        };
        let mut line_y = y + 38.0;
        for line in p.wrap(body, (l.main_width - 40.0).min(280.0), 64) {
            p.centered(&line, center, line_y, th.muted, false);
            line_y += 10.0;
        }
        if nobody {
            p.centered(
                "Messages are off. Turn them on in Chat Settings.",
                center,
                line_y + 8.0,
                th.warning,
                false,
            );
        }
    }

    fn draw_add_friend(&mut self, cx: &mut Cx, l: &Layout, now: f64) {
        let th = cx.p.theme;
        cx.p.text(
            "Add Friend",
            l.main_x + PAD + 4.0,
            l.py + ((HEADER - 8.0) / 2.0).floor(),
            th.text,
            false,
        );
        let form_width = (l.main_width - 4.0 * PAD).min(260.0);
        let x = (l.main_x + (l.main_width - form_width) / 2.0).floor();
        let mut y = l.py + HEADER + 16.0;
        let help = "Ask your friend for their Incari username, or pick a player on this server from the left.";
        for line in cx.p.wrap(help, form_width, 64) {
            cx.p.text(&line, x, y, th.muted, false);
            y += 10.0;
        }
        if !online::with(|o| o.social().friend_requests()) {
            let warning = "Your friend requests are off, so nobody can add you.";
            cx.p.text(warning, x, l.py + HEADER + 112.0, th.warning, false);
        }
        if std::mem::take(&mut self.focus_next) {
            cx.ui.focus = Some(INPUT_ID);
            cx.ui.caret = self.friend_draft.chars().count();
        }
        let field_y = l.py + HEADER + 54.0;
        let r = [x, field_y, form_width, CONTROL_HEIGHT];
        form::field(
            cx.p,
            cx.ui,
            INPUT_ID,
            r,
            &mut self.friend_draft,
            "Their Incari username",
            rules::MAX_USERNAME,
            FieldKind::Username,
        );
        let enter = cx.ui.key(keys::ENTER) || cx.ui.key(keys::KEYPAD_ENTER);
        let button = [
            x,
            field_y + CONTROL_HEIGHT + 6.0,
            form_width,
            CONTROL_HEIGHT,
        ];
        let pressed = ui::menu_button(
            cx.p,
            cx.ui,
            button,
            "Send Friend Request",
            Some(Icon::PersonAdd),
            "",
            true,
        );
        if pressed || enter {
            cx.ui.focus = Some(INPUT_ID);
            self.befriend_by_name(now, th);
        }
    }

    fn draw_settings(&mut self, cx: &mut Cx, l: &Layout) {
        let th = cx.p.theme;
        cx.p.text(
            "Chat Settings",
            l.main_x + PAD + 4.0,
            l.py + ((HEADER - 8.0) / 2.0).floor(),
            th.text,
            false,
        );
        let x = l.main_x + 2.0 * PAD;
        let y = l.py + HEADER + 30.0;
        let settings = [
            ("Who can message you", "Choose who can send you messages."),
            (
                "Friend requests",
                "Off: nobody can send you one. You can still send them.",
            ),
            (
                "Show friends your server",
                "Friends can see which server you're on.",
            ),
        ];
        let column = l.main_x + l.main_width - PAD * 2.0 - SETTING_CONTROL_WIDTH - 6.0 - x;
        for (i, (name, help)) in settings.iter().enumerate() {
            let sy = y + i as f32 * 40.0;
            let shown = cx.p.fit(name, column).into_owned();
            cx.p.text(&shown, x, sy + 2.0, th.text, false);
            for (j, line) in cx.p.wrap(help, column / 0.75, 2).iter().enumerate() {
                cx.p.text_scaled(
                    line,
                    x,
                    sy + 13.0 + j as f32 * 10.0 * 0.75,
                    th.muted,
                    false,
                    0.75,
                );
            }
        }
        let (from, requests, shares) = online::with(|o| {
            (
                o.social().messages_from(),
                o.social().friend_requests(),
                o.shares_server(),
            )
        });
        let cxr = l.main_x + l.main_width - PAD * 2.0 - SETTING_CONTROL_WIDTH;
        let control = |dy: f32| -> Rect { [cxr, y + dy, SETTING_CONTROL_WIDTH, CONTROL_HEIGHT] };
        let back = cx.ui.shift;
        let choice = control(0.0);
        let stepped = if ui::button(cx.p, cx.ui, choice, from.label(), true) {
            Some(back)
        } else if cx.ui.right_clicked(choice) {
            Some(true)
        } else {
            None
        };
        if let Some(back) = stepped {
            let all = MessagesFrom::ALL;
            let at = all.iter().position(|m| *m == from).unwrap_or(1);
            let next = all[if back {
                (at + all.len() - 1) % all.len()
            } else {
                (at + 1) % all.len()
            }];
            let t = online::with(|o| o.set_messages_from(next));
            self.pending.push((t, Then::Saved));
        }
        if ui::switch(cx.p, cx.ui, control(40.0), requests) {
            let t = online::with(|o| o.set_friend_requests(!requests));
            self.pending.push((t, Then::Saved));
        }
        if ui::switch(cx.p, cx.ui, control(80.0), shares) {
            let t = online::with(|o| o.set_share_server(!shares));
            self.pending.push((t, Then::Saved));
        }
        let half = ((l.main_width - 4.0 * PAD - 6.0) / 2.0).floor();
        let blocked = [l.main_x + 2.0 * PAD, y + 128.0, half, CONTROL_HEIGHT];
        if ui::menu_button(
            cx.p,
            cx.ui,
            blocked,
            "Blocked Players",
            Some(Icon::Block),
            "",
            true,
        ) {
            cx.open(BlockedPlayers::default());
        }
        let account = [
            l.main_x + 2.0 * PAD + half + 6.0,
            y + 128.0,
            half,
            CONTROL_HEIGHT,
        ];
        if ui::menu_button(
            cx.p,
            cx.ui,
            account,
            "Account",
            Some(Icon::Person),
            "",
            true,
        ) {
            cx.open(Account::default());
        }
    }

    fn draw_chat(&mut self, cx: &mut Cx, l: &Layout, now: f64) {
        let Some(peer) = self.selected.clone() else {
            return;
        };
        let th = cx.p.theme;
        let relation = self.relation(&peer);
        let (friend, sent) = online::with(|o| {
            let friend = if peer.friend {
                o.social().friend(&peer.id).cloned()
            } else {
                None
            };
            (friend, o.social().sent_to(&peer).is_some())
        });

        // The header's actions, right to left from the close button: Report, Block, then the
        // friendship's.
        let armed = self.block_armed_until > 0.0;
        let mut actions = vec![
            Action {
                icon: Icon::Flag,
                label: "Report",
                kind: ActionKind::Report,
                warn: false,
            },
            Action {
                icon: Icon::Block,
                label: if armed { "Click to Block" } else { "Block" },
                kind: ActionKind::Block,
                warn: armed,
            },
        ];
        match relation {
            Relation::Friend => {
                actions.push(Action {
                    icon: Icon::Close,
                    label: "Unfriend",
                    kind: ActionKind::Unfriend,
                    warn: false,
                });
            }
            // Only a sent request: a received one is answered on its row, since its sender's
            // username can't be tied to a player here. Add Friend to someone who already asked
            // makes them friends at once.
            Relation::Player if !sent => {
                actions.push(Action {
                    icon: Icon::PersonAdd,
                    label: "Add Friend",
                    kind: ActionKind::AddFriend,
                    warn: false,
                });
            }
            Relation::Player => actions.push(Action {
                icon: Icon::Close,
                label: "Cancel Request",
                kind: ActionKind::CancelRequest,
                warn: false,
            }),
            Relation::Gone => {}
        }
        let mut right = l.px + l.pw - PAD - ICON_BUTTON - 6.0;
        let width = |p: &Painter, label: &str| 24.0 + p.width(label) + 8.0;
        let labelled: f32 = actions.iter().map(|a| width(cx.p, a.label) + 4.0).sum();
        // Labels while the name has room; on a narrow panel icons with tooltips.
        let icons_only = right - labelled < l.main_x + PAD + 2.0 + AVATAR + 7.0 + MIN_NAME;
        let mut acted = None;
        for a in &actions {
            if icons_only {
                let y = l.py + ((HEADER - ICON_BUTTON) / 2.0).floor();
                let tint = a.warn.then_some(th.negative);
                if icon_button(cx, right - ICON_BUTTON, y, a.icon, a.label, tint) {
                    acted = Some(a.kind);
                }
                right -= ICON_BUTTON + 4.0;
            } else {
                let w = width(cx.p, a.label);
                let r = [
                    right - w,
                    l.py + ((HEADER - CONTROL_HEIGHT) / 2.0).floor(),
                    w,
                    CONTROL_HEIGHT,
                ];
                if ui::menu_button(cx.p, cx.ui, r, a.label, Some(a.icon), "", true) {
                    acted = Some(a.kind);
                }
                right -= w + 4.0;
            }
        }
        let actions_left = right;

        // Who, and how the player stands with them.
        let avatar_y = l.py + ((HEADER - AVATAR) / 2.0).floor();
        avatars::draw(cx.p, l.main_x + PAD + 2.0, avatar_y, AVATAR, &peer.label);
        if let Some(f) = &friend {
            avatars::status(
                cx.p,
                l.main_x + PAD + 2.0,
                avatar_y,
                AVATAR,
                f.online,
                0xFF26_2E37,
            );
        }
        let text_x = l.main_x + PAD + 2.0 + AVATAR + 7.0;
        let text_width = (actions_left - 4.0 - text_x).max(0.0);
        let name = cx.p.fit(&peer.label, text_width).into_owned();
        cx.p.text(&name, text_x, avatar_y - 1.0, th.text, false);
        let about = match relation {
            Relation::Friend => match &friend {
                Some(f) if f.online => {
                    format!(
                        "Friend · {}",
                        f.server
                            .as_ref()
                            .map_or("online".to_owned(), |s| format!("on {s}"))
                    )
                }
                _ => "Friend · offline".to_owned(),
            },
            Relation::Player if sent => "Friend request sent".into(),
            Relation::Player => "Incari player on this server".into(),
            Relation::Gone => "Left the server".into(),
        };
        let about = cx.p.fit(&about, text_width / 0.75).into_owned();
        let about_color = if relation == Relation::Friend {
            alpha(th.accent, 0.9)
        } else {
            th.muted
        };
        cx.p.text_scaled(&about, text_x, avatar_y + 10.0, about_color, false, 0.75);

        // The conversation, newest at the bottom.
        self.lay_out_lines(cx.p, l, &peer);
        let (has_more, loading) = online::with(|o| {
            o.social()
                .existing(&peer)
                .map_or((peer.friend, false), |c| (c.has_more(), c.loading()))
        });
        let area_top = l.chat_top + 4.0;
        let area_bottom = l.chat_bottom - 2.0;
        let area = [l.main_x, area_top, l.pw - l.side, area_bottom - area_top];
        let max = (self.lines_height - (l.chat_bottom - l.chat_top - 8.0)).max(0.0);
        let steps = cx.ui.scrolled([
            l.main_x,
            l.chat_top,
            l.main_width,
            l.chat_bottom - l.chat_top,
        ]);
        if steps != 0.0 {
            self.chat_scroll = (self.chat_scroll + steps * 20.0).clamp(0.0, max);
            if self.chat_scroll >= max - 4.0 {
                // At the top: the page before, if there is one.
                online::with(|o| o.load_older(&peer));
            }
        }
        cx.p.clip_start(area);
        let mut base = area_bottom - self.lines_height + self.chat_scroll.floor();
        if self.lines_height < area_bottom - area_top {
            base = area_bottom - self.lines_height;
        }
        if !has_more || !peer.friend {
            // The start of the conversation, as Discord marks it.
            let start_y = base - 44.0;
            if start_y > area_top - 44.0 {
                avatars::draw(cx.p, l.main_x + PAD + 4.0, start_y, 22.0, &peer.label);
                cx.p.text(
                    &peer.label,
                    l.main_x + PAD + 32.0,
                    start_y + 2.0,
                    th.text,
                    false,
                );
                let start = if peer.friend {
                    format!("This is the start of your messages with {}.", peer.label)
                } else {
                    "Messages with players here end when one of you leaves.".to_owned()
                };
                let start = cx.p.fit(&start, l.main_width - 50.0).into_owned();
                cx.p.text(
                    &start,
                    l.main_x + PAD + 32.0,
                    start_y + 13.0,
                    th.muted,
                    false,
                );
            }
        } else if loading {
            cx.p.centered(
                "Loading...",
                l.main_x + l.main_width / 2.0,
                base - 14.0,
                th.muted,
                false,
            );
        }
        let text_x0 = l.main_x + PAD + 4.0;
        for line in &self.lines {
            let y = base + line.y;
            if y + 12.0 < area_top || y > area_bottom {
                continue;
            }
            if line.head {
                avatars::draw(cx.p, text_x0, y + 1.0, AVATAR, &line.name);
                let color = if line.mine {
                    0xFFE6_EBF0
                } else {
                    avatars::lighter(&line.name)
                };
                cx.p.text(&line.name, text_x0 + AVATAR + 7.0, y, color, false);
                let name_end = text_x0 + AVATAR + 7.0 + cx.p.width(&line.name) + 6.0;
                cx.p.text_scaled(
                    &line.time,
                    name_end,
                    y + 2.0,
                    alpha(th.muted, 0.7),
                    false,
                    0.75,
                );
            } else {
                cx.p.text(&line.text, text_x0 + AVATAR + 7.0, y, line.color, false);
            }
        }
        cx.p.clip_end();

        // The input in its rounded well, as Discord's.
        let well_y = l.py + l.ph - PAD - INPUT;
        cx.p.round(
            l.main_x + PAD - 3.0,
            well_y - 3.0,
            l.main_width - 2.0 * PAD + 6.0,
            INPUT + 6.0,
            6.0,
            0x2400_0000,
        );
        if std::mem::take(&mut self.focus_next) {
            cx.ui.focus = Some(INPUT_ID);
            cx.ui.caret = self
                .drafts
                .get(&peer.key())
                .map_or(0, |d| d.chars().count());
        }
        let field_width = l.main_width - 2.0 * PAD - ICON_BUTTON - 6.0;
        let draft = self.drafts.entry(peer.key()).or_default();
        let hint = format!("Message @{}", peer.label);
        let r = [l.main_x + PAD, well_y, field_width, INPUT];
        form::field(
            cx.p,
            cx.ui,
            INPUT_ID,
            r,
            draft,
            &hint,
            online::service::MAX_TEXT,
            FieldKind::Plain,
        );
        let enter = cx.ui.focus == Some(INPUT_ID)
            && (cx.ui.key(keys::ENTER) || cx.ui.key(keys::KEYPAD_ENTER));
        let send_y = well_y + ((INPUT - ICON_BUTTON) / 2.0).floor();
        let send = icon_button(
            cx,
            l.main_x + l.main_width - PAD - ICON_BUTTON,
            send_y,
            Icon::Send,
            "Send",
            None,
        );
        if send || acted.is_some() {
            // A click on a button gives the field its focus back: with none, the Friends key would
            // close the menu as the player went on typing.
            cx.ui.focus = Some(INPUT_ID);
        }
        if send || enter {
            self.send();
        }
        if let Some(kind) = acted {
            self.act(cx, kind, now);
        }
    }

    /// Wraps the conversation into lines, grouping each run of one person's messages under one head.
    fn lay_out_lines(&mut self, p: &Painter, l: &Layout, peer: &Peer) {
        let text_width = l.main_width - 2.0 * PAD - 4.0 - AVATAR - 7.0 - 6.0;
        let revision = online::with(|o| o.social().revision());
        let key = (revision, text_width as i32, peer.key());
        if self.lines_key.as_ref() == Some(&key) {
            return;
        }
        self.lines_key = Some(key);
        self.lines.clear();
        let th = p.theme;
        let (me, messages, now, offset) = online::with(|o| {
            let me = o.account_name().unwrap_or("You").to_owned();
            let messages = o
                .social()
                .existing(peer)
                .map(|c| c.messages().to_vec())
                .unwrap_or_default();
            (me, messages, o.epoch(), online::utc_offset())
        });
        let mut y = 0.0;
        let mut last: Option<&online::social::Message> = None;
        for message in &messages {
            let grouped =
                last.is_some_and(|l| l.mine == message.mine && message.at - l.at < GROUP_MILLIS);
            if !grouped {
                if last.is_some() {
                    y += 6.0;
                }
                let name = if message.mine {
                    me.clone()
                } else {
                    peer.label.clone()
                };
                self.lines.push(ChatLine {
                    head: true,
                    name,
                    mine: message.mine,
                    time: time(message.at, now, offset),
                    text: String::new(),
                    color: 0,
                    y,
                });
                y += 11.0;
            }
            let color = match message.delivery {
                Delivery::Sent => 0xFFDC_E2E8,
                Delivery::Sending => alpha(th.muted, 0.8),
                Delivery::Failed => th.negative,
            };
            for text in p.wrap(&message.text, text_width, 1000) {
                self.lines.push(ChatLine {
                    head: false,
                    name: String::new(),
                    mine: message.mine,
                    time: String::new(),
                    text,
                    color,
                    y,
                });
                y += 10.0;
            }
            if message.delivery == Delivery::Failed {
                self.lines.push(ChatLine {
                    head: false,
                    name: String::new(),
                    mine: true,
                    time: String::new(),
                    text: "Not delivered".into(),
                    color: alpha(th.negative, 0.7),
                    y,
                });
                y += 10.0;
            }
            last = Some(message);
        }
        self.lines_height = y;
    }
}

fn draw_row(cx: &mut Cx, l: &Layout, row: &Row, y: f32, chosen: Option<&Peer>) {
    let th = cx.p.theme;
    let x = l.px + 5.0;
    let w = l.side - 10.0;
    let mouse = cx.ui.mouse;
    let hot = mouse[0] >= x
        && mouse[0] < x + w
        && mouse[1] >= y
        && mouse[1] < y + ROW
        && mouse[1] >= l.list_top
        && mouse[1] < l.list_bottom;
    let on = row
        .peer
        .as_ref()
        .zip(chosen)
        .is_some_and(|(a, b)| a.key() == b.key());
    if on || hot {
        cx.p.round(
            x,
            y + 1.0,
            w,
            ROW - 2.0,
            th.radius,
            if on { th.button_hot } else { th.button },
        );
    }
    let avatar_y = y + ((ROW - AVATAR) / 2.0).floor();
    avatars::draw(cx.p, x + 3.0, avatar_y, AVATAR, &row.label);
    if row.friend {
        avatars::status(
            cx.p,
            x + 3.0,
            avatar_y,
            AVATAR,
            row.online,
            if on || hot { 0xFF2A_333D } else { 0xFF20_272F },
        );
    }
    let text_x = x + 3.0 + AVATAR + 6.0;
    let mut right_edge = x + w - 4.0;
    if let Some(request) = &row.request {
        // Accept and Decline right on the row: one click, no screen between.
        let by = y + ((ROW - 12.0) / 2.0).floor();
        row_button(cx, Icon::Close, right_edge - 12.0, by, th.negative);
        if request.incoming {
            row_button(cx, Icon::Check, right_edge - 28.0, by, avatars::ONLINE);
            right_edge -= 30.0;
        } else {
            right_edge -= 14.0;
        }
    } else if row.unread > 0 {
        let count = if row.unread > 99 {
            "99+".to_owned()
        } else {
            row.unread.to_string()
        };
        let cw = cx.p.width(&count);
        let badge = (cw + 6.0).max(12.0);
        let by = y + ((ROW - 11.0) / 2.0).floor();
        cx.p.round(right_edge - badge, by, badge, 11.0, 5.0, BADGE_RED);
        cx.p.text(
            &count,
            right_edge - badge + ((badge - cw) / 2.0).floor() + 1.0,
            by + 2.0,
            th.text,
            false,
        );
        right_edge -= badge + 4.0;
    }
    let name_width = right_edge - text_x;
    let dim = !row.friend && row.request.is_none() && !row.online;
    let mut name_color = if on || row.unread > 0 {
        th.text
    } else if dim {
        alpha(th.muted, 0.7)
    } else {
        th.muted
    };
    if row.friend && row.online {
        name_color = if on { th.text } else { 0xFFE6_EBF0 };
    }
    let name = cx.p.fit(&row.label, name_width).into_owned();
    cx.p.text(&name, text_x, y + 3.0, name_color, false);
    let (detail, detail_color) = if !row.friend && row.request.is_none() && row.online {
        // A player here, not a friend: a small tag says where they're from.
        ("On this server".to_owned(), alpha(th.accent, 0.8))
    } else {
        (row.detail.clone(), alpha(th.muted, 0.75))
    };
    let detail = cx.p.fit(&detail, name_width / 0.75).into_owned();
    cx.p.text_scaled(&detail, text_x, y + 13.0, detail_color, false, 0.75);
}

fn row_button(cx: &mut Cx, icon: Icon, x: f32, y: f32, hot_color: u32) {
    let th = cx.p.theme;
    let hot = cx.ui.over([x, y, 12.0, 12.0]);
    cx.p.round(
        x,
        y,
        12.0,
        12.0,
        6.0,
        if hot {
            alpha(hot_color, 0.35)
        } else {
            0x2EFF_FFFF
        },
    );
    cx.p.icon(
        icon,
        x + 2.0,
        y + 2.0,
        8.0,
        if hot { hot_color } else { th.text },
    );
}

/// The player's own strip at the sidebar's foot: their account and whether they're connected.
fn draw_strip(p: &mut Painter, l: &Layout) {
    let th = p.theme;
    let y = l.py + l.ph - STRIP + ((STRIP - AVATAR) / 2.0).floor();
    let x = l.px + PAD;
    let (label, signed_in, state) = online::with(|o| {
        let label = o.account_name().unwrap_or("Not signed in").to_owned();
        let state = match o.status() {
            Status::SignedIn if o.social().messages_from() == MessagesFrom::Nobody => {
                "Messages off"
            }
            Status::SignedIn => "Online",
            Status::Connecting => "Connecting...",
            Status::SignedOut => "Signed out",
            Status::UpdateNeeded => "Update needed",
            Status::Off => "Unavailable",
        };
        (label, o.signed_in(), state)
    });
    avatars::draw(p, x, y, AVATAR, &label);
    avatars::status(p, x, y, AVATAR, signed_in, 0xFF1C_2229);
    let text_width = l.side - 2.0 * PAD - AVATAR - 6.0 - ICON_BUTTON - 4.0;
    let shown = p.fit(&label, text_width).into_owned();
    p.text(&shown, x + AVATAR + 6.0, y - 1.0, th.text, false);
    p.text_scaled(
        state,
        x + AVATAR + 6.0,
        y + 10.0,
        alpha(th.muted, 0.85),
        false,
        0.75,
    );
}

/// "Today at 14:02", "Yesterday at ...", or the date: in the player's time zone (`offset` ms
/// ahead of UTC), as of `now` (ms since 1970).
pub fn time(at: i64, now: i64, offset: i64) -> String {
    const DAY: i64 = 86_400_000;
    let (local_at, local_now) = (at + offset, now + offset);
    let (day, today) = (local_at.div_euclid(DAY), local_now.div_euclid(DAY));
    let minutes = local_at.rem_euclid(DAY) / 60_000;
    let clock = format!("{:02}:{:02}", minutes / 60, minutes % 60);
    if day == today {
        return format!("Today at {clock}");
    }
    if day == today - 1 {
        return format!("Yesterday at {clock}");
    }
    let (y, m, d) = civil(day);
    format!("{m}/{d}/{y} {clock}")
}

/// Days since 1970 to a calendar date (Howard Hinnant's `civil_from_days`).
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_read_as_today_yesterday_or_a_date() {
        // 2026-11-01 14:02 UTC.
        let at = 1_793_541_720_000;
        assert_eq!(time(at, at + 60_000, 0), "Today at 14:02");
        assert_eq!(time(at, at + 86_400_000, 0), "Yesterday at 14:02");
        assert_eq!(time(at, at + 3 * 86_400_000, 0), "11/1/2026 14:02");
        assert_eq!(
            time(at, at, 3_600_000),
            "Today at 15:02",
            "an hour ahead of UTC"
        );
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(-1), (1969, 12, 31));
    }

    #[test]
    fn the_menu_fits_a_small_window() {
        let l = Layout::of([320.0, 240.0]);
        assert_eq!(l.side, 118.0);
        assert!(l.px >= 0.0 && l.px + l.pw <= 320.0 && l.py + l.ph <= 240.0);
        assert!(l.list_top < l.list_bottom && l.chat_top < l.chat_bottom);
    }
}
