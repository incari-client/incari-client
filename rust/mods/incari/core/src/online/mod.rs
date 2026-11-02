//! Incari's online service (ADR 0058, 0059, 0062): accounts, friends, messages, blocking and
//! reporting, and which players in the world use Incari. `service::Online` is the protocol as a
//! state machine with no I/O; the game side owns the WebSocket (a worker thread in the browser)
//! and drives it once a frame. Kept here for the window's thread, as the Java client's
//! `IncariOnline.service()`, so every screen reads the same one.
pub mod avatars;
pub mod game;
pub mod rules;
pub mod service;
pub mod sha256;
pub mod social;
pub mod toast;
pub mod world;

#[cfg(test)]
mod fake;
#[cfg(test)]
mod tests;

pub use service::{Answer, Effect, Failure, Online, Outcome, ReportReason, Status, Ticket};
pub use social::{MessagesFrom, Peer};

use std::cell::RefCell;

thread_local! {
    // The window's thread only: in the browser it's the page's, where a lock that waits traps.
    static ONLINE: RefCell<Option<Online>> = const { RefCell::new(None) };
}

/// `hello`'s `client`: `incari/<game version>`, as the Java client sends it.
pub fn build_name(game_version: &str) -> String {
    format!("incari/{game_version}")
}

/// Puts the service in place: the game side does this once, before the first frame.
pub fn install(online: Online) {
    ONLINE.with(|cell| *cell.borrow_mut() = Some(online));
}

/// Runs `f` with the service. Before the game side installed one, a service that's off: with no
/// service the client behaves as if accounts didn't exist.
pub fn with<R>(f: impl FnOnce(&mut Online) -> R) -> R {
    ONLINE.with(|cell| {
        let mut slot = cell.borrow_mut();
        f(slot.get_or_insert_with(|| Online::new("", "incari", None, 1)))
    })
}

/// The title card's account button: who's signed in, kept current (TitleLook.refreshAccount).
pub fn account_button_label() -> String {
    with(|o| match o.account_name().filter(|_| o.signed_in()) {
        Some(name) => format!("Incari: {name}"),
        None => "Incari Account".into(),
    })
}

/// Incari Settings' account tile's line.
pub fn account_tile_text() -> &'static str {
    with(|o| {
        if o.signed_in() {
            "Signed in. Manage your account."
        } else {
            "Sign in to chat, add friends and save cosmetics."
        }
    })
}

/// Incari Settings' "Show others I use Incari" row: its value, and whether it can change (signed
/// in). The tooltip is `SHOW_MY_BADGE_TIP`.
pub fn show_my_badge() -> (bool, bool) {
    with(|o| (o.shows_badge(), o.signed_in()))
}

pub const SHOW_MY_BADGE: &str = "Show others I use Incari";
pub const SHOW_MY_BADGE_TIP: &str = "Shows Incari's badge beside your name to other Incari players. Needs an \
                                     Incari account. Turning it off doesn't hide you from friends or messages.";

/// The count of what's unread and the friend requests waiting: the chat box button's badge.
pub fn unread_count() -> u32 {
    with(|o| o.social().unread() + o.social().pending_requests())
}

/// The update notice reads the service's releases (`crate::release`).
impl crate::release::ReleaseSource for Online {
    fn latest_release(&self) -> Option<String> {
        Online::latest_release(self).map(str::to_owned)
    }
    fn update_needed(&self) -> bool {
        self.status() == Status::UpdateNeeded
    }
    fn release_notes(&self) -> Vec<String> {
        Online::release_notes(self).to_vec()
    }
}

/// The server list's featured servers, replaced by `hello`'s for a run (`crate::featured`).
impl crate::featured::FeaturedSource for Online {
    fn featured_servers(&self) -> Option<Vec<crate::featured::Entry>> {
        Online::featured_servers(self).map(|list| {
            list.iter()
                .map(|f| crate::featured::Entry { name: f.name.clone(), address: f.address.clone() })
                .collect()
        })
    }
}

thread_local! {
    /// Files the screens give the player (the recovery codes), for the game side to download.
    static DOWNLOADS: RefCell<Vec<(String, Vec<u8>)>> = const { RefCell::new(Vec::new()) };
}

/// Gives the player `data` as a file named `name`: the game side downloads it (`take_downloads`).
pub fn download(name: &str, data: Vec<u8>) {
    DOWNLOADS.with(|d| d.borrow_mut().push((name.to_owned(), data)));
}

/// The files asked for since the last call, for the game side.
pub fn take_downloads() -> Vec<(String, Vec<u8>)> {
    DOWNLOADS.with(|d| std::mem::take(&mut *d.borrow_mut()))
}

thread_local! {
    /// The player's time zone's offset from UTC, in ms (the game side sets it from the browser's).
    static UTC_OFFSET: std::cell::Cell<i64> = const { std::cell::Cell::new(0) };
}

/// The player's time zone, ms ahead of UTC (`-new Date().getTimezoneOffset() * 60000`).
pub fn set_utc_offset(ms: i64) {
    UTC_OFFSET.with(|c| c.set(ms));
}

pub fn utc_offset() -> i64 {
    UTC_OFFSET.with(|c| c.get())
}
