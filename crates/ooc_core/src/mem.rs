//! Vault's memory system: 400-cycle global memory behind a small L1, shared
//! memory in 32 banks, and the atomic unit.
//!
//! Everything here is timing and bookkeeping; the words themselves live in
//! the ROM, the output buffer and [`MemSys::smem`].

use std::collections::HashSet;

/// Cycles from a missed read to its data.
pub const GLOBAL_LAT: u32 = 400;
/// Cycles for an L1 hit.
pub const HIT_LAT: u32 = 4;
/// Words per cache line.
pub const LINE: i32 = 32;
pub const SETS: usize = 4;
pub const WAYS: usize = 4;
/// DRAM serves one line fill per this many cycles; misses queue behind it.
pub const FILL_GAP: u32 = 4;
pub const BANKS: i32 = 32;
pub const SMEM_WORDS: usize = 8192;
/// Cycles the atomic unit spends on each lane that hits the same address.
pub const ATOM_COST: u32 = 2;
pub const ATOM_LAT: u32 = 40;

/// Tags of output-buffer lines sit above every ROM line.
const OUT_TAG: u32 = 1 << 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    Rom,
    Out,
}

#[derive(Clone, Copy, Debug, Default)]
struct Line {
    tag: u32,
    valid: bool,
    last: u64,
    /// Which words of the line have been read since the fill.
    used: u32,
    ready: u32,
    fill_step: u32,
    warp: u16,
}

/// A line pushed out of L1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Eviction {
    pub cycle: u32,
    pub set: u8,
    pub tag: u32,
    /// Distinct words read while it lived there.
    pub used: u8,
    /// Issue index and warp of the read that filled it.
    pub fill_step: u32,
    pub warp: u16,
}

/// A line brought into L1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fill {
    pub cycle: u32,
    pub set: u8,
    pub tag: u32,
    pub step: u32,
    pub warp: u16,
    /// The line had been in L1 before and was pushed out.
    pub refill: bool,
}

#[derive(Clone, Debug, Default)]
pub struct MemLog {
    pub evictions: Vec<Eviction>,
    pub fills: Vec<Fill>,
}

/// What one warp-wide global read cost.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Access {
    /// Cycle at which the slowest line has arrived.
    pub ready: u32,
    /// Distinct cache lines touched: the transaction count.
    pub lines: u8,
    pub misses: u8,
}

#[derive(Clone, Debug)]
pub struct MemSys {
    l1: [[Line; WAYS]; SETS],
    tick: u64,
    dram_free: u32,
    evicted: HashSet<u32>,
    /// One [`SMEM_WORDS`] region per resident block.
    pub smem: Vec<i32>,
    smem_free: u32,
    atom_free: u32,
    /// Highest shared word touched, plus one.
    pub smem_hwm: usize,
    pub log: MemLog,
}

impl Default for MemSys {
    fn default() -> Self {
        Self::new(1)
    }
}

impl MemSys {
    /// A memory system with room for `slots` blocks' shared memory.
    pub fn new(slots: usize) -> Self {
        Self {
            l1: [[Line::default(); WAYS]; SETS],
            tick: 0,
            dram_free: 0,
            evicted: HashSet::new(),
            smem: vec![0; SMEM_WORDS * slots.max(1)],
            smem_free: 0,
            atom_free: 0,
            smem_hwm: 0,
            log: MemLog::default(),
        }
    }
}

/// Distinct words per bank, worst bank: how many passes the access takes.
pub fn bank_passes(addrs: &[i32]) -> u32 {
    let mut per: [Vec<i32>; BANKS as usize] = std::array::from_fn(|_| Vec::new());
    for &a in addrs {
        let b = &mut per[a.rem_euclid(BANKS) as usize];
        if !b.contains(&a) {
            b.push(a);
        }
    }
    per.iter().map(|b| b.len() as u32).max().unwrap_or(0)
}

/// The most lanes that hit one address.
pub fn longest_chain(addrs: &[i32]) -> u32 {
    let mut sorted = addrs.to_vec();
    sorted.sort_unstable();
    sorted
        .chunk_by(|a, b| a == b)
        .map(|c| c.len() as u32)
        .max()
        .unwrap_or(0)
}

impl MemSys {
    /// Zeroes the shared memory of a block slot that is about to be reused.
    pub fn clear_slot(&mut self, slot: usize) {
        let lo = slot * SMEM_WORDS;
        if let Some(r) = self.smem.get_mut(lo..lo + SMEM_WORDS) {
            r.fill(0);
        }
    }

    pub fn in_smem(addr: i32) -> Option<usize> {
        usize::try_from(addr).ok().filter(|&a| a < SMEM_WORDS)
    }

    /// One warp reads the given in-range global addresses at cycle `t`.
    pub fn global_load(
        &mut self,
        space: Space,
        addrs: &[i32],
        t: u32,
        warp: u16,
        step: u32,
    ) -> Access {
        let base = if space == Space::Out { OUT_TAG } else { 0 };
        let mut lines: Vec<(u32, u32)> = Vec::new();
        for &a in addrs {
            let tag = base + (a / LINE) as u32;
            let bit = 1u32 << (a % LINE);
            match lines.iter_mut().find(|(g, _)| *g == tag) {
                Some((_, m)) => *m |= bit,
                None => lines.push((tag, bit)),
            }
        }
        let mut acc = Access {
            ready: t + 2,
            lines: lines.len() as u8,
            misses: 0,
        };
        for (tag, mask) in lines {
            let (ready, miss) = self.touch(tag, mask, t, warp, step);
            acc.ready = acc.ready.max(ready);
            acc.misses += miss as u8;
        }
        acc
    }

    fn touch(&mut self, tag: u32, mask: u32, t: u32, warp: u16, step: u32) -> (u32, bool) {
        self.tick += 1;
        let set = tag as usize % SETS;
        if let Some(l) = self.l1[set].iter_mut().find(|l| l.valid && l.tag == tag) {
            l.last = self.tick;
            l.used |= mask;
            return ((t + HIT_LAT).max(l.ready), false);
        }
        let way = self.l1[set]
            .iter()
            .position(|l| !l.valid)
            .unwrap_or_else(|| (0..WAYS).min_by_key(|&w| self.l1[set][w].last).unwrap_or(0));
        let old = self.l1[set][way];
        if old.valid {
            self.evicted.insert(old.tag);
            self.log.evictions.push(Eviction {
                cycle: t,
                set: set as u8,
                tag: old.tag,
                used: old.used.count_ones() as u8,
                fill_step: old.fill_step,
                warp: old.warp,
            });
        }
        let start = t.max(self.dram_free);
        self.dram_free = start + FILL_GAP;
        let ready = start + GLOBAL_LAT;
        self.log.fills.push(Fill {
            cycle: t,
            set: set as u8,
            tag,
            step,
            warp,
            refill: self.evicted.contains(&tag),
        });
        self.l1[set][way] = Line {
            tag,
            valid: true,
            last: self.tick,
            used: mask,
            ready,
            fill_step: step,
            warp,
        };
        (ready, true)
    }

    /// Occupies the shared-memory pipe for `passes` cycles from `t`; returns
    /// the cycle the data is back.
    pub fn smem_access(&mut self, t: u32, passes: u32) -> u32 {
        let start = t.max(self.smem_free);
        self.smem_free = start + passes.max(1);
        self.smem_free + 1
    }

    /// The atomic unit works through `chain` same-address lanes one by one.
    pub fn atom_access(&mut self, t: u32, chain: u32) -> u32 {
        let start = t.max(self.atom_free);
        self.atom_free = start + chain.max(1) * ATOM_COST;
        self.atom_free + ATOM_LAT
    }

    /// Cycle by which every queued atomic has been applied.
    pub fn drained(&self) -> u32 {
        self.atom_free
    }

    pub fn touch_smem(&mut self, addr: usize) {
        self.smem_hwm = self.smem_hwm.max(addr + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banks_count_distinct_words() {
        let col: Vec<i32> = (0..32).map(|l| l * 32).collect();
        assert_eq!(bank_passes(&col), 32);
        let padded: Vec<i32> = (0..32).map(|l| l * 33).collect();
        assert_eq!(bank_passes(&padded), 1);
        // The same word read by every lane is a broadcast, not a conflict.
        assert_eq!(bank_passes(&[5; 32]), 1);
        let four: Vec<i32> = (0..32).map(|l| l * 36).collect();
        assert_eq!(bank_passes(&four), 4);
    }

    #[test]
    fn a_line_is_fetched_once_then_hits() {
        let mut m = MemSys::default();
        let a = m.global_load(Space::Rom, &[0], 0, 0, 0);
        assert_eq!((a.lines, a.misses, a.ready), (1, 1, GLOBAL_LAT));
        let b = m.global_load(Space::Rom, &[1], 1, 0, 1);
        assert_eq!((b.misses, b.ready), (0, GLOBAL_LAT));
        let c = m.global_load(Space::Rom, &[2], 500, 0, 2);
        assert_eq!(c.ready, 500 + HIT_LAT);
    }

    #[test]
    fn misses_queue_behind_the_dram() {
        let mut m = MemSys::default();
        let a = m.global_load(Space::Rom, &[0, 32, 64, 96], 0, 0, 0);
        assert_eq!((a.lines, a.misses), (4, 4));
        assert_eq!(a.ready, GLOBAL_LAT + 3 * FILL_GAP);
    }

    #[test]
    fn five_lines_in_one_set_evict_the_oldest() {
        let mut m = MemSys::default();
        // Lines 0, 4, 8, 12, 16 all map to set 0 of a 4-way set.
        for (i, line) in [0, 4, 8, 12, 16].into_iter().enumerate() {
            m.global_load(Space::Rom, &[line * LINE], i as u32, 0, i as u32);
        }
        assert_eq!(m.log.evictions.len(), 1);
        assert_eq!(m.log.evictions[0].tag, 0);
        assert_eq!(m.log.evictions[0].used, 1);
        m.global_load(Space::Rom, &[0], 9, 0, 9);
        assert!(m.log.fills.last().unwrap().refill);
    }
}
