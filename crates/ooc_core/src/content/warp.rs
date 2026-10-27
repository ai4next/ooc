//! Helpers shared by the full-load Lumen chapters (3 and 4): one warp, thirty-two lanes.

use super::{below, rng, stub};
use crate::isa::{LANES, OpKind};
use crate::task::*;
use crate::vm::Launch;

pub(super) fn base(chapter: u8, id: &'static str, slot: u8, ops: &'static [OpKind]) -> Task {
    Task {
        init: lanes_blank,
        ..stub(
            id,
            chapter,
            slot,
            View::Lanes,
            Editor::Code(ops),
            Launch::ONE_WARP,
            LANES,
        )
    }
}

/// `n` random values in `lo..hi`.
pub(super) fn values(seed: u64, n: usize, lo: i32, hi: i32) -> Vec<i32> {
    let mut r = rng(seed);
    (0..n).map(|_| lo + below(&mut r, hi - lo)).collect()
}

pub(super) fn rom_cases(make: impl Fn(u64) -> Vec<i32>, seeds: &[u64]) -> Vec<Case> {
    seeds
        .iter()
        .map(|&s| Case {
            rom: make(s),
            frame: 0,
        })
        .collect()
}

pub(super) fn lanes(f: impl Fn(usize) -> i32) -> Vec<Option<i32>> {
    (0..LANES).map(|l| Some(f(l))).collect()
}
