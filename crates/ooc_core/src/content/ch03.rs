//! Chapter 3 · Lumen at full load: one warp, thirty-two lanes.

use oo_base::Tr;

use super::warp::{lanes, rom_cases, values};
use super::{SCALAR_OPS, WARP_OPS, below, rng};
use crate::isa::{FB_LEN, LANES};
use crate::task::*;
use crate::vm::Launch;

fn base(id: &'static str, slot: u8, ops: &'static [crate::isa::OpKind]) -> Task {
    super::warp::base(3, id, slot, ops)
}

pub fn tasks() -> Vec<Task> {
    vec![
        thirty_two(),
        warp_map(),
        the_shuffle(),
        the_crawl(),
        the_reduction(),
        the_vote(),
        edge_lane(),
        camel_count(),
    ]
}

/// 256 readings; each warp's first one is replaced by an extreme.
fn warp_readings(seed: u64) -> Vec<i32> {
    let mut v = values(seed, 256, 0, 500);
    for (w, e) in [900, 5, 640, 0, 999, 300, 7, 800].into_iter().enumerate() {
        v[w * 32] = e;
    }
    v
}

fn warp_map() -> Task {
    Task {
        view: View::Screen,
        launch: Launch::SCREEN,
        out_len: FB_LEN,
        init: screen_blank,
        title: Tr::new("The Warp Map", "warp 地图"),
        brief: Tr::new(
            "256 threads now run as 8 warps of 32, and rom[0..256] holds one reading per thread.\nEach warp has a reference reading: the first one of its own block of 32 (thread 0, 32, 64, …).\nWrite out[tid] = rom[tid] − the reference of the warp the thread belongs to.\nThe old program subtracts rom[0] everywhere, which is right for exactly one warp.",
            "现在 256 个线程分成 8 个 warp，每个 warp 32 个；rom[0..256] 里每个线程有一个读数。\n每个 warp 有一个基准读数：它自己那一块 32 个里的第一个（线程 0、32、64……）。\n写入 out[tid] = rom[tid] − 本线程所属 warp 的基准读数。\n旧程序处处减去 rom[0]，这只对其中一个 warp 成立。",
        ),
        cases: || rom_cases(|s| values(s, 256, 0, 500), &[151, 152, 153]),
        hidden: || rom_cases(warp_readings, &[154, 155]),
        expect: |c| {
            (0..FB_LEN)
                .map(|t| Some(c.rom[t] - c.rom[t & !31]))
                .collect()
        },
        goals: [Goal::new(Metric::Cycles, 40), Goal::new(Metric::Size, 5)],
        starter: Source::Code(
            "; one reference for everybody | 所有人共用一个基准\nLD r0, [tid]\nLD r1, [0]\nSUB r0, r0, r1\nST [tid], r0\n",
        ),
        reference: Source::Code(
            "SHL r1, warp, 5\nLD r2, [r1]\nLD r0, [tid]\nSUB r0, r0, r2\nST [tid], r0\n",
        ),
        predict: Predict {
            question: Tr::new(
                "Thread 70 runs on which warp, and as which lane of it?",
                "70 号线程在哪个 warp 上，是其中的第几号车道？",
            ),
            options: [
                Tr::new("Warp 2, lane 6", "2 号 warp，6 号车道"),
                Tr::new("Warp 3, lane 6", "3 号 warp，6 号车道"),
                Tr::new("Warp 2, lane 70", "2 号 warp，70 号车道"),
            ],
            answer: 0,
            why: Tr::new(
                "tid = warp·32 + lane: 70 = 2·32 + 6. The lane number never exceeds 31.",
                "tid = warp·32 + lane：70 = 2·32 + 6。车道号永远不会超过 31。",
            ),
        },
        hints: [
            Tr::new(
                "Each thread knows tid, warp and lane. Which of them tells you where your block's reference sits?",
                "每个线程都知道自己的 tid、warp 和 lane。哪一个能告诉你本块的基准放在哪里？",
            ),
            Tr::new(
                "A warp's first thread is warp·32, i.e. warp SHL 5. Load the reference from that address.",
                "一个 warp 的第一个线程是 warp·32，也就是 warp SHL 5。从这个地址读出基准。",
            ),
            Tr::new(
                "SHL r1, warp, 5 / LD r2, [r1] / LD r0, [tid] / SUB r0, r0, r2 / ST [tid], r0.",
                "SHL r1, warp, 5 / LD r2, [r1] / LD r0, [tid] / SUB r0, r0, r2 / ST [tid], r0。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Warps tile the threads", "warp 铺满线程"),
            line: Tr::new(
                "A launch of many threads is cut into warps of 32. Knowing which warp and lane you are is how a thread finds its work.",
                "成百上千个线程会被切成每 32 个一组的 warp。知道自己是哪个 warp、哪条车道，线程才能找到自己的活。",
            ),
            reality: Tr::new(
                "CUDA exposes threadIdx, blockIdx and the lane id; the warp a thread belongs to is just its index divided by 32.",
                "CUDA 暴露了 threadIdx、blockIdx 和车道号；线程所属的 warp 就是它的编号除以 32。",
            ),
            myth: Tr::new(
                "Myth: all threads of a launch see the same world. Per-warp values need per-warp addresses.",
                "误解：一次启动里所有线程看到的是同一个世界。每个 warp 自己的值，需要每个 warp 自己的地址。",
            ),
            simplified: Tr::new(
                "Real chips run warps on many multiprocessors at once; OC-1 has one that works through them in turn.",
                "真实芯片会在许多个多处理器上同时跑 warp；OC-1 只有一个，依次处理它们。",
            ),
        },
        ..base("warp-map", 2, SCALAR_OPS)
    }
}

fn the_crawl() -> Task {
    Task {
        title: Tr::new("The Crawl", "爬行"),
        brief: Tr::new(
            "The night-watch ring has 32 posts, rom[0..32]. Each post reports the sum of itself and the next three clockwise: out[lane] = rom[lane] + rom[lane+1] + rom[lane+2] + rom[lane+3], wrapping around.\nThe old program has every lane crawl along the ring, loading four times — 128 loads for 32 numbers.\nThe bus allows 32. Load once, then pass the readings sideways.",
            "夜巡环有 32 个岗哨，读数在 rom[0..32]。每个岗哨报告自己加顺时针后面三个的和：out[lane] = rom[lane] + rom[lane+1] + rom[lane+2] + rom[lane+3]，首尾相接。\n旧程序让每条车道沿环爬行，读四次——32 个数读了 128 次。\n总线只允许 32 次。读一次，然后把读数横着传。",
        ),
        cases: || rom_cases(|s| values(s, 32, 0, 500), &[161, 162, 163]),
        hidden: || rom_cases(|s| values(s, 32, 0, 500), &[164, 165]),
        expect: |c| lanes(|l| (0..4).map(|k| c.rom[(l + k) % LANES]).sum()),
        limit: Some(Goal::new(Metric::Loads, 32)),
        goals: [Goal::new(Metric::Cycles, 12), Goal::new(Metric::Size, 12)],
        starter: Source::Code(
            "MOV r1, 0\nMOV r2, lane\nLOOP 4\n  AND r3, r2, 31\n  LD r0, [r3]\n  ADD r1, r1, r0\n  ADD r2, r2, 1\nENDLOOP\nST [lane], r1\n",
        ),
        reference: Source::Code(concat!(
            "LD r0, [lane]\nMOV r1, r0\n",
            "ADD r2, lane, 1\nSHFL r3, r0, r2\nADD r1, r1, r3\n",
            "ADD r2, lane, 2\nSHFL r3, r0, r2\nADD r1, r1, r3\n",
            "ADD r2, lane, 3\nSHFL r3, r0, r2\nADD r1, r1, r3\n",
            "ST [lane], r1\n",
        )),
        predict: Predict {
            question: Tr::new(
                "Neighbouring lanes crawl over almost the same posts. How many times is rom[5] loaded in the starter?",
                "相邻的车道爬过的几乎是同一批岗哨。初始程序里 rom[5] 被读了多少次？",
            ),
            options: [
                Tr::new("Once", "一次"),
                Tr::new("Four times", "四次"),
                Tr::new("32 times", "32 次"),
            ],
            answer: 1,
            why: Tr::new(
                "Lanes 2, 3, 4 and 5 each reach post 5 once: four loads of one number. The whole warp fetches every value four times.",
                "2、3、4、5 号车道各会路过 5 号岗哨一次：同一个数读了四遍。整个 warp 把每个值都取了四遍。",
            ),
        },
        hints: [
            Tr::new(
                "Count how often the same address is loaded across the warp. Each lane already holds one of those values in a register.",
                "数一数整个 warp 里同一个地址被读了几次。每条车道的寄存器里其实已经有其中一个值了。",
            ),
            Tr::new(
                "Load rom[lane] once. For k = 1, 2, 3 take the r0 of lane + k with SHFL and add it up.",
                "只读一次 rom[lane]。对 k = 1、2、3，用 SHFL 取第 lane + k 号车道的 r0 并累加。",
            ),
            Tr::new(
                "LD r0, [lane] / MOV r1, r0, then three times: ADD r2, lane, k / SHFL r3, r0, r2 / ADD r1, r1, r3; finally ST [lane], r1.",
                "LD r0, [lane] / MOV r1, r0，然后三次：ADD r2, lane, k / SHFL r3, r0, r2 / ADD r1, r1, r3；最后 ST [lane], r1。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Load once, share many", "读一次，多人用"),
            line: Tr::new(
                "When neighbours need overlapping data, each value should come from memory once and travel between lanes in registers.",
                "当相邻车道需要重叠的数据时，每个值只该从内存读一次，然后在车道的寄存器间传递。",
            ),
            reality: Tr::new(
                "Stencils and sliding-window filters on GPUs are written this way: a load per element, shuffles for the neighbours.",
                "GPU 上的模板计算和滑动窗口滤波都是这样写的：每个元素读一次，邻居靠洗牌取得。",
            ),
            myth: Tr::new(
                "Myth: caches make repeated loads free. They help, but the load instructions still issue and the bus still carries them.",
                "误解：有了缓存，重复读取就是免费的。缓存有帮助，但读取指令照样发射，总线照样要传。",
            ),
            simplified: Tr::new(
                "Real GPUs coalesce neighbouring loads into one transaction, which softens — not removes — the cost.",
                "真实 GPU 会把相邻的读取合并成一次事务，这减轻了代价，但并没有消除它。",
            ),
        },
        clue: Some(Clue {
            what: Tr::new(
                "the first load that fetches again what another lane already fetched",
                "第一次把别的车道已经读过的东西重新读一遍的那个读取",
            ),
            find: crate::grade::first_repeat_load,
        }),
        ..base("the-crawl", 4, WARP_OPS)
    }
}

fn thirty_two() -> Task {
    Task {
        title: Tr::new("Thirty-Two Stars", "三十二颗星"),
        brief: Tr::new(
            "The star chart has 32 brightness readings in rom[0..32]. Each must be tripled into out[0..32].\nThe old program makes lane 0 do all the work while 31 lanes watch.\nIt is correct. It is also far too slow: the target is 8 cycles.",
            "星图在 rom[0..32] 里有 32 个亮度读数，每个都要乘 3 写进 out[0..32]。\n旧程序让 0 号车道包揽全部工作，其余 31 条车道干看着。\n结果是对的，但太慢了：目标是 8 个周期内完成。",
        ),
        cases: || rom_cases(|s| values(s, 32, 0, 1000), &[101, 102, 103]),
        hidden: || rom_cases(|s| values(s, 32, 0, 1000), &[104, 105]),
        expect: |c| lanes(|l| c.rom[l] * 3),
        limit: Some(Goal::new(Metric::Cycles, 8)),
        goals: [Goal::new(Metric::Cycles, 3), Goal::new(Metric::Size, 3)],
        starter: Source::Code(
            "; only lane 0 works | 只有 0 号车道在干活\nSETP.EQ p0, lane, 0\nIF p0\n  MOV r1, 0\n  LOOP 32\n    LD r0, [r1]\n    MUL r0, r0, 3\n    ST [r1], r0\n    ADD r1, r1, 1\n  ENDLOOP\nENDIF\n",
        ),
        reference: Source::Code("LD r0, [lane]\nMUL r0, r0, 3\nST [lane], r0\n"),
        predict: Predict {
            question: Tr::new(
                "The warp issues one instruction per cycle for all 32 lanes. How many cycles does the starter take?",
                "warp 每个周期为 32 条车道发出一条指令。初始程序要多少个周期？",
            ),
            options: [
                Tr::new("About 8", "大约 8"),
                Tr::new("About 40", "大约 40"),
                Tr::new("About 160", "大约 160"),
            ],
            answer: 2,
            why: Tr::new(
                "32 trips around a 5-instruction loop is 160 slots, each with 31 idle lanes. Utilisation: about 3%.",
                "5 条指令的循环跑 32 圈就是 160 个周期，每个周期都有 31 条车道闲着。利用率约 3%。",
            ),
        },
        hints: [
            Tr::new(
                "Every lane already knows its own number. What if each lane handled one star?",
                "每条车道都知道自己的编号。如果每条车道只处理一颗星呢？",
            ),
            Tr::new(
                "`lane` is 0..31, one per lane. Use it as the address for both the load and the store.",
                "`lane` 取值 0..31，每条车道一个。把它同时用作读取和写入的地址。",
            ),
            Tr::new(
                "Three lines: LD r0, [lane] / MUL r0, r0, 3 / ST [lane], r0.",
                "三行：LD r0, [lane] / MUL r0, r0, 3 / ST [lane], r0。",
            ),
        ],
        insight: Insight {
            title: Tr::new("SIMT", "单指令多线程"),
            line: Tr::new(
                "One instruction, thirty-two lanes. Parallel code is often shorter than serial code, not longer.",
                "一条指令，三十二条车道。并行代码常常比串行代码更短，而不是更长。",
            ),
            reality: Tr::new(
                "NVIDIA GPUs group 32 threads into a warp; AMD uses 32 or 64 in a wavefront. All share one program counter.",
                "NVIDIA GPU 把 32 个线程编成一个 warp；AMD 的 wavefront 是 32 或 64 个。它们共用一个程序计数器。",
            ),
            myth: Tr::new(
                "Myth: GPU threads are like CPU threads. Here they cannot even take a different step without the others waiting.",
                "误解：GPU 线程和 CPU 线程一样。在这里，一个线程想走不同的一步，其他线程都得等着。",
            ),
            simplified: Tr::new(
                "Modern GPUs let lanes of a warp schedule somewhat independently; the lock-step model is still how they run fastest.",
                "现代 GPU 允许 warp 内的车道有一定独立调度；但步调一致仍是它们跑得最快的方式。",
            ),
        },
        ..base("thirty-two", 1, SCALAR_OPS)
    }
}

fn the_shuffle() -> Task {
    Task {
        title: Tr::new("The Shuffle", "洗牌"),
        brief: Tr::new(
            "The ring of 32 sensors smooths its readings: out[lane] = rom[lane] + rom[lane + 1], wrapping 31 back to 0.\nThe old program reads every value twice — 64 loads for 32 numbers.\nThe memory bus allows 32. Each lane already holds its own reading; borrow the neighbour's.",
            "由 32 个传感器组成的环要平滑读数：out[lane] = rom[lane] + rom[lane + 1]，31 号接回 0 号。\n旧程序把每个值都读了两遍——32 个数读了 64 次。\n内存总线只允许 32 次。每条车道手里已经有自己的读数了，去借邻居的。",
        ),
        cases: || rom_cases(|s| values(s, 32, 0, 1000), &[131, 132, 133]),
        hidden: || rom_cases(|s| values(s, 32, 0, 1000), &[134, 135]),
        expect: |c| lanes(|l| c.rom[l] + c.rom[(l + 1) % LANES]),
        limit: Some(Goal::new(Metric::Loads, 32)),
        goals: [Goal::new(Metric::Cycles, 5), Goal::new(Metric::Size, 5)],
        starter: Source::Code(
            "LD r0, [lane]\nADD r2, lane, 1\nAND r2, r2, 31\nLD r1, [r2]\nADD r0, r0, r1\nST [lane], r0\n",
        ),
        reference: Source::Code(
            "LD r0, [lane]\nADD r2, lane, 1\nSHFL r1, r0, r2\nADD r0, r0, r1\nST [lane], r0\n",
        ),
        predict: Predict {
            question: Tr::new(
                "SHFL r1, r0, r2 gives each lane the r0 of lane r2. What does it cost in memory loads?",
                "SHFL r1, r0, r2 让每条车道拿到第 r2 号车道的 r0。它要花多少次内存读取？",
            ),
            options: [
                Tr::new("None", "零次"),
                Tr::new("One per lane", "每条车道一次"),
                Tr::new("32 per lane", "每条车道 32 次"),
            ],
            answer: 0,
            why: Tr::new(
                "Lanes trade registers directly inside the warp. Memory is never touched.",
                "车道在 warp 内部直接交换寄存器，完全不碰内存。",
            ),
        },
        hints: [
            Tr::new(
                "The second LD fetches a value that lane + 1 has already loaded into its own r0.",
                "第二个 LD 取的值，lane + 1 号车道早已读进了它自己的 r0。",
            ),
            Tr::new(
                "SHFL d, a, src reads register a from lane src of the same warp, wrapping around 32.",
                "SHFL d, a, src 从同一 warp 的第 src 号车道读取寄存器 a，超过 32 会自动环绕。",
            ),
            Tr::new(
                "Replace AND r2, r2, 31 and LD r1, [r2] with SHFL r1, r0, r2.",
                "把 AND r2, r2, 31 和 LD r1, [r2] 换成 SHFL r1, r0, r2。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Warp shuffles", "warp 洗牌"),
            line: Tr::new(
                "Registers are the fastest memory there is, and a warp can pass them sideways.",
                "寄存器是最快的存储，而 warp 可以把它们横向传递。",
            ),
            reality: Tr::new(
                "CUDA's __shfl_sync and similar instructions on every vendor move data between lanes in a single cycle.",
                "CUDA 的 __shfl_sync 以及各厂商的同类指令，能在一个周期内在车道间搬运数据。",
            ),
            myth: Tr::new(
                "Myth: threads can only share data through memory. Within a warp they share registers.",
                "误解：线程之间只能通过内存共享数据。在 warp 之内，它们共享寄存器。",
            ),
            simplified: Tr::new(
                "Real shuffles work only within a warp too; sharing across warps goes through on-chip shared memory, which OC-1 omits.",
                "真实的洗牌同样只在 warp 内有效；跨 warp 共享要经过片上共享内存，OC-1 省略了它。",
            ),
        },
        ..base("the-shuffle", 3, WARP_OPS)
    }
}

fn the_reduction() -> Task {
    Task {
        title: Tr::new("The Reduction", "归约"),
        brief: Tr::new(
            "The observatory needs the total of all 32 readings in rom[0..32], written to out[0].\nThe old program sends lane 0 around the loop 32 times while the rest wait.\nThe dome closes in 24 cycles. Let the lanes add in pairs, then pairs of pairs.",
            "天文台要把 rom[0..32] 的 32 个读数加总，写入 out[0]。\n旧程序让 0 号车道绕循环 32 圈，其余车道都在等。\n圆顶 24 个周期后就要关闭。让车道两两相加，再把结果两两相加。",
        ),
        cases: || rom_cases(|s| values(s, 32, 0, 1000), &[141, 142, 143]),
        hidden: || rom_cases(|s| values(s, 32, 0, 1000), &[144, 145]),
        expect: |c| {
            let mut v = vec![None; LANES];
            v[0] = Some(c.rom.iter().sum());
            v
        },
        limit: Some(Goal::new(Metric::Cycles, 24)),
        goals: [Goal::new(Metric::Cycles, 17), Goal::new(Metric::Size, 17)],
        starter: Source::Code(
            "SETP.EQ p0, lane, 0\nIF p0\n  MOV r1, 0\n  MOV r2, 0\n  LOOP 32\n    LD r0, [r1]\n    ADD r2, r2, r0\n    ADD r1, r1, 1\n  ENDLOOP\n  ST [0], r2\nENDIF\n",
        ),
        reference: Source::Code(concat!(
            "LD r0, [lane]\n",
            "; fold the warp in half, five times | 把 warp 对折，一共折五次\n",
            "XOR r2, lane, 16\nSHFL r1, r0, r2\nADD r0, r0, r1\n",
            "XOR r2, lane, 8\nSHFL r1, r0, r2\nADD r0, r0, r1\n",
            "XOR r2, lane, 4\nSHFL r1, r0, r2\nADD r0, r0, r1\n",
            "XOR r2, lane, 2\nSHFL r1, r0, r2\nADD r0, r0, r1\n",
            "XOR r2, lane, 1\nSHFL r1, r0, r2\nADD r0, r0, r1\n",
            "ST [lane], r0\n",
        )),
        predict: Predict {
            question: Tr::new(
                "If every step halves the number of partial sums, how many steps reduce 32 numbers to one?",
                "如果每一步都让部分和的数量减半，32 个数要几步才能归成一个？",
            ),
            options: [Tr::new("5", "5"), Tr::new("16", "16"), Tr::new("31", "31")],
            answer: 0,
            why: Tr::new(
                "32 → 16 → 8 → 4 → 2 → 1: log2(32) = 5 steps. The same 31 additions happen, but 16 at a time.",
                "32 → 16 → 8 → 4 → 2 → 1：log2(32) = 5 步。加法总数仍是 31 次，只是每次同时做 16 次。",
            ),
        },
        hints: [
            Tr::new(
                "Start by giving every lane its own reading. Then each lane adds a partner's value — which partner?",
                "先让每条车道拿到自己的读数。然后每条车道加上一个伙伴的值——哪个伙伴？",
            ),
            Tr::new(
                "Partner = lane XOR 16, then XOR 8, 4, 2, 1. After each step every lane holds the sum of a bigger group.",
                "伙伴 = lane XOR 16，接着 XOR 8、4、2、1。每一步之后，每条车道持有的都是更大一组的和。",
            ),
            Tr::new(
                "LD r0, [lane]; then five times: XOR r2, lane, k / SHFL r1, r0, r2 / ADD r0, r0, r1 for k = 16, 8, 4, 2, 1; then ST [lane], r0.",
                "LD r0, [lane]；然后重复五次：XOR r2, lane, k / SHFL r1, r0, r2 / ADD r0, r0, r1，k 依次为 16、8、4、2、1；最后 ST [lane], r0。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Tree reduction", "树形归约"),
            line: Tr::new(
                "Adding 32 numbers took five steps, not thirty-one. Parallelism changes the shape of an algorithm.",
                "把 32 个数相加只用了五步，而不是三十一步。并行改变了算法的形状。",
            ),
            reality: Tr::new(
                "Sums, maxima and dot products on GPUs — including inside neural networks — are built from exactly this butterfly.",
                "GPU 上的求和、求最大值与点积——包括神经网络内部——都是由这种蝶形结构搭成的。",
            ),
            myth: Tr::new(
                "Myth: some problems are just serial. Any associative operation can be split into a tree.",
                "误解：有些问题天生就是串行的。任何满足结合律的运算都能拆成一棵树。",
            ),
            simplified: Tr::new(
                "Real reductions span many warps and blocks, combining warp results through shared memory and atomics.",
                "真实的归约跨越很多 warp 和线程块，通过共享内存与原子操作合并各 warp 的结果。",
            ),
        },
        ..base("the-reduction", 4, WARP_OPS)
    }
}

/// Thirty-two water readings and the bound in rom[32]; the lanes in `low`
/// read below it.
fn herd(seed: u64, low: &[usize]) -> Case {
    let mut r = rng(seed);
    let mut rom: Vec<i32> = (0..LANES)
        .map(|l| {
            if low.contains(&l) {
                below(&mut r, 300)
            } else {
                300 + below(&mut r, 300)
            }
        })
        .collect();
    rom.push(300);
    Case { rom, frame: 0 }
}

fn the_vote() -> Task {
    Task {
        title: Tr::new("The Vote", "表决"),
        brief: Tr::new(
            "Thirty-two wells report their water level in rom[0..32]; the safe bound is rom[32].\nIf every well is at or above the bound, all lanes write 1 to their out cell. If even one is below, all write 0.\nThe old program walks the wells one at a time. The caravan leaves in 24 cycles — ask the whole warp at once.",
            "三十二口水井在 rom[0..32] 里报告水位，安全线是 rom[32]。\n如果每口井都不低于安全线，所有车道往自己的 out 格写 1；只要有一口井低于，就全写 0。\n旧程序一口一口地巡井。商队 24 个周期后出发——一次问遍整个 warp。",
        ),
        cases: || {
            vec![
                herd(171, &[]),
                herd(172, &[0]),
                herd(173, &[17]),
                herd(174, &[3, 9, 20, 26]),
            ]
        },
        hidden: || {
            vec![
                herd(175, &[31]),
                Case {
                    rom: vec![300; 33],
                    frame: 0,
                },
                herd(176, &(0..LANES).collect::<Vec<_>>()),
            ]
        },
        expect: |c| {
            let ok = c.rom[..LANES].iter().all(|&w| w >= c.rom[LANES]);
            lanes(|_| ok as i32)
        },
        limit: Some(Goal::new(Metric::Cycles, 24)),
        goals: [Goal::new(Metric::Cycles, 6), Goal::new(Metric::Size, 6)],
        starter: Source::Code(
            "LD r1, [32]\nMOV r2, 1\nMOV r3, 0\nLOOP 32\n  LD r0, [r3]\n  SETP.LT p0, r0, r1\n  IF p0\n    MOV r2, 0\n  ENDIF\n  ADD r3, r3, 1\nENDLOOP\nST [lane], r2\n",
        ),
        reference: Source::Code(
            "LD r0, [lane]\nLD r1, [32]\nSETP.GE p0, r0, r1\nVOTE.ALL p1, p0\nSEL r2, p1, 1, 0\nST [lane], r2\n",
        ),
        predict: Predict {
            question: Tr::new(
                "One well in thirty-two is dry. After VOTE.ALL, how many lanes see \"not everyone is fine\"?",
                "三十二口井里有一口干了。VOTE.ALL 之后，有多少条车道看到“并非人人安好”？",
            ),
            options: [
                Tr::new("Only the dry well's lane", "只有那口干井的车道"),
                Tr::new("Its neighbours", "它的邻居们"),
                Tr::new("All 32 lanes", "全部 32 条车道"),
            ],
            answer: 2,
            why: Tr::new(
                "A vote is one answer for the whole warp, so every lane gets the same result. That is why it can replace a loop of 32 checks.",
                "表决对整个 warp 只有一个答案，每条车道拿到的结果都相同。所以它能顶替 32 次检查的循环。",
            ),
        },
        hints: [
            Tr::new(
                "Each lane can say yes or no about its own well. You need a way to combine all 32 answers.",
                "每条车道都能对自己的那口井说是或否。你需要一种方式把 32 个回答合到一起。",
            ),
            Tr::new(
                "SETP.GE p0, r0, r1 gives each lane its own answer; VOTE.ALL p1, p0 is true only if p0 holds in every lane.",
                "SETP.GE p0, r0, r1 给出每条车道自己的回答；VOTE.ALL p1, p0 只有在每条车道的 p0 都成立时才为真。",
            ),
            Tr::new(
                "LD r0, [lane] / LD r1, [32] / SETP.GE p0, r0, r1 / VOTE.ALL p1, p0 / SEL r2, p1, 1, 0 / ST [lane], r2.",
                "LD r0, [lane] / LD r1, [32] / SETP.GE p0, r0, r1 / VOTE.ALL p1, p0 / SEL r2, p1, 1, 0 / ST [lane], r2。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Warp votes", "warp 表决"),
            line: Tr::new(
                "Asking \"does anyone, does everyone?\" costs one instruction, not a loop over the lanes.",
                "问一句“有没有人、是不是所有人”只要一条指令，不用在车道上循环。",
            ),
            reality: Tr::new(
                "CUDA's __all_sync and __any_sync, and ballot in other APIs, let a warp agree on a condition in a single cycle.",
                "CUDA 的 __all_sync、__any_sync，以及其他 API 里的 ballot，让一个 warp 在一个周期内就某个条件达成一致。",
            ),
            myth: Tr::new(
                "Myth: checking all the data takes a pass over all the data. Within a warp it takes one vote.",
                "误解：检查所有数据就要把所有数据过一遍。在一个 warp 之内，只要一次表决。",
            ),
            simplified: Tr::new(
                "Votes only cover one warp. Agreeing across a whole block needs a barrier and shared memory.",
                "表决只覆盖一个 warp。要在整个线程块里达成一致，需要栅栏和共享内存。",
            ),
        },
        ..base("the-vote", 6, WARP_OPS)
    }
}

fn edge_lane() -> Task {
    Task {
        title: Tr::new("The Edge Lane", "边缘车道"),
        brief: Tr::new(
            "Fault report: the fence sensors are a line, not a ring. Each reports itself plus its left and right neighbour: out[l] = rom[l−1] + rom[l] + rom[l+1], where a post past either end counts as 0.\nThe smoothing program works in the middle, but the two end posts report nonsense.\nFind what the end lanes borrow.",
            "故障报告：围栏传感器是一条线，不是一个环。每个报告自己加左右邻居：out[l] = rom[l−1] + rom[l] + rom[l+1]，越过任一端的岗哨按 0 计。\n平滑程序在中间都正常，但两端的岗哨报出了荒唐的数字。\n查一查两端的车道借到了什么。",
        ),
        cases: || rom_cases(|s| values(s, 32, 0, 500), &[181, 182, 183]),
        hidden: || {
            rom_cases(
                |s| {
                    let mut v = values(s, 32, 0, 500);
                    (v[0], v[31]) = (900, 950);
                    v
                },
                &[184, 185],
            )
        },
        expect: |c| {
            lanes(|l| {
                let at = |i: isize| usize::try_from(i).ok().and_then(|i| c.rom.get(i)).copied();
                (l as isize - 1..=l as isize + 1)
                    .filter(|&i| i < LANES as isize)
                    .map(|i| at(i).unwrap_or(0))
                    .sum()
            })
        },
        goals: [Goal::new(Metric::Cycles, 12), Goal::new(Metric::Size, 12)],
        starter: Source::Code(
            "LD r0, [lane]\nSUB r2, lane, 1\nSHFL r1, r0, r2\nADD r3, r0, r1\nADD r2, lane, 1\nSHFL r1, r0, r2\nADD r3, r3, r1\nST [lane], r3\n",
        ),
        reference: Source::Code(concat!(
            "LD r0, [lane]\n",
            "SUB r2, lane, 1\nSHFL r1, r0, r2\nSETP.GT p0, lane, 0\nSEL r1, p0, r1, 0\n",
            "ADD r2, lane, 1\nSHFL r3, r0, r2\nSETP.LT p1, lane, 31\nSEL r3, p1, r3, 0\n",
            "ADD r0, r0, r1\nADD r0, r0, r3\nST [lane], r0\n",
        )),
        predict: Predict {
            question: Tr::new(
                "Lane 0 asks SHFL for lane −1. Which lane's value does it get?",
                "0 号车道用 SHFL 去要 −1 号车道的值。它会拿到哪条车道的值？",
            ),
            options: [
                Tr::new("Zero — there is no such lane", "零——根本没有这条车道"),
                Tr::new(
                    "Lane 31's — the request wraps around",
                    "31 号车道的——请求会环绕",
                ),
                Tr::new("Its own", "它自己的"),
            ],
            answer: 1,
            why: Tr::new(
                "SHFL takes the lane number modulo 32, so −1 becomes 31. The end of the line borrows from the other end.",
                "SHFL 把车道号对 32 取模，−1 就变成了 31。队伍的一端从另一端借来了数据。",
            ),
        },
        hints: [
            Tr::new(
                "Look at lanes 0 and 31 only. What does each of them ask SHFL for, and what answers?",
                "只看 0 号和 31 号车道。它们各向 SHFL 要什么，是谁在回答？",
            ),
            Tr::new(
                "SHFL never fails: an out-of-range request wraps. So the end lanes must refuse the borrowed value, with a predicate and SEL.",
                "SHFL 从不失败：越界的请求会环绕。所以两端的车道必须用谓词和 SEL 拒收借来的值。",
            ),
            Tr::new(
                "After each SHFL: SETP.GT p0, lane, 0 and SEL r1, p0, r1, 0 for the left neighbour; SETP.LT p1, lane, 31 and SEL r3, p1, r3, 0 for the right.",
                "每次 SHFL 之后：对左邻居用 SETP.GT p0, lane, 0 和 SEL r1, p0, r1, 0；对右邻居用 SETP.LT p1, lane, 31 和 SEL r3, p1, r3, 0。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Boundaries wrap silently", "边界会悄悄环绕"),
            line: Tr::new(
                "A shuffle past the end of the warp does not fail. It lands on the far side and returns a perfectly valid, wrong number.",
                "越过 warp 末端的洗牌不会失败，它会落到另一头，返回一个完全合法却错误的数。",
            ),
            reality: Tr::new(
                "Halo and edge handling is the classic stencil bug: the interior is right, the borders quietly are not.",
                "光环和边缘处理是模板计算里的经典 bug：内部全对，边缘悄悄出错。",
            ),
            myth: Tr::new(
                "Myth: tests on typical data are enough. Edge lanes only show their bug when the ends carry big values.",
                "误解：用典型数据测试就够了。只有两端的值很大时，边缘车道的 bug 才会现形。",
            ),
            simplified: Tr::new(
                "CUDA's shuffles return the caller's own value for out-of-range lanes; OC-1 wraps, to make the trap easy to see.",
                "CUDA 的洗牌在车道越界时返回调用者自己的值；OC-1 选择环绕，让这个陷阱更容易看见。",
            ),
        },
        clue: Some(Clue {
            what: Tr::new(
                "the first SHFL that asked for a lane beyond the end of the warp",
                "第一个向 warp 末端之外的车道要数据的 SHFL",
            ),
            find: crate::grade::first_wrapped_shfl,
        }),
        ..base("edge-lane", 7, WARP_OPS)
    }
}

/// rom[0..32]: water per camel; rom[32..64]: its load. The camels in `over`
/// carry more than twice their water; with `exact` the rest carry exactly that.
fn fleet(seed: u64, over: &[usize], exact: bool) -> Case {
    let mut r = rng(seed);
    let water: Vec<i32> = (0..LANES).map(|_| 100 + below(&mut r, 150)).collect();
    let load: Vec<i32> = water
        .iter()
        .enumerate()
        .map(|(l, &w)| {
            if over.contains(&l) {
                2 * w + 1 + below(&mut r, 40)
            } else if exact {
                2 * w
            } else {
                20 + below(&mut r, 2 * w - 20)
            }
        })
        .collect();
    Case {
        rom: water.into_iter().chain(load).collect(),
        frame: 0,
    }
}

fn camel_count() -> Task {
    Task {
        title: Tr::new("The Camel Count", "骆驼清点"),
        brief: Tr::new(
            "The caravan master wants three numbers from the roll in rom: water per camel in rom[0..32], load per camel in rom[32..64].\nout[0] = total water, out[1] = total load, out[2] = 1 if every camel carries at most twice its water in load, otherwise 0.\nThe other cells are not checked. The caravan leaves in 60 cycles; two sums and a check each want a whole warp.",
            "商队长要从名册里知道三个数：每头骆驼的水量在 rom[0..32]，驮的货在 rom[32..64]。\nout[0] = 水的总量，out[1] = 货的总量，out[2] = 若每头骆驼驮的货都不超过它水量的两倍则为 1，否则为 0。\n其余的格子不检查。商队 60 个周期后出发；两个求和加一次检查，每一项都要动用整个 warp。",
        ),
        cases: || {
            vec![
                fleet(191, &[], false),
                fleet(192, &[9], false),
                fleet(193, &[2, 14, 15, 28], false),
                fleet(194, &[], false),
            ]
        },
        hidden: || {
            vec![
                fleet(195, &[31], false),
                fleet(196, &[], true),
                fleet(197, &[0], true),
            ]
        },
        expect: |c| {
            let (w, l) = (&c.rom[..LANES], &c.rom[LANES..2 * LANES]);
            let mut v = vec![None; LANES];
            v[0] = Some(w.iter().sum());
            v[1] = Some(l.iter().sum());
            v[2] = Some(w.iter().zip(l).all(|(w, l)| 2 * w >= *l) as i32);
            v
        },
        limit: Some(Goal::new(Metric::Cycles, 60)),
        goals: [Goal::new(Metric::Cycles, 31), Goal::new(Metric::Size, 31)],
        starter: Source::Code(""),
        reference: Source::Code(concat!(
            "LD r0, [lane]\nADD r2, lane, 32\nLD r1, [r2]\n",
            "; the check: does every camel carry at most twice its water? | 检查：每头骆驼驮的货都不超过水量的两倍？\n",
            "SHL r2, r0, 1\nSETP.GE p0, r2, r1\nVOTE.ALL p1, p0\nSEL r4, p1, 1, 0\n",
            "; water in the low half-word, load in the high one: one sum does both | 水放低半字，货放高半字：一次求和同时算两样\n",
            "SHL r1, r1, 16\nADD r0, r0, r1\n",
            "XOR r2, lane, 16\nSHFL r1, r0, r2\nADD r0, r0, r1\n",
            "XOR r2, lane, 8\nSHFL r1, r0, r2\nADD r0, r0, r1\n",
            "XOR r2, lane, 4\nSHFL r1, r0, r2\nADD r0, r0, r1\n",
            "XOR r2, lane, 2\nSHFL r1, r0, r2\nADD r0, r0, r1\n",
            "XOR r2, lane, 1\nSHFL r1, r0, r2\nADD r0, r0, r1\n",
            "AND r5, r0, 65535\nSHR r6, r0, 16\n",
            "SETP.EQ p0, lane, 1\nSEL r7, p0, r6, r4\nSETP.EQ p0, lane, 0\nSEL r7, p0, r5, r7\nST [lane], r7\n",
        )),
        live: Some(|frame, _| Case {
            rom: fleet(198, &[], false).rom,
            frame,
        }),
        predict: Predict {
            question: Tr::new(
                "Water totals up to 8000, load totals up to 17000. Packed as water + load·65536, how many reductions do both sums need?",
                "水的总量最多 8000，货的总量最多 17000。打包成 水 + 货·65536，两个求和需要几次归约？",
            ),
            options: [
                Tr::new("One", "一次"),
                Tr::new("Two", "两次"),
                Tr::new("Five", "五次"),
            ],
            answer: 0,
            why: Tr::new(
                "The water sum stays below 65536, so it never spills into the high half-word. One butterfly adds both at once; AND and SHR pull them apart.",
                "水的总和小于 65536，不会溢出到高半字。一次蝶形归约同时加好两样，再用 AND 和 SHR 把它们拆开。",
            ),
        },
        hints: [
            Tr::new(
                "Each of the three answers is something a warp can compute in a few steps you already know: two sums and one vote.",
                "三个答案各自都能用你已经会的几步算出来：两个求和，一次表决。",
            ),
            Tr::new(
                "Both sums can ride in one register: water + (load SHL 16). Add up the packed value with the butterfly, then split it with AND 65535 and SHR 16.",
                "两个求和可以共用一个寄存器：水 + (货 SHL 16)。用蝶形归约加总打包后的值，再用 AND 65535 和 SHR 16 拆开。",
            ),
            Tr::new(
                "Vote on 2·water ≥ load before packing. At the end lane 0 stores the water total, lane 1 the load total, every other lane the vote — pick with SETP.EQ and SEL.",
                "在打包之前，对 2·水 ≥ 货 做表决。最后 0 号车道存水的总和，1 号车道存货的总和，其他车道存表决结果——用 SETP.EQ 和 SEL 来挑。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Warp-level toolbox", "warp 级工具箱"),
            line: Tr::new(
                "Shuffles, reductions and votes combine: a warp can answer several questions about its own 32 numbers without touching memory twice.",
                "洗牌、归约和表决可以组合：一个 warp 能回答关于自己这 32 个数的好几个问题，而不必再碰内存。",
            ),
            reality: Tr::new(
                "Packing two small values into one word to halve the shuffle traffic is an everyday trick in GPU reductions.",
                "把两个小数打包进一个字，让洗牌的次数减半，是 GPU 归约里的日常技巧。",
            ),
            myth: Tr::new(
                "Myth: every question needs its own pass. Related questions can ride the same one.",
                "误解：每个问题都要单独过一遍。相关的问题可以搭同一趟车。",
            ),
            simplified: Tr::new(
                "Packing only works while the sums fit their half-word; real code checks that before relying on it.",
                "打包只在总和装得进自己那半个字时才成立；真实代码在依赖它之前会先核对这一点。",
            ),
        },
        ..base("camel-count", 8, WARP_OPS)
    }
}
