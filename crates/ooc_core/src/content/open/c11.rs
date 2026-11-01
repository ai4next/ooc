//! Chapter 11 open problems: Yinpo's well records (sort, histogram, argmax).

use oo_base::Tr;

use super::Open;
use crate::content::atlas::{ATLAS_OPS, base};
use crate::content::prism::rom_case;
use crate::content::{below, rng};
use crate::task::*;
use crate::vm::Launch;

pub fn problems() -> Vec<Open> {
    vec![sort_all(), tally(), high_mark()]
}

/// The four blocks of 512 threads every problem here runs on.
const FOUR: Launch = Launch::grid(4, 512);

// ---------------------------------------------------------------- sort-4096

/// 4096 marks in 0..256: `kind` 0 random, 1 a handful of values, 2 falling.
fn marks_case(kind: u8, seed: u64) -> Case {
    let mut r = rng(seed);
    rom_case(
        (0..4096)
            .map(|i| match kind {
                0 => below(&mut r, 256),
                1 => 40 * below(&mut r, 5) + 7,
                _ => 255 - i / 16,
            })
            .collect(),
    )
}

fn sorted(c: &Case) -> Vec<Option<i32>> {
    let mut v = c.rom.clone();
    v.sort();
    v.into_iter().map(Some).collect()
}

/// Every block counts all 4096 marks into a shared histogram, scans the 256
/// counts, then finds the mark at each of its 1024 output places by binary
/// search on the running totals.
const SORT_REF: &str = ".smem 520\n\
    LD r1, [tx]\n\
    ADD r9, tx, 512\n\
    LD r2, [r9]\n\
    ADD r9, tx, 1024\n\
    LD r3, [r9]\n\
    ADD r9, tx, 1536\n\
    LD r4, [r9]\n\
    ADD r9, tx, 2048\n\
    LD r5, [r9]\n\
    ADD r9, tx, 2560\n\
    LD r6, [r9]\n\
    ADD r9, tx, 3072\n\
    LD r7, [r9]\n\
    ADD r9, tx, 3584\n\
    LD r8, [r9]\n\
    ATOMS r9, [r1], 1\n\
    ATOMS r9, [r2], 1\n\
    ATOMS r9, [r3], 1\n\
    ATOMS r9, [r4], 1\n\
    ATOMS r9, [r5], 1\n\
    ATOMS r9, [r6], 1\n\
    ATOMS r9, [r7], 1\n\
    ATOMS r9, [r8], 1\n\
    BAR\n\
    SETP.LT p0, tx, 256\n\
    IF p0\n\
    LDS r1, [tx]\n\
    SUB r2, lane, 1\n\
    SHFL r3, r1, r2\n\
    SETP.GE p1, lane, 1\n\
    SEL r3, p1, r3, 0\n\
    ADD r1, r1, r3\n\
    SUB r2, lane, 2\n\
    SHFL r3, r1, r2\n\
    SETP.GE p1, lane, 2\n\
    SEL r3, p1, r3, 0\n\
    ADD r1, r1, r3\n\
    SUB r2, lane, 4\n\
    SHFL r3, r1, r2\n\
    SETP.GE p1, lane, 4\n\
    SEL r3, p1, r3, 0\n\
    ADD r1, r1, r3\n\
    SUB r2, lane, 8\n\
    SHFL r3, r1, r2\n\
    SETP.GE p1, lane, 8\n\
    SEL r3, p1, r3, 0\n\
    ADD r1, r1, r3\n\
    SUB r2, lane, 16\n\
    SHFL r3, r1, r2\n\
    SETP.GE p1, lane, 16\n\
    SEL r3, p1, r3, 0\n\
    ADD r1, r1, r3\n\
    SHR r4, tx, 5\n\
    SETP.EQ p1, lane, 31\n\
    IF p1\n\
    ADD r5, r4, 512\n\
    STS [r5], r1\n\
    ENDIF\n\
    ENDIF\n\
    BAR\n\
    IF p0\n\
    MOV r5, 0\n\
    MOV r6, 512\n\
    LOOP 8\n\
    LDS r7, [r6]\n\
    SUB r8, r6, 512\n\
    SETP.LT p1, r8, r4\n\
    SEL r7, p1, r7, 0\n\
    ADD r5, r5, r7\n\
    ADD r6, r6, 1\n\
    ENDLOOP\n\
    ADD r1, r1, r5\n\
    ADD r2, tx, 256\n\
    STS [r2], r1\n\
    ENDIF\n\
    BAR\n\
    SHL r10, bx, 10\n\
    ADD r10, r10, tx\n\
    ADD r11, r10, 512\n\
    MOV r12, 0\n\
    MOV r13, 0\n\
    MOV r14, 128\n\
    LOOP 8\n\
    ADD r1, r12, r14\n\
    ADD r1, r1, 255\n\
    LDS r2, [r1]\n\
    ADD r3, r13, r14\n\
    ADD r3, r3, 255\n\
    LDS r4, [r3]\n\
    SETP.LE p1, r2, r10\n\
    SEL r5, p1, r14, 0\n\
    ADD r12, r12, r5\n\
    SETP.LE p1, r4, r11\n\
    SEL r5, p1, r14, 0\n\
    ADD r13, r13, r5\n\
    SHR r14, r14, 1\n\
    ENDLOOP\n\
    ST [r10], r12\n\
    ST [r11], r13\n";

/// The same counting sort with block 0 alone writing all 4096 places.
#[cfg(test)]
const SORT_ONE: &str = ".smem 520\n\
    LD r1, [tx]\n\
    ADD r9, tx, 512\n\
    LD r2, [r9]\n\
    ADD r9, tx, 1024\n\
    LD r3, [r9]\n\
    ADD r9, tx, 1536\n\
    LD r4, [r9]\n\
    ADD r9, tx, 2048\n\
    LD r5, [r9]\n\
    ADD r9, tx, 2560\n\
    LD r6, [r9]\n\
    ADD r9, tx, 3072\n\
    LD r7, [r9]\n\
    ADD r9, tx, 3584\n\
    LD r8, [r9]\n\
    ATOMS r9, [r1], 1\n\
    ATOMS r9, [r2], 1\n\
    ATOMS r9, [r3], 1\n\
    ATOMS r9, [r4], 1\n\
    ATOMS r9, [r5], 1\n\
    ATOMS r9, [r6], 1\n\
    ATOMS r9, [r7], 1\n\
    ATOMS r9, [r8], 1\n\
    BAR\n\
    SETP.LT p0, tx, 256\n\
    IF p0\n\
    LDS r1, [tx]\n\
    SUB r2, lane, 1\n\
    SHFL r3, r1, r2\n\
    SETP.GE p1, lane, 1\n\
    SEL r3, p1, r3, 0\n\
    ADD r1, r1, r3\n\
    SUB r2, lane, 2\n\
    SHFL r3, r1, r2\n\
    SETP.GE p1, lane, 2\n\
    SEL r3, p1, r3, 0\n\
    ADD r1, r1, r3\n\
    SUB r2, lane, 4\n\
    SHFL r3, r1, r2\n\
    SETP.GE p1, lane, 4\n\
    SEL r3, p1, r3, 0\n\
    ADD r1, r1, r3\n\
    SUB r2, lane, 8\n\
    SHFL r3, r1, r2\n\
    SETP.GE p1, lane, 8\n\
    SEL r3, p1, r3, 0\n\
    ADD r1, r1, r3\n\
    SUB r2, lane, 16\n\
    SHFL r3, r1, r2\n\
    SETP.GE p1, lane, 16\n\
    SEL r3, p1, r3, 0\n\
    ADD r1, r1, r3\n\
    SHR r4, tx, 5\n\
    SETP.EQ p1, lane, 31\n\
    IF p1\n\
    ADD r5, r4, 512\n\
    STS [r5], r1\n\
    ENDIF\n\
    ENDIF\n\
    BAR\n\
    IF p0\n\
    MOV r5, 0\n\
    MOV r6, 512\n\
    LOOP 8\n\
    LDS r7, [r6]\n\
    SUB r8, r6, 512\n\
    SETP.LT p1, r8, r4\n\
    SEL r7, p1, r7, 0\n\
    ADD r5, r5, r7\n\
    ADD r6, r6, 1\n\
    ENDLOOP\n\
    ADD r1, r1, r5\n\
    ADD r2, tx, 256\n\
    STS [r2], r1\n\
    ENDIF\n\
    BAR\n\
    SETP.EQ p0, bx, 0\n\
    IF p0\n\
    MOV r10, tx\n\
    LOOP 8\n\
    MOV r12, 0\n\
    MOV r14, 128\n\
    LOOP 8\n\
    ADD r1, r12, r14\n\
    ADD r1, r1, 255\n\
    LDS r2, [r1]\n\
    SETP.LE p1, r2, r10\n\
    SEL r5, p1, r14, 0\n\
    ADD r12, r12, r5\n\
    SHR r14, r14, 1\n\
    ENDLOOP\n\
    ST [r10], r12\n\
    ADD r10, r10, 512\n\
    ENDLOOP\n\
    ENDIF\n";

fn sort_all() -> Open {
    Open {
        task: Task {
            chapter: 11,
            slot: 9,
            title: Tr::new("Sort the Record", "排好记录"),
            brief: Tr::new(
                "Yinpo's book holds 4096 well marks, rom[0..4096), each 0..255. Write them to out[0..4096) in rising order. Four blocks of 512 threads.\nScored by total cycles: fewer is better.",
                "银婆的本子上有 4096 个井水记号，rom[0..4096)，每个 0..255。把它们从小到大写进 out[0..4096)。四个块，每块 512 个线程。\n按总周期计分：越少越好。",
            ),
            cases: || vec![marks_case(0, 11901), marks_case(1, 11902), marks_case(2, 11903)],
            hidden: || vec![marks_case(0, 11904), marks_case(1, 11905)],
            expect: sorted,
            starter: Source::Code(""),
            reference: Source::Code(SORT_REF),
            ..base("sort-4096", 9, FOUR, 4096, ATLAS_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[5190, 12294],
    }
}

// ---------------------------------------------------------------- tally

/// rom[0] = n, rom[1..=n] = bucket numbers 0..63: `kind` 0 even, 1 most in
/// bucket 3, 2 all in one bucket.
fn tally_case(kind: u8, seed: u64, n: usize) -> Case {
    let mut r = rng(seed);
    let mut rom = vec![n as i32];
    rom.extend((0..n).map(|_| match kind {
        0 => below(&mut r, 64),
        1 => {
            if below(&mut r, 10) < 7 {
                3
            } else {
                below(&mut r, 64)
            }
        }
        _ => 41,
    }));
    rom_case(rom)
}

fn tallied(c: &Case) -> Vec<Option<i32>> {
    let mut t = vec![0; 64];
    for &b in &c.rom[1..] {
        t[b as usize] += 1;
    }
    t.into_iter().map(Some).collect()
}

/// Each block counts its share into 64 shared counters (ATOMS), then 64
/// threads add the block's counters to out with one ATOM each. Two sticks per
/// thread: tid and tid + 2048.
const TALLY_REF: &str = ".smem 64\n\
    LD r0, [0]\n\
    ADD r1, tid, 1\n\
    LD r2, [r1]\n\
    ADD r1, tid, 2049\n\
    LD r3, [r1]\n\
    SETP.LT p0, tid, r0\n\
    IF p0\n\
    ATOMS r4, [r2], 1\n\
    ENDIF\n\
    ADD r1, tid, 2048\n\
    SETP.LT p0, r1, r0\n\
    IF p0\n\
    ATOMS r4, [r3], 1\n\
    ENDIF\n\
    BAR\n\
    SETP.LT p0, tx, 64\n\
    IF p0\n\
    LDS r3, [tx]\n\
    ATOM r4, [tx], r3\n\
    ENDIF\n";

/// Every bucket number goes straight to the global counter.
#[cfg(test)]
const TALLY_GLOBAL: &str = "LD r0, [0]\n\
    ADD r1, tid, 1\n\
    LD r2, [r1]\n\
    ADD r1, tid, 2049\n\
    LD r3, [r1]\n\
    SETP.LT p0, tid, r0\n\
    IF p0\n\
    ATOM r4, [r2], 1\n\
    ENDIF\n\
    ADD r1, tid, 2048\n\
    SETP.LT p0, r1, r0\n\
    IF p0\n\
    ATOM r4, [r3], 1\n\
    ENDIF\n";

fn tally() -> Open {
    Open {
        task: Task {
            chapter: 11,
            slot: 10,
            title: Tr::new("Tally Sticks", "记数签"),
            brief: Tr::new(
                "Each visitor to the well drops a stick in one of 64 buckets. rom[0] = n (up to 4096), rom[1..=n] are the bucket numbers 0..63. Write how many sticks each bucket got to out[0..64). Four blocks of 512 threads.\nScored by total cycles: fewer is better.",
                "来井边的人每人往 64 个桶里的一个投一根签。rom[0] = n（最多 4096），rom[1..=n] 是桶号 0..63。把每个桶收到几根签写进 out[0..64)。四个块，每块 512 个线程。\n按总周期计分：越少越好。",
            ),
            cases: || {
                vec![
                    tally_case(0, 11911, 4096),
                    tally_case(1, 11912, 3000),
                    tally_case(2, 11913, 1000),
                ]
            },
            hidden: || vec![tally_case(1, 11914, 4096), tally_case(0, 11915, 77)],
            expect: tallied,
            starter: Source::Code(""),
            reference: Source::Code(TALLY_REF),
            ..base("tally", 10, FOUR, 64, ATLAS_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[1156, 1890],
    }
}

// ---------------------------------------------------------------- high-mark

/// 4096 readings 0..9999 with the top value planted twice.
fn readings(seed: u64) -> Case {
    let mut r = rng(seed);
    let mut rom: Vec<i32> = (0..4096).map(|_| below(&mut r, 9000)).collect();
    let (a, b) = (below(&mut r, 4096) as usize, below(&mut r, 4096) as usize);
    let top = 9000 + below(&mut r, 1000);
    rom[a] = top;
    rom[b] = top;
    rom_case(rom)
}

/// out[0] = the first index of the largest reading; out[1..16) is free.
fn highest(c: &Case) -> Vec<Option<i32>> {
    let top = *c.rom.iter().max().unwrap();
    let at = c.rom.iter().position(|&v| v == top).unwrap() as i32;
    let mut out = vec![None; 16];
    out[0] = Some(at);
    out
}

/// Each reading becomes the key (value << 12) | (4095 - index), so the
/// largest key is the answer; warps reduce with SHFL and MAX, blocks through
/// shared memory, and the four block keys meet in out after a GSYNC.
const HIGH_REF: &str = ".smem 16\n\
    LD r0, [tid]\n\
    ADD r1, tid, 2048\n\
    LD r1, [r1]\n\
    SHL r0, r0, 12\n\
    SUB r2, 4095, tid\n\
    OR r0, r0, r2\n\
    SHL r1, r1, 12\n\
    SUB r2, 2047, tid\n\
    OR r1, r1, r2\n\
    MAX r0, r0, r1\n\
    XOR r2, lane, 16\n\
    SHFL r3, r0, r2\n\
    MAX r0, r0, r3\n\
    XOR r2, lane, 8\n\
    SHFL r3, r0, r2\n\
    MAX r0, r0, r3\n\
    XOR r2, lane, 4\n\
    SHFL r3, r0, r2\n\
    MAX r0, r0, r3\n\
    XOR r2, lane, 2\n\
    SHFL r3, r0, r2\n\
    MAX r0, r0, r3\n\
    XOR r2, lane, 1\n\
    SHFL r3, r0, r2\n\
    MAX r0, r0, r3\n\
    SHR r4, tx, 5\n\
    SETP.EQ p0, lane, 0\n\
    IF p0\n\
    STS [r4], r0\n\
    ENDIF\n\
    BAR\n\
    SETP.LT p0, tx, 32\n\
    IF p0\n\
    SETP.LT p1, tx, 16\n\
    MOV r0, 0\n\
    IF p1\n\
    LDS r0, [tx]\n\
    ENDIF\n\
    XOR r2, lane, 8\n\
    SHFL r3, r0, r2\n\
    MAX r0, r0, r3\n\
    XOR r2, lane, 4\n\
    SHFL r3, r0, r2\n\
    MAX r0, r0, r3\n\
    XOR r2, lane, 2\n\
    SHFL r3, r0, r2\n\
    MAX r0, r0, r3\n\
    XOR r2, lane, 1\n\
    SHFL r3, r0, r2\n\
    MAX r0, r0, r3\n\
    SETP.EQ p1, tx, 0\n\
    IF p1\n\
    ADD r5, bx, 1\n\
    ATOM r6, [r5], r0\n\
    ENDIF\n\
    ENDIF\n\
    GSYNC\n\
    SETP.EQ p0, tid, 0\n\
    IF p0\n\
    ATOM r0, [1], 0\n\
    ATOM r1, [2], 0\n\
    ATOM r2, [3], 0\n\
    ATOM r3, [4], 0\n\
    MAX r0, r0, r1\n\
    MAX r2, r2, r3\n\
    MAX r0, r0, r2\n\
    AND r0, r0, 4095\n\
    SUB r0, 4095, r0\n\
    ST [0], r0\n\
    ENDIF\n";

/// Block 0 alone walks all 4096 readings, eight per thread.
#[cfg(test)]
const HIGH_ONE: &str = ".smem 16\n\
    SETP.EQ p0, bx, 0\n\
    IF p0\n\
    MOV r0, 0\n\
    MOV r5, tx\n\
    SUB r6, 4095, tx\n\
    LOOP 8\n\
    LD r1, [r5]\n\
    SHL r1, r1, 12\n\
    OR r1, r1, r6\n\
    MAX r0, r0, r1\n\
    ADD r5, r5, 512\n\
    SUB r6, r6, 512\n\
    ENDLOOP\n\
    XOR r2, lane, 16\n\
    SHFL r3, r0, r2\n\
    MAX r0, r0, r3\n\
    XOR r2, lane, 8\n\
    SHFL r3, r0, r2\n\
    MAX r0, r0, r3\n\
    XOR r2, lane, 4\n\
    SHFL r3, r0, r2\n\
    MAX r0, r0, r3\n\
    XOR r2, lane, 2\n\
    SHFL r3, r0, r2\n\
    MAX r0, r0, r3\n\
    XOR r2, lane, 1\n\
    SHFL r3, r0, r2\n\
    MAX r0, r0, r3\n\
    SHR r4, tx, 5\n\
    SETP.EQ p1, lane, 0\n\
    IF p1\n\
    STS [r4], r0\n\
    ENDIF\n\
    BAR\n\
    SETP.LT p1, tx, 32\n\
    IF p1\n\
    SETP.LT p2, tx, 16\n\
    MOV r0, 0\n\
    IF p2\n\
    LDS r0, [tx]\n\
    ENDIF\n\
    XOR r2, lane, 8\n\
    SHFL r3, r0, r2\n\
    MAX r0, r0, r3\n\
    XOR r2, lane, 4\n\
    SHFL r3, r0, r2\n\
    MAX r0, r0, r3\n\
    XOR r2, lane, 2\n\
    SHFL r3, r0, r2\n\
    MAX r0, r0, r3\n\
    XOR r2, lane, 1\n\
    SHFL r3, r0, r2\n\
    MAX r0, r0, r3\n\
    AND r0, r0, 4095\n\
    SUB r0, 4095, r0\n\
    SETP.EQ p2, tx, 0\n\
    IF p2\n\
    ST [0], r0\n\
    ENDIF\n\
    ENDIF\n\
    ENDIF\n";

fn high_mark() -> Open {
    Open {
        task: Task {
            chapter: 11,
            slot: 11,
            title: Tr::new("High Water Mark", "最高水位"),
            brief: Tr::new(
                "rom[0..4096) are the well's readings, 0..9999. Write to out[0] the index of the highest one (the first, if it appears more than once); out[1..16) is yours to use. Four blocks of 512 threads.\nScored by total cycles: fewer is better.",
                "rom[0..4096) 是井的水位读数，0..9999。把最高那个的下标写进 out[0]（出现不止一次时取第一个）；out[1..16) 随你用。四个块，每块 512 个线程。\n按总周期计分：越少越好。",
            ),
            cases: || vec![readings(11921), readings(11922), readings(11923)],
            hidden: || vec![readings(11924), readings(11925)],
            expect: highest,
            starter: Source::Code(""),
            reference: Source::Code(HIGH_REF),
            ..base("high-mark", 11, FOUR, 16, ATLAS_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[1005, 3769],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_11_alternatives_pass_and_score_as_marked() {
        super::super::c09::check(
            11,
            &problems(),
            &[
                ("sort-4096", SORT_ONE),
                ("tally", TALLY_GLOBAL),
                ("high-mark", HIGH_ONE),
            ],
        );
    }
}
