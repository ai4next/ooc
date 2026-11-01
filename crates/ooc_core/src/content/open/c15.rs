//! Chapter 15 open problems: Juye's origin machine (idle SMs, a bucket
//! brigade across the links, a ring of coupled stones).

use oo_base::Tr;

use super::Open;
use crate::content::origin::{ORIGIN_INT_OPS, base};
use crate::content::{below, rng};
use crate::task::*;
use crate::vm::Launch;

pub fn problems() -> Vec<Open> {
    vec![all_hands(), bucket_brigade(), stone_ring()]
}

fn case(rom: Vec<i32>) -> Case {
    Case { rom, frame: 0 }
}

// ---------------------------------------------------------------- all-hands

/// Collatz steps from n down to 1.
fn steps(mut n: i32) -> i32 {
    let mut k = 0;
    while n > 1 {
        n = if n % 2 == 0 { n / 2 } else { 3 * n + 1 };
        k += 1;
    }
    k
}

/// 2048 numbers below 3000; where `heavy(i)` holds, ones of at least 100
/// steps, elsewhere ones of at most 20.
fn hands(seed: u64, heavy: fn(usize) -> bool) -> Case {
    let mut r = rng(seed);
    case(
        (0..2048)
            .map(|i| loop {
                let n = 1 + below(&mut r, 2999);
                let s = steps(n);
                if (heavy(i) && s >= 100) || (!heavy(i) && s <= 20) {
                    break n;
                }
            })
            .collect(),
    )
}

/// out[i] = steps of rom[i]; out[2048..2056) is free (a counter starts at 0).
fn stepped(c: &Case) -> Vec<Option<i32>> {
    let mut out: Vec<Option<i32>> = c.rom.iter().map(|&n| Some(steps(n))).collect();
    out.resize(2056, None);
    out
}

/// The numbers are copied to shared memory; each round one lane takes the
/// next 32 from the counter at out[2048] (asked for a round ahead, so the
/// answer is back in time), and the warp walks them 32 steps at a time
/// while any lane is still on its way.
const HANDS_REF: &str = "DMA 0, 0, 2048\n\
    SETP.EQ p0, lane, 0\n\
    IF p0\n\
    ATOM r9, [2048], 32\n\
    ENDIF\n\
    BAR\n\
    SHFL r8, r9, 0\n\
    LOOP 64\n\
    IF p0\n\
    ATOM r9, [2048], 32\n\
    ENDIF\n\
    ADD r6, r8, lane\n\
    MIN r7, r6, 2047\n\
    LDS r1, [r7]\n\
    SETP.LT p1, r6, 2048\n\
    SEL r1, p1, r1, 1\n\
    MOV r2, 0\n\
    LOOP 8\n\
    SETP.GT p2, r1, 1\n\
    VOTE.ANY p3, p2\n\
    IF p3\n\
    LOOP 32\n\
    SETP.GT p2, r1, 1\n\
    AND r3, r1, 1\n\
    SETP.EQ p3, r3, 0\n\
    SHR r4, r1, 1\n\
    MUL r5, r1, 3\n\
    ADD r5, r5, 1\n\
    SEL r4, p3, r4, r5\n\
    SEL r1, p2, r4, r1\n\
    SEL r3, p2, 1, 0\n\
    ADD r2, r2, r3\n\
    ENDLOOP\n\
    ENDIF\n\
    ENDLOOP\n\
    IF p1\n\
    ST [r6], r2\n\
    ENDIF\n\
    SHFL r8, r9, 0\n\
    ENDLOOP\n";

/// A fixed quarter per SM: blocks of 512 in index order.
#[cfg(test)]
const HANDS_FIXED: &str = "SHL r6, bx, 9\n\
    ADD r6, r6, lane\n\
    LOOP 16\n\
    LD r1, [r6]\n\
    MOV r2, 0\n\
    LOOP 8\n\
    SETP.GT p2, r1, 1\n\
    VOTE.ANY p3, p2\n\
    IF p3\n\
    LOOP 32\n\
    SETP.GT p2, r1, 1\n\
    AND r3, r1, 1\n\
    SETP.EQ p3, r3, 0\n\
    SHR r4, r1, 1\n\
    MUL r5, r1, 3\n\
    ADD r5, r5, 1\n\
    SEL r4, p3, r4, r5\n\
    SEL r1, p2, r4, r1\n\
    SEL r3, p2, 1, 0\n\
    ADD r2, r2, r3\n\
    ENDLOOP\n\
    ENDIF\n\
    ENDLOOP\n\
    ST [r6], r2\n\
    ADD r6, r6, 32\n\
    ENDLOOP\n";

/// A fixed share dealt out in turn: SM b takes the groups of 32 at b, b + 4, ...
#[cfg(test)]
const HANDS_INTER: &str = "SHL r6, bx, 5\n\
    ADD r6, r6, lane\n\
    LOOP 16\n\
    LD r1, [r6]\n\
    MOV r2, 0\n\
    LOOP 8\n\
    SETP.GT p2, r1, 1\n\
    VOTE.ANY p3, p2\n\
    IF p3\n\
    LOOP 32\n\
    SETP.GT p2, r1, 1\n\
    AND r3, r1, 1\n\
    SETP.EQ p3, r3, 0\n\
    SHR r4, r1, 1\n\
    MUL r5, r1, 3\n\
    ADD r5, r5, 1\n\
    SEL r4, p3, r4, r5\n\
    SEL r1, p2, r4, r1\n\
    SEL r3, p2, 1, 0\n\
    ADD r2, r2, r3\n\
    ENDLOOP\n\
    ENDIF\n\
    ENDLOOP\n\
    ST [r6], r2\n\
    ADD r6, r6, 128\n\
    ENDLOOP\n";

fn all_hands() -> Open {
    Open {
        task: Task {
            chapter: 15,
            slot: 9,
            title: Tr::new("All Hands", "全员上工"),
            brief: Tr::new(
                "Juye's 2048 numbers each go down the old 3n + 1 road: halve an even n, turn an odd one into 3n + 1, until it reaches 1. rom[i] is the number (below 3000), out[i] the steps it takes. Some stretches are far longer than others.\nFour SMs, one warp each; out[2048..2056) is yours and starts at 0. Scored by the idle share of the idlest SM: the goal line is 10%.",
                "橘爷有 2048 个数，每个都走老路 3n + 1：偶数减半，奇数变成 3n + 1，一直走到 1。rom[i] 是那个数（小于 3000），out[i] 写它走的步数。有些段比别的长得多。\n四个 SM，各一个 warp；out[2048..2056) 归你用，初值为 0。按最闲的 SM 的空闲比例计分：目标线是 10%。",
            ),
            cases: || {
                vec![
                    hands(15901, |i| i < 512),
                    hands(15902, |i| i % 7 == 0),
                    hands(15903, |i| i >= 1792),
                ]
            },
            hidden: || vec![hands(15904, |i| (1024..1536).contains(&i)), hands(15905, |i| i % 3 == 1)],
            expect: stepped,
            starter: Source::Code(""),
            reference: Source::Code(HANDS_REF),
            ..base("all-hands", 15, 9, Launch::grid(4, 32), 2056, ORIGIN_INT_OPS)
        },
        metric: Metric::Idle,
        score: None,
        score_label: None,
        marks: &[8, 41, 85],
    }
}

// ---------------------------------------------------------------- bucket-brigade

/// Juye's notice: 1024 words below 10000.
fn notice(seed: u64) -> Case {
    let mut r = rng(seed);
    case((0..1024).map(|_| below(&mut r, 10000)).collect())
}

/// Every SM pins up its own copy: out[1024 s + j] = rom[j].
fn pinned(c: &Case) -> Vec<Option<i32>> {
    (0..4).flat_map(|_| c.rom.iter().map(|&v| Some(v))).collect()
}

/// SM 0 sends its first half round one way (0, 1, 2, 3) and its second half
/// the other (0, 3, 2, 1), 64 words a message. The first two chunks of each
/// half come in by LD while the DMA brings the rest, so the links start a
/// hundred cycles early. Each SM forwards a chunk the moment it arrives and
/// pins it up; SM 2 takes the two streams in turn.
const BRIGADE_REF: &str = "SHL r14, bx, 10\n\
    SETP.EQ p0, bx, 0\n\
    IF p0\n\
    MOV r6, lane\n\
    LD r31, [r6]\n\
    ADD r5, r6, 32\n\
    LD r30, [r5]\n\
    ADD r5, r6, 64\n\
    LD r29, [r5]\n\
    ADD r5, r6, 96\n\
    LD r28, [r5]\n\
    ADD r5, r6, 512\n\
    LD r27, [r5]\n\
    ADD r5, r6, 544\n\
    LD r26, [r5]\n\
    ADD r5, r6, 576\n\
    LD r25, [r5]\n\
    ADD r5, r6, 608\n\
    LD r24, [r5]\n\
    DMA 128, 128, 384\n\
    DMA 640, 640, 384\n\
    STS [r6], r31\n\
    ADD r5, r6, 32\n\
    STS [r5], r30\n\
    SEND 1, 0, 64\n\
    ADD r5, r6, 512\n\
    STS [r5], r27\n\
    ADD r5, r6, 544\n\
    STS [r5], r26\n\
    SEND 3, 512, 64\n\
    ADD r5, r6, 64\n\
    STS [r5], r29\n\
    ADD r5, r6, 96\n\
    STS [r5], r28\n\
    SEND 1, 64, 64\n\
    ADD r5, r6, 576\n\
    STS [r5], r25\n\
    ADD r5, r6, 608\n\
    STS [r5], r24\n\
    SEND 3, 576, 64\n\
    BAR\n\
    MOV r1, 128\n\
    MOV r2, 640\n\
    LOOP 6\n\
    SEND 1, r1, 64\n\
    SEND 3, r2, 64\n\
    ADD r1, r1, 64\n\
    ADD r2, r2, 64\n\
    ENDLOOP\n\
    MOV r6, lane\n\
    LOOP 32\n\
    LDS r7, [r6]\n\
    ST [r6], r7\n\
    ADD r6, r6, 32\n\
    ENDLOOP\n\
    ENDIF\n\
    SETP.EQ p1, bx, 2\n\
    IF p1\n\
    MOV r1, 0\n\
    MOV r2, 512\n\
    LOOP 8\n\
    RECV 1, r1, 64\n\
    SEND 3, r1, 64\n\
    RECV 3, r2, 64\n\
    SEND 1, r2, 64\n\
    ADD r6, r1, lane\n\
    ADD r8, r6, r14\n\
    LDS r7, [r6]\n\
    ADD r6, r6, 32\n\
    LDS r9, [r6]\n\
    ST [r8], r7\n\
    ADD r8, r8, 32\n\
    ST [r8], r9\n\
    ADD r6, r2, lane\n\
    ADD r8, r6, r14\n\
    LDS r7, [r6]\n\
    ADD r6, r6, 32\n\
    LDS r9, [r6]\n\
    ST [r8], r7\n\
    ADD r8, r8, 32\n\
    ST [r8], r9\n\
    ADD r1, r1, 64\n\
    ADD r2, r2, 64\n\
    ENDLOOP\n\
    ENDIF\n\
    AND r3, bx, 1\n\
    SETP.EQ p1, r3, 1\n\
    IF p1\n\
    SHR r1, bx, 1\n\
    SHL r1, r1, 9\n\
    LOOP 8\n\
    RECV 0, r1, 64\n\
    SEND 2, r1, 64\n\
    ADD r6, r1, lane\n\
    ADD r8, r6, r14\n\
    LDS r7, [r6]\n\
    ADD r6, r6, 32\n\
    LDS r9, [r6]\n\
    ST [r8], r7\n\
    ADD r8, r8, 32\n\
    ST [r8], r9\n\
    ADD r1, r1, 64\n\
    ENDLOOP\n\
    AND r1, r1, 1023\n\
    LOOP 8\n\
    RECV 2, r1, 64\n\
    ADD r6, r1, lane\n\
    ADD r8, r6, r14\n\
    LDS r7, [r6]\n\
    ADD r6, r6, 32\n\
    LDS r9, [r6]\n\
    ST [r8], r7\n\
    ADD r8, r8, 32\n\
    ST [r8], r9\n\
    ADD r1, r1, 64\n\
    ENDLOOP\n\
    ENDIF\n";

/// The same routes with one DMA for the whole notice before the first SEND.
#[cfg(test)]
const BRIGADE_ONE_DMA: &str = "SHL r14, bx, 10\n\
    SETP.EQ p0, bx, 0\n\
    IF p0\n\
    DMA 0, 0, 1024\n\
    BAR\n\
    MOV r1, 0\n\
    MOV r2, 512\n\
    LOOP 8\n\
    SEND 1, r1, 64\n\
    SEND 3, r2, 64\n\
    ADD r1, r1, 64\n\
    ADD r2, r2, 64\n\
    ENDLOOP\n\
    MOV r6, lane\n\
    LOOP 32\n\
    LDS r7, [r6]\n\
    ST [r6], r7\n\
    ADD r6, r6, 32\n\
    ENDLOOP\n\
    ENDIF\n\
    SETP.EQ p1, bx, 2\n\
    IF p1\n\
    MOV r1, 0\n\
    MOV r2, 512\n\
    LOOP 8\n\
    RECV 1, r1, 64\n\
    SEND 3, r1, 64\n\
    RECV 3, r2, 64\n\
    SEND 1, r2, 64\n\
    ADD r6, r1, lane\n\
    ADD r8, r6, r14\n\
    LDS r7, [r6]\n\
    ADD r6, r6, 32\n\
    LDS r9, [r6]\n\
    ST [r8], r7\n\
    ADD r8, r8, 32\n\
    ST [r8], r9\n\
    ADD r6, r2, lane\n\
    ADD r8, r6, r14\n\
    LDS r7, [r6]\n\
    ADD r6, r6, 32\n\
    LDS r9, [r6]\n\
    ST [r8], r7\n\
    ADD r8, r8, 32\n\
    ST [r8], r9\n\
    ADD r1, r1, 64\n\
    ADD r2, r2, 64\n\
    ENDLOOP\n\
    ENDIF\n\
    AND r3, bx, 1\n\
    SETP.EQ p1, r3, 1\n\
    IF p1\n\
    SHR r1, bx, 1\n\
    SHL r1, r1, 9\n\
    LOOP 8\n\
    RECV 0, r1, 64\n\
    SEND 2, r1, 64\n\
    ADD r6, r1, lane\n\
    ADD r8, r6, r14\n\
    LDS r7, [r6]\n\
    ADD r6, r6, 32\n\
    LDS r9, [r6]\n\
    ST [r8], r7\n\
    ADD r8, r8, 32\n\
    ST [r8], r9\n\
    ADD r1, r1, 64\n\
    ENDLOOP\n\
    AND r1, r1, 1023\n\
    LOOP 8\n\
    RECV 2, r1, 64\n\
    ADD r6, r1, lane\n\
    ADD r8, r6, r14\n\
    LDS r7, [r6]\n\
    ADD r6, r6, 32\n\
    LDS r9, [r6]\n\
    ST [r8], r7\n\
    ADD r8, r8, 32\n\
    ST [r8], r9\n\
    ADD r1, r1, 64\n\
    ENDLOOP\n\
    ENDIF\n";

/// SM 0 sends the whole notice both ways; SM 1 passes it on to SM 2.
#[cfg(test)]
const BRIGADE_BOTH: &str = "SHL r14, bx, 10\n\
    SETP.EQ p0, bx, 0\n\
    IF p0\n\
    DMA 0, 0, 1024\n\
    BAR\n\
    MOV r1, 0\n\
    LOOP 16\n\
    SEND 1, r1, 64\n\
    SEND 3, r1, 64\n\
    ADD r1, r1, 64\n\
    ENDLOOP\n\
    MOV r6, lane\n\
    LOOP 32\n\
    LDS r7, [r6]\n\
    ST [r6], r7\n\
    ADD r6, r6, 32\n\
    ENDLOOP\n\
    ELSE\n\
    SETP.EQ p1, bx, 2\n\
    SEL r10, p1, 1, 0\n\
    SETP.EQ p2, bx, 1\n\
    MOV r11, 0\n\
    LOOP 16\n\
    RECV r10, r11, 64\n\
    IF p2\n\
    SEND 2, r11, 64\n\
    ENDIF\n\
    ADD r6, r11, lane\n\
    LDS r7, [r6]\n\
    ADD r8, r6, r14\n\
    ST [r8], r7\n\
    ADD r6, r6, 32\n\
    LDS r7, [r6]\n\
    ADD r8, r6, r14\n\
    ST [r8], r7\n\
    ADD r11, r11, 64\n\
    ENDLOOP\n\
    ENDIF\n";

/// A chain 0, 1, 2, 3 where each SM takes the whole notice before passing it on.
#[cfg(test)]
const BRIGADE_CHAIN: &str = "SHL r14, bx, 10\n\
    SETP.EQ p0, bx, 0\n\
    IF p0\n\
    DMA 0, 0, 1024\n\
    BAR\n\
    ELSE\n\
    SUB r10, bx, 1\n\
    MOV r11, 0\n\
    LOOP 16\n\
    RECV r10, r11, 64\n\
    ADD r11, r11, 64\n\
    ENDLOOP\n\
    ENDIF\n\
    SETP.LT p2, bx, 3\n\
    IF p2\n\
    ADD r12, bx, 1\n\
    MOV r11, 0\n\
    LOOP 16\n\
    SEND r12, r11, 64\n\
    ADD r11, r11, 64\n\
    ENDLOOP\n\
    ENDIF\n\
    MOV r6, lane\n\
    LOOP 32\n\
    LDS r7, [r6]\n\
    ADD r8, r6, r14\n\
    ST [r8], r7\n\
    ADD r6, r6, 32\n\
    ENDLOOP\n";

fn bucket_brigade() -> Open {
    Open {
        task: Task {
            chapter: 15,
            slot: 10,
            title: Tr::new("Bucket Brigade", "传水桶"),
            brief: Tr::new(
                "Juye's notice is 1024 words, rom[0..1024), and every post station pins up its own copy: SM s writes out[1024 s + j] = rom[j]. Only SM 0 may read rom; the others hear it over the links (64 words a message at most).\nFour SMs, one warp each. Scored by total cycles: fewer is better.",
                "橘爷的告示有 1024 个字，rom[0..1024)，每个驿站都要贴一份：SM s 写 out[1024 s + j] = rom[j]。只有 SM 0 能读 rom；别的 SM 只能从链路上听到（一条消息最多 64 个字）。\n四个 SM，各一个 warp。按总周期计分：越少越好。",
            ),
            cases: || vec![notice(15911), notice(15912), notice(15913)],
            hidden: || vec![notice(15914), notice(15915)],
            expect: pinned,
            starter: Source::Code(""),
            reference: Source::Code(BRIGADE_REF),
            ..base("bucket-brigade", 15, 10, Launch::grid(4, 32), 4096, ORIGIN_INT_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[836, 898, 1004, 1902],
    }
}

// ---------------------------------------------------------------- stone-ring

/// Steps of the ring.
const RING_STEPS: usize = 48;

/// 1024 stones in a ring, 0..9999; `spikes` of them at 30000.
fn stones(seed: u64, spikes: usize) -> Case {
    let mut r = rng(seed);
    let mut rom: Vec<i32> = (0..1024).map(|_| below(&mut r, 10000)).collect();
    for _ in 0..spikes {
        rom[below(&mut r, 1024) as usize] = 30000;
    }
    case(rom)
}

/// 48 steps of v' = (2v + left + right) >> 2 round the ring.
fn rung(c: &Case) -> Vec<Option<i32>> {
    let mut v = c.rom.clone();
    for _ in 0..RING_STEPS {
        v = (0..1024)
            .map(|i| (2 * v[i] + v[(i + 1023) % 1024] + v[(i + 1) % 1024]) >> 2)
            .collect();
    }
    v.into_iter().map(Some).collect()
}

/// Each lane keeps nine stones in registers: the SM's 256 and 16 more on
/// either side, so the links are used once every 16 steps. Rubbish creeps in
/// from the far ends one stone a step and never reaches the SM's own.
const RING_REF: &str = "MUL r16, lane, 9\n\
    ADD r17, lane, 31\n\
    AND r17, r17, 31\n\
    ADD r18, lane, 1\n\
    AND r18, r18, 31\n\
    ADD r19, bx, 3\n\
    AND r19, r19, 3\n\
    ADD r20, bx, 1\n\
    AND r20, r20, 3\n\
    SHL r22, bx, 8\n\
    DMA 16, r22, 256\n\
    BAR\n\
    LOOP 3\n\
    SEND r19, 16, 16\n\
    SEND r20, 256, 16\n\
    RECV r19, 0, 16\n\
    RECV r20, 272, 16\n\
    LDS r0, [r16]\n\
    ADD r21, r16, 1\n\
    LDS r1, [r21]\n\
    ADD r21, r16, 2\n\
    LDS r2, [r21]\n\
    ADD r21, r16, 3\n\
    LDS r3, [r21]\n\
    ADD r21, r16, 4\n\
    LDS r4, [r21]\n\
    ADD r21, r16, 5\n\
    LDS r5, [r21]\n\
    ADD r21, r16, 6\n\
    LDS r6, [r21]\n\
    ADD r21, r16, 7\n\
    LDS r7, [r21]\n\
    ADD r21, r16, 8\n\
    LDS r8, [r21]\n\
    LOOP 16\n\
    SHFL r12, r8, r17\n\
    SHFL r15, r0, r18\n\
    ADD r13, r12, r1\n\
    MOV r12, r0\n\
    SHL r0, r0, 1\n\
    ADD r0, r0, r13\n\
    SHR r0, r0, 2\n\
    ADD r13, r12, r2\n\
    MOV r12, r1\n\
    SHL r1, r1, 1\n\
    ADD r1, r1, r13\n\
    SHR r1, r1, 2\n\
    ADD r13, r12, r3\n\
    MOV r12, r2\n\
    SHL r2, r2, 1\n\
    ADD r2, r2, r13\n\
    SHR r2, r2, 2\n\
    ADD r13, r12, r4\n\
    MOV r12, r3\n\
    SHL r3, r3, 1\n\
    ADD r3, r3, r13\n\
    SHR r3, r3, 2\n\
    ADD r13, r12, r5\n\
    MOV r12, r4\n\
    SHL r4, r4, 1\n\
    ADD r4, r4, r13\n\
    SHR r4, r4, 2\n\
    ADD r13, r12, r6\n\
    MOV r12, r5\n\
    SHL r5, r5, 1\n\
    ADD r5, r5, r13\n\
    SHR r5, r5, 2\n\
    ADD r13, r12, r7\n\
    MOV r12, r6\n\
    SHL r6, r6, 1\n\
    ADD r6, r6, r13\n\
    SHR r6, r6, 2\n\
    ADD r13, r12, r8\n\
    MOV r12, r7\n\
    SHL r7, r7, 1\n\
    ADD r7, r7, r13\n\
    SHR r7, r7, 2\n\
    ADD r13, r12, r15\n\
    SHL r8, r8, 1\n\
    ADD r8, r8, r13\n\
    SHR r8, r8, 2\n\
    ENDLOOP\n\
    STS [r16], r0\n\
    ADD r21, r16, 1\n\
    STS [r21], r1\n\
    ADD r21, r16, 2\n\
    STS [r21], r2\n\
    ADD r21, r16, 3\n\
    STS [r21], r3\n\
    ADD r21, r16, 4\n\
    STS [r21], r4\n\
    ADD r21, r16, 5\n\
    STS [r21], r5\n\
    ADD r21, r16, 6\n\
    STS [r21], r6\n\
    ADD r21, r16, 7\n\
    STS [r21], r7\n\
    ADD r21, r16, 8\n\
    STS [r21], r8\n\
    ENDLOOP\n\
    ADD r6, lane, 16\n\
    ADD r8, r22, lane\n\
    LOOP 8\n\
    LDS r1, [r6]\n\
    ST [r8], r1\n\
    ADD r6, r6, 32\n\
    ADD r8, r8, 32\n\
    ENDLOOP\n";

/// The same wide edges kept in shared memory, eight stones a lane each step.
#[cfg(test)]
const RING_SMEM: &str = "ADD r22, bx, 3\n\
    AND r22, r22, 3\n\
    ADD r23, bx, 1\n\
    AND r23, r23, 3\n\
    MOV r20, 0\n\
    MOV r21, 512\n\
    SHL r24, bx, 8\n\
    DMA 16, r24, 256\n\
    BAR\n\
    LOOP 3\n\
    ADD r9, r20, 16\n\
    SEND r22, r9, 16\n\
    ADD r9, r20, 256\n\
    SEND r23, r9, 16\n\
    RECV r22, r20, 16\n\
    ADD r9, r20, 272\n\
    RECV r23, r9, 16\n\
    LOOP 16\n\
    ADD r6, r20, lane\n\
    ADD r6, r6, 1\n\
    SUB r10, r21, r20\n\
    LOOP 9\n\
    ADD r7, r6, -1\n\
    LDS r1, [r7]\n\
    ADD r7, r6, 0\n\
    LDS r2, [r7]\n\
    ADD r7, r6, 1\n\
    LDS r3, [r7]\n\
    SHL r2, r2, 1\n\
    ADD r1, r1, r2\n\
    ADD r1, r1, r3\n\
    SHR r1, r1, 2\n\
    ADD r8, r6, r10\n\
    ADD r8, r8, 0\n\
    STS [r8], r1\n\
    ADD r6, r6, 32\n\
    ENDLOOP\n\
    XOR r20, r20, 512\n\
    XOR r21, r21, 512\n\
    ENDLOOP\n\
    ENDLOOP\n\
    ADD r6, r20, lane\n\
    ADD r6, r6, 16\n\
    ADD r8, r24, lane\n\
    LOOP 8\n\
    LDS r1, [r6]\n\
    ST [r8], r1\n\
    ADD r6, r6, 32\n\
    ADD r8, r8, 32\n\
    ENDLOOP\n";

/// Shared memory and one stone from each neighbour every step: 48 trips.
#[cfg(test)]
const RING_EVERY: &str = "ADD r22, bx, 3\n\
    AND r22, r22, 3\n\
    ADD r23, bx, 1\n\
    AND r23, r23, 3\n\
    MOV r20, 0\n\
    MOV r21, 512\n\
    SHL r24, bx, 8\n\
    DMA 1, r24, 256\n\
    BAR\n\
    LOOP 48\n\
    ADD r9, r20, 1\n\
    SEND r22, r9, 1\n\
    ADD r9, r20, 256\n\
    SEND r23, r9, 1\n\
    RECV r22, r20, 1\n\
    ADD r9, r20, 257\n\
    RECV r23, r9, 1\n\
    LOOP 1\n\
    ADD r6, r20, lane\n\
    ADD r6, r6, 1\n\
    SUB r10, r21, r20\n\
    LOOP 8\n\
    ADD r7, r6, -1\n\
    LDS r1, [r7]\n\
    ADD r7, r6, 0\n\
    LDS r2, [r7]\n\
    ADD r7, r6, 1\n\
    LDS r3, [r7]\n\
    SHL r2, r2, 1\n\
    ADD r1, r1, r2\n\
    ADD r1, r1, r3\n\
    SHR r1, r1, 2\n\
    ADD r8, r6, r10\n\
    ADD r8, r8, 0\n\
    STS [r8], r1\n\
    ADD r6, r6, 32\n\
    ENDLOOP\n\
    XOR r20, r20, 512\n\
    XOR r21, r21, 512\n\
    ENDLOOP\n\
    ENDLOOP\n\
    ADD r6, r20, lane\n\
    ADD r6, r6, 1\n\
    ADD r8, r24, lane\n\
    LOOP 8\n\
    LDS r1, [r6]\n\
    ST [r8], r1\n\
    ADD r6, r6, 32\n\
    ADD r8, r8, 32\n\
    ENDLOOP\n";

fn stone_ring() -> Open {
    Open {
        task: Task {
            chapter: 15,
            slot: 11,
            title: Tr::new("The Ring of Stones", "石环"),
            brief: Tr::new(
                "Juye's 1024 echo stones stand in a ring, rom[0..1024), and each hums with its neighbours: 48 times over, every stone becomes (2v + left + right) >> 2, stone 0 and stone 1023 being neighbours. Write the last hum to out[i].\nSM s may only read stones 256 s to 256 s + 255 and hears the rest over the links. One warp per SM. Scored by total cycles: fewer is better.",
                "橘爷的 1024 块回响石围成一圈，rom[0..1024)，每块都和邻居一起响：一共 48 次，每块石头变成 (2v + 左 + 右) >> 2，0 号和 1023 号也是邻居。把最后一次的值写进 out[i]。\nSM s 只能读第 256 s 到 256 s + 255 块，其余的只能从链路上听。每个 SM 一个 warp。按总周期计分：越少越好。",
            ),
            cases: || vec![stones(15921, 0), stones(15922, 9), stones(15923, 40)],
            hidden: || vec![stones(15924, 3), stones(15925, 100)],
            expect: rung,
            starter: Source::Code(""),
            reference: Source::Code(RING_REF),
            ..base("stone-ring", 15, 11, Launch::grid(4, 32), 1024, ORIGIN_INT_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[3018, 7483, 9229],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_15_alternatives_pass_and_score_as_marked() {
        super::super::c09::check(
            15,
            &problems(),
            &[
                ("all-hands", HANDS_FIXED),
                ("all-hands", HANDS_INTER),
                ("bucket-brigade", BRIGADE_ONE_DMA),
                ("bucket-brigade", BRIGADE_BOTH),
                ("bucket-brigade", BRIGADE_CHAIN),
                ("stone-ring", RING_SMEM),
                ("stone-ring", RING_EVERY),
            ],
        );
    }
}
