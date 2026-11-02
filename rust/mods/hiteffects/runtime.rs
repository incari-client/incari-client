// Incari's Hit & Kill Effects for the Rust build (ADR 0068, 0069), as features/hiteffects and the
// Java client's ModuleEffects do it: particles on whatever you hit, and a bigger burst with the
// effect's sound when it dies within 3 seconds of your last hit. Only you see them. Its settings
// are the module's in `incari_core::modules` (the effect keys of `modules::EFFECTS`). No game
// types: rust/lib.rs tests it.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "hiteffects";

/// A death counts as your kill this long after your hit on it (the Java client's 60 ticks).
pub const KILL_WINDOW: f32 = 3.0;
/// The kill sound's volume and pitch.
pub const VOLUME: f32 = 0.6;
pub const PITCH: f32 = 1.0;
/// A mob's size when Horizon doesn't list it: a player's.
pub const SIZE: [f64; 2] = [0.6, 1.8];

/// An effect key's particle, how fast its particles fly apart, and its sound.
pub fn effect(key: &str) -> (&'static str, f64, &'static str) {
    match key {
        "crits" => ("minecraft:crit", 0.3, "minecraft:entity.player.attack.crit"),
        "sparks" => (
            "minecraft:electric_spark",
            0.3,
            "minecraft:block.amethyst_block.chime",
        ),
        "hearts" => (
            "minecraft:heart",
            0.05,
            "minecraft:entity.experience_orb.pickup",
        ),
        "flames" => ("minecraft:flame", 0.3, "minecraft:item.firecharge.use"),
        "snow" => (
            "minecraft:snowflake",
            0.3,
            "minecraft:block.powder_snow.break",
        ),
        "firework" => (
            "minecraft:firework",
            0.5,
            "minecraft:entity.firework_rocket.blast",
        ),
        "souls" => ("minecraft:soul", 0.05, "minecraft:particle.soul_escape"),
        "smoke" => ("minecraft:poof", 0.1, "minecraft:block.fire.extinguish"),
        "totem" => (
            "minecraft:totem_of_undying",
            0.5,
            "minecraft:item.totem.use",
        ),
        _ => (
            "minecraft:enchanted_hit",
            0.3,
            "minecraft:block.enchantment_table.use",
        ),
    }
}

/// What the player hit last, how long ago, its size, and a little random state.
#[derive(Clone, Copy, Debug)]
pub struct State {
    target: Option<u32>,
    since: f32,
    size: [f64; 2],
    seed: u64,
}

impl State {
    pub const fn new() -> Self {
        Self {
            target: None,
            since: 0.0,
            size: SIZE,
            seed: 0x9E37_79B9_7F4A_7C15,
        }
    }

    /// The player hit `entity`, `size` (width, height) big.
    pub fn hit(&mut self, entity: u32, size: [f64; 2]) {
        self.target = Some(entity);
        self.since = 0.0;
        self.size = size;
    }

    pub fn advance(&mut self, seconds: f32) {
        self.since += seconds;
        if self.since > KILL_WINDOW {
            self.target = None;
        }
    }

    /// Whether `entity` dying is the player's kill, forgetting it.
    pub fn killed(&mut self, entity: u32) -> bool {
        if self.target != Some(entity) {
            return false;
        }
        self.target = None;
        true
    }

    /// The last hit's size.
    pub fn size(&self) -> [f64; 2] {
        self.size
    }

    /// A number from 0 to 1 (xorshift).
    pub fn random(&mut self) -> f64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 11) as f64 / (1u64 << 53) as f64
    }

    /// A particle's start in the box of a mob `size` big standing at `at`, and its velocity, as
    /// the Java client sprays them.
    pub fn spray(&mut self, at: [f64; 3], size: [f64; 2], speed: f64) -> ([f64; 3], [f64; 3]) {
        let [w, h] = size;
        let start = [
            at[0] - w / 2.0 + self.random() * w,
            at[1] + h * (0.2 + self.random() * 0.7),
            at[2] - w / 2.0 + self.random() * w,
        ];
        let velocity = [
            (self.random() - 0.5) * speed,
            self.random() * speed * 0.6,
            (self.random() - 0.5) * speed,
        ];
        (start, velocity)
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

// The window's thread only: attacks, deaths and frames all run there.
thread_local! {
    static STATE: std::cell::RefCell<State> = const { std::cell::RefCell::new(State::new()) };
}

pub fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_a_death_as_a_kill_only_soon_after_a_hit() {
        let mut s = State::new();
        s.hit(7, [1.0, 2.0]);
        s.advance(1.0);
        assert!(s.killed(7));
        assert!(!s.killed(7));
        s.hit(8, SIZE);
        s.advance(KILL_WINDOW + 0.1);
        assert!(!s.killed(8));
        s.hit(8, SIZE);
        s.hit(9, SIZE);
        assert!(!s.killed(8) && s.killed(9));
    }

    #[test]
    fn sprays_in_the_body() {
        let mut s = State::new();
        for _ in 0..100 {
            let r = s.random();
            assert!((0.0..1.0).contains(&r));
            let (start, v) = s.spray([0.0; 3], [0.6, 1.8], 0.3);
            assert!(start[1] >= 0.36 && start[1] <= 1.62 && start[0].abs() <= 0.3);
            assert!(v[1] >= 0.0 && v[0].abs() <= 0.15);
        }
    }

    #[test]
    fn knows_every_effect() {
        assert_eq!(effect("magic").0, "minecraft:enchanted_hit");
        assert_eq!(
            effect("firework"),
            (
                "minecraft:firework",
                0.5,
                "minecraft:entity.firework_rocket.blast"
            )
        );
        for key in [
            "crits", "sparks", "hearts", "flames", "snow", "souls", "smoke", "totem",
        ] {
            assert_ne!(effect(key).0, "minecraft:enchanted_hit", "{key}");
        }
    }
}
