// The held item in 1.7's poses: the block-hit's swing, the sword-blocking pose, the rod turned
// round and flat items held as 1.7 held them, in that order (ModuleHands's).
fn incari_oldanimations_pose(
    game: &crate::game::Game<'_>,
    hand: horizon_client::first_person::Hand,
    stack: &horizon_client::items::ItemStack,
    using: bool,
    pose: &mut glam::Mat4,
) {
    use crate::game::{incari_client as c, incari_oldanimations as m};
    let _ = game;
    let Some(o) = c::settings(m::ID, |s| m::Options {
        block_hit: s.bool("block-hit"),
        items: s.bool("items"),
        rod: s.bool("rod"),
        sword_block: s.bool("sword-block"),
    }) else {
        return;
    };
    // Horizon's hook doesn't say which arm holds the hand: the main hand's is taken as the right.
    let main = hand == horizon_client::first_person::Hand::Main;
    let invert = if main { 1.0 } else { -1.0 };
    let mut ops = m::Ops::default();
    if main && using {
        // The window's own milliseconds, for the block-hit's swing.
        thread_local! {
            static START: std::cell::Cell<Option<horizon_os::time::Instant>> =
                const { std::cell::Cell::new(None) };
        }
        let start = START.with(|s| {
            s.get().unwrap_or_else(|| {
                let now = horizon_os::time::Instant::now();
                s.set(Some(now));
                now
            })
        });
        let now = start.elapsed().as_secs_f64() * 1000.0;
        let swing = m::with(|b| {
            b.seen_using();
            b.swing(now)
        });
        if o.block_hit
            && let Some(a) = swing
        {
            m::attack(&mut ops, invert, a);
        }
    }
    let block_posed = o.sword_block && using && m::is_sword(&stack.item);
    if block_posed {
        m::block_pose(&mut ops, invert);
    }
    if o.rod && &*stack.item == m::FISHING_ROD {
        m::rod(&mut ops, invert);
    }
    if (o.items || block_posed) && m::held_flat(&stack.item) {
        m::legacy_item(&mut ops, invert);
    }
    for op in ops.as_slice() {
        *pose *= match *op {
            m::Op::Scale(s) => glam::Mat4::from_scale(glam::Vec3::from_array(s)),
            m::Op::TurnX(d) => glam::Mat4::from_rotation_x(d.to_radians()),
            m::Op::TurnY(d) => glam::Mat4::from_rotation_y(d.to_radians()),
            m::Op::TurnZ(d) => glam::Mat4::from_rotation_z(d.to_radians()),
            m::Op::Move(v) => glam::Mat4::from_translation(glam::Vec3::from_array(v)),
        };
    }
}
