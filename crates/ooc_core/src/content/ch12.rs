//! Chapter 12 · Atlas: occupancy, spills, DMA and the fence. Client: Yinpo, at the wind tower.

use oo_base::Tr;

use super::atlas::{ATLAS_OPS, base};
use super::{below, rng};
use crate::isa::OpKind;
use crate::task::*;
use crate::vm::{Launch, Profile};

pub fn tasks() -> Vec<Task> {
    vec![
        occupancy(),
        smem_budget(),
        scoreboard(),
        spill(),
        blocksize(),
        dma(),
        fence(),
        sand_atlas(),
        // __TASKS__
    ]
}

/// `n` random values in `lo..hi`.
fn values(seed: u64, n: usize, lo: i32, hi: i32) -> Vec<i32> {
    let mut r = rng(seed);
    (0..n).map(|_| lo + below(&mut r, hi - lo)).collect()
}

fn case(rom: Vec<i32>, frame: i32) -> Case {
    Case { rom, frame }
}

// ---------------------------------------------------------------- 12-1

/// rom[0] = n, rom[1] = stride between the eight planes, data from rom[32].
fn planes(seed: u64, n: usize, stride: usize) -> Case {
    let mut rom = vec![0; 32];
    rom[0] = n as i32;
    rom[1] = stride as i32;
    rom.extend(values(seed, 7 * stride + n, 0, 100));
    case(rom, 0)
}

fn plane_sums(c: &Case) -> Vec<Option<i32>> {
    let (n, s) = (c.rom[0] as usize, c.rom[1] as usize);
    (0..16384)
        .map(|t| {
            Some(if t < n {
                (0..8).map(|k| c.rom[32 + t + k * s]).sum()
            } else {
                0
            })
        })
        .collect()
}

/// Eight reads per thread, hoisted addresses, two batches of four: 14 registers.
const OCC_STARTER: &str = "; correct, but every thread keeps 14 registers | 结果是对的，但每个线程占着 14 个寄存器\n\
    LD r1, [1]\n\
    ADD r0, tid, 32\n\
    ADD r2, r0, r1\n\
    ADD r3, r2, r1\n\
    ADD r4, r3, r1\n\
    ADD r5, r4, r1\n\
    ADD r6, r5, r1\n\
    ADD r7, r6, r1\n\
    ADD r8, r7, r1\n\
    LD r9, [r0]\n\
    LD r10, [r2]\n\
    LD r11, [r3]\n\
    LD r12, [r4]\n\
    ADD r13, r9, r10\n\
    ADD r11, r11, r12\n\
    ADD r13, r13, r11\n\
    LD r9, [r5]\n\
    LD r10, [r6]\n\
    LD r11, [r7]\n\
    LD r12, [r8]\n\
    ADD r9, r9, r10\n\
    ADD r11, r11, r12\n\
    ADD r9, r9, r11\n\
    ADD r13, r13, r9\n\
    ST [tid], r13\n";

const OCC_REF: &str = "LD r1, [1]\n\
    ADD r0, tid, 32\n\
    LOOP 2\n\
    LD r2, [r0]\n\
    ADD r0, r0, r1\n\
    LD r3, [r0]\n\
    ADD r0, r0, r1\n\
    LD r4, [r0]\n\
    ADD r0, r0, r1\n\
    LD r5, [r0]\n\
    ADD r0, r0, r1\n\
    ADD r2, r2, r3\n\
    ADD r4, r4, r5\n\
    ADD r2, r2, r4\n\
    ADD r6, r6, r2\n\
    ENDLOOP\n\
    ST [tid], r6\n";

fn occupancy() -> Task {
    Task {
        title: Tr::new("Packed Full", "占满"),
        brief: Tr::new(
            "Yinpo's wind tower reads the gale on eight stacked pressure planes. rom[0] is n, rom[1] is the stride between the planes, and the readings start at rom[32]: plane k, position i lives at rom[32 + i + k·stride].\nFor every position i < n, out[i] is the sum of its eight readings, one from each plane.\nThe old kernel is right, but each thread hoards registers and the SM can keep only a few warps in flight. Make it fit more of them. Launched as blocks of 64 threads.",
            "银婆的风塔把风压读在八层叠起的压力板上。rom[0] 是 n，rom[1] 是层与层之间的间隔 stride，读数从 rom[32] 开始：第 k 层第 i 个位置在 rom[32 + i + k·stride]。\n对每个位置 i < n，out[i] 是它八层读数之和。\n旧内核结果是对的，可每个线程都囤着寄存器，一个 SM 里同时能留住的 warp 很少。让它能容下更多。按每块 64 个线程发射。",
        ),
        cases: || {
            vec![
                planes(1201, 4096, 4096),
                planes(1202, 8192, 8192),
                planes(1203, 16384, 16384),
            ]
        },
        hidden: || {
            vec![
                planes(1204, 2048, 3072),
                planes(1205, 12288, 12352),
                planes(1206, 16384, 20000),
            ]
        },
        expect: plane_sums,
        grid: Some(|c| Launch::grid((c.rom[0] as usize) / 64, 64)),
        limit: Some(Goal::new(Metric::Cycles, 10000)),
        goals: [Goal::new(Metric::Cycles, 8000), Goal::new(Metric::Regs, 8)],
        starter: Source::Code(OCC_STARTER),
        reference: Source::Code(OCC_REF),
        predict: Predict {
            question: Tr::new(
                "An SM holds 256 register slots per lane. A kernel using 8 registers per thread keeps 32 warps resident. If it grows to 16 registers per thread, how many warps stay resident?",
                "一个 SM 每条车道有 256 个寄存器槽。每线程用 8 个寄存器的内核能留住 32 个 warp。如果涨到每线程 16 个寄存器，还能留住几个 warp？",
            ),
            options: [Tr::new("32", "32"), Tr::new("16", "16"), Tr::new("8", "8")],
            answer: 1,
            why: Tr::new(
                "256 / 16 = 16. Registers are shared out of one fixed pool, so doubling the use per thread halves the warps that fit. Fewer warps means fewer loads in flight to cover the 400-cycle wait.",
                "256 / 16 = 16。寄存器从同一个固定的池子里分，每线程多用一倍，能装下的 warp 就少一半。warp 少了，在途的读取就少，盖不住 400 拍的等待。",
            ),
        },
        hints: [
            Tr::new(
                "Read the Regs number on the result card, then divide 256 by it. That is how many warps an SM keeps; 32 is the ceiling.",
                "看结果卡上的 Regs，再用 256 去除。得数就是 SM 能留住的 warp 数，32 是上限。",
            ),
            Tr::new(
                "The starter keeps eight addresses and four loaded values alive at once. A single running address that you step by the stride needs far fewer registers.",
                "旧内核同时留着八个地址和四个读到的值。只留一个地址、每次加一个 stride 往前走，要的寄存器少得多。",
            ),
            Tr::new(
                "Wrap four loads and their sum in LOOP 2, advancing one address register with ADD r0, r0, stride after each LD. Do not go down to one load per trip: with too few loads in flight the wait is no longer hidden.",
                "把四次 LD 和求和放进 LOOP 2，每次 LD 之后用 ADD r0, r0, stride 推进同一个地址寄存器。别缩到每趟只读一次：在途的读取太少，等待就盖不住了。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Registers decide who can stay", "寄存器决定谁能留下"),
            line: Tr::new(
                "Every resident warp holds its registers for its whole life. The fewer a thread uses, the more warps fit on an SM, and the more waiting one warp can hide behind another.",
                "每个驻留的 warp 在整个生命周期里都占着自己的寄存器。线程用得越少，SM 上装得下的 warp 越多，一个 warp 的等待就越有别的 warp 来遮住。",
            ),
            reality: Tr::new(
                "A real SM has a 64K-register file shared by all resident threads; the compiler's register count per thread sets occupancy directly. Tools such as the CUDA occupancy calculator exist because this arithmetic matters so often.",
                "真实的 SM 有一个 64K 个寄存器的寄存器堆，由所有驻留线程分享；编译器给每线程分配的寄存器数直接决定占用率。CUDA 有专门的占用率计算器，就因为这笔账太常用。",
            ),
            myth: Tr::new(
                "Myth: more unrolling and more registers is always faster. Past the point where the memory system is busy, extra registers only cost you resident warps.",
                "误解：多展开、多用寄存器总是更快。一旦内存系统已经忙起来，多出来的寄存器只会让你少留住几个 warp。",
            ),
            simplified: Tr::new(
                "OC-1 has 256 slots per lane and at most 16 registers per thread, and ignores allocation granularity. Real hardware rounds register counts up in chunks.",
                "OC-1 每条车道 256 个槽，每线程最多 16 个寄存器，也不考虑分配粒度。真实硬件会把寄存器数按块向上取整。",
            ),
        },
        ..base("occupancy", 1, Launch::grid(64, 64), 16384, ATLAS_OPS)
    }
}

// ---------------------------------------------------------------- 12-2

/// rom[0] = n, rom[1] = window radius R (4 or 8), readings from rom[32].
fn series(seed: u64, n: usize, r: usize) -> Case {
    let mut rom = vec![0; 32];
    rom[0] = n as i32;
    rom[1] = r as i32;
    rom.extend(values(seed, n, 0, 100));
    case(rom, 0)
}

fn window_sums(c: &Case) -> Vec<Option<i32>> {
    let (n, r) = (c.rom[0] as i64, c.rom[1] as i64);
    (0..4096i64)
        .map(|i| {
            Some(if i < n {
                (i - r..=i + r)
                    .filter(|j| (0..n).contains(j))
                    .map(|j| c.rom[32 + j as usize])
                    .sum()
            } else {
                0
            })
        })
        .collect()
}

/// Correct, but every element sits on its own 18-word row: 4900 words per block.
const SMEM_STARTER: &str = ".smem 4900 ; a roomy layout: one 18-word row per reading | 宽松的布局：每个读数占 18 个字的一行\n\
    LD r1, [0]\n\
    LD r2, [1]\n\
    MUL r3, bx, bdx\n\
    ADD r3, r3, tx\n\
    SUB r4, r3, r2\n\
    ADD r4, r4, 32\n\
    LD r5, [r4]\n\
    MUL r6, tx, 18\n\
    STS [r6], r5\n\
    SHL r6, r2, 1\n\
    SETP.LT p0, tx, r6\n\
    IF p0\n\
    ADD r4, r4, 256\n\
    LD r5, [r4]\n\
    ADD r6, tx, 256\n\
    MUL r6, r6, 18\n\
    STS [r6], r5\n\
    ENDIF\n\
    BAR\n\
    MUL r6, tx, 18\n\
    LOOP 9\n\
    LDS r5, [r6]\n\
    ADD r6, r6, 18\n\
    ADD r7, r7, r5\n\
    ENDLOOP\n\
    SETP.EQ p1, r2, 8\n\
    IF p1\n\
    LOOP 8\n\
    LDS r5, [r6]\n\
    ADD r6, r6, 18\n\
    ADD r7, r7, r5\n\
    ENDLOOP\n\
    ENDIF\n\
    SETP.LT p2, r3, r1\n\
    IF p2\n\
    ST [r3], r7\n\
    ENDIF\n";

const SMEM_REF: &str = ".smem 272\n\
    LD r1, [0]\n\
    LD r2, [1]\n\
    MUL r3, bx, bdx\n\
    ADD r3, r3, tx\n\
    SUB r4, r3, r2\n\
    ADD r4, r4, 32\n\
    LD r5, [r4]\n\
    STS [tx], r5\n\
    SHL r6, r2, 1\n\
    SETP.LT p0, tx, r6\n\
    IF p0\n\
    ADD r4, r4, 256\n\
    LD r5, [r4]\n\
    ADD r6, tx, 256\n\
    STS [r6], r5\n\
    ENDIF\n\
    BAR\n\
    MOV r6, tx\n\
    LOOP 9\n\
    LDS r5, [r6]\n\
    ADD r6, r6, 1\n\
    ADD r7, r7, r5\n\
    ENDLOOP\n\
    SETP.EQ p1, r2, 8\n\
    IF p1\n\
    LOOP 8\n\
    LDS r5, [r6]\n\
    ADD r6, r6, 1\n\
    ADD r7, r7, r5\n\
    ENDLOOP\n\
    ENDIF\n\
    SETP.LT p2, r3, r1\n\
    IF p2\n\
    ST [r3], r7\n\
    ENDIF\n";

const SMEM_OPS: &[OpKind] = &[
    OpKind::Mov,
    OpKind::Alu(crate::isa::Alu::Add),
    OpKind::Alu(crate::isa::Alu::Sub),
    OpKind::Alu(crate::isa::Alu::Mul),
    OpKind::Alu(crate::isa::Alu::Shl),
    OpKind::SetP,
    OpKind::If,
    OpKind::EndIf,
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::Ld,
    OpKind::St,
    OpKind::Lds,
    OpKind::Sts,
    OpKind::Bar,
];

fn smem_budget() -> Task {
    Task {
        title: Tr::new("Shared Budget", "共享内存账"),
        brief: Tr::new(
            "Yinpo wants the wind smoothed. rom[0] is n, rom[1] is the radius R (4 or 8), and the n gale readings follow from rom[32]. Anything outside the list counts as 0.\nFor every position i < n, out[i] is the sum of the readings from i - R to i + R. Blocks have 256 threads; cells past n must stay 0.\nThe old kernel stages each block's readings in shared memory, and the answers are right, but it reserves a huge slab per block. The SM can only seat the blocks whose slabs fit in its 8192 words.",
            "银婆想把风读数磨平。rom[0] 是 n，rom[1] 是半径 R（4 或 8），n 个风读数从 rom[32] 开始。列表之外的位置按 0 算。\n对每个位置 i < n，out[i] 是从 i - R 到 i + R 的读数之和。每块 256 个线程；n 之后的格子必须保持 0。\n旧内核把每块的读数先放进共享内存，答案是对的，可每块都预订了一大块地方。SM 只能安排下“地方加起来不超过 8192 个字”的那些块。",
        ),
        cases: || {
            vec![
                series(1211, 4096, 4),
                series(1212, 4096, 8),
                series(1213, 2048, 4),
            ]
        },
        hidden: || {
            vec![
                series(1214, 3000, 8),
                series(1215, 1777, 8),
                series(1216, 4096, 8),
            ]
        },
        expect: window_sums,
        grid: Some(|c| Launch::grid((c.rom[0] as usize).div_ceil(256), 256)),
        limit: Some(Goal::new(Metric::Cycles, 5000)),
        goals: [
            Goal::new(Metric::Cycles, 4000),
            Goal::new(Metric::Smem, 272),
        ],
        starter: Source::Code(SMEM_STARTER),
        reference: Source::Code(SMEM_REF),
        predict: Predict {
            question: Tr::new(
                "An SM has 8192 shared words. The old kernel declares .smem 4900 for each block. How many of its blocks can sit on one SM at the same time?",
                "一个 SM 有 8192 个共享字。旧内核每块声明 .smem 4900。同一时刻一个 SM 上能放几个它的块？",
            ),
            options: [
                Tr::new("1", "1 个"),
                Tr::new("2", "2 个"),
                Tr::new("3", "3 个"),
            ],
            answer: 0,
            why: Tr::new(
                "8192 / 4900 = 1.67, and a block cannot be cut in pieces, so only one fits. The other blocks wait in the queue until it ends, even though the SM has warps and registers to spare.",
                "8192 / 4900 = 1.67，块不能切开，所以只放得下一个。别的块只能排队等它结束，哪怕 SM 的 warp 和寄存器还有富余。",
            ),
        },
        hints: [
            Tr::new(
                "Smem on the result card is what each block reserves, not what it touches. Blocks per SM = 8192 divided by that, rounded down.",
                "结果卡上的 Smem 是每块预订的量，不是实际碰到的量。每个 SM 的块数 = 8192 除以它，向下取整。",
            ),
            Tr::new(
                "A block needs its 256 readings plus R extra on each side: 256 + 2R words, at most 272. The readings can sit side by side, one word each.",
                "一块需要自己的 256 个读数，再加两边各 R 个：256 + 2R 个字，最多 272。读数可以一个挨一个地放，每个只占一个字。",
            ),
            Tr::new(
                "Write .smem 272, store the tile at tx (and the 2R extra words at 256 + tx), BAR, then walk 2R + 1 words from tx with LDS.",
                "写 .smem 272，把读数放在 tx 位置（多出的 2R 个放在 256 + tx），BAR，然后从 tx 起用 LDS 走 2R + 1 个字。",
            ),
        ],
        insight: Insight {
            title: Tr::new(
                "Shared memory is a budget, not a free lunch",
                "共享内存是一笔预算",
            ),
            line: Tr::new(
                "A block books its shared words when it lands and keeps them until it ends. Booking less lets more blocks live together, and their waiting overlaps.",
                "块一落地就订下它的共享字，直到结束才还。订得少，就有更多块能同时住下，它们的等待也就能叠在一起。",
            ),
            reality: Tr::new(
                "A real SM has about 48 to 228 KB of shared memory depending on the generation. Kernels that stage big tiles often trade occupancy for reuse, and the best tile size is found by measuring.",
                "真实的 SM 依代际不同有大约 48 到 228 KB 的共享内存。搬大块数据的内核常常拿占用率换复用，最佳的块大小要靠实测。",
            ),
            myth: Tr::new(
                "Myth: a bigger tile is always better. Past the point where the data fits, a bigger booking only evicts neighbouring blocks.",
                "误解：块越大越好。数据装得下之后，多订的地方只会把邻居块挤出去。",
            ),
            simplified: Tr::new(
                "OC-1 books exactly what .smem says and has no allocation granularity or carve-out between L1 and shared memory.",
                "OC-1 严格按 .smem 订位，没有分配粒度，也没有 L1 与共享内存之间的划分。",
            ),
        },
        ..base("smem-budget", 2, Launch::grid(16, 256), 4096, SMEM_OPS)
    }
}

// ---------------------------------------------------------------- 12-3

/// rom[0] = n, rom[1] = distance between consecutive arrays, rom[2] = pairs (2
/// or 3); array k starts at rom[32 + k * stride].
fn pairs(seed: u64, n: usize, stride: usize, p: usize) -> Case {
    let mut rom = vec![0; 32];
    rom[0] = n as i32;
    rom[1] = stride as i32;
    rom[2] = p as i32;
    rom.extend(values(seed, 2 * p * stride, 0, 30));
    case(rom, 0)
}

fn pair_dots(c: &Case) -> Vec<Option<i32>> {
    let (n, m, p) = (c.rom[0] as usize, c.rom[1] as usize, c.rom[2] as usize);
    (0..4096)
        .map(|i| {
            Some(if i < n {
                (0..p)
                    .map(|k| c.rom[32 + 2 * k * m + i] * c.rom[32 + (2 * k + 1) * m + i])
                    .sum()
            } else {
                0
            })
        })
        .collect()
}

/// Each product waits for its own two reads before the next pair is even asked for.
const SCORE_STARTER: &str = "; one pair at a time: the multiply waits, then the next reads start | 一对一对地来：乘法在等，等完才发下一对的读\n\
    LD r1, [0]\n\
    LD r2, [1]\n\
    LD r3, [2]\n\
    ADD r0, tid, 32\n\
    SETP.LT p1, tid, r1\n\
    SETP.EQ p0, r3, 3\n\
    LD r4, [r0]\n\
    ADD r0, r0, r2\n\
    LD r5, [r0]\n\
    ADD r0, r0, r2\n\
    MUL r6, r4, r5\n\
    LD r4, [r0]\n\
    ADD r0, r0, r2\n\
    LD r5, [r0]\n\
    ADD r0, r0, r2\n\
    MUL r7, r4, r5\n\
    ADD r6, r6, r7\n\
    IF p0\n\
    LD r4, [r0]\n\
    ADD r0, r0, r2\n\
    LD r5, [r0]\n\
    MUL r7, r4, r5\n\
    ADD r6, r6, r7\n\
    ENDIF\n\
    IF p1\n\
    ST [tid], r6\n\
    ENDIF\n";

const SCORE_REF: &str = "LD r1, [0]\n\
    LD r2, [1]\n\
    LD r3, [2]\n\
    ADD r0, tid, 32\n\
    SETP.LT p1, tid, r1\n\
    SETP.EQ p0, r3, 3\n\
    LD r1, [r0]\n\
    ADD r0, r0, r2\n\
    LD r3, [r0]\n\
    ADD r0, r0, r2\n\
    LD r4, [r0]\n\
    ADD r0, r0, r2\n\
    LD r5, [r0]\n\
    IF p0\n\
    ADD r0, r0, r2\n\
    LD r6, [r0]\n\
    ADD r0, r0, r2\n\
    LD r7, [r0]\n\
    ENDIF\n\
    MUL r1, r1, r3\n\
    MUL r4, r4, r5\n\
    ADD r1, r1, r4\n\
    IF p0\n\
    MUL r6, r6, r7\n\
    ADD r1, r1, r6\n\
    ENDIF\n\
    IF p1\n\
    ST [tid], r1\n\
    ENDIF\n";

const SCORE_OPS: &[OpKind] = &[
    OpKind::Mov,
    OpKind::Alu(crate::isa::Alu::Add),
    OpKind::Alu(crate::isa::Alu::Sub),
    OpKind::Alu(crate::isa::Alu::Mul),
    OpKind::SetP,
    OpKind::If,
    OpKind::EndIf,
    OpKind::Ld,
    OpKind::St,
];

fn scoreboard() -> Task {
    Task {
        title: Tr::new("Ask First, Wait Once", "先问再等"),
        brief: Tr::new(
            "At each position the wind tower has pairs of sensors: pressure and area, tension and length. rom[0] is n, rom[1] the stride between consecutive arrays, rom[2] the number of pairs P (2 or 3). Array k starts at rom[32 + k·stride]; arrays 2j and 2j+1 form pair j.\nFor every position i < n: out[i] = a₀[i]·a₁[i] + a₂[i]·a₃[i] (+ a₄[i]·a₅[i] when P is 3). Blocks have 64 threads.\nThe old kernel asks for one pair, waits for it, then asks for the next. Yinpo says the tower spends its life waiting.",
            "风塔每个位置上有几对传感器：压强和面积，张力和长度。rom[0] 是 n，rom[1] 是相邻两个数组的间隔，rom[2] 是对数 P（2 或 3）。第 k 个数组从 rom[32 + k·stride] 开始；数组 2j 与 2j+1 组成第 j 对。\n对每个位置 i < n：out[i] = a₀[i]·a₁[i] + a₂[i]·a₃[i]（P 为 3 时再加 a₄[i]·a₅[i]）。每块 64 个线程。\n旧内核问一对、等一对，再问下一对。银婆说风塔一辈子都花在等上了。",
        ),
        cases: || {
            vec![
                pairs(1221, 1024, 1024, 2),
                pairs(1222, 2048, 2048, 2),
                pairs(1223, 4096, 4096, 2),
            ]
        },
        hidden: || {
            vec![
                pairs(1224, 2048, 2112, 3),
                pairs(1225, 1000, 1056, 2),
                pairs(1226, 2999, 3072, 3),
            ]
        },
        expect: pair_dots,
        grid: Some(|c| Launch::grid((c.rom[0] as usize).div_ceil(64), 64)),
        limit: Some(Goal::new(Metric::Cycles, 1650)),
        goals: [Goal::new(Metric::Cycles, 1500), Goal::new(Metric::Size, 30)],
        starter: Source::Code(SCORE_STARTER),
        reference: Source::Code(SCORE_REF),
        predict: Predict {
            question: Tr::new(
                "Eight warps each issue one LD to a different line (a 400-cycle miss) and then one ADD that uses the result. About how many cycles until all eight are done?",
                "八个 warp 各发一条读不同行的 LD（未命中要 400 拍），再各做一条用到结果的 ADD。大约多少拍后八个都做完？",
            ),
            options: [
                Tr::new("About 8 × 400 = 3200", "约 8 × 400 = 3200"),
                Tr::new("About 400 plus a few dozen", "约 400 再多几十"),
                Tr::new("About 200", "约 200"),
            ],
            answer: 1,
            why: Tr::new(
                "A warp only stalls on the register it needs. While one warp waits, the others issue their loads, so all eight misses are in flight together. The memory channel starts a new fill every 4 cycles, hence 400 plus a few dozen, not 3200.",
                "warp 只会在它要用的那个寄存器上停下。一个 warp 在等的时候，别的 warp 照样发出自己的读，八次未命中是同时在途的。内存通道每 4 拍开始一次新的填充，所以是 400 再多几十，而不是 3200。",
            ),
        },
        hints: [
            Tr::new(
                "A warp does not stall on LD itself, only on the first instruction that reads the loaded register. Which instruction in the starter is the first to wait?",
                "warp 不会因为 LD 本身停下，只会在第一条读到该寄存器的指令上停下。旧内核里是哪一条最先在等？",
            ),
            Tr::new(
                "Issue every LD first, each into its own register, and only then multiply. The whole group then costs one wait instead of one per pair.",
                "先把所有 LD 发出去，各读进自己的寄存器，然后再乘。整组只付一次等待，而不是每对一次。",
            ),
            Tr::new(
                "Compute the six-or-four addresses with a running register (ADD r0, r0, stride), put the third pair's two LDs inside IF p0 (P = 3) right after the others, and do all MULs and ADDs after the last LD.",
                "用一个往前走的寄存器算地址（ADD r0, r0, stride）；第三对的两条 LD 放在 IF p0（P = 3）里，紧跟在其他 LD 之后；所有 MUL 和 ADD 都放到最后一条 LD 之后。",
            ),
        ],
        insight: Insight {
            title: Tr::new(
                "The scoreboard: ask early, wait once",
                "计分板：早问，只等一次",
            ),
            line: Tr::new(
                "Loads do not block the warp. Only a read of the pending register does. So put independent loads next to each other and the waits overlap.",
                "读取不会让 warp 停住，只有读取尚未到货的寄存器才会。把互不相干的读排在一起，等待就叠在一起了。",
            ),
            reality: Tr::new(
                "GPUs track pending registers with a scoreboard and let the warp run ahead until it needs a result. Compilers hoist loads to the top of a block for exactly this reason.",
                "GPU 用计分板记录还没到货的寄存器，让 warp 一直跑到真正需要结果为止。编译器把读取提到块的最前面，正是为了这个。",
            ),
            myth: Tr::new(
                "Myth: the order of instructions does not matter because the hardware sorts it out. A warp issues in order; only different warps are shuffled.",
                "误解：指令顺序无所谓，硬件会自己理顺。一个 warp 是按顺序发射的，被打乱的只有 warp 之间的顺序。",
            ),
            simplified: Tr::new(
                "OC-1 has one fixed latency per memory level and no limit on loads in flight per warp. Real hardware caps outstanding requests.",
                "OC-1 每层内存只有一个固定延迟，也不限制每个 warp 在途的读取数。真实硬件会限制未完成请求的数量。",
            ),
        },
        ..base("the-scoreboard", 3, Launch::grid(64, 64), 4096, SCORE_OPS)
    }
}

// ---------------------------------------------------------------- 12-4

/// rom[0] = n, rom[1] = stride between the twelve planes, planes from rom[32].
fn twelve(seed: u64, n: usize, stride: usize) -> Case {
    let mut rom = vec![0; 32];
    rom[0] = n as i32;
    rom[1] = stride as i32;
    rom.extend(values(seed, 12 * stride, 0, 20));
    case(rom, 0)
}

/// out[i] = sum over the twelve planes of x * x + x.
fn square_sums(c: &Case) -> Vec<Option<i32>> {
    let (n, s) = (c.rom[0] as usize, c.rom[1] as usize);
    (0..4096)
        .map(|i| {
            Some(if i < n {
                (0..12)
                    .map(|k| c.rom[32 + k * s + i])
                    .map(|x| x * x + x)
                    .sum()
            } else {
                0
            })
        })
        .collect()
}

/// Twelve partial results, parked in local memory until the merge.
const SPILL_STARTER: &str = "; twelve partial results do not fit in 8 registers: they go to local memory | 十二个部分结果装不进 8 个寄存器：只好放进本地内存\n\
    LD r1, [1]\n\
    LD r6, [0]\n\
    ADD r0, tid, 32\n\
    SETP.LT p0, tid, r6\n\
    MOV r5, 0\n\
    LOOP 12\n\
    LD r2, [r0]\n\
    ADD r0, r0, r1\n\
    MUL r3, r2, r2\n\
    ADD r3, r3, r2\n\
    STL [r5], r3\n\
    ADD r5, r5, 1\n\
    ENDLOOP\n\
    MOV r5, 0\n\
    MOV r4, 0\n\
    LOOP 12\n\
    LDL r2, [r5]\n\
    ADD r4, r4, r2\n\
    ADD r5, r5, 1\n\
    ENDLOOP\n\
    IF p0\n\
    ST [tid], r4\n\
    ENDIF\n";

const SPILL_REF: &str = "LD r1, [1]\n\
    LD r2, [0]\n\
    ADD r0, tid, 32\n\
    SETP.LT p0, tid, r2\n\
    LOOP 3\n\
    LD r2, [r0]\n\
    ADD r0, r0, r1\n\
    LD r3, [r0]\n\
    ADD r0, r0, r1\n\
    LD r4, [r0]\n\
    ADD r0, r0, r1\n\
    LD r5, [r0]\n\
    ADD r0, r0, r1\n\
    ADD r6, r6, r2\n\
    MUL r2, r2, r2\n\
    ADD r6, r6, r2\n\
    ADD r6, r6, r3\n\
    MUL r3, r3, r3\n\
    ADD r6, r6, r3\n\
    ADD r6, r6, r4\n\
    MUL r4, r4, r4\n\
    ADD r6, r6, r4\n\
    ADD r6, r6, r5\n\
    MUL r5, r5, r5\n\
    ADD r6, r6, r5\n\
    ENDLOOP\n\
    IF p0\n\
    ST [tid], r6\n\
    ENDIF\n";

const SPILL_OPS: &[OpKind] = &[
    OpKind::Mov,
    OpKind::Alu(crate::isa::Alu::Add),
    OpKind::Alu(crate::isa::Alu::Sub),
    OpKind::Alu(crate::isa::Alu::Mul),
    OpKind::SetP,
    OpKind::If,
    OpKind::EndIf,
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::Ld,
    OpKind::St,
    OpKind::Ldl,
    OpKind::Stl,
];

fn spill() -> Task {
    Task {
        title: Tr::new("Out to Local", "溢出到本地"),
        brief: Tr::new(
            "The tower keeps twelve gale planes. rom[0] is n, rom[1] the stride between the planes, and plane k, position i is at rom[32 + k·stride + i]. For each position i < n, out[i] is the sum over the twelve planes of x·x + x.\nOn this machine a thread may use at most 8 registers (a program that needs more is refused). Extra values can be parked in the thread's own local memory with STL and fetched back with LDL, but a local read takes 80 cycles.\nThe old kernel is right, yet Yinpo's tower crawls. Blocks of 64 threads. Find where the time goes, then keep everything in registers.",
            "塔上有十二层风压板。rom[0] 是 n，rom[1] 是层间间隔，第 k 层第 i 个位置在 rom[32 + k·stride + i]。对每个位置 i < n，out[i] 是十二层上 x·x + x 的总和。\n这台机器上每个线程最多只能用 8 个寄存器（需要更多的程序会被拒绝）。多出来的值可以用 STL 放进线程自己的本地内存，再用 LDL 取回，但读一次本地内存要 80 拍。\n旧内核结果是对的，银婆的塔却慢得像在爬。每块 64 个线程。找出时间花在哪里，然后把一切都留在寄存器里。",
        ),
        cases: || vec![twelve(1231, 4096, 4096), twelve(1232, 2048, 2048)],
        hidden: || {
            vec![
                twelve(1233, 3000, 3008),
                twelve(1234, 4096, 4160),
                twelve(1235, 1000, 1024),
            ]
        },
        expect: square_sums,
        grid: Some(|c| Launch::grid((c.rom[0] as usize).div_ceil(64), 64)),
        limit: Some(Goal::new(Metric::Cycles, 3300)),
        goals: [Goal::new(Metric::Cycles, 3100), Goal::new(Metric::Regs, 7)],
        reg_cap: Some(8),
        starter: Source::Code(SPILL_STARTER),
        reference: Source::Code(SPILL_REF),
        clue: Some(Clue {
            what: Tr::new(
                "the first value sent out to local memory because the registers ran out",
                "第一个因为寄存器不够而被送出去的值",
            ),
            find: crate::grade::first_spill,
        }),
        predict: Predict {
            question: Tr::new(
                "A thread may hold 8 registers and its code needs a 9th live value. Where can that value go?",
                "一个线程只能用 8 个寄存器，而代码需要同时保留第 9 个值。这个值能放到哪里？",
            ),
            options: [
                Tr::new(
                    "Nowhere: the kernel simply fails",
                    "哪里也放不了：内核直接失败",
                ),
                Tr::new(
                    "Into local memory, 80 cycles to get back",
                    "放进本地内存，取回要 80 拍",
                ),
                Tr::new(
                    "Into a neighbouring lane's register",
                    "放进相邻车道的寄存器",
                ),
            ],
            answer: 1,
            why: Tr::new(
                "Each thread has 64 private words of local memory. STL parks a value there and LDL brings it back after 80 cycles, with no cache in between. It works, but every trip is a wait the other warps must cover.",
                "每个线程有 64 个私有的本地内存字。STL 把值放进去，LDL 过 80 拍把它取回，中间没有缓存。能用，但每一趟都是要别的 warp 去盖的等待。",
            ),
        },
        hints: [
            Tr::new(
                "Look at the Warp River at the first spill: which register value is being parked, and how soon is it needed again? The starter also waits for each load before asking for the next.",
                "在 Warp River 上看第一次溢出：被放走的是哪个寄存器的值，多久之后又要用它？旧内核还要等每次读完才发下一次。",
            ),
            Tr::new(
                "Nothing needs to be kept for the merge: the result is just a sum. Add each x·x + x into one running total as soon as it arrives.",
                "合并时什么都不必留着：结果只是一个总和。每个 x·x + x 一到手就加进同一个累加器。",
            ),
            Tr::new(
                "Load four planes at once into four registers, fold them into the total, and go round LOOP 3 times. That is 4 values + address + stride + total = 7 registers.",
                "一次把四层读进四个寄存器，折进总和，LOOP 3 次。4 个值 + 地址 + 间隔 + 总和 = 7 个寄存器。",
            ),
        ],
        insight: Insight {
            title: Tr::new("When the registers run out", "寄存器不够用的时候"),
            line: Tr::new(
                "Values that do not fit in registers are spilled to local memory, which is slow, and every spill is also a lost chance to keep loads in flight.",
                "放不进寄存器的值会溢出到本地内存，那里很慢；每一次溢出也让你少了一次让读取并行在途的机会。",
            ),
            reality: Tr::new(
                "Real GPUs spill to local memory that lives in the global memory system (cached in L1 and L2). Compilers report spill counts, and shaving live values is a standard way to recover performance.",
                "真实的 GPU 会把值溢出到本地内存，它实际位于全局内存系统里（经过 L1 与 L2 缓存）。编译器会报告溢出数，减少活跃值是找回性能的标准手段。",
            ),
            myth: Tr::new(
                "Myth: unrolling more always helps. Past the register budget, more unrolling only makes more spills.",
                "误解：多展开几次总有好处。超过寄存器预算之后，多展开只会多出溢出。",
            ),
            simplified: Tr::new(
                "OC-1 has a hard cap per task and an 80-cycle local read with no cache. Real spills are cached and the compiler chooses what to spill.",
                "OC-1 对每关设了硬上限，本地读固定 80 拍、没有缓存。真实的溢出会被缓存，而且由编译器决定溢出什么。",
            ),
        },
        ..base("the-spill", 4, Launch::grid(64, 64), 4096, SPILL_OPS)
    }
}

// ---------------------------------------------------------------- 12-5

/// rom[0] = n, values from rom[32].
fn heap(seed: u64, n: usize) -> Case {
    let mut rom = vec![0; 32];
    rom[0] = n as i32;
    rom.extend(values(seed, n, 0, 100));
    case(rom, 0)
}

fn total(c: &Case) -> Vec<Option<i32>> {
    let n = c.rom[0] as usize;
    let mut out = vec![Some(0); 8];
    out[0] = Some(c.rom[32..32 + n].iter().sum());
    out
}

/// The right shape with the wrong block size: each thread sums four readings, a butterfly runs in
/// every warp, one word per warp goes to shared memory, and the first warp adds those up and
/// issues one ATOM per block.
const BLOCK_STARTER: &str = ".block 32 ; the block size is yours to choose | 块多大由你来选\n\
    .smem 16\n\
    MUL r1, gdx, bdx\n\
    ADD r0, tid, 32\n\
    LD r2, [r0]\n\
    ADD r0, r0, r1\n\
    LD r3, [r0]\n\
    ADD r0, r0, r1\n\
    LD r4, [r0]\n\
    ADD r0, r0, r1\n\
    LD r5, [r0]\n\
    ADD r2, r2, r3\n\
    ADD r4, r4, r5\n\
    ADD r2, r2, r4\n\
    MOV r3, 16\n\
    LOOP 5\n\
    XOR r4, lane, r3\n\
    SHFL r5, r2, r4\n\
    ADD r2, r2, r5\n\
    SHR r3, r3, 1\n\
    ENDLOOP\n\
    SHR r6, tx, 5\n\
    SETP.EQ p1, lane, 0\n\
    IF p1\n\
    STS [r6], r2\n\
    ENDIF\n\
    BAR\n\
    SETP.LT p2, tx, 32\n\
    IF p2\n\
    LDS r2, [lane]\n\
    MOV r3, 16\n\
    LOOP 5\n\
    XOR r4, lane, r3\n\
    SHFL r5, r2, r4\n\
    ADD r2, r2, r5\n\
    SHR r3, r3, 1\n\
    ENDLOOP\n\
    IF p1\n\
    ATOM r4, [0], r2\n\
    ENDIF\n\
    ENDIF\n";

const BLOCK_REF: &str = ".block 256\n\
    .smem 16\n\
    MUL r1, gdx, bdx\n\
    ADD r0, tid, 32\n\
    LD r2, [r0]\n\
    ADD r0, r0, r1\n\
    LD r3, [r0]\n\
    ADD r0, r0, r1\n\
    LD r4, [r0]\n\
    ADD r0, r0, r1\n\
    LD r5, [r0]\n\
    ADD r2, r2, r3\n\
    ADD r4, r4, r5\n\
    ADD r2, r2, r4\n\
    MOV r3, 16\n\
    LOOP 5\n\
    XOR r4, lane, r3\n\
    SHFL r5, r2, r4\n\
    ADD r2, r2, r5\n\
    SHR r3, r3, 1\n\
    ENDLOOP\n\
    SHR r6, tx, 5\n\
    SETP.EQ p1, lane, 0\n\
    IF p1\n\
    STS [r6], r2\n\
    ENDIF\n\
    BAR\n\
    SETP.LT p2, tx, 32\n\
    IF p2\n\
    LDS r2, [lane]\n\
    MOV r3, 16\n\
    LOOP 5\n\
    XOR r4, lane, r3\n\
    SHFL r5, r2, r4\n\
    ADD r2, r2, r5\n\
    SHR r3, r3, 1\n\
    ENDLOOP\n\
    IF p1\n\
    ATOM r4, [0], r2\n\
    ENDIF\n\
    ENDIF\n";

const BLOCK_OPS: &[OpKind] = &[
    OpKind::Mov,
    OpKind::Alu(crate::isa::Alu::Add),
    OpKind::Alu(crate::isa::Alu::Sub),
    OpKind::Alu(crate::isa::Alu::Mul),
    OpKind::Alu(crate::isa::Alu::Xor),
    OpKind::Alu(crate::isa::Alu::Shr),
    OpKind::SetP,
    OpKind::If,
    OpKind::EndIf,
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::Ld,
    OpKind::Lds,
    OpKind::Sts,
    OpKind::Bar,
    OpKind::Shfl,
    OpKind::Atom,
];

fn blocksize() -> Task {
    Task {
        title: Tr::new("How Big a Block", "块有多大"),
        brief: Tr::new(
            "Yinpo wants the day's total wind pressure from the tower. rom[0] is n and the n readings follow from rom[32]. Add them all and leave the sum in out[0]; the other cells stay 0.\nThe kernel below is correct: each thread sums four readings, each warp folds its sum with SHFL, the warps of a block meet in shared memory, and one ATOM per block adds the block's sum to out[0]. The program starts with .block 32. This level lets you choose the block size with a .block line (32 to 512, a multiple of 32); the machine launches just enough blocks of that size to cover all the threads. Find the size that makes the tower fastest on every day.",
            "银婆想知道塔上一天的风压总和。rom[0] 是 n，n 个读数从 rom[32] 开始。把它们全部加起来，和放进 out[0]，其余格子保持 0。\n下面这个内核是对的：每个线程加四个读数，每个 warp 用 SHFL 折叠自己的和，块里的各个 warp 在共享内存里碰头，每块用一次 ATOM 把本块的和加进 out[0]。程序一开始写着 .block 32。这一关让你用一行 .block（32 到 512，32 的倍数）自己选块大小；机器会按这个大小发出刚好够用的块。找出能让塔在每一天都最快的大小。",
        ),
        cases: || vec![heap(1241, 8192), heap(1242, 4096), heap(1243, 12000)],
        hidden: || vec![heap(1244, 5000), heap(1245, 777), heap(1246, 12000)],
        expect: total,
        grid: Some(|c| Launch::grid((c.rom[0] as usize).div_ceil(4).div_ceil(64), 64)),
        free_block: true,
        limit: Some(Goal::new(Metric::Cycles, 2000)),
        goals: [Goal::new(Metric::Cycles, 1650), Goal::new(Metric::Size, 40)],
        starter: Source::Code(BLOCK_STARTER),
        reference: Source::Code(BLOCK_REF),
        predict: Predict {
            question: Tr::new(
                "The same kernel runs on the busiest day (12000 readings) with blocks of 32, 128 or 512 threads. Which is fastest?",
                "同一个内核在最忙的一天（12000 个读数）分别用 32、128、512 线程的块来跑。哪个最快？",
            ),
            options: [
                Tr::new("32: small blocks fit anywhere", "32：小块哪里都塞得下"),
                Tr::new(
                    "128: not too many blocks, not too few",
                    "128：块不太多，也不太少",
                ),
                Tr::new("512: fewer blocks always wins", "512：块越少越好"),
            ],
            answer: 1,
            why: Tr::new(
                "With 32 there are about a hundred blocks, each with its own BAR and its own ATOM, and the atomic unit queues them. With 512 there are only six blocks for four SMs, so two SMs seat two blocks while the others seat one, and the busiest SM sets the time. 128 has enough blocks to spread evenly and few enough to keep the overhead small.",
                "用 32 时大约有一百个块，每块各有一次 BAR 和一次 ATOM，原子单元要排队处理。用 512 时只有六个块要分给四个 SM，两个 SM 各摆两块、另外两个只摆一块，最忙的那个 SM 决定总时间。128 的块数足够均匀分摊，又少到开销不大。",
            ),
        },
        hints: [
            Tr::new(
                "Time the same program with .block 32, 128 and 512 and compare the Cycles on the busiest day. Then look at the Warp River of a small block size: how many warps are doing nothing but waiting at BAR or for the atomic unit?",
                "同一个程序分别用 .block 32、128、512 跑一遍，比较最忙那天的拍数。再看小块的 Warp River：有多少 warp 除了在 BAR 或原子单元前等着，什么也没做？",
            ),
            Tr::new(
                "Small blocks pay the per-block cost (the BAR and the ATOM) many times. Huge blocks leave whole SMs with less to do when the number of blocks does not divide evenly among the four SMs.",
                "小块要把每块的固定开销（BAR 和 ATOM）付很多次。大块则会在块数无法均分给四个 SM 时，让有的 SM 分到的活少得多。",
            ),
            Tr::new(
                "Try 64, 128 and 256 as well. The best size for the 12000 day is the one that hands every SM the same number of blocks while keeping the per-block costs small.",
                "也试试 64、128 和 256。12000 那天的最佳大小，是让每个 SM 分到同样多的块，同时让每块的固定开销保持很小的那个。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Choosing a block size", "怎么选块大小"),
            line: Tr::new(
                "Blocks that are too small repeat the per-block work; blocks that are too large cannot be spread evenly across the SMs. The best size sits between.",
                "块太小，每块的固定工作要重复很多次；块太大，又没法均匀分给各个 SM。最合适的大小在两者之间。",
            ),
            reality: Tr::new(
                "Real GPUs favour block sizes that are multiples of the warp size, usually 128 to 256, so that the resident-block limit, register and shared memory use, and the number of blocks per SM all stay balanced. Occupancy calculators exist because the answer depends on the kernel.",
                "真实的 GPU 偏爱 warp 大小的整数倍，通常是 128 到 256，让常驻块数上限、寄存器和共享内存用量、每个 SM 上的块数都保持平衡。之所以有占用率计算器，是因为答案取决于内核。",
            ),
            myth: Tr::new(
                "Myth: bigger blocks are always better, or smaller blocks are always more flexible. Both extremes lose something.",
                "误解：块越大越好，或者块越小越灵活。两个极端都会有所损失。",
            ),
            simplified: Tr::new(
                "OC-1 has four SMs and a free choice of 32 to 512. A real GPU has dozens of SMs and a resident-block limit that this machine does not model.",
                "OC-1 只有四个 SM，块大小可在 32 到 512 里随便选。真实的 GPU 有几十个 SM，还有这台机器没有模拟的常驻块数上限。",
            ),
        },
        ..base("the-blocksize", 5, Launch::grid(64, 64), 8, BLOCK_OPS)
    }
}

// ---------------------------------------------------------------- 12-6

/// rom[0] = n, rom[1] = w (rounds are w groups of 6), data from rom[32].
fn sand(seed: u64, n: usize, w: i32) -> Case {
    let mut rom = vec![0; 32];
    rom[0] = n as i32;
    rom[1] = w;
    rom.extend(values(seed, n, 0, 1000));
    case(rom, 0)
}

fn churn(c: &Case) -> Vec<Option<i32>> {
    let n = c.rom[0] as usize;
    let w = c.rom[1];
    let mut out = vec![Some(0); 4096];
    for (i, cell) in out.iter_mut().enumerate().take(n) {
        let mut x = c.rom[32 + i];
        for _ in 0..w * 6 {
            x = x.wrapping_mul(5).wrapping_add(3);
            x ^= x >> 3;
        }
        *cell = Some(x);
    }
    out
}

/// Copy a tile, wait for it, compute it, wait again, next tile: the copy and the compute never overlap.
const DMA_STARTER: &str = "; copy a tile, then compute it, one tile at a time | 搬一块，算一块，一块接一块\n\
    .smem 128\n\
    LD r0, [0]\n\
    LD r1, [1]\n\
    SHL r4, bx, 10\n\
    ADD r5, r4, 32\n\
    SETP.EQ p3, tx, 0\n\
    LOOP 8\n\
    IF p3\n\
    DMA 0, r5, 128\n\
    ENDIF\n\
    BAR\n\
    MOV r9, tx\n\
    LDS r2, [r9]\n\
    MOV r9, 0\n\
    LOOP 4\n\
    SETP.LT p1, r9, r1\n\
    IF p1\n\
    LOOP 6\n\
    MUL r2, r2, 5\n\
    ADD r2, r2, 3\n\
    SHR r3, r2, 3\n\
    XOR r2, r2, r3\n\
    ENDLOOP\n\
    ENDIF\n\
    ADD r9, r9, 1\n\
    ENDLOOP\n\
    ADD r3, r4, tx\n\
    SETP.LT p2, r3, r0\n\
    IF p2\n\
    ST [r3], r2\n\
    ENDIF\n\
    ADD r4, r4, 128\n\
    ADD r5, r5, 128\n\
    BAR\n\
    ENDLOOP\n";

/// Two buffers: the copy of the next tile is issued before the current tile is computed.
const DMA_REF: &str = ".smem 256\n\
    LD r0, [0]\n\
    LD r1, [1]\n\
    SHL r4, bx, 10\n\
    ADD r5, r4, 32\n\
    SETP.EQ p3, tx, 0\n\
    IF p3\n\
    DMA 0, r5, 128\n\
    ENDIF\n\
    MOV r6, 0\n\
    MOV r7, 0\n\
    LOOP 8\n\
    BAR\n\
    XOR r8, r6, 128\n\
    ADD r5, r5, 128\n\
    SETP.LT p0, r7, 7\n\
    IF p0\n\
    IF p3\n\
    DMA r8, r5, 128\n\
    ENDIF\n\
    ENDIF\n\
    ADD r9, r6, tx\n\
    LDS r2, [r9]\n\
    MOV r9, 0\n\
    LOOP 4\n\
    SETP.LT p1, r9, r1\n\
    IF p1\n\
    LOOP 6\n\
    MUL r2, r2, 5\n\
    ADD r2, r2, 3\n\
    SHR r3, r2, 3\n\
    XOR r2, r2, r3\n\
    ENDLOOP\n\
    ENDIF\n\
    ADD r9, r9, 1\n\
    ENDLOOP\n\
    ADD r3, r4, tx\n\
    SETP.LT p2, r3, r0\n\
    IF p2\n\
    ST [r3], r2\n\
    ENDIF\n\
    ADD r4, r4, 128\n\
    MOV r6, r8\n\
    ADD r7, r7, 1\n\
    ENDLOOP\n";

const DMA_OPS: &[OpKind] = &[
    OpKind::Mov,
    OpKind::Alu(crate::isa::Alu::Add),
    OpKind::Alu(crate::isa::Alu::Mul),
    OpKind::Alu(crate::isa::Alu::Xor),
    OpKind::Alu(crate::isa::Alu::Shl),
    OpKind::Alu(crate::isa::Alu::Shr),
    OpKind::SetP,
    OpKind::If,
    OpKind::EndIf,
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::Ld,
    OpKind::St,
    OpKind::Lds,
    OpKind::Bar,
    OpKind::Dma,
];

fn dma() -> Task {
    Task {
        title: Tr::new("The Carrier Engine", "搬运引擎"),
        brief: Tr::new(
            "Yinpo keeps the gale readings in the depot far below the tower. rom[0] is n, rom[1] is w, and the n readings follow from rom[32]. For every reading x, run 6·w rounds of: x = x·5 + 3, then x = x XOR (x >> 3). out[i] is the result for reading i; cells past n stay 0.\nBlocks have 128 threads. Block b takes the 1024 readings from 1024·b on, as eight tiles of 128, one reading per thread. DMA dst, src, count copies count words from the depot, starting at rom[src], into the block's shared memory at dst. It runs in the background and lands about 400 cycles later; BAR waits until the block's copies have landed, and reading the words earlier gives old data.\nThe old kernel copies a tile, waits, computes it, waits, then fetches the next one. It is correct, but it spends the whole day waiting. Keep the copy engine busy while the warps compute.",
            "银婆把风压读数存在塔下很深的仓库里。rom[0] 是 n，rom[1] 是 w，n 个读数从 rom[32] 开始。对每个读数 x，做 6·w 轮：x = x·5 + 3，然后 x = x XOR (x >> 3)。out[i] 是第 i 个读数的结果；n 之后的格子保持 0。\n每块 128 个线程。第 b 块负责从 1024·b 开始的 1024 个读数，分成八个 128 个的小块，每个线程管一个。DMA dst, src, count 把仓库里从 rom[src] 起的 count 个字搬进本块共享内存的 dst 处。它在后台进行，约 400 拍后到货；BAR 会等到本块的搬运都到货，更早去读这些字只会读到旧数据。\n旧内核搬一块、等、算、再等，然后才去取下一块。结果是对的，可它一整天都在等。让搬运引擎一直有活干，同时让 warp 去计算。",
        ),
        cases: || {
            vec![
                sand(1251, 4096, 1),
                sand(1252, 4096, 2),
                sand(1253, 4096, 4),
            ]
        },
        hidden: || {
            vec![
                sand(1254, 3000, 3),
                sand(1255, 1700, 4),
                sand(1256, 4096, 3),
            ]
        },
        expect: churn,
        grid: Some(|c| Launch::grid((c.rom[0] as usize).div_ceil(1024), 128)),
        limit: Some(Goal::new(Metric::Cycles, 7000)),
        goals: [
            Goal::new(Metric::Cycles, 6000),
            Goal::new(Metric::Smem, 256),
        ],
        starter: Source::Code(DMA_STARTER),
        reference: Source::Code(DMA_REF),
        predict: Predict {
            question: Tr::new(
                "A block has eight tiles. Copying one takes about 400 cycles and computing it about 400 more. If the copy of the next tile runs while the current tile is computed, the block's time is closest to:",
                "一个块有八个小块。搬一块约 400 拍，算一块约 400 拍。如果下一块的搬运和当前块的计算同时进行，这个块的总时间最接近：",
            ),
            options: [
                Tr::new("8 × 800", "8 × 800"),
                Tr::new("8 × 400 + 400", "8 × 400 + 400"),
                Tr::new("8 × 400 ÷ 2", "8 × 400 ÷ 2"),
            ],
            answer: 1,
            why: Tr::new(
                "Only the first copy cannot hide behind anything: 400 cycles. After that, each tile's compute covers the next tile's copy, so every step costs about 400. Overlap removes the repeated waiting, but it cannot make one stage finish sooner than its own 400 cycles.",
                "只有第一次搬运没有东西可以遮住：400 拍。此后每一块的计算都盖住下一块的搬运，每一步约 400 拍。重叠去掉的是反复的等待，却不能让任何一个环节比它自己的 400 拍更早做完。",
            ),
        },
        hints: [
            Tr::new(
                "Watch the warps in the Warp River: during each copy they all sit at BAR with nothing to do, and during each compute the copy engine is idle. The two never overlap.",
                "在 Warp River 里看 warp：每次搬运的时候它们都停在 BAR 前无事可做，每次计算的时候搬运引擎又闲着。两者从不重叠。",
            ),
            Tr::new(
                "Use two buffers of 128 words. While the warps compute tile k from one, a DMA fetches tile k + 1 into the other. BAR waits for every copy in flight, so issue the next DMA after the BAR, not before it.",
                "用两个 128 字的缓冲。warp 从一个缓冲里算第 k 块的时候，DMA 把第 k + 1 块取进另一个缓冲。BAR 会等所有在途的搬运，所以下一次 DMA 要放在 BAR 之后，而不是之前。",
            ),
            Tr::new(
                "Before the loop, one thread issues the DMA of tile 0. In each round: BAR, then (unless it is the last tile) one thread issues the DMA of the next tile into the other buffer, then compute the current tile, then swap the buffers. One huge DMA of all 1024 words also beats the old kernel, but it needs 1024 words of shared memory against 256.",
                "进循环前，由一个线程发出第 0 块的 DMA。每一轮：先 BAR，然后（如果不是最后一块）由一个线程把下一块的 DMA 发进另一个缓冲，再计算当前块，最后交换缓冲。一次性把 1024 个字全搬进来也能胜过旧内核，但要用 1024 个共享字，而不是 256 个。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Hiding the wait behind the work", "把等待藏在工作后面"),
            line: Tr::new(
                "A copy engine works while the warps compute. With two buffers, the copy of the next piece hides behind the compute of this one.",
                "搬运引擎可以在 warp 计算的同时工作。有了两个缓冲，下一块的搬运就能藏在这一块的计算后面。",
            ),
            reality: Tr::new(
                "Modern GPUs have asynchronous copy units (cp.async, the tensor memory accelerator) and software pipelines of two or more stages. Double buffering is the standard way to keep both the memory system and the math units busy.",
                "现代 GPU 有异步拷贝单元（cp.async、张量内存加速器），以及两级或更多级的软件流水线。双缓冲是让存储系统和运算单元同时有活干的标准做法。",
            ),
            myth: Tr::new(
                "Myth: an asynchronous copy is free. It still takes its full latency; you only gain by having other work to do meanwhile.",
                "误解：异步拷贝是免费的。它仍然要花满它的延迟；只有在这期间有别的活可干，你才赚到。",
            ),
            simplified: Tr::new(
                "OC-1 has one copy engine per SM, 400 cycles of latency plus 4 per line, and it only copies from the input. Real engines handle many shapes and many copies in flight.",
                "OC-1 每个 SM 只有一个搬运引擎，延迟 400 拍外加每行 4 拍，而且只能从输入里搬。真实的引擎能处理多种形状，也能同时有很多次搬运在途。",
            ),
        },
        ..base("the-dma", 6, Launch::grid(4, 128), 4096, DMA_OPS)
    }
}

// ---------------------------------------------------------------- 12-7

/// A weak-memory case: the seed (frame) decides how late the stores of block 0 become visible.
fn gale(frame: i32) -> Case {
    case(vec![0; 32], frame)
}

/// out[t] = 7t + 3 for the 256 data words, out[256] = 1 (the flag), out[320 + t] = the reader's copy.
fn mailbox(_: &Case) -> Vec<Option<i32>> {
    let mut out = vec![Some(0); 576];
    for t in 0..256 {
        out[t] = Some(7 * t as i32 + 3);
        out[320 + t] = out[t];
    }
    out[256] = Some(1);
    out
}

/// Block 0 writes the words, then the flag; block 1 polls the flag and copies the words.
const FENCE_STARTER: &str = "; data, then flag: nothing makes the data visible first | 先写数据再写标志：没有东西保证数据先可见\n\
    SETP.EQ p0, bx, 0\n\
    IF p0\n\
    MUL r1, tx, 7\n\
    ADD r1, r1, 3\n\
    ST [tx], r1\n\
    SETP.EQ p1, tx, 0\n\
    IF p1\n\
    ST [256], 1\n\
    ENDIF\n\
    ELSE\n\
    ADD r4, tx, 320\n\
    MOV r3, 0\n\
    LOOP 64\n\
    LDO r0, [256]\n\
    SETP.EQ p2, r0, 1\n\
    SETP.EQ p3, r3, 0\n\
    IF p2\n\
    IF p3\n\
    LDO r1, [tx]\n\
    ST [r4], r1\n\
    MOV r3, 1\n\
    ENDIF\n\
    ENDIF\n\
    ENDLOOP\n\
    ENDIF\n";

const FENCE_REF: &str = "SETP.EQ p0, bx, 0\n\
    IF p0\n\
    MUL r1, tx, 7\n\
    ADD r1, r1, 3\n\
    ST [tx], r1\n\
    FENCE\n\
    BAR\n\
    SETP.EQ p1, tx, 0\n\
    IF p1\n\
    ST [256], 1\n\
    ENDIF\n\
    ELSE\n\
    ADD r4, tx, 320\n\
    MOV r3, 0\n\
    LOOP 64\n\
    LDO r0, [256]\n\
    SETP.EQ p2, r0, 1\n\
    SETP.EQ p3, r3, 0\n\
    IF p2\n\
    IF p3\n\
    LDO r1, [tx]\n\
    ST [r4], r1\n\
    MOV r3, 1\n\
    ENDIF\n\
    ENDIF\n\
    ENDLOOP\n\
    ENDIF\n";

const FENCE_OPS: &[OpKind] = &[
    OpKind::Mov,
    OpKind::Alu(crate::isa::Alu::Add),
    OpKind::Alu(crate::isa::Alu::Mul),
    OpKind::SetP,
    OpKind::If,
    OpKind::Else,
    OpKind::EndIf,
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::St,
    OpKind::Ldo,
    OpKind::Bar,
    OpKind::Fence,
];

fn fence() -> Task {
    Task {
        title: Tr::new("The Fence", "栅栏"),
        brief: Tr::new(
            "Yinpo sends a message down the well in two parts: first 256 words of readings, then a flag that says the readings are ready. Block 0 of the kernel writes out[t] = 7t + 3 for t < 256, and then one thread sets out[256] = 1. Block 1 polls out[256] with LDO (64 tries, one read each), and when it sees 1 it copies the 256 words into out[320 + t].\nOn this machine a store reaches the other block's view only after a delay that depends on the day's weather (the seed), and a store with a few lanes is not necessarily seen in the order it was issued. The old kernel writes the words and then the flag, and still block 1 copies zeros. Find out why, and make the copy correct on every seed.",
            "银婆把消息分两部分送下井去：先是 256 个字的读数，然后是一个说明读数已备好的标志。内核里的 0 号块写 out[t] = 7t + 3（t < 256），然后由一个线程写 out[256] = 1。1 号块用 LDO 轮询 out[256]（最多 64 次，每次读一遍），一旦看到 1，就把这 256 个字拷进 out[320 + t]。\n在这台机器上，一次写入要过一段由当天天气（种子）决定的延迟，才会进入另一个块的视野，而且先发出的写入未必先被看见。旧内核先写字、再写标志，可 1 号块拷出来的却是零。找出原因，让拷贝在每一个种子下都正确。",
        ),
        cases: || vec![gale(1), gale(2)],
        hidden: || vec![gale(3), gale(4), gale(7), gale(10)],
        expect: mailbox,
        profile: Profile::ATLAS_WEAK,
        limit: Some(Goal::new(Metric::Cycles, 5600)),
        goals: [Goal::new(Metric::Cycles, 5200), Goal::new(Metric::Size, 28)],
        starter: Source::Code(FENCE_STARTER),
        reference: Source::Code(FENCE_REF),
        clue: Some(Clue {
            what: Tr::new(
                "the first read of a word that is still old although the flag written after it is already visible",
                "第一次读到仍是旧值的字，尽管写在它之后的标志已经可见",
            ),
            find: crate::grade::first_reordered,
        }),
        predict: Predict {
            question: Tr::new(
                "Block 0 stores the data, then stores the flag. Can block 1 see the flag set and still read the old data?",
                "0 号块先存数据，再存标志。1 号块能不能看到标志已置位，却仍读到旧数据？",
            ),
            options: [
                Tr::new(
                    "No: stores always become visible in the order they were issued",
                    "不能：写入总是按发出的顺序变得可见",
                ),
                Tr::new(
                    "Yes: each store reaches other blocks after its own delay",
                    "能：每次写入各自过一段延迟才到达别的块",
                ),
                Tr::new(
                    "Only if the two blocks share an SM",
                    "只有两个块在同一个 SM 上才可能",
                ),
            ],
            answer: 1,
            why: Tr::new(
                "A store leaves its block and becomes visible to others after a delay that grows with the number of lanes and has some jitter. A one-lane flag store is quick; a full warp of data takes longer. So the flag can arrive first.",
                "一次写入离开本块后，要过一段延迟才对别的块可见，延迟随参与的车道数增长，还带一点抖动。单车道的标志写得快，整个 warp 的数据写得慢。所以标志可能先到。",
            ),
        },
        hints: [
            Tr::new(
                "Look at the Warp River at the marked read: block 1 already sees the flag as 1, but the word it reads is still the old value. The data store was issued first, yet it has not arrived.",
                "在 Warp River 里看被标出的那次读：1 号块已经看到标志是 1，可读到的字还是旧值。数据的写入先发出，却还没到。",
            ),
            Tr::new(
                "Waiting a few cycles before the flag may work for the weather you tried and fail on a seed with a longer delay. A BAR does not help either: it lines the warps up, but it does not make their stores visible.",
                "在置标志前多等几拍，也许对你试过的天气有效，换成延迟更长的种子又会失败。BAR 也没用：它让各个 warp 对齐，却不会让它们的写入变得可见。",
            ),
            Tr::new(
                "FENCE stalls a warp until its block's earlier stores are visible. Put FENCE right after the data store (every thread), then BAR so that all warps have fenced, and only then let one thread store the flag.",
                "FENCE 会让 warp 停到本块较早的写入都已可见。在数据写入之后立刻加 FENCE（每个线程都执行），再 BAR 让所有 warp 都过了栅栏，最后才让一个线程写标志。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Visibility is not order", "可见性不等于顺序"),
            line: Tr::new(
                "Another block may see your later store before your earlier one. A fence makes the earlier stores visible before anything after it is written.",
                "别的块可能先看到你较晚的写入，后看到较早的。栅栏保证它之前的写入先变得可见，之后的才写。",
            ),
            reality: Tr::new(
                "GPU memory is weakly ordered between blocks. Producer-consumer code needs __threadfence() (or release/acquire atomics) between writing the data and writing the flag, and the consumer must read the flag with the matching care.",
                "GPU 的内存在块与块之间是弱序的。生产者与消费者的代码，要在写数据和写标志之间加 __threadfence()（或 release/acquire 原子操作），消费者读标志时也要对应地小心。",
            ),
            myth: Tr::new(
                "Myth: if I wrote A before B, everyone sees A before B. Only a fence or a synchronisation point promises that.",
                "误解：只要我先写 A 再写 B，所有人就会先看到 A 再看到 B。只有栅栏或同步点才有这个保证。",
            ),
            simplified: Tr::new(
                "OC-1 models the delay with a single number per store and one jitter. Real hardware has caches, write buffers and scopes (block, device, system).",
                "OC-1 只用每次写入一个延迟和一点抖动来模拟。真实硬件有缓存、写缓冲，还有不同的作用域（块、设备、系统）。",
            ),
        },
        ..base("the-fence", 7, Launch::grid(2, 256), 576, FENCE_OPS)
    }
}

// ---------------------------------------------------------------- 12-8

const REGIONS: usize = 64;
const MAP: usize = 16384;
const FLAGS: usize = MAP + 64;

/// One day of wind. rom[r] = direction (0..4), rom[64 + r] = speed (1..10), terrain of region r
/// at rom[128 + 256r ..] (a 16 x 16 grid, values 0..16). `mix` decides the direction pattern.
fn day(seed: u64, mix: u8, frame: i32) -> Case {
    let mut rom = vec![0; 128];
    let dirs = values(seed, REGIONS, 0, 4);
    for r in 0..REGIONS {
        rom[r] = match mix {
            0 => dirs[r],
            1 => (r / 16) as i32,
            2 => 2 + dirs[r] % 2,
            _ => dirs[r] % 2,
        };
    }
    for (r, v) in values(seed + 50, REGIONS, 1, 10).into_iter().enumerate() {
        rom[64 + r] = v;
    }
    rom.extend(values(seed + 100, REGIONS * 256, 0, 16));
    case(rom, frame)
}

/// Sand carried by one cell: the formula depends on the wind direction of its region.
fn sand_q(d: i32, s: i32, t: i32) -> i32 {
    match d {
        0 => t * s + 3,
        1 => ((t ^ s) << 1) + 1,
        2 => t * t + s,
        _ if t > s => t * 3,
        _ => t + (s << 1),
    }
}

fn sand_map(c: &Case) -> Vec<Option<i32>> {
    let mut out = vec![Some(0); FLAGS + REGIONS];
    for r in 0..REGIONS {
        let (d, s) = (c.rom[r], c.rom[64 + r]);
        let q = |y: i32, x: i32| {
            if (0..16).contains(&y) && (0..16).contains(&x) {
                sand_q(d, s, c.rom[128 + 256 * r + (y * 16 + x) as usize])
            } else {
                0
            }
        };
        for y in 0..16 {
            for x in 0..16 {
                let m: i32 = (-1..=1)
                    .flat_map(|dy| (-1..=1).map(move |dx| (dy, dx)))
                    .map(|(dy, dx)| q(y + dy, x + dx))
                    .sum();
                out[256 * r + (y * 16 + x) as usize] = Some(m);
                if y == 8 && x == 8 {
                    out[MAP + r] = Some(m);
                }
            }
        }
        out[FLAGS + r] = Some(1);
    }
    out
}

const SAND_REF: &str = ".smem 836\n\
SHR r5, bx, 4\n\
SHR r6, tx, 5\n\
SETP.GT p2, r5, r6\n\
IF p2\n\
SUB r0, bx, 16\n\
SHL r0, r0, 5\n\
ADD r0, r0, tx\n\
MOV r3, 0\n\
LOOP 4\n\
LOOP 64\n\
ADD r5, r0, 16448\n\
LDO r2, [r5]\n\
SETP.GT p1, r2, r3\n\
IF p1\n\
SHL r5, r0, 8\n\
ADD r5, r5, 136\n\
LDO r4, [r5]\n\
ADD r5, r0, 16384\n\
ST [r5], r4\n\
MOV r3, 1\n\
ENDIF\n\
ENDLOOP\n\
ENDLOOP\n\
ENDIF\n\
SETP.LT p2, bx, 16\n\
IF p2\n\
SHL r0, bx, 2\n\
SETP.EQ p3, tx, 0\n\
SHL r1, r0, 8\n\
ADD r1, r1, 128\n\
ADD r1, r1, tx\n\
LOOP 4\n\
LD r2, [r1]\n\
ADD r1, r1, 256\n\
LD r3, [r0]\n\
ADD r7, r0, 64\n\
LD r4, [r7]\n\
SETP.EQ p0, r3, 0\n\
IF p0\n\
MUL r5, r2, r4\n\
ADD r2, r5, 3\n\
ENDIF\n\
SETP.EQ p0, r3, 1\n\
IF p0\n\
XOR r5, r2, r4\n\
SHL r5, r5, 1\n\
ADD r2, r5, 1\n\
ENDIF\n\
SETP.EQ p0, r3, 2\n\
IF p0\n\
MUL r5, r2, r2\n\
ADD r2, r5, r4\n\
ENDIF\n\
SETP.EQ p0, r3, 3\n\
IF p0\n\
SETP.GT p1, r2, r4\n\
IF p1\n\
MUL r2, r2, 3\n\
ELSE\n\
SHL r5, r4, 1\n\
ADD r2, r2, r5\n\
ENDIF\n\
ENDIF\n\
SHR r5, tx, 4\n\
MUL r5, r5, 18\n\
AND r7, tx, 15\n\
ADD r5, r5, r7\n\
ADD r5, r5, 512\n\
ADD r7, r5, 19\n\
STS [r7], r2\n\
BAR\n\
MOV r7, 0\n\
LOOP 3\n\
LOOP 3\n\
LDS r2, [r5]\n\
ADD r7, r7, r2\n\
ADD r5, r5, 1\n\
ENDLOOP\n\
ADD r5, r5, 15\n\
ENDLOOP\n\
SHL r5, r0, 8\n\
ADD r5, r5, tx\n\
ST [r5], r7\n\
FENCE\n\
BAR\n\
IF p3\n\
ADD r5, r0, 16448\n\
ST [r5], 1\n\
ENDIF\n\
ADD r0, r0, 1\n\
ENDLOOP\n\
ENDIF\n";

const SAND_OPS: &[OpKind] = &[
    OpKind::Mov,
    OpKind::Alu(crate::isa::Alu::Add),
    OpKind::Alu(crate::isa::Alu::Sub),
    OpKind::Alu(crate::isa::Alu::Mul),
    OpKind::Alu(crate::isa::Alu::And),
    OpKind::Alu(crate::isa::Alu::Xor),
    OpKind::Alu(crate::isa::Alu::Shl),
    OpKind::Alu(crate::isa::Alu::Shr),
    OpKind::SetP,
    OpKind::If,
    OpKind::Else,
    OpKind::EndIf,
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::Ld,
    OpKind::Ldo,
    OpKind::St,
    OpKind::Lds,
    OpKind::Sts,
    OpKind::Bar,
    OpKind::Dma,
    OpKind::Fence,
];

fn sand_atlas() -> Task {
    Task {
        title: Tr::new("Sand Charts of the Atlas", "区域沙风图"),
        brief: Tr::new(
            "The caravan wants a sand chart for 64 regions of the desert, and a small overview of all of them. Region r has a wind direction d = rom[r] (0..3), a speed s = rom[64 + r], and a 16 x 16 terrain grid t at rom[128 + 256r + 16y + x] (values 0..15). Each cell carries q sand: d0: t*s + 3; d1: ((t ^ s) << 1) + 1; d2: t*t + s; d3: t > s ? t*3 : t + (s << 1). Then each cell of the region chart is the sum of q over its 3 x 3 neighbourhood, with cells outside the region counted as 0. Write the chart of region r to out[256r + 16y + x].\nThe overview is stitched by other blocks once a region is done: out[16384 + r] must hold the chart cell (8, 8) of region r, and the producer of region r sets out[16448 + r] = 1 after its chart is written. The launch is fixed: 18 blocks of 256 threads, and how you split the work between them is yours. Hidden days have another mix of winds and another weather for the stores. There is no starter; Yinpo is waiting.",
            "驼队想要沙漠 64 个区域的沙风图，外加一张总览。区域 r 有风向 d = rom[r]（0..3）、风速 s = rom[64 + r]，以及位于 rom[128 + 256r + 16y + x] 的 16 × 16 地形 t（值 0..15）。每格带的沙量 q：d0：t*s + 3；d1：((t ^ s) << 1) + 1；d2：t*t + s；d3：t > s ? t*3 : t + (s << 1)。区域图的每一格，是它 3 × 3 邻域里 q 的总和，区域外的格子按 0 计。区域 r 的图写到 out[256r + 16y + x]。\n总览由别的块在区域完成后拼入：out[16384 + r] 要放区域 r 图的 (8, 8) 格，区域 r 的生产者在写完图之后，置 out[16448 + r] = 1。启动是固定的：18 个块、每块 256 个线程，工作怎么在块之间分，由你决定。隐藏的几天风向分布不同，写入的天气也不同。没有初始方案，银婆在等。",
        ),
        cases: || vec![day(1261, 0, 1), day(1262, 0, 2), day(1263, 1, 3)],
        hidden: || vec![day(1264, 2, 4), day(1265, 3, 7)],
        expect: sand_map,
        profile: Profile::ATLAS_WEAK,
        limit: Some(Goal::new(Metric::Cycles, 24000)),
        goals: [Goal::new(Metric::Cycles, 19500), Goal::new(Metric::Regs, 8)],
        starter: Source::Code("; no starter: the caravan is waiting | 没有初始方案：驼队在等\n"),
        reference: Source::Code(SAND_REF),
        live: Some(|frame, at| {
            let k = at[0].rem_euclid(4);
            day(1270 + k as u64, k as u8, frame)
        }),
        steer: Some(Tr::new("PICK A DAY", "挑一天")),
        predict: Predict {
            question: Tr::new(
                "One version of the kernel uses 8 registers per thread, another uses 9. How many warps can one SM keep resident for each?",
                "内核的一个版本每线程用 8 个寄存器，另一个用 9 个。一个 SM 各能同时驻留多少个 warp？",
            ),
            options: [
                Tr::new(
                    "32 and 32: one register is too small to matter",
                    "32 和 32：差一个寄存器不起作用",
                ),
                Tr::new(
                    "32 and 28: 256 registers shared out, rounded down",
                    "32 和 28：256 个寄存器分下去，向下取整",
                ),
                Tr::new(
                    "32 and 16: a ninth register halves the warps",
                    "32 和 16：多一个寄存器，warp 减半",
                ),
            ],
            answer: 1,
            why: Tr::new(
                "An SM has 256 registers per lane to hand out: 256 / 8 = 32 warps, but 256 / 9 = 28. With 28 warps only three 8-warp blocks fit, so the fourth block of a wave waits for a seat. The Regs star is exactly this line.",
                "每个 SM 每条车道有 256 个寄存器可分：256 / 8 = 32 个 warp，256 / 9 却只有 28。28 个 warp 只装得下三个 8 warp 的块，同一波里的第四个块要等座位。寄存器那颗星，说的正是这条线。",
            ),
        },
        hints: [
            Tr::new(
                "Split the work first: 16 blocks each take 4 regions, one after another (a block has 256 threads, a region has 256 cells). The other two blocks only stitch. Do not let all eight warps of a stitching block poll; one warp is enough and the rest only crowd the SM.",
                "先分工：16 个块各管 4 个区域，一个接一个（块有 256 个线程，区域有 256 格）。另外两个块只负责拼总览。拼总览的块不必八个 warp 全去轮询，一个 warp 就够，其余的只会挤占 SM。",
            ),
            Tr::new(
                "Count registers. Keep the values you need in as few registers as you can: the direction and speed can be reloaded when needed, and a loaded register can be reused as soon as its value has been consumed. Eight or fewer is the goal. Prefetching the next region into spare registers is faster to write and costs the star.",
                "数一数寄存器。尽量让需要的值占用最少的寄存器：风向和风速要用时再读一遍，读来的寄存器用完立刻可以复用。目标是 8 个或更少。把下一个区域预取到多余的寄存器里写起来快，却会丢掉那颗星。",
            ),
            Tr::new(
                "A producer must FENCE after storing its chart and BAR before it sets the region's flag, or a stitcher may see the flag before the chart. A stitcher that polls too briefly fails on a slow day: the hidden days have different weather, so poll with a generous bound and stitch each region once its flag is seen.",
                "生产者写完图后要先 FENCE，再 BAR，才能置该区域的标志，否则拼总览的块可能先看到标志、后看到图。轮询太短的拼接者，在慢的天气里会失败：隐藏的几天天气不同，所以轮询的次数要给足，看到某个区域的标志后再拼它。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Tune for the machine you have", "为手上的机器调优"),
            line: Tr::new(
                "Fewer registers keep more warps resident, and a fence makes the handover safe. The cycle count and the register count are separate stars because they are separate costs.",
                "寄存器少，驻留的 warp 就多；栅栏让交接安全。周期数和寄存器数是两颗星，因为它们是两种代价。",
            ),
            reality: Tr::new(
                "Real kernels are tuned the same way: check register use against occupancy (the compiler's -maxrregcount or launch bounds), overlap copies with compute, and put a fence between data and flag. Which of them pays depends on whether the kernel is bound by issue, by memory latency or by waiting.",
                "真实的内核也是这样调的：对照占用率检查寄存器用量（编译器的 -maxrregcount 或 launch bounds），让搬运与计算重叠，在数据与标志之间加栅栏。哪一项划算，取决于内核卡在发射、内存延迟还是等待上。",
            ),
            myth: Tr::new(
                "Myth: use every trick on every kernel. Here the SMs are busy issuing instructions, so a DMA double buffer buys little; measure first.",
                "误解：每个内核都该用上所有技巧。这里 SM 忙着发射指令，DMA 双缓冲几乎省不下什么；先量一量。",
            ),
            simplified: Tr::new(
                "OC-1 has one register file size, one fence and one delay model. A real GPU has register allocation granularity, shared-memory limits and several memory scopes.",
                "OC-1 只有一种寄存器堆大小、一种栅栏、一种延迟模型。真实 GPU 还有寄存器分配粒度、共享内存上限和多种内存作用域。",
            ),
        },
        ..base(
            "sand-atlas",
            8,
            Launch::grid(18, 256),
            FLAGS + REGIONS,
            SAND_OPS,
        )
    }
}

// __LEVELS__
