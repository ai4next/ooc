//! Chapter 13 open problems: Cenli's tensor cores on Lyra (matrix products).

use oo_base::Tr;

use super::Open;
use crate::content::lyra::{LYRA_OPS, base};
use crate::content::prism::rom_case;
use crate::content::{below, rng};
use crate::task::*;
use crate::vm::Launch;

pub fn problems() -> Vec<Open> {
    vec![many_fours(), chain(), sixteen()]
}

/// `n` small integers in -9..=9.
fn small(seed: u64, n: usize) -> Vec<i32> {
    let mut r = rng(seed);
    (0..n).map(|_| below(&mut r, 19) - 9).collect()
}

/// `C = A·B` for row-major `n x n` integer matrices.
fn product(a: &[i32], b: &[i32], n: usize) -> Vec<i32> {
    (0..n * n)
        .map(|o| (0..n).map(|k| a[o / n * n + k] * b[k * n + o % n]).sum())
        .collect()
}

// ---------------------------------------------------------------- many-fours

/// 256 pairs of 4 x 4 matrices: pair p is A at rom[32p], B at rom[32p + 16].
fn pairs(seed: u64) -> Case {
    rom_case(small(seed, 8192))
}

fn fours(c: &Case) -> Vec<Option<i32>> {
    c.rom
        .chunks(32)
        .flat_map(|p| product(&p[..16], &p[16..], 4))
        .map(Some)
        .collect()
}

/// One MMA per half warp: each half loads its pair's A and B straight into
/// the fragment layout.
const FOUR_MMA: &str = "SHR r0, tid, 4\n\
    SHL r0, r0, 5\n\
    AND r1, lane, 15\n\
    ADD r0, r0, r1\n\
    LD r2, [r0]\n\
    ADD r0, r0, 16\n\
    LD r3, [r0]\n\
    CVT.H.I r2, r2\n\
    CVT.H.I r3, r3\n\
    MMA r4, r2, r3, r5\n\
    CVT.I.F r4, r4\n\
    ST [tid], r4\n";

/// One thread per output cell, four multiply-adds, all eight reads issued
/// before the first multiply.
#[cfg(test)]
const FOUR_UNROLLED: &str = "SHR r0, tid, 4\n\
    SHL r0, r0, 5\n\
    SHR r1, tid, 2\n\
    AND r1, r1, 3\n\
    SHL r1, r1, 2\n\
    ADD r1, r1, r0\n\
    AND r2, tid, 3\n\
    ADD r2, r2, r0\n\
    ADD r2, r2, 16\n\
    LD r3, [r1]\n\
    LD r4, [r2]\n\
    ADD r1, r1, 1\n\
    ADD r2, r2, 4\n\
    LD r5, [r1]\n\
    LD r6, [r2]\n\
    ADD r1, r1, 1\n\
    ADD r2, r2, 4\n\
    LD r7, [r1]\n\
    LD r8, [r2]\n\
    ADD r1, r1, 1\n\
    ADD r2, r2, 4\n\
    LD r9, [r1]\n\
    LD r10, [r2]\n\
    MUL r3, r3, r4\n\
    MUL r5, r5, r6\n\
    MUL r7, r7, r8\n\
    MUL r9, r9, r10\n\
    ADD r3, r3, r5\n\
    ADD r7, r7, r9\n\
    ADD r3, r3, r7\n\
    ST [tid], r3\n";

/// The same in a loop: each read waits for the one before it.
#[cfg(test)]
const FOUR_LOOP: &str = "SHR r0, tid, 4\n\
    SHL r0, r0, 5\n\
    SHR r1, tid, 2\n\
    AND r1, r1, 3\n\
    SHL r1, r1, 2\n\
    ADD r1, r1, r0\n\
    AND r2, tid, 3\n\
    ADD r2, r2, r0\n\
    ADD r2, r2, 16\n\
    MOV r3, 0\n\
    LOOP 4\n\
    LD r4, [r1]\n\
    LD r5, [r2]\n\
    MUL r4, r4, r5\n\
    ADD r3, r3, r4\n\
    ADD r1, r1, 1\n\
    ADD r2, r2, 4\n\
    ENDLOOP\n\
    ST [tid], r3\n";

fn many_fours() -> Open {
    Open {
        task: Task {
            chapter: 13,
            slot: 9,
            title: Tr::new("A Stack of Fours", "一摞四乘四"),
            brief: Tr::new(
                "Cenli has 256 small products to check. Pair p is a 4 x 4 matrix A at rom[32p] and a 4 x 4 matrix B at rom[32p + 16], both row-major integers -9..9. Write A·B row-major to out[16p..16p + 16). 16 blocks of 256 threads.\nScored by total cycles: fewer is better.",
                "岑理有 256 个小乘法要核对。第 p 对是位于 rom[32p] 的 4 x 4 矩阵 A 和位于 rom[32p + 16] 的 4 x 4 矩阵 B，都按行主序存放，是 -9..9 的整数。把 A·B 按行主序写进 out[16p..16p + 16)。16 个块，每块 256 个线程。\n按总周期计分：越少越好。",
            ),
            cases: || vec![pairs(13901), pairs(13902)],
            hidden: || vec![pairs(13903)],
            expect: fours,
            starter: Source::Code(""),
            reference: Source::Code(FOUR_MMA),
            ..base("many-fours", 13, 9, Launch::grid(16, 256), 4096, LYRA_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[684, 1520, 2069],
    }
}

// ---------------------------------------------------------------- chain

/// Three 16 x 16 integer matrices -3..3: A at rom[0], B at rom[256], C at rom[512].
fn triple(seed: u64) -> Case {
    let mut r = rng(seed);
    rom_case((0..768).map(|_| below(&mut r, 7) - 3).collect())
}

fn chained(c: &Case) -> Vec<Option<i32>> {
    let ab = product(&c.rom[..256], &c.rom[256..512], 16);
    product(&ab, &c.rom[512..], 16).into_iter().map(Some).collect()
}

/// Each half warp owns one 4 x 4 tile: four MMAs (two accumulators) for its
/// tile of A·B, which goes to shared memory; after a BAR four more MMAs for
/// its tile of (A·B)·C.
const CHAIN_REF: &str = ".smem 256\n\
    AND r0, lane, 15\n\
    SHR r5, tid, 4\n\
    SHR r1, r5, 2\n\
    SHL r1, r1, 6\n\
    SHR r6, r0, 2\n\
    SHL r7, r6, 4\n\
    ADD r1, r1, r7\n\
    AND r2, r5, 3\n\
    SHL r2, r2, 2\n\
    AND r8, r0, 3\n\
    ADD r2, r2, r8\n\
    ADD r3, r1, r8\n\
    ADD r4, r7, r2\n\
    ADD r9, r4, 256\n\
    ADD r11, r3, 0\n\
    LD r12, [r11]\n\
    ADD r11, r9, 0\n\
    LD r16, [r11]\n\
    ADD r11, r3, 4\n\
    LD r13, [r11]\n\
    ADD r11, r9, 64\n\
    LD r17, [r11]\n\
    ADD r11, r3, 8\n\
    LD r14, [r11]\n\
    ADD r11, r9, 128\n\
    LD r18, [r11]\n\
    ADD r11, r3, 12\n\
    LD r15, [r11]\n\
    ADD r11, r9, 192\n\
    LD r19, [r11]\n\
    CVT.H.I r12, r12\n\
    CVT.H.I r16, r16\n\
    CVT.H.I r13, r13\n\
    CVT.H.I r17, r17\n\
    CVT.H.I r14, r14\n\
    CVT.H.I r18, r18\n\
    CVT.H.I r15, r15\n\
    CVT.H.I r19, r19\n\
    MMA r20, r12, r16, r24\n\
    MMA r21, r13, r17, r24\n\
    MMA r20, r14, r18, r20\n\
    MMA r21, r15, r19, r21\n\
    FADD r20, r20, r21\n\
    CVT.I.F r20, r20\n\
    ADD r11, r1, r2\n\
    STS [r11], r20\n\
    BAR\n\
    ADD r9, r4, 512\n\
    ADD r11, r3, 0\n\
    LDS r12, [r11]\n\
    ADD r11, r9, 0\n\
    LD r16, [r11]\n\
    ADD r11, r3, 4\n\
    LDS r13, [r11]\n\
    ADD r11, r9, 64\n\
    LD r17, [r11]\n\
    ADD r11, r3, 8\n\
    LDS r14, [r11]\n\
    ADD r11, r9, 128\n\
    LD r18, [r11]\n\
    ADD r11, r3, 12\n\
    LDS r15, [r11]\n\
    ADD r11, r9, 192\n\
    LD r19, [r11]\n\
    CVT.H.I r12, r12\n\
    CVT.H.I r16, r16\n\
    CVT.H.I r13, r13\n\
    CVT.H.I r17, r17\n\
    CVT.H.I r14, r14\n\
    CVT.H.I r18, r18\n\
    CVT.H.I r15, r15\n\
    CVT.H.I r19, r19\n\
    MMA r20, r12, r16, r24\n\
    MMA r21, r13, r17, r24\n\
    MMA r20, r14, r18, r20\n\
    MMA r21, r15, r19, r21\n\
    FADD r20, r20, r21\n\
    CVT.I.F r20, r20\n\
    ADD r11, r1, r2\n\
    ST [r11], r20\n";

/// A thread per cell: sixteen multiply-adds for A·B into shared memory, then
/// sixteen more for the product with C.
#[cfg(test)]
const CHAIN_SCALAR: &str = ".smem 256\n\
    SHR r0, tid, 4\n\
    SHL r0, r0, 4\n\
    AND r1, tid, 15\n\
    MOV r2, r0\n\
    ADD r3, r1, 256\n\
    MOV r4, 0\n\
    LOOP 16\n\
    LD r5, [r2]\n\
    LD r6, [r3]\n\
    MUL r5, r5, r6\n\
    ADD r4, r4, r5\n\
    ADD r2, r2, 1\n\
    ADD r3, r3, 16\n\
    ENDLOOP\n\
    STS [tid], r4\n\
    BAR\n\
    MOV r2, r0\n\
    ADD r3, r1, 512\n\
    MOV r4, 0\n\
    LOOP 16\n\
    LDS r5, [r2]\n\
    LD r6, [r3]\n\
    MUL r5, r5, r6\n\
    ADD r4, r4, r5\n\
    ADD r2, r2, 1\n\
    ADD r3, r3, 16\n\
    ENDLOOP\n\
    ST [tid], r4\n";

fn chain() -> Open {
    Open {
        task: Task {
            chapter: 13,
            slot: 10,
            title: Tr::new("Three in a Row", "连乘三个"),
            brief: Tr::new(
                "Cenli wants A·B·C for three 16 x 16 integer matrices (-3..3, row-major): A at rom[0], B at rom[256], C at rom[512]. Write the product row-major to out[0..256). One block of 256 threads.\nScored by total cycles: fewer is better.",
                "岑理要三个 16 x 16 整数矩阵（-3..3，行主序）的连乘 A·B·C：A 在 rom[0]，B 在 rom[256]，C 在 rom[512]。把乘积按行主序写进 out[0..256)。一个 256 线程的块。\n按总周期计分：越少越好。",
            ),
            cases: || vec![triple(13911), triple(13912)],
            hidden: || vec![triple(13913)],
            expect: chained,
            starter: Source::Code(""),
            reference: Source::Code(CHAIN_REF),
            ..base("chain", 13, 10, Launch::grid(1, 256), 256, LYRA_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[1235, 8039],
    }
}

// ---------------------------------------------------------------- sixteen

/// A 64 x 64 matrix at rom[0] and sixteen 64-long columns X (64 x 16,
/// row-major) at rom[4096], integers -3..3.
fn tall(seed: u64) -> Case {
    let mut r = rng(seed);
    rom_case((0..5120).map(|_| below(&mut r, 7) - 3).collect())
}

/// Y = A·X, 64 x 16 row-major.
fn tall_product(c: &Case) -> Vec<Option<i32>> {
    (0..1024)
        .map(|o| {
            let (i, j) = (o / 16, o % 16);
            Some((0..64).map(|k| c.rom[64 * i + k] * c.rom[4096 + 16 * k + j]).sum())
        })
        .collect()
}

/// A half warp per 4 x 4 tile of Y: sixteen K steps of MMA, two
/// accumulators so neighbouring MMAs do not wait on each other.
const TALL_REF: &str = "AND r0, lane, 15\n\
    SHR r5, tid, 4\n\
    SHR r1, r5, 2\n\
    SHL r1, r1, 2\n\
    SHR r6, r0, 2\n\
    ADD r1, r1, r6\n\
    AND r7, r0, 3\n\
    SHL r9, r1, 6\n\
    ADD r9, r9, r7\n\
    AND r2, r5, 3\n\
    SHL r2, r2, 2\n\
    ADD r2, r2, r7\n\
    SHL r10, r6, 4\n\
    ADD r10, r10, r2\n\
    ADD r10, r10, 4096\n\
    LOOP 8\n\
    LD r12, [r9]\n\
    ADD r11, r9, 4\n\
    LD r13, [r11]\n\
    LD r16, [r10]\n\
    ADD r11, r10, 64\n\
    LD r17, [r11]\n\
    CVT.H.I r12, r12\n\
    CVT.H.I r13, r13\n\
    CVT.H.I r16, r16\n\
    CVT.H.I r17, r17\n\
    MMA r20, r12, r16, r20\n\
    MMA r21, r13, r17, r21\n\
    ADD r9, r9, 8\n\
    ADD r10, r10, 128\n\
    ENDLOOP\n\
    FADD r20, r20, r21\n\
    CVT.I.F r20, r20\n\
    SHL r1, r1, 4\n\
    ADD r1, r1, r2\n\
    ST [r1], r20\n";

/// A thread per cell of Y: sixty-four multiply-adds, one gemv per column.
#[cfg(test)]
const TALL_SCALAR: &str = "SHR r0, tid, 4\n\
    SHL r0, r0, 6\n\
    AND r1, tid, 15\n\
    ADD r1, r1, 4096\n\
    MOV r4, 0\n\
    LOOP 64\n\
    LD r5, [r0]\n\
    LD r6, [r1]\n\
    MUL r5, r5, r6\n\
    ADD r4, r4, r5\n\
    ADD r0, r0, 1\n\
    ADD r1, r1, 16\n\
    ENDLOOP\n\
    ST [tid], r4\n";

fn sixteen() -> Open {
    Open {
        task: Task {
            chapter: 13,
            slot: 11,
            title: Tr::new("Sixteen at Once", "十六个一起"),
            brief: Tr::new(
                "Cenli has one 64 x 64 matrix A (rom[0], row-major) and sixteen vectors, the columns of X (64 x 16, row-major, rom[4096]); all integers -3..3. Write Y = A·X row-major to out[0..1024). 4 blocks of 256 threads.\nScored by total cycles: fewer is better.",
                "岑理有一个 64 x 64 的矩阵 A（rom[0]，行主序）和十六个向量，即 X 的各列（64 x 16，行主序，rom[4096]）；都是 -3..3 的整数。把 Y = A·X 按行主序写进 out[0..1024)。4 个块，每块 256 个线程。\n按总周期计分：越少越好。",
            ),
            cases: || vec![tall(13921), tall(13922)],
            hidden: || vec![tall(13923)],
            expect: tall_product,
            starter: Source::Code(""),
            reference: Source::Code(TALL_REF),
            ..base("sixteen-at-once", 13, 11, Launch::grid(4, 256), 1024, LYRA_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[3606, 26367],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_13_alternatives_pass_and_score_as_marked() {
        super::super::c09::check(
            13,
            &problems(),
            &[
                ("many-fours", FOUR_UNROLLED),
                ("many-fours", FOUR_LOOP),
                ("chain", CHAIN_SCALAR),
                ("sixteen-at-once", TALL_SCALAR),
            ],
        );
    }
}
