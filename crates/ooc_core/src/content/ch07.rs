//! Chapter 7 · Vault: the memory wall and caches. Fast arithmetic is useless
//! if the data is late: 400 cycles to memory, a small L1 in front of it.

use oo_base::Tr;

use super::prism::rom_case;
use super::vault::{ALL_WARPS, CACHE_OPS, base};
use super::{below, rng};
use crate::task::*;
use crate::vm::Launch;

pub fn tasks() -> Vec<Task> {
    vec![
        the_intensity(),
        the_wait(),
        cache_line(),
        the_stride(),
        the_reuse(),
        the_capacity(),
        the_alias(),
        clinic_ledger(),
    ]
}

/// `n` random values in `lo..hi`.
fn values(seed: u64, n: usize, lo: i32, hi: i32) -> Vec<i32> {
    let mut r = rng(seed);
    (0..n).map(|_| lo + below(&mut r, hi - lo)).collect()
}

fn cells(f: impl Fn(usize) -> i32, n: usize) -> Vec<Option<i32>> {
    (0..n).map(|i| Some(f(i))).collect()
}

fn the_intensity() -> Task {
    Task {
        title: Tr::new("Arithmetic Intensity", "算术强度"),
        brief: Tr::new(
            "Bohe's clinic chart room runs three tiny kernels over the day's readings, one after another. Each reads every cell from global memory and writes it back: 3x + 1, then squared, then ×5 + 7.\nrom[0..512] holds the readings x (0..9), one per thread; write out[i] = (3x + 1)² × 5 + 7 for all 512 threads (16 warps). The out buffer can be read back with LDO.\nThe three passes below are right but slow. Make the same sums in fewer cycles.",
            "薄荷诊所的病历室每天对读数跑三个小内核，一个接一个：每个都把每个格子从全局内存读出来、再写回去。第一个算 3x + 1，第二个平方，第三个 ×5 + 7。\nrom[0..512] 是读数 x（0..9），每个线程一个；对全部 512 个线程（16 个 warp）写出 out[i] = (3x + 1)² × 5 + 7。out 缓冲区可以用 LDO 读回来。\n下面的三趟程序结果是对的，但很慢。用更少的周期算出同样的数。",
        ),
        cases: || {
            [201, 202, 203]
                .map(|s| rom_case(values(s, 512, 0, 10)))
                .to_vec()
        },
        hidden: || {
            vec![
                rom_case(values(204, 512, 0, 10)),
                rom_case(values(205, 512, 0, 10)),
                rom_case(vec![0; 512]),
                rom_case(vec![9; 512]),
            ]
        },
        expect: |c| cells(|i| (3 * c.rom[i] + 1).pow(2) * 5 + 7, 512),
        limit: Some(Goal::new(Metric::Cycles, 520)),
        goals: [Goal::new(Metric::Cycles, 500), Goal::new(Metric::Size, 7)],
        starter: Source::Code(
            "; three passes, each through global memory | 三趟，每趟都过一遍全局内存\nLD r0, [tid]\nMUL r0, r0, 3\nADD r0, r0, 1\nST [tid], r0\nLDO r0, [tid]\nMUL r0, r0, r0\nST [tid], r0\nLDO r0, [tid]\nMUL r0, r0, 5\nADD r0, r0, 7\nST [tid], r0\n",
        ),
        reference: Source::Code(
            "LD r0, [tid]\nMUL r0, r0, 3\nADD r0, r0, 1\nMUL r0, r0, r0\nMUL r0, r0, 5\nADD r0, r0, 7\nST [tid], r0\n",
        ),
        predict: Predict {
            question: Tr::new(
                "Fuse the three kernels into one. Each still does its arithmetic, but only one read and one write per cell remain. What happens to the total work and the total cycles?",
                "把三个内核合成一个。每步运算都还在，但每个格子只剩一次读、一次写。总运算量和总周期会怎样？",
            ),
            options: [
                Tr::new(
                    "Same arithmetic, clearly fewer cycles",
                    "运算量不变，周期明显下降",
                ),
                Tr::new("Both arithmetic and cycles go down", "运算量和周期都下降"),
                Tr::new("Neither changes", "两者都不变"),
            ],
            answer: 0,
            why: Tr::new(
                "Every multiply and add is still done. What disappears are two full trips to memory, 400 cycles each; the arithmetic hides inside the one wait that is left.",
                "每个乘法和加法都还在做。消失的是两趟完整的内存往返，每趟 400 周期；运算藏进了剩下的那一次等待里。",
            ),
        },
        hints: [
            Tr::new(
                "Open the memory view after a run. Most of the time is spent waiting on LDO, not computing.",
                "运行后打开内存视图。时间大多花在等 LDO 上，而不是计算。",
            ),
            Tr::new(
                "Each pass stores a value only for the next pass to load it back. A register can carry it instead.",
                "每一趟存下一个值，只是为了让下一趟再读回来。寄存器完全可以代劳。",
            ),
            Tr::new(
                "LD once, then MUL, ADD, MUL, MUL, ADD in the same register, then ST once. Seven instructions.",
                "只 LD 一次，在同一个寄存器里依次 MUL、ADD、MUL、MUL、ADD，最后只 ST 一次。一共七条指令。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Arithmetic intensity", "算术强度"),
            line: Tr::new(
                "Count operations per word moved. When that ratio is low, memory sets the speed and extra arithmetic is nearly free.",
                "数一数每搬一个字能做几次运算。比值低的时候，速度由内存决定，多做的运算几乎不要钱。",
            ),
            reality: Tr::new(
                "Roofline models plot exactly this: operations per byte against a chip's compute and bandwidth ceilings. Kernel fusion in GPU compilers exists to raise that ratio.",
                "Roofline 模型画的就是这个：每字节的运算次数，对着芯片的算力与带宽上限。GPU 编译器做内核融合，就是为了抬高这个比值。",
            ),
            myth: Tr::new(
                "Myth: more arithmetic means more time. Here the fused kernel does the same maths in a third of the memory trips.",
                "误解：算得越多越慢。这里融合后的内核算的是同样的数，内存往返只剩三分之一。",
            ),
            simplified: Tr::new(
                "Real chips overlap memory and arithmetic across thousands of threads; Vault has 16 warps and one wait you can see.",
                "真实芯片靠成千上万个线程让访存和运算重叠；Vault 只有 16 个 warp，和一次看得见的等待。",
            ),
        },
        ..base("the-intensity", 1, ALL_WARPS, 512, CACHE_OPS)
    }
}

fn the_wait() -> Task {
    Task {
        title: Tr::new("The Wait", "等待"),
        brief: Tr::new(
            "Bohe has two columns of morning readings and needs them added up. rom[0..512] holds the first column, rom[512..1024] the second; write out[i] = rom[i] + rom[512 + i] for i in 0..512.\nThe program below starts one warp and walks all 512 cells in 16 rounds, waiting on memory every round. Put the first line `.warps N` (1..16) at the top of your program to choose how many warps run. Memory still takes 400 cycles. What can the others do meanwhile?",
            "薄荷有两列早上的读数，要加起来。rom[0..512] 是第一列，rom[512..1024] 是第二列；对 i 在 0..512 写出 out[i] = rom[i] + rom[512 + i]。\n下面的程序只启动一个 warp，分 16 轮走完 512 个格子，每一轮都要等内存。在程序第一行写 `.warps N`（1..16）可以选择启动多少个 warp。内存仍然要 400 周期，其他 warp 在这段时间里能做什么？",
        ),
        cases: || {
            [211, 212, 213]
                .map(|s| rom_case(values(s, 1024, 0, 100)))
                .to_vec()
        },
        hidden: || {
            vec![
                rom_case(values(214, 1024, -50, 50)),
                rom_case(values(215, 1024, -1000, 1000)),
            ]
        },
        expect: |c| cells(|i| c.rom[i] + c.rom[512 + i], 512),
        limit: Some(Goal::new(Metric::Cycles, 600)),
        goals: [Goal::new(Metric::Cycles, 540), Goal::new(Metric::Regs, 2)],
        free_warps: true,
        starter: Source::Code(
            ".warps 1\n; one warp, 16 rounds, a wait every round | 一个 warp，16 轮，每轮都要等\nMOV r3, lane\nLOOP 16\nLD r0, [r3]\nADD r1, r3, 512\nLD r1, [r1]\nADD r0, r0, r1\nST [r3], r0\nADD r3, r3, 32\nENDLOOP\n",
        ),
        reference: Source::Code(
            ".warps 16\nLD r0, [tid]\nADD r1, tid, 512\nLD r1, [r1]\nADD r0, r0, r1\nST [tid], r0\n",
        ),
        predict: Predict {
            question: Tr::new(
                "One warp has just issued a read and must wait 400 cycles for it. What can the other warps do during that wait?",
                "一个 warp 刚发出读请求，要等 400 周期。这段时间里其他 warp 能做什么？",
            ),
            options: [
                Tr::new(
                    "They wait too, the whole chip stalls",
                    "一起等，整块芯片都停住",
                ),
                Tr::new(
                    "They issue their own reads or do other work",
                    "发出自己的读请求，或者做别的运算",
                ),
                Tr::new(
                    "They may only run after the read returns",
                    "必须等读回来才能运行",
                ),
            ],
            answer: 1,
            why: Tr::new(
                "A stall belongs to one warp only. The scheduler issues from whichever warp is ready, so sixteen warps put sixteen requests in flight and share one 400-cycle wait.",
                "停顿只属于那一个 warp。调度器从任何一个就绪的 warp 发射，所以十六个 warp 同时发出十六个请求，共用同一次 400 周期的等待。",
            ),
        },
        hints: [
            Tr::new(
                "Run the starter and watch the memory view: one warp, sixteen waits in a row, the other fifteen warp slots empty.",
                "运行初始程序看内存视图：一个 warp，连着十六次等待，其余十五个 warp 的位置是空的。",
            ),
            Tr::new(
                "Give each warp one slice of the work. `.warps 16` and tid already name a different cell for every thread, so no loop is needed.",
                "每个 warp 只做一小份。`.warps 16` 之后 tid 已经给每个线程指了不同的格子，不需要循环。",
            ),
            Tr::new(
                "Issue both LDs before the ADD, so both requests are in flight together. Two registers are enough: reuse one for the second address and the value.",
                "两次 LD 都先发出去再做 ADD，两个请求就一起在路上。两个寄存器就够：第二个地址和读回的值共用一个。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Hiding latency with warps", "用 warp 藏起延迟"),
            line: Tr::new(
                "A chip cannot make memory faster, so it keeps many warps in flight. While one waits, another issues; the wait is paid once, not once per warp.",
                "芯片没法让内存变快，就让很多 warp 同时在路上。一个在等，另一个在发射；这次等待只付一遍，不是每个 warp 各付一遍。",
            ),
            reality: Tr::new(
                "A GPU keeps tens of resident warps per SM so there is almost always one ready to issue. Occupancy is the name for how many.",
                "GPU 的每个 SM 会同时驻留几十个 warp，几乎总有一个可以发射。驻留了多少，叫占用率（occupancy）。",
            ),
            myth: Tr::new(
                "Myth: a faster loop fixes slow memory. Here the loop is not the cost; the sixteen separate waits are.",
                "误解：把循环写得更紧就能治慢内存。这里代价不在循环，而在那十六次各自的等待。",
            ),
            simplified: Tr::new(
                "Real schedulers weigh registers and shared memory when admitting warps; Vault simply lets you pick up to 16.",
                "真实的调度器要按寄存器和共享内存的用量决定能放几个 warp；Vault 只让你在 1 到 16 里选。",
            ),
        },
        ..base("the-wait", 2, Launch::flat(32), 512, CACHE_OPS)
    }
}

fn cache_line() -> Task {
    Task {
        title: Tr::new("Cache Line", "缓存行"),
        brief: Tr::new(
            "Bohe keeps a short tally sheet: 128 numbers in rom[0..128]. Add them all up and write the total to out[0]. One warp runs; every lane reads the same address, so the lanes agree.\nA global read takes 400 cycles, but memory hands back a whole 32-word line at once and keeps it in the small L1. The program below reads the sheet one number at a time and stops dead at the start of every line. Make it faster.",
            "薄荷有一张小统计单：rom[0..128] 里 128 个数。把它们全部加起来，总数写到 out[0]。只有一个 warp，所有车道读同一个地址，所以大家结果一致。\n一次全局读要 400 周期，但内存一次会交回整整一行 32 个字，放进小小的 L1。下面的程序一个数一个数地读，每到一行的开头就停下来干等。让它更快。",
        ),
        cases: || {
            [221, 222, 223]
                .map(|s| rom_case(values(s, 128, 0, 100)))
                .to_vec()
        },
        hidden: || {
            vec![
                rom_case(values(224, 128, -100, 100)),
                rom_case(values(225, 128, -1000, 1000)),
                rom_case(vec![0; 128]),
            ]
        },
        expect: |c| vec![Some(c.rom[..128].iter().sum())],
        limit: Some(Goal::new(Metric::Cycles, 1250)),
        goals: [Goal::new(Metric::Cycles, 1180), Goal::new(Metric::Size, 14)],
        starter: Source::Code(
            "; one number at a time, a 400-cycle wait at every new line | 一个数一个数读，每到新的一行就等 400 周期\nMOV r0, 0\nMOV r1, 0\nLOOP 4\nLOOP 32\nLD r2, [r1]\nADD r1, r1, 1\nADD r0, r0, r2\nENDLOOP\nENDLOOP\nST [0], r0\n",
        ),
        reference: Source::Code(
            "LD r3, [0]\nLD r4, [32]\nLD r5, [64]\nLD r6, [96]\nMOV r0, 0\nMOV r1, 0\nLOOP 4\nLOOP 32\nLD r2, [r1]\nADD r1, r1, 1\nADD r0, r0, r2\nENDLOOP\nENDLOOP\nST [0], r0\n",
        ),
        predict: Predict {
            question: Tr::new(
                "A program has just read rom[0], which took 400 cycles. Next it reads rom[1], in the same 32-word line. How long does that second read take?",
                "程序刚读完 rom[0]，用了 400 周期。接着读 rom[1]，它在同一行 32 个字里。第二次读要多久？",
            ),
            options: [
                Tr::new("About 400 cycles again", "大约又是 400 周期"),
                Tr::new("A few cycles, an L1 hit", "几个周期，L1 命中"),
                Tr::new(
                    "Zero, it is already in a register",
                    "零周期，已经在寄存器里了",
                ),
            ],
            answer: 1,
            why: Tr::new(
                "The first read brought the whole line into L1. The next 31 words of that line cost only the L1 latency, 4 cycles. Only the first touch of a line pays 400.",
                "第一次读把整行都带进了 L1。这一行里其余 31 个字只付 L1 的延迟，4 周期。只有一行的第一次接触要付 400。",
            ),
        },
        hints: [
            Tr::new(
                "Count the long waits in the memory view: one per line, four in a row, because each read finishes before the next line's read starts.",
                "在内存视图里数长等待：每行一次，一共四次，而且一个接一个，因为上一行读完才轮到下一行。",
            ),
            Tr::new(
                "A read whose result you do not use yet still starts the fill. Touch all four lines first, then the four 400-cycle waits overlap into one.",
                "一次暂时不用结果的读，照样会让内存开始取整行。先把四行各碰一下，四个 400 周期的等待就叠成了一个。",
            ),
            Tr::new(
                "Four LDs with addresses 0, 32, 64, 96 into spare registers, then the same loop as before. Every read in the loop is now a hit.",
                "用四条 LD，地址 0、32、64、96，读进备用寄存器，然后还是原来的循环。循环里的每次读都是命中。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Cache lines", "缓存行"),
            line: Tr::new(
                "Memory moves in lines, not words. The first touch of a line is expensive and the rest are nearly free; touching lines early lets their waits overlap.",
                "内存以行为单位搬运，不是以字。一行的第一次接触很贵，其余几乎免费；提前碰一下各行，它们的等待就能叠在一起。",
            ),
            reality: Tr::new(
                "Real GPU cache lines are 128 bytes and CPUs use 64. Software prefetch instructions exist for exactly this reason.",
                "真实 GPU 的缓存行是 128 字节，CPU 常见 64 字节。软件预取指令就是为这件事设计的。",
            ),
            myth: Tr::new(
                "Myth: every load costs the same. A load costs 400 or 4 depending on whether its line is already here.",
                "误解：每次读的代价都一样。一次读是 400 还是 4，取决于它那一行是否已经在这里。",
            ),
            simplified: Tr::new(
                "Vault has one cache level, 32-word lines and a flat 400-cycle miss; real chips have several levels and varying latency.",
                "Vault 只有一级缓存、32 字的行和固定的 400 周期缺失；真实芯片有多级缓存，延迟各不相同。",
            ),
        },
        ..base("cache-line", 3, Launch::flat(32), 1, CACHE_OPS)
    }
}

fn the_stride() -> Task {
    Task {
        title: Tr::new("The Stride", "步长"),
        brief: Tr::new(
            "Bohe's long tally sheet has 1024 numbers in rom[0..1024]: 32 lines of 32 words. Add them up into out[0]. One warp, every lane reading the same address.\nThe program below is correct, but it takes ages. It walks with a stride of 32: the first word of every line, then the second word of every line, and so on. The L1 holds only 16 lines. Find out why every read pays the full wait, and fix it.",
            "薄荷的长统计单有 1024 个数，在 rom[0..1024]：32 行，每行 32 个字。加起来写到 out[0]。一个 warp，所有车道读同一个地址。\n下面的程序结果是对的，但慢得离谱。它每次跨 32 步：先读每一行的第一个字，再读每一行的第二个字，如此往下。L1 只装得下 16 行。找出为什么每次读都要付满额的等待，然后改掉。",
        ),
        cases: || {
            [231, 232, 233]
                .map(|s| rom_case(values(s, 1024, 0, 100)))
                .to_vec()
        },
        hidden: || {
            vec![
                rom_case(values(234, 1024, -100, 100)),
                rom_case(values(235, 1024, -1000, 1000)),
            ]
        },
        expect: |c| vec![Some(c.rom[..1024].iter().sum())],
        limit: Some(Goal::new(Metric::Cycles, 20000)),
        goals: [
            Goal::new(Metric::Cycles, 15000),
            Goal::new(Metric::Txns, 32),
        ],
        starter: Source::Code(
            "; stride 32: first word of every line, then the second | 步长 32：先读每行第一个字，再读每行第二个\nMOV r0, 0\nMOV r3, 0\nLOOP 32\nMOV r1, r3\nLOOP 32\nLD r2, [r1]\nADD r1, r1, 32\nADD r0, r0, r2\nENDLOOP\nADD r3, r3, 1\nENDLOOP\nST [0], r0\n",
        ),
        reference: Source::Code(
            "MOV r0, 0\nMOV r1, lane\nLOOP 32\nLD r2, [r1]\nADD r1, r1, 32\nADD r0, r0, r2\nENDLOOP\nXOR r4, lane, 16\nSHFL r5, r0, r4\nADD r0, r0, r5\nXOR r4, lane, 8\nSHFL r5, r0, r4\nADD r0, r0, r5\nXOR r4, lane, 4\nSHFL r5, r0, r4\nADD r0, r0, r5\nXOR r4, lane, 2\nSHFL r5, r0, r4\nADD r0, r0, r5\nXOR r4, lane, 1\nSHFL r5, r0, r4\nADD r0, r0, r5\nST [0], r0\n",
        ),
        predict: Predict {
            question: Tr::new(
                "The program reads 1024 words, but the L1 holds only 16 of the 32 lines. Which order of reading do you expect to keep the number of misses lowest?",
                "程序要读 1024 个字，而 L1 只装得下 32 行里的 16 行。哪一种读的顺序，缺失次数最少？",
            ),
            options: [
                Tr::new(
                    "Hop from line to line, one word each",
                    "在各行之间跳，每行读一个字",
                ),
                Tr::new(
                    "Finish one line, then move to the next",
                    "读完一行，再换下一行",
                ),
                Tr::new("The order makes no difference", "顺序没有区别"),
            ],
            answer: 1,
            why: Tr::new(
                "Hopping through 32 lines pushes each line out of the 16-line L1 before you come back for its next word, so every read misses. Finishing a line first costs 32 misses in total.",
                "在 32 行之间跳来跳去，每一行在你回来读它的下一个字之前，就已被挤出只有 16 行的 L1，所以每次读都缺失。读完一行再换行，总共只缺失 32 次。",
            ),
        },
        hints: [
            Tr::new(
                "Open the memory view. Lines are filled, thrown out after one word, and filled again. That waste is the clue.",
                "打开内存视图。整行被取进来，只用了一个字就被挤出去，然后又被取回来。这种浪费就是线索。",
            ),
            Tr::new(
                "More warps will not help: they share the same 16 lines. Change the order so one line is used up before the next arrives.",
                "多开 warp 没用：它们共用同样的 16 行。改变顺序，让一行用完之后再来下一行。",
            ),
            Tr::new(
                "Step the address by 1, not 32, with one pointer through all 1024 words. Only 32 reads miss; the other 992 hit.",
                "地址每次加 1 而不是 32，用一个指针走完 1024 个字。只有 32 次读缺失，其余 992 次命中。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Spatial locality", "空间局部性"),
            line: Tr::new(
                "A line is fetched whole, so use what you fetched. Reading neighbours back to back turns one miss into many hits; hopping makes every read a miss.",
                "一行是整行取来的，取来了就要用上。紧挨着读相邻的字，一次缺失换来许多次命中；跳着读，每次都是缺失。",
            ),
            reality: Tr::new(
                "Loop interchange and tiling are standard compiler passes for this reason. Walking a row-major array column by column is a classic slowdown.",
                "循环交换和分块是编译器的标准手段，原因就在这里。按列走一个按行存放的数组，是经典的变慢方式。",
            ),
            myth: Tr::new(
                "Myth: the same amount of data costs the same time however you walk it. The same 1024 words cost 32 misses or 1024, depending on order.",
                "误解：同样多的数据，怎么走花的时间都一样。同样的 1024 个字，因顺序不同，是 32 次缺失还是 1024 次。",
            ),
            simplified: Tr::new(
                "Real caches have more sets, more ways and a prefetcher that may rescue a stride; Vault's L1 has none of that.",
                "真实的缓存有更多的组和路，还有硬件预取器可能救回规则的步长；Vault 的 L1 都没有。",
            ),
        },
        clue: Some(Clue {
            what: Tr::new(
                "This line was fetched whole, used for one word, then thrown out.",
                "这一行被整行取进来，只用了一个字，就被挤出去了。",
            ),
            find: crate::grade::first_wasted_line,
        }),
        ..base("the-stride", 4, Launch::flat(32), 1, CACHE_OPS)
    }
}

fn the_reuse() -> Task {
    Task {
        title: Tr::new("The Reuse", "复用"),
        brief: Tr::new(
            "Bohe smooths a row of 256 readings by blending each with its two right-hand neighbours: out[i] = rom[i] + 2 * rom[i + 1] + rom[i + 2]. The sheet is rom[0..258].\nOne warp runs; lane l handles i = l + 32j for j = 0..7. Each i needs three reads and neighbouring cells share lines, so most reads should be cheap. The program below is correct but spends the whole time waiting, one new line at a time. Cut the cycles.",
            "薄荷要把一排 256 个读数抹平：每个读数和它右边的两个邻居混合，out[i] = rom[i] + 2 * rom[i + 1] + rom[i + 2]。读数单是 rom[0..258]。\n一个 warp，车道 l 负责 i = l + 32j（j = 0..7）。每个 i 要读三个字，相邻的格子共用同一行，所以大多数读取应该很便宜。下面的程序结果是对的，但时间全花在等上了，一次只等一个新行。把周期降下来。",
        ),
        cases: || {
            [241, 242, 243]
                .map(|s| rom_case(values(s, 258, -50, 50)))
                .to_vec()
        },
        hidden: || {
            let swing: Vec<i32> = (0..258)
                .map(|i| if i % 2 == 0 { 100_000 } else { -100_000 })
                .collect();
            vec![
                rom_case(values(244, 258, -1000, 1000)),
                rom_case(values(245, 258, -50, 50)),
                rom_case(vec![7; 258]),
                rom_case(swing),
            ]
        },
        expect: |c| cells(|i| c.rom[i] + 2 * c.rom[i + 1] + c.rom[i + 2], 256),
        limit: Some(Goal::new(Metric::Cycles, 750)),
        goals: [Goal::new(Metric::Cycles, 520), Goal::new(Metric::Regs, 8)],
        starter: Source::Code(
            "; three reads per cell, one new line per round | 每格三次读，每轮碰一个新行\nMOV r0, lane\nLOOP 8\nLD r1, [r0]\nADD r2, r0, 1\nLD r2, [r2]\nADD r3, r0, 2\nLD r3, [r3]\nSHL r2, r2, 1\nADD r1, r1, r2\nADD r1, r1, r3\nST [r0], r1\nADD r0, r0, 32\nENDLOOP\n",
        ),
        reference: Source::Code(
            "MIN r4, lane, 8\nSHL r4, r4, 5\nLD r5, [r4]\nMOV r0, lane\nLOOP 8\nLD r1, [r0]\nADD r2, r0, 1\nLD r2, [r2]\nADD r3, r0, 2\nLD r3, [r3]\nSHL r2, r2, 1\nADD r1, r1, r2\nADD r1, r1, r3\nST [r0], r1\nADD r0, r0, 32\nENDLOOP\n",
        ),
        predict: Predict {
            question: Tr::new(
                "A lane has read rom[i] and now reads rom[i + 1] from the same line. How long does that second read take?",
                "一条车道刚读了 rom[i]，接着读同一行里的 rom[i + 1]。第二次读要多久？",
            ),
            options: [
                Tr::new("Another 400 cycles", "又是 400 周期"),
                Tr::new(
                    "A few cycles: the line is already in L1",
                    "几个周期：这一行已经在 L1 里",
                ),
                Tr::new("It depends on the value read", "取决于读到的值"),
            ],
            answer: 1,
            why: Tr::new(
                "Neighbouring words share a line. The first read pays 400 and the next two hit in L1 for 4 cycles each, so the whole cost is the first touch of each of the 9 lines.",
                "相邻的字同在一行。第一次读付 400，后面两次在 L1 里各只要 4 周期，所以总代价就是 9 行各自的第一次接触。",
            ),
        },
        hints: [
            Tr::new(
                "Reads hit in L1 already. What is left is nine first-touch waits, taken one per round: the next line is only requested when the round needs it.",
                "读取本身已经命中 L1。剩下的是九次第一次接触的等待，一轮一个：下一行要到那一轮用到时才被请求。",
            ),
            Tr::new(
                "Request all nine lines before the loop starts. One LD with a different address in every lane can start many lines at once, into a single register.",
                "在循环开始之前，把九行都请求出来。一条 LD，让每条车道的地址不同，就能同时启动许多行，只占一个寄存器。",
            ),
            Tr::new(
                "Lane l asks for line min(l, 8): r4 = MIN lane, 8, SHL by 5, then LD r5, [r4]. Lines 0..8 are all in flight together; run the old loop after.",
                "车道 l 请求第 min(l, 8) 行：r4 = MIN lane, 8，左移 5，再 LD r5, [r4]。第 0..8 行同时上路；之后照旧跑原来的循环。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Temporal locality", "时间局部性"),
            line: Tr::new(
                "Data you touched a moment ago is still in the cache. Reuse it while it is there, and start the lines you will need before you need them.",
                "刚碰过的数据还在缓存里。趁它在就复用它，并在需要之前先把将要用的行启动起来。",
            ),
            reality: Tr::new(
                "Stencil and convolution kernels on GPUs live on this: neighbouring outputs share inputs, and the cache or shared memory supplies the repeats.",
                "GPU 上的模板和卷积核全靠这个：相邻的输出共用输入，重复的读取由缓存或共享内存来供给。",
            ),
            myth: Tr::new(
                "Myth: three reads per output means three times the memory cost. Here the repeats are L1 hits; only 9 lines are ever fetched.",
                "误解：每个输出读三次，内存代价就是三倍。这里重复的读都是 L1 命中，总共只取了 9 行。",
            ),
            simplified: Tr::new(
                "Real stencils tile into shared memory or registers; Vault lets one wide load stand in for prefetching.",
                "真实的模板计算会分块放进共享内存或寄存器；Vault 里用一条宽读取代替预取。",
            ),
        },
        ..base("the-reuse", 5, Launch::flat(32), 256, CACHE_OPS)
    }
}

/// A 32x32 matrix (one cache line per row) followed by a vector x of 32 values.
fn matvec(seed: u64) -> Vec<i32> {
    values(seed, 1056, -9, 10)
}

fn matvec_with(seed: u64, zero_row: Option<usize>, unit_x: Option<usize>) -> Case {
    let mut rom = matvec(seed);
    if let Some(r) = zero_row {
        rom[r * 32..r * 32 + 32].fill(0);
    }
    if let Some(k) = unit_x {
        rom[1024..1056].fill(0);
        rom[1024 + k] = 1;
    }
    rom_case(rom)
}

fn the_capacity() -> Task {
    Task {
        title: Tr::new("The Capacity", "容量"),
        brief: Tr::new(
            "Bohe's ward-by-test table is a 32 x 32 grid of weights in rom[0..1024], one row per cache line, followed by a weight vector x in rom[1024..1056]. Write out[r] = sum over c of A[r][c] * x[c] for the 32 rows.\nOne warp; lane r owns row r. The program below sets all 32 lanes walking along their rows at once. It is right, but 32 rows plus the x line is twice what the 16-line L1 can hold. Rearrange the work so the cache can keep up.",
            "薄荷的病房检测对照表是 rom[0..1024] 里一张 32 x 32 的权重表，一行一个缓存行，后面 rom[1024..1056] 是权重向量 x。对 32 行写出 out[r] = 对 c 求和 A[r][c] * x[c]。\n一个 warp；车道 r 负责第 r 行。下面的程序让 32 条车道同时沿各自的行往前走。结果是对的，但 32 行加上 x 那一行，是 16 行 L1 容量的两倍多。重新安排一下工作，让缓存跟得上。",
        ),
        cases: || [251, 252, 253].map(|s| matvec_with(s, None, None)).to_vec(),
        hidden: || {
            vec![
                matvec_with(254, Some(5), None),
                matvec_with(255, Some(31), Some(17)),
                matvec_with(256, None, Some(0)),
            ]
        },
        expect: |c| {
            cells(
                |r| (0..32).map(|k| c.rom[r * 32 + k] * c.rom[1024 + k]).sum(),
                32,
            )
        },
        limit: Some(Goal::new(Metric::Cycles, 3300)),
        goals: [Goal::new(Metric::Cycles, 3100), Goal::new(Metric::Size, 21)],
        starter: Source::Code(
            "; all 32 rows at once: 33 lines in a 16-line cache | 32 行同时走：33 行塞进只有 16 行的缓存\nSHL r1, lane, 5\nMOV r0, 0\nMOV r3, 0\nLOOP 32\nADD r4, r1, r3\nLD r4, [r4]\nADD r5, r3, 1024\nLD r5, [r5]\nADD r3, r3, 1\nMUL r4, r4, r5\nADD r0, r0, r4\nENDLOOP\nST [lane], r0\n",
        ),
        reference: Source::Code(
            "SHL r1, lane, 5\nSHR r2, lane, 3\nMOV r0, 0\nMOV r6, 0\nLOOP 4\nSETP.EQ p0, r2, r6\nIF p0\nMOV r3, 0\nLOOP 32\nADD r4, r1, r3\nLD r4, [r4]\nADD r5, r3, 1024\nLD r5, [r5]\nADD r3, r3, 1\nMUL r4, r4, r5\nADD r0, r0, r4\nENDLOOP\nENDIF\nADD r6, r6, 1\nENDLOOP\nST [lane], r0\n",
        ),
        predict: Predict {
            question: Tr::new(
                "All 32 lanes step along their own rows together, so each round touches 32 different lines plus the x line, twice what the L1 holds. What fraction of those reads hit in L1?",
                "32 条车道一起沿各自的行往前走，每一轮要碰 32 个不同的行，外加 x 那一行，是 L1 容量的两倍。这些读取里有多大比例命中 L1？",
            ),
            options: [
                Tr::new("Almost all of them", "几乎全部"),
                Tr::new("Almost none of them", "几乎一个也没有"),
                Tr::new("About half", "大约一半"),
            ],
            answer: 1,
            why: Tr::new(
                "Each line is used once per round, and by the time the next round comes round to it, 32 other lines have pushed it out. The cache throws away exactly what is needed next, so nearly every read misses.",
                "每一行每轮只用一次，等下一轮回来时，已经有 32 个别的行把它挤走了。缓存总是把下一步马上要用的丢掉，所以几乎每次读都缺失。",
            ),
        },
        hints: [
            Tr::new(
                "In the memory view the same lines are fetched round after round. Count the distinct lines one round needs against the 16 the L1 holds.",
                "在内存视图里，同样的几行一轮又一轮地被取来。数一数一轮需要的不同行数，对照 L1 的 16 行。",
            ),
            Tr::new(
                "Do fewer rows at a time. Eight rows plus the x line is 9 lines, which fit. Work through the 32 rows as 4 groups of 8, and let the other lanes sit out with IF.",
                "一次少做几行。八行加上 x 那一行是 9 行，装得下。把 32 行分成 4 组、每组 8 行来做，其余车道用 IF 歇着。",
            ),
            Tr::new(
                "Group g is the lanes whose lane >> 3 equals g. LOOP 4 over g, SETP.EQ, then IF around the whole inner LOOP 32. Idle lanes keep their accumulator, and every lane stores at the end.",
                "第 g 组是 lane >> 3 等于 g 的那些车道。对 g 做 LOOP 4，SETP.EQ，然后把整个内层 LOOP 32 包进 IF。歇着的车道保持自己的累加器不动，最后每条车道都存结果。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Working set and tiling", "工作集与分块"),
            line: Tr::new(
                "A cache only helps while the data you keep reusing fits in it. When it does not, shrink the piece you work on until it does, then move to the next piece.",
                "缓存只有在你反复用的数据装得下时才有用。装不下，就把手头这一块缩小到装得下，做完再换下一块。",
            ),
            reality: Tr::new(
                "Tiled matrix multiply is the textbook case: blocks of the matrices are sized to fit cache or shared memory, so each element is fetched once per block instead of once per use.",
                "分块矩阵乘法是教科书式的例子：矩阵的块按缓存或共享内存的容量来定大小，每个元素每块只取一次，而不是每次用都取。",
            ),
            myth: Tr::new(
                "Myth: doing everything at once is the fastest way. Past the cache's capacity, doing less at a time is faster.",
                "误解：一次全做完最快。超过缓存容量之后，一次少做一点反而更快。",
            ),
            simplified: Tr::new(
                "Real caches have conflict and capacity misses mixed together, and the compiler picks tile sizes; here the arithmetic is 8 rows plus one line.",
                "真实缓存里冲突缺失和容量缺失混在一起，分块大小由编译器去选；这里只要算：8 行加一行。",
            ),
        },
        ..base("the-capacity", 6, Launch::flat(32), 32, CACHE_OPS)
    }
}

/// Eight rows with a row stride of 128 words; only the first 32 words of each row are data.
fn alias_case(seed: u64, lo: i32, hi: i32) -> Case {
    rom_case(values(seed, 7 * 128 + 32, lo, hi))
}

fn the_alias() -> Task {
    Task {
        title: Tr::new("The Alias", "同组"),
        brief: Tr::new(
            "Bohe's eight wards each keep a 32-word daily sheet, but the sheets sit 128 words apart: ward k starts at rom[128k]. Lane c adds up column c over the eight wards, then weights the four shifts of the day by 1, 2, 3 and 4: out[c] = sum over passes p = 0..3 of (p + 1) * (sum over wards k of rom[128k + c]).\nThe program below does it pass by pass, walking all eight wards four times. Eight lines is only half of the 16-line L1, yet every read misses. Find out why and fix it.",
            "薄荷的八个病房各有一张 32 个字的日报，但这些日报相隔 128 个字：病房 k 从 rom[128k] 开始。车道 c 把第 c 列在八个病房里加起来，再按一天四个班次的权重 1、2、3、4 加权：out[c] = 对第 p = 0..3 遍求和 (p + 1) * (对病房 k 求和 rom[128k + c])。\n下面的程序一遍一遍地做，把八个病房走了四遍。八行只占 16 行 L1 的一半，可每次读都缺失。找出原因，改掉它。",
        ),
        cases: || [261, 262, 263].map(|s| alias_case(s, -9, 10)).to_vec(),
        hidden: || vec![alias_case(264, -50, 50), alias_case(265, 0, 100)],
        expect: |c| {
            cells(
                |col| 10 * (0..8).map(|k| c.rom[128 * k + col]).sum::<i32>(),
                32,
            )
        },
        limit: Some(Goal::new(Metric::Cycles, 3300)),
        goals: [Goal::new(Metric::Cycles, 900), Goal::new(Metric::Size, 18)],
        starter: Source::Code(
            "; four passes over the eight wards | 把八个病房走四遍\nMOV r0, 0\nMOV r3, 1\nLOOP 4\nMOV r1, lane\nLOOP 8\nLD r2, [r1]\nADD r1, r1, 128\nMUL r2, r2, r3\nADD r0, r0, r2\nENDLOOP\nADD r3, r3, 1\nENDLOOP\nST [lane], r0\n",
        ),
        reference: Source::Code(
            "MOV r0, 0\nMOV r1, lane\nLOOP 2\nADD r3, r1, 128\nADD r4, r1, 256\nADD r5, r1, 384\nLD r2, [r1]\nLD r3, [r3]\nLD r4, [r4]\nLD r5, [r5]\nADD r1, r1, 512\nADD r2, r2, r3\nADD r4, r4, r5\nADD r2, r2, r4\nADD r0, r0, r2\nENDLOOP\nMUL r0, r0, 10\nST [lane], r0\n",
        ),
        predict: Predict {
            question: Tr::new(
                "Eight lines are far fewer than the 16 the L1 holds, yet the program re-fetches them every pass. What could make lines collide when there is plenty of room?",
                "八行远少于 L1 能装的 16 行，程序却每一遍都重新取它们。空间明明很富余，是什么让这些行互相冲突？",
            ),
            options: [
                Tr::new("The cache is smaller than it says", "缓存实际比标称的小"),
                Tr::new(
                    "All eight lines map to the same set of 4 ways",
                    "八行全都映射到同一组，那一组只有 4 路",
                ),
                Tr::new(
                    "Reads of different lanes cancel each other",
                    "不同车道的读取互相抵消",
                ),
            ],
            answer: 1,
            why: Tr::new(
                "A line goes to set (line number mod 4). Rows 128 words apart are 4 lines apart, so all eight land in set 0, which holds only 4. The other 3 sets sit empty while set 0 thrashes.",
                "一行放进第（行号 mod 4）组。相隔 128 个字的行，行号相差 4，所以八行全落在第 0 组，而它只能装 4 行。其余 3 组空着，第 0 组却在反复被换出。",
            ),
        },
        hints: [
            Tr::new(
                "Look at the memory view's refills: the same line is thrown out and fetched again. The set number of each line is its line index mod 4.",
                "看内存视图里的重新装入：同一行被挤出去又取回来。每一行所在的组号，是它的行号对 4 取余。",
            ),
            Tr::new(
                "You cannot move the data, but you can change when it is used. Take everything you need from a row in one visit, so it never has to come back.",
                "数据挪不动，但可以改它被用的时间。一次造访就把这一行要用的全部拿走，它就不必再被取回来。",
            ),
            Tr::new(
                "Sum the eight rows once and multiply by 1 + 2 + 3 + 4 = 10 at the end. Reading four rows before adding lets those misses overlap too.",
                "八行只加一遍，最后乘以 1 + 2 + 3 + 4 = 10。先读四行再相加，这些缺失还能叠在一起。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Cache sets and conflict misses", "缓存的组与冲突缺失"),
            line: Tr::new(
                "A cache is split into sets and each line may only live in one of them. Addresses a power of two apart pile into one set, however empty the rest of the cache is.",
                "缓存分成若干组，每一行只能住进其中一组。地址相隔 2 的幂次的数据会挤进同一组，不管缓存其余部分有多空。",
            ),
            reality: Tr::new(
                "Power-of-two strides in matrix code are a famous slowdown. The usual cures are padding rows by one line or reordering loops so a row is finished in one visit.",
                "矩阵代码里 2 的幂次步长是出了名的变慢原因。常见的解法是每行多垫一个缓存行，或者调整循环顺序，让一行一次用完。",
            ),
            myth: Tr::new(
                "Myth: a cache that is half empty cannot be thrashing. It can, if everything you use hashes to the same set.",
                "误解：缓存还空着一半，就不可能被冲垮。如果你用的东西全哈希到同一组，照样会。",
            ),
            simplified: Tr::new(
                "Vault's L1 is 4 sets of 4 ways with a plain mod-4 index; real caches hash more and have more ways.",
                "Vault 的 L1 是 4 组、每组 4 路，只用简单的对 4 取余；真实缓存会做更多哈希，路数也更多。",
            ),
        },
        clue: Some(Clue {
            what: Tr::new(
                "This line was fetched, evicted by its set-mates and fetched again.",
                "这一行被取进来，被同组的别的行挤走，然后又取了一遍。",
            ),
            find: crate::grade::first_refill,
        }),
        ..base("the-alias", 7, Launch::flat(32), 32, CACHE_OPS)
    }
}

/// 512 patient records of 8 words (age, dose, ward, flag, four spare) and, in
/// rom[4096], the age line. Modes: 0 mixed, 1 everyone matches, 2 nobody matches,
/// 3 mostly exactly on the age line, 4 all elderly.
fn ledger(seed: u64, mode: u8, line: i32) -> Case {
    let mut r = rng(seed);
    let mut rom = Vec::with_capacity(4097);
    for _ in 0..512 {
        let (age, flag) = match mode {
            1 => (60 + below(&mut r, 40), 1),
            2 => {
                let a = 1 + below(&mut r, 99);
                (a, i32::from(a < 60))
            }
            3 => {
                let a = if below(&mut r, 10) < 8 {
                    60
                } else {
                    1 + below(&mut r, 99)
                };
                (a, below(&mut r, 2))
            }
            4 => (70 + below(&mut r, 30), below(&mut r, 2)),
            _ => (1 + below(&mut r, 99), below(&mut r, 2)),
        };
        let dose = below(&mut r, 1000);
        let ward = below(&mut r, 12);
        rom.extend([age, dose, ward, flag]);
        rom.extend((0..4).map(|_| below(&mut r, 100)));
    }
    rom.push(line);
    rom_case(rom)
}

/// The showcase: today's ledger, four of them in turn (the last one all elderly),
/// with the age line dragged between 40 and 80.
fn ledger_live(frame: i32, at: [i32; 2]) -> Case {
    let k = (frame / 45).rem_euclid(4);
    let line = (60 + 5 * (at[0] - 4)).clamp(40, 80);
    match k {
        3 => ledger(274, 4, line),
        k => ledger(271 + k as u64, 0, line),
    }
}

fn clinic_ledger() -> Task {
    Task {
        title: Tr::new("The Clinic Ledger", "诊所台账"),
        brief: Tr::new(
            "Bohe needs today's ledger tallied before closing. rom[0..4096] is 512 patient records of 8 words each: age, dose, ward, flag, then four spare words. The clinic's age line for today is in rom[4096] (usually 60); the flag is 0 or 1.\nWrite out[0] = how many patients are at or above the age line and flagged, and out[1] = the sum of their doses. Choose how many warps with `.warps N` if you like.\nEvery record sits in a 32-word line with three others, and every line costs 400 cycles to fetch. Fewer trips and fewer instructions both count.",
            "薄荷想在关门前把今天的台账结清。rom[0..4096] 是 512 条病历，每条 8 个字：年龄、剂量、病房、标志，后面四个字备用。今天的年龄线在 rom[4096]（通常是 60）；标志是 0 或 1。\n写出 out[0] = 年龄不低于年龄线且打了标志的病人数，out[1] = 他们的剂量之和。想用几个 warp，可以用 `.warps N` 选。\n每条病历和另外三条同住一个 32 字的行，每取一行要 400 周期。少跑几趟、少写几条指令，都算数。",
        ),
        cases: || (271..274u64).map(|s| ledger(s, 0, 60)).collect(),
        hidden: || vec![ledger(274, 1, 60), ledger(275, 2, 60), ledger(276, 3, 60)],
        expect: |c| {
            let line = c.rom[4096];
            let hit = |r: &[i32]| r[0] >= line && r[3] != 0;
            let rows = c.rom[..4096].chunks(8).filter(|r| hit(r));
            vec![
                Some(rows.clone().count() as i32),
                Some(rows.map(|r| r[1]).sum()),
            ]
        },
        limit: Some(Goal::new(Metric::Cycles, 8000)),
        goals: [Goal::new(Metric::Cycles, 4000), Goal::new(Metric::Size, 39)],
        free_warps: true,
        live: Some(ledger_live),
        steer: Some(Tr::new("MOVE THE AGE LINE", "移动年龄线")),
        starter: Source::Code(
            "; record = 8 words: age, dose, ward, flag, ... | 每条病历 8 个字：年龄、剂量、病房、标志……\n; age line in rom[4096]; out[0] = count, out[1] = dose sum | 年龄线在 rom[4096]；out[0] 是人数，out[1] 是剂量和\nLD r3, [4096]\n",
        ),
        reference: Source::Code(
            "SHL r2, lane, 3\nLD r3, [4096]\nLOOP 8\nLD r4, [r2]\nADD r2, r2, 1\nLD r5, [r2]\nADD r2, r2, 2\nLD r6, [r2]\nADD r2, r2, 253\nLD r7, [r2]\nADD r2, r2, 1\nLD r8, [r2]\nADD r2, r2, 2\nLD r9, [r2]\nADD r2, r2, 253\nSETP.GE p0, r4, r3\nSEL r6, p0, r6, 0\nADD r0, r0, r6\nMUL r5, r5, r6\nADD r1, r1, r5\nSETP.GE p0, r7, r3\nSEL r9, p0, r9, 0\nADD r0, r0, r9\nMUL r8, r8, r9\nADD r1, r1, r8\nENDLOOP\nSHL r1, r1, 10\nADD r0, r0, r1\nMOV r10, 16\nLOOP 5\nXOR r11, lane, r10\nSHFL r12, r0, r11\nADD r0, r0, r12\nSHR r10, r10, 1\nENDLOOP\nAND r13, r0, 1023\nSHR r14, r0, 10\nST [0], r13\nST [1], r14\n",
        ),
        predict: Predict {
            question: Tr::new(
                "Each record's age, dose and flag are words 0, 1 and 3 of the same 8-word record. If you read the age first and then the dose and flag of the same record, which of those reads pay the 400-cycle wait?",
                "每条病历的年龄、剂量、标志是同一条 8 字记录里的第 0、1、3 个字。先读年龄，再读同一条的剂量和标志，这几次读里哪些要付 400 周期？",
            ),
            options: [
                Tr::new("All three", "三次都要"),
                Tr::new(
                    "Only the first; the other two share its line",
                    "只有第一次；另外两次和它同一行",
                ),
                Tr::new("Only the last", "只有最后一次"),
            ],
            answer: 1,
            why: Tr::new(
                "A record never straddles a line, so the first read fetches all of it. The other fields cost the L1 latency, 4 cycles, or simply wait for the same fill already on its way.",
                "一条病历不会跨行，所以第一次读就把它整个取来了。其余字段只付 L1 的延迟 4 周期，或者干脆等同一次正在路上的取行。",
            ),
        },
        hints: [
            Tr::new(
                "Lane l takes record l + 32j. A wide read of the age field touches 8 lines at once; the dose and flag reads of that slab then hit the same lines.",
                "车道 l 取第 l + 32j 条病历。一次读年龄字段会同时碰 8 个行；同一批的剂量和标志读取落在同样的行上，直接命中。",
            ),
            Tr::new(
                "The wait is per trip, not per read. Take two slabs of records per round (all six reads first, then the arithmetic) and you pay half as many 400s.",
                "等待是按趟算的，不是按读算的。每一轮取两批病历（六次读先发出去，再做运算），400 周期就只付一半的次数。",
            ),
            Tr::new(
                "Keep a count and a dose sum per lane; select the dose by flag and age with SEL. At the end fold the lanes with a SHFL butterfly. To fold both numbers at once, pack the sum above the count: sum * 1024 + count.",
                "每条车道各存一个人数和一个剂量和；用 SEL 按标志与年龄筛出剂量。最后用 SHFL 蝶形把车道合并。想一次合并两个数，可以把和放在人数的上面：和 * 1024 + 人数。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Moving less memory", "少搬内存"),
            line: Tr::new(
                "When a job is memory-bound, count trips to memory, not instructions. Take all you need from a line in one visit, and put as many independent requests in flight as you can.",
                "任务受内存限制时，要数的是去内存的趟数，不是指令条数。一行一次造访就拿齐要用的，同时让尽量多的独立请求在路上。",
            ),
            reality: Tr::new(
                "Database scans and analytics kernels live here: columnar layouts, fused filters and batched loads all exist to cut trips to memory.",
                "数据库扫描和分析型内核就活在这里：列式存储、融合过滤、批量读取，都是为了减少去内存的趟数。",
            ),
            myth: Tr::new(
                "Myth: fewer instructions means faster. Here the longer program with two slabs per round is the faster one.",
                "误解：指令越少越快。这里每轮取两批的那个更长的程序，反而更快。",
            ),
            simplified: Tr::new(
                "Real kernels split the ledger over many warps and combine with atomics or shared memory; Vault has no tool for that yet in this chapter.",
                "真实的内核会把台账分给很多个 warp，再用原子操作或共享内存合并；这一章里 Vault 还没有这样的工具。",
            ),
        },
        ..base("clinic-ledger", 8, Launch::flat(32), 2, CACHE_OPS)
    }
}
