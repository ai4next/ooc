//! Chapter 10 open problems: Yinpo's well on the Atlas (grids, blocks, stencils).

use oo_base::Tr;

use super::Open;
use crate::content::atlas::{ATLAS_OPS, base};
use crate::content::{below, rng};
use crate::task::*;
use crate::vm::Launch;

pub fn problems() -> Vec<Open> {
    vec![life_step(), week_sum(), two_steps()]
}

/// What the output holds past the data before the run.
const GUARD: i32 = -7;
/// Output cells of the map problems: a 64 x 64 map and a guard strip.
const MAP_OUT: usize = 4224;

fn guard_init(len: usize) -> Vec<i32> {
    vec![GUARD; len]
}

/// `f(i)` for the first `n` cells, the untouched guard after them.
fn guarded(n: usize, len: usize, f: impl Fn(usize) -> i32) -> Vec<Option<i32>> {
    (0..len)
        .map(|i| Some(if i < n { f(i) } else { GUARD }))
        .collect()
}

fn case(rom: Vec<i32>, frame: i32) -> Case {
    Case { rom, frame }
}

/// One 16 x 16 block per tile of a W x H map (rom[0] = W, rom[1] = H).
fn map_grid(c: &Case) -> Launch {
    let (w, h) = (c.rom[0] as usize, c.rom[1] as usize);
    Launch::grid2(w.div_ceil(16), h.div_ceil(16), 16, 16)
}

// ---------------------------------------------------------------- life-step

/// rom[0] = W, rom[1] = H, rom[2..] = W x H cells, 1 alive (about one in
/// `one_in`), 0 dead.
fn colony(seed: u64, w: usize, h: usize, one_in: i32, frame: i32) -> Case {
    let mut r = rng(seed);
    let mut rom = vec![w as i32, h as i32];
    rom.extend((0..w * h).map(|_| (below(&mut r, one_in) == 0) as i32));
    case(rom, frame)
}

/// One step of Life with dead cells outside the map.
fn lived(c: &Case) -> Vec<Option<i32>> {
    let (w, h) = (c.rom[0], c.rom[1]);
    let at = |x: i32, y: i32| {
        if (0..w).contains(&x) && (0..h).contains(&y) {
            c.rom[(2 + y * w + x) as usize]
        } else {
            0
        }
    };
    guarded((w * h) as usize, MAP_OUT, |i| {
        let (x, y) = (i as i32 % w, i as i32 / w);
        let n: i32 = (-1..=1)
            .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
            .filter(|&d| d != (0, 0))
            .map(|(dx, dy)| at(x + dx, y + dy))
            .sum();
        (n == 3 || (n == 2 && at(x, y) == 1)) as i32
    })
}

/// Two loops over the 3 x 3 window; a neighbour outside the map is read at
/// the clamped cell and then dropped. With n = window - self, the cell lives
/// when (n OR self) == 3.
const LIFE_REF: &str = "LD r0, [0]\n\
    LD r1, [1]\n\
    SUB r9, r0, 1\n\
    SUB r10, r1, 1\n\
    SHL r2, bx, 4\n\
    ADD r2, r2, tx\n\
    SHL r3, by, 4\n\
    ADD r3, r3, ty\n\
    SETP.LT p0, r2, r0\n\
    SETP.LT p1, r3, r1\n\
    IF p0\n\
    IF p1\n\
    MOV r4, 0\n\
    SUB r5, r3, 1\n\
    LOOP 3\n\
    MAX r7, r5, 0\n\
    MIN r7, r7, r10\n\
    SETP.EQ p2, r7, r5\n\
    MUL r7, r7, r0\n\
    ADD r7, r7, 2\n\
    SUB r6, r2, 1\n\
    LOOP 3\n\
    MAX r8, r6, 0\n\
    MIN r8, r8, r9\n\
    SETP.EQ p3, r8, r6\n\
    ADD r8, r8, r7\n\
    LD r8, [r8]\n\
    SEL r8, p3, r8, 0\n\
    SEL r8, p2, r8, 0\n\
    ADD r4, r4, r8\n\
    ADD r6, r6, 1\n\
    ENDLOOP\n\
    ADD r5, r5, 1\n\
    ENDLOOP\n\
    MUL r11, r3, r0\n\
    ADD r11, r11, r2\n\
    ADD r12, r11, 2\n\
    LD r12, [r12]\n\
    SUB r4, r4, r12\n\
    OR r4, r4, r12\n\
    SETP.EQ p2, r4, 3\n\
    SEL r4, p2, 1, 0\n\
    ST [r11], r4\n\
    ENDIF\n\
    ENDIF\n";

/// The tile and a one-cell ring (18 x 18, zero outside the map) staged in
/// shared memory first; nine LDS per cell instead of nine LD.
#[cfg(test)]
const LIFE_TILE: &str = ".smem 324\n\
    LD r0, [0]\n\
    LD r1, [1]\n\
    MUL r13, ty, 16\n\
    ADD r13, r13, tx\n\
    MOV r14, r13\n\
    LOOP 2\n\
    SETP.LT p0, r14, 324\n\
    IF p0\n\
    MUL r5, r14, 3641\n\
    SHR r5, r5, 16\n\
    MUL r6, r5, 18\n\
    SUB r6, r14, r6\n\
    SHL r7, by, 4\n\
    ADD r7, r7, r5\n\
    SUB r7, r7, 1\n\
    SHL r8, bx, 4\n\
    ADD r8, r8, r6\n\
    SUB r8, r8, 1\n\
    MOV r9, 0\n\
    SETP.GE p1, r7, 0\n\
    SETP.LT p2, r7, r1\n\
    SETP.GE p3, r8, 0\n\
    SETP.LT p0, r8, r0\n\
    SEL r11, p1, 1, 0\n\
    SEL r11, p2, r11, 0\n\
    SEL r11, p3, r11, 0\n\
    SEL r11, p0, r11, 0\n\
    SETP.EQ p1, r11, 1\n\
    IF p1\n\
    MUL r10, r7, r0\n\
    ADD r10, r10, r8\n\
    ADD r10, r10, 2\n\
    LD r9, [r10]\n\
    ENDIF\n\
    STS [r14], r9\n\
    ENDIF\n\
    ADD r14, r14, 256\n\
    ENDLOOP\n\
    BAR\n\
    SHL r2, bx, 4\n\
    ADD r2, r2, tx\n\
    SHL r3, by, 4\n\
    ADD r3, r3, ty\n\
    SETP.LT p0, r2, r0\n\
    SETP.LT p1, r3, r1\n\
    IF p0\n\
    IF p1\n\
    MUL r5, ty, 18\n\
    ADD r5, r5, tx\n\
    MOV r4, 0\n\
    LOOP 3\n\
    LDS r6, [r5]\n\
    ADD r4, r4, r6\n\
    ADD r7, r5, 1\n\
    LDS r6, [r7]\n\
    ADD r4, r4, r6\n\
    ADD r7, r5, 2\n\
    LDS r6, [r7]\n\
    ADD r4, r4, r6\n\
    ADD r5, r5, 18\n\
    ENDLOOP\n\
    MUL r5, ty, 18\n\
    ADD r5, r5, tx\n\
    ADD r5, r5, 19\n\
    LDS r12, [r5]\n\
    SUB r4, r4, r12\n\
    OR r4, r4, r12\n\
    SETP.EQ p2, r4, 3\n\
    SEL r4, p2, 1, 0\n\
    MUL r11, r3, r0\n\
    ADD r11, r11, r2\n\
    ST [r11], r4\n\
    ENDIF\n\
    ENDIF\n";

fn life_step() -> Open {
    Open {
        task: Task {
            chapter: 10,
            slot: 9,
            title: Tr::new("Moss on the Well", "井沿的苔"),
            brief: Tr::new(
                "Yinpo's moss grows by the old rule of Life. rom[0] = W, rom[1] = H, the W x H map (1 = moss, 0 = bare) from rom[2]. One block per 16 x 16 tile. A cell with 3 mossy neighbours of 8 grows moss, a mossy cell with 2 keeps it, all others go bare; outside the map is bare.\nWrite the next map to out[y·W + x]; cells after it keep their -7. Scored by instruction count: fewer is better.",
                "银婆的苔藓按生命游戏的老规矩长。rom[0] = W，rom[1] = H，W x H 的地图（1 = 有苔，0 = 光秃）从 rom[2] 起。每个 16 x 16 的格子一个块。8 个邻居里有 3 个有苔的格子长出苔，有苔且有 2 个的保留，其余变秃；地图外面算光秃。\n把下一张图写到 out[y·W + x]；之后的格子保持 -7。按指令数计分：越少越好。",
            ),
            cases: || {
                vec![
                    colony(10901, 32, 32, 3, 0),
                    colony(10902, 48, 20, 4, 1),
                    colony(10903, 17, 40, 2, 0),
                ]
            },
            hidden: || vec![colony(10904, 64, 64, 3, 1), colony(10905, 23, 9, 3, 2)],
            init: |_| guard_init(MAP_OUT),
            expect: lived,
            grid: Some(map_grid),
            starter: Source::Code(""),
            reference: Source::Code(LIFE_REF),
            ..base("life-step", 9, Launch::grid2(2, 2, 16, 16), MAP_OUT, ATLAS_OPS)
        },
        metric: Metric::Size,
        score: None,
        score_label: None,
        marks: &[45, 74],
    }
}

// ---------------------------------------------------------------- week-sum

/// rom[0] = n, rom[1..=n] = the well's daily levels.
fn levels(seed: u64, n: usize) -> Case {
    let mut r = rng(seed);
    let mut rom = vec![n as i32];
    rom.extend((0..n).map(|_| below(&mut r, 100)));
    case(rom, 0)
}

const WEEK_OUT: usize = 1056;

/// out[i] = levels i-3 ..= i+3, days outside the record counting 0.
fn week_sums(c: &Case) -> Vec<Option<i32>> {
    let n = c.rom[0] as usize;
    guarded(n, WEEK_OUT, |i| {
        (i.saturating_sub(3)..(i + 4).min(n))
            .map(|j| c.rom[1 + j])
            .sum()
    })
}

/// Block-wide: lane 0 of each warp reads n, the block stages its 64 days and
/// three on each side in shared memory, then sums seven LDS per day.
const WEEK_REF: &str = ".smem 70\n\
    SETP.EQ p0, lane, 0\n\
    IF p0\n\
    LD r0, [0]\n\
    ENDIF\n\
    SHFL r0, r0, 0\n\
    SHL r1, bx, 6\n\
    SUB r1, r1, 3\n\
    MOV r2, tx\n\
    LOOP 2\n\
    SETP.LT p1, r2, 70\n\
    IF p1\n\
    ADD r3, r1, r2\n\
    MOV r4, 0\n\
    SETP.GE p2, r3, 0\n\
    SETP.LT p3, r3, r0\n\
    SEL r5, p2, 1, 0\n\
    SEL r5, p3, r5, 0\n\
    SETP.EQ p2, r5, 1\n\
    IF p2\n\
    ADD r3, r3, 1\n\
    LD r4, [r3]\n\
    ENDIF\n\
    STS [r2], r4\n\
    ENDIF\n\
    ADD r2, r2, 64\n\
    ENDLOOP\n\
    BAR\n\
    ADD r6, r1, 3\n\
    ADD r6, r6, tx\n\
    SETP.LT p1, r6, r0\n\
    IF p1\n\
    MOV r7, 0\n\
    MOV r2, tx\n\
    LOOP 7\n\
    LDS r4, [r2]\n\
    ADD r7, r7, r4\n\
    ADD r2, r2, 1\n\
    ENDLOOP\n\
    ST [r6], r7\n\
    ENDIF\n";

/// Warp-wide: each lane reads its own day, lanes 29-31 and 0-2 fetch the
/// three days past each end, SHFL brings the window together.
#[cfg(test)]
const WEEK_SHFL: &str = "LD r0, [0]\n\
    SHL r6, bx, 6\n\
    ADD r6, r6, tx\n\
    MOV r4, 0\n\
    SETP.LT p1, r6, r0\n\
    IF p1\n\
    ADD r3, r6, 1\n\
    LD r4, [r3]\n\
    ENDIF\n\
    SETP.GE p1, lane, 29\n\
    SETP.LT p2, lane, 3\n\
    MOV r8, 0\n\
    MOV r9, 0\n\
    IF p1\n\
    SUB r3, r6, 32\n\
    SETP.GE p3, r3, 0\n\
    IF p3\n\
    ADD r3, r3, 1\n\
    LD r8, [r3]\n\
    ENDIF\n\
    ENDIF\n\
    IF p2\n\
    ADD r3, r6, 32\n\
    SETP.LT p3, r3, r0\n\
    IF p3\n\
    ADD r3, r3, 1\n\
    LD r9, [r3]\n\
    ENDIF\n\
    ENDIF\n\
    MOV r7, 0\n\
    SUB r10, lane, 3\n\
    LOOP 7\n\
    AND r11, r10, 31\n\
    SHFL r12, r4, r11\n\
    SHFL r13, r8, r11\n\
    SHFL r14, r9, r11\n\
    SETP.LT p1, r10, 0\n\
    SETP.GE p2, r10, 32\n\
    SEL r12, p1, r13, r12\n\
    SEL r12, p2, r14, r12\n\
    ADD r7, r7, r12\n\
    ADD r10, r10, 1\n\
    ENDLOOP\n\
    SETP.LT p1, r6, r0\n\
    IF p1\n\
    ST [r6], r7\n\
    ENDIF\n";

/// Seven global reads per day.
#[cfg(test)]
const WEEK_PLAIN: &str = "LD r0, [0]\n\
    SHL r6, bx, 6\n\
    ADD r6, r6, tx\n\
    SETP.LT p1, r6, r0\n\
    IF p1\n\
    MOV r7, 0\n\
    SUB r3, r6, 3\n\
    LOOP 7\n\
    SETP.GE p2, r3, 0\n\
    SETP.LT p3, r3, r0\n\
    SEL r5, p2, 1, 0\n\
    SEL r5, p3, r5, 0\n\
    SETP.EQ p2, r5, 1\n\
    IF p2\n\
    ADD r4, r3, 1\n\
    LD r4, [r4]\n\
    ADD r7, r7, r4\n\
    ENDIF\n\
    ADD r3, r3, 1\n\
    ENDLOOP\n\
    ST [r6], r7\n\
    ENDIF\n";

fn week_sum() -> Open {
    Open {
        task: Task {
            chapter: 10,
            slot: 10,
            title: Tr::new("The Week Around", "前后一周"),
            brief: Tr::new(
                "Yinpo marks the well every day. rom[0] = n, the levels are rom[1..=n]. Write out[i] = the sum of days i-3 to i+3 (days outside the record count 0); cells after n keep their -7. Blocks of 64 threads, as many as needed.\nScored by global reads (LD lanes, worst case): fewer is better.",
                "银婆每天给井水做记号。rom[0] = n，水位在 rom[1..=n]。写 out[i] = 第 i-3 天到第 i+3 天之和（记录之外的日子算 0）；n 之后的格子保持 -7。每块 64 个线程，要多少块有多少块。\n按全局读取计分（LD 的车道数，取最坏的用例）：越少越好。",
            ),
            cases: || vec![levels(11001, 200), levels(11002, 64), levels(11003, 777)],
            hidden: || vec![levels(11004, 1000), levels(11005, 5), levels(11006, 333)],
            init: |_| guard_init(WEEK_OUT),
            expect: week_sums,
            grid: Some(|c| Launch::grid((c.rom[0] as usize).div_ceil(64), 64)),
            starter: Source::Code(""),
            reference: Source::Code(WEEK_REF),
            ..base("week-sum", 10, Launch::grid(4, 64), WEEK_OUT, ATLAS_OPS)
        },
        metric: Metric::Loads,
        score: None,
        score_label: None,
        marks: &[1122, 2210, 8012],
    }
}

// ---------------------------------------------------------------- two-steps

/// rom[0] = W, rom[1] = H, rom[2..] = the field, with a few hot cells.
fn field(seed: u64, w: usize, h: usize) -> Case {
    let mut r = rng(seed);
    let mut rom = vec![w as i32, h as i32];
    rom.extend((0..w * h).map(|i| {
        if i % 53 == 7 {
            900
        } else {
            below(&mut r, 200)
        }
    }));
    case(rom, 0)
}

/// One seep step: the border ring is copied, inside it (4c + n + s + e + w) >> 3.
fn seep(w: usize, h: usize, f: &[i32]) -> Vec<i32> {
    (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
                f[i]
            } else {
                (4 * f[i] + f[i - 1] + f[i + 1] + f[i - w] + f[i + w]) >> 3
            }
        })
        .collect()
}

fn two_seeped(c: &Case) -> Vec<Option<i32>> {
    let (w, h) = (c.rom[0] as usize, c.rom[1] as usize);
    let f = seep(w, h, &seep(w, h, &c.rom[2..2 + w * h]));
    guarded(w * h, MAP_OUT, |i| f[i])
}

/// Temporal blocking: the block stages its tile with a two-cell ring (20 x 20,
/// clamped reads), works out the first step on the inner 18 x 18 in shared
/// memory, then the second step on its own 16 x 16.
const TWO_REF: &str = ".smem 724\n\
    LD r0, [0]\n\
    LD r1, [1]\n\
    SUB r14, r0, 1\n\
    SUB r15, r1, 1\n\
    SHL r12, bx, 4\n\
    SHL r13, by, 4\n\
    MUL r2, ty, 16\n\
    ADD r2, r2, tx\n\
    MOV r3, r2\n\
    LOOP 2\n\
    SETP.LT p0, r3, 400\n\
    IF p0\n\
    MUL r4, r3, 3277\n\
    SHR r4, r4, 16\n\
    MUL r5, r4, 20\n\
    SUB r5, r3, r5\n\
    ADD r4, r4, r13\n\
    SUB r4, r4, 2\n\
    MAX r4, r4, 0\n\
    MIN r4, r4, r15\n\
    ADD r5, r5, r12\n\
    SUB r5, r5, 2\n\
    MAX r5, r5, 0\n\
    MIN r5, r5, r14\n\
    MUL r4, r4, r0\n\
    ADD r4, r4, r5\n\
    ADD r4, r4, 2\n\
    LD r4, [r4]\n\
    STS [r3], r4\n\
    ENDIF\n\
    ADD r3, r3, 256\n\
    ENDLOOP\n\
    BAR\n\
    MOV r3, r2\n\
    LOOP 2\n\
    SETP.LT p0, r3, 324\n\
    IF p0\n\
    MUL r4, r3, 3641\n\
    SHR r4, r4, 16\n\
    MUL r5, r4, 18\n\
    SUB r5, r3, r5\n\
    MUL r6, r4, 20\n\
    ADD r6, r6, r5\n\
    ADD r6, r6, 21\n\
    ADD r4, r4, r13\n\
    SUB r4, r4, 1\n\
    ADD r5, r5, r12\n\
    SUB r5, r5, 1\n\
    SUB r7, r14, r5\n\
    MIN r7, r7, r5\n\
    SUB r8, r15, r4\n\
    MIN r8, r8, r4\n\
    MIN r7, r7, r8\n\
    LDS r9, [r6]\n\
    SETP.GE p1, r7, 1\n\
    IF p1\n\
    SHL r9, r9, 2\n\
    SUB r8, r6, 1\n\
    LDS r10, [r8]\n\
    ADD r9, r9, r10\n\
    ADD r8, r6, 1\n\
    LDS r10, [r8]\n\
    ADD r9, r9, r10\n\
    SUB r8, r6, 20\n\
    LDS r10, [r8]\n\
    ADD r9, r9, r10\n\
    ADD r8, r6, 20\n\
    LDS r10, [r8]\n\
    ADD r9, r9, r10\n\
    SHR r9, r9, 3\n\
    ENDIF\n\
    ADD r8, r3, 400\n\
    STS [r8], r9\n\
    ENDIF\n\
    ADD r3, r3, 256\n\
    ENDLOOP\n\
    BAR\n\
    ADD r4, r13, ty\n\
    ADD r5, r12, tx\n\
    SETP.LE p0, r5, r14\n\
    SETP.LE p1, r4, r15\n\
    IF p0\n\
    IF p1\n\
    SUB r7, r14, r5\n\
    MIN r7, r7, r5\n\
    SUB r8, r15, r4\n\
    MIN r8, r8, r4\n\
    MIN r7, r7, r8\n\
    MUL r6, ty, 18\n\
    ADD r6, r6, tx\n\
    ADD r6, r6, 419\n\
    LDS r9, [r6]\n\
    SETP.GE p2, r7, 1\n\
    IF p2\n\
    SHL r9, r9, 2\n\
    SUB r8, r6, 1\n\
    LDS r10, [r8]\n\
    ADD r9, r9, r10\n\
    ADD r8, r6, 1\n\
    LDS r10, [r8]\n\
    ADD r9, r9, r10\n\
    SUB r8, r6, 18\n\
    LDS r10, [r8]\n\
    ADD r9, r9, r10\n\
    ADD r8, r6, 18\n\
    LDS r10, [r8]\n\
    ADD r9, r9, r10\n\
    SHR r9, r9, 3\n\
    ENDIF\n\
    MUL r4, r4, r0\n\
    ADD r4, r4, r5\n\
    ST [r4], r9\n\
    ENDIF\n\
    ENDIF\n";

/// No sharing: every cell recomputes the first step at itself and its four
/// neighbours from global memory, 25 reads.
#[cfg(test)]
const TWO_PLAIN: &str = "LD r0, [0]\n\
    LD r1, [1]\n\
    SUB r14, r0, 1\n\
    SUB r15, r1, 1\n\
    SHL r2, bx, 4\n\
    ADD r2, r2, tx\n\
    SHL r3, by, 4\n\
    ADD r3, r3, ty\n\
    SETP.LE p0, r2, r14\n\
    SETP.LE p1, r3, r15\n\
    SEL r4, p0, 1, 0\n\
    SEL r4, p1, r4, 0\n\
    SETP.EQ p0, r4, 1\n\
    IF p0\n\
    MOV r4, 0\n\
    SUB r5, r3, 1\n\
    LOOP 3\n\
    SUB r6, r2, 1\n\
    LOOP 3\n\
    SETP.EQ p1, r6, r2\n\
    SEL r7, p1, 1, 0\n\
    SETP.EQ p1, r5, r3\n\
    SEL r8, p1, 1, 0\n\
    ADD r7, r7, r8\n\
    MUL r7, r7, r7\n\
    SETP.NE p1, r7, 0\n\
    IF p1\n\
    MUL r8, r5, r0\n\
    ADD r8, r8, r6\n\
    ADD r8, r8, 2\n\
    LD r9, [r8]\n\
    SUB r10, r8, 1\n\
    LD r10, [r10]\n\
    ADD r11, r8, 1\n\
    LD r11, [r11]\n\
    ADD r10, r10, r11\n\
    SUB r11, r8, r0\n\
    LD r11, [r11]\n\
    ADD r10, r10, r11\n\
    ADD r11, r8, r0\n\
    LD r11, [r11]\n\
    ADD r10, r10, r11\n\
    SHL r11, r9, 2\n\
    ADD r10, r10, r11\n\
    SHR r10, r10, 3\n\
    SUB r11, r14, r6\n\
    MIN r11, r11, r6\n\
    SUB r12, r15, r5\n\
    MIN r12, r12, r5\n\
    MIN r11, r11, r12\n\
    SETP.GE p2, r11, 1\n\
    SEL r9, p2, r10, r9\n\
    MUL r9, r9, r7\n\
    ADD r4, r4, r9\n\
    ENDIF\n\
    ADD r6, r6, 1\n\
    ENDLOOP\n\
    ADD r5, r5, 1\n\
    ENDLOOP\n\
    SHR r4, r4, 3\n\
    MUL r8, r3, r0\n\
    ADD r8, r8, r2\n\
    ADD r9, r8, 2\n\
    LD r9, [r9]\n\
    SUB r11, r14, r2\n\
    MIN r11, r11, r2\n\
    SUB r12, r15, r3\n\
    MIN r12, r12, r3\n\
    MIN r11, r11, r12\n\
    SETP.GE p2, r11, 1\n\
    SEL r4, p2, r4, r9\n\
    ST [r8], r4\n\
    ENDIF\n";

fn two_steps() -> Open {
    Open {
        task: Task {
            chapter: 10,
            slot: 11,
            title: Tr::new("Two Days of Seeping", "渗两天"),
            brief: Tr::new(
                "Yinpo wants the field two seep steps ahead in one launch. rom[0] = W, rom[1] = H, the W x H cells from rom[2]. A step keeps the border ring and gives every inner cell (4c + n + s + e + w) >> 3. Blocks are 16 x 16.\nWrite the field after two steps to out[y·W + x]; cells after it keep their -7. Scored by total cycles: fewer is better.",
                "银婆想在一次启动里算出两步之后的渗流。rom[0] = W，rom[1] = H，W x H 个格子从 rom[2] 起。每一步边框一圈不变，里面每格变成 (4c + 北 + 南 + 东 + 西) >> 3。块是 16 x 16。\n把两步之后的场写到 out[y·W + x]；之后的格子保持 -7。按总周期计分：越少越好。",
            ),
            cases: || vec![field(11101, 32, 32), field(11102, 48, 20), field(11103, 64, 64)],
            hidden: || vec![field(11104, 37, 50), field(11105, 16, 16)],
            init: |_| guard_init(MAP_OUT),
            expect: two_seeped,
            grid: Some(map_grid),
            starter: Source::Code(""),
            reference: Source::Code(TWO_REF),
            ..base("two-steps", 11, Launch::grid2(2, 2, 16, 16), MAP_OUT, ATLAS_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[6056, 16539],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_10_alternatives_pass_and_score_as_marked() {
        super::super::c09::check(
            10,
            &problems(),
            &[
                ("life-step", LIFE_TILE),
                ("week-sum", WEEK_SHFL),
                ("week-sum", WEEK_PLAIN),
                ("two-steps", TWO_PLAIN),
            ],
        );
    }
}
