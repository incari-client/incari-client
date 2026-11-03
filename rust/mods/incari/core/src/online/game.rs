//! What the game side does for the service each tick (IncariOnline.tick and ChatMenu.tick in
//! Java), with no game types: when to connect, joins and leaves, the tab list, and the button under
//! the chat box.
use super::world::Uuid;
use super::{Online, Status};
use crate::icons::Icon;
use crate::paint::Painter;
use std::collections::HashMap;

/// Ticks between tab-list reads: a second. Names are looked up as they appear, not faster.
pub const TAB_LIST_TICKS: u32 = 20;

/// Which world the player is in, as of the last tick.
#[derive(Default)]
pub struct WorldTracker {
    world: Option<u64>,
    countdown: u32,
}

impl WorldTracker {
    /// Once a client tick. `title`: the title screen shows (the service connects once it has, never
    /// while the game loads). `world`: the current world's connection, by any number that changes
    /// when it does (a dimension change keeps it, a transfer replaces it); None outside a world.
    /// `server`: the address joined and the player's name there, only on another's server (never
    /// singleplayer or the player's own LAN world). `tab`: the tab list, read once a second.
    pub fn tick(
        &mut self,
        online: &mut Online,
        title: bool,
        world: Option<u64>,
        server: Option<(&str, &str)>,
        tab: impl FnOnce() -> HashMap<Uuid, String>,
    ) {
        if !online.started() && (title || world.is_some()) {
            online.start();
        }
        if world != self.world {
            if self.world.is_some() {
                online.left();
            }
            self.world = world;
            if world.is_some()
                && let Some((address, name)) = server
            {
                online.joined(address, name);
                self.countdown = 0;
            }
        }
        if world.is_some() && online.in_world() {
            self.countdown = self.countdown.saturating_sub(1);
            if self.countdown == 0 {
                self.countdown = TAB_LIST_TICKS;
                online.tab_list(&tab());
            }
        }
    }
}

/// The button under the chat box: a chat icon, "Incari Chat", and a red count of what's unread.
pub const CHAT_BUTTON_LABEL: &str = "Incari Chat";
pub const CHAT_BUTTON_HEIGHT: f32 = 14.0;
pub const CHAT_BUTTON_TOOLTIP: &str = "Friends and messages (O)";

/// Where it goes: under the chat box's last line (40 above the bottom), over the input field.
pub fn chat_button_rect(p: &Painter, screen: [f32; 2]) -> [f32; 4] {
    [
        2.0,
        screen[1] - 33.0,
        18.0 + p.width(CHAT_BUTTON_LABEL) + 8.0,
        CHAT_BUTTON_HEIGHT,
    ]
}

/// Draws it, hot under the pointer at `mouse`, with `count` unread (`super::unread_count`).
pub fn draw_chat_button(p: &mut Painter, screen: [f32; 2], mouse: [f32; 2], count: u32) {
    let th = p.theme;
    let [x, y, w, h] = chat_button_rect(p, screen);
    let hot = crate::draw::contains([x, y, w, h], mouse);
    p.round(
        x,
        y,
        w,
        h,
        h / 2.0,
        if hot { 0xCC2A_3542 } else { 0x9916_1C24 },
    );
    p.icon(
        Icon::Chat,
        x + 5.0,
        y + 3.0,
        8.0,
        if hot { th.accent } else { th.text },
    );
    p.text(CHAT_BUTTON_LABEL, x + 17.0, y + 3.0, th.text, false);
    if count > 0 {
        let label = if count > 9 {
            "9+".to_owned()
        } else {
            count.to_string()
        };
        let lw = p.width(&label);
        let badge = (lw + 5.0).max(10.0);
        let bx = (x + w - badge / 2.0 - 1.0).floor();
        p.round(bx, y - 3.0, badge, 10.0, 5.0, 0xFFE0_565B);
        p.text(
            &label,
            bx + ((badge - lw) / 2.0).floor() + 1.0,
            y - 2.0,
            th.text,
            false,
        );
    }
}

/// The game's chat screen shows: draws the button into `client.list` (cleared first), for the
/// game side to replay over the chat screen.
pub fn chat_button_frame(
    client: &mut crate::client::Client,
    measure: &dyn crate::paint::Measure,
    size: [f32; 2],
    scale: f32,
) {
    client.list.clear();
    let mouse = client.ui.mouse;
    let count = super::unread_count();
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
    draw_chat_button(&mut p, size, mouse, count);
}

/// Whether a press at `at` lands on the chat box's button.
pub fn chat_button_hit(measure: &dyn crate::paint::Measure, size: [f32; 2], at: [f32; 2]) -> bool {
    let w = 18.0 + measure.width(CHAT_BUTTON_LABEL) + 8.0;
    crate::draw::contains([2.0, size[1] - 33.0, w, CHAT_BUTTON_HEIGHT], at)
}

/// Whether the account screens can do anything: the service is on in this build.
pub fn available(online: &Online) -> bool {
    online.status() != Status::Off
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::online::fake::{Client, Fake};

    #[test]
    fn joins_and_leaves_follow_the_connection() {
        let mut fake = Fake::new();
        let mut c = Client::new(None);
        let mut tracker = WorldTracker::default();
        tracker.tick(&mut c.online, false, None, None, HashMap::new);
        assert!(!c.online.started(), "never while the game loads");
        tracker.tick(&mut c.online, true, None, None, HashMap::new);
        assert!(c.online.started());
        c.settle(&mut fake);
        let mut reads = 0;
        tracker.tick(
            &mut c.online,
            false,
            Some(1),
            Some(("a.example", "Me")),
            || {
                reads += 1;
                HashMap::new()
            },
        );
        assert!(c.online.in_world());
        assert_eq!(reads, 1);
        for _ in 0..TAB_LIST_TICKS - 1 {
            tracker.tick(
                &mut c.online,
                false,
                Some(1),
                Some(("a.example", "Me")),
                || {
                    reads += 1;
                    HashMap::new()
                },
            );
        }
        assert_eq!(reads, 1, "once a second");
        tracker.tick(&mut c.online, false, Some(2), None, HashMap::new);
        assert!(!c.online.in_world(), "singleplayer is never reported");
    }
}
