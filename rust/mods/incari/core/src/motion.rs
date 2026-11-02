//! Eased values for the look's animations in Normal (Motion.java): how far a control has lit up
//! toward its hover state, and when the hover began. Kept by the control's rectangle, as widgets
//! stay put between frames. In Potato every value is its target at once.

/// How many controls are animated at once; the least recently drawn gives up its slot.
const SLOTS: usize = 64;
/// How long a hover takes to ease in or out, roughly: the time constant, in milliseconds.
const EASE_MILLIS: f64 = 70.0;

#[derive(Clone, Copy, Default)]
struct Slot {
    key: u64,
    value: f32,
    updated: f64,
    since: f64,
    used: bool,
}

/// The animated controls. Hot path: a fixed array, nothing allocates.
pub struct Motion {
    slots: [Slot; SLOTS],
    last: usize,
    /// Milliseconds now, set once a frame.
    pub now: f64,
    /// Whether controls ease (Normal); off, every value is its target.
    pub on: bool,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            slots: [Slot::default(); SLOTS],
            last: 0,
            now: 0.0,
            on: false,
        }
    }
}

/// A key for the control in this rectangle (GUI units).
pub fn key(x: f32, y: f32, w: f32, h: f32) -> u64 {
    let k = |v: f32| (v.round() as i32 as u64) & 0xFFFF;
    k(x) << 48 | k(y) << 32 | k(w) << 16 | k(h)
}

impl Motion {
    /// Moves the control's value toward `target` (0 or 1) by the time since it was last drawn.
    pub fn ease(&mut self, key: u64, target: f32) -> f32 {
        if !self.on {
            return target;
        }
        let now = self.now;
        let i = self.find(key);
        let slot = &mut self.slots[i];
        if !slot.used || slot.key != key {
            *slot = Slot {
                key,
                value: target,
                updated: now,
                since: if target > 0.5 { now - 10_000.0 } else { now },
                used: true,
            };
        } else {
            if target > 0.5 && slot.value <= 0.001 {
                slot.since = now;
            }
            let dt = (now - slot.updated).clamp(0.0, 200.0);
            let value = slot.value + (target - slot.value) * (1.0 - (-dt / EASE_MILLIS).exp()) as f32;
            slot.value = if (target - value).abs() < 0.002 { target } else { value };
            slot.updated = now;
        }
        self.last = i;
        self.slots[i].value
    }

    /// Milliseconds since the control last eased turned on, after `ease`.
    pub fn since(&self) -> f64 {
        self.now - self.slots[self.last].since
    }

    /// Forgets every control (a preset switch).
    pub fn reset(&mut self) {
        for slot in &mut self.slots {
            slot.used = false;
        }
    }

    fn find(&self, key: u64) -> usize {
        let mut oldest = 0;
        let mut oldest_time = f64::MAX;
        for (i, slot) in self.slots.iter().enumerate() {
            if !slot.used {
                if oldest_time != f64::MIN {
                    oldest = i;
                    oldest_time = f64::MIN;
                }
                continue;
            }
            if slot.key == key {
                return i;
            }
            if slot.updated < oldest_time {
                oldest = i;
                oldest_time = slot.updated;
            }
        }
        oldest
    }
}

/// `1 - (1 - t)^3`: fast, then settling.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn potato_jumps_and_normal_eases() {
        let mut m = Motion::default();
        assert_eq!(m.ease(key(1.0, 2.0, 3.0, 4.0), 1.0), 1.0);
        m.on = true;
        let k = key(10.0, 10.0, 50.0, 20.0);
        assert_eq!(m.ease(k, 0.0), 0.0);
        m.now += 35.0;
        let v = m.ease(k, 1.0);
        assert!(v > 0.3 && v < 0.5, "{v}");
        for _ in 0..5 {
            m.now += 200.0;
            m.ease(k, 1.0);
        }
        assert_eq!(m.ease(k, 1.0), 1.0);
    }
}
