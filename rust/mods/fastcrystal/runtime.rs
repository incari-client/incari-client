// Incari's Fast Crystal for the Rust build (ADR 0068, 0069, 0073), as features/fastcrystal does
// it: a crystal the player hits, and a charged anchor they set off outside the Nether, gone on
// this client at once instead of a ping later. Prediction only: Horizon takes the crystal off
// (`native_take_crystal`) or shows the anchor as air under the use's sequence
// (`native_predict_air`); nothing more is sent. This keeps the crystals waiting for the server,
// and when each is due to come back. No game types: rust/lib.rs tests it.

use std::cell::RefCell;

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "fastcrystal";

/// The least and most a crystal waits for the server before coming back, in milliseconds.
const MIN_WAIT_MS: i64 = 600;
const MAX_WAIT_MS: i64 = 3000;
const TICK_MS: i64 = 50;

/// Ticks a crystal waits for the server: twice the ping and 0.3 s, from 0.6 s to 3 s, as Java.
pub fn wait_ticks(ping_ms: Option<i32>) -> u32 {
    let ms = (2 * i64::from(ping_ms.unwrap_or(0).max(0)) + 300).clamp(MIN_WAIT_MS, MAX_WAIT_MS);
    // Whole ticks, rounded up: never back sooner than the wait.
    ((ms + TICK_MS - 1) / TICK_MS) as u32
}

/// A crystal taken off this client: its entity id, and the ticks left before it's due back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Waiting {
    id: u32,
    ticks: u32,
}

thread_local! {
    /// Rarely more than a few: one per crystal hit in the last few seconds.
    static WAITING: RefCell<Vec<Waiting>> = const { RefCell::new(Vec::new()) };
}

/// The crystal `id` was taken off this client; it waits `wait_ticks(ping_ms)` for the server.
pub fn taken(id: u32, ping_ms: Option<i32>) {
    let ticks = wait_ticks(ping_ms);
    WAITING.with(|w| {
        let mut w = w.borrow_mut();
        w.retain(|c| c.id != id);
        w.push(Waiting { id, ticks });
    });
}

/// One client tick: `back` gets each crystal whose wait ran out, to put back if the server kept
/// it (Horizon forgets the ones the server removed). Hot path: nothing allocated.
pub fn tick(mut back: impl FnMut(u32)) {
    WAITING.with(|w| {
        w.borrow_mut().retain_mut(|c| {
            if c.ticks == 0 {
                back(c.id);
                false
            } else {
                c.ticks -= 1;
                true
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain() {
        while WAITING.with(|w| !w.borrow().is_empty()) {
            tick(|_| {});
        }
    }

    #[test]
    fn the_wait_is_twice_the_ping_and_some_within_bounds() {
        assert_eq!(wait_ticks(None), 12); // 0.6 s at least
        assert_eq!(wait_ticks(Some(100)), 12); // 500 ms: still the least
        assert_eq!(wait_ticks(Some(200)), 14); // 700 ms
        assert_eq!(wait_ticks(Some(201)), 15); // 702 ms rounds up
        assert_eq!(wait_ticks(Some(5000)), 60); // 3 s at most
        assert_eq!(wait_ticks(Some(-7)), 12);
    }

    #[test]
    fn a_crystal_comes_due_once_after_its_wait() {
        drain();
        taken(42, None);
        let mut due = Vec::new();
        for _ in 0..12 {
            tick(|id| due.push(id));
        }
        assert!(due.is_empty());
        tick(|id| due.push(id));
        assert_eq!(due, [42]);
        tick(|id| due.push(id));
        assert_eq!(due, [42]);
    }

    #[test]
    fn hitting_the_same_crystal_again_restarts_its_wait() {
        drain();
        taken(7, None);
        for _ in 0..10 {
            tick(|_| {});
        }
        taken(7, None);
        let mut due = 0;
        for _ in 0..12 {
            tick(|_| due += 1);
        }
        assert_eq!(due, 0);
        tick(|_| due += 1);
        assert_eq!(due, 1);
    }
}
