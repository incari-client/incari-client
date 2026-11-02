//! Compares a placed schematic with the world, a 16-block section at a time, a few sections a
//! tick: which blocks are right, missing, the wrong block, the right block in the wrong state, or
//! there when the schematic wants air. Sections near the player are looked at again every few
//! ticks, so what they build shows at once; the rest in turn, for the counts and the material list.
//! A section whose chunk the client hasn't loaded counts as nothing yet.
//!
//! `tick` is a hot path: it allocates only when a section first needs room for its problems.
use super::placement::Placement;
use super::{Blocks, UNKNOWN};

/// A problem's kind, as `Section::issues` packs it.
pub const MISSING: u32 = 1;
pub const WRONG_BLOCK: u32 = 2;
pub const WRONG_STATE: u32 = 3;
pub const EXTRA: u32 = 4;

/// Blocks looked at in a tick, at most (a section past it is finished).
pub const BUDGET: usize = 8192;
/// How far round the player, in blocks, is "where they're building": looked at every few ticks.
pub const REACH: i32 = 6;
/// Ticks between looks at a section where the player is building.
pub const NEAR_TICKS: u64 = 4;
/// Ticks between looks at a section near enough that its problems are drawn.
pub const SHOWN_TICKS: u64 = 40;

/// One 16-block section of the world, as far as the schematic's box reaches into it.
#[derive(Debug, Default)]
pub struct Section {
    pub at: [i32; 3],
    /// The tick it was last looked at; None before the first.
    looked_at: Option<u64>,
    pub loaded: bool,
    /// Goes up whenever its missing blocks or problems change.
    pub revision: u64,
    hash: i32,
    pub right: u32,
    pub missing: u32,
    pub wrong_blocks: u32,
    pub wrong_states: u32,
    pub extra: u32,
    /// Problems other than missing blocks, while it's near enough to draw them: the kind in the
    /// low three bits, then the block in the section as `y << 8 | z << 4 | x`.
    pub issues: Option<Vec<u32>>,
    /// Right blocks by palette index: (index, how many).
    rights: Vec<(u32, u32)>,
}

impl Section {
    fn has_problems(&self) -> bool {
        self.wrong_blocks + self.wrong_states + self.extra > 0
    }

    pub fn issues(&self) -> &[u32] {
        self.issues.as_deref().unwrap_or(&[])
    }
}

#[derive(Debug)]
pub struct Verifier {
    lo: [i32; 3],
    hi: [i32; 3],
    min_section: [i32; 3],
    count: [i32; 3],
    sections: Vec<Section>,
    right_by_index: Vec<u32>,
    scratch: Vec<u32>,
    touched: Vec<u32>,
    right: u32,
    missing: u32,
    wrong_blocks: u32,
    wrong_states: u32,
    extra: u32,
    looked: usize,
    ticks: u64,
    cursor: usize,
}

impl Verifier {
    /// For `placement` as it stands now, with `palette_len` palette indices.
    pub fn new(placement: &Placement, palette_len: usize) -> Self {
        let (lo, hi) = (placement.min(), placement.max());
        let min_section = lo.map(|v| v >> 4);
        let count: [i32; 3] = std::array::from_fn(|i| ((hi[i] - 1) >> 4) - min_section[i] + 1);
        let mut sections = Vec::with_capacity((count[0] * count[1] * count[2]) as usize);
        for sy in 0..count[1] {
            for sz in 0..count[2] {
                for sx in 0..count[0] {
                    sections.push(Section {
                        at: [min_section[0] + sx, min_section[1] + sy, min_section[2] + sz],
                        ..Section::default()
                    });
                }
            }
        }
        Self {
            lo,
            hi,
            min_section,
            count,
            sections,
            right_by_index: vec![0; palette_len],
            scratch: vec![0; palette_len],
            touched: Vec::with_capacity(palette_len.min(4096)),
            right: 0,
            missing: 0,
            wrong_blocks: 0,
            wrong_states: 0,
            extra: 0,
            looked: 0,
            ticks: 0,
            cursor: 0,
        }
    }

    fn slot(&self, sx: i32, sy: i32, sz: i32) -> Option<usize> {
        let [ix, iy, iz] = [sx - self.min_section[0], sy - self.min_section[1], sz - self.min_section[2]];
        let [cx, cy, cz] = self.count;
        (ix >= 0 && iy >= 0 && iz >= 0 && ix < cx && iy < cy && iz < cz).then(|| ((iy * cz + iz) * cx + ix) as usize)
    }

    /// The section at section coordinates, or None if the box doesn't reach it.
    pub fn section(&self, sx: i32, sy: i32, sz: i32) -> Option<&Section> {
        self.slot(sx, sy, sz).map(|i| &self.sections[i])
    }

    /// A tick's looking, round the player at `at`: where they build first, then what's within
    /// `shown_range` blocks (whose problems are drawn), then the rest in turn.
    pub fn tick(&mut self, w: &World, at: [i32; 3], shown_range: i32) {
        self.ticks += 1;
        // Where they're building, but never more than half the tick.
        let mut budget = BUDGET as i64 - self.look_around(w, at, REACH, NEAR_TICKS, BUDGET / 2, shown_range) as i64;
        // One section in turn every tick, whatever else is due, so far ones are still counted.
        budget -= self.in_turn(w, at, shown_range, 1) as i64;
        if budget > 0 {
            budget -= self.look_around(w, at, shown_range, SHOWN_TICKS, budget as usize, shown_range) as i64;
        }
        if budget > 0 {
            self.in_turn(w, at, shown_range, budget as usize);
        }
    }

    fn in_turn(&mut self, w: &World, at: [i32; 3], shown_range: i32, budget: usize) -> usize {
        let mut spent = 0;
        let mut tries = 0;
        while spent < budget && tries < self.sections.len() {
            tries += 1;
            let i = self.cursor;
            self.cursor = (self.cursor + 1) % self.sections.len();
            let stale = self.sections[i].looked_at.is_none_or(|t| self.ticks - t > SHOWN_TICKS * 5);
            if stale {
                let near = near(&self.sections[i], at, shown_range);
                spent += self.look(w, i, near);
            }
        }
        spent
    }

    fn look_around(&mut self, w: &World, at: [i32; 3], range: i32, every: u64, budget: usize, shown_range: i32) -> usize {
        let mut spent = 0;
        let lo: [i32; 3] = std::array::from_fn(|i| self.min_section[i].max((at[i] - range) >> 4));
        let hi: [i32; 3] = std::array::from_fn(|i| (self.min_section[i] + self.count[i] - 1).min((at[i] + range) >> 4));
        for sy in lo[1]..=hi[1] {
            for sz in lo[2]..=hi[2] {
                for sx in lo[0]..=hi[0] {
                    if spent >= budget {
                        return spent;
                    }
                    let Some(i) = self.slot(sx, sy, sz) else {
                        continue;
                    };
                    let s = &self.sections[i];
                    let shown = near(s, at, shown_range);
                    let stale = s.looked_at.is_none_or(|t| self.ticks - t >= every);
                    // A section come into view needs its problems listed now, not at its next turn.
                    if stale || (shown && s.loaded && s.issues.is_none() && s.has_problems()) {
                        spent += self.look(w, i, shown);
                    }
                }
            }
        }
        spent
    }

    /// Compares one section with the world and updates the counts; returns how many blocks it
    /// looked at.
    fn look(&mut self, w: &World, i: usize, keep_issues: bool) -> usize {
        self.forget(i);
        let ticks = self.ticks;
        let (lo_box, hi_box) = (self.lo, self.hi);
        let s = &mut self.sections[i];
        s.looked_at = Some(ticks);
        let base = s.at.map(|v| v << 4);
        let lo: [i32; 3] = std::array::from_fn(|k| base[k].max(lo_box[k]));
        let hi: [i32; 3] = std::array::from_fn(|k| (base[k] + 16).min(hi_box[k]));
        let mut issues = if keep_issues { s.issues.take().or_else(|| Some(Vec::new())) } else { None };
        if let Some(list) = issues.as_mut() {
            list.clear();
        }
        let (mut right, mut missing, mut wrong_blocks, mut wrong_states, mut extra) = (0, 0, 0, 0, 0);
        let mut hash: i32 = 1;
        let mut loaded = true;
        let mut visited = 0;
        self.touched.clear();
        'scan: for y in lo[1]..hi[1] {
            for z in lo[2]..hi[2] {
                for x in lo[0]..hi[0] {
                    visited += 1;
                    let index = w.placement.index_at(x, y, z);
                    if index == 0 {
                        continue;
                    }
                    let want = w.states[index];
                    if want == UNKNOWN {
                        continue;
                    }
                    let have = w.blocks.at(x, y, z);
                    if have == UNKNOWN {
                        loaded = false;
                        break 'scan;
                    }
                    let kind = if have == want {
                        0
                    } else if w.blocks.is_air(want) {
                        if w.blocks.is_air(have) { 0 } else { EXTRA }
                    } else if w.blocks.is_air(have) {
                        MISSING
                    } else if w.blocks.block(have) == w.blocks.block(want) {
                        WRONG_STATE
                    } else {
                        WRONG_BLOCK
                    };
                    if kind == 0 {
                        if !w.blocks.is_air(want) {
                            right += 1;
                            if self.scratch[index] == 0 {
                                self.touched.push(index as u32);
                            }
                            self.scratch[index] += 1;
                        }
                        continue;
                    }
                    let issue = (((y & 15) << 8 | (z & 15) << 4 | (x & 15)) as u32) << 3 | kind;
                    hash = hash.wrapping_mul(31).wrapping_add(issue as i32);
                    match kind {
                        MISSING => missing += 1,
                        WRONG_BLOCK => wrong_blocks += 1,
                        WRONG_STATE => wrong_states += 1,
                        _ => extra += 1,
                    }
                    if kind != MISSING
                        && let Some(list) = issues.as_mut()
                    {
                        list.push(issue);
                    }
                }
            }
        }
        if !loaded {
            (right, missing, wrong_blocks, wrong_states, extra, hash) = (0, 0, 0, 0, 0, 0);
            for &t in &self.touched {
                self.scratch[t as usize] = 0;
            }
            self.touched.clear();
            if let Some(list) = issues.as_mut() {
                list.clear();
            }
        }
        let s = &mut self.sections[i];
        (s.right, s.missing, s.wrong_blocks, s.wrong_states, s.extra) = (right, missing, wrong_blocks, wrong_states, extra);
        s.loaded = loaded;
        s.issues = issues;
        s.rights.clear();
        for &t in &self.touched {
            s.rights.push((t, self.scratch[t as usize]));
            self.scratch[t as usize] = 0;
        }
        if hash != s.hash {
            s.hash = hash;
            s.revision += 1;
        }
        self.remember(i);
        visited.max(1)
    }

    /// Takes the section's counts out of the totals, before it's looked at again.
    fn forget(&mut self, i: usize) {
        let s = &mut self.sections[i];
        if s.looked_at.is_none() {
            return;
        }
        self.right -= s.right;
        self.missing -= s.missing;
        self.wrong_blocks -= s.wrong_blocks;
        self.wrong_states -= s.wrong_states;
        self.extra -= s.extra;
        self.looked -= 1;
        for &(index, n) in &s.rights {
            self.right_by_index[index as usize] -= n;
        }
        (s.right, s.missing, s.wrong_blocks, s.wrong_states, s.extra) = (0, 0, 0, 0, 0);
    }

    fn remember(&mut self, i: usize) {
        let s = &self.sections[i];
        self.right += s.right;
        self.missing += s.missing;
        self.wrong_blocks += s.wrong_blocks;
        self.wrong_states += s.wrong_states;
        self.extra += s.extra;
        self.looked += 1;
        for &(index, n) in &s.rights {
            self.right_by_index[index as usize] += n;
        }
    }

    /// Blocks found right, air left out.
    pub fn right(&self) -> u32 {
        self.right
    }
    pub fn missing(&self) -> u32 {
        self.missing
    }
    pub fn wrong_blocks(&self) -> u32 {
        self.wrong_blocks
    }
    pub fn wrong_states(&self) -> u32 {
        self.wrong_states
    }
    pub fn extra(&self) -> u32 {
        self.extra
    }
    /// Right blocks of palette index `index`, over the sections looked at.
    pub fn right_of(&self, index: usize) -> u32 {
        self.right_by_index[index]
    }
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }
    /// Whether every section has been looked at once.
    pub fn looked_at_all(&self) -> bool {
        self.looked == self.sections.len()
    }
}

/// What a look reads: the placement, the game's state for each palette index (mirrored and
/// turned as placed; `UNKNOWN` to leave alone) and the world.
pub struct World<'a> {
    pub placement: &'a Placement,
    pub states: &'a [u32],
    pub blocks: &'a dyn Blocks,
}

/// Whether the section is within `range` blocks of `at`, its problems kept to draw.
fn near(s: &Section, at: [i32; 3], range: i32) -> bool {
    let d: [i64; 3] = std::array::from_fn(|i| {
        let base = s.at[i] << 4;
        i64::from(0.max((base - at[i]).max(at[i] - base - 15)))
    });
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2] <= i64::from(range) * i64::from(range)
}

#[cfg(test)]
mod tests {
    use super::super::data::{Builder, Schematic};
    use super::super::fake::FakeBlocks;
    use super::super::spec::BlockSpec;
    use super::*;
    use std::rc::Rc;

    fn states(s: &Schematic, blocks: &FakeBlocks) -> Vec<u32> {
        s.palette
            .iter()
            .map(|p| if p.is_nothing() { UNKNOWN } else { blocks.state(&p.name, &p.properties, 0) })
            .collect()
    }

    /// A row of five along x at y 64: stone, stone, oak stairs facing north, air, planks.
    fn row() -> Placement {
        let mut b = Builder::new(5, 1, 1).unwrap();
        let stone = b.index(BlockSpec::of("minecraft:stone", Vec::new()));
        b.set(0, 0, 0, stone);
        b.set(1, 0, 0, stone);
        let stairs = b.index(BlockSpec::of("minecraft:oak_stairs", vec![("facing".into(), "north".into())]));
        b.set(2, 0, 0, stairs);
        let air = b.index(BlockSpec::of("minecraft:air", Vec::new()));
        b.set(3, 0, 0, air);
        let planks = b.index(BlockSpec::of("minecraft:oak_planks", Vec::new()));
        b.set(4, 0, 0, planks);
        Placement::new(Rc::new(b.build()), 10, 64, 20)
    }

    #[test]
    fn each_kind_of_problem_is_counted_and_listed() {
        let blocks = FakeBlocks::default();
        let p = row();
        let st = states(&p.schematic, &blocks);
        let mut v = Verifier::new(&p, st.len());
        blocks.set(10, 64, 20, "minecraft:stone");
        // 11: missing. 12: stairs the wrong way. 13: dirt where air is wanted. 14: glass, not planks.
        blocks.set(12, 64, 20, "minecraft:oak_stairs[facing=south]");
        blocks.set(13, 64, 20, "minecraft:dirt");
        blocks.set(14, 64, 20, "minecraft:glass");
        let w = World { placement: &p, states: &st, blocks: &blocks };
        v.tick(&w, [12, 64, 20], 48);
        assert_eq!((v.right(), v.missing(), v.wrong_states(), v.extra(), v.wrong_blocks()), (1, 1, 1, 1, 1));
        assert!(v.looked_at_all());
        let s = v.section(0, 4, 1).unwrap();
        let at = |x: u32| (4 << 4 | x) << 3;
        assert_eq!(s.issues(), &[at(12) | WRONG_STATE, at(13) | EXTRA, at(14) | WRONG_BLOCK]);
    }

    #[test]
    fn building_nearby_shows_within_a_few_ticks_and_changes_the_revision() {
        let blocks = FakeBlocks::default();
        let p = row();
        let st = states(&p.schematic, &blocks);
        let mut v = Verifier::new(&p, st.len());
        let w = World { placement: &p, states: &st, blocks: &blocks };
        v.tick(&w, [12, 64, 20], 48);
        assert_eq!(v.missing(), 4);
        let before = v.section(0, 4, 1).unwrap().revision;
        blocks.set(11, 64, 20, "minecraft:stone");
        for _ in 0..NEAR_TICKS {
            v.tick(&w, [12, 64, 20], 48);
        }
        assert_eq!((v.missing(), v.right()), (3, 1));
        assert_eq!(v.right_of(p.schematic.at(1, 0, 0)), 1);
        assert_ne!(v.section(0, 4, 1).unwrap().revision, before);
    }

    #[test]
    fn an_unloaded_chunk_counts_as_nothing_yet() {
        let blocks = FakeBlocks::default();
        let p = row();
        let st = states(&p.schematic, &blocks);
        let mut v = Verifier::new(&p, st.len());
        let w = World { placement: &p, states: &st, blocks: &blocks };
        blocks.loaded.set(false);
        v.tick(&w, [12, 64, 20], 48);
        assert_eq!(v.missing() + v.right() + v.extra(), 0);
        blocks.loaded.set(true);
        for _ in 0..NEAR_TICKS {
            v.tick(&w, [12, 64, 20], 48);
        }
        assert_eq!(v.missing(), 4);
    }

    #[test]
    fn a_tick_looks_at_no_more_than_its_budget() {
        let blocks = FakeBlocks::default();
        let mut b = Builder::new(96, 32, 96).unwrap();
        let stone = b.index(BlockSpec::of("minecraft:stone", Vec::new()));
        for y in 0..32 {
            for z in 0..96 {
                for x in 0..96 {
                    b.set(x, y, z, stone);
                }
            }
        }
        let p = Placement::new(Rc::new(b.build()), 0, 0, 0);
        let st = states(&p.schematic, &blocks);
        let mut v = Verifier::new(&p, st.len());
        let w = World { placement: &p, states: &st, blocks: &blocks };
        v.tick(&w, [48, 16, 48], 32);
        assert!(blocks.reads.get() <= BUDGET + 4096);
        for _ in 0..200 {
            v.tick(&w, [48, 16, 48], 32);
        }
        assert!(v.looked_at_all());
        assert_eq!(v.missing(), 96 * 32 * 96);
    }
}
