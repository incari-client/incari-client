//! Emotes as the player model plays them (ADR 0061; `Emotes.java`): each a pose for the arms and
//! head, eased in and out, some moving as they go (a wave, a clap). Laid over the game's own pose
//! after it's set up, so the legs still walk if the player does, and sleeves, hats and capes follow
//! the parts they hang from. Pure, and nothing allocates: it runs while the model is posed.

/// Ticks an emote takes to ease in, and out.
const EASE: f32 = 4.0;

/// How long `emote` plays, in ticks.
pub fn duration(emote: &str) -> u32 {
    match emote {
        "emote.wave" | "emote.clap" | "emote.facepalm" => 50,
        "emote.cheer" => 45,
        "emote.salute" | "emote.bow" => 40,
        "emote.shrug" => 35,
        "emote.dab" => 30,
        _ => 40,
    }
}

/// What an emote turns, `weight` of the way from the game's pose (0 to 1): each part's x, y and z
/// rotation in radians, as the game's model parts take them. None leaves a part as it was.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pose {
    pub weight: f32,
    pub head: Option<[f32; 3]>,
    pub right_arm: Option<[f32; 3]>,
    pub left_arm: Option<[f32; 3]>,
}

impl Pose {
    /// `part`'s rotation `from`, moved toward this pose (`Mth.lerp` per angle).
    pub fn blend(weight: f32, from: [f32; 3], to: Option<[f32; 3]>) -> [f32; 3] {
        match to {
            Some(to) => [0, 1, 2].map(|i| from[i] + (to[i] - from[i]) * weight),
            None => from,
        }
    }
}

/// `emote`'s pose `elapsed` ticks after it started, or None once it's over (or before it began).
pub fn pose(emote: &str, elapsed: f32) -> Option<Pose> {
    let duration = duration(emote) as f32;
    if !(0.0..duration).contains(&elapsed) {
        return None;
    }
    let weight = (elapsed / EASE).min((duration - elapsed) / EASE).min(1.0);
    let e = elapsed;
    let mut p = Pose {
        weight,
        ..Pose::default()
    };
    match emote {
        "emote.wave" => p.right_arm = Some([0.0, 0.0, 2.6 + 0.35 * (e * 0.6).sin()]),
        "emote.clap" => {
            let swing = 0.25 + 0.3 * (0.5 + 0.5 * (e * 0.9).sin());
            p.right_arm = Some([-1.35, -swing, 0.0]);
            p.left_arm = Some([-1.35, swing, 0.0]);
        }
        "emote.cheer" => {
            let bounce = 0.15 * (e * 0.5).sin();
            p.right_arm = Some([-2.9 + bounce, 0.0, 0.35]);
            p.left_arm = Some([-2.9 - bounce, 0.0, -0.35]);
            p.head = Some([-0.25, 0.0, 0.0]);
        }
        "emote.salute" => {
            p.right_arm = Some([-2.3, -0.55, 0.1]);
            p.head = Some([-0.05, 0.0, 0.0]);
        }
        "emote.facepalm" => {
            p.right_arm = Some([-2.1, -0.75, 0.0]);
            p.head = Some([0.35, 0.0, 0.0]);
        }
        "emote.shrug" => {
            let lift = 0.1 * (e * 0.4).sin();
            p.right_arm = Some([-0.45, 0.0, 0.75 + lift]);
            p.left_arm = Some([-0.45, 0.0, -0.75 - lift]);
            p.head = Some([-0.1, 0.0, 0.12 * (e * 0.3).sin()]);
        }
        "emote.dab" => {
            p.right_arm = Some([-1.6, -1.0, 0.0]);
            p.left_arm = Some([-0.4, 0.0, -2.3]);
            p.head = Some([0.55, 0.6, 0.0]);
        }
        "emote.bow" => {
            p.right_arm = Some([-0.9, -0.6, 0.0]);
            p.left_arm = Some([0.3, 0.0, 0.0]);
            p.head = Some([0.75, 0.0, 0.0]);
        }
        _ => {}
    }
    Some(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cosmetics::catalog::{self, Slot};

    #[test]
    fn emotes_ease_in_and_out_and_end() {
        for e in catalog::in_slot(Slot::Emote) {
            let d = duration(e.id) as f32;
            assert_eq!(pose(e.id, 0.0).unwrap().weight, 0.0, "{}", e.id);
            assert_eq!(pose(e.id, d / 2.0).unwrap().weight, 1.0);
            assert!(pose(e.id, d).is_none() && pose(e.id, -1.0).is_none());
            let p = pose(e.id, d / 2.0).unwrap();
            assert!(
                p.right_arm.is_some() || p.head.is_some(),
                "{} turns something",
                e.id
            );
        }
        assert_eq!(
            Pose::blend(0.5, [0.0, 2.0, 0.0], Some([1.0, 0.0, 0.0])),
            [0.5, 1.0, 0.0]
        );
    }
}
