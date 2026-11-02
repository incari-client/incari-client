//! Friends, friend requests and conversations (ADR 0059; Social.java), as the client knows them:
//! the chat menu draws this, and the service keeps it current from answers and events.
//!
//! Conversations with friends are kept by the service and loaded a page at a time. Those with
//! other Incari players on the same server live only here and go when the player leaves, since a
//! `pid` means nothing on another server.
use std::collections::HashMap;

/// At most this many messages a conversation keeps; older ones load again on demand.
pub const MAX_KEPT: usize = 300;

/// Who may message the player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessagesFrom {
    /// Friends, and Incari players on the same server.
    Everyone,
    /// Friends only: the default.
    Friends,
    /// Nobody: messaging is off.
    Nobody,
}

impl MessagesFrom {
    pub const ALL: [MessagesFrom; 3] = [MessagesFrom::Everyone, MessagesFrom::Friends, MessagesFrom::Nobody];

    pub fn wire(self) -> &'static str {
        match self {
            MessagesFrom::Everyone => "everyone",
            MessagesFrom::Friends => "friends",
            MessagesFrom::Nobody => "nobody",
        }
    }

    /// What the settings call it.
    pub fn label(self) -> &'static str {
        match self {
            MessagesFrom::Everyone => "Friends and players on your server",
            MessagesFrom::Friends => "Friends only",
            MessagesFrom::Nobody => "Nobody (off)",
        }
    }

    pub fn of(wire: &str) -> Self {
        Self::ALL.into_iter().find(|m| m.wire() == wire).unwrap_or(MessagesFrom::Friends)
    }
}

/// A friend: `server` only while they're online and share it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Friend {
    pub fid: String,
    pub username: String,
    pub online: bool,
    pub server: Option<String>,
}

/// A friend request, either way: `label` is the sender's username for one received, the name it
/// was sent to for one sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub rid: String,
    pub incoming: bool,
    pub label: String,
}

/// Where a message is: sent, still going, or turned away.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    Sent,
    Sending,
    /// The service wouldn't deliver it.
    Failed,
}

/// One message in a conversation; `at` in ms since 1970.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub mid: String,
    pub mine: bool,
    pub text: String,
    pub at: i64,
    pub delivery: Delivery,
}

/// Who a conversation is with: a friend (by `fid`) or an Incari player on this server (by `pid`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Peer {
    pub friend: bool,
    pub id: String,
    pub label: String,
}

impl Peer {
    pub fn new(friend: bool, id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { friend, id: id.into(), label: label.into() }
    }

    /// The conversation's name: the same for one person whatever their label.
    pub fn key(&self) -> String {
        format!("{}{}", if self.friend { "f:" } else { "p:" }, self.id)
    }

    fn same(&self, other: &Peer) -> bool {
        self.friend == other.friend && self.id == other.id
    }
}

/// One conversation's messages, oldest first.
#[derive(Clone, Debug)]
pub struct Conversation {
    messages: Vec<Message>,
    unread: u32,
    /// Whether the service has older messages than the first here (friends only).
    more: bool,
    loading: bool,
    loaded: bool,
    /// Messages may have come while disconnected: the newest page loads again before anything is
    /// marked read.
    stale: bool,
}

impl Default for Conversation {
    fn default() -> Self {
        Self { messages: Vec::new(), unread: 0, more: true, loading: false, loaded: false, stale: false }
    }
}

impl Conversation {
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }
    pub fn unread(&self) -> u32 {
        self.unread
    }
    /// Whether older messages can be loaded.
    pub fn has_more(&self) -> bool {
        self.more
    }
    /// Whether a page is on its way.
    pub fn loading(&self) -> bool {
        self.loading
    }
    pub(crate) fn loaded(&self) -> bool {
        self.loaded
    }
    pub(crate) fn stale(&self) -> bool {
        self.stale
    }
    pub(crate) fn start_loading(&mut self) {
        self.loading = true;
    }

    fn find(&self, mid: &str) -> Option<usize> {
        self.messages.iter().rposition(|m| m.mid == mid)
    }

    /// Older messages arrived, oldest first.
    fn older(&mut self, page: Vec<Message>, more: bool) {
        self.loading = false;
        self.loaded = true;
        self.more = more;
        let mut merged: Vec<Message> = page.into_iter().filter(|m| self.find(&m.mid).is_none()).collect();
        merged.append(&mut self.messages);
        self.messages = merged;
    }

    /// The newest page again, after a reconnect: joined to what's held if they meet; if they
    /// don't, the page replaces them and scrolling up loads the rest again. The player's own
    /// unsent messages stay either way.
    fn newest(&mut self, page: Vec<Message>, more: bool) {
        self.loading = false;
        self.loaded = true;
        self.stale = false;
        let meets = self.messages.is_empty() || page.iter().any(|m| self.find(&m.mid).is_some());
        let mut merged = Vec::with_capacity(page.len() + self.messages.len());
        if meets {
            if self.messages.is_empty() {
                self.more = more;
            }
            let fresh: Vec<Message> = page.into_iter().filter(|m| self.find(&m.mid).is_none()).collect();
            merged.append(&mut self.messages);
            merged.extend(fresh);
        } else {
            self.more = more;
            merged.extend(page);
            merged.extend(self.messages.drain(..).filter(|m| m.delivery != Delivery::Sent));
        }
        merged.sort_by_key(|m| m.at);
        if merged.len() > MAX_KEPT {
            merged.drain(..merged.len() - MAX_KEPT);
            self.more = true;
        }
        self.messages = merged;
    }

    /// A new message at the end; drops the oldest past `MAX_KEPT`.
    fn add(&mut self, message: Message) {
        if self.find(&message.mid).is_some() {
            return;
        }
        self.messages.push(message);
        if self.messages.len() > MAX_KEPT {
            self.messages.remove(0);
            self.more = true;
        }
    }

    fn replace(&mut self, mid: &str, message: Message) {
        if let Some(at) = self.find(mid) {
            self.messages[at] = message;
        }
    }
}

/// Something to tell the player about while the chat menu is closed: a toast. `request` for
/// friends (a request, a new friend) rather than a message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub who: String,
    pub request: bool,
    pub title: String,
    pub text: String,
}

#[derive(Default)]
pub struct Social {
    available: Option<bool>,
    loaded: bool,
    /// In the order they came (the Java client's LinkedHashMaps).
    friends: Vec<Friend>,
    requests: Vec<Request>,
    conversations: HashMap<String, Conversation>,
    /// The names of the Incari players on this server whose conversations are here, by pid.
    player_names: Vec<(String, String)>,
    /// The pid each request sent to a player on this server went to, by rid.
    sent_to_pid: HashMap<String, String>,
    notices: Vec<Notice>,
    messages_from: Option<MessagesFrom>,
    friend_requests_off: bool,
    revision: u64,
}

impl Social {
    // ---------------------------------------------------------------------------------------------
    // What the chat menu reads
    // ---------------------------------------------------------------------------------------------

    /// Whether the service has friends and messages: None until it answers.
    pub fn available(&self) -> Option<bool> {
        self.available
    }

    /// Whether the friend list has arrived since signing in.
    pub fn loaded(&self) -> bool {
        self.loaded
    }

    /// Every friend: online ones first, then by name.
    pub fn friends(&self) -> Vec<Friend> {
        let mut list = self.friends.clone();
        list.sort_by(|a, b| b.online.cmp(&a.online).then_with(|| case_insensitive(&a.username, &b.username)));
        list
    }

    pub fn friend(&self, fid: &str) -> Option<&Friend> {
        self.friends.iter().find(|f| f.fid == fid)
    }

    /// Friend requests: received ones first, newest last within each.
    pub fn requests(&self) -> Vec<Request> {
        let mut list: Vec<Request> = self.requests.iter().filter(|r| r.incoming).cloned().collect();
        list.extend(self.requests.iter().filter(|r| !r.incoming).cloned());
        list
    }

    /// The request the player sent to `peer`, an Incari player on this server, still waiting.
    /// Never one received (its label is a username, which says nothing about who they are here).
    /// Known by `pid` when sent from here, else by the name it was sent to.
    pub fn sent_to(&self, peer: &Peer) -> Option<&Request> {
        if peer.friend {
            return None;
        }
        self.requests
            .iter()
            .find(|r| !r.incoming && self.sent_to_pid.get(&r.rid) == Some(&peer.id))
            .or_else(|| {
                self.requests.iter().find(|r| {
                    !r.incoming && !self.sent_to_pid.contains_key(&r.rid) && r.label.eq_ignore_ascii_case(&peer.label)
                })
            })
    }

    /// The conversation with `peer`, or None if nothing was said yet.
    pub fn existing(&self, peer: &Peer) -> Option<&Conversation> {
        self.conversations.get(&peer.key())
    }

    pub(crate) fn conversation(&mut self, peer: &Peer) -> &mut Conversation {
        self.conversations.entry(peer.key()).or_default()
    }

    /// Incari players on this server who messaged the player, as (pid, name): they stay listed
    /// after leaving, until the player does.
    pub fn players_messaged(&self) -> &[(String, String)] {
        &self.player_names
    }

    /// Unread messages across every conversation.
    pub fn unread(&self) -> u32 {
        self.conversations.values().map(|c| c.unread).sum()
    }

    /// Received friend requests waiting for an answer.
    pub fn pending_requests(&self) -> u32 {
        self.requests.iter().filter(|r| r.incoming).count() as u32
    }

    pub fn messages_from(&self) -> MessagesFrom {
        self.messages_from.unwrap_or(MessagesFrom::Friends)
    }

    /// Whether others can send the player friend requests.
    pub fn friend_requests(&self) -> bool {
        !self.friend_requests_off
    }

    /// What happened since the last call, to show as toasts.
    pub fn take_notices(&mut self) -> Vec<Notice> {
        std::mem::take(&mut self.notices)
    }

    /// Goes up whenever anything here changes.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    // ---------------------------------------------------------------------------------------------
    // What the service changes
    // ---------------------------------------------------------------------------------------------

    pub(crate) fn settings(&mut self, from: MessagesFrom, requests: bool) {
        self.messages_from = Some(from);
        self.friend_requests_off = !requests;
        self.revision += 1;
    }

    pub(crate) fn set_available(&mut self, yes: bool) {
        self.available = Some(yes);
        self.revision += 1;
    }

    /// The full list after signing in (and each reconnect): replaces what was here. Friends'
    /// conversations still held may have missed messages: marked stale, their newest page loads
    /// again when next shown. Unread counts are the service's.
    pub(crate) fn set_loaded(&mut self, friends: Vec<Friend>, requests: Vec<Request>, unread: Vec<(String, u32)>) {
        self.loaded = true;
        self.friends.clear();
        for f in friends {
            self.put_friend(f);
        }
        self.requests.clear();
        for r in requests {
            self.put_request(r);
        }
        let rids: Vec<String> = self.requests.iter().map(|r| r.rid.clone()).collect();
        self.sent_to_pid.retain(|rid, _| rids.contains(rid));
        let fids: Vec<String> = self.friends.iter().map(|f| f.fid.clone()).collect();
        self.conversations.retain(|key, c| {
            let Some(fid) = key.strip_prefix("f:") else {
                return true;
            };
            if !fids.iter().any(|f| f == fid) {
                // Unfriended while the client wasn't connected: the conversation went with it.
                return false;
            }
            c.stale = true;
            c.unread = 0;
            true
        });
        for (fid, count) in unread {
            self.conversation(&Peer::new(true, fid, "")).unread = count;
        }
        self.revision += 1;
    }

    fn put_friend(&mut self, friend: Friend) -> bool {
        match self.friends.iter_mut().find(|f| f.fid == friend.fid) {
            Some(old) => {
                *old = friend;
                false
            }
            None => {
                self.friends.push(friend);
                true
            }
        }
    }

    fn put_request(&mut self, request: Request) -> bool {
        match self.requests.iter_mut().find(|r| r.rid == request.rid) {
            Some(old) => {
                *old = request;
                false
            }
            None => {
                self.requests.push(request);
                true
            }
        }
    }

    pub(crate) fn friend_changed(&mut self, friend: Friend, announce: bool) {
        let name = friend.username.clone();
        if self.put_friend(friend) && announce {
            self.notices.push(Notice {
                who: name.clone(),
                request: true,
                title: "New friend".into(),
                text: format!("{name} is now your friend."),
            });
        }
        self.revision += 1;
    }

    pub(crate) fn unfriended(&mut self, fid: &str) {
        let before = self.friends.len();
        self.friends.retain(|f| f.fid != fid);
        if self.friends.len() != before {
            self.conversations.remove(&format!("f:{fid}"));
            self.revision += 1;
        }
    }

    pub(crate) fn request(&mut self, request: Request) {
        let (label, incoming) = (request.label.clone(), request.incoming);
        if self.put_request(request) && incoming {
            self.notices.push(Notice {
                who: label.clone(),
                request: true,
                title: label,
                text: "Sent you a friend request".into(),
            });
        }
        self.revision += 1;
    }

    /// Request `rid` went to the Incari player here with this pid.
    pub(crate) fn request_sent_to(&mut self, rid: &str, pid: &str) {
        self.sent_to_pid.insert(rid.to_owned(), pid.to_owned());
        self.revision += 1;
    }

    pub(crate) fn request_gone(&mut self, rid: &str) {
        self.sent_to_pid.remove(rid);
        let before = self.requests.len();
        self.requests.retain(|r| r.rid != rid);
        if self.requests.len() != before {
            self.revision += 1;
        }
    }

    /// A message arrived, or one of the player's was sent.
    pub(crate) fn message(&mut self, peer: &Peer, message: Message, viewing: bool) {
        let (mine, text) = (message.mine, message.text.clone());
        let conversation = self.conversation(peer);
        conversation.add(message);
        if !mine && !viewing {
            conversation.unread += 1;
        }
        if !peer.friend {
            match self.player_names.iter_mut().find(|(pid, _)| *pid == peer.id) {
                Some(entry) => entry.1 = peer.label.clone(),
                None => self.player_names.push((peer.id.clone(), peer.label.clone())),
            }
        }
        if !mine && !viewing {
            self.notices.push(Notice { who: peer.label.clone(), request: false, title: peer.label.clone(), text });
        }
        self.revision += 1;
    }

    pub(crate) fn replace(&mut self, peer: &Peer, mid: &str, message: Message) {
        self.conversation(peer).replace(mid, message);
        self.revision += 1;
    }

    pub(crate) fn older(&mut self, peer: &Peer, page: Vec<Message>, more: bool) {
        self.conversation(peer).older(page, more);
        self.revision += 1;
    }

    pub(crate) fn newest(&mut self, peer: &Peer, page: Vec<Message>, more: bool) {
        self.conversation(peer).newest(page, more);
        self.revision += 1;
    }

    pub(crate) fn load_failed(&mut self, peer: &Peer) {
        self.conversation(peer).loading = false;
        self.revision += 1;
    }

    pub(crate) fn read(&mut self, peer: &Peer) {
        if let Some(c) = self.conversations.get_mut(&peer.key())
            && c.unread > 0
        {
            c.unread = 0;
            self.revision += 1;
        }
    }

    /// The player left the server: conversations with its players go, their ids meaning nothing now.
    pub(crate) fn left_server(&mut self) {
        if self.player_names.is_empty() {
            return;
        }
        for (pid, _) in self.player_names.drain(..) {
            self.conversations.remove(&format!("p:{pid}"));
        }
        self.revision += 1;
    }

    /// Signed out: everything goes.
    pub(crate) fn clear(&mut self) {
        let revision = self.revision + 1;
        *self = Self::default();
        self.revision = revision;
    }

    pub(crate) fn same_peer(a: &Peer, b: &Peer) -> bool {
        a.same(b)
    }
}

/// Java's `String.CASE_INSENSITIVE_ORDER`, for the names it sorts (ASCII usernames).
pub fn case_insensitive(a: &str, b: &str) -> std::cmp::Ordering {
    a.chars()
        .map(|c| c.to_lowercase().next().unwrap_or(c))
        .cmp(b.chars().map(|c| c.to_lowercase().next().unwrap_or(c)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sent(mid: &str, at: i64) -> Message {
        Message { mid: mid.into(), mine: false, text: mid.into(), at, delivery: Delivery::Sent }
    }

    #[test]
    fn friends_online_first_then_by_name() {
        let mut s = Social::default();
        let f = |fid: &str, name: &str, online| Friend { fid: fid.into(), username: name.into(), online, server: None };
        s.set_loaded(vec![f("1", "zed", false), f("2", "Bob", true), f("3", "amy", false)], vec![], vec![]);
        let names: Vec<String> = s.friends().into_iter().map(|f| f.username).collect();
        assert_eq!(names, ["Bob", "amy", "zed"]);
    }

    #[test]
    fn a_message_while_not_viewing_counts_and_toasts() {
        let mut s = Social::default();
        let peer = Peer::new(false, "p1", "Steve");
        s.message(&peer, sent("m1", 1), false);
        assert_eq!(s.unread(), 1);
        assert_eq!(s.take_notices()[0].text, "m1");
        s.message(&peer, sent("m1", 1), false);
        assert_eq!(s.existing(&peer).unwrap().messages().len(), 1, "repeats are dropped");
        s.read(&peer);
        assert_eq!(s.unread(), 0);
        assert_eq!(s.players_messaged(), [("p1".to_owned(), "Steve".to_owned())]);
        s.left_server();
        assert!(s.existing(&peer).is_none());
    }

    #[test]
    fn newest_page_after_a_reconnect_joins_or_replaces() {
        let mut s = Social::default();
        let peer = Peer::new(true, "f1", "Alex");
        s.older(&peer, vec![sent("a", 1), sent("b", 2)], false);
        s.newest(&peer, vec![sent("b", 2), sent("c", 3)], true);
        let mids: Vec<&str> = s.existing(&peer).unwrap().messages().iter().map(|m| m.mid.as_str()).collect();
        assert_eq!(mids, ["a", "b", "c"]);
        assert!(!s.existing(&peer).unwrap().has_more(), "met: what's held still reaches the start");
        s.newest(&peer, vec![sent("x", 10), sent("y", 11)], true);
        let mids: Vec<&str> = s.existing(&peer).unwrap().messages().iter().map(|m| m.mid.as_str()).collect();
        assert_eq!(mids, ["x", "y"]);
        assert!(s.existing(&peer).unwrap().has_more());
    }

    #[test]
    fn sent_to_by_pid_or_by_name() {
        let mut s = Social::default();
        s.request(Request { rid: "r1".into(), incoming: false, label: "Steve".into() });
        let steve = Peer::new(false, "p9", "steve");
        assert_eq!(s.sent_to(&steve).map(|r| r.rid.as_str()), Some("r1"));
        s.request_sent_to("r1", "p1");
        assert!(s.sent_to(&steve).is_none(), "known by pid now");
        assert!(s.sent_to(&Peer::new(false, "p1", "anyone")).is_some());
        s.request(Request { rid: "r2".into(), incoming: true, label: "Alex".into() });
        assert_eq!(s.pending_requests(), 1);
        assert_eq!(s.requests()[0].rid, "r2", "received first");
        assert_eq!(s.take_notices().len(), 1, "only the received one toasts");
    }

    #[test]
    fn keeps_at_most_max_kept() {
        let mut s = Social::default();
        let peer = Peer::new(true, "f", "A");
        for i in 0..MAX_KEPT + 5 {
            s.message(&peer, sent(&i.to_string(), i as i64), true);
        }
        let c = s.existing(&peer).unwrap();
        assert_eq!(c.messages().len(), MAX_KEPT);
        assert_eq!(c.messages()[0].mid, "5");
    }
}
