//! Chapter 12 open problems: Ali's wind tower on the Atlas (occupancy,
//! latency hiding, the copy engine, block sizes).

use oo_base::Tr;

use super::Open;
use crate::content::atlas::base;
use crate::content::prism::rom_case;
use crate::content::{below, rng};
use crate::isa::{Alu, OpKind};
use crate::task::*;
use crate::vm::Launch;

pub fn problems() -> Vec<Open> {
    vec![four_hops(), mid_five(), running_total()]
}

/// Everything Atlas has by the end of the chapter: the Atlas set plus local
/// memory, the copy engine and the fence.
const CH12_OPS: &[OpKind] = &[
    OpKind::Mov,
    OpKind::Alu(Alu::Add),
    OpKind::Alu(Alu::Sub),
    OpKind::Alu(Alu::Mul),
    OpKind::Alu(Alu::And),
    OpKind::Alu(Alu::Or),
    OpKind::Alu(Alu::Xor),
    OpKind::Alu(Alu::Shl),
    OpKind::Alu(Alu::Shr),
    OpKind::Alu(Alu::Min),
    OpKind::Alu(Alu::Max),
    OpKind::SetP,
    OpKind::Sel,
    OpKind::If,
    OpKind::Else,
    OpKind::EndIf,
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::Ld,
    OpKind::Ldo,
    OpKind::St,
    OpKind::Nop,
    OpKind::Shfl,
    OpKind::Vote,
    OpKind::Lds,
    OpKind::Sts,
    OpKind::Bar,
    OpKind::Atom,
    OpKind::AtomS,
    OpKind::Gsync,
    OpKind::Ldl,
    OpKind::Stl,
    OpKind::Dma,
    OpKind::Fence,
];

// ---------------------------------------------------------------- four-hops

/// 4096 signposts: rom[i] is where the path goes from post i.
fn posts(seed: u64) -> Case {
    let mut r = rng(seed);
    rom_case((0..4096).map(|_| below(&mut r, 4096)).collect())
}

fn hopped(c: &Case) -> Vec<Option<i32>> {
    let at = |i: i32| c.rom[i as usize];
    (0..4096).map(|i| Some(at(at(at(at(i)))))).collect()
}

/// Blocks of 512: the first four copy the whole table into shared memory
/// with one DMA (4096 words, so one such block per SM), then follow two paths
/// per thread with LDS; the other four blocks leave at once.
const HOP_REF: &str = ".block 512\n\
    .smem 4096\n\
    SETP.LT p0, bx, 4\n\
    IF p0\n\
    SETP.EQ p1, tx, 0\n\
    IF p1\n\
    DMA 0, 0, 4096\n\
    ENDIF\n\
    BAR\n\
    SHL r1, bx, 10\n\
    ADD r1, r1, tx\n\
    ADD r2, r1, 512\n\
    LDS r0, [r1]\n\
    LDS r3, [r2]\n\
    LDS r0, [r0]\n\
    LDS r3, [r3]\n\
    LDS r0, [r0]\n\
    LDS r3, [r3]\n\
    LDS r0, [r0]\n\
    LDS r3, [r3]\n\
    ST [r1], r0\n\
    ST [r2], r3\n\
    ENDIF\n";

/// A path per thread with the starting block size.
#[cfg(test)]
const HOP_PLAIN: &str = "LD r0, [tid]\n\
    LD r0, [r0]\n\
    LD r0, [r0]\n\
    LD r0, [r0]\n\
    ST [tid], r0\n";

/// All eight blocks of 512 copy the table with one DMA each and follow a
/// path per thread: two blocks per SM, twice the copying.
#[cfg(test)]
const HOP_DMA: &str = ".block 512\n\
    .smem 4096\n\
    SETP.EQ p0, tx, 0\n\
    IF p0\n\
    DMA 0, 0, 4096\n\
    ENDIF\n\
    BAR\n\
    LDS r0, [tid]\n\
    LDS r0, [r0]\n\
    LDS r0, [r0]\n\
    LDS r0, [r0]\n\
    ST [tid], r0\n";

/// Blocks of 512 copy the table with LD and STS, eight words per thread.
#[cfg(test)]
const HOP_STAGE: &str = ".block 512\n\
    .smem 4096\n\
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
    STS [tx], r1\n\
    ADD r9, tx, 512\n\
    STS [r9], r2\n\
    ADD r9, tx, 1024\n\
    STS [r9], r3\n\
    ADD r9, tx, 1536\n\
    STS [r9], r4\n\
    ADD r9, tx, 2048\n\
    STS [r9], r5\n\
    ADD r9, tx, 2560\n\
    STS [r9], r6\n\
    ADD r9, tx, 3072\n\
    STS [r9], r7\n\
    ADD r9, tx, 3584\n\
    STS [r9], r8\n\
    BAR\n\
    LDS r0, [tid]\n\
    LDS r0, [r0]\n\
    LDS r0, [r0]\n\
    LDS r0, [r0]\n\
    ST [tid], r0\n";

fn four_hops() -> Open {
    Open {
        task: Task {
            chapter: 12,
            slot: 9,
            title: Tr::new("Four Hops Down", "往下跳四次"),
            brief: Tr::new(
                "Ali's message runs down the tower post to post: from post i it goes to post rom[i], for 4096 posts. Write to out[i] the post the message reaches after four hops from post i. One thread per post, 64 blocks of 64; choose another block size with .block N.\nScored by total cycles: fewer is better.",
                "阿砾的口信顺着塔里的柱子往下传：从第 i 根柱子传到第 rom[i] 根，一共 4096 根。把口信从第 i 根出发跳四次之后到达的柱子写进 out[i]。每根柱子一个线程，64 个块，每块 64 个；可以用 .block N 换块大小。\n按总周期计分：越少越好。",
            ),
            cases: || vec![posts(12901), posts(12902)],
            hidden: || vec![posts(12903), posts(12904)],
            expect: hopped,
            free_block: true,
            starter: Source::Code(""),
            reference: Source::Code(HOP_REF),
            ..base("four-hops", 9, Launch::grid(64, 64), 4096, CH12_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[1311, 1626, 2209, 11357],
    }
}

// ---------------------------------------------------------------- mid-five

/// 4096 gauge readings 0..999, with a spike in every 37th.
fn gusts(seed: u64) -> Case {
    let mut r = rng(seed);
    rom_case(
        (0..4096)
            .map(|i| if i % 37 == 5 { 5000 } else { below(&mut r, 1000) })
            .collect(),
    )
}

/// out[i] = the median of readings i-2 ..= i+2, indices clamped to the record.
fn calmed(c: &Case) -> Vec<Option<i32>> {
    (0..4096i32)
        .map(|i| {
            let mut w: Vec<i32> = (i - 2..=i + 2).map(|j| c.rom[j.clamp(0, 4095) as usize]).collect();
            w.sort();
            Some(w[2])
        })
        .collect()
}

/// Six registers: the median of five is the median of the fifth reading, the
/// larger of the two pair minima and the smaller of the two pair maxima.
/// All 128 warps fit on the SMs at once.
const MID_REF: &str = "SUB r5, tid, 2\n\
    MAX r5, r5, 0\n\
    LD r0, [r5]\n\
    SUB r5, tid, 1\n\
    MAX r5, r5, 0\n\
    LD r1, [r5]\n\
    LD r2, [tid]\n\
    ADD r5, tid, 1\n\
    MIN r5, r5, 4095\n\
    LD r3, [r5]\n\
    ADD r5, tid, 2\n\
    MIN r5, r5, 4095\n\
    LD r4, [r5]\n\
    MIN r5, r0, r1\n\
    MAX r0, r0, r1\n\
    MIN r1, r2, r3\n\
    MAX r2, r2, r3\n\
    MAX r5, r5, r1\n\
    MIN r0, r0, r2\n\
    MIN r1, r4, r5\n\
    MAX r2, r4, r5\n\
    MIN r2, r2, r0\n\
    MAX r1, r1, r2\n\
    ST [tid], r1\n";

/// Five addresses, five readings, a full bubble sort: eleven registers, so
/// only 23 warps fit on an SM and the rest wait for a second round.
#[cfg(test)]
const MID_SORT: &str = "SUB r0, tid, 2\n\
    MAX r0, r0, 0\n\
    SUB r1, tid, 1\n\
    MAX r1, r1, 0\n\
    MOV r2, tid\n\
    ADD r3, tid, 1\n\
    MIN r3, r3, 4095\n\
    ADD r4, tid, 2\n\
    MIN r4, r4, 4095\n\
    LD r5, [r0]\n\
    LD r6, [r1]\n\
    LD r7, [r2]\n\
    LD r8, [r3]\n\
    LD r9, [r4]\n\
    MIN r10, r5, r6\n\
    MAX r6, r5, r6\n\
    MOV r5, r10\n\
    MIN r10, r6, r7\n\
    MAX r7, r6, r7\n\
    MOV r6, r10\n\
    MIN r10, r7, r8\n\
    MAX r8, r7, r8\n\
    MOV r7, r10\n\
    MIN r10, r8, r9\n\
    MAX r9, r8, r9\n\
    MOV r8, r10\n\
    MIN r10, r5, r6\n\
    MAX r6, r5, r6\n\
    MOV r5, r10\n\
    MIN r10, r6, r7\n\
    MAX r7, r6, r7\n\
    MOV r6, r10\n\
    MIN r10, r7, r8\n\
    MAX r8, r7, r8\n\
    MOV r7, r10\n\
    MIN r10, r5, r6\n\
    MAX r6, r5, r6\n\
    MOV r5, r10\n\
    MIN r10, r6, r7\n\
    MAX r7, r6, r7\n\
    MOV r6, r10\n\
    MIN r10, r5, r6\n\
    MAX r6, r5, r6\n\
    MOV r5, r10\n\
    ST [tid], r7\n";

fn mid_five() -> Open {
    Open {
        task: Task {
            chapter: 12,
            slot: 10,
            title: Tr::new("Calm the Gauge", "压住毛刺"),
            brief: Tr::new(
                "Ali's wind gauge spikes now and then. rom[0..4096) are its readings. Write out[i] = the median of readings i-2 to i+2 (a reading off either end counts as the nearest end one). One thread per reading, 64 blocks of 64.\nScored by total cycles: fewer is better.",
                "阿砾的风速计时不时跳一下。rom[0..4096) 是它的读数。写 out[i] = 第 i-2 到 i+2 个读数的中位数（超出两端的读数按最近的端点算）。每个读数一个线程，64 个块，每块 64 个。\n按总周期计分：越少越好。",
            ),
            cases: || vec![gusts(12911), gusts(12912)],
            hidden: || vec![gusts(12913)],
            expect: calmed,
            starter: Source::Code(""),
            reference: Source::Code(MID_REF),
            ..base("mid-five", 10, Launch::grid(64, 64), 4096, CH12_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[1541, 2215],
    }
}

// ---------------------------------------------------------------- running-total

/// 4096 hourly gusts 0..99.
fn hours(seed: u64) -> Case {
    let mut r = rng(seed);
    rom_case((0..4096).map(|_| below(&mut r, 100)).collect())
}

/// out[i] = gusts 0 ..= i; out[4096..4104) is scratch.
fn totals(c: &Case) -> Vec<Option<i32>> {
    let mut sum = 0;
    let mut out: Vec<Option<i32>> = c
        .rom
        .iter()
        .map(|&v| {
            sum += v;
            Some(sum)
        })
        .collect();
    out.resize(4104, None);
    out
}

/// Warp scan, block scan through shared memory, block totals to
/// out[4096 + b], GSYNC, then the earlier blocks' totals are added. Seven
/// registers, so all 128 warps are resident and the GSYNC can release.
const RUN_REF: &str = ".smem 16\n\
    LD r0, [tid]\n\
    SUB r2, lane, 1\n\
    SHFL r1, r0, r2\n\
    SETP.GE p0, lane, 1\n\
    SEL r1, p0, r1, 0\n\
    ADD r0, r0, r1\n\
    SUB r2, lane, 2\n\
    SHFL r1, r0, r2\n\
    SETP.GE p0, lane, 2\n\
    SEL r1, p0, r1, 0\n\
    ADD r0, r0, r1\n\
    SUB r2, lane, 4\n\
    SHFL r1, r0, r2\n\
    SETP.GE p0, lane, 4\n\
    SEL r1, p0, r1, 0\n\
    ADD r0, r0, r1\n\
    SUB r2, lane, 8\n\
    SHFL r1, r0, r2\n\
    SETP.GE p0, lane, 8\n\
    SEL r1, p0, r1, 0\n\
    ADD r0, r0, r1\n\
    SUB r2, lane, 16\n\
    SHFL r1, r0, r2\n\
    SETP.GE p0, lane, 16\n\
    SEL r1, p0, r1, 0\n\
    ADD r0, r0, r1\n\
    SHR r3, tx, 5\n\
    SETP.EQ p0, lane, 31\n\
    IF p0\n\
    STS [r3], r0\n\
    ENDIF\n\
    BAR\n\
    MOV r1, 0\n\
    SETP.LT p0, lane, 16\n\
    IF p0\n\
    LDS r1, [lane]\n\
    ENDIF\n\
    SUB r4, lane, 1\n\
    SHFL r2, r1, r4\n\
    SETP.GE p0, lane, 1\n\
    SEL r2, p0, r2, 0\n\
    ADD r1, r1, r2\n\
    SUB r4, lane, 2\n\
    SHFL r2, r1, r4\n\
    SETP.GE p0, lane, 2\n\
    SEL r2, p0, r2, 0\n\
    ADD r1, r1, r2\n\
    SUB r4, lane, 4\n\
    SHFL r2, r1, r4\n\
    SETP.GE p0, lane, 4\n\
    SEL r2, p0, r2, 0\n\
    ADD r1, r1, r2\n\
    SUB r4, lane, 8\n\
    SHFL r2, r1, r4\n\
    SETP.GE p0, lane, 8\n\
    SEL r2, p0, r2, 0\n\
    ADD r1, r1, r2\n\
    SUB r2, r3, 1\n\
    SHFL r4, r1, r2\n\
    SETP.EQ p0, r3, 0\n\
    SEL r4, p0, 0, r4\n\
    ADD r0, r0, r4\n\
    SHFL r1, r1, 15\n\
    SETP.EQ p0, tx, 0\n\
    IF p0\n\
    ADD r2, bx, 4096\n\
    ST [r2], r1\n\
    ENDIF\n\
    GSYNC\n\
    MOV r1, 0\n\
    SETP.LT p0, lane, bx\n\
    IF p0\n\
    ADD r2, lane, 4096\n\
    LDO r1, [r2]\n\
    ENDIF\n\
    XOR r2, lane, 4\n\
    SHFL r4, r1, r2\n\
    ADD r1, r1, r4\n\
    XOR r2, lane, 2\n\
    SHFL r4, r1, r2\n\
    ADD r1, r1, r4\n\
    XOR r2, lane, 1\n\
    SHFL r4, r1, r2\n\
    ADD r1, r1, r4\n\
    SHFL r1, r1, 0\n\
    ADD r0, r0, r1\n\
    ST [tid], r0\n";

/// No grid barrier: every block also sums all the gusts before its own
/// share, eight registers.
#[cfg(test)]
const RUN_ALONE: &str = ".smem 32\n\
    LD r0, [tid]\n\
    SUB r2, lane, 1\n\
    SHFL r1, r0, r2\n\
    SETP.GE p0, lane, 1\n\
    SEL r1, p0, r1, 0\n\
    ADD r0, r0, r1\n\
    SUB r2, lane, 2\n\
    SHFL r1, r0, r2\n\
    SETP.GE p0, lane, 2\n\
    SEL r1, p0, r1, 0\n\
    ADD r0, r0, r1\n\
    SUB r2, lane, 4\n\
    SHFL r1, r0, r2\n\
    SETP.GE p0, lane, 4\n\
    SEL r1, p0, r1, 0\n\
    ADD r0, r0, r1\n\
    SUB r2, lane, 8\n\
    SHFL r1, r0, r2\n\
    SETP.GE p0, lane, 8\n\
    SEL r1, p0, r1, 0\n\
    ADD r0, r0, r1\n\
    SUB r2, lane, 16\n\
    SHFL r1, r0, r2\n\
    SETP.GE p0, lane, 16\n\
    SEL r1, p0, r1, 0\n\
    ADD r0, r0, r1\n\
    SHR r3, tx, 5\n\
    SETP.EQ p0, lane, 31\n\
    IF p0\n\
    STS [r3], r0\n\
    ENDIF\n\
    BAR\n\
    MOV r1, 0\n\
    SETP.LT p0, lane, 16\n\
    IF p0\n\
    LDS r1, [lane]\n\
    ENDIF\n\
    SUB r4, lane, 1\n\
    SHFL r2, r1, r4\n\
    SETP.GE p0, lane, 1\n\
    SEL r2, p0, r2, 0\n\
    ADD r1, r1, r2\n\
    SUB r4, lane, 2\n\
    SHFL r2, r1, r4\n\
    SETP.GE p0, lane, 2\n\
    SEL r2, p0, r2, 0\n\
    ADD r1, r1, r2\n\
    SUB r4, lane, 4\n\
    SHFL r2, r1, r4\n\
    SETP.GE p0, lane, 4\n\
    SEL r2, p0, r2, 0\n\
    ADD r1, r1, r2\n\
    SUB r4, lane, 8\n\
    SHFL r2, r1, r4\n\
    SETP.GE p0, lane, 8\n\
    SEL r2, p0, r2, 0\n\
    ADD r1, r1, r2\n\
    SUB r2, r3, 1\n\
    SHFL r4, r1, r2\n\
    SETP.EQ p0, r3, 0\n\
    SEL r4, p0, 0, r4\n\
    ADD r0, r0, r4\n\
    SHFL r1, r1, 15\n\
    MOV r5, 0\n\
    MOV r2, tx\n\
    SHL r6, bx, 9\n\
    LOOP 7\n\
    SETP.LT p0, r2, r6\n\
    IF p0\n\
    LD r7, [r2]\n\
    ADD r5, r5, r7\n\
    ENDIF\n\
    ADD r2, r2, 512\n\
    ENDLOOP\n\
    XOR r2, lane, 16\n\
    SHFL r7, r5, r2\n\
    ADD r5, r5, r7\n\
    XOR r2, lane, 8\n\
    SHFL r7, r5, r2\n\
    ADD r5, r5, r7\n\
    XOR r2, lane, 4\n\
    SHFL r7, r5, r2\n\
    ADD r5, r5, r7\n\
    XOR r2, lane, 2\n\
    SHFL r7, r5, r2\n\
    ADD r5, r5, r7\n\
    XOR r2, lane, 1\n\
    SHFL r7, r5, r2\n\
    ADD r5, r5, r7\n\
    SETP.EQ p0, lane, 0\n\
    IF p0\n\
    ADD r2, r3, 16\n\
    STS [r2], r5\n\
    ENDIF\n\
    BAR\n\
    MOV r5, 0\n\
    SETP.LT p0, lane, 16\n\
    IF p0\n\
    ADD r2, lane, 16\n\
    LDS r5, [r2]\n\
    ENDIF\n\
    XOR r2, lane, 16\n\
    SHFL r7, r5, r2\n\
    ADD r5, r5, r7\n\
    XOR r2, lane, 8\n\
    SHFL r7, r5, r2\n\
    ADD r5, r5, r7\n\
    XOR r2, lane, 4\n\
    SHFL r7, r5, r2\n\
    ADD r5, r5, r7\n\
    XOR r2, lane, 2\n\
    SHFL r7, r5, r2\n\
    ADD r5, r5, r7\n\
    XOR r2, lane, 1\n\
    SHFL r7, r5, r2\n\
    ADD r5, r5, r7\n\
    ADD r0, r0, r5\n\
    ST [tid], r0\n";

fn running_total() -> Open {
    Open {
        task: Task {
            chapter: 12,
            slot: 11,
            title: Tr::new("The Gust Log", "累计风压"),
            brief: Tr::new(
                "Ali logs 4096 hourly gusts, rom[0..4096). Write out[i] = the sum of gusts 0 to i. out[4096..4104) is scratch. 8 blocks of 512 threads; a GSYNC only releases when every block is on an SM at once.\nScored by total cycles: fewer is better.",
                "阿砾记了 4096 个小时的风压，rom[0..4096)。写 out[i] = 第 0 到第 i 小时的总和。out[4096..4104) 可作草稿。8 个块，每块 512 个线程；只有所有块同时在 SM 上时 GSYNC 才会放行。\n按总周期计分：越少越好。",
            ),
            cases: || vec![hours(12921), hours(12922)],
            hidden: || vec![hours(12923), hours(12924)],
            expect: totals,
            starter: Source::Code(""),
            reference: Source::Code(RUN_REF),
            ..base("gust-log", 11, Launch::grid(8, 512), 4104, CH12_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[3258, 6389],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_12_alternatives_pass_and_score_as_marked() {
        super::super::c09::check(
            12,
            &problems(),
            &[
                ("four-hops", HOP_PLAIN),
                ("four-hops", HOP_DMA),
                ("four-hops", HOP_STAGE),
                ("mid-five", MID_SORT),
                ("gust-log", RUN_ALONE),
            ],
        );
    }
}
