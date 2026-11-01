//! Chapter 16 open problems: ORI speaks to the town square (a faster first
//! sentence, attention split across the SMs, a link that heals itself).

use oo_base::Tr;

use super::Open;
use crate::content::origin::{ORIGIN_INT_OPS, ORIGIN_OPS, base};
use crate::content::{below, rng};
use crate::fp::{self, h_to_f64};
use crate::grade::Report;
use crate::task::*;
use crate::vm::Launch;

pub fn problems() -> Vec<Open> {
    vec![quick_words(), long_memory(), mended_words()]
}

fn case(rom: Vec<i32>) -> Case {
    Case { rom, frame: 0 }
}

// ---------------------------------------------------------------- quick-words

/// Token orders of ORI's sentences: from the start token 0 each token is
/// followed by the next, and the last one leads back to `order[wrap]`.
/// 栅纹到此为止。下面那层，会长。 and 下面那层会长，栅纹到此为止。
const FIRST: [i32; 15] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14];
const SECOND: [i32; 15] = [0, 8, 9, 10, 11, 13, 14, 12, 1, 2, 3, 4, 5, 6, 7];

/// The token after `a` in a sentence whose last token leads back to `order[1]`
/// (the chapter's sentence: 长 → 。 → 下, so FIRST wraps to 7).
fn follows(order: &[i32; 15], wrap: usize, a: i32) -> i32 {
    match order.iter().position(|&t| t == a) {
        Some(14) => order[wrap],
        Some(i) => order[i + 1],
        None => 0,
    }
}

/// A 16 × 16 FP16 matrix with `v` at (i, f(i)).
fn perm_matrix(f: impl Fn(usize) -> usize, v: f64) -> Vec<i32> {
    (0..256).map(|n| fp::h(if f(n / 16) == n % 16 { v } else { 0.0 })).collect()
}

/// ORI's model in the chapter's layout (E, Wq, Wk, Wv, Wo from rom[32]),
/// with the rows and columns shuffled by `shift` (0 is the chapter's own
/// model): token a embeds at row σ(a), Wq and Wk are scaled permutations
/// whose product is 6 · I, Wv sends σ(a) to the token after a and Wo
/// undoes the shuffle.
fn ori_model(order: &[i32; 15], wrap: usize, shift: usize) -> Vec<i32> {
    let sig = |a: usize| (a * (2 * shift + 1) + shift) % 16;
    let inv = |r: usize| (0..16).find(|&a| sig(a) == r).unwrap();
    let mut rom = perm_matrix(sig, 1.0);
    let (q, k) = if shift == 0 { (6.0, 1.0) } else { (3.0, 2.0) };
    rom.extend(perm_matrix(|i| (i + 5 * shift) % 16, q));
    rom.extend(perm_matrix(|i| (i + 5 * shift) % 16, k));
    rom.extend(perm_matrix(|r| sig(follows(order, wrap, inv(r) as i32) as usize), 1.0));
    rom.extend(perm_matrix(inv, 1.0));
    rom
}

/// The sentence with its first `k` characters given: rom[0] = L tokens to
/// say, rom[1] = index of the last prompt token, rom[2..] = the prompt.
fn sentence(order: &[i32; 15], wrap: usize, shift: usize, k: usize) -> Case {
    let mut rom = vec![0; 32];
    rom[0] = 15 - k as i32;
    rom[1] = k as i32;
    rom[2..3 + k].copy_from_slice(&order[..=k]);
    rom.extend(ori_model(order, wrap, shift));
    case(rom)
}

/// Runs the model in FP64 with a KV cache: out[g] = g-th token said.
fn spoken(c: &Case) -> Vec<Option<i32>> {
    let w = |m: usize, i: usize, j: usize| h_to_f64(c.rom[32 + 256 * m + 16 * i + j]);
    let proj = |x: &[f64], m: usize| -> Vec<f64> {
        (0..16).map(|j| (0..16).map(|i| x[i] * w(m, i, j)).sum()).collect()
    };
    let (l, last) = (c.rom[0] as usize, c.rom[1] as usize);
    let mut toks: Vec<i32> = c.rom[2..3 + last].to_vec();
    let (mut ks, mut vs) = (Vec::new(), Vec::new());
    let mut out = vec![None; 16];
    for t in 0..last + l {
        let x: Vec<f64> = (0..16).map(|j| w(0, toks[t] as usize, j)).collect();
        let q = proj(&x, 1);
        ks.push(proj(&x, 2));
        vs.push(proj(&x, 3));
        let e: Vec<f64> = ks.iter().map(|k| (0..16).map(|i| q[i] * k[i]).sum::<f64>().exp()).collect();
        let o: Vec<f64> = (0..16).map(|j| (0..=t).map(|p| e[p] * vs[p][j]).sum()).collect();
        let logits = proj(&o, 4);
        let next = (0..16).max_by(|&a, &b| logits[a].total_cmp(&logits[b])).unwrap() as i32;
        if t >= last {
            out[t - last] = Some(next);
            toks.push(next);
        }
    }
    out
}

/// Worst cycles over the cases per character of the 15-token sentence.
fn per_char(rep: &Report) -> u32 {
    let worst = rep.runs.iter().map(|r| r.run.stats.cycles).max().unwrap_or(0);
    worst.div_ceil(15)
}

/// Folds the model into two tables first, S = (E·Wq)(E·Wk)ᵀ and VO =
/// E·Wv·Wo, with MMA tiles; then a step is one score per cached token, an
/// exponent, a weighted sum of VO rows and an argmax.
const QUICK_REF: &str = ".smem 2560\n\
    DMA 0, 32, 1280\n\
    LD r0, [0]\n\
    LD r1, [1]\n\
    AND r2, lane, 3\n\
    SHR r3, lane, 2\n\
    AND r3, r3, 3\n\
    SHL r4, r3, 4\n\
    ADD r4, r4, r2\n\
    SHL r5, r2, 4\n\
    ADD r5, r5, r3\n\
    SHR r6, lane, 4\n\
    MOV r7, 0\n\
    MOV r8, 1280\n\
    BAR\n\
    LOOP 5\n\
    MOV r9, 0x80500\n\
    SHR r9, r9, r7\n\
    AND r9, r9, 15\n\
    SHL r9, r9, 8\n\
    MOV r10, 0x43621\n\
    SHR r10, r10, r7\n\
    AND r10, r10, 15\n\
    SHL r10, r10, 8\n\
    SETP.EQ p0, r7, 4\n\
    MOV r11, r6\n\
    LOOP 8\n\
    SHR r12, r11, 2\n\
    AND r13, r11, 3\n\
    SHL r14, r12, 6\n\
    ADD r14, r14, r9\n\
    ADD r14, r14, r4\n\
    SHL r15, r13, 2\n\
    ADD r15, r15, r10\n\
    ADD r15, r15, r4\n\
    MOV r16, 0\n\
    LOOP 4\n\
    LDS r17, [r14]\n\
    LDS r18, [r15]\n\
    MMA r16, r17, r18, r16\n\
    ADD r14, r14, 4\n\
    ADD r15, r15, 64\n\
    ENDLOOP\n\
    CVT.H.F r16, r16\n\
    SEL r17, p0, r13, r12\n\
    SEL r18, p0, r12, r13\n\
    SHL r17, r17, 6\n\
    SHL r18, r18, 2\n\
    ADD r17, r17, r18\n\
    ADD r17, r17, r8\n\
    SEL r18, p0, r5, r4\n\
    ADD r17, r17, r18\n\
    STS [r17], r16\n\
    ADD r11, r11, 2\n\
    ENDLOOP\n\
    ADD r7, r7, 4\n\
    ADD r8, r8, 256\n\
    ENDLOOP\n\
    AND r23, lane, 15\n\
    ADD r24, r23, 2304\n\
    MOV r21, 0\n\
    MOV r19, 0\n\
    LOOP 15\n\
    ADD r13, r21, 2\n\
    LD r14, [r13]\n\
    SETP.LE p1, r21, r1\n\
    SEL r20, p1, r14, r20\n\
    SHL r22, r20, 4\n\
    SETP.EQ p2, lane, r21\n\
    SEL r19, p2, r22, r19\n\
    ADD r13, r22, 1792\n\
    SHR r14, r19, 4\n\
    ADD r13, r13, r14\n\
    LDS r15, [r13]\n\
    HEXP r15, r15\n\
    SETP.LE p2, lane, r21\n\
    SEL r15, p2, r15, 0\n\
    MOV r16, 0\n\
    MOV r17, r6\n\
    LOOP 8\n\
    SHFL r13, r15, r17\n\
    SHFL r14, r19, r17\n\
    ADD r14, r14, r24\n\
    LDS r14, [r14]\n\
    HMUL r14, r14, r13\n\
    HADD r16, r16, r14\n\
    ADD r17, r17, 2\n\
    ENDLOOP\n\
    XOR r13, lane, 16\n\
    SHFL r14, r16, r13\n\
    HADD r16, r16, r14\n\
    SHL r16, r16, 4\n\
    OR r16, r16, r23\n\
    MOV r12, 8\n\
    LOOP 4\n\
    XOR r13, lane, r12\n\
    SHFL r14, r16, r13\n\
    MAX r16, r16, r14\n\
    SHR r12, r12, 1\n\
    ENDLOOP\n\
    AND r20, r16, 15\n\
    SUB r13, r21, r1\n\
    SETP.GE p2, r13, 0\n\
    IF p2\n\
    ST [r13], r20\n\
    ENDIF\n\
    ADD r21, r21, 1\n\
    ENDLOOP\n";

fn quick_words() -> Open {
    Open {
        task: Task {
            chapter: 16,
            slot: 9,
            title: Tr::new("Quick Words", "快嘴"),
            brief: Tr::new(
                "The square wants ORI to speak faster. The model and rules are those of First Words (rom[0] = L, rom[1] = index of the last prompt token, the prompt from rom[2], E, Wq, Wk, Wv, Wo from rom[32]; out[0..L) gets the tokens said). Some days ORI runs a reshuffled model that says another sentence, so every word must come from the weights. One warp.\nScored by cycles per character: the slowest day's cycles over the sentence's 15 characters, rounded up; fewer is better. The goal line is 602, half of the chapter's KV-cached decoder (1204).",
                "广场上的人想让 ORI 说快一点。模型和规矩都和「第一句话」一样（rom[0] = L，rom[1] = 提示里最后一个字的下标，提示从 rom[2] 起，E、Wq、Wk、Wv、Wo 从 rom[32] 起；说出的字写进 out[0..L)）。有些日子 ORI 用的是打乱过的模型，说的是另一句话，所以每个字都得由权重算出来。一个 warp。\n按每字周期计分：最慢那天的总周期除以整句的 15 个字，向上取整；越少越好。目标线是 602，本章 KV 缓存解码器（1204）的一半。",
            ),
            cases: || vec![sentence(&FIRST, 7, 0, 0), sentence(&FIRST, 7, 0, 2), sentence(&SECOND, 1, 1, 0)],
            hidden: || vec![sentence(&FIRST, 7, 0, 9), sentence(&SECOND, 1, 3, 4), sentence(&FIRST, 7, 6, 14)],
            expect: spoken,
            starter: Source::Code(""),
            reference: Source::Code(QUICK_REF),
            ..base("quick-words", 16, 9, Launch::grid(1, 32), 16, ORIGIN_OPS)
        },
        metric: Metric::Cycles,
        score: Some(per_char),
        score_label: Some(Tr::new("cycles per character", "每字周期")),
        marks: &[339, 1204, 13074],
    }
}

// ---------------------------------------------------------------- long-memory

/// Remembered positions, and where K and V start.
const T: usize = 256;
const K0: usize = 16;
const V0: usize = K0 + 16 * T;

/// One FP16 value in quarters, -2 ..= 2 (or -`reach` ..= `reach` quarters).
fn quarter(r: &mut oo_base::Rng, reach: i32) -> i32 {
    fp::h(0.25 * (below(r, 2 * reach + 1) - reach) as f64)
}

/// ORI's query q (rom[0..16)) and the day's 256 keys and values, FP16:
/// K[p][d] at rom[16 + 16p + d], V[p][d] at rom[4112 + 16p + d]. `reach`
/// bounds the keys in quarters; `plant` copies q into the keys at those
/// positions, so they score highest.
fn memory(seed: u64, reach: i32, plant: &[usize]) -> Case {
    let mut r = rng(seed);
    let mut rom: Vec<i32> = (0..K0).map(|_| quarter(&mut r, 8)).collect();
    rom.extend((0..16 * T).map(|_| quarter(&mut r, reach)));
    rom.extend((0..16 * T).map(|_| fp::h((below(&mut r, 65) - 32) as f64 / 8.0)));
    for &p in plant {
        rom.copy_within(0..16, K0 + 16 * p);
    }
    case(rom)
}

/// out[d] = round(100 · o_d), o = softmax(q · K) · V over all 256 positions.
fn recalled(c: &Case) -> Vec<Option<i32>> {
    let x = |i: usize| h_to_f64(c.rom[i]);
    let s: Vec<f64> = (0..T).map(|p| (0..16).map(|d| x(d) * x(K0 + 16 * p + d)).sum()).collect();
    let m = s.iter().cloned().fold(f64::MIN, f64::max);
    let e: Vec<f64> = s.iter().map(|v| (v - m).exp()).collect();
    let l: f64 = e.iter().sum();
    (0..16)
        .map(|d| {
            let o: f64 = (0..T).map(|p| e[p] * x(V0 + 16 * p + d)).sum();
            Some((100.0 * o / l).round() as i32)
        })
        .collect()
}

/// Each SM takes a quarter of the day: scores, its own max m, l = Σ e^(s-m)
/// and o = Σ e^(s-m) v; SM 2 hands its (m, l, o) to SM 1, SMs 1 and 3 to
/// SM 0, and every merge rescales by e^(m_old - m_new).
const LONG_REF: &str = ".smem 2176\n\
    SHL r1, bx, 10\n\
    ADD r2, r1, 16\n\
    SETP.EQ p0, lane, 0\n\
    IF p0\n\
    DMA 0, r2, 1024\n\
    ADD r2, r1, 4112\n\
    DMA 1024, r2, 1024\n\
    ENDIF\n\
    LD r30, [lane]\n\
    SHR r3, lane, 1\n\
    SHL r4, lane, 4\n\
    BAR\n\
    LOOP 16\n\
    AND r5, r3, 15\n\
    SHFL r6, r30, r5\n\
    ADD r7, r4, r5\n\
    ADD r8, r7, 512\n\
    LDS r7, [r7]\n\
    LDS r8, [r8]\n\
    HMUL r7, r7, r6\n\
    HMUL r8, r8, r6\n\
    HADD r11, r11, r7\n\
    HADD r12, r12, r8\n\
    ADD r3, r3, 1\n\
    ENDLOOP\n\
    ADD r7, lane, 2048\n\
    STS [r7], r11\n\
    ADD r7, r7, 32\n\
    STS [r7], r12\n\
    HMAX r23, r11, r12\n\
    MOV r9, 16\n\
    LOOP 5\n\
    XOR r5, lane, r9\n\
    SHFL r6, r23, r5\n\
    HMAX r23, r23, r6\n\
    SHR r9, r9, 1\n\
    ENDLOOP\n\
    AND r16, lane, 15\n\
    SHR r17, lane, 4\n\
    SHL r18, r17, 4\n\
    ADD r18, r18, r16\n\
    ADD r18, r18, 1024\n\
    ADD r19, r17, 2048\n\
    LOOP 32\n\
    LDS r12, [r19]\n\
    LDS r11, [r18]\n\
    ADD r19, r19, 2\n\
    ADD r18, r18, 32\n\
    HSUB r12, r12, r23\n\
    HEXP r12, r12\n\
    CVT.F.H r11, r11\n\
    CVT.F.H r12, r12\n\
    FMUL r11, r11, r12\n\
    FADD r24, r24, r12\n\
    FADD r20, r20, r11\n\
    ENDLOOP\n\
    XOR r5, lane, 16\n\
    SHFL r6, r20, r5\n\
    FADD r20, r20, r6\n\
    SHFL r6, r24, r5\n\
    FADD r24, r24, r6\n\
    SETP.EQ p1, lane, 16\n\
    SEL r20, p1, r24, r20\n\
    SETP.LT p2, bx, 2\n\
    SUB r25, 3, bx\n\
    LOOP 2\n\
    IF p2\n\
    RECV r25, 2144, 18\n\
    ADD r5, lane, 2144\n\
    LDS r6, [r5]\n\
    LDS r7, [2161]\n\
    HMAX r9, r23, r7\n\
    HSUB r10, r23, r9\n\
    HEXP r10, r10\n\
    CVT.F.H r10, r10\n\
    HSUB r11, r7, r9\n\
    HEXP r11, r11\n\
    CVT.F.H r11, r11\n\
    FMUL r20, r20, r10\n\
    FMUL r6, r6, r11\n\
    FADD r20, r20, r6\n\
    MOV r23, r9\n\
    ENDIF\n\
    SETP.EQ p2, bx, 0\n\
    MOV r25, 1\n\
    ENDLOOP\n\
    SETP.NE p2, bx, 0\n\
    IF p2\n\
    SETP.EQ p1, bx, 2\n\
    SEL r25, p1, 1, 0\n\
    SETP.EQ p1, lane, 17\n\
    SEL r5, p1, r23, r20\n\
    ADD r6, lane, 2112\n\
    STS [r6], r5\n\
    SEND r25, 2112, 18\n\
    ELSE\n\
    SHFL r5, r20, 16\n\
    CVT.H.F r5, r5\n\
    HRCP r5, r5\n\
    CVT.F.H r5, r5\n\
    FMUL r20, r20, r5\n\
    FMUL r20, r20, 0x42C80000\n\
    CVT.I.F r20, r20\n\
    SETP.LT p1, lane, 16\n\
    IF p1\n\
    ST [lane], r20\n\
    ENDIF\n\
    ENDIF\n";

/// The same with the states handed down the chain 3 → 2 → 1 → 0.
#[cfg(test)]
const LONG_CHAIN: &str = ".smem 2176\n\
    SHL r1, bx, 10\n\
    ADD r2, r1, 16\n\
    SETP.EQ p0, lane, 0\n\
    IF p0\n\
    DMA 0, r2, 1024\n\
    ADD r2, r1, 4112\n\
    DMA 1024, r2, 1024\n\
    ENDIF\n\
    LD r30, [lane]\n\
    SHR r3, lane, 1\n\
    SHL r4, lane, 4\n\
    BAR\n\
    LOOP 16\n\
    AND r5, r3, 15\n\
    SHFL r6, r30, r5\n\
    ADD r7, r4, r5\n\
    ADD r8, r7, 512\n\
    LDS r7, [r7]\n\
    LDS r8, [r8]\n\
    HMUL r7, r7, r6\n\
    HMUL r8, r8, r6\n\
    HADD r11, r11, r7\n\
    HADD r12, r12, r8\n\
    ADD r3, r3, 1\n\
    ENDLOOP\n\
    ADD r7, lane, 2048\n\
    STS [r7], r11\n\
    ADD r7, r7, 32\n\
    STS [r7], r12\n\
    HMAX r23, r11, r12\n\
    MOV r9, 16\n\
    LOOP 5\n\
    XOR r5, lane, r9\n\
    SHFL r6, r23, r5\n\
    HMAX r23, r23, r6\n\
    SHR r9, r9, 1\n\
    ENDLOOP\n\
    AND r16, lane, 15\n\
    SHR r17, lane, 4\n\
    SHL r18, r17, 4\n\
    ADD r18, r18, r16\n\
    ADD r18, r18, 1024\n\
    ADD r19, r17, 2048\n\
    LOOP 32\n\
    LDS r12, [r19]\n\
    LDS r11, [r18]\n\
    ADD r19, r19, 2\n\
    ADD r18, r18, 32\n\
    HSUB r12, r12, r23\n\
    HEXP r12, r12\n\
    CVT.F.H r11, r11\n\
    CVT.F.H r12, r12\n\
    FMUL r11, r11, r12\n\
    FADD r24, r24, r12\n\
    FADD r20, r20, r11\n\
    ENDLOOP\n\
    XOR r5, lane, 16\n\
    SHFL r6, r20, r5\n\
    FADD r20, r20, r6\n\
    SHFL r6, r24, r5\n\
    FADD r24, r24, r6\n\
    SETP.EQ p1, lane, 16\n\
    SEL r20, p1, r24, r20\n\
    SETP.LT p2, bx, 3\n\
    IF p2\n\
    ADD r25, bx, 1\n\
    RECV r25, 2144, 18\n\
    ADD r5, lane, 2144\n\
    LDS r6, [r5]\n\
    LDS r7, [2161]\n\
    HMAX r9, r23, r7\n\
    HSUB r10, r23, r9\n\
    HEXP r10, r10\n\
    CVT.F.H r10, r10\n\
    HSUB r11, r7, r9\n\
    HEXP r11, r11\n\
    CVT.F.H r11, r11\n\
    FMUL r20, r20, r10\n\
    FMUL r6, r6, r11\n\
    FADD r20, r20, r6\n\
    MOV r23, r9\n\
    ENDIF\n\
    SETP.NE p2, bx, 0\n\
    IF p2\n\
    SUB r25, bx, 1\n\
    SETP.EQ p1, lane, 17\n\
    SEL r5, p1, r23, r20\n\
    ADD r6, lane, 2112\n\
    STS [r6], r5\n\
    SEND r25, 2112, 18\n\
    ELSE\n\
    SHFL r5, r20, 16\n\
    CVT.H.F r5, r5\n\
    HRCP r5, r5\n\
    CVT.F.H r5, r5\n\
    FMUL r20, r20, r5\n\
    FMUL r20, r20, 0x42C80000\n\
    CVT.I.F r20, r20\n\
    SETP.LT p1, lane, 16\n\
    IF p1\n\
    ST [lane], r20\n\
    ENDIF\n\
    ENDIF\n";

/// SM 0 alone walks the four quarters, folding each into a running state.
#[cfg(test)]
const LONG_ONE: &str = ".smem 2176\n\
    SETP.EQ p3, bx, 0\n\
    IF p3\n\
    MOV r27, 0xFC00\n\
    MOV r1, 0\n\
    LD r30, [lane]\n\
    LOOP 4\n\
    ADD r2, r1, 16\n\
    SETP.EQ p0, lane, 0\n\
    IF p0\n\
    DMA 0, r2, 1024\n\
    ADD r2, r1, 4112\n\
    DMA 1024, r2, 1024\n\
    ENDIF\n\
    SHR r3, lane, 1\n\
    SHL r4, lane, 4\n\
    MOV r11, 0\n\
    MOV r12, 0\n\
    MOV r20, 0\n\
    MOV r24, 0\n\
    BAR\n\
    LOOP 16\n\
    AND r5, r3, 15\n\
    SHFL r6, r30, r5\n\
    ADD r7, r4, r5\n\
    ADD r8, r7, 512\n\
    LDS r7, [r7]\n\
    LDS r8, [r8]\n\
    HMUL r7, r7, r6\n\
    HMUL r8, r8, r6\n\
    HADD r11, r11, r7\n\
    HADD r12, r12, r8\n\
    ADD r3, r3, 1\n\
    ENDLOOP\n\
    ADD r7, lane, 2048\n\
    STS [r7], r11\n\
    ADD r7, r7, 32\n\
    STS [r7], r12\n\
    HMAX r23, r11, r12\n\
    MOV r9, 16\n\
    LOOP 5\n\
    XOR r5, lane, r9\n\
    SHFL r6, r23, r5\n\
    HMAX r23, r23, r6\n\
    SHR r9, r9, 1\n\
    ENDLOOP\n\
    AND r16, lane, 15\n\
    SHR r17, lane, 4\n\
    SHL r18, r17, 4\n\
    ADD r18, r18, r16\n\
    ADD r18, r18, 1024\n\
    ADD r19, r17, 2048\n\
    LOOP 32\n\
    LDS r11, [r18]\n\
    LDS r12, [r19]\n\
    CVT.F.H r11, r11\n\
    HSUB r12, r12, r23\n\
    HEXP r12, r12\n\
    CVT.F.H r12, r12\n\
    FMUL r11, r11, r12\n\
    FADD r20, r20, r11\n\
    FADD r24, r24, r12\n\
    ADD r18, r18, 32\n\
    ADD r19, r19, 2\n\
    ENDLOOP\n\
    XOR r5, lane, 16\n\
    SHFL r6, r20, r5\n\
    FADD r20, r20, r6\n\
    SHFL r6, r24, r5\n\
    FADD r24, r24, r6\n\
    SETP.EQ p1, lane, 16\n\
    SEL r20, p1, r24, r20\n\
    HMAX r9, r27, r23\n\
    HSUB r10, r27, r9\n\
    HEXP r10, r10\n\
    CVT.F.H r10, r10\n\
    HSUB r11, r23, r9\n\
    HEXP r11, r11\n\
    CVT.F.H r11, r11\n\
    FMUL r26, r26, r10\n\
    FMUL r20, r20, r11\n\
    FADD r26, r26, r20\n\
    MOV r27, r9\n\
    ADD r1, r1, 1024\n\
    BAR\n\
    ENDLOOP\n\
    SHFL r5, r26, 16\n\
    CVT.H.F r5, r5\n\
    HRCP r5, r5\n\
    CVT.F.H r5, r5\n\
    FMUL r26, r26, r5\n\
    FMUL r26, r26, 0x42C80000\n\
    CVT.I.F r26, r26\n\
    SETP.LT p1, lane, 16\n\
    IF p1\n\
    ST [lane], r26\n\
    ENDIF\n\
    ENDIF\n";

fn long_memory() -> Open {
    Open {
        task: Task {
            chapter: 16,
            slot: 10,
            title: Tr::new("A Long Memory", "长长的记性"),
            brief: Tr::new(
                "ORI remembers the whole day in the square: a query q at rom[0..16), 256 keys K[p] at rom[16 + 16p ..] and values V[p] at rom[4112 + 16p ..], 16 FP16 words each. Write out[d] = round(100 · o_d) for o = softmax(q·K)·V, within 2. The scores run high enough to overflow a half. Four SMs, one warp each.\nScored by total cycles: fewer is better.",
                "ORI 记得广场上一整天的话：查询 q 在 rom[0..16)，256 个键 K[p] 在 rom[16 + 16p ..]，值 V[p] 在 rom[4112 + 16p ..]，每个 16 个 FP16 字。写 out[d] = round(100 · o_d)，o = softmax(q·K)·V，误差不超过 2。分数大到能让半精度溢出。四个 SM，每个一个 warp。\n按总周期计分：越少越好。",
            ),
            cases: || vec![memory(16201, 8, &[]), memory(16202, 2, &[]), memory(16203, 8, &[200, 77])],
            hidden: || vec![memory(16204, 8, &[150]), memory(16205, 4, &[])],
            expect: recalled,
            tolerance: 2,
            starter: Source::Code(""),
            reference: Source::Code(LONG_REF),
            ..base("long-memory", 16, 10, Launch::grid(4, 32), 16, ORIGIN_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[1554, 1612, 5621],
    }
}

// ---------------------------------------------------------------- mended-words

/// Words in Cenli's letter.
const LETTER: usize = 1024;

/// 1024 random 32-bit words. The low ten bits of word 0 hide the link's
/// flaw: bit (w & 31) of the first word of message ((w >> 5) & 31) on the
/// link from SM 0 to SM 1 is inverted, and 31 means a clean day.
fn letter(seed: u64, nth: i32, bit: i32) -> Case {
    let mut r = rng(seed);
    let mut rom: Vec<i32> = (0..LETTER).map(|_| (below(&mut r, 65536) << 16) | below(&mut r, 65536)).collect();
    rom[0] = (rom[0] & !1023) | (nth << 5) | bit;
    case(rom)
}

fn letter_flip(c: &Case) -> Vec<crate::vm::Flip> {
    let (nth, bit) = ((c.rom[0] >> 5) & 31, c.rom[0] & 31);
    if nth == 31 {
        return Vec::new();
    }
    vec![crate::vm::Flip { from: 0, to: 1, nth: nth as u16, word: 0, bit: bit as u8 }]
}

/// Words on every link plus one header word per message, worst case.
fn with_headers(rep: &Report) -> u32 {
    let per = |r: &crate::grade::CaseRun| r.run.stats.link_words + r.run.msgs.len() as u32;
    rep.runs.iter().map(per).max().unwrap_or(0)
}

fn letter_out(c: &Case) -> Vec<Option<i32>> {
    c.rom.iter().map(|&w| Some(w)).collect()
}

/// Sixteen messages of 64 words, then one check word (S << 6) ^ f(X): X is
/// the XOR of all words and f(X) the XOR of j | 32 over its set bits j; S is
/// the XOR of i + 1 over the words i of odd parity. SM 1 computes the same
/// word: the difference holds 32 | the flipped bit and i + 1, or a single
/// bit when the check word itself was hit.
const MENDED_REF: &str = ".smem 1056\n\
    SETP.EQ p3, bx, 0\n\
    SETP.EQ p2, bx, 1\n\
    IF p3\n\
    SETP.EQ p0, lane, 0\n\
    IF p0\n\
    DMA 0, 0, 1024\n\
    ENDIF\n\
    BAR\n\
    MOV r1, 0\n\
    LOOP 16\n\
    AND r2, r1, 1023\n\
    SEND 1, r2, 64\n\
    ADD r1, r1, 64\n\
    ENDLOOP\n\
    ENDIF\n\
    IF p2\n\
    MOV r1, 0\n\
    LOOP 16\n\
    RECV 0, r1, 64\n\
    ADD r1, r1, 64\n\
    ENDLOOP\n\
    ENDIF\n\
    MOV r2, lane\n\
    ADD r3, lane, 1\n\
    LOOP 32\n\
    LDS r4, [r2]\n\
    XOR r5, r5, r4\n\
    SHR r6, r4, 16\n\
    XOR r4, r4, r6\n\
    SHR r6, r4, 8\n\
    XOR r4, r4, r6\n\
    SHR r6, r4, 4\n\
    XOR r4, r4, r6\n\
    SHR r6, r4, 2\n\
    XOR r4, r4, r6\n\
    SHR r6, r4, 1\n\
    XOR r4, r4, r6\n\
    AND r4, r4, 1\n\
    SUB r4, 0, r4\n\
    AND r4, r4, r3\n\
    XOR r7, r7, r4\n\
    ADD r2, r2, 32\n\
    ADD r3, r3, 32\n\
    ENDLOOP\n\
    SHL r7, r7, 6\n\
    MOV r10, 16\n\
    LOOP 5\n\
    XOR r8, lane, r10\n\
    SHFL r9, r5, r8\n\
    XOR r5, r5, r9\n\
    SHR r10, r10, 1\n\
    ENDLOOP\n\
    SHR r11, r5, lane\n\
    AND r11, r11, 1\n\
    SUB r11, 0, r11\n\
    OR r12, lane, 32\n\
    AND r11, r11, r12\n\
    XOR r11, r11, r7\n\
    MOV r10, 16\n\
    LOOP 5\n\
    XOR r8, lane, r10\n\
    SHFL r9, r11, r8\n\
    XOR r11, r11, r9\n\
    SHR r10, r10, 1\n\
    ENDLOOP\n\
    IF p3\n\
    SETP.EQ p0, lane, 0\n\
    IF p0\n\
    STS [1024], r11\n\
    ENDIF\n\
    SEND 1, 1024, 1\n\
    ENDIF\n\
    IF p2\n\
    RECV 0, 1024, 1\n\
    LDS r12, [1024]\n\
    XOR r12, r12, r11\n\
    AND r13, r12, 31\n\
    SHR r14, r12, 6\n\
    AND r15, r12, 32\n\
    SETP.NE p0, r15, 0\n\
    SEL r14, p0, r14, 0\n\
    MOV r15, 1\n\
    SHL r13, r15, r13\n\
    SETP.NE p0, r14, 0\n\
    IF p0\n\
    SUB r14, r14, 1\n\
    LDS r15, [r14]\n\
    XOR r15, r15, r13\n\
    STS [r14], r15\n\
    ENDIF\n\
    MOV r2, lane\n\
    LOOP 32\n\
    LDS r4, [r2]\n\
    ST [r2], r4\n\
    ADD r2, r2, 32\n\
    ENDLOOP\n\
    ENDIF\n";

/// X and S as two check words.
#[cfg(test)]
const MENDED_TWO: &str = ".smem 1056\n\
    SETP.EQ p3, bx, 0\n\
    SETP.EQ p2, bx, 1\n\
    IF p3\n\
    SETP.EQ p0, lane, 0\n\
    IF p0\n\
    DMA 0, 0, 1024\n\
    ENDIF\n\
    BAR\n\
    MOV r1, 0\n\
    LOOP 16\n\
    AND r2, r1, 1023\n\
    SEND 1, r2, 64\n\
    ADD r1, r1, 64\n\
    ENDLOOP\n\
    ENDIF\n\
    IF p2\n\
    MOV r1, 0\n\
    LOOP 16\n\
    RECV 0, r1, 64\n\
    ADD r1, r1, 64\n\
    ENDLOOP\n\
    ENDIF\n\
    MOV r2, lane\n\
    ADD r3, lane, 1\n\
    LOOP 32\n\
    LDS r4, [r2]\n\
    XOR r5, r5, r4\n\
    SHR r6, r4, 16\n\
    XOR r4, r4, r6\n\
    SHR r6, r4, 8\n\
    XOR r4, r4, r6\n\
    SHR r6, r4, 4\n\
    XOR r4, r4, r6\n\
    SHR r6, r4, 2\n\
    XOR r4, r4, r6\n\
    SHR r6, r4, 1\n\
    XOR r4, r4, r6\n\
    AND r4, r4, 1\n\
    SUB r4, 0, r4\n\
    AND r4, r4, r3\n\
    XOR r7, r7, r4\n\
    ADD r2, r2, 32\n\
    ADD r3, r3, 32\n\
    ENDLOOP\n\
    MOV r10, 16\n\
    LOOP 5\n\
    XOR r8, lane, r10\n\
    SHFL r9, r5, r8\n\
    XOR r5, r5, r9\n\
    SHFL r9, r7, r8\n\
    XOR r7, r7, r9\n\
    SHR r10, r10, 1\n\
    ENDLOOP\n\
    IF p3\n\
    SETP.EQ p0, lane, 0\n\
    IF p0\n\
    STS [1024], r5\n\
    STS [1025], r7\n\
    ENDIF\n\
    SEND 1, 1024, 2\n\
    ENDIF\n\
    IF p2\n\
    RECV 0, 1024, 2\n\
    LDS r12, [1024]\n\
    LDS r14, [1025]\n\
    XOR r13, r12, r5\n\
    XOR r14, r14, r7\n\
    SETP.NE p0, r13, 0\n\
    SEL r14, p0, r14, 0\n\
    SETP.NE p0, r14, 0\n\
    IF p0\n\
    SUB r14, r14, 1\n\
    LDS r15, [r14]\n\
    XOR r15, r15, r13\n\
    STS [r14], r15\n\
    ENDIF\n\
    MOV r2, lane\n\
    LOOP 32\n\
    LDS r4, [r2]\n\
    ST [r2], r4\n\
    ADD r2, r2, 32\n\
    ENDLOOP\n\
    ENDIF\n";

/// The letter three times over, a bitwise vote at SM 1.
#[cfg(test)]
const MENDED_TRIPLE: &str = ".smem 3104\n\
    SETP.EQ p3, bx, 0\n\
    SETP.EQ p2, bx, 1\n\
    IF p3\n\
    SETP.EQ p0, lane, 0\n\
    IF p0\n\
    DMA 0, 0, 1024\n\
    ENDIF\n\
    BAR\n\
    MOV r1, 0\n\
    LOOP 48\n\
    AND r2, r1, 1023\n\
    SEND 1, r2, 64\n\
    ADD r1, r1, 64\n\
    ENDLOOP\n\
    ENDIF\n\
    IF p2\n\
    MOV r1, 0\n\
    LOOP 48\n\
    RECV 0, r1, 64\n\
    ADD r1, r1, 64\n\
    ENDLOOP\n\
    ENDIF\n\
    IF p2\n\
    MOV r2, lane\n\
    LOOP 32\n\
    LDS r4, [r2]\n\
    ADD r3, r2, 1024\n\
    LDS r5, [r3]\n\
    ADD r3, r2, 2048\n\
    LDS r6, [r3]\n\
    AND r7, r4, r5\n\
    AND r8, r4, r6\n\
    OR r7, r7, r8\n\
    AND r8, r5, r6\n\
    OR r7, r7, r8\n\
    ST [r2], r7\n\
    ADD r2, r2, 32\n\
    ENDLOOP\n\
    ENDIF\n";

/// The letter with no check at all.
#[cfg(test)]
const MENDED_PLAIN: &str = ".smem 3104\n\
    SETP.EQ p3, bx, 0\n\
    SETP.EQ p2, bx, 1\n\
    IF p3\n\
    SETP.EQ p0, lane, 0\n\
    IF p0\n\
    DMA 0, 0, 1024\n\
    ENDIF\n\
    BAR\n\
    MOV r1, 0\n\
    LOOP 16\n\
    AND r2, r1, 1023\n\
    SEND 1, r2, 64\n\
    ADD r1, r1, 64\n\
    ENDLOOP\n\
    ENDIF\n\
    IF p2\n\
    MOV r1, 0\n\
    LOOP 16\n\
    RECV 0, r1, 64\n\
    ADD r1, r1, 64\n\
    ENDLOOP\n\
    ENDIF\n\
    IF p2\n\
    MOV r2, lane\n\
    LOOP 32\n\
    LDS r4, [r2]\n\
    ST [r2], r4\n\
    ADD r2, r2, 32\n\
    ENDLOOP\n\
    ENDIF\n";

fn mended_words() -> Open {
    Open {
        task: Task {
            chapter: 16,
            slot: 11,
            title: Tr::new("Mended Words", "补字"),
            brief: Tr::new(
                "Cenli sends a 1024-word letter from SM 0 to SM 1: only SM 0 reads rom, only SM 1 writes out[i] = rom[i], and SM 1 cannot answer. Some days the link from SM 0 to SM 1 inverts one bit of one word; a message holds at most 64 words.\nScored by the words sent on all links plus one header per message, on the worst day: fewer is better.",
                "岑理要把一封 1024 个字的信从 SM 0 送到 SM 1：只有 SM 0 读 rom，只有 SM 1 写 out[i] = rom[i]，SM 1 不能回话。有些日子 SM 0 到 SM 1 的链路会把某个字的一位翻过来；一条消息最多 64 个字。\n按所有链路上发出的字数加每条消息一个消息头计分，取最差的一天：越少越好。",
            ),
            cases: || vec![letter(16301, 3, 7), letter(16302, 16, 2), letter(16303, 31, 0)],
            hidden: || vec![letter(16304, 0, 31), letter(16305, 15, 0), letter(16306, 9, 20)],
            expect: letter_out,
            flips: Some(letter_flip),
            starter: Source::Code(""),
            reference: Source::Code(MENDED_REF),
            ..base("mended-words", 16, 11, Launch::grid(4, 32), LETTER, ORIGIN_INT_OPS)
        },
        metric: Metric::LinkWords,
        score: Some(with_headers),
        score_label: Some(Tr::new("link words + headers", "链路字数 + 消息头")),
        marks: &[1042, 1043, 3120],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A chapter level's reference or starter.
    fn level(id: &str, starter: bool) -> &'static str {
        let t = crate::content::task(id).unwrap();
        match if starter { t.starter } else { t.reference } {
            Source::Code(s) => s,
            _ => unreachable!(),
        }
    }

    #[test]
    fn chapter_16_alternatives_pass_and_score_as_marked() {
        super::super::c09::check(
            16,
            &problems(),
            &[
                ("quick-words", level("first-words", false)),
                ("quick-words", level("kv-cache", true)),
                ("long-memory", LONG_CHAIN),
                ("long-memory", LONG_ONE),
                ("mended-words", MENDED_TWO),
                ("mended-words", MENDED_TRIPLE),
            ],
        );
    }

    #[test]
    fn an_unguarded_letter_is_damaged() {
        let o = mended_words();
        let rep = crate::grade::grade(&o.task, &Source::Code(MENDED_PLAIN).solution()).unwrap();
        assert!(!rep.passed(), "{:?}", rep.verdict);
    }
}
