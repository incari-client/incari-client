// Incari's 1.7 Animations for the Rust build (ADR 0068, 0069), as features/oldanimations and the
// Java client's ModuleHands do it: first person as 1.7 drew it. Flat items held smaller and tilted,
// the fishing rod the other way round, 1.7's sword-blocking pose for a sword in use (on servers
// where swords block), and the block-hit: attacking a block while an item's in use swings the arm,
// drawn only. Horizon's hand pose hook moves the drawn item, before the model's own first-person
// transform, as the Java hook does. Its settings are the module's in `incari_core::modules`.
// No game types: rust/lib.rs tests it; the hook turns the steps here into matrices.

/// The module's id, as the Java client's profiles name it.
pub const ID: &str = "oldanimations";

pub const FISHING_ROD: &str = "minecraft:fishing_rod";

/// One step of a pose, as a PoseStack takes them: each applies after (to the right of) the last.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    Scale([f32; 3]),
    /// Degrees about x, y or z.
    TurnX(f32),
    TurnY(f32),
    TurnZ(f32),
    Move([f32; 3]),
}

/// Steps, without allocating: the hook runs for each hand each frame.
#[derive(Clone, Copy, Debug)]
pub struct Ops {
    ops: [Op; 24],
    len: usize,
}

impl Default for Ops {
    fn default() -> Self {
        Self {
            ops: [Op::Scale([1.0; 3]); 24],
            len: 0,
        }
    }
}

impl Ops {
    pub fn push(&mut self, op: Op) {
        if self.len < self.ops.len() {
            self.ops[self.len] = op;
            self.len += 1;
        }
    }

    pub fn as_slice(&self) -> &[Op] {
        &self.ops[..self.len]
    }
}

/// The module's settings, read once a hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub block_hit: bool,
    pub items: bool,
    pub rod: bool,
    pub sword_block: bool,
}

pub fn is_sword(item: &str) -> bool {
    item.ends_with("_sword")
}

/// Whether 1.7 held `item` as a flat item and the game's model holds it as flat and handheld
/// items are (`item/handheld`, `item/generated`: turned 0, -90, 25), the transform `legacy_item`
/// undoes. Horizon's hook doesn't say which model an item has, or whether it's a block, so these
/// are the ones known to be: tools, weapons, sticks and the bow.
pub fn held_flat(item: &str) -> bool {
    let Some(name) = item.strip_prefix("minecraft:") else {
        return false;
    };
    ["_sword", "_axe", "_pickaxe", "_shovel", "_hoe"]
        .iter()
        .any(|s| name.ends_with(s))
        || matches!(name, "stick" | "bone" | "blaze_rod" | "breeze_rod" | "bow")
}

/// 1.7's blocking pose for a sword in use, in place of today's (which Horizon doesn't draw: its
/// pose for an item in use is the arm's alone). `invert` is 1 for the right arm, -1 the left.
pub fn block_pose(ops: &mut Ops, invert: f32) {
    // 1.7's block turn, in the frame its item renderer worked in (turned 45 degrees, at 0.4 scale).
    ops.push(Op::TurnY(invert * 45.0));
    ops.push(Op::Scale([0.4; 3]));
    ops.push(Op::Move([invert * -0.5, 0.2, 0.0]));
    ops.push(Op::TurnY(invert * 30.0));
    ops.push(Op::TurnX(-80.0));
    ops.push(Op::TurnY(invert * 60.0));
    ops.push(Op::Scale([2.5; 3]));
    ops.push(Op::TurnY(invert * -45.0));
}

/// 1.7's fishing rod: turned round.
pub fn rod(ops: &mut Ops, invert: f32) {
    ops.push(Op::TurnY(invert * 180.0));
}

/// The first-person transform vanilla's flat and handheld models give the right hand: turned
/// (x, y, z) degrees, moved (in blocks), scaled. The left hand's is mirrored.
pub const FLAT_TURN: [f32; 3] = [0.0, -90.0, 25.0];
pub const FLAT_MOVE: [f32; 3] = [1.13 / 16.0, 3.2 / 16.0, 1.13 / 16.0];
pub const FLAT_SCALE: f32 = 0.68;

/// 1.7's flat item, from the arm: smaller, turned edge-on and tilted, its grip at the hand; then
/// the model's own first-person transform undone, so that transform leaves it where 1.7 put it.
pub fn legacy_item(ops: &mut Ops, invert: f32) {
    let tilt = 25f32.to_radians();
    ops.push(Op::Scale([0.6; 3]));
    ops.push(Op::TurnY(invert * 275.0));
    ops.push(Op::TurnZ(invert * 25.0));
    ops.push(Op::Move([
        invert * (-0.2 * tilt.sin() + 0.4375),
        -0.2 * tilt.cos() + 0.4375,
        0.03125,
    ]));
    // The inverse of `ItemTransform.apply` (moved, turned x then y then z, scaled), which mirrors
    // the move's x and the y and z turns for the left hand; that hand's sprite seen from behind,
    // as 1.7's pose mirrored for it wants.
    let left = invert < 0.0;
    if left {
        ops.push(Op::TurnY(180.0));
    }
    let [rx, ry, rz] = FLAT_TURN;
    let [tx, ty, tz] = FLAT_MOVE;
    ops.push(Op::Scale([1.0 / FLAT_SCALE; 3]));
    ops.push(Op::TurnZ(-rz));
    ops.push(Op::TurnY(-ry));
    ops.push(Op::TurnX(-rx));
    ops.push(Op::Move([-invert * tx, -ty, -tz]));
}

/// The attack swing's turn of the arm at `attack` (0 to 1), as it's drawn unused
/// (`applyItemArmAttackTransform`): what the block-hit adds to an item in use.
pub fn attack(ops: &mut Ops, invert: f32, attack: f32) {
    let pi = std::f32::consts::PI;
    let root = (attack.sqrt() * pi).sin();
    ops.push(Op::TurnY(
        invert * (45.0 + (attack * attack * pi).sin() * -20.0),
    ));
    ops.push(Op::TurnZ(invert * root * -20.0));
    ops.push(Op::TurnX(root * -80.0));
    ops.push(Op::TurnY(invert * -45.0));
}

/// A swing's length: vanilla's, without Haste or Mining Fatigue (6 ticks).
pub const SWING_MS: f64 = 300.0;

/// The block-hit's own swing: Horizon swings no arm while an item's in use, so the module times
/// its own, as `LivingEntity.swing` would.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BlockHit {
    /// Ticks so far.
    pub tick: u64,
    /// The tick the main hand was last drawn in use, and the crosshair last on a block.
    pub using: Option<u64>,
    pub block: Option<u64>,
    /// A tick asked for a swing.
    pub wanted: bool,
    /// When the swing started, in the hook's milliseconds.
    pub start: Option<f64>,
}

impl BlockHit {
    pub fn seen_using(&mut self) {
        self.using = Some(self.tick);
    }

    pub fn seen_block(&mut self) {
        self.block = Some(self.tick);
    }

    /// Once a tick: with the block-hit `on`, playing, and the attack key held while the main hand
    /// is in use and the crosshair is on a block (both seen since the last tick), the arm swings.
    pub fn tick(&mut self, on: bool, playing: bool, attack_held: bool) {
        let recent = |t: Option<u64>| t.is_some_and(|t| t >= self.tick);
        self.wanted = on && playing && attack_held && recent(self.using) && recent(self.block);
        self.tick += 1;
    }

    /// The swing's progress at `now` (0 to 1), None with no swing. A swing asked for starts
    /// again once the last is half done, as `LivingEntity.swing` does.
    pub fn swing(&mut self, now: f64) -> Option<f32> {
        if self.wanted {
            if self.start.is_none_or(|s| now - s >= SWING_MS / 2.0) {
                self.start = Some(now);
            }
            self.wanted = false;
        }
        let since = now - self.start?;
        if !(0.0..SWING_MS).contains(&since) {
            self.start = None;
            return None;
        }
        Some((since / SWING_MS) as f32)
    }
}

// The window's thread only: ticks and the hand's drawing both run there.
thread_local! {
    static BLOCK_HIT: std::cell::Cell<BlockHit> = std::cell::Cell::new(BlockHit::default());
}

/// Runs `f` on the block-hit's state.
pub fn with<R>(f: impl FnOnce(&mut BlockHit) -> R) -> R {
    BLOCK_HIT.with(|cell| {
        let mut state = cell.get();
        let r = f(&mut state);
        cell.set(state);
        r
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_flat_items_and_swords() {
        assert!(is_sword("minecraft:netherite_sword") && !is_sword("minecraft:shield"));
        assert!(held_flat("minecraft:diamond_pickaxe") && held_flat("minecraft:stick"));
        assert!(
            !held_flat("minecraft:stone")
                && !held_flat(FISHING_ROD)
                && !held_flat("minecraft:mace")
        );
    }

    #[test]
    fn poses_add_up_without_allocating() {
        let mut ops = Ops::default();
        block_pose(&mut ops, 1.0);
        legacy_item(&mut ops, -1.0);
        rod(&mut ops, 1.0);
        attack(&mut ops, 1.0, 0.5);
        assert_eq!(ops.as_slice().len(), 8 + 10 + 1 + 4);
        assert_eq!(ops.as_slice()[0], Op::TurnY(45.0));
        // At rest the swing turns nothing.
        let mut rest = Ops::default();
        attack(&mut rest, 1.0, 0.0);
        assert_eq!(rest.as_slice()[0], Op::TurnY(45.0));
        assert_eq!(rest.as_slice()[3], Op::TurnY(-45.0));
    }

    #[test]
    fn block_hits_while_using_on_a_block() {
        let mut b = BlockHit::default();
        b.seen_using();
        b.seen_block();
        b.tick(true, true, true);
        assert_eq!(b.swing(1000.0), Some(0.0));
        assert_eq!(b.swing(1150.0), Some(0.5));
        b.tick(true, true, true);
        let carried = b.swing(1200.0).unwrap();
        assert!(
            (carried - 2.0 / 3.0).abs() < 1e-6,
            "the stale use and block ask for nothing"
        );
        b.seen_using();
        b.seen_block();
        b.tick(true, true, true);
        assert_eq!(
            b.swing(1260.0),
            Some(0.0),
            "past half done: it starts again"
        );
        b.seen_using();
        b.seen_block();
        b.tick(true, true, true);
        assert_eq!(b.swing(1320.0), Some(0.2), "not half done: it carries on");
        assert_eq!(b.swing(1600.0), None);
        b.seen_using();
        b.seen_block();
        b.tick(false, true, true);
        assert_eq!(b.swing(2000.0), None);
    }
}
