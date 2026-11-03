//! Toasts (ChatToast.java): a message or friend request that arrived while the chat menu was
//! closed, on a glass card in the top right corner for five seconds. One card a sender and kind:
//! more from them while it's up update it with a count instead of queueing a card each.
use super::avatars;
use super::social::Notice;
use crate::client::Client;
use crate::icons::Icon;
use crate::paint::{Measure, Painter};
use std::cell::{Cell, RefCell};

const SHOWN_MILLIS: f64 = 5000.0;
/// The game's toasts' size.
pub const WIDTH: f32 = 160.0;
pub const HEIGHT: f32 = 32.0;
/// How long a card takes to slide in or out, in Normal.
const SLIDE_MILLIS: f64 = 250.0;
/// At most this many cards at once; the rest wait.
const MAX_SHOWN: usize = 5;

struct Card {
    notice: Notice,
    token: String,
    /// How many notices the card stands for.
    count: u32,
    /// When its five seconds last started; None while it waits for room.
    from: Option<f64>,
}

thread_local! {
    static CARDS: RefCell<Vec<Card>> = const { RefCell::new(Vec::new()) };
    /// When the chat menu last drew: while it shows, notices are taken unseen.
    static MENU_AT: Cell<f64> = const { Cell::new(f64::NEG_INFINITY) };
}

/// The chat menu is showing (it calls this as it draws).
pub fn menu_shown(now: f64) {
    MENU_AT.with(|m| m.set(now));
}

/// What tells one sender's card from another's.
fn token(notice: &Notice) -> String {
    format!(
        "{}{}",
        if notice.request {
            "request:"
        } else {
            "message:"
        },
        notice.who
    )
}

/// Takes what arrived and draws the cards over everything (`Client::frame`'s list is left as it
/// was replayed: this clears it and draws the toasts into it). The game side calls it once a frame,
/// after the client's frame, and replays `client.list` again.
pub fn frame(client: &mut Client, measure: &dyn Measure, size: [f32; 2], scale: f32, now: f64) {
    let notices = super::with(|o| o.take_notices());
    let menu = MENU_AT.with(|m| now - m.get() < 250.0);
    if !menu {
        // The chat menu isn't showing (closed, or a screen over it): its conversation's messages
        // count as unread again, as the Java menu's `removed` lets go of it.
        super::with(|o| {
            if o.viewing().is_some() {
                o.set_viewing(None);
            }
        });
    }
    client.list.clear();
    CARDS.with(|cards| {
        let mut cards = cards.borrow_mut();
        if !menu {
            for notice in notices {
                let token = token(&notice);
                match cards.iter_mut().find(|c| c.token == token) {
                    Some(c) => {
                        c.notice = notice;
                        c.count += 1;
                        if c.from.is_some() {
                            c.from = Some(now);
                        }
                    }
                    None => cards.push(Card {
                        notice,
                        token,
                        count: 1,
                        from: None,
                    }),
                }
            }
        }
        cards.retain(|c| c.from.is_none_or(|f| now - f < SHOWN_MILLIS + SLIDE_MILLIS));
        if cards.is_empty() {
            return;
        }
        let mut p = Painter {
            list: &mut client.list,
            canvas: &mut client.canvas,
            assets: &client.assets,
            motion: &mut client.motion,
            measure,
            theme: client.theme,
            scale,
            fade: 1.0,
            origin: [0.0, 0.0],
            zoom: 1.0,
        };
        let slide = p.theme.normal();
        for (i, card) in cards.iter_mut().take(MAX_SHOWN).enumerate() {
            let from = *card.from.get_or_insert(now);
            let age = now - from;
            let t = if !slide {
                1.0
            } else if age < SLIDE_MILLIS {
                age / SLIDE_MILLIS
            } else if age > SHOWN_MILLIS {
                1.0 - (age - SHOWN_MILLIS) / SLIDE_MILLIS
            } else {
                1.0
            } as f32;
            let x = size[0] - WIDTH * t.clamp(0.0, 1.0);
            draw(&mut p, x, i as f32 * HEIGHT, &card.notice, card.count);
        }
    });
}

/// One card at (`x`, `y`).
pub fn draw(p: &mut Painter, x: f32, y: f32, notice: &Notice, count: u32) {
    let th = p.theme;
    p.round_box(
        x,
        y,
        WIDTH,
        HEIGHT,
        th.panel_radius,
        th.floating,
        th.glass_edge,
        0.0,
        0.0,
        0,
    );
    avatars::draw(p, x + 8.0, y + 8.0, 16.0, &notice.who);
    let icon = if notice.request {
        Icon::PersonAdd
    } else {
        Icon::Chat
    };
    p.icon(icon, x + WIDTH - 16.0, y + 7.0, 9.0, th.accent);
    let suffix = if count > 1 {
        format!(" ({count})")
    } else {
        String::new()
    };
    let title_width = WIDTH - 52.0 - p.width(&suffix);
    let title = format!("{}{suffix}", p.fit(&notice.title, title_width));
    p.text(&title, x + 31.0, y + 7.0, th.text, false);
    let text = p.fit(&notice.text, WIDTH - 38.0).into_owned();
    p.text(&text, x + 31.0, y + 18.0, th.muted, false);
}
