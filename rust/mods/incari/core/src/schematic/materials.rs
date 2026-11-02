//! What a schematic takes to build, as items: how many of each in all, and how many are still
//! needed once the right blocks already in the world are taken off. A door is one item for its two
//! blocks, a double slab two; blocks no item places (water, fire) are left out. `refresh`
//! allocates nothing.
use super::data::Schematic;
use super::spec::BlockSpec;
use super::verifier::Verifier;
use super::{Blocks, UNKNOWN};
use std::collections::HashMap;

/// Properties that say how many items a block holds.
const COUNTED: [&str; 6] = ["candles", "pickles", "eggs", "layers", "flower_amount", "segment_amount"];

#[derive(Debug, Default)]
pub struct Materials {
    items: Vec<String>,
    totals: Vec<i64>,
    needed: Vec<i64>,
    order: Vec<usize>,
    /// By palette index: its item, and how many of it a block takes.
    item_of: Vec<Option<usize>>,
    cost_of: Vec<i64>,
    kinds_needed: usize,
    total_needed: i64,
}

/// How many items one block in state `spec` takes: none for the second half of a door, bed or
/// tall plant, two for a double slab, as many as it holds of candles, pickles, eggs, snow layers
/// and petals.
pub fn cost(spec: &BlockSpec) -> i64 {
    if spec.property("half", "") == "upper" || spec.property("part", "") == "head" {
        return 0;
    }
    if spec.name.ends_with("_slab") && spec.property("type", "") == "double" {
        return 2;
    }
    for key in COUNTED {
        if let Some((_, v)) = spec.properties.iter().find(|(k, _)| k == key) {
            return v.parse::<i64>().map_or(1, |n| n.max(1));
        }
    }
    1
}

impl Materials {
    /// `states`: the game's state for each palette index as the schematic has it (unturned),
    /// `UNKNOWN` for one the game hasn't.
    pub fn new(schematic: &Schematic, states: &[u32], blocks: &dyn Blocks) -> Self {
        let n = schematic.palette.len();
        let mut m = Self { item_of: vec![None; n], cost_of: vec![0; n], ..Self::default() };
        let mut by_id: HashMap<String, usize> = HashMap::new();
        for (i, spec) in schematic.palette.iter().enumerate() {
            let state = states[i];
            if spec.is_nothing() || spec.is_air() || state == UNKNOWN {
                continue;
            }
            let id = blocks.item_id(state);
            let c = cost(spec);
            if id.is_empty() || c == 0 {
                continue;
            }
            let item = *by_id.entry(id.clone()).or_insert_with(|| {
                m.items.push(id);
                m.totals.push(0);
                m.items.len() - 1
            });
            m.item_of[i] = Some(item);
            m.cost_of[i] = c;
            m.totals[item] += i64::from(schematic.count(i)) * c;
        }
        m.needed = m.totals.clone();
        m.order = (0..m.items.len()).collect();
        m.sort();
        m
    }

    /// Takes what's already built off the totals, and sorts by what's needed most.
    pub fn refresh(&mut self, verifier: &Verifier) {
        self.needed.copy_from_slice(&self.totals);
        for (i, item) in self.item_of.iter().enumerate() {
            if let Some(item) = *item {
                self.needed[item] -= i64::from(verifier.right_of(i)) * self.cost_of[i];
            }
        }
        self.sort();
    }

    /// Most needed first, then by id; and the counts of what's left.
    fn sort(&mut self) {
        self.kinds_needed = self.needed.iter().filter(|&&n| n > 0).count();
        self.total_needed = self.needed.iter().filter(|&&n| n > 0).sum();
        let (needed, items) = (&self.needed, &self.items);
        // In place, nothing allocated; ids are unique, so the order is the same every time.
        self.order.sort_unstable_by(|&a, &b| needed[b].cmp(&needed[a]).then_with(|| items[a].cmp(&items[b])));
    }

    /// How many kinds of item there are.
    pub fn kinds(&self) -> usize {
        self.items.len()
    }

    /// The `rank`th item, most needed first: its index for the getters.
    pub fn ranked(&self, rank: usize) -> usize {
        self.order[rank]
    }

    pub fn id(&self, item: usize) -> &str {
        &self.items[item]
    }

    pub fn total(&self, item: usize) -> i64 {
        self.totals[item]
    }

    /// What's still needed of it: never below 0.
    pub fn needed(&self, item: usize) -> i64 {
        self.needed[item].max(0)
    }

    /// How many kinds still need placing.
    pub fn kinds_needed(&self) -> usize {
        self.kinds_needed
    }

    /// How many items still need placing, all kinds together.
    pub fn total_needed(&self) -> i64 {
        self.total_needed
    }
}

#[cfg(test)]
mod tests {
    use super::super::data::Builder;
    use super::super::fake::FakeBlocks;
    use super::*;

    fn spec(text: &str) -> BlockSpec {
        BlockSpec::parse(text)
    }

    #[test]
    fn counts_items_not_blocks() {
        assert_eq!(cost(&spec("oak_door[half=upper]")), 0);
        assert_eq!(cost(&spec("oak_door[half=lower]")), 1);
        assert_eq!(cost(&spec("red_bed[part=head]")), 0);
        assert_eq!(cost(&spec("stone_slab[type=double]")), 2);
        assert_eq!(cost(&spec("candle[candles=3]")), 3);
        assert_eq!(cost(&spec("snow[layers=x]")), 1);
        assert_eq!(cost(&spec("stone")), 1);
    }

    #[test]
    fn lists_most_needed_first_and_leaves_out_what_no_item_places() {
        let blocks = FakeBlocks::default();
        let mut b = Builder::new(4, 1, 2).unwrap();
        let list = ["stone", "stone", "stone", "oak_slab[type=double]", "water", "wall_torch", "air", "torch"];
        for (i, t) in list.iter().enumerate() {
            let index = b.index(spec(t));
            b.set(i as i32 % 4, 0, i as i32 / 4, index);
        }
        let s = b.build();
        let states: Vec<u32> =
            s.palette.iter().map(|p| if p.is_nothing() { UNKNOWN } else { blocks.state(&p.name, &p.properties, 0) }).collect();
        let m = Materials::new(&s, &states, &blocks);
        let ranked: Vec<(&str, i64)> = (0..m.kinds()).map(|r| (m.id(m.ranked(r)), m.needed(m.ranked(r)))).collect();
        assert_eq!(ranked, [("minecraft:stone", 3), ("minecraft:oak_slab", 2), ("minecraft:torch", 2)]);
        assert_eq!((m.kinds_needed(), m.total_needed()), (3, 7));
    }
}
