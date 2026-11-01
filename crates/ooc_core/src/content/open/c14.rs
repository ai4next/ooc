//! Chapter 14 open problems: Cenli's relay stone on Lyra (the sorter in
//! INT8, a variance without cancellation, a sigmoid from half-precision parts).

use oo_base::Tr;

use super::Open;
use crate::content::lyra::{LYRA_OPS, base};
use crate::content::{below, rng};
use crate::fp::{self, f_to_f32, f32_bits, h_to_f64};
use crate::task::*;
use crate::vm::Launch;

pub fn problems() -> Vec<Open> {
    vec![four_sorters(), steady_stone(), the_gate()]
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn case(rom: Vec<i32>) -> Case {
    Case { rom, frame: 0 }
}

// ---------------------------------------------------------------- four-sorters

/// Pictures per batch, batches per case.
const BATCH: usize = 64;
const BATCHES: usize = 4;
/// The weights after the 256 pictures, in the order of the chapter's sorter:
/// W2 as FP16 tiles, W1 as INT8 words, the W1 scales.
const WTS: usize = BATCH * 64 * BATCHES;
/// Where the weights sit in the chapter sorter's input, and how many words.
const LVL_WTS: usize = 4096;
const WTS_LEN: usize = 928;

fn sorter() -> &'static Task {
    crate::content::task("the-sorter").expect("the sorter level")
}

/// The batches of the chapter sorter's cases and showcase, by number.
fn batch(k: usize) -> Case {
    let t = sorter();
    let (public, hidden) = ((t.cases)(), (t.hidden)());
    if k < 3 {
        public[k].clone()
    } else if k < 6 {
        hidden[k - 3].clone()
    } else {
        (t.live.unwrap())((k - 6) as i32, [(k % 5) as i32, 0])
    }
}

/// Four batches side by side, then the weights (FP16 W1 left out).
fn sorter_case(ks: [usize; BATCHES]) -> Case {
    let bs: Vec<Case> = ks.iter().map(|&k| batch(k)).collect();
    let mut rom: Vec<i32> = bs.iter().flat_map(|b| b.rom[..BATCH * 64].to_vec()).collect();
    rom.extend_from_slice(&bs[0].rom[LVL_WTS..LVL_WTS + WTS_LEN]);
    case(rom)
}

/// The exact network's class for every picture, batch by batch.
fn sorter_expect(c: &Case) -> Vec<Option<i32>> {
    let t = sorter();
    (0..BATCHES)
        .flat_map(|b| {
            let mut rom = c.rom[b * BATCH * 64..(b + 1) * BATCH * 64].to_vec();
            rom.resize(7072, 0);
            (t.expect)(&case(rom))
        })
        .collect()
}

/// The chapter's packed sorter for one batch per block. `warps` of the
/// block's four share its eight pairs of picture tiles, each with its own
/// 256 words of hidden tiles; warp 0 starts both copies.
fn sorter_src(warps: u32) -> &'static str {
    let mut s = String::from(".smem 6048\nSHR r30, tx, 5\nSETP.EQ p0, r30, 0\nIF p0\n\
        SHL r31, bx, 12\nDMA 0, r31, 4096\n");
    s += &format!("DMA 4096, {WTS}, {WTS_LEN}\nENDIF\n");
    s += "AND r1, lane, 15\nSHR r2, lane, 2\nAND r2, r2, 3\nSHR r3, lane, 4\nAND r4, lane, 3\n\
          SHL r5, r3, 8\nSHL r11, r2, 6\nADD r5, r5, r11\nADD r5, r5, r4\n\
          ADD r7, r1, 4096\nSHL r9, r3, 2\nADD r9, r9, r2\n\
          SHL r31, r30, 8\nADD r31, r31, lane\nADD r31, r31, 5024\n\
          ADD r6, r2, 4480\nSHL r23, r4, 3\nMOV r22, 24\nSUB r23, r22, r23\n";
    // warp w starts at pair w · 8 / warps: 512 pixel words and 8 outputs a pair
    let shift = (8 / warps).trailing_zeros();
    s += &format!(
        "SHL r8, r30, {}\nSHL r10, bx, 6\nSHL r22, r30, {}\nADD r10, r10, r22\nBAR\n",
        9 + shift,
        3 + shift
    );
    if warps < 4 {
        s += &format!("SETP.LT p0, r30, {warps}\nIF p0\n");
    }
    s += &format!("LOOP {}\nMOV r11, r6\nMOV r12, r31\nADD r21, r4, 4992\n", 8 / warps);
    s += "LOOP 8\nADD r13, r8, r5\nMOV r14, 0\nLOOP 16\nLDS r15, [r13]\nCVT.H.I r15, r15\n\
          HMUL r15, r15, 7168\nLDS r16, [r11]\nSHL r16, r16, r23\nSHR r16, r16, 24\nCVT.H.I r16, r16\n\
          MMA r14, r15, r16, r14\nADD r13, r13, 4\nADD r11, r11, 4\nENDLOOP\n\
          LDS r22, [r21]\nADD r21, r21, 4\nFMUL r14, r14, r22\n\
          FMAX r14, r14, 0\nCVT.H.F r14, r14\nSTS [r12], r14\nADD r12, r12, 32\nENDLOOP\n\
          MOV r12, r31\nMOV r13, r7\nMOV r17, 0\nMOV r18, 0\nMOV r19, 0\n\
          LOOP 8\nLDS r15, [r12]\nADD r12, r12, 32\n";
    for acc in [17, 18, 19] {
        s += &format!("LDS r16, [r13]\nADD r13, r13, 16\nMMA r{acc}, r15, r16, r{acc}\n");
    }
    s += "ENDLOOP\nFMAX r24, r17, r18\nFMAX r24, r24, r19\n";
    for m in [1, 2] {
        s += &format!("XOR r25, lane, {m}\nSHFL r26, r24, r25\nFMAX r24, r24, r26\n");
    }
    s += "MOV r27, 99\nADD r28, r4, 8\nSETP.EQ p1, r19, r24\nSEL r27, p1, r28, r27\n\
          ADD r28, r4, 4\nSETP.EQ p1, r18, r24\nSEL r27, p1, r28, r27\n\
          SETP.EQ p1, r17, r24\nSEL r27, p1, r4, r27\n";
    for m in [1, 2] {
        s += &format!("XOR r25, lane, {m}\nSHFL r26, r27, r25\nMIN r27, r27, r26\n");
    }
    s += "SETP.EQ p2, r4, 0\nADD r20, r10, r9\nIF p2\nST [r20], r27\nENDIF\n\
          ADD r8, r8, 512\nADD r10, r10, 8\nENDLOOP\n";
    if warps < 4 {
        s += "ENDIF\n";
    }
    leak(s)
}

fn four_sorters() -> Open {
    Open {
        task: Task {
            chapter: 14,
            slot: 9,
            title: Tr::new("Four Sorters", "四台分拣器"),
            brief: Tr::new(
                "The post office sends Cenli four batches of 64 glyph pictures at once: picture i is rom[64i .. 64i + 64], its class goes to out[i]. Run the chapter's sorter in INT8 with under 1% accuracy loss: a batch has 64 pictures, so every class must match the exact network.\nThe weights follow at rom[16384]: W2 as FP16 tiles (384 words), W1 as INT8 words (512) and the 32 FP32 scales, laid out as in the sorter; there is no FP16 W1. Four blocks of 128 threads.\nScored by total cycles: fewer is better.",
                "邮局一次给岑理送来四批字符图，每批 64 张：第 i 张是 rom[64i .. 64i + 64]，类号写到 out[i]。用 INT8 跑本章的分拣器，准确率损失低于 1%：一批 64 张，所以每一张都要和精确网络一致。\n权重从 rom[16384] 起：W2 的 FP16 块（384 个字）、W1 的 INT8 字（512 个）和 32 个 FP32 缩放，排法和分拣器一样；没有 FP16 的 W1。四个块，每块 128 个线程。\n按总周期计分：越少越好。",
            ),
            cases: || vec![sorter_case([0, 1, 6, 7]), sorter_case([2, 8, 9, 10])],
            hidden: || vec![sorter_case([3, 4, 5, 11]), sorter_case([12, 13, 5, 3])],
            expect: sorter_expect,
            starter: Source::Code(""),
            reference: Source::Code(sorter_src(4)),
            ..base("four-sorters", 14, 9, Launch::grid(BATCHES, 128), BATCH * BATCHES, LYRA_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[16364, 17722, 23489],
    }
}

// ---------------------------------------------------------------- steady-stone

/// 1024 FP16 readings around `mean`, in steps of 0.5 up to `spread` either side.
fn steady_case(seed: u64, mean: f64, spread: i32) -> Case {
    let mut r = rng(seed);
    case(
        (0..1024)
            .map(|_| fp::h(mean + 0.5 * (below(&mut r, 4 * spread + 1) - 2 * spread) as f64))
            .collect(),
    )
}

/// out[0] = round(100 · the variance of the readings).
fn steady_expect(c: &Case) -> Vec<Option<i32>> {
    let x: Vec<f64> = c.rom.iter().map(|&b| h_to_f64(b)).collect();
    let m = x.iter().sum::<f64>() / 1024.0;
    let v = x.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / 1024.0;
    vec![Some((100.0 * v).round() as i32)]
}

/// FP32 2^-10 and 100.
const INV_N: &str = "0x3A800000";
const HUNDRED: &str = "0x42C80000";

/// Sums each register over the block of 256 and leaves the totals in every
/// thread: an FADD butterfly per warp, the eight warp sums through shared
/// memory from word `slot` on, a second butterfly.
fn block_sum(regs: &[u8], slot: u32) -> String {
    let fly = |s: &mut String, masks: &[u32]| {
        for m in masks {
            *s += &format!("XOR r8, lane, {m}\n");
            for r in regs {
                *s += &format!("SHFL r9, r{r}, r8\nFADD r{r}, r{r}, r9\n");
            }
        }
    };
    let mut s = String::new();
    fly(&mut s, &[16, 8, 4, 2, 1]);
    s += &format!("SHR r10, tid, 5\nADD r10, r10, {slot}\nSETP.EQ p0, lane, 0\nIF p0\n");
    for (i, r) in regs.iter().enumerate() {
        s += &format!("STS [r10], r{r}\n");
        if i + 1 < regs.len() {
            s += "ADD r10, r10, 8\n";
        }
    }
    s += "ENDIF\nBAR\n";
    for r in regs {
        s += &format!("MOV r{r}, 0\n");
    }
    s += &format!("SETP.LT p0, lane, 8\nIF p0\nADD r10, lane, {slot}\n");
    for (i, r) in regs.iter().enumerate() {
        s += &format!("LDS r{r}, [r10]\n");
        if i + 1 < regs.len() {
            s += "ADD r10, r10, 8\n";
        }
    }
    s += "ENDIF\n";
    fly(&mut s, &[4, 2, 1]);
    s
}

/// Sum of r1..r4 into `into` and of their squares into `sq` (r1..r4 are squared).
fn sums4(into: u8, sq: Option<u8>) -> String {
    let mut s = format!("FADD r{into}, r1, r2\nFADD r7, r3, r4\nFADD r{into}, r{into}, r7\n");
    if let Some(q) = sq {
        s += "FMUL r1, r1, r1\nFMUL r2, r2, r2\nFMUL r3, r3, r3\nFMUL r4, r4, r4\n";
        s += &format!("FADD r{q}, r1, r2\nFADD r7, r3, r4\nFADD r{q}, r{q}, r7\n");
    }
    s
}

/// Variance of the four readings each thread holds in r1..r4, by `kind`:
/// 0 shifts by the first reading and sums deviations and their squares in
/// one pass; 1 finds the mean first, then sums squared deviations from it;
/// 2 sums the raw squares (cancellation eats the answer).
fn steady_src(kind: u8) -> &'static str {
    let mut s = String::from(".smem 24\nLD r1, [tid]\n");
    for (k, r) in [(1, 2), (2, 3), (3, 4)] {
        s += &format!("ADD r5, tid, {}\nLD r{r}, [r5]\n", 256 * k);
    }
    if kind == 0 {
        s += "LD r0, [0]\nCVT.F.H r0, r0\n";
    }
    for r in 1..=4 {
        s += &format!("CVT.F.H r{r}, r{r}\n");
        if kind == 0 {
            s += &format!("FSUB r{r}, r{r}, r0\n");
        }
    }
    if kind == 1 {
        s += &sums4(5, None);
        s += &block_sum(&[5], 0);
        // only lanes 0..7 hold the total after the second butterfly
        s += &format!("SHFL r5, r5, 0\nFMUL r5, r5, {INV_N}\n");
        for r in 1..=4 {
            s += &format!("FSUB r{r}, r{r}, r5\n");
        }
        s += &sums4(5, Some(6));
        s += &block_sum(&[6], 16);
        s += &format!(
            "SETP.EQ p0, tid, 0\nIF p0\nFMUL r6, r6, {INV_N}\nFMUL r6, r6, {HUNDRED}\nCVT.I.F r6, r6\nST [0], r6\nENDIF\n"
        );
    } else {
        s += &sums4(5, Some(6));
        s += &block_sum(&[5, 6], 0);
        s += &format!(
            "SETP.EQ p0, tid, 0\nIF p0\nFMUL r5, r5, {INV_N}\nFMUL r6, r6, {INV_N}\nFMUL r7, r5, r5\n\
             FSUB r6, r6, r7\nFMUL r6, r6, {HUNDRED}\nCVT.I.F r6, r6\nST [0], r6\nENDIF\n"
        );
    }
    leak(s)
}

fn steady_stone() -> Open {
    Open {
        task: Task {
            chapter: 14,
            slot: 10,
            title: Tr::new("A Steady Stone", "稳当的石头"),
            brief: Tr::new(
                "Cenli checks how steady the relay stone's signal is. rom[0..1024) are 1024 FP16 readings, all close to one large value. Write out[0] = round(100 · variance), the variance being the mean of (reading - mean)², within 1.\nOne block of 256 threads. Scored by total cycles: fewer is better.",
                "岑理要看中继石的信号稳不稳。rom[0..1024) 是 1024 个 FP16 读数，都挨着同一个很大的值。写 out[0] = round(100 · 方差)，方差是 (读数 - 均值)² 的平均，误差不超过 1。\n一个块，256 个线程。按总周期计分：越少越好。",
            ),
            cases: || vec![steady_case(14101, 900.0, 6), steady_case(14102, 700.0, 3)],
            hidden: || vec![steady_case(14103, 1000.0, 1), steady_case(14104, 600.0, 10)],
            expect: steady_expect,
            tolerance: 1,
            starter: Source::Code(""),
            reference: Source::Code(steady_src(0)),
            ..base("steady-stone", 14, 10, Launch::grid(1, 256), 1, LYRA_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[1048, 1215],
    }
}

// ---------------------------------------------------------------- the-gate

/// 256 FP32 inputs in thousandths, -8 ..= 8, with the ends always present.
fn gate_case(seed: u64, reach: i32) -> Case {
    let mut r = rng(seed);
    case(
        (0..256)
            .map(|i| {
                let v = match i {
                    0 => -reach,
                    1 => reach,
                    2 => 0,
                    _ => below(&mut r, 2 * reach + 1) - reach,
                };
                f32_bits(v as f32 / 1000.0)
            })
            .collect(),
    )
}

/// out[i] = round(10000 · sigmoid(x[i])).
fn gated(c: &Case) -> Vec<Option<i32>> {
    c.rom
        .iter()
        .map(|&b| {
            let x = f_to_f32(b) as f64;
            Some((10000.0 / (1.0 + (-x).exp())).round() as i32)
        })
        .collect()
}

/// FP16 1 and 10000; FP32 1, 2 and 10000 (FP32 zero is the word 0).
const H_ONE: &str = "0x3C00";
const H_TEN_K: &str = "0x70E2";
const F_ONE: &str = "0x3F800000";
const F_TWO: &str = "0x40000000";
const F_TEN_K: &str = "0x461C4000";

/// The gate for x in r1, result in r1, by `kind`: 0 everything in halves;
/// 1 the same with the last scaling in FP32; 2 the small side e^-|x| /
/// (1 + e^-|x|) in halves, mirrored for x > 0 so the half never has to hold
/// a value near 1; 3 that plus FP32 repairs: the exponent's rounding is put
/// back as e · (1 + δ) and the half reciprocal gets one Newton step.
fn gate_body(kind: u8) -> String {
    let mut s = String::new();
    if kind < 2 {
        s += &format!("FSUB r1, 0, r1\nCVT.H.F r1, r1\nHEXP r1, r1\nHADD r1, r1, {H_ONE}\nHRCP r1, r1\n");
        s += &if kind == 0 {
            format!("HMUL r1, r1, {H_TEN_K}\nCVT.I.H r1, r1\n")
        } else {
            format!("CVT.F.H r1, r1\nFMUL r1, r1, {F_TEN_K}\nCVT.I.F r1, r1\n")
        };
        return s;
    }
    // r2 = -|x|, p0: x > 0
    s += "SETP.GT p0, r1, 0\nFSUB r2, 0, r1\nFMAX r2, r1, r2\nFSUB r2, 0, r2\nCVT.H.F r3, r2\n";
    if kind == 2 {
        s += &format!("HEXP r4, r3\nHADD r5, r4, {H_ONE}\nHRCP r5, r5\nHMUL r4, r4, r5\nCVT.F.H r4, r4\n");
    } else {
        s += &format!(
            "CVT.F.H r6, r3\nFSUB r6, r2, r6\nFADD r6, r6, {F_ONE}\nHEXP r4, r3\nCVT.F.H r4, r4\nFMUL r4, r4, r6\n\
             FADD r5, r4, {F_ONE}\nCVT.H.F r6, r5\nHRCP r6, r6\nCVT.F.H r6, r6\n\
             FMUL r7, r5, r6\nFSUB r7, {F_TWO}, r7\nFMUL r6, r6, r7\nFMUL r4, r4, r6\n"
        );
    }
    s += &format!("FMUL r4, r4, {F_TEN_K}\nFSUB r5, {F_TEN_K}, r4\nSEL r1, p0, r5, r4\nCVT.I.F r1, r1\n");
    s
}

/// One warp, eight inputs per lane, loads batched ahead of the work.
fn gate_src(kind: u8) -> &'static str {
    let mut s = String::from("MOV r10, lane\nLOOP 8\nLD r1, [r10]\n");
    s += &gate_body(kind);
    s += "ST [r10], r1\nADD r10, r10, 32\nENDLOOP\n";
    leak(s)
}

fn the_gate() -> Open {
    Open {
        task: Task {
            chapter: 14,
            slot: 11,
            title: Tr::new("The Gate", "闸门"),
            brief: Tr::new(
                "Cenli's stone opens a gate by how sure it is: rom[0..256) are FP32 scores x from -8 to 8, and out[i] = round(10000 / (1 + e^-x)). HEXP and HRCP only work in halves; answers within 20 pass.\nOne warp. Scored by the largest error over all cells: smaller is better.",
                "岑理的石头按有多确定来开闸：rom[0..256) 是 -8 到 8 之间的 FP32 分数 x，out[i] = round(10000 / (1 + e^-x))。HEXP 和 HRCP 只能算半精度；误差 20 以内算通过。\n一个 warp。按所有格子里最大的误差计分：越小越好。",
            ),
            cases: || vec![gate_case(14201, 8000), gate_case(14202, 3000)],
            hidden: || vec![gate_case(14203, 8000), gate_case(14204, 1500)],
            expect: gated,
            tolerance: 20,
            starter: Source::Code(""),
            reference: Source::Code(gate_src(3)),
            ..base("the-gate", 14, 11, Launch::ONE_WARP, 256, LYRA_OPS)
        },
        metric: Metric::Err,
        score: None,
        score_label: None,
        marks: &[1, 5, 7, 10],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_14_alternatives_pass_and_score_as_marked() {
        super::super::c09::check(
            14,
            &problems(),
            &[
                ("four-sorters", sorter_src(1)),
                ("four-sorters", sorter_src(2)),
                ("steady-stone", steady_src(1)),
                ("the-gate", gate_src(0)),
                ("the-gate", gate_src(1)),
                ("the-gate", gate_src(2)),
            ],
        );
    }

    #[test]
    fn raw_squares_lose_the_variance_to_cancellation() {
        let o = steady_stone();
        let rep = crate::grade::grade(&o.task, &Source::Code(steady_src(2)).solution()).unwrap();
        assert!(!rep.passed(), "{:?}", rep.verdict);
    }
}
