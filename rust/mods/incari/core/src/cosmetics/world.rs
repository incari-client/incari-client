//! Cosmetics in the world (ADR 0061; `Cosmetics.java`), with no game types: what each player
//! shows (their `Worn`), the emotes playing and when they end, the camera turned for the player's
//! own emote, and the trails' particles. The game side asks once a frame per player drawn
//! (`worn_by`), once a tick (`tick`), and plays the wheel's emote (`play_own`).
//!
//! Others' cosmetics show only for Incari players the service named, and only while the Wardrobe's
//! "Show others' cosmetics" is on. Window's thread; per player per frame nothing is made unless
//! what someone wears changed.
use super::catalog::{self, Cosmetic};
use super::emotes;
use super::loadout::Loadout;
use std::cell::RefCell;
use std::collections::HashMap;

/// Trails are left only by players this near (blocks).
pub const TRAIL_DISTANCE: f64 = 32.0;
/// At most this many trail particles a tick, however many players are about.
pub const TRAIL_BUDGET: u32 = 12;
/// Past this, worn models aren't drawn (blocks): a hat is a pixel or two.
pub const MAX_DISTANCE: f64 = 48.0;
/// Another player walking faster than this (`walkAnimation.speed`) cuts their emote short.
pub const WALK_ENDS_EMOTE: f32 = 0.35;

/// The camera views (`native_camera_type`): first person, behind, in front.
pub const FIRST_PERSON: u8 = 0;
pub const THIRD_PERSON_FRONT: u8 = 2;

/// What one player shows: what they wear, and the emote they're playing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Worn {
    pub loadout: Loadout,
    pub emote: Option<&'static Cosmetic>,
    /// When the emote started, in client ticks (`World::now`'s clock).
    pub emote_start: f32,
}

impl Worn {
    pub const NONE: Worn = Worn {
        loadout: Loadout::EMPTY,
        emote: None,
        emote_start: 0.0,
    };

    pub fn is_empty(&self) -> bool {
        self.loadout.is_empty() && self.emote.is_none()
    }

    /// The emote's pose at client time `now` (ticks, the partial tick added).
    pub fn pose(&self, now: f32) -> Option<emotes::Pose> {
        emotes::pose(self.emote?.id, now - self.emote_start)
    }
}

/// An emote playing: which, and the client ticks it started and ends on.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Play {
    emote: &'static Cosmetic,
    start: u64,
    ends_at: u64,
}

/// A trail particle: its type, where from the player's feet, and its speed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    pub kind: &'static str,
    pub offset: [f64; 3],
    pub velocity: [f64; 3],
}

#[derive(Default)]
pub struct World {
    ticks: u64,
    plays: HashMap<u128, Play>,
    /// Others' cosmetic ids as last seen, what they came to, and the tick they were last drawn.
    others: HashMap<u128, (Vec<String>, Loadout, u64)>,
    /// The camera to go back to once the player's own emote ends.
    camera_before: Option<u8>,
    own: Option<u128>,
    trails_left: u32,
    random: u64,
    /// The emote the wheel chose, for the game side to play on its next tick.
    pending_own: Option<&'static Cosmetic>,
}

impl World {
    /// Client ticks since the game started.
    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// The clock emotes are timed by: client ticks, the frame's partial tick added.
    pub fn now(&self, partial: f32) -> f32 {
        self.ticks as f32 + partial
    }

    /// What player `id` shows. `local`: the window's own player, who wears `own`. `incari`: the ids
    /// the service says they wear, when they're an Incari player.
    pub fn worn_by(
        &mut self,
        id: u128,
        local: bool,
        own: Loadout,
        incari: Option<&[String]>,
        show_others: bool,
    ) -> Worn {
        let play = self.plays.get(&id).copied();
        let worn = |loadout: Loadout| Worn {
            loadout,
            emote: play.map(|p| p.emote),
            emote_start: play.map_or(0.0, |p| p.start as f32),
        };
        if local {
            self.own = Some(id);
            return worn(own);
        }
        if !show_others || (incari.is_none() && play.is_none()) {
            return Worn::NONE;
        }
        let source = incari.unwrap_or(&[]);
        let ticks = self.ticks;
        let loadout = match self.others.get_mut(&id) {
            Some((seen, loadout, drawn)) if seen.as_slice() == source => {
                *drawn = ticks;
                *loadout
            }
            _ => {
                if self.others.len() > 512 {
                    self.others.clear();
                }
                let loadout = Loadout::of(source);
                self.others.insert(id, (source.to_vec(), loadout, ticks));
                loadout
            }
        };
        worn(loadout)
    }

    /// Another player `id` was seen walking at `speed` (`walkAnimation.speed`): moving cuts their
    /// emote short.
    pub fn walking(&mut self, id: u128, speed: f32) {
        if speed > WALK_ENDS_EMOTE && Some(id) != self.own {
            self.plays.remove(&id);
        }
    }

    /// The player `own` plays `emote` (the emote wheel, after the service took it): the camera
    /// turns to face them while it plays. Returns the view to set, if it changes (`camera` is the
    /// view now).
    pub fn play_own(&mut self, own: u128, emote: &'static Cosmetic, camera: u8) -> Option<u8> {
        self.own = Some(own);
        self.start(own, emote);
        if camera == FIRST_PERSON {
            self.camera_before = Some(FIRST_PERSON);
            return Some(THIRD_PERSON_FRONT);
        }
        None
    }

    /// The emote wheel chose `emote` (the service took it): the game side plays it on its next
    /// tick (`take_own`, then `play_own`), where it knows the player and the camera.
    pub fn play_own_later(&mut self, emote: &'static Cosmetic) {
        self.pending_own = Some(emote);
    }

    /// The emote the wheel chose since the last call.
    pub fn take_own(&mut self) -> Option<&'static Cosmetic> {
        self.pending_own.take()
    }

    fn start(&mut self, id: u128, emote: &'static Cosmetic) {
        let ends_at = self.ticks + u64::from(emotes::duration(emote.id));
        self.plays.insert(
            id,
            Play {
                emote,
                start: self.ticks,
                ends_at,
            },
        );
    }

    /// Once a client tick. `in_world`: a world is open. `played`: the emotes Incari players played
    /// since the last tick (`Online::take_emotes`: UUID and id). `own_moving`: the player's keys
    /// move them, they jump or swing. `camera`: the view now. Returns the view to set when the
    /// player's own emote ended and turned the camera.
    pub fn tick(
        &mut self,
        in_world: bool,
        show_others: bool,
        played: &[(u128, String)],
        own_moving: bool,
        camera: u8,
    ) -> Option<u8> {
        self.ticks += 1;
        self.trails_left = TRAIL_BUDGET;
        if !in_world {
            self.plays.clear();
            self.others.clear();
            self.own = None;
            // Left the world mid-emote: the camera goes back now, not with the next world.
            return self.emote_ended(camera);
        }
        if show_others {
            for (id, emote) in played {
                if let Some(e) = catalog::by_id(emote).filter(|e| e.slot == catalog::Slot::Emote) {
                    self.start(*id, e);
                }
            }
        }
        let (ticks, own) = (self.ticks, self.own);
        let mut mine_ended = false;
        self.plays.retain(|id, play| {
            let mine = Some(*id) == own;
            let over = ticks >= play.ends_at || (!mine && !show_others) || (mine && own_moving);
            mine_ended |= over && mine;
            !over
        });
        let mine_playing = own.is_some_and(|o| self.plays.contains_key(&o));
        if mine_ended || (self.camera_before.is_some() && !mine_playing) {
            return self.emote_ended(camera);
        }
        if ticks % 200 == 0 {
            // Players who left (not drawn for ten seconds), forgotten now and then.
            self.others.retain(|_, (_, _, drawn)| ticks - *drawn < 200);
        }
        None
    }

    fn emote_ended(&mut self, camera: u8) -> Option<u8> {
        let before = self.camera_before.take()?;
        // Unless the player moved the camera themselves meanwhile.
        (camera == THIRD_PERSON_FRONT).then_some(before)
    }

    /// Whether any emote plays (the game side poses players for it).
    pub fn any_playing(&self) -> bool {
        !self.plays.is_empty()
    }

    /// Whether the player's own emote plays: the camera faces them.
    pub fn own_playing(&self) -> bool {
        self.own.is_some_and(|o| self.plays.contains_key(&o))
    }

    /// The trail particle player `entity` (the game's entity id) leaves this tick wearing `trail`,
    /// `distance_sq` blocks² from the player and having moved `moved_sq` blocks² since the last
    /// tick; None when there's none to leave (every other tick, at most `TRAIL_BUDGET` a tick).
    pub fn trail(
        &mut self,
        trail: &Cosmetic,
        entity: u32,
        distance_sq: f64,
        moved_sq: f64,
    ) -> Option<Particle> {
        if self.ticks & 1 != 0 || self.trails_left == 0 {
            return None;
        }
        if distance_sq > TRAIL_DISTANCE * TRAIL_DISTANCE || moved_sq < 0.0004 {
            return None;
        }
        // Hearts are big: fewer of them.
        if trail.id == "trail.hearts" && !(self.ticks / 2 + u64::from(entity)).is_multiple_of(3) {
            return None;
        }
        self.trails_left -= 1;
        let mut r = || self.next_random();
        let (x, y, z) = ((r() - 0.5) * 0.5, 0.1 + r() * 0.2, (r() - 0.5) * 0.5);
        Some(match trail.id {
            "trail.hearts" => Particle {
                kind: "minecraft:heart",
                offset: [x, y + 0.3, z],
                velocity: [0.0; 3],
            },
            "trail.sparkles" => Particle {
                kind: "minecraft:end_rod",
                offset: [x, y + 0.2, z],
                velocity: [(r() - 0.5) * 0.02, 0.02, (r() - 0.5) * 0.02],
            },
            // A note's colour is its first speed (a pitch, 0 to 1).
            "trail.notes" => Particle {
                kind: "minecraft:note",
                offset: [x, y + 0.4, z],
                velocity: [r(), 0.0, 0.0],
            },
            _ => Particle {
                kind: "minecraft:snowflake",
                offset: [x, y, z],
                velocity: [0.0, 0.02, 0.0],
            },
        })
    }

    /// A number in [0, 1).
    fn next_random(&mut self) -> f64 {
        // xorshift64*: the trails only need to look scattered.
        let mut x = self.random.wrapping_add(0x9E37_79B9_7F4A_7C15) | 1;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.random = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
    }
}

thread_local! {
    static WORLD: RefCell<World> = RefCell::new(World::default());
}

/// Runs `f` with the world's cosmetics (the window's thread).
pub fn with<R>(f: impl FnOnce(&mut World) -> R) -> R {
    WORLD.with(|w| f(&mut w.borrow_mut()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(id: &str) -> &'static Cosmetic {
        catalog::by_id(id).unwrap()
    }

    #[test]
    fn others_show_only_as_incari_players_and_while_shown() {
        let mut w = World::default();
        let ids = vec!["hat.halo".to_owned()];
        assert_eq!(w.worn_by(7, false, Loadout::EMPTY, None, true), Worn::NONE);
        assert_eq!(
            w.worn_by(7, false, Loadout::EMPTY, Some(&ids), true)
                .loadout
                .ids(),
            ["hat.halo"]
        );
        assert_eq!(
            w.worn_by(7, false, Loadout::EMPTY, Some(&ids), false),
            Worn::NONE
        );
        let own = Loadout::of(&["cape.tide"]);
        assert_eq!(
            w.worn_by(1, true, own, None, false).loadout,
            own,
            "your own always show"
        );
    }

    #[test]
    fn your_emote_turns_the_camera_and_moving_ends_it() {
        let mut w = World::default();
        assert_eq!(
            w.play_own(1, c("emote.wave"), FIRST_PERSON),
            Some(THIRD_PERSON_FRONT)
        );
        assert!(w.own_playing());
        let worn = w.worn_by(1, true, Loadout::EMPTY, None, true);
        assert!(worn.pose(w.now(0.5) + 10.0).is_some());
        assert_eq!(w.tick(true, true, &[], false, THIRD_PERSON_FRONT), None);
        assert_eq!(
            w.tick(true, true, &[], true, THIRD_PERSON_FRONT),
            Some(FIRST_PERSON),
            "moved: back to first person"
        );
        assert!(!w.own_playing());
        // Played out, with the camera moved by hand meanwhile: left where the player put it.
        w.play_own(1, c("emote.dab"), FIRST_PERSON);
        for _ in 0..emotes::duration("emote.dab") - 1 {
            assert_eq!(w.tick(true, true, &[], false, 1), None);
        }
        assert_eq!(w.tick(true, true, &[], false, 1), None);
        assert!(!w.own_playing());
    }

    #[test]
    fn others_emotes_play_until_they_walk() {
        let mut w = World::default();
        w.tick(
            true,
            true,
            &[(9, "emote.bow".into()), (8, "emote.nope".into())],
            false,
            0,
        );
        assert!(
            w.worn_by(9, false, Loadout::EMPTY, None, true)
                .emote
                .is_some(),
            "an emote shows on anyone named"
        );
        assert_eq!(w.worn_by(8, false, Loadout::EMPTY, None, true), Worn::NONE);
        w.walking(9, 0.5);
        assert!(
            w.worn_by(9, false, Loadout::EMPTY, None, true)
                .emote
                .is_none()
        );
    }

    #[test]
    fn trails_keep_to_their_budget() {
        let mut w = World::default();
        w.tick(true, true, &[], false, 0);
        w.tick(true, true, &[], false, 0);
        let snow = c("trail.snow");
        let left = (0..20)
            .filter(|_| w.trail(snow, 1, 4.0, 1.0).is_some())
            .count();
        assert_eq!(left as u32, TRAIL_BUDGET);
        w.tick(true, true, &[], false, 0);
        assert!(w.trail(snow, 1, 4.0, 1.0).is_none(), "every other tick");
        w.tick(true, true, &[], false, 0);
        assert!(w.trail(snow, 1, 40.0 * 40.0, 1.0).is_none(), "too far");
        assert!(w.trail(snow, 1, 4.0, 0.0).is_none(), "standing still");
        let p = w.trail(snow, 1, 4.0, 1.0).unwrap();
        assert!(p.offset[0].abs() <= 0.25 && p.kind == "minecraft:snowflake");
    }
}
