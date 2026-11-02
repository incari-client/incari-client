//! Incari's service in the test, speaking `docs/online-protocol.md` as the Java `FakeService`
//! (online's test fixtures, also served by `tools/onlinedev`) does, closely enough to drive
//! `Online` through every request. Not a model of the backend's security.
use super::rules;
use super::service::{Effect, Online};
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};

pub const BITS: u32 = 8;

#[derive(Clone)]
pub struct Account {
    pub username: String,
    pub password: String,
    pub tokens: Vec<String>,
    pub codes: Vec<String>,
    pub share_server: bool,
    pub messages: String,
    pub friend_requests: bool,
    pub badge: bool,
}

#[derive(Default)]
pub struct Socket {
    pub open: bool,
    pub account: Option<String>,
    pub greeted: bool,
    pub outbox: Vec<String>,
    pub place: Option<(String, String, Option<String>)>,
    pub looked_up: HashSet<String>,
}

#[derive(Default)]
pub struct Fake {
    pub accounts: HashMap<String, Account>,
    pub sockets: Vec<Socket>,
    /// Every message the clients sent.
    pub received: Vec<Value>,
    pub protocol: u32,
    pub release: Option<Value>,
    pub featured: Option<Value>,
    /// Request types it takes and never answers.
    pub unanswered: HashSet<String>,
    /// The next request of this type gets this error code.
    pub fail_next: Option<(String, String)>,
    /// Friendships as (fid, key a, key b).
    pub friendships: Vec<(String, String, String)>,
    /// Requests as (rid, from key, to key, label).
    pub requests: Vec<(String, String, String, String)>,
    /// Friends' messages as (fid, sender key, text, mid, at).
    pub messages: Vec<(String, String, String, String, i64)>,
    /// Blocks as (bid, blocker key, blocked key, label).
    pub blocks: Vec<(String, String, String, String)>,
    pub reports: Vec<Value>,
    challenges: HashSet<String>,
    ids: u32,
    now: i64,
}

impl Fake {
    pub fn new() -> Self {
        Self { protocol: 1, now: 1_700_000_000_000, ..Default::default() }
    }

    fn id(&mut self, prefix: &str) -> String {
        self.ids += 1;
        format!("{prefix}{}", self.ids)
    }

    pub fn open(&mut self) -> usize {
        self.sockets.push(Socket { open: true, ..Default::default() });
        self.sockets.len() - 1
    }

    pub fn close(&mut self, s: usize) {
        self.sockets[s].open = false;
        self.sockets[s].account = None;
        self.sockets[s].place = None;
    }

    fn push(&mut self, s: usize, message: Value) {
        if self.sockets[s].open {
            self.sockets[s].outbox.push(message.to_string());
        }
    }

    /// Sends `message` to every signed-in socket of account `key`.
    fn send_to_account(&mut self, key: &str, message: &Value, except: Option<usize>) {
        for s in 0..self.sockets.len() {
            if Some(s) != except && self.sockets[s].account.as_deref() == Some(key) {
                self.push(s, message.clone());
            }
        }
    }

    fn pid(hash: &str, key: &str) -> String {
        format!("p-{}-{key}", &hash[..6])
    }

    fn blocked(&self, a: &str, b: &str) -> bool {
        self.blocks.iter().any(|(_, x, y, _)| (x == a && y == b) || (x == b && y == a))
    }

    fn friendship(&self, a: &str, b: &str) -> Option<String> {
        self.friendships
            .iter()
            .find(|(_, x, y, ..)| (x == a && y == b) || (x == b && y == a))
            .map(|f| f.0.clone())
    }

    fn other(&self, fid: &str, me: &str) -> Option<String> {
        let (_, a, b) = self.friendships.iter().find(|f| f.0 == fid)?;
        Some(if a == me { b.clone() } else { a.clone() })
    }

    fn online(&self, key: &str) -> bool {
        self.sockets.iter().any(|s| s.account.as_deref() == Some(key))
    }

    fn friend_json(&self, fid: &str, other: &str) -> Value {
        let acc = &self.accounts[other];
        let online = self.online(other);
        let server = self
            .sockets
            .iter()
            .filter(|s| s.account.as_deref() == Some(other))
            .find_map(|s| s.place.as_ref().and_then(|p| p.2.clone()))
            .filter(|_| online && acc.share_server);
        json!({"fid": fid, "username": acc.username, "online": online, "server": server})
    }

    fn account_json(&self, key: &str) -> Value {
        let a = &self.accounts[key];
        json!({"username": a.username, "settings": {
            "shareServer": a.share_server, "messages": a.messages,
            "friendRequests": a.friend_requests, "badge": a.badge}})
    }

    fn codes() -> Vec<String> {
        (0..10).map(|i| format!("ABCD-EFGH-{:04}", i)).collect()
    }

    fn sign_in(&mut self, s: usize, key: &str) -> String {
        let token = self.id("token-");
        self.accounts.get_mut(key).unwrap().tokens.push(token.clone());
        self.sockets[s].account = Some(key.to_owned());
        token
    }

    /// Ends every session of `key` but socket `keep`'s.
    fn end_sessions(&mut self, key: &str, keep: Option<usize>, keep_token: Option<String>) {
        let tokens = &mut self.accounts.get_mut(key).unwrap().tokens;
        tokens.retain(|t| Some(t) == keep_token.as_ref());
        for s in 0..self.sockets.len() {
            if Some(s) != keep && self.sockets[s].account.as_deref() == Some(key) {
                self.push(s, json!({"t": "signedout"}));
                self.sockets[s].account = None;
            }
        }
    }

    /// A message from a client, answered into socket outboxes.
    pub fn handle(&mut self, s: usize, text: &str) {
        let m: Value = serde_json::from_str(text).unwrap();
        self.received.push(m.clone());
        let t = m["t"].as_str().unwrap().to_owned();
        let id = m["id"].clone();
        if self.unanswered.contains(&t) {
            return;
        }
        if let Some((kind, code)) = self.fail_next.clone()
            && kind == t
        {
            self.fail_next = None;
            return self.push(s, json!({"t": "err", "id": id, "code": code}));
        }
        let answer = self.answer(s, &t, &m);
        let mut out = match answer {
            Ok(fields) => {
                let mut o = fields;
                o.insert("t".into(), json!("ok"));
                Value::Object(o)
            }
            Err(code) => json!({"t": "err", "code": code}),
        };
        out["id"] = id;
        self.push(s, out);
    }

    fn me(&self, s: usize) -> Result<String, &'static str> {
        self.sockets[s].account.clone().ok_or("no_session")
    }

    fn answer(&mut self, s: usize, t: &str, m: &Value) -> Result<Map<String, Value>, &'static str> {
        let str_of = |k: &str| m.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
        let mut out = Map::new();
        if t != "hello" && !self.sockets[s].greeted {
            return Err("bad_request");
        }
        match t {
            "hello" => {
                if m["v"].as_u64() != Some(u64::from(self.protocol)) {
                    return Err("update");
                }
                self.sockets[s].greeted = true;
                out.insert("v".into(), json!(self.protocol));
                if let Some(r) = &self.release {
                    out.insert("release".into(), r.clone());
                }
                if let Some(f) = &self.featured {
                    out.insert("featured".into(), f.clone());
                }
            }
            "count" => {}
            "challenge" => {
                self.ids += 1;
                let bytes: Vec<u8> = (0..16).map(|i| (i as u32 * 37 + self.ids) as u8).collect();
                let text = base64url(&bytes);
                self.challenges.insert(text.clone());
                out.insert("challenge".into(), json!(text));
                out.insert("bits".into(), json!(BITS));
            }
            "signup" => {
                let (username, password, challenge) = (str_of("username"), str_of("password"), str_of("challenge"));
                let nonce = m["nonce"].as_u64().ok_or("bad_request")?;
                let bytes = rules::base64_url(&challenge).ok_or("bad_request")?;
                let mut work = rules::ProofOfWork::new(&bytes, BITS).ok_or("bad_request")?;
                if !self.challenges.contains(&challenge) || !work.passes(nonce) {
                    return Err("bad_request");
                }
                if rules::username_problem(&username).is_some() || rules::password_problem(&password, &username).is_some() {
                    return Err("weak_password");
                }
                let key = username.to_lowercase();
                if self.accounts.contains_key(&key) {
                    return Err("taken");
                }
                self.challenges.remove(&challenge);
                self.accounts.insert(key.clone(), Account {
                    username,
                    password,
                    tokens: vec![],
                    codes: Self::codes(),
                    share_server: true,
                    messages: "friends".into(),
                    friend_requests: true,
                    badge: true,
                });
                let token = self.sign_in(s, &key);
                out.insert("token".into(), json!(token));
                out.insert("recoveryCodes".into(), json!(Self::codes().iter().map(|c| c.to_lowercase()).collect::<Vec<_>>()));
                out.insert("account".into(), self.account_json(&key));
            }
            "signin" => {
                let key = str_of("username").to_lowercase();
                if self.accounts.get(&key).is_none_or(|a| a.password != str_of("password")) {
                    return Err("unauthorized");
                }
                let token = self.sign_in(s, &key);
                out.insert("token".into(), json!(token));
                out.insert("account".into(), self.account_json(&key));
            }
            "resume" => {
                let token = str_of("token");
                let key = self
                    .accounts
                    .iter()
                    .find(|(_, a)| a.tokens.contains(&token))
                    .map(|(k, _)| k.clone())
                    .ok_or("unauthorized")?;
                self.sockets[s].account = Some(key.clone());
                out.insert("account".into(), self.account_json(&key));
            }
            "recover" => {
                let key = str_of("username").to_lowercase();
                let code = str_of("code");
                let ok = self.accounts.get(&key).is_some_and(|a| a.codes.contains(&code));
                if !ok {
                    return Err("unauthorized");
                }
                let a = self.accounts.get_mut(&key).unwrap();
                a.codes.retain(|c| *c != code);
                a.password = str_of("newPassword");
                self.end_sessions(&key, None, None);
                let token = self.sign_in(s, &key);
                out.insert("token".into(), json!(token));
                out.insert("account".into(), self.account_json(&key));
            }
            "password" => {
                let key = self.me(s)?;
                if self.accounts[&key].password != str_of("old") {
                    return Err("unauthorized");
                }
                self.accounts.get_mut(&key).unwrap().password = str_of("new");
                let keep = self.accounts[&key].tokens.last().cloned();
                self.end_sessions(&key, Some(s), keep);
            }
            "codes" => {
                let key = self.me(s)?;
                if self.accounts[&key].password != str_of("password") {
                    return Err("unauthorized");
                }
                out.insert("recoveryCodes".into(), json!(Self::codes()));
            }
            "signout" => {
                let key = self.me(s)?;
                if m["everywhere"].as_bool() == Some(true) {
                    self.end_sessions(&key, Some(s), None);
                }
                self.sockets[s].account = None;
            }
            "delete" => {
                let key = self.me(s)?;
                if self.accounts[&key].password != str_of("password") {
                    return Err("unauthorized");
                }
                self.end_sessions(&key, Some(s), None);
                self.accounts.remove(&key);
                self.sockets[s].account = None;
            }
            "settings" => {
                let key = self.me(s)?;
                let a = self.accounts.get_mut(&key).unwrap();
                if let Some(v) = m["shareServer"].as_bool() {
                    a.share_server = v;
                }
                if let Some(v) = m["messages"].as_str() {
                    a.messages = v.to_owned();
                }
                if let Some(v) = m["friendRequests"].as_bool() {
                    a.friend_requests = v;
                }
                if let Some(v) = m["badge"].as_bool() {
                    a.badge = v;
                }
            }
            "presence" => {
                let key = self.me(s)?;
                match m["serverHash"].as_str() {
                    None => self.sockets[s].place = None,
                    Some(hash) => {
                        let name = str_of("name");
                        self.sockets[s].place = Some((hash.to_owned(), name.clone(), m["server"].as_str().map(str::to_owned)));
                        // Whoever looked this name up on the same server hears of it.
                        let badge = self.accounts[&key].badge;
                        for other in 0..self.sockets.len() {
                            let same = self.sockets[other].place.as_ref().is_some_and(|p| p.0 == hash);
                            if other != s && same && self.sockets[other].looked_up.contains(&name) {
                                let them = self.sockets[other].account.clone().unwrap_or_default();
                                if !self.blocked(&key, &them) {
                                    let friend = self.friendship(&key, &them).is_some();
                                    let seen = json!({"t": "seen", "name": name, "pid": Self::pid(hash, &key),
                                        "friend": friend, "cosmetics": [], "badge": badge});
                                    self.push(other, seen);
                                }
                            }
                        }
                    }
                }
            }
            "lookup" => {
                let me = self.me(s)?;
                let hash = self.sockets[s].place.as_ref().map(|p| p.0.clone()).ok_or("bad_request")?;
                let mut players = Vec::new();
                for name in m["names"].as_array().unwrap().iter().filter_map(Value::as_str) {
                    self.sockets[s].looked_up.insert(name.to_owned());
                    for other in 0..self.sockets.len() {
                        let Some(key) = self.sockets[other].account.clone() else { continue };
                        let here = self.sockets[other].place.as_ref().is_some_and(|p| p.0 == hash && p.1 == name);
                        if here && key != me && !self.blocked(&key, &me) {
                            players.push(json!({"name": name, "pid": Self::pid(&hash, &key),
                                "friend": self.friendship(&key, &me).is_some(), "cosmetics": [],
                                "badge": self.accounts[&key].badge}));
                        }
                    }
                }
                out.insert("players".into(), json!(players));
            }
            "wardrobe" | "equip" | "emote" => return Err("bad_request"),
            "friends" => {
                let me = self.me(s)?;
                let mut friends = Vec::new();
                for (fid, a, b) in self.friendships.clone() {
                    if a == me || b == me {
                        let other = if a == me { b } else { a };
                        let mut f = self.friend_json(&fid, &other);
                        let unread = self.messages.iter().filter(|x| x.0 == fid && x.1 != me).count();
                        f["unread"] = json!(unread);
                        friends.push(f);
                    }
                }
                let requests: Vec<Value> = self
                    .requests
                    .iter()
                    .filter(|r| r.1 == me || r.2 == me)
                    .map(|r| self.request_json(r, &me))
                    .collect();
                out.insert("friends".into(), json!(friends));
                out.insert("requests".into(), json!(requests));
            }
            "befriend" => {
                let me = self.me(s)?;
                let (to, label) = if let Some(pid) = m["pid"].as_str() {
                    (pid.rsplit('-').next().unwrap_or("").to_owned(), str_of("name"))
                } else {
                    let key = str_of("username").to_lowercase();
                    if !self.accounts.contains_key(&key) || key == me {
                        return Err("bad_request");
                    }
                    (key, str_of("username"))
                };
                if let Some(back) = self.requests.iter().position(|r| r.1 == to && r.2 == me) {
                    // Crossing requests make a friendship.
                    let (rid, ..) = self.requests.remove(back);
                    let fid = self.id("f");
                    self.friendships.push((fid.clone(), me.clone(), to.clone()));
                    self.send_to_account(&to, &json!({"t": "requestgone", "rid": rid}), None);
                    let mine = self.friend_json(&fid, &me);
                    self.send_to_account(&to, &json!({"t": "friend", "friend": mine}), None);
                    out.insert("friend".into(), self.friend_json(&fid, &to));
                    return Ok(out);
                }
                if let Some(r) = self.requests.iter().find(|r| r.1 == me && r.2 == to) {
                    out.insert("request".into(), self.request_json(&r.clone(), &me));
                    return Ok(out);
                }
                let rid = self.id("r");
                let r = (rid, me.clone(), to.clone(), label);
                self.requests.push(r.clone());
                if self.accounts[&to].friend_requests && !self.blocked(&me, &to) {
                    let incoming = self.request_json(&r, &to);
                    let mut event = incoming.as_object().unwrap().clone();
                    event.insert("t".into(), json!("request"));
                    self.send_to_account(&to, &Value::Object(event), None);
                }
                out.insert("request".into(), self.request_json(&r, &me));
            }
            "accept" | "decline" => {
                let me = self.me(s)?;
                let rid = str_of("rid");
                let at = self.requests.iter().position(|r| r.0 == rid && (r.2 == me || (t == "decline" && r.1 == me)));
                let (rid, from, to, _) = self.requests.remove(at.ok_or("bad_request")?);
                let other = if from == me { to } else { from };
                self.send_to_account(&other, &json!({"t": "requestgone", "rid": rid}), None);
                if t == "accept" {
                    let fid = self.id("f");
                    self.friendships.push((fid.clone(), me.clone(), other.clone()));
                    let mine = self.friend_json(&fid, &me);
                    self.send_to_account(&other, &json!({"t": "friend", "friend": mine}), None);
                    out.insert("friend".into(), self.friend_json(&fid, &other));
                }
            }
            "unfriend" => {
                let me = self.me(s)?;
                let fid = str_of("fid");
                let other = self.other(&fid, &me).ok_or("bad_request")?;
                self.friendships.retain(|f| f.0 != fid);
                self.send_to_account(&other, &json!({"t": "unfriended", "fid": fid}), None);
            }
            "send" => {
                let me = self.me(s)?;
                let text = str_of("text").trim().to_owned();
                if text.is_empty() {
                    return Err("bad_request");
                }
                let mid = self.id("m");
                self.now += 1000;
                let at = self.now;
                if let Some(fid) = m["fid"].as_str() {
                    let other = self.other(fid, &me).ok_or("closed")?;
                    if self.accounts[&other].messages == "nobody" {
                        return Err("closed");
                    }
                    self.messages.push((fid.to_owned(), me.clone(), text.clone(), mid.clone(), at));
                    let event = json!({"t": "message", "mid": mid, "at": at, "text": text, "fid": fid});
                    self.send_to_account(&other, &event, None);
                } else {
                    let to = str_of("pid").rsplit('-').next().unwrap_or("").to_owned();
                    let open = self.accounts.get(&to).is_some_and(|a| a.messages == "everyone");
                    if !open || self.blocked(&me, &to) {
                        return Err("closed");
                    }
                    let place = self.sockets[s].place.clone().ok_or("closed")?;
                    let event = json!({"t": "message", "mid": mid, "at": at, "text": text,
                        "pid": Self::pid(&place.0, &me), "name": place.1});
                    self.send_to_account(&to, &event, None);
                }
                out.insert("mid".into(), json!(mid));
                out.insert("at".into(), json!(at));
            }
            "history" => {
                let me = self.me(s)?;
                let fid = str_of("fid");
                self.other(&fid, &me).ok_or("bad_request")?;
                let all: Vec<_> = self.messages.iter().filter(|x| x.0 == fid).collect();
                let end = match m["before"].as_str() {
                    Some(before) => all.iter().position(|x| x.3 == before).unwrap_or(all.len()),
                    None => all.len(),
                };
                let start = end.saturating_sub(50);
                let page: Vec<Value> = all[start..end]
                    .iter()
                    .map(|x| json!({"mid": x.3, "from": if x.1 == me { "me" } else { "them" }, "text": x.2, "at": x.4}))
                    .collect();
                out.insert("messages".into(), json!(page));
                out.insert("more".into(), json!(start > 0));
            }
            "read" => {}
            "block" => {
                let me = self.me(s)?;
                let (them, label) = if let Some(pid) = m["pid"].as_str() {
                    (pid.rsplit('-').next().unwrap_or("").to_owned(), str_of("name"))
                } else {
                    (str_of("username").to_lowercase(), str_of("username"))
                };
                if let Some(fid) = self.friendship(&me, &them) {
                    self.friendships.retain(|f| f.0 != fid);
                    self.send_to_account(&them, &json!({"t": "unfriended", "fid": fid}), None);
                    self.send_to_account(&me, &json!({"t": "unfriended", "fid": fid}), Some(s));
                }
                let bid = self.id("b");
                self.blocks.push((bid, me, them, label));
            }
            "blocks" => {
                let me = self.me(s)?;
                let list: Vec<Value> =
                    self.blocks.iter().filter(|b| b.1 == me).map(|b| json!({"bid": b.0, "label": b.3})).collect();
                out.insert("blocks".into(), json!(list));
            }
            "unblock" => {
                let me = self.me(s)?;
                let bid = str_of("bid");
                self.blocks.retain(|b| !(b.0 == bid && b.1 == me));
            }
            "report" => {
                self.me(s)?;
                self.reports.push(m.clone());
            }
            _ => return Err("bad_request"),
        }
        Ok(out)
    }

    fn request_json(&self, r: &(String, String, String, String), viewer: &str) -> Value {
        let incoming = r.2 == viewer;
        let label = if incoming { self.accounts[&r.1].username.clone() } else { r.3.clone() };
        json!({"rid": r.0, "incoming": incoming, "label": label})
    }
}

fn base64url(bytes: &[u8]) -> String {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    let (mut bits, mut count) = (0u32, 0);
    for &b in bytes {
        bits = bits << 8 | u32::from(b);
        count += 8;
        while count >= 6 {
            count -= 6;
            out.push(A[(bits >> count & 63) as usize] as char);
        }
    }
    if count > 0 {
        out.push(A[(bits << (6 - count) & 63) as usize] as char);
    }
    out
}

/// A client of the fake: the service, its socket, its session file and its clock.
pub struct Client {
    pub online: Online,
    pub socket: Option<(u32, usize)>,
    pub session: Option<String>,
    pub now: f64,
    /// Whether the transport can open (false: every connect fails).
    pub reachable: bool,
}

impl Client {
    pub fn new(session: Option<String>) -> Self {
        let mut online = Online::new("wss://test/v1/ws", "incari/26.3", session.as_deref(), 7);
        online.pow_tries_per_tick = 1 << 20;
        Self { online, socket: None, session, now: 0.0, reachable: true }
    }

    /// Does what the service asked and hands it what came back, until both are quiet.
    pub fn settle(&mut self, fake: &mut Fake) -> &mut Self {
        for _ in 0..1000 {
            self.online.tick(self.now, 1_700_000_000_000 + self.now as i64);
            let effects = self.online.take_effects();
            let mut busy = !effects.is_empty();
            for e in effects {
                match e {
                    Effect::Connect { connection, .. } => {
                        if self.reachable {
                            let s = fake.open();
                            self.socket = Some((connection, s));
                            self.online.opened(connection);
                        } else {
                            self.online.closed(connection, "unreachable");
                        }
                    }
                    Effect::Send { connection, text } => {
                        if let Some((c, s)) = self.socket
                            && c == connection
                        {
                            fake.handle(s, &text);
                        }
                    }
                    Effect::Close { connection } => {
                        if let Some((c, s)) = self.socket
                            && c == connection
                        {
                            fake.close(s);
                            self.socket = None;
                        }
                    }
                    Effect::SaveSession(text) => self.session = Some(text),
                    Effect::DeleteSession => self.session = None,
                }
            }
            if let Some((c, s)) = self.socket {
                let inbox = std::mem::take(&mut fake.sockets[s].outbox);
                busy |= !inbox.is_empty();
                for text in inbox {
                    self.online.received(c, &text);
                }
            }
            if !busy {
                break;
            }
        }
        self
    }

    /// The server hung up.
    pub fn drop_socket(&mut self, fake: &mut Fake) {
        if let Some((c, s)) = self.socket.take() {
            fake.close(s);
            self.online.closed(c, "gone");
        }
    }
}
