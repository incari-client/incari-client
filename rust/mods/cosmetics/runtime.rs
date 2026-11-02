// Incari's cosmetics in the world for the Rust build (ADR 0061, 0069): worn capes, hats, wings and
// back items, name tag styles, Incari's badge, trails and emotes on the players Horizon draws. What
// they are and how they look is `incari_core::cosmetics` (the client's, no game types); this is
// the game side's own bookkeeping between its hooks: where each picture is on the fork's entity
// canvas, what each player drawn this frame wears, and the frame's timing. No game types either:
// rust/lib.rs tests it.

/// The entity canvas's size (the fork's `MOD_ENTITY_CANVAS_SIZE`).
pub const CANVAS: [u32; 2] = [512, 256];
/// A cape's picture: the game's 64x32 cape layout.
pub const CAPE: [u32; 2] = [64, 32];
/// A model's texture (`incari_core::cosmetics::models::TEXTURE`).
pub const MODEL: u32 = 64;
/// How many capes and models the canvas holds.
pub const CAPES: usize = 8;
pub const MODELS: usize = 12;
/// A moving cape's frames a second, and how long it goes on moving after it was last drawn.
pub const CAPE_FPS: f64 = 10.0;
pub const CAPE_IDLE_MS: f64 = 2_000.0;
/// How often the client's `show-badges` setting is read again, in ticks.
pub const SETTINGS_TICKS: u64 = 100;

/// Where cape `n` (its place among the capes) is on the canvas: the top row.
pub fn cape_slot(n: usize) -> [u32; 2] {
    [n as u32 * CAPE[0], 0]
}

/// Where model `n` (its place among the hats and back cosmetics) is: rows of eight under the capes.
pub fn model_slot(n: usize) -> [u32; 2] {
    [(n % 8) as u32 * MODEL, CAPE[1] + (n / 8) as u32 * MODEL]
}

/// A texture rectangle `[u0, v0, u1, v1]` in pixels of a picture at `slot`, as `native_draw_quad`
/// takes it: texels of 16 per side of the whole canvas.
pub fn uv16(slot: [u32; 2], uv: [f32; 4]) -> [f32; 4] {
    let (x, y) = (slot[0] as f32, slot[1] as f32);
    let (w, h) = (CANVAS[0] as f32, CANVAS[1] as f32);
    [(x + uv[0]) * 16.0 / w, (y + uv[1]) * 16.0 / h, (x + uv[2]) * 16.0 / w, (y + uv[3]) * 16.0 / h]
}

/// What `look` found a player drawn this frame wears on their model, for `layer` right after.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Job {
    pub mob: u32,
    /// The hat's and back cosmetic's places in the catalog, when they're drawn.
    pub hat: Option<usize>,
    pub back: Option<usize>,
    pub age: f32,
    pub crouching: bool,
    pub fall_flying: bool,
    pub chest_armor: bool,
    /// The cape's turn back (radians), when one hangs.
    pub cape_swing: Option<f32>,
}

/// A player seen this frame, for the trails left once a tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Seen {
    pub mob: u32,
    /// The trail's place in the catalog.
    pub trail: Option<usize>,
    pub position: [f64; 3],
    /// Where they were at the last tick's trail.
    pub ticked: Option<[f64; 3]>,
    /// The tick they were last seen.
    pub tick: u64,
}

#[derive(Default)]
pub struct State {
    pub jobs: Vec<Job>,
    pub seen: Vec<Seen>,
    /// The canvas holds the still pictures (a new atlas starts blank).
    pub painted: bool,
    /// When each cape was last drawn, and when moving ones were last painted (ms).
    pub cape_drawn: [f64; CAPES],
    pub capes_painted: f64,
    /// The player's own feet at the last tick.
    pub own: Option<[f64; 3]>,
    pub ticks: u64,
    /// The client's "Show Incari badges" (`settings.txt`'s `show-badges`).
    pub show_badges: bool,
    /// Milliseconds since the game started, as the last frame said.
    pub now_ms: f64,
}

impl State {
    /// `look`'s job for `job.mob`, in place of last frame's.
    pub fn set_job(&mut self, job: Job) {
        match self.jobs.iter_mut().find(|j| j.mob == job.mob) {
            Some(j) => *j = job,
            None => {
                if self.jobs.len() >= 256 {
                    self.jobs.clear();
                }
                self.jobs.push(job);
            }
        }
    }

    pub fn job(&self, mob: u32) -> Option<Job> {
        self.jobs.iter().find(|j| j.mob == mob).copied()
    }

    /// Player `mob` was drawn at `position`, wearing `trail`.
    pub fn saw(&mut self, mob: u32, trail: Option<usize>, position: [f64; 3]) {
        let tick = self.ticks;
        match self.seen.iter_mut().find(|s| s.mob == mob) {
            Some(s) => {
                s.trail = trail;
                s.position = position;
                s.tick = tick;
            }
            None => self.seen.push(Seen { mob, trail, position, ticked: None, tick }),
        }
    }

    /// Once a tick: players not drawn for two seconds forgotten.
    pub fn tick(&mut self) {
        self.ticks += 1;
        let ticks = self.ticks;
        self.seen.retain(|s| ticks - s.tick < 40);
    }

    /// Cape `n` was drawn now.
    pub fn cape_drawn(&mut self, n: usize) {
        if n < CAPES {
            self.cape_drawn[n] = self.now_ms;
        }
    }

    /// Whether moving capes are due another frame; marks them painted.
    pub fn capes_due(&mut self) -> bool {
        if self.now_ms - self.capes_painted < 1000.0 / CAPE_FPS {
            return false;
        }
        self.capes_painted = self.now_ms;
        true
    }

    /// Whether cape `n` was drawn lately enough to move.
    pub fn cape_moving(&self, n: usize) -> bool {
        n < CAPES && self.cape_drawn[n] > 0.0 && self.now_ms - self.cape_drawn[n] < CAPE_IDLE_MS
    }
}

thread_local! {
    // The window's thread only.
    static STATE: std::cell::RefCell<State> = std::cell::RefCell::new(State { show_badges: true, ..State::default() });
}

pub fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// `settings.txt`'s `show-badges` (on unless it says false).
pub fn show_badges(settings: &str) -> bool {
    !settings
        .lines()
        .filter_map(|l| l.split_once('='))
        .any(|(k, v)| k.trim() == "show-badges" && v.trim() == "false")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pictures_fit_the_canvas_apart() {
        let mut rects = Vec::new();
        for n in 0..CAPES {
            let [x, y] = cape_slot(n);
            rects.push([x, y, CAPE[0], CAPE[1]]);
        }
        for n in 0..MODELS {
            let [x, y] = model_slot(n);
            rects.push([x, y, MODEL, MODEL]);
        }
        for (i, a) in rects.iter().enumerate() {
            assert!(a[0] + a[2] <= CANVAS[0] && a[1] + a[3] <= CANVAS[1], "{a:?}");
            for b in &rects[..i] {
                let apart = a[0] + a[2] <= b[0] || b[0] + b[2] <= a[0] || a[1] + a[3] <= b[1] || b[1] + b[3] <= a[1];
                assert!(apart, "{a:?} {b:?}");
            }
        }
        assert_eq!(uv16([0, 0], [0.0, 0.0, 512.0, 256.0]), [0.0, 0.0, 16.0, 16.0]);
    }

    #[test]
    fn jobs_and_sightings_are_kept_per_player() {
        let mut s = State::default();
        s.set_job(Job { mob: 3, hat: Some(8), ..Job::default() });
        s.set_job(Job { mob: 3, hat: None, ..Job::default() });
        assert_eq!(s.jobs.len(), 1);
        assert_eq!(s.job(3).unwrap().hat, None);
        s.saw(5, Some(26), [1.0, 2.0, 3.0]);
        for _ in 0..40 {
            s.tick();
        }
        assert!(s.seen.is_empty(), "not drawn for two seconds: forgotten");
        assert!(show_badges("copy-screenshots=true\n") && !show_badges("show-badges=false\n"));
    }

    #[test]
    fn moving_capes_paint_ten_times_a_second_while_seen() {
        let mut s = State { now_ms: 1_000.0, ..State::default() };
        s.cape_drawn(1);
        assert!(s.capes_due() && !s.capes_due());
        assert!(s.cape_moving(1) && !s.cape_moving(2));
        s.now_ms = 3_500.0;
        assert!(!s.cape_moving(1));
    }
}
