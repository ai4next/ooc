//! Chapter 7 open problems: Bohe's clinic on Vault, where memory is 400
//! cycles away behind a 16-line L1.

use oo_base::Tr;

use super::Open;
use crate::content::prism::rom_case;
use crate::content::vault::{ALL_WARPS, CACHE_OPS, base};
use crate::content::{below, rng};
use crate::task::*;
use crate::vm::Launch;

fn cells(f: impl Fn(usize) -> i32, n: usize) -> Vec<Option<i32>> {
    (0..n).map(|i| Some(f(i))).collect()
}

fn values(seed: u64, n: usize, lo: i32, hi: i32) -> Vec<i32> {
    let mut r = rng(seed);
    (0..n).map(|_| lo + below(&mut r, hi - lo)).collect()
}

pub fn problems() -> Vec<Open> {
    vec![referrals(), fever_map(), running_total()]
}

/// Four referrals on from patient i.
fn referred(rom: &[i32], i: usize) -> i32 {
    (0..4).fold(i, |p, _| rom[p] as usize) as i32
}

/// Every patient refers on to the next, the last one back to patient 0.
fn ring() -> Case {
    rom_case((0..512).map(|i| (i + 1) % 512).collect())
}

fn referrals() -> Open {
    Open {
        task: Task {
            title: Tr::new("The Referrals", "转诊"),
            brief: Tr::new(
                "Each of Bohe's 512 patients is referred on to another: patient i to rom[i], 0 to 511. Write out[i] = the patient reached after four referrals from i.\nAll 16 warps run unless your first line is `.warps N`. The referral sheet is 16 lines long, exactly the size of the L1.\nScore: cycles.",
                "薄荷的 512 位病人每人都会被转诊给另一位：病人 i 转给 rom[i]（0 到 511）。把从 i 开始转诊四次后到达的病人写到 out[i]。\n默认 16 个 warp 都运行，除非第一行写 `.warps N`。转诊单正好 16 行，和 L1 一样大。\n得分：周期数。",
            ),
            cases: || {
                [711, 712, 713]
                    .map(|s| rom_case(values(s, 512, 0, 512)))
                    .to_vec()
            },
            hidden: || vec![ring(), rom_case(values(714, 512, 0, 8))],
            expect: |c| cells(|i| referred(&c.rom, i), 512),
            free_warps: true,
            reference: REFER_ALL,
            chapter: 7,
            ..base("referrals", 9, ALL_WARPS, 512, CACHE_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[509, 2122, 6642],
    }
}

/// One patient per thread, 16 warps: the first hop is coalesced and fills the
/// whole L1 between the warps, so the other hops all hit.
const REFER_ALL: Source =
    Source::Code("LD r0, [tid]\nLD r0, [r0]\nLD r0, [r0]\nLD r0, [r0]\nST [tid], r0\n");

/// Four warps, four patients per thread.
#[cfg(test)]
const REFER_FOUR: Source = Source::Code(concat!(
    ".warps 4\nMOV r1, tid\n",
    "LOOP 4\n  LD r0, [r1]\n  LD r0, [r0]\n  LD r0, [r0]\n  LD r0, [r0]\n  ST [r1], r0\n  ADD r1, r1, 128\nENDLOOP\n",
));

/// One warp walking all 512 patients.
#[cfg(test)]
const REFER_ONE: Source = Source::Code(concat!(
    ".warps 1\nMOV r1, lane\n",
    "LOOP 16\n  LD r0, [r1]\n  LD r0, [r0]\n  LD r0, [r0]\n  LD r0, [r0]\n  ST [r1], r0\n  ADD r1, r1, 32\nENDLOOP\n",
));

/// The 3×3 sum around (x, y) over the cells inside the 32×32 grid.
fn fever(rom: &[i32], i: usize) -> i32 {
    let (x, y) = ((i % 32) as i32, (i / 32) as i32);
    let mut s = 0;
    for v in y - 1..=y + 1 {
        for u in x - 1..=x + 1 {
            if (0..32).contains(&u) && (0..32).contains(&v) {
                s += rom[(32 * v + u) as usize];
            }
        }
    }
    s
}

/// A hot spot at each corner and in the middle, for the borders.
fn corners() -> Case {
    let mut rom = vec![36; 1024];
    for i in [0, 31, 992, 1023, 528] {
        rom[i] = 41;
    }
    rom_case(rom)
}

fn fever_map() -> Open {
    Open {
        task: Task {
            title: Tr::new("The Fever Map", "体温图"),
            brief: Tr::new(
                "The ward's temperature map is a 32×32 grid, cell (x, y) at rom[32y + x]. Write out[32y + x] = the sum of the cell and its eight neighbours, skipping those outside the grid.\nOne warp runs. Reads outside rom give 0.\nScore: cycles.",
                "病房的体温图是 32×32 的网格，格子 (x, y) 在 rom[32y + x]。把这个格子和它八个邻居之和写到 out[32y + x]，网格外的邻居不算。\n只运行一个 warp。读 rom 范围外的地址得到 0。\n得分：周期数。",
            ),
            cases: || {
                [721, 722]
                    .map(|s| rom_case(values(s, 1024, 35, 42)))
                    .to_vec()
            },
            hidden: || vec![corners(), rom_case(values(723, 1024, -99, 100))],
            expect: |c| cells(|i| fever(&c.rom, i), 1024),
            reference: FEVER_AHEAD,
            chapter: 7,
            ..base("fever-map", 10, Launch::flat(32), 1024, CACHE_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[1350, 13234, 13432],
    }
}

/// Setup for the column walk: r10, r11 are the lanes to the left and right,
/// p1, p2 say whether they exist; row 0 is summed across into r4.
macro_rules! fever_setup {
    () => {
        concat!(
            "SUB r10, lane, 1\nMAX r10, r10, 0\nADD r11, lane, 1\nMIN r11, r11, 31\nSETP.GT p1, lane, 0\nSETP.LT p2, lane, 31\n",
            "MOV r3, 0\nLD r5, [lane]\nSHFL r6, r5, r10\nSHFL r7, r5, r11\nSEL r6, p1, r6, 0\nSEL r7, p2, r7, 0\n",
            "ADD r4, r5, r6\nADD r4, r4, r7\nADD r1, lane, 32\nMOV r2, lane\n",
        )
    };
}

/// One step down: sum the next row across, add the rows above and below, store.
macro_rules! fever_row {
    () => {
        concat!(
            "  LD r5, [r1]\n  ADD r1, r1, 32\n  SHFL r6, r5, r10\n  SHFL r7, r5, r11\n  SEL r6, p1, r6, 0\n  SEL r7, p2, r7, 0\n",
            "  ADD r8, r5, r6\n  ADD r8, r8, r7\n  ADD r9, r3, r4\n  ADD r9, r9, r8\n  ST [r2], r9\n  ADD r2, r2, 32\n  MOV r3, r4\n  MOV r4, r8\n",
        )
    };
}

/// Lane x walks down column x. Each row is read once, as one line; its
/// neighbours across come by SHFL and the rows above and below stay in
/// registers. One LD with a line per lane asks for sixteen rows at once, so
/// each half of the grid waits on memory only once.
const FEVER_AHEAD: Source = Source::Code(concat!(
    "AND r12, lane, 15\nSHL r12, r12, 5\nLD r13, [r12]\n",
    fever_setup!(),
    "LOOP 15\n",
    fever_row!(),
    "ENDLOOP\n",
    "ADD r12, r12, 512\nLD r13, [r12]\n",
    "LOOP 17\n",
    fever_row!(),
    "ENDLOOP\n",
));

/// The same walk down the columns, one miss at a time.
#[cfg(test)]
const FEVER_ROLL: Source = Source::Code(concat!(
    fever_setup!(),
    "LOOP 32\n",
    fever_row!(),
    "ENDLOOP\n"
));

/// Nine loads per cell; the left and right columns are zeroed at the borders.
#[cfg(test)]
const FEVER_NINE: Source = Source::Code(concat!(
    "SETP.GT p1, lane, 0\nSETP.LT p2, lane, 31\nMOV r2, lane\n",
    "LOOP 32\n  SUB r5, r2, 33\n  LD r5, [r5]\n  SUB r6, r2, 1\n  LD r6, [r6]\n  ADD r7, r2, 31\n  LD r7, [r7]\n",
    "  ADD r5, r5, r6\n  ADD r5, r5, r7\n  SEL r5, p1, r5, 0\n",
    "  SUB r6, r2, 32\n  LD r6, [r6]\n  LD r7, [r2]\n  ADD r8, r2, 32\n  LD r8, [r8]\n  ADD r6, r6, r7\n  ADD r6, r6, r8\n",
    "  SUB r7, r2, 31\n  LD r7, [r7]\n  ADD r8, r2, 1\n  LD r8, [r8]\n  ADD r9, r2, 33\n  LD r9, [r9]\n",
    "  ADD r7, r7, r8\n  ADD r7, r7, r9\n  SEL r7, p2, r7, 0\n",
    "  ADD r5, r5, r6\n  ADD r5, r5, r7\n  ST [r2], r5\n  ADD r2, r2, 32\nENDLOOP\n",
));

fn prefix(rom: &[i32]) -> Vec<Option<i32>> {
    rom.iter()
        .scan(0, |s, &v| {
            *s += v;
            Some(Some(*s))
        })
        .collect()
}

fn running_total() -> Open {
    Open {
        task: Task {
            title: Tr::new("The Running Total", "累计总数"),
            brief: Tr::new(
                "Bohe's day book lists 1024 payments in rom[0..1024]. Write out[i] = rom[0] + rom[1] + … + rom[i], the total so far after each one.\nOne warp runs.\nScore: cycles.",
                "薄荷的流水账在 rom[0..1024] 里记了 1024 笔款项。把 out[i] = rom[0] + rom[1] + … + rom[i]，即每一笔之后的累计总数写出来。\n只运行一个 warp。\n得分：周期数。",
            ),
            cases: || {
                [731, 732]
                    .map(|s| rom_case(values(s, 1024, 0, 100)))
                    .to_vec()
            },
            hidden: || vec![rom_case(values(733, 1024, -500, 500))],
            expect: |c| prefix(&c.rom),
            reference: TOTAL_SCAN,
            chapter: 7,
            ..base("running-total", 11, Launch::flat(32), 1024, CACHE_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[1563, 13463, 20899],
    }
}

/// Setup for the scans below: r4..r8 mask in the lane d below (all ones when
/// lane ≥ d, for d = 1, 2, 4, 8, 16), r9..r13 are those source lanes.
macro_rules! scan_setup {
    () => {
        concat!(
        "SETP.GE p0, lane, 1\nSEL r4, p0, -1, 0\nSETP.GE p0, lane, 2\nSEL r5, p0, -1, 0\nSETP.GE p0, lane, 4\nSEL r6, p0, -1, 0\n",
        "SETP.GE p0, lane, 8\nSEL r7, p0, -1, 0\nSETP.GE p0, lane, 16\nSEL r8, p0, -1, 0\n",
        "SUB r9, lane, 1\nMAX r9, r9, 0\nSUB r10, lane, 2\nMAX r10, r10, 0\nSUB r11, lane, 4\nMAX r11, r11, 0\n",
        "SUB r12, lane, 8\nMAX r12, r12, 0\nSUB r13, lane, 16\nMAX r13, r13, 0\nMOV r2, 0\nMOV r3, lane\n",
        )
    };
}

/// One row: read it, scan it across the lanes, add the carry, store, and pass
/// lane 31's total on as the next carry.
macro_rules! scan_row {
    () => {
        concat!(
            "  LD r0, [r3]\n  SHFL r1, r0, r9\n  AND r1, r1, r4\n  ADD r0, r0, r1\n  SHFL r1, r0, r10\n  AND r1, r1, r5\n  ADD r0, r0, r1\n",
            "  SHFL r1, r0, r11\n  AND r1, r1, r6\n  ADD r0, r0, r1\n  SHFL r1, r0, r12\n  AND r1, r1, r7\n  ADD r0, r0, r1\n",
            "  SHFL r1, r0, r13\n  AND r1, r1, r8\n  ADD r0, r0, r1\n  ADD r0, r0, r2\n  ST [r3], r0\n  SHFL r2, r0, 31\n  ADD r3, r3, 32\n",
        )
    };
}

/// A scan across the lanes for each row of 32. One LD with a line per lane
/// asks for sixteen rows at once, so each half waits on memory only once.
const TOTAL_SCAN: Source = Source::Code(concat!(
    "AND r14, lane, 15\nSHL r14, r14, 5\nLD r15, [r14]\n",
    scan_setup!(),
    "LOOP 16\n",
    scan_row!(),
    "ENDLOOP\n",
    "ADD r14, r14, 512\nLD r15, [r14]\n",
    "LOOP 16\n",
    scan_row!(),
    "ENDLOOP\n",
));

/// The same scan, one miss at a time.
#[cfg(test)]
const TOTAL_PLAIN: Source = Source::Code(concat!(
    scan_setup!(),
    "LOOP 32\n",
    scan_row!(),
    "ENDLOOP\n"
));

/// Every lane walks the book in order, one payment at a time.
#[cfg(test)]
const TOTAL_WALK: Source = Source::Code(concat!(
    "MOV r1, 0\nMOV r2, 0\n",
    "LOOP 16\n  LOOP 64\n    LD r0, [r1]\n    ADD r2, r2, r0\n    ST [r1], r2\n    ADD r1, r1, 1\n  ENDLOOP\nENDLOOP\n",
));

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grade::grade;

    #[test]
    fn alternatives_pass_and_score_on_marks() {
        let alts: [&[Source]; 3] = [
            &[REFER_FOUR, REFER_ONE],
            &[FEVER_ROLL, FEVER_NINE],
            &[TOTAL_PLAIN, TOTAL_WALK],
        ];
        for (o, alts) in problems().iter().zip(alts) {
            for a in alts {
                let rep = grade(&o.task, &a.solution()).unwrap();
                let s = o.score_of(&rep);
                assert!(
                    s.is_some_and(|s| o.marks.contains(&s)),
                    "{}: {s:?}",
                    o.task.id
                );
            }
        }
    }
}
