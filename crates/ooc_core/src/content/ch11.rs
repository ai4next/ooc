//! Chapter 6 · Atlas: the screen goes dark; arrays and grids remain.

use oo_base::Tr;

use super::atlas::{ATLAS_OPS, base as atlas_base};
use super::{WARP_OPS, below, rng, stub};
use crate::isa::LANES;
use crate::task::*;
use crate::vm::Launch;

fn base(id: &'static str, slot: u8) -> Task {
    Task {
        init: lanes_blank,
        ..stub(
            id,
            6,
            slot,
            View::Lanes,
            Editor::Code(WARP_OPS),
            Launch::ONE_WARP,
            LANES,
        )
    }
}

/// `n` random values in `lo..hi`.
fn values(seed: u64, n: usize, lo: i32, hi: i32) -> Vec<i32> {
    let mut r = rng(seed);
    (0..n).map(|_| lo + below(&mut r, hi - lo)).collect()
}

fn rom_cases(make: impl Fn(u64) -> Vec<i32>, seeds: &[u64]) -> Vec<Case> {
    seeds
        .iter()
        .map(|&s| Case {
            rom: make(s),
            frame: 0,
        })
        .collect()
}

fn lanes(f: impl Fn(usize) -> i32) -> Vec<Option<i32>> {
    (0..LANES).map(|l| Some(f(l))).collect()
}

pub fn tasks() -> Vec<Task> {
    vec![
        grid_reduce(),
        the_prefix(),
        the_compact(),
        shuffled_order(),
        radix_pass(),
        the_csr(),
        the_hub(),
        storm_surge(),
    ]
}

/// One Hillis–Steele step at distance `$d`: add lane − d's value if it exists.
macro_rules! scan_step {
    ($d:literal) => {
        concat!(
            "SUB r2, lane, ",
            $d,
            "\nSHFL r1, r0, r2\nSETP.GE p0, lane, ",
            $d,
            "\nSEL r1, p0, r1, 0\nADD r0, r0, r1\n",
        )
    };
}

fn the_prefix() -> Task {
    Task {
        title: Tr::new("The Prefix", "前缀和"),
        brief: Tr::new(
            "The well gauge logs 32 inflows in rom[0..32]. Each lane must report the running total so far:\nout[lane] = rom[0] + rom[1] + … + rom[lane].\nThe first step is wired. It only looks one lane back — keep doubling the distance.",
            "井水水位计在 rom[0..32] 记录了 32 次涨水量。每条车道要报告到它为止的累计总量：\nout[lane] = rom[0] + rom[1] + … + rom[lane]。\n第一步已经接好了，但它只往回看一条车道——继续把距离翻倍。",
        ),
        cases: || rom_cases(|s| values(s, 32, 0, 100), &[151, 152, 153]),
        hidden: || rom_cases(|s| values(s, 32, 0, 100), &[154, 155]),
        expect: |c| lanes(|l| c.rom[..=l].iter().sum()),
        goals: [Goal::new(Metric::Cycles, 27), Goal::new(Metric::Size, 27)],
        starter: Source::Code(concat!(
            "LD r0, [lane]\n; add the value one lane back (lane 0 has none) | 加上前一条车道的值（0 号车道没有）\n",
            scan_step!(1),
            "ST [lane], r0\n",
        )),
        reference: Source::Code(concat!(
            "LD r0, [lane]\n",
            scan_step!(1),
            scan_step!(2),
            scan_step!(4),
            scan_step!(8),
            scan_step!(16),
            "ST [lane], r0\n",
        )),
        predict: Predict {
            question: Tr::new(
                "After the steps at distance 1 and 2, how many readings does lane 7 hold the sum of?",
                "在距离 1 和距离 2 两步之后，7 号车道持有多少个读数的和？",
            ),
            options: [Tr::new("2", "2"), Tr::new("3", "3"), Tr::new("4", "4")],
            answer: 2,
            why: Tr::new(
                "Step 1 gives lanes 6..7; step 2 adds lane 5's pair, 4..5. Each step doubles the window: 1, 2, 4, 8 …",
                "第一步得到 6..7；第二步加上 5 号车道的那一对 4..5。每一步窗口都翻倍：1、2、4、8……",
            ),
        },
        hints: [
            Tr::new(
                "After the given step each lane has 2 readings. What would a second, identical-looking step at distance 2 give?",
                "给定的那一步之后，每条车道有 2 个读数的和。再来一个距离为 2 的同样步骤会得到什么？",
            ),
            Tr::new(
                "Repeat the five-line step with distances 2, 4, 8 and 16. The SETP guard stops lanes near 0 from wrapping around.",
                "把这五行的步骤按距离 2、4、8、16 重复。SETP 的保护能防止靠近 0 号的车道绕回去。",
            ),
            Tr::new(
                "Copy the step four times, changing both 1s (in SUB and in SETP.GE) to 2, 4, 8, 16. ST stays last.",
                "把这一步复制四次，把两个 1（SUB 里和 SETP.GE 里的）分别改成 2、4、8、16。ST 仍放在最后。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Parallel scan", "并行扫描"),
            line: Tr::new(
                "A running total looks hopelessly serial. Doubling the look-back distance makes it five steps.",
                "累计和看起来注定是串行的。把回看距离不断翻倍，它就只要五步。",
            ),
            reality: Tr::new(
                "Scans power GPU sorting, stream compaction and allocation; libraries like CUB tune them per chip.",
                "扫描支撑着 GPU 上的排序、流压缩与内存分配；CUB 等库会为每款芯片专门调优。",
            ),
            myth: Tr::new(
                "Myth: the fastest parallel algorithm does the least work. Hillis–Steele does more additions than a loop — and still wins.",
                "误解：最快的并行算法做的工作最少。Hillis–Steele 做的加法比循环还多——却依然更快。",
            ),
            simplified: Tr::new(
                "Production scans use the work-efficient Blelloch form across blocks; one warp keeps this one simple.",
                "生产环境的扫描在多个线程块之间使用工作量更省的 Blelloch 形式；这里用一个 warp 保持简单。",
            ),
        },
        ..base("the-prefix", 2)
    }
}

// ---------------------------------------------------------------------------
// Atlas levels (11-1, 11-3 .. 11-8)
// ---------------------------------------------------------------------------

fn abase(id: &'static str, slot: u8, launch: Launch, out_len: usize) -> Task {
    atlas_base(id, slot, launch, out_len, ATLAS_OPS)
}

/// A case with a scheduling seed (`frame`) of its own.
fn seeded(rom: Vec<i32>, frame: i32) -> Case {
    Case { rom, frame }
}

// ---- 11-1 grid-reduce ------------------------------------------------------

fn grid_reduce() -> Task {
    Task {
        title: Tr::new("Grid Reduce", "整格归约"),
        brief: Tr::new(
            "Yinpo's well has a ring of gauges: rom[0..n] holds n inflow readings, n up to 4096. Anything past the end reads as 0.\nAdd them all up into out[0].\nThe tower gives you 8 blocks of 512 threads. Sending every thread to the same counter is correct, but the counter serves one lane at a time: at most 16 lanes may use ATOM.",
            "银婆的井边围了一圈水位计：rom[0..n] 是 n 个涨水读数，n 最多 4096，越过末尾的读数为 0。\n把它们全部加起来，写进 out[0]。\n风塔给你 8 个块、每块 512 个线程。让每个线程都去加同一个计数器，结果没错，但计数器一次只服务一条车道：全程最多只允许 16 条车道使用 ATOM。",
        ),
        cases: || {
            vec![
                seeded(values(1101, 1024, 0, 100), 1),
                seeded(values(1102, 2048, 0, 100), 2),
                seeded(values(1103, 4096, 0, 100), 3),
            ]
        },
        hidden: || {
            vec![
                seeded(values(1104, 3000, 0, 100), 5),
                seeded(vec![37], 6),
                seeded(values(1105, 4096, -50, 50), 7),
            ]
        },
        expect: |c| vec![Some(c.rom.iter().sum())],
        limit: Some(Goal::new(Metric::Atoms, 16)),
        goals: [Goal::new(Metric::Cycles, 1250), Goal::new(Metric::Size, 28)],
        starter: Source::Code(
            "LD r0, [tid]    ; one reading per thread | 每个线程一个读数\n; every thread adds to out[0]: correct, but 4096 lanes queue on one counter\n; 每个线程都加到 out[0]：结果对，但 4096 条车道在同一个计数器前排队\nATOM r1, [0], r0\n",
        ),
        reference: Source::Code(concat!(
            "LD r0, [tid]\n",
            "XOR r2, lane, 16\nSHFL r1, r0, r2\nADD r0, r0, r1\n",
            "XOR r2, lane, 8\nSHFL r1, r0, r2\nADD r0, r0, r1\n",
            "XOR r2, lane, 4\nSHFL r1, r0, r2\nADD r0, r0, r1\n",
            "XOR r2, lane, 2\nSHFL r1, r0, r2\nADD r0, r0, r1\n",
            "XOR r2, lane, 1\nSHFL r1, r0, r2\nADD r0, r0, r1\n",
            "SETP.EQ p0, lane, 0\nIF p0\nATOMS r1, [0], r0\nENDIF\nBAR\n",
            "SETP.EQ p1, tx, 0\nIF p1\nLDS r0, [0]\nATOM r1, [0], r0\nENDIF\n",
        )),
        predict: Predict {
            question: Tr::new(
                "4096 threads all ATOM-add into the same address. The cycle count grows roughly with:",
                "4096 个线程同时对同一个地址做 ATOM 加法。周期数大致随什么增长？",
            ),
            options: [
                Tr::new("the number of threads", "线程的数量"),
                Tr::new("the logarithm of the thread count", "线程数量的对数"),
                Tr::new(
                    "nothing: ATOM is a single instruction",
                    "与线程数无关：ATOM 只是一条指令",
                ),
            ],
            answer: 0,
            why: Tr::new(
                "Lanes that hit the same address are served one after another, 2 cycles each. 4096 lanes mean thousands of cycles in a line, no matter how few instructions the program has.",
                "命中同一地址的车道被一条接一条地服务，每条 2 个周期。4096 条车道就是排成一队的几千个周期，不管程序有多少条指令。",
            ),
        },
        hints: [
            Tr::new(
                "Do not let 4096 lanes meet at one counter. Add inside each warp first: SHFL can bring a neighbour's value to you.",
                "别让 4096 条车道都挤到同一个计数器前。先在每个 warp 内部相加：SHFL 能把邻居的值取过来。",
            ),
            Tr::new(
                "A butterfly: for d = 16, 8, 4, 2, 1 do r0 += SHFL(r0, lane XOR d). Afterwards every lane holds its warp's sum. Then combine the 16 warps of a block in shared memory (ATOMS is fine) behind a BAR.",
                "蝶形求和：对 d = 16、8、4、2、1，做 r0 += SHFL(r0, lane XOR d)。之后每条车道都持有本 warp 的和。再把一个块的 16 个 warp 在共享内存里合并（用 ATOMS 即可），中间隔一个 BAR。",
            ),
            Tr::new(
                "After the butterfly, lane 0 of each warp does ATOMS [0] += r0. BAR. Then only thread tx = 0 reads LDS [0] and does one ATOM into out[0]: 8 global atomics for the whole grid.",
                "蝶形之后，每个 warp 的 0 号车道做 ATOMS [0] += r0。BAR。然后只让 tx = 0 的线程读 LDS [0]，对 out[0] 做一次 ATOM：整个网格一共 8 次全局原子操作。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Reduce in tiers", "分层归约"),
            line: Tr::new(
                "Sum inside the warp, then inside the block, and only then touch global memory once per block.",
                "先在 warp 内求和，再在块内求和，最后每个块只碰一次全局内存。",
            ),
            reality: Tr::new(
                "Every GPU sum, dot product and histogram is built this way: warp shuffles, shared memory, one atomic per block.",
                "GPU 上的求和、点积和直方图都是这样搭起来的：warp 内洗牌、共享内存、每块一次原子操作。",
            ),
            myth: Tr::new(
                "Myth: atomics make parallel sums trivial. They are correct, but they turn a parallel program back into a queue.",
                "误解：有了原子操作，并行求和就很简单。它们是对的，但会把并行程序重新变成一条队。",
            ),
            simplified: Tr::new(
                "Real atomics on one address are pipelined by the L2 cache and are faster than this machine's 2 cycles; the queue is still real.",
                "真实硬件里，同一地址的原子操作由 L2 缓存流水处理，比这台机器的 2 个周期快；但排队依然存在。",
            ),
        },
        ..abase("grid-reduce", 1, Launch::grid(8, 512), 1)
    }
}

// ---- 11-3 the-compact ------------------------------------------------------

const CELLS: usize = 4096;
/// Rounds of the wet-cell update (4 issue slots each with the loop edge).
const F_ROUNDS: usize = 24;

/// The wet-cell update on register `$r`: `F_ROUNDS` rounds of `v = (v * 5 + 3) & 1023`.
macro_rules! wet_f {
    ($r:literal) => {
        concat!(
            "LOOP 24\nMUL ",
            $r,
            ", ",
            $r,
            ", 5\nADD ",
            $r,
            ", ",
            $r,
            ", 3\nAND ",
            $r,
            ", ",
            $r,
            ", 1023\nENDLOOP\n"
        )
    };
}

/// The wet-cell update: twenty-four rounds of `v = (v * 5 + 3) & 1023`.
fn wet_update(mut v: i32) -> i32 {
    for _ in 0..F_ROUNDS {
        v = (v * 5 + 3) & 1023;
    }
    v
}

/// A field of 4096 cells: `rom[0..4096]` the wet mask, `rom[4096..]` the values.
fn field(seed: u64, wet: impl Fn(usize, &mut super::Rng) -> bool) -> Vec<i32> {
    let mut r = rng(seed);
    let mut rom: Vec<i32> = (0..CELLS).map(|i| wet(i, &mut r) as i32).collect();
    rom.extend((0..CELLS).map(|_| below(&mut r, 1000)));
    rom
}

fn wet_share(seed: u64, per_mille: i32) -> Vec<i32> {
    field(seed, |_, r| below(r, 1000) < per_mille)
}

fn wet_run(seed: u64, from: usize, len: usize) -> Vec<i32> {
    field(seed, |i, _| i >= from && i < from + len)
}

fn the_compact() -> Task {
    Task {
        title: Tr::new("The Compact", "紧凑清点"),
        brief: Tr::new(
            "Yinpo's wind tower watches a field of 4096 cells. rom[0..4096] is a mask: 1 marks a wet cell, 0 a dry one. rom[4096..8192] holds the cell values.\nWet cells need the update f: twenty-four rounds of v = (v * 5 + 3) AND 1023. Dry cells are copied unchanged. Write every result to out[i].\nThe starter skips dry cells with an IF and is correct, but too slow: a warp pays for f if even one of its 32 cells is wet.",
            "银婆的风塔守着一片 4096 格的田地。rom[0..4096] 是掩码：1 表示湿格，0 表示干格；rom[4096..8192] 是各格的数值。\n湿格要做更新 f：二十四轮 v = (v * 5 + 3) AND 1023；干格原样拷贝。把每格的结果写进 out[i]。\n起始程序用 IF 跳过干格，结果没错，却太慢：只要一个 warp 的 32 格里有一格是湿的，整个 warp 就要付 f 的钱。",
        ),
        cases: || {
            vec![
                seeded(wet_share(1301, 100), 1),
                seeded(wet_share(1302, 50), 2),
                seeded(wet_share(1303, 250), 3),
            ]
        },
        hidden: || {
            vec![
                seeded(wet_share(1304, 20), 5),
                seeded(wet_share(1305, 150), 6),
                seeded(wet_run(1306, 1300, 400), 7),
            ]
        },
        expect: |c| {
            (0..CELLS)
                .map(|i| {
                    let v = c.rom[CELLS + i];
                    Some(if c.rom[i] != 0 { wet_update(v) } else { v })
                })
                .collect()
        },
        limit: Some(Goal::new(Metric::Cycles, 2600)),
        goals: [Goal::new(Metric::Cycles, 2150), Goal::new(Metric::Size, 34)],
        starter: Source::Code(concat!(
            "LD r0, [tid]    ; mask | 掩码\nADD r1, tid, 4096\nLD r1, [r1]     ; value | 数值\nSETP.NE p0, r0, 0\n",
            "IF p0           ; only wet cells... but a warp with one wet lane still pays | 只算湿格……但只要有一条车道湿，整个 warp 仍要付费\n",
            wet_f!("r1"),
            "ENDIF\nST [tid], r1\n",
        )),
        reference: Source::Code(concat!(
            "LD r0, [tid]\nADD r1, tid, 4096\nLD r1, [r1]\nSETP.NE p0, r0, 0\nSETP.EQ p1, r0, 0\n",
            "IF p1\nST [tid], r1\nENDIF\n",
            "IF p0\nATOMS r2, [0], 1\nADD r3, r2, 1\nSTS [r3], tid\nADD r3, r3, 512\nSTS [r3], r1\nENDIF\n",
            "BAR\nLDS r4, [0]\nSETP.LT p2, tx, r4\n",
            "IF p2\nADD r5, tx, 1\nLDS r6, [r5]\nADD r5, r5, 512\nLDS r7, [r5]\n",
            wet_f!("r7"),
            "ST [r6], r7\nENDIF\n",
        )),
        predict: Predict {
            question: Tr::new(
                "10% of the cells are wet, scattered at random. The chance that a given 32-cell warp holds at least one wet cell is closest to:",
                "10% 的格子随机地湿了。某个 32 格的 warp 里至少有一格湿的概率最接近：",
            ),
            options: [
                Tr::new("10%", "10%"),
                Tr::new("50%", "50%"),
                Tr::new("97%", "97%"),
            ],
            answer: 2,
            why: Tr::new(
                "All 32 cells must be dry for the warp to skip f: 0.9^32 is about 3%. So about 97% of the warps run f anyway, and the IF saves almost nothing.",
                "要想让 warp 跳过 f，32 格必须全干：0.9 的 32 次方约 3%。所以大约 97% 的 warp 照样要算 f，IF 几乎什么也没省下。",
            ),
        },
        hints: [
            Tr::new(
                "The IF skips lanes, not warps. To really save time, the wet cells must end up together in a few warps.",
                "IF 跳过的是车道，不是 warp。要真省时间，就得让湿格集中到少数几个 warp 里。",
            ),
            Tr::new(
                "Compact first: every wet cell takes the next free slot in a list in shared memory (ATOMS on a counter works; a scan would too) and stores its index and value there. After a BAR, thread tx takes list entry tx.",
                "先压紧：每个湿格在共享内存的列表里领下一个空位（对计数器做 ATOMS 即可，用扫描也行），把自己的下标和数值存进去。BAR 之后，线程 tx 取列表的第 tx 项。",
            ),
            Tr::new(
                "Dry cells store their copy at once. Wet cells: ATOMS r2, [0], 1 gives the slot; STS the index at slot+1 and the value at slot+513. After BAR, threads with tx below the count (LDS [0]) run f and ST to the stored index.",
                "干格立刻写出拷贝。湿格：ATOMS r2, [0], 1 得到位次；把下标存在 slot+1，数值存在 slot+513。BAR 之后，tx 小于计数（LDS [0]）的线程计算 f，并 ST 到存下的下标。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Stream compaction", "流压缩"),
            line: Tr::new(
                "When the work is sparse, do not ask every lane whether to work. Pack the work first, then spend full warps on it.",
                "工作稀疏时，不要让每条车道都问一遍要不要干活。先把活儿打包，再用满员的 warp 去做。",
            ),
            reality: Tr::new(
                "Particle culling, ray sorting and sparse solvers all begin with a compaction: scan the flags, scatter the survivors, process the dense list.",
                "粒子剔除、光线分组、稀疏求解器都从压缩开始：扫描标志、散布幸存者、再处理紧凑的列表。",
            ),
            myth: Tr::new(
                "Myth: an IF around the work makes sparse work cheap. A warp only skips it when all 32 lanes agree.",
                "误解：给工作套一个 IF 就能让稀疏任务变便宜。只有 32 条车道全部一致，warp 才会跳过。",
            ),
            simplified: Tr::new(
                "Here each block compacts its own cells. Large GPUs also compact across the whole grid, with a global scan.",
                "这里每个块只压缩自己的格子。真实的大型 GPU 还会借助全局扫描跨整个网格压缩。",
            ),
        },
        ..abase("the-compact", 3, Launch::grid(8, 512), CELLS)
    }
}

// ---- 11-4 shuffled-order ---------------------------------------------------

/// out[0..4096]: the list; out[4096]: the count; out[4097..4105]: scratch.
const LIST_OUT: usize = CELLS + 1 + 8;

fn active_mask(seed: u64) -> Vec<i32> {
    let mut r = rng(seed);
    (0..CELLS)
        .map(|_| (below(&mut r, 100) < 20) as i32)
        .collect()
}

fn shuffled_order() -> Task {
    Task {
        title: Tr::new("Shuffled Order", "乱序"),
        brief: Tr::new(
            "rom[0..4096] marks which of the tower's cells are active (about one in five). Yinpo reads her list top-down, so out[0..k] must hold the indices of the active cells in ascending order, and out[4096] the count k. Unused list cells keep their -1. out[4097..4105] is free scratch.\nThe starter hands out slots with an atomic counter. The count is right, but the order is whoever arrived first, and the scheduling seed changes who that is.",
            "rom[0..4096] 标出塔里哪些格子是活跃的（约五分之一）。银婆自上而下读她的清单，所以 out[0..k] 必须按升序放着活跃格的下标，out[4096] 放个数 k。清单里没用到的格子保持 -1。out[4097..4105] 是可随便用的草稿区。\n起始程序用原子计数器发位次：个数是对的，但顺序是谁先到谁在前，而调度种子会改变谁先到。",
        ),
        grid: None,
        cases: || vec![seeded(active_mask(1401), 1), seeded(active_mask(1402), 2)],
        hidden: || {
            vec![
                seeded(active_mask(1403), 3),
                seeded(active_mask(1404), 4),
                seeded(active_mask(1405), 5),
            ]
        },
        init: |_| vec![-1; CELLS],
        expect: |c| {
            let live: Vec<i32> = (0..CELLS)
                .filter(|&i| c.rom[i] != 0)
                .map(|i| i as i32)
                .collect();
            (0..LIST_OUT)
                .map(|i| match i {
                    i if i < live.len() => Some(live[i]),
                    i if i < CELLS => Some(-1),
                    CELLS => Some(live.len() as i32),
                    _ => None,
                })
                .collect()
        },
        limit: Some(Goal::new(Metric::Cycles, 3200)),
        goals: [Goal::new(Metric::Cycles, 2700), Goal::new(Metric::Size, 90)],
        starter: Source::Code(
            "LD r0, [tid]            ; active? | 活跃吗？\nSETP.NE p0, r0, 0\nIF p0\n; take the next slot, in order of arrival | 按到达顺序领下一个位次\nATOM r1, [4096], 1\nST [r1], tid\nENDIF\n",
        ),
        reference: Source::Code(SO_REF),
        clue: Some(Clue {
            what: Tr::new(
                "the first slot taken by a warp that arrived after a higher-numbered warp",
                "第一个位次：它被一个在更高编号 warp 之后才到的 warp 领走",
            ),
            find: crate::grade::first_out_of_order,
        }),
        predict: Predict {
            question: Tr::new(
                "Run the starter twice on the very same mask, with two different scheduling seeds. The two lists will be:",
                "用同一份掩码、两个不同的调度种子，把起始程序跑两遍。两份清单将会：",
            ),
            options: [
                Tr::new("identical", "完全一样"),
                Tr::new("the same cells, in a different order", "格子相同，顺序不同"),
                Tr::new("different cells", "格子也不同"),
            ],
            answer: 1,
            why: Tr::new(
                "Every active cell still gets exactly one slot, so the set is the same. But blocks and warps start in a different order under each seed, so who takes slot 0, 1, 2 ... changes.",
                "每个活跃格仍然恰好领到一个位次，所以集合不变。但在不同的种子下，块和 warp 的起跑顺序不同，谁拿到 0、1、2 号位次也就变了。",
            ),
        },
        hints: [
            Tr::new(
                "An atomic counter only promises each caller a different number, never an order. Where a cell goes must depend on its index alone.",
                "原子计数器只保证每个调用者拿到不同的数，从不保证顺序。一个格子该去哪儿，只能取决于它自己的下标。",
            ),
            Tr::new(
                "Slot = the number of active cells before this one. That is an exclusive scan of the mask: scan inside each warp, scan the 16 warp totals, write each block's total to scratch, GSYNC, and add the totals of the blocks before yours.",
                "位次 = 排在它前面的活跃格个数，也就是掩码的一次独占扫描：先在每个 warp 内扫描，再扫描 16 个 warp 的总数，把每个块的总数写进草稿区，GSYNC，再加上排在你前面的那些块的总数。",
            ),
            Tr::new(
                "GSYNC needs every block resident at once: keep the program to r0..r7 so all 128 warps fit. Thread 4095 knows the grand total (its slot plus its own flag): let it ST out[4096].",
                "GSYNC 要求所有块同时驻留：把程序限制在 r0..r7，这样 128 个 warp 才能全部放下。4095 号线程知道总数（它的位次加上自己的标志）：让它去 ST out[4096]。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Order from arithmetic", "顺序来自算术"),
            line: Tr::new(
                "When the order matters, compute each item's place from its index. Never let arrival order decide.",
                "顺序重要时，用下标算出每一项的位置，绝不能让到达顺序说了算。",
            ),
            reality: Tr::new(
                "Atomic appends are fast and everywhere, but their output order changes from run to run. Deterministic builds use scans, then scatter.",
                "原子追加又快又随处可见，但输出顺序每次运行都会变。要得到确定的结果，就用扫描再散布。",
            ),
            myth: Tr::new(
                "Myth: if it passed on my seed, it works. A race that needs the right timing to hurt is still a race.",
                "误解：在我的种子上过了，就没问题。只在特定时机才出错的竞争，依然是竞争。",
            ),
            simplified: Tr::new(
                "Real GPUs have no seed knob; the order shifts with clocks, load and driver. Here the knob makes it repeatable.",
                "真实 GPU 没有种子旋钮；顺序随时钟、负载和驱动而变。这里的旋钮让它可以重现。",
            ),
        },
        ..abase("shuffled-order", 4, Launch::grid(8, 512), LIST_OUT)
    }
}

// ---- 11-5 radix-pass -------------------------------------------------------

/// `rom[0] = n`, then `n` keys. `low` limits the low two bits to the listed values.
fn keys_case(seed: u64, n: usize, low: &[i32]) -> Vec<i32> {
    let mut r = rng(seed);
    let mut rom = vec![n as i32];
    for _ in 0..n {
        let hi = below(&mut r, 16384);
        let lo = low[below(&mut r, low.len() as i32) as usize];
        rom.push((hi << 2) | lo);
    }
    rom
}

fn radix_pass() -> Task {
    Task {
        title: Tr::new("Radix Pass", "基数一趟"),
        brief: Tr::new(
            "rom[0] is n (at most 512); rom[1..=n] are 16-bit keys. Sort them by their low two bits only, and keep the original order among keys that share the same two bits (a stable pass). Write the sorted keys to out[0..n]; the rest of out stays -1.\nOne block of 512 threads is enough: a thread per key. Yinpo wants four piles: bits 00, 01, 10, 11, in that order.",
            "rom[0] 是 n（最多 512）；rom[1..=n] 是 16 位的键。只按低两位排序，低两位相同的键保持原来的先后次序（稳定的一趟）。把排好的键写进 out[0..n]；out 的其余部分保持 -1。\n一个 512 线程的块就够了：一个线程管一个键。银婆要四堆：低两位 00、01、10、11，按这个顺序。",
        ),
        launch: Launch::flat(512),
        cases: || {
            vec![
                seeded(keys_case(1501, 512, &[0, 1, 2, 3]), 1),
                seeded(keys_case(1502, 512, &[1, 3]), 2),
                seeded(keys_case(1503, 256, &[0, 1, 2, 3]), 3),
            ]
        },
        hidden: || {
            vec![
                seeded(keys_case(1504, 500, &[0, 1, 2, 3]), 5),
                seeded(keys_case(1505, 33, &[0, 1, 2, 3]), 6),
                seeded(keys_case(1506, 300, &[2]), 7),
            ]
        },
        init: |_| vec![-1; 512],
        expect: |c| {
            let n = c.rom[0] as usize;
            let mut keys: Vec<i32> = c.rom[1..=n].to_vec();
            keys.sort_by_key(|k| k & 3);
            (0..512)
                .map(|i| Some(keys.get(i).copied().unwrap_or(-1)))
                .collect()
        },
        limit: Some(Goal::new(Metric::Cycles, 5200)),
        goals: [Goal::new(Metric::Cycles, 4300), Goal::new(Metric::Size, 85)],
        starter: Source::Code(
            "LD r7, [0]              ; n | 个数\nADD r1, tid, 1\nLD r0, [r1]             ; my key | 我的键\nSETP.LT p1, tid, r7     ; do I have a key? | 我有键吗？\n; for each pile 0..3: flag my key, scan the flags, ST to start + rank - 1\n; 对每一堆 0..3：给我的键打标志、扫描标志、ST 到 起点 + 序号 - 1\n",
        ),
        reference: Source::Code(RP_REF),
        predict: Predict {
            question: Tr::new(
                "With count0, count1, count2 and count3 keys in the four piles, pile 2 starts at out position:",
                "四堆里分别有 count0、count1、count2、count3 个键，第 2 堆从 out 的哪个位置开始？",
            ),
            options: [
                Tr::new("count0 + count1", "count0 + count1"),
                Tr::new("count2", "count2"),
                Tr::new("count0", "count0"),
            ],
            answer: 0,
            why: Tr::new(
                "Piles 0 and 1 come first and fill count0 + count1 cells, so pile 2 starts right after them. Each start is the sum of the counts before it, an exclusive scan of the four counts.",
                "第 0、1 两堆排在前面，占 count0 + count1 个位置，第 2 堆紧跟其后。每一堆的起点就是它前面各堆个数之和，也就是四个个数的一次独占扫描。",
            ),
        },
        hints: [
            Tr::new(
                "A key's place = the start of its pile + how many keys of the same pile come before it. Count them with a scan, one pile at a time.",
                "键的位置 = 它所在堆的起点 + 同一堆里排在它前面的键数。一次扫描一堆，把它们数出来。",
            ),
            Tr::new(
                "For pile b: flag = (valid and key AND 3 = b), block-wide inclusive scan of the flags, then flagged keys ST to start + scan - 1. After the pile, start += the pile's total. Threads past n must not count: LD of a missing key gives 0, which looks like pile 0.",
                "对第 b 堆：标志 =（有效且 key AND 3 = b），对标志做整块的包含扫描，然后带标志的键 ST 到 起点 + 扫描值 - 1。这一堆做完后，起点 += 这一堆的总数。超过 n 的线程不能计数：读不到的键会给出 0，看上去像第 0 堆。",
            ),
            Tr::new(
                "Wrap the pile in LOOP 4. Block scan = warp scan with SHFL, lane 31 stores its total in shared memory, BAR, warp 0 scans the 16 totals, BAR, everyone adds its warp's offset. Put a BAR at the loop's end so no warp reads last round's shared words after they are overwritten.",
                "把一堆的处理放进 LOOP 4。块扫描 = 用 SHFL 做 warp 扫描，31 号车道把总数存进共享内存，BAR，0 号 warp 扫描这 16 个总数，BAR，每个线程再加上自己 warp 的偏移。循环末尾放一个 BAR，防止有 warp 还在读上一轮的共享字时它们就被覆盖了。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Counting sort by scan", "用扫描做计数排序"),
            line: Tr::new(
                "A stable sort on a few bits is just flags, scans and one scatter. No comparisons needed.",
                "对少数几位做稳定排序，不过是标志、扫描加一次散布。完全不需要比较。",
            ),
            reality: Tr::new(
                "GPU radix sort repeats this pass on 4 to 8 bits at a time. It is the fastest general sort for integer keys.",
                "GPU 上的基数排序每次对 4 到 8 位重复这一趟，是整数键最快的通用排序。",
            ),
            myth: Tr::new(
                "Myth: sorting needs compare-and-swap. Odd-even sorting needs 512 rounds here; four scans do it in four passes.",
                "误解：排序离不开比较交换。这里的奇偶排序要 512 轮；四次扫描只要四趟。",
            ),
            simplified: Tr::new(
                "Real sorts keep per-block histograms and merge them across blocks; one block keeps the idea visible.",
                "真实的排序会为每个块保存直方图再跨块合并；这里用一个块，让思路一目了然。",
            ),
        },
        ..abase("radix-pass", 5, Launch::flat(512), 512)
    }
}

const RP_REF: &str = concat!(
    "LD r7, [0]\nADD r1, tid, 1\nLD r0, [r1]\nSETP.LT p1, tid, r7\nMOV r3, 0\nMOV r4, 0\n",
    "LOOP 4\nAND r1, r0, 3\nSETP.EQ p0, r1, r3\nSEL r5, p0, 1, 0\nSEL r5, p1, r5, 0\nMOV r6, r5\n",
    "SUB r2, lane, 1\nSHFL r1, r5, r2\nSETP.GE p0, lane, 1\nSEL r1, p0, r1, 0\nADD r5, r5, r1\nSUB r2, lane, 2\n",
    "SHFL r1, r5, r2\nSETP.GE p0, lane, 2\nSEL r1, p0, r1, 0\nADD r5, r5, r1\nSUB r2, lane, 4\nSHFL r1, r5, r2\n",
    "SETP.GE p0, lane, 4\nSEL r1, p0, r1, 0\nADD r5, r5, r1\nSUB r2, lane, 8\nSHFL r1, r5, r2\nSETP.GE p0, lane, 8\n",
    "SEL r1, p0, r1, 0\nADD r5, r5, r1\nSUB r2, lane, 16\nSHFL r1, r5, r2\nSETP.GE p0, lane, 16\nSEL r1, p0, r1, 0\n",
    "ADD r5, r5, r1\nSHR r8, tx, 5\nSETP.EQ p2, lane, 31\nIF p2\nSTS [r8], r5\nENDIF\n",
    "BAR\nSETP.EQ p2, r8, 0\nIF p2\nLDS r9, [lane]\nMOV r10, r9\nMOV r11, 1\n",
    "LOOP 5\nSUB r2, lane, r11\nSHFL r1, r9, r2\nSETP.GE p0, lane, r11\nSEL r1, p0, r1, 0\nADD r9, r9, r1\n",
    "SHL r11, r11, 1\nENDLOOP\nSUB r10, r9, r10\nSTS [lane], r10\nSETP.EQ p3, lane, 15\nIF p3\n",
    "STS [32], r9\nENDIF\nENDIF\nBAR\nLDS r9, [r8]\nLDS r10, [32]\n",
    "ADD r9, r9, r5\nADD r9, r9, r4\nSUB r9, r9, 1\nSETP.NE p2, r6, 0\nIF p2\nST [r9], r0\n",
    "ENDIF\nADD r4, r4, r10\nADD r3, r3, 1\nBAR\nENDLOOP\n",
);

// ---- 11-6 the-csr ----------------------------------------------------------

const ROWS: usize = 128;
/// rom layout shared by 11-6 and 11-7: rowptr, x, col, val.
const X_AT: usize = 160;
const COL_AT: usize = 288;
const NNZ_MAX: usize = 8192;
const VAL_AT: usize = COL_AT + NNZ_MAX;

/// A CSR matrix of `lens.len()` rows in the shared layout.
fn csr_rom(seed: u64, lens: &[usize]) -> Vec<i32> {
    let mut r = rng(seed);
    let nnz: usize = lens.iter().sum();
    assert!(lens.len() == ROWS && nnz <= NNZ_MAX);
    let mut rom = vec![0; VAL_AT + nnz];
    let mut at = 0;
    for (i, &l) in lens.iter().enumerate() {
        rom[i] = at as i32;
        at += l;
    }
    rom[ROWS] = at as i32;
    for x in &mut rom[X_AT..X_AT + ROWS] {
        *x = below(&mut r, 19) - 9;
    }
    for k in 0..nnz {
        rom[COL_AT + k] = below(&mut r, ROWS as i32);
        rom[VAL_AT + k] = below(&mut r, 19) - 9;
    }
    rom
}

fn csr_expect(c: &Case) -> Vec<Option<i32>> {
    (0..ROWS)
        .map(|i| {
            let (a, b) = (c.rom[i] as usize, c.rom[i + 1] as usize);
            Some(
                (a..b)
                    .map(|k| c.rom[VAL_AT + k] * c.rom[X_AT + c.rom[COL_AT + k] as usize])
                    .sum(),
            )
        })
        .collect()
}

fn lens_fixed(l: usize) -> Vec<usize> {
    vec![l; ROWS]
}

fn lens_mixed(seed: u64) -> Vec<usize> {
    let mut r = rng(seed);
    (0..ROWS)
        .map(|_| [2, 4, 8, 16, 32, 64][below(&mut r, 6) as usize])
        .collect()
}

fn lens_geometric(seed: u64) -> Vec<usize> {
    let mut r = rng(seed);
    (0..ROWS)
        .map(|_| {
            let mut l = 1;
            while below(&mut r, 100) < 88 && l < 64 {
                l += 1;
            }
            l
        })
        .collect()
}

fn the_csr() -> Task {
    Task {
        title: Tr::new("The CSR", "稀疏行"),
        brief: Tr::new(
            "Yinpo's tower stores a sparse 128-row matrix A in compressed rows: row i owns the entries rowptr[i]..rowptr[i+1] (rom[0..=128]). Entry k has a column col[k] (rom[288 + k]) and a value val[k] (rom[8480 + k]). The vector x is rom[160..288].\nCompute y = A x: out[i] = sum of val[k] * x[col[k]] over row i. Empty rows give 0.\nThe tower gives one warp per row (8 blocks of 512 threads: warp i owns row i). The starter gives each row to one thread; it is correct, and slow.",
            "银婆的塔里用压缩行格式存着一个 128 行的稀疏矩阵 A：第 i 行拥有 rowptr[i]..rowptr[i+1] 这段条目（rom[0..=128]）。第 k 个条目有列号 col[k]（rom[288 + k]）和数值 val[k]（rom[8480 + k]）。向量 x 是 rom[160..288]。\n计算 y = A x：out[i] = 第 i 行所有 val[k] * x[col[k]] 之和。空行得 0。\n风塔给每一行一个 warp（8 个块、每块 512 线程：第 i 个 warp 管第 i 行）。起始程序让一个线程独自管一行，结果对，却很慢。",
        ),
        launch: Launch::grid(8, 512),
        cases: || {
            vec![
                seeded(csr_rom(1601, &lens_fixed(8)), 1),
                seeded(csr_rom(1602, &lens_mixed(1602)), 2),
                seeded(csr_rom(1603, &lens_geometric(1603)), 3),
            ]
        },
        hidden: || {
            let mut long = lens_fixed(8);
            long[77] = 200;
            let mut holes = lens_geometric(1608);
            for l in holes.iter_mut().step_by(3) {
                *l = 0;
            }
            vec![
                seeded(csr_rom(1604, &long), 5),
                seeded(csr_rom(1605, &holes), 6),
                seeded(csr_rom(1606, &lens_fixed(1)), 7),
            ]
        },
        expect: csr_expect,
        limit: Some(Goal::new(Metric::Cycles, 8500)),
        goals: [Goal::new(Metric::Cycles, 7300), Goal::new(Metric::Size, 42)],
        starter: Source::Code(CSR_STARTER),
        reference: Source::Code(CSR_REF),
        predict: Predict {
            question: Tr::new(
                "One thread per row: a warp's 32 rows all have different lengths, the longest being 64. How many rounds does that warp run?",
                "一线程一行：一个 warp 的 32 行长度各不相同，最长的是 64。这个 warp 要跑多少轮？",
            ),
            options: [
                Tr::new("the average length", "平均长度"),
                Tr::new("the longest length: 64", "最长的长度：64"),
                Tr::new("the sum of the 32 lengths", "32 个长度之和"),
            ],
            answer: 1,
            why: Tr::new(
                "The lanes march together. Short rows finish early and sit masked until the longest row is done, so the warp pays for the maximum, not the average.",
                "车道们同步前进。短的行早早做完，只能被屏蔽着干等，直到最长的那一行结束，所以 warp 付的是最大值，不是平均值。",
            ),
        },
        hints: [
            Tr::new(
                "Give the row to the whole warp instead of one lane: the 32 lanes take entries k = start + lane, then +32, +64 ...",
                "把一行交给整个 warp，而不是一条车道：32 条车道取条目 k = start + lane，然后 +32、+64……",
            ),
            Tr::new(
                "Each lane keeps a private partial sum over its entries; a loop of 8 rounds covers rows up to 256 entries, with an IF that skips k beyond the row's end. At the end a SHFL butterfly adds the 32 partial sums.",
                "每条车道对自己负责的条目保存一个私有部分和；循环 8 轮可覆盖最长 256 项的行，用 IF 跳过超出行尾的 k。最后用 SHFL 蝶形把 32 个部分和加起来。",
            ),
            Tr::new(
                "row = warp. start = LD [row], end = LD [row + 1]. In the loop: col = LD [288 + k], xv = LD [160 + col], v = LD [8480 + k], acc += v * xv. After the butterfly, lane 0 does ST [row], acc; an empty row simply stores 0.",
                "row = warp。start = LD [row]，end = LD [row + 1]。循环里：col = LD [288 + k]，xv = LD [160 + col]，v = LD [8480 + k]，acc += v * xv。蝶形之后，0 号车道做 ST [row], acc；空行自然就存 0。",
            ),
        ],
        insight: Insight {
            title: Tr::new("A warp per row", "一行一个 warp"),
            line: Tr::new(
                "Uneven rows punish a thread per row; a warp per row reads them side by side and loses only the tail.",
                "长短不一的行会惩罚“一线程一行”；“一行一个 warp”让相邻车道并排读取，只损失尾巴。",
            ),
            reality: Tr::new(
                "This is the CSR-vector kernel behind sparse solvers and graph analytics, and the reason sparse formats come in several flavours.",
                "这就是稀疏求解器和图分析背后的 CSR-vector 核，也是稀疏格式有好几种的原因。",
            ),
            myth: Tr::new(
                "Myth: more threads always means more speed. A thread per row has 32 times the parallelism and still loses to a warp per row.",
                "误解：线程越多越快。一线程一行的并行度高 32 倍，却输给了一行一个 warp。",
            ),
            simplified: Tr::new(
                "Real kernels pick the strategy per matrix from its row-length statistics.",
                "真实的核会根据矩阵的行长统计，为每个矩阵挑选策略。",
            ),
        },
        ..abase("the-csr", 6, Launch::grid(8, 512), ROWS)
    }
}

// ---- 11-7 the-hub ----------------------------------------------------------

/// Rows of about eight entries (4..=11), with some of them replaced by hubs.
fn lens_hubs(seed: u64, hubs: &[(usize, usize)]) -> Vec<usize> {
    let mut r = rng(seed);
    let mut lens: Vec<usize> = (0..ROWS).map(|_| 4 + below(&mut r, 8) as usize).collect();
    for &(row, len) in hubs {
        lens[row] = len;
    }
    lens
}

fn the_hub() -> Task {
    Task {
        title: Tr::new("The Hub", "大节点"),
        brief: Tr::new(
            "The same sparse matrix as before (same layout: rowptr, x, col, val), but one row is a hub: three to four thousand entries, while the others have about eight.\nThe tower now wakes 2048 warps. Warp w can be row w >> 4, segment w AND 15 (segments are 256 entries long). The starter uses only the first warp of each row (w AND 15 = 0), and is correct; but while every other warp has long gone home, one warp walks the hub alone.\nCompute y = A x into out[0..128] again, but make the hub fast. out starts at 0, so ATOM can add into it.",
            "还是那个稀疏矩阵（布局相同：rowptr、x、col、val），但其中有一行是大节点：三四千个条目，而其他行只有八个左右。\n风塔现在唤醒了 2048 个 warp。第 w 个 warp 可以当作第 w >> 4 行的第 w AND 15 段（每段 256 个条目）。起始程序只用每行的第一个 warp（w AND 15 = 0），结果是对的；但当别的 warp 早已收工回家，却只剩一个 warp 在独自走完大节点。\n仍然把 y = A x 写进 out[0..128]，但要让大节点变快。out 初始为 0，所以可以用 ATOM 往里加。",
        ),
        launch: Launch::grid(2048, 32),
        cases: || {
            vec![
                seeded(csr_rom(1701, &lens_hubs(1701, &[(0, 3500)])), 1),
                seeded(csr_rom(1702, &lens_hubs(1702, &[(77, 4000)])), 2),
            ]
        },
        hidden: || {
            vec![
                seeded(
                    csr_rom(1703, &lens_hubs(1703, &[(20, 3000), (99, 2600)])),
                    5,
                ),
                seeded(csr_rom(1704, &lens_hubs(1704, &[(127, 3800)])), 6),
                seeded(csr_rom(1705, &lens_hubs(1705, &[])), 7),
            ]
        },
        expect: csr_expect,
        limit: Some(Goal::new(Metric::Cycles, 18000)),
        goals: [
            Goal::new(Metric::Cycles, 15000),
            Goal::new(Metric::Size, 52),
        ],
        starter: Source::Code(HUB_STARTER),
        reference: Source::Code(HUB_REF),
        clue: Some(Clue {
            what: Tr::new(
                "the first moment only one warp is still running",
                "第一次只剩一个 warp 还在运行的时刻",
            ),
            find: crate::grade::first_lonely,
        }),
        predict: Predict {
            question: Tr::new(
                "One warp walks a 4000-entry hub row alone. If the blocks are made twice as big, the time that row takes will:",
                "一个 warp 独自走完一条 4000 项的大节点行。如果把块开成两倍大，这一行要花的时间将会：",
            ),
            options: [
                Tr::new("halve", "减半"),
                Tr::new("stay the same", "保持不变"),
                Tr::new("double", "翻倍"),
            ],
            answer: 1,
            why: Tr::new(
                "A row belongs to one warp, and a warp walks its row in order, 32 entries at a time. Bigger blocks bring more warps, but none of them is given a piece of this row.",
                "一行只属于一个 warp，而一个 warp 是按顺序、一次 32 个条目地走完它的行。块更大只是多来了几个 warp，但没有一个会分到这一行的一部分。",
            ),
        },
        hints: [
            Tr::new(
                "The hub needs more than one warp. The idle warps are already launched: warp w can work on row w >> 4, segment w AND 15.",
                "大节点需要不止一个 warp。空闲的 warp 其实已经启动了：第 w 个 warp 可以去做第 w >> 4 行的第 w AND 15 段。",
            ),
            Tr::new(
                "A segment is 256 entries: s0 = rowptr[row] + 256 * seg, s1 = min(rowptr[row + 1], s0 + 256). A warp whose s0 is not below the row's end has nothing to do and should skip everything. Otherwise it sums its segment as in the last level, a butterfly adds the lanes, and lane 0 does ATOM into out[row].",
                "一段是 256 个条目：s0 = rowptr[row] + 256 * seg，s1 = min(rowptr[row + 1], s0 + 256)。如果 s0 不小于该行末尾，这个 warp 无事可做，应当整个跳过。否则它像上一关那样累加自己的这一段，用蝶形把各车道加起来，再由 0 号车道对 out[row] 做 ATOM。",
            ),
            Tr::new(
                "row = warp >> 4; seg256 = (warp AND 15) << 8. Wrap everything in IF s0 < end. Loop 8 rounds of 32 entries with an IF k < s1 inside. Short rows only ever use segment 0; empty rows never pass the outer IF, so their y stays 0.",
                "row = warp >> 4；seg256 = (warp AND 15) << 8。把全部内容包进 IF s0 < end。循环 8 轮、每轮 32 个条目，里面再放一个 IF k < s1。短行永远只会用到第 0 段；空行过不了外层 IF，所以它们的 y 保持为 0。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Split the big job", "拆开大的那一件"),
            line: Tr::new(
                "Parallel time is the slowest worker's time. Cut the biggest job into pieces until no piece is the biggest.",
                "并行的时间就是最慢那位工人的时间。把最大的活儿切成小块，直到没有哪一块特别大。",
            ),
            reality: Tr::new(
                "Social graphs and web graphs have a few hub nodes with millions of edges. Graph engines split them across warps or blocks and merge the partial sums with atomics.",
                "社交图和网页图里总有几个拥有百万条边的大节点。图计算引擎会把它们拆给多个 warp 或块，再用原子操作合并部分和。",
            ),
            myth: Tr::new(
                "Myth: a big job just needs a bigger block. A block is a group of workers; the job must still be cut.",
                "误解：大任务只需要更大的块。块只是一群工人；活儿本身仍然得切开。",
            ),
            simplified: Tr::new(
                "The segment size is fixed at 256 here; real engines pick it from the degree distribution.",
                "这里的段长固定为 256；真实的引擎会根据度数分布来选择。",
            ),
        },
        ..abase("the-hub", 7, Launch::grid(2048, 32), ROWS)
    }
}

// ---- 11-8 storm-surge ------------------------------------------------------

const SIDE: usize = 64;
const SURGE_STEPS: usize = 96;
/// The padded map a dense sweep keeps in scratch: one ring of dry cells around the 64 x 64.
const PADDED: usize = (SIDE + 2) * (SIDE + 2);
/// out[0..4096] is the mask; the rest is free scratch (two padded maps).
const SURGE_OUT: usize = CELLS + 2 * PADDED;

/// A terrain case: rom[0] = source cell, rom[1] = water level, rom[32 + i] = height of cell i.
fn flood_case(h: Vec<i32>, src: (usize, usize), level: i32, frame: i32) -> Case {
    let mut rom = vec![0; 32];
    rom[0] = (src.1 * SIDE + src.0) as i32;
    rom[1] = level;
    rom.extend(h);
    seeded(rom, frame)
}

/// Heights from a formula of (x, y) plus noise in `0..noise`.
fn terrain(seed: u64, noise: i32, f: impl Fn(i32, i32) -> i32) -> Vec<i32> {
    let mut r = rng(seed);
    (0..CELLS)
        .map(|i| f((i % SIDE) as i32, (i / SIDE) as i32) + below(&mut r, noise))
        .collect()
}

fn valley(seed: u64) -> Vec<i32> {
    terrain(seed, 3, |x, y| 3 * (x - 32).abs() + y / 4)
}

/// Two bowls with a two-cell dam at x = 31, 32 between them.
fn dammed(seed: u64, crest: i32) -> Vec<i32> {
    let mut h = terrain(seed, 4, |x, _| 2 * (x - if x < 32 { 15 } else { 48 }).abs());
    for y in 0..SIDE {
        h[y * SIDE + 31] = crest;
        h[y * SIDE + 32] = crest;
    }
    h
}

/// Flat ground with a few hills.
fn plain(seed: u64) -> Vec<i32> {
    let mut h = terrain(seed, 10, |_, _| 20);
    let mut r = rng(seed ^ 0x51);
    for _ in 0..12 {
        let (x0, y0) = (below(&mut r, 60) as usize, below(&mut r, 60) as usize);
        for y in y0..y0 + 4 {
            for x in x0..x0 + 4 {
                h[y * SIDE + x] = 90;
            }
        }
    }
    h
}

/// A one-cell channel that snakes down the map; everything else is a wall.
fn winding() -> Vec<i32> {
    let mut h = vec![99; CELLS];
    for k in 0..16 {
        let y = 4 * k + 1;
        for x in 1..=62 {
            h[y * SIDE + x] = 10;
        }
        let bend = if k % 2 == 0 { 62 } else { 1 };
        for dy in 0..4.min(SIDE - y) {
            h[(y + dy) * SIDE + bend] = 10;
        }
    }
    h
}

/// A crest column at exactly the water level; the source sits next to it.
fn crested(seed: u64, level: i32) -> Vec<i32> {
    let mut h = terrain(seed, 3, |x, y| 2 * (y - 32).abs() + (x - 32).abs() / 4);
    for y in 0..SIDE {
        h[y * SIDE + 32] = level;
    }
    h
}

fn flood_expect(c: &Case) -> Vec<Option<i32>> {
    let src = c.rom[0] as usize;
    let level = c.rom[1];
    let mut wet = vec![false; CELLS];
    wet[src] = true;
    let mut rim = vec![src];
    for _ in 0..SURGE_STEPS {
        let mut next = Vec::new();
        for &i in &rim {
            let (x, y) = (i % SIDE, i / SIDE);
            let mut around = Vec::new();
            if x > 0 {
                around.push(i - 1);
            }
            if x + 1 < SIDE {
                around.push(i + 1);
            }
            if y > 0 {
                around.push(i - SIDE);
            }
            if y + 1 < SIDE {
                around.push(i + SIDE);
            }
            for j in around {
                if !wet[j] && c.rom[32 + j] <= level {
                    wet[j] = true;
                    next.push(j);
                }
            }
        }
        rim = next;
    }
    let mut want: Vec<Option<i32>> = wet.into_iter().map(|w| Some(w as i32)).collect();
    want.resize(SURGE_OUT, None);
    want
}

/// The showcase: the valley, with the water level and the source moved by the point.
fn surge_live(_frame: i32, at: [i32; 2]) -> Case {
    let level = (13 + 8 * at[0]).clamp(0, 99);
    let x = (32 + 4 * (at[1] - 4)).clamp(0, 63) as usize;
    flood_case(valley(1800), (x, 0), level, 0)
}

fn storm_surge() -> Task {
    Task {
        title: Tr::new("Storm Surge", "山洪"),
        brief: Tr::new(
            "Yinpo is simulating a flash flood from the wind tower. The map is 64 x 64 cells, cell i = y * 64 + x. rom[0] is the water source cell, rom[1] the water level L, and rom[32 + i] the height of cell i.\nThe source is wet from the start, whatever its height. Each step, every wet cell wets its four neighbours (left, right, up, down), but only those whose height is at most L. Run exactly 96 steps. Then out[i] = 1 for every wet cell and 0 for the rest (out[0..4096]).\nout[4096..] is free scratch, 8712 cells. The tower wakes 8 blocks of 512 threads, so one thread per cell is on offer. There is no starter. The sweep is yours to design, and the frame budget is tight.",
            "银婆在风塔上模拟一场山洪。地图是 64 x 64 的格子，第 i 格是 y * 64 + x。rom[0] 是水源格，rom[1] 是水位线 L，rom[32 + i] 是第 i 格的高度。\n水源一开始就是湿的，不管它多高。每一步，每个湿格都会淹到它的四个邻格（左、右、上、下），但只淹高度不超过 L 的格。恰好走 96 步。然后每个湿格的 out[i] = 1，其余为 0（out[0..4096]）。\nout[4096..] 是可随意使用的草稿区，共 8712 格。风塔唤醒了 8 个块、每块 512 个线程，所以每格一个线程是现成的选择。这一关没有起始程序。怎么扫由你设计，而帧预算很紧。",
        ),
        cases: || {
            vec![
                flood_case(valley(1801), (32, 0), 45, 1),
                flood_case(dammed(1802, 90), (15, 32), 40, 2),
                flood_case(plain(1803), (32, 32), 30, 3),
            ]
        },
        hidden: || {
            vec![
                flood_case(winding(), (1, 1), 50, 4),
                flood_case(crested(1804, 60), (31, 32), 60, 5),
                flood_case(valley(1805), (60, 60), 20, 6),
                flood_case(plain(1806), (9, 50), 30, 7),
            ]
        },
        init: |_| vec![0; SURGE_OUT],
        expect: flood_expect,
        limit: Some(Goal::new(Metric::Cycles, 100_000)),
        goals: [
            Goal::new(Metric::Cycles, 85_000),
            Goal::new(Metric::Cycles, 30_000),
        ],
        starter: Source::Code(""),
        reference: Source::Code(SS_REF),
        live: Some(surge_live),
        steer: Some(Tr::new("RAISE THE WATER", "抬高水位")),
        predict: Predict {
            question: Tr::new(
                "The flood has run for 30 steps. How many cells can newly turn wet in the next step?",
                "洪水已经流了 30 步。下一步最多有多少格会新变湿？",
            ),
            options: [
                Tr::new("Any of the 4096", "4096 格中的任何一格"),
                Tr::new(
                    "Only cells next to the water's rim: at most about 4 x 30",
                    "只有紧挨着水边的格：至多约 4 x 30 格",
                ),
                Tr::new("Exactly one", "恰好一格"),
            ],
            answer: 1,
            why: Tr::new(
                "A dry cell can only get wet from a wet neighbour, so only the rim can grow. On open ground the rim is a diamond, and a diamond of radius r has 4r cells. On a narrow channel it is a single cell.",
                "干格只会被湿的邻格淹到，所以只有水边会扩张。在开阔地上，水边是一个菱形，半径 r 的菱形只有 4r 格。在窄水渠里，它只有一格。",
            ),
        },
        hints: [
            Tr::new(
                "The plain way is fine: one thread per cell, two copies of the map in scratch, and a GSYNC after every step. It works, but 96 sweeps read 4096 cells five times each, and it earns two stars at best.",
                "朴素的做法没问题：每格一个线程，草稿区里放两份地图，每一步之后 GSYNC。它能行，但 96 次扫描要把 4096 格各读五遍，最多拿到两颗星。",
            ),
            Tr::new(
                "Only the rim grows. Keep the rim as a queue in shared memory: each step, one thread per rim cell tries to claim its neighbours with ATOMS, and a winner appends the new cell with a tail counter. One block is enough, and the other warps can simply leave early. Two BARs per step: one after everybody has read the tail, one after the appends.",
                "只有水边会扩张。把水边存成共享内存里的队列：每一步，每个水边格用一个线程，试着用 ATOMS 抢占它的邻格，抢到的线程用队尾计数器把新格追加进去。一个块就够了，其余的 warp 可以直接提前退出。每一步两个 BAR：全员读完队尾后一个，追加完之后一个。",
            ),
            Tr::new(
                "Keep a 66 x 66 map in shared memory with a ring of dead cells around it, so no neighbour needs a bounds check: 1 means dry and low, 0 means wall or already wet. ATOMS with -1 returns the old value; old == 1 means you won the cell. The rim never exceeds 128 cells, so 128 threads cover it. Fill the map first with all 512 threads of block 0.",
                "在共享内存里放一张 66 x 66 的地图，四周围一圈死格，邻格就不用做边界检查：1 表示又干又低，0 表示墙或已经湿了。ATOMS 加 -1 会返回旧值；旧值等于 1 就说明你抢到了这一格。水边永远不会超过 128 格，所以 128 个线程就够。先用 0 号块的 512 个线程把地图填好。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Work where the water is", "只在有水的地方干活"),
            line: Tr::new(
                "When little changes per step, keep a list of what changed. A rim of a few dozen cells beats a sweep of four thousand.",
                "每一步变化很少时，就记下变了什么。几十格的水边，比四千格的整片扫描划算。",
            ),
            reality: Tr::new(
                "Breadth-first search, flood fill and sparse solvers on GPUs all keep a frontier queue built with atomic counters or scans, and switch to a dense sweep only when the frontier becomes huge.",
                "GPU 上的广度优先搜索、洪水填充和稀疏求解器都维护一个用原子计数器或扫描构建的前沿队列，只有前沿变得很大时才切换为整片扫描。",
            ),
            myth: Tr::new(
                "Myth: using every thread on every step is always fastest. With a grid barrier each step, most of the machine waits to do nothing useful.",
                "误解：每一步都动用全部线程总是最快的。每步一次全网格屏障，机器的大部分只是在等待，什么有用的事也没做。",
            ),
            simplified: Tr::new(
                "Real engines also run a frontier across blocks. Here the rim always fits one block of 128 threads, so a single block does it.",
                "真实的引擎也会让前沿跨块运行。这里水边总是能放进一个 128 线程的块，所以一个块就够了。",
            ),
        },
        ..abase("storm-surge", 8, Launch::grid(8, 512), SURGE_OUT)
    }
}

const SO_REF: &str = concat!(
    "LD r0, [tid]\nMOV r3, r0\nSUB r2, lane, 1\nSHFL r1, r0, r2\nSETP.GE p0, lane, 1\nSEL r1, p0, r1, 0\n",
    "ADD r0, r0, r1\nSUB r2, lane, 2\nSHFL r1, r0, r2\nSETP.GE p0, lane, 2\nSEL r1, p0, r1, 0\nADD r0, r0, r1\n",
    "SUB r2, lane, 4\nSHFL r1, r0, r2\nSETP.GE p0, lane, 4\nSEL r1, p0, r1, 0\nADD r0, r0, r1\nSUB r2, lane, 8\n",
    "SHFL r1, r0, r2\nSETP.GE p0, lane, 8\nSEL r1, p0, r1, 0\nADD r0, r0, r1\nSUB r2, lane, 16\nSHFL r1, r0, r2\n",
    "SETP.GE p0, lane, 16\nSEL r1, p0, r1, 0\nADD r0, r0, r1\nSHR r4, tx, 5\nSETP.EQ p1, lane, 31\nIF p1\n",
    "STS [r4], r0\nENDIF\nBAR\nSETP.EQ p2, r4, 0\nIF p2\nLDS r5, [lane]\n",
    "MOV r6, r5\nMOV r7, 1\nLOOP 5\nSUB r2, lane, r7\nSHFL r1, r5, r2\nSETP.GE p0, lane, r7\n",
    "SEL r1, p0, r1, 0\nADD r5, r5, r1\nSHL r7, r7, 1\nENDLOOP\nSETP.EQ p3, lane, 15\nADD r7, bx, 4097\n",
    "IF p3\nST [r7], r5\nENDIF\nSUB r5, r5, r6\nENDIF\nGSYNC\n",
    "IF p2\nADD r7, lane, 4097\nLDO r6, [r7]\nSETP.LT p3, lane, bx\nSEL r6, p3, r6, 0\nMOV r7, 16\n",
    "LOOP 5\nXOR r2, lane, r7\nSHFL r1, r6, r2\nADD r6, r6, r1\nSHR r7, r7, 1\nENDLOOP\n",
    "ADD r5, r5, r6\nSTS [lane], r5\nENDIF\nBAR\nLDS r6, [r4]\nADD r6, r6, r0\n",
    "SUB r7, r6, r3\nSETP.NE p0, r3, 0\nIF p0\nST [r7], tid\nENDIF\nSETP.EQ p1, tid, 4095\n",
    "IF p1\nST [4096], r6\nENDIF\n",
);

const CSR_STARTER: &str = concat!(
    "; starter: one thread per row, correct but slow | 起点：每行一个线程，结果对但太慢\n",
    "SETP.LT p3, tid, 128\n",
    "IF p3\n",
    "LD r1, [tid]\n",
    "ADD r2, tid, 1\n",
    "LD r2, [r2]\n",
    "MOV r5, 0\n",
    "LOOP 4\n",
    "LOOP 64\n",
    "SETP.LT p0, r1, r2\n",
    "IF p0\n",
    "ADD r4, r1, 288\n",
    "LD r4, [r4]\n",
    "ADD r4, r4, 160\n",
    "LD r4, [r4]\n",
    "ADD r6, r1, 8480\n",
    "LD r6, [r6]\n",
    "MUL r4, r4, r6\n",
    "ADD r5, r5, r4\n",
    "ADD r1, r1, 1\n",
    "ENDIF\n",
    "ENDLOOP\n",
    "ENDLOOP\n",
    "ST [tid], r5\n",
    "ENDIF\n",
);

const CSR_REF: &str = concat!(
    "LD r1, [warp]\n",
    "ADD r2, warp, 1\n",
    "LD r2, [r2]\n",
    "MOV r5, 0\n",
    "ADD r3, r1, lane\n",
    "LOOP 8\n",
    "SETP.LT p0, r3, r2\n",
    "IF p0\n",
    "ADD r4, r3, 288\n",
    "LD r4, [r4]\n",
    "ADD r4, r4, 160\n",
    "LD r4, [r4]\n",
    "ADD r6, r3, 8480\n",
    "LD r6, [r6]\n",
    "MUL r4, r4, r6\n",
    "ADD r5, r5, r4\n",
    "ENDIF\n",
    "ADD r3, r3, 32\n",
    "ENDLOOP\n",
    "XOR r2, lane, 16\n",
    "SHFL r1, r5, r2\n",
    "ADD r5, r5, r1\n",
    "XOR r2, lane, 8\n",
    "SHFL r1, r5, r2\n",
    "ADD r5, r5, r1\n",
    "XOR r2, lane, 4\n",
    "SHFL r1, r5, r2\n",
    "ADD r5, r5, r1\n",
    "XOR r2, lane, 2\n",
    "SHFL r1, r5, r2\n",
    "ADD r5, r5, r1\n",
    "XOR r2, lane, 1\n",
    "SHFL r1, r5, r2\n",
    "ADD r5, r5, r1\n",
    "SETP.EQ p1, lane, 0\n",
    "IF p1\n",
    "ST [warp], r5\n",
    "ENDIF\n",
);

const HUB_STARTER: &str = concat!(
    "; starter: one warp per row, the hub row is walked alone | 起点：每行一个线程束，枢纽行只能独自走完\n",
    "AND r0, warp, 15\n",
    "SETP.EQ p3, r0, 0\n",
    "IF p3\n",
    "SHR r0, warp, 4\n",
    "LD r1, [r0]\n",
    "ADD r2, r0, 1\n",
    "LD r2, [r2]\n",
    "MOV r5, 0\n",
    "ADD r1, r1, lane\n",
    "LOOP 2\n",
    "LOOP 64\n",
    "SETP.LT p0, r1, r2\n",
    "IF p0\n",
    "ADD r4, r1, 288\n",
    "LD r4, [r4]\n",
    "ADD r4, r4, 160\n",
    "LD r4, [r4]\n",
    "ADD r6, r1, 8480\n",
    "LD r6, [r6]\n",
    "MUL r4, r4, r6\n",
    "ADD r5, r5, r4\n",
    "ENDIF\n",
    "ADD r1, r1, 32\n",
    "ENDLOOP\n",
    "ENDLOOP\n",
    "XOR r7, lane, 16\n",
    "SHFL r6, r5, r7\n",
    "ADD r5, r5, r6\n",
    "XOR r7, lane, 8\n",
    "SHFL r6, r5, r7\n",
    "ADD r5, r5, r6\n",
    "XOR r7, lane, 4\n",
    "SHFL r6, r5, r7\n",
    "ADD r5, r5, r6\n",
    "XOR r7, lane, 2\n",
    "SHFL r6, r5, r7\n",
    "ADD r5, r5, r6\n",
    "XOR r7, lane, 1\n",
    "SHFL r6, r5, r7\n",
    "ADD r5, r5, r6\n",
    "SETP.EQ p1, lane, 0\n",
    "IF p1\n",
    "ST [r0], r5\n",
    "ENDIF\n",
    "ENDIF\n",
);

const HUB_REF: &str = concat!(
    "SHR r0, warp, 4\n",
    "AND r1, warp, 15\n",
    "SHL r1, r1, 8\n",
    "LD r2, [r0]\n",
    "ADD r3, r0, 1\n",
    "LD r3, [r3]\n",
    "ADD r2, r2, r1\n",
    "SETP.LT p0, r2, r3\n",
    "IF p0\n",
    "ADD r4, r2, 256\n",
    "MIN r4, r4, r3\n",
    "ADD r2, r2, lane\n",
    "MOV r5, 0\n",
    "LOOP 8\n",
    "SETP.LT p1, r2, r4\n",
    "IF p1\n",
    "ADD r6, r2, 288\n",
    "LD r6, [r6]\n",
    "ADD r7, r2, 8480\n",
    "LD r7, [r7]\n",
    "ADD r6, r6, 160\n",
    "LD r6, [r6]\n",
    "MUL r6, r6, r7\n",
    "ADD r5, r5, r6\n",
    "ENDIF\n",
    "ADD r2, r2, 32\n",
    "ENDLOOP\n",
    "XOR r3, lane, 16\n",
    "SHFL r1, r5, r3\n",
    "ADD r5, r5, r1\n",
    "XOR r3, lane, 8\n",
    "SHFL r1, r5, r3\n",
    "ADD r5, r5, r1\n",
    "XOR r3, lane, 4\n",
    "SHFL r1, r5, r3\n",
    "ADD r5, r5, r1\n",
    "XOR r3, lane, 2\n",
    "SHFL r1, r5, r3\n",
    "ADD r5, r5, r1\n",
    "XOR r3, lane, 1\n",
    "SHFL r1, r5, r3\n",
    "ADD r5, r5, r1\n",
    "SETP.EQ p2, lane, 0\n",
    "IF p2\n",
    "ATOM r6, [r0], r5\n",
    "ENDIF\n",
    "ENDIF\n",
);

const SS_REF: &str = concat!(
    "; reference: wet-rim queue in one block | 参考：单块里的湿边队列\n",
    "SETP.LT p3, tid, 512\n",
    "IF p3\n",
    "LD r0, [1]\n",
    "MOV r11, tid\n",
    "LOOP 4\n",
    "ADD r10, r11, 32\n",
    "LD r10, [r10]\n",
    "ADD r12, r11, 544\n",
    "LD r12, [r12]\n",
    "SHR r3, r11, 6\n",
    "SHL r3, r3, 1\n",
    "ADD r3, r3, r11\n",
    "ADD r3, r3, 67\n",
    "SETP.LE p0, r10, r0\n",
    "SEL r10, p0, 1, 0\n",
    "STS [r3], r10\n",
    "SETP.LE p0, r12, r0\n",
    "SEL r12, p0, 1, 0\n",
    "ADD r13, r3, 528\n",
    "STS [r13], r12\n",
    "ADD r11, r11, 1024\n",
    "ENDLOOP\n",
    "BAR\n",
    "LD r4, [0]\n",
    "SHR r3, r4, 6\n",
    "SHL r3, r3, 1\n",
    "ADD r3, r3, r4\n",
    "ADD r3, r3, 67\n",
    "STS [r3], 0\n",
    "STS [4500], r4\n",
    "STS [5600], 1\n",
    "ST [r4], 1\n",
    "ENDIF\n",
    "SETP.LT p3, tid, 128\n",
    "IF p3\n",
    "SHL r11, tx, 2\n",
    "ADD r11, r11, 6000\n",
    "MOV r1, 0\n",
    "LOOP 6\n",
    "LOOP 16\n",
    "LDS r2, [5600]\n",
    "BAR\n",
    "ADD r3, r1, tx\n",
    "SETP.LT p0, r3, r2\n",
    "AND r3, r3, 1023\n",
    "ADD r3, r3, 4500\n",
    "LDS r4, [r3]\n",
    "SHR r5, r4, 6\n",
    "SHL r5, r5, 1\n",
    "ADD r5, r5, r4\n",
    "ADD r5, r5, 67\n",
    "SEL r5, p0, r5, r11\n",
    "SUB r6, r5, 1\n",
    "ATOMS r7, [r6], -1\n",
    "SETP.EQ p1, r7, 1\n",
    "IF p1\n",
    "SUB r8, r4, 1\n",
    "ST [r8], 1\n",
    "ATOMS r9, [5600], 1\n",
    "AND r9, r9, 1023\n",
    "ADD r9, r9, 4500\n",
    "STS [r9], r8\n",
    "ENDIF\n",
    "ADD r6, r5, 1\n",
    "ATOMS r7, [r6], -1\n",
    "SETP.EQ p1, r7, 1\n",
    "IF p1\n",
    "ADD r8, r4, 1\n",
    "ST [r8], 1\n",
    "ATOMS r9, [5600], 1\n",
    "AND r9, r9, 1023\n",
    "ADD r9, r9, 4500\n",
    "STS [r9], r8\n",
    "ENDIF\n",
    "SUB r6, r5, 66\n",
    "ATOMS r7, [r6], -1\n",
    "SETP.EQ p1, r7, 1\n",
    "IF p1\n",
    "SUB r8, r4, 64\n",
    "ST [r8], 1\n",
    "ATOMS r9, [5600], 1\n",
    "AND r9, r9, 1023\n",
    "ADD r9, r9, 4500\n",
    "STS [r9], r8\n",
    "ENDIF\n",
    "ADD r6, r5, 66\n",
    "ATOMS r7, [r6], -1\n",
    "SETP.EQ p1, r7, 1\n",
    "IF p1\n",
    "ADD r8, r4, 64\n",
    "ST [r8], 1\n",
    "ATOMS r9, [5600], 1\n",
    "AND r9, r9, 1023\n",
    "ADD r9, r9, 4500\n",
    "STS [r9], r8\n",
    "ENDIF\n",
    "BAR\n",
    "MOV r1, r2\n",
    "ENDLOOP\n",
    "ENDLOOP\n",
    "ENDIF\n",
);

/// Reference B: the dense sweep. Correct, but it only earns two stars (checked below).
#[cfg(test)]
const SS_DENSE: &str = concat!(
    "; reference B: dense sweep, GSYNC every step | 参考 B：整片扫描，每步 GSYNC\n",
    "LD r0, [1]\n",
    "ADD r1, tid, 32\n",
    "LD r1, [r1]\n",
    "SETP.LE p0, r1, r0\n",
    "SEL r0, p0, 1, 0\n",
    "SHR r1, tid, 6\n",
    "SHL r1, r1, 1\n",
    "ADD r1, r1, tid\n",
    "ADD r1, r1, 67\n",
    "ADD r5, r1, 4096\n",
    "ADD r6, r5, 4356\n",
    "LD r2, [0]\n",
    "SETP.EQ p1, r2, tid\n",
    "SEL r3, p1, 1, 0\n",
    "ST [r5], r3\n",
    "GSYNC\n",
    "LOOP 48\n",
    "LOOP 2\n",
    "LDO r1, [r5]\n",
    "ADD r2, r5, 1\n",
    "LDO r2, [r2]\n",
    "SUB r3, r5, 1\n",
    "LDO r3, [r3]\n",
    "ADD r4, r5, 66\n",
    "LDO r4, [r4]\n",
    "SUB r7, r5, 66\n",
    "LDO r7, [r7]\n",
    "OR r2, r2, r3\n",
    "OR r4, r4, r7\n",
    "OR r2, r2, r4\n",
    "AND r2, r2, r0\n",
    "OR r1, r1, r2\n",
    "ST [r6], r1\n",
    "MOV r2, r5\n",
    "MOV r5, r6\n",
    "MOV r6, r2\n",
    "GSYNC\n",
    "ENDLOOP\n",
    "ENDLOOP\n",
    "LDO r1, [r5]\n",
    "ST [tid], r1\n",
);

#[cfg(test)]
mod surge_tests {
    use crate::task::Solution;

    #[test]
    fn dense_sweep_is_right_but_earns_two_stars() {
        let t = crate::content::task("storm-surge").expect("task");
        let rep =
            crate::grade::grade(t, &Solution::Code(super::SS_DENSE.to_string())).expect("build");
        assert!(rep.runs.iter().all(|r| r.wrong.is_empty()));
        assert_eq!(rep.stars(), 2, "{:?}", rep.measure);
    }
}
