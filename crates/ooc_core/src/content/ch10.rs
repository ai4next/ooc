//! Chapter 10 · Atlas: grids, blocks and stencils. Client: Yinpo, by the well.

use oo_base::Tr;

use super::atlas::{ATLAS_OPS, base};
use super::{below, rng};
use crate::task::*;
use crate::vm::Launch;

/// What the guard strip behind the data holds before the run.
const GUARD: i32 = -7;

pub fn tasks() -> Vec<Task> {
    vec![
        the_grid(),
        grid_2d(),
        grid_stride(),
        the_collide(),
        the_stencil(),
        the_analyzer(),
        the_race(),
        well_seep(),
    ]
}

/// `n` random values in `lo..hi`.
fn values(seed: u64, n: usize, lo: i32, hi: i32) -> Vec<i32> {
    let mut r = rng(seed);
    (0..n).map(|_| lo + below(&mut r, hi - lo)).collect()
}

/// An output buffer of `len` cells that starts out all guard.
fn guard_init(len: usize) -> Vec<i32> {
    vec![GUARD; len]
}

/// Expected cells: `f(i)` for the first `n`, the untouched guard after that.
fn guarded(n: usize, len: usize, f: impl Fn(usize) -> i32) -> Vec<Option<i32>> {
    (0..len)
        .map(|i| Some(if i < n { f(i) } else { GUARD }))
        .collect()
}

fn case(rom: Vec<i32>, frame: i32) -> Case {
    Case { rom, frame }
}

// ---------------------------------------------------------------- 10-1

/// rom[0] = n, rom[1..=n] = the readings.
fn tablet(seed: u64, n: usize) -> Case {
    let mut rom = vec![n as i32];
    rom.extend(values(seed, n, 0, 100));
    case(rom, 0)
}

fn the_grid() -> Task {
    Task {
        title: Tr::new("The Grid", "网格"),
        brief: Tr::new(
            "Yinpo keeps her well readings on a stone tablet: rom[0] is how many there are, n, and the readings follow in rom[1..=n].\nTriple each one and add 1: out[i] = in[i]·3 + 1.\nThe kernel is launched as blocks of 64 threads, ⌈n/64⌉ of them, so there may be threads left over past the end. Behind the answers lies a strip of guard cells. Anything written there is a mistake.",
            "银婆把井水读数刻在一块石板上：rom[0] 是个数 n，读数依次放在 rom[1..=n]。\n把每个读数乘 3 再加 1：out[i] = in[i]·3 + 1。\n内核按每块 64 个线程来发射，共 ⌈n/64⌉ 块，所以末尾可能多出几个线程。答案后面还有一段守卫格，写进去就是错。",
        ),
        cases: || vec![tablet(1001, 128), tablet(1002, 192), tablet(1003, 256)],
        hidden: || vec![tablet(1004, 130), tablet(1005, 201), tablet(1006, 63)],
        init: |_| guard_init(320),
        expect: |c| {
            let n = c.rom[0] as usize;
            guarded(n, 320, |i| c.rom[1 + i] * 3 + 1)
        },
        grid: Some(|c| Launch::grid((c.rom[0] as usize).div_ceil(64), 64)),
        goals: [Goal::new(Metric::Cycles, 900), Goal::new(Metric::Size, 11)],
        starter: Source::Code(
            "; i = bx * bdx + tx is this thread's place in the whole array | i 是这个线程在整个数组里的位置\n; n is in rom[0], the readings from rom[1] | n 在 rom[0]，读数从 rom[1] 开始\n; threads past the end must not write | 越过末尾的线程不许写\n",
        ),
        reference: Source::Code(
            "LD r1, [0]\nMUL r0, bx, bdx\nADD r0, r0, tx\nSETP.LT p0, r0, r1\nIF p0\nADD r2, r0, 1\nLD r3, [r2]\nMUL r3, r3, 3\nADD r3, r3, 1\nST [r0], r3\nENDIF\n",
        ),
        predict: Predict {
            question: Tr::new(
                "There are n = 130 readings and each block has 64 threads. How many blocks are needed to cover them?",
                "n = 130 个读数，每块 64 个线程。要开几个块才够覆盖？",
            ),
            options: [Tr::new("2", "2"), Tr::new("3", "3"), Tr::new("4", "4")],
            answer: 1,
            why: Tr::new(
                "Two blocks cover 128, two short. Three cover 192, which is 62 more than needed. The grid is rounded up, so the last block is only partly useful.",
                "两块只覆盖 128 个，差 2 个。三块覆盖 192 个，多出 62 个。网格是向上取整的，所以最后一块只有一部分有用。",
            ),
        },
        hints: [
            Tr::new(
                "Each thread must know where it sits in the whole array, not just in its block. Which two specials give that?",
                "每个线程要知道自己在整个数组里的位置，而不只是在块里的位置。哪两个特殊寄存器能给出它？",
            ),
            Tr::new(
                "i = bx · bdx + tx. Threads with i ≥ n have nothing to do: compare with SETP and put the store inside an IF.",
                "i = bx · bdx + tx。i ≥ n 的线程无事可做：用 SETP 比较，把 ST 放进 IF 里。",
            ),
            Tr::new(
                "LD n, compute i, SETP.LT p0, i, n, then IF p0: load rom[i + 1], multiply by 3, add 1, ST [i].",
                "LD n，算出 i，SETP.LT p0, i, n，再 IF p0：读 rom[i + 1]，乘 3，加 1，ST [i]。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Global index and the ragged tail", "全局下标与参差的尾部"),
            line: Tr::new(
                "A grid is made of whole blocks, so the thread count is rounded up. The threads past the end must check before they write.",
                "网格由整块组成，所以线程数是向上取整的。越过末尾的线程在写之前必须先检查。",
            ),
            reality: Tr::new(
                "Every CUDA kernel starts with `i = blockIdx.x * blockDim.x + threadIdx.x; if (i < n)`. Forgetting the check corrupts whatever memory lies after the array.",
                "每个 CUDA 内核都以 `i = blockIdx.x * blockDim.x + threadIdx.x; if (i < n)` 开头。漏掉检查，数组后面的内存就被写坏了。",
            ),
            myth: Tr::new(
                "Myth: lengths are multiples of the block size. Real data never is; only the tail shows it.",
                "误解：长度总是块大小的倍数。真实数据从来不是；只有尾部会暴露这一点。",
            ),
            simplified: Tr::new(
                "Here `tid` happens to equal bx · bdx + tx in a one-dimensional grid. In two dimensions it no longer does, as the next level shows.",
                "在一维网格里，`tid` 恰好等于 bx · bdx + tx。到了二维就不再相等，下一关会看到。",
            ),
        },
        ..base("the-grid", 1, Launch::grid(2, 64), 320, ATLAS_OPS)
    }
}

// ---------------------------------------------------------------- 10-2

/// rom[0] = W, rom[1] = H, rom[2..] = the image, row-major.
fn image(seed: u64, w: usize, h: usize) -> Case {
    let mut rom = vec![w as i32, h as i32];
    rom.extend(values(seed, w * h, 0, 256));
    case(rom, 0)
}

fn grid_2d() -> Task {
    Task {
        title: Tr::new("Two Dimensions", "二维网格"),
        brief: Tr::new(
            "The wind tower's pressure plate is an image: rom[0] is its width W, rom[1] its height H, and the W·H cells follow row by row from rom[2].\nFor every cell, out[y·W + x] = (in[y][x] + x·y) AND 255.\nThe kernel is launched as 16 × 16 blocks in a two-dimensional grid. W and H need not be multiples of 16, and the guard cells behind the image must stay as they are.",
            "风塔的压力板是一幅图：rom[0] 是宽 W，rom[1] 是高 H，从 rom[2] 起按行排着 W·H 个格子。\n对每个格子：out[y·W + x] = (in[y][x] + x·y) AND 255。\n内核按 16 × 16 的块、二维网格来发射。W 和 H 不一定是 16 的倍数，图后面的守卫格必须保持原样。",
        ),
        cases: || {
            vec![
                image(1011, 32, 32),
                image(1012, 48, 16),
                image(1013, 64, 32),
            ]
        },
        hidden: || {
            vec![
                image(1014, 40, 24),
                image(1015, 17, 33),
                image(1016, 64, 16),
            ]
        },
        init: |_| guard_init(2304),
        expect: |c| {
            let (w, h) = (c.rom[0] as usize, c.rom[1] as usize);
            guarded(w * h, 2304, |i| {
                let (x, y) = ((i % w) as i32, (i / w) as i32);
                (c.rom[2 + i] + x * y) & 255
            })
        },
        grid: Some(|c| {
            let (w, h) = (c.rom[0] as usize, c.rom[1] as usize);
            Launch::grid2(w.div_ceil(16), h.div_ceil(16), 16, 16)
        }),
        goals: [Goal::new(Metric::Cycles, 1200), Goal::new(Metric::Size, 20)],
        starter: Source::Code(
            "; x = bx * bdx + tx, y = by * bdy + ty | x、y 是整幅图里的列和行\n; W = rom[0], H = rom[1], pixels from rom[2] | 宽 W、高 H，像素从 rom[2] 开始\n; both x < W and y < H must hold before writing | 写之前 x < W 和 y < H 都要成立\n",
        ),
        reference: Source::Code(
            "LD r0, [0]\nLD r1, [1]\nMUL r2, bx, bdx\nADD r2, r2, tx\nMUL r3, by, bdy\nADD r3, r3, ty\nSETP.LT p0, r2, r0\nSETP.LT p1, r3, r1\nIF p0\nIF p1\nMUL r4, r3, r0\nADD r4, r4, r2\nADD r5, r4, 2\nLD r5, [r5]\nMUL r6, r2, r3\nADD r5, r5, r6\nAND r5, r5, 255\nST [r4], r5\nENDIF\nENDIF\n",
        ),
        predict: Predict {
            question: Tr::new(
                "The image is 40 wide and 24 high, and blocks are 16 × 16. How big must the grid be, in blocks across × blocks down?",
                "图宽 40、高 24，块是 16 × 16。网格要多大？（横向块数 × 纵向块数）",
            ),
            options: [
                Tr::new("3 × 2", "3 × 2"),
                Tr::new("2 × 2", "2 × 2"),
                Tr::new("2 × 3", "2 × 3"),
            ],
            answer: 0,
            why: Tr::new(
                "⌈40/16⌉ = 3 across and ⌈24/16⌉ = 2 down. Each direction rounds up on its own, and both have a ragged edge.",
                "⌈40/16⌉ = 3 列块，⌈24/16⌉ = 2 行块。两个方向各自向上取整，而且两边都有参差的边。",
            ),
        },
        hints: [
            Tr::new(
                "Rows and columns are separate: x comes from bx, bdx, tx and y from by, bdy, ty. tid cannot give you y, because blocks are numbered one after another.",
                "行和列是分开的：x 来自 bx、bdx、tx，y 来自 by、bdy、ty。tid 给不出 y，因为块是一个接一个编号的。",
            ),
            Tr::new(
                "The cell lives at y · W + x. Check x < W and y < H separately; one check alone lets the other edge spill into the next row or the guard.",
                "格子在 y · W + x 处。x < W 和 y < H 要分别检查；只查一个，另一条边就会溢到下一行或守卫格里。",
            ),
            Tr::new(
                "LD W and H, compute x and y, two SETP.LT, then IF p0 / IF p1 around: index = y·W + x, LD rom[index + 2], add x·y, AND 255, ST.",
                "LD 出 W 和 H，算 x 和 y，两个 SETP.LT，用 IF p0 / IF p1 包住：下标 = y·W + x，LD rom[下标 + 2]，加 x·y，AND 255，ST。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Two-dimensional grids", "二维网格"),
            line: Tr::new(
                "Blocks and threads both have an x and a y, and each direction has its own ragged edge to guard.",
                "块和线程都有 x 和 y 两个方向，每个方向都有自己参差的边要守。",
            ),
            reality: Tr::new(
                "Image kernels use dim3 grids and blocks, typically 16 × 16 or 32 × 8, and check both x < width and y < height. A missing x check does not run off the array: it quietly writes into the next row.",
                "图像内核用 dim3 的网格和块，常见 16 × 16 或 32 × 8，并同时检查 x < 宽 与 y < 高。漏掉 x 检查不会越出数组：它会悄悄写进下一行。",
            ),
            myth: Tr::new(
                "Myth: a one-dimensional thread id is enough for pictures. The id counts whole blocks, so dividing it by W does not give the row.",
                "误解：画面用一维线程号就够了。线程号是一整块一整块数的，除以 W 得不到行号。",
            ),
            simplified: Tr::new(
                "There is no division here, so rows come straight from by, bdy and ty. Real hardware has the same idea, with a cheaper index.",
                "这里没有除法，所以行号直接由 by、bdy、ty 得到。真实硬件是同样的思路。",
            ),
        },
        ..base("grid-2d", 2, Launch::grid2(2, 2, 16, 16), 2304, ATLAS_OPS)
    }
}

// ---------------------------------------------------------------- 10-3

fn bucket(seed: u64, n: usize) -> Case {
    tablet(seed, n)
}

fn grid_stride() -> Task {
    Task {
        title: Tr::new("The Grid-Stride Loop", "网格步长循环"),
        brief: Tr::new(
            "Yinpo's tablet has grown: up to 2048 readings, n in rom[0] and the readings from rom[1].\nThe machine now launches a fixed grid of 4 blocks of 64 threads, 256 threads in all, whatever n is.\nDouble each reading and add 5: out[i] = in[i]·2 + 5. Every element must be handled exactly once, and the guard strip behind the answers must stay as it is.",
            "银婆的石板变长了：最多 2048 个读数，n 在 rom[0]，读数从 rom[1] 起。\n机器现在固定发射 4 块、每块 64 个线程，一共 256 个线程，不管 n 是多少。\n把每个读数乘 2 再加 5：out[i] = in[i]·2 + 5。每个元素都要恰好处理一次，答案后面的守卫格必须保持原样。",
        ),
        cases: || vec![bucket(1021, 256), bucket(1022, 1000), bucket(1023, 2000)],
        hidden: || vec![bucket(1024, 1), bucket(1025, 257), bucket(1026, 2047)],
        init: |_| guard_init(2112),
        expect: |c| {
            let n = c.rom[0] as usize;
            guarded(n, 2112, |i| c.rom[1 + i] * 2 + 5)
        },
        goals: [Goal::new(Metric::Cycles, 4200), Goal::new(Metric::Size, 15)],
        starter: Source::Code(
            "; the grid is fixed: 4 blocks of 64 threads | 网格是固定的：4 块、每块 64 个线程\n; start at i = bx * bdx + tx, then jump by gdx * bdx | 从 i = bx * bdx + tx 起，每次跳 gdx * bdx\n; LOOP counts must be fixed numbers; guard i < n each time | LOOP 的次数必须是固定数；每圈都要检查 i < n\n",
        ),
        reference: Source::Code(
            "LD r1, [0]\nMUL r2, bx, bdx\nADD r2, r2, tx\nMUL r3, gdx, bdx\nLOOP 8\nSETP.LT p0, r2, r1\nIF p0\nADD r4, r2, 1\nLD r5, [r4]\nSHL r5, r5, 1\nADD r5, r5, 5\nST [r2], r5\nENDIF\nADD r2, r2, r3\nENDLOOP\n",
        ),
        predict: Predict {
            question: Tr::new(
                "256 threads walk over 1000 elements, each jumping by the grid size. What is the most elements any single thread handles?",
                "256 个线程各自按网格大小跳着走遍 1000 个元素。单个线程最多处理几个元素？",
            ),
            options: [Tr::new("3", "3"), Tr::new("4", "4"), Tr::new("8", "8")],
            answer: 1,
            why: Tr::new(
                "⌈1000/256⌉ = 4. Thread 0 takes elements 0, 256, 512 and 768; thread 232 takes only three, because 1000 + 232 would be past the end.",
                "⌈1000/256⌉ = 4。线程 0 处理 0、256、512、768 四个元素；线程 232 只处理三个，因为下一个会越过末尾。",
            ),
        },
        hints: [
            Tr::new(
                "Each thread starts at its own global index and steps forward by the total number of threads, gdx · bdx, until it runs past n.",
                "每个线程从自己的全局下标出发，每次前进线程总数 gdx · bdx，直到越过 n。",
            ),
            Tr::new(
                "2048 elements over 256 threads is at most 8 rounds, so LOOP 8 is enough. Inside, test i < n each round and add the stride at the end.",
                "2048 个元素分给 256 个线程，最多 8 圈，所以 LOOP 8 就够。圈内每次先测 i < n，圈末加上步长。",
            ),
            Tr::new(
                "LD n; i = bx·bdx + tx; stride = gdx·bdx; LOOP 8 { SETP.LT p0, i, n; IF p0 { LD rom[i+1], double, add 5, ST [i] } i += stride }.",
                "LD n；i = bx·bdx + tx；步长 = gdx·bdx；LOOP 8 { SETP.LT p0, i, n；IF p0 { LD rom[i+1]，乘 2，加 5，ST [i] } i += 步长 }。",
            ),
        ],
        insight: Insight {
            title: Tr::new("The grid-stride loop", "网格步长循环"),
            line: Tr::new(
                "When the grid is smaller than the data, each thread walks the array in jumps of the whole grid, so neighbouring threads always touch neighbouring elements.",
                "网格比数据小时，每个线程以整个网格为步长走过数组，这样相邻线程总是碰相邻的元素。",
            ),
            reality: Tr::new(
                "The grid-stride loop is the standard CUDA idiom for any n: `for (i = tid; i < n; i += gridDim.x * blockDim.x)`. One launch shape then fits every input size.",
                "网格步长循环是 CUDA 处理任意 n 的标准写法：`for (i = tid; i < n; i += gridDim.x * blockDim.x)`。同一种发射形状适用于所有输入大小。",
            ),
            myth: Tr::new(
                "Myth: give every element its own thread. The hardware only has so many threads at once, and extra blocks just wait their turn.",
                "误解：每个元素配一个线程。硬件同时能容纳的线程有限，多出来的块只能排队。",
            ),
            simplified: Tr::new(
                "LOOP counts are fixed numbers here, so we guess an upper bound and let the check drop the rounds that are not needed.",
                "这里 LOOP 的次数是固定数，所以先猜一个上界，再靠检查丢掉用不着的圈。",
            ),
        },
        ..base("grid-stride", 3, Launch::grid(4, 64), 2112, ATLAS_OPS)
    }
}

// ---------------------------------------------------------------- 10-4

/// 512 readings in rom[0..512]; `frame` is the scheduling seed.
fn smooth_case(seed: u64, frame: i32) -> Case {
    case(values(seed, 512, 1, 50), frame)
}

fn smoothed(c: &Case, i: usize) -> i32 {
    let at = |j: isize| {
        c.rom
            .get(j as usize)
            .copied()
            .filter(|_| j >= 0)
            .unwrap_or(0)
    };
    let i = i as isize;
    at(i - 1) + 2 * at(i) + at(i + 1)
}

fn the_collide() -> Task {
    Task {
        title: Tr::new("The Collision", "撞车"),
        brief: Tr::new(
            "The wind tower smooths its 512 readings: out[i] = in[i-1] + 2·in[i] + in[i+1], where anything past either end counts as 0.\nEight blocks of 64 threads share the work, and each block owns 64 consecutive cells. This sketch is nearly done, but the answers change from run to run when the blocks start in a different order. Find out why.",
            "风塔要把 512 个读数抹平：out[i] = in[i-1] + 2·in[i] + in[i+1]，两端之外的按 0 算。\n八个块、每块 64 个线程分担这件事，每个块负责连续的 64 个格子。这份草稿几乎写完了，可块的启动顺序一变，答案就跟着变。查查为什么。",
        ),
        cases: || vec![smooth_case(1031, 1), smooth_case(1032, 2)],
        hidden: || {
            vec![
                smooth_case(1033, 3),
                smooth_case(1034, 4),
                smooth_case(1035, 5),
            ]
        },
        expect: |c| (0..512).map(|i| Some(smoothed(c, i))).collect(),
        goals: [Goal::new(Metric::Cycles, 600), Goal::new(Metric::Size, 11)],
        starter: Source::Code(
            "MUL r0, bx, bdx\nADD r0, r0, tx\nSUB r1, r0, 1\nADD r2, r0, 1\nLD r3, [r1]\nLD r4, [r0]\nLD r5, [r2]\nSHL r6, r4, 1\nADD r6, r6, r3\nADD r6, r6, r5\nST [r0], r6\nSETP.EQ p0, tx, 0\nIF p0\nSHL r7, r3, 1\nADD r7, r7, r4\nST [r1], r7\nENDIF\nSETP.EQ p1, tx, 63\nIF p1\nSHL r7, r5, 1\nADD r7, r7, r4\nST [r2], r7\nENDIF\n",
        ),
        reference: Source::Code(
            "MUL r0, bx, bdx\nADD r0, r0, tx\nSUB r1, r0, 1\nADD r2, r0, 1\nLD r3, [r1]\nLD r4, [r0]\nLD r5, [r2]\nSHL r4, r4, 1\nADD r4, r4, r3\nADD r4, r4, r5\nST [r0], r4\n",
        ),
        clue: Some(Clue {
            what: Tr::new(
                "the first write to a cell that a different block had already written",
                "第一次写进另一个块已经写过的格子",
            ),
            find: crate::grade::first_collide,
        }),
        predict: Predict {
            question: Tr::new(
                "Block 3 and block 4 both write the cell between them. Which write lands last?",
                "块 3 和块 4 都往它们之间的那个格子写。哪一次写入落在最后？",
            ),
            options: [
                Tr::new("Block 3, it has the lower number", "块 3，它的编号小"),
                Tr::new("Block 4, it has the higher number", "块 4，它的编号大"),
                Tr::new(
                    "Neither is guaranteed; it depends on timing",
                    "都不保证，取决于时机",
                ),
            ],
            answer: 2,
            why: Tr::new(
                "Blocks are dealt to the SMs in queue order but start at different moments, and no rule orders them. Whoever happens to write last wins, and the next seed may change who that is.",
                "块按队列顺序分给各个 SM，但开始的时刻不同，没有任何规则排定它们的先后。谁碰巧最后写谁赢，换一个种子可能就换了人。",
            ),
        },
        hints: [
            Tr::new(
                "Run it with both seeds and compare. Which cells differ, and whose cells are they: the writer's own or a neighbour's?",
                "用两个种子各跑一遍再比较。哪些格子不同？它们是写入者自己的，还是邻居的？",
            ),
            Tr::new(
                "Threads 0 and 63 of each block also write a cell of the neighbouring block, with a guess that lacks the far neighbour. The owner writes the right value, and whoever lands last wins.",
                "每块的 0 号和 63 号线程还会往邻块的格子里写一个缺了远邻的猜测值。主人写的是对的，谁落在最后谁说了算。",
            ),
            Tr::new(
                "Give each cell exactly one writer: delete the halo stores. Reading a neighbour's input is safe, because input never changes; only the writes need owners.",
                "让每个格子只有一个写入者：删掉那两段光环写入。读邻块的输入是安全的，因为输入从不变；只有写入需要主人。",
            ),
        ],
        insight: Insight {
            title: Tr::new(
                "Blocks race, so partition the output",
                "块之间会抢跑，所以要划分输出",
            ),
            line: Tr::new(
                "No rule says which block runs first. Give every output cell exactly one writer, and reading across the border becomes harmless.",
                "没有任何规则规定哪个块先跑。让每个输出格子恰好一个写入者，跨边界读取就没有害处了。",
            ),
            reality: Tr::new(
                "Blocks of a grid may run in any order and in parallel, and CUDA gives no ordering between them. Two blocks writing the same address is a data race: the result can change from run to run.",
                "网格里的块可以任意顺序、并行运行，CUDA 不保证它们之间的先后。两个块写同一个地址就是数据竞争：结果每次运行都可能不同。",
            ),
            myth: Tr::new(
                "Myth: blocks run in order, so a later block overwrites an earlier one. Nothing promises that, and the Atlas shuffles start times on purpose.",
                "误解：块是按顺序跑的，后面的块会盖掉前面的。没有任何保证，Atlas 还故意打乱了启动时间。",
            ),
            simplified: Tr::new(
                "The scheduling seed shows two orders; the real hardware has far more, and the one you tested is rarely the one that breaks.",
                "调度种子只展示了两种顺序；真实硬件有多得多的顺序，而你测过的那个往往不是出错的那个。",
            ),
        },
        ..base("the-collide", 4, Launch::grid(8, 64), 512, ATLAS_OPS)
    }
}

// ---------------------------------------------------------------- 10-5

/// rom[0] = W, rom[1] = H, rom[2..] = the field, row-major. A few hot cells
/// stand out so a read from the wrong row shows.
fn field(seed: u64, w: usize, h: usize) -> Case {
    let mut rom = vec![w as i32, h as i32];
    let mut f = values(seed, w * h, 0, 200);
    for k in 0..(w * h / 40).max(1) {
        let at = (k * 37 + 11) % (w * h);
        f[at] = 900 + (k as i32 % 7) * 13;
    }
    rom.extend(f);
    case(rom, 0)
}

const FIELD_OUT: usize = 4224;

/// One relaxation step: the border ring is copied, inside it
/// (4c + n + s + e + w) >> 3.
fn relaxed(c: &Case) -> Vec<Option<i32>> {
    let (w, h) = (c.rom[0] as usize, c.rom[1] as usize);
    guarded(w * h, FIELD_OUT, |i| {
        let (x, y) = (i % w, i / w);
        let at = |j: usize| c.rom[2 + j];
        if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
            at(i)
        } else {
            (4 * at(i) + at(i - 1) + at(i + 1) + at(i - w) + at(i + w)) >> 3
        }
    })
}

/// The plain version: five global reads per interior cell.
#[cfg(test)]
const STENCIL_PLAIN: &str = "LD r0, [0]\n\
    LD r1, [1]\n\
    SHL r2, bx, 4\n\
    ADD r2, r2, tx\n\
    SHL r3, by, 4\n\
    ADD r3, r3, ty\n\
    SETP.LT p0, r2, r0\n\
    SETP.LT p1, r3, r1\n\
    IF p0\n\
    IF p1\n\
    MUL r4, r3, r0\n\
    ADD r4, r4, r2\n\
    ADD r5, r4, 2\n\
    LD r6, [r5]\n\
    SUB r7, r0, 1\n\
    SUB r7, r7, r2\n\
    MIN r7, r7, r2\n\
    SUB r8, r1, 1\n\
    SUB r8, r8, r3\n\
    MIN r8, r8, r3\n\
    MIN r7, r7, r8\n\
    SETP.GE p2, r7, 1\n\
    IF p2\n\
    SUB r8, r5, 1\n\
    LD r8, [r8]\n\
    ADD r9, r5, 1\n\
    LD r9, [r9]\n\
    SUB r10, r5, r0\n\
    LD r10, [r10]\n\
    ADD r11, r5, r0\n\
    LD r11, [r11]\n\
    SHL r12, r6, 2\n\
    ADD r12, r12, r8\n\
    ADD r12, r12, r9\n\
    ADD r12, r12, r10\n\
    ADD r12, r12, r11\n\
    SHR r6, r12, 3\n\
    ENDIF\n\
    ST [r4], r6\n\
    ENDIF\n\
    ENDIF\n";

/// The tiled version: each block stages its 16 x 16 tile and a one-cell halo
/// (18 x 18) in shared memory, then every cell reads its neighbours from there.
const STENCIL_TILED: &str = "LD r0, [0]\n\
    LD r1, [1]\n\
    SHL r2, bx, 4\n\
    ADD r2, r2, tx\n\
    SHL r3, by, 4\n\
    ADD r3, r3, ty\n\
    MUL r4, r3, r0\n\
    ADD r4, r4, r2\n\
    ADD r5, r4, 2\n\
    MUL r9, ty, 18\n\
    ADD r9, r9, tx\n\
    ADD r9, r9, 19\n\
    SETP.EQ p0, ty, 0\n\
    SETP.EQ p1, ty, 15\n\
    SETP.EQ p2, tx, 0\n\
    SETP.EQ p3, tx, 15\n\
    LD r6, [r5]\n\
    IF p0\n\
    SUB r7, r5, r0\n\
    LD r7, [r7]\n\
    ENDIF\n\
    IF p1\n\
    ADD r8, r5, r0\n\
    LD r8, [r8]\n\
    ENDIF\n\
    IF p2\n\
    SUB r10, r5, 1\n\
    LD r10, [r10]\n\
    ENDIF\n\
    IF p3\n\
    ADD r11, r5, 1\n\
    LD r11, [r11]\n\
    ENDIF\n\
    STS [r9], r6\n\
    IF p0\n\
    ADD r12, tx, 1\n\
    STS [r12], r7\n\
    ENDIF\n\
    IF p1\n\
    ADD r12, tx, 307\n\
    STS [r12], r8\n\
    ENDIF\n\
    IF p2\n\
    SUB r12, r9, 1\n\
    STS [r12], r10\n\
    ENDIF\n\
    IF p3\n\
    ADD r12, r9, 1\n\
    STS [r12], r11\n\
    ENDIF\n\
    BAR\n\
    SUB r7, r0, 1\n\
    SUB r7, r7, r2\n\
    MIN r7, r7, r2\n\
    SUB r8, r1, 1\n\
    SUB r8, r8, r3\n\
    MIN r8, r8, r3\n\
    MIN r7, r7, r8\n\
    SETP.GE p2, r7, 1\n\
    IF p2\n\
    SUB r8, r9, 1\n\
    LDS r10, [r8]\n\
    ADD r8, r9, 1\n\
    LDS r11, [r8]\n\
    SUB r8, r9, 18\n\
    LDS r12, [r8]\n\
    ADD r8, r9, 18\n\
    LDS r13, [r8]\n\
    SHL r6, r6, 2\n\
    ADD r6, r6, r10\n\
    ADD r6, r6, r11\n\
    ADD r6, r6, r12\n\
    ADD r6, r6, r13\n\
    SHR r6, r6, 3\n\
    ENDIF\n\
    SETP.LT p0, r2, r0\n\
    SETP.LT p1, r3, r1\n\
    IF p0\n\
    IF p1\n\
    ST [r4], r6\n\
    ENDIF\n\
    ENDIF\n";

fn the_stencil() -> Task {
    Task {
        title: Tr::new("The Stencil", "模板"),
        brief: Tr::new(
            "Yinpo's well water spreads sideways. The field is W × H cells: rom[0] = W, rom[1] = H, the cells from rom[2], row by row.\nOne step of seeping gives each inner cell out = (4·c + north + south + east + west) >> 3, with c the cell's own value. The ring of border cells is copied unchanged.\nBlocks are 16 × 16. Every inner cell needs its four neighbours, and neighbours at a block's edge belong to the next block. Fewer global reads earn the last star.",
            "银婆的井水会向四周渗开。场是 W × H 个格子：rom[0] = W，rom[1] = H，格子从 rom[2] 起按行排列。\n渗一步，每个内部格得到 out = (4·c + 北 + 南 + 东 + 西) >> 3，c 是它自己的值。最外一圈边界格原样复制。\n块是 16 × 16。每个内部格都要读四个邻居，而块边缘的邻居属于相邻的块。全局读取越少，越能拿到最后一颗星。",
        ),
        cases: || {
            vec![
                field(1041, 32, 32),
                field(1042, 48, 32),
                field(1043, 64, 16),
            ]
        },
        hidden: || {
            vec![
                field(1044, 30, 20),
                field(1045, 17, 17),
                field(1046, 64, 64),
            ]
        },
        init: |_| guard_init(FIELD_OUT),
        expect: relaxed,
        grid: Some(|c| {
            let (w, h) = (c.rom[0] as usize, c.rom[1] as usize);
            Launch::grid2(w.div_ceil(16), h.div_ceil(16), 16, 16)
        }),
        goals: [
            Goal::new(Metric::Cycles, 4000),
            Goal::new(Metric::Loads, 15000),
        ],
        starter: Source::Code(
            "; x = bx * 16 + tx, y = by * 16 + ty, cell at rom[2 + y * W + x] | 格子在 rom[2 + y * W + x]\n; border cells (x or y on the edge) just copy themselves | 边界格（x 或 y 在边上）原样复制\n; inner cells: (4c + n + s + e + w) >> 3 | 内部格：(4c + 北 + 南 + 东 + 西) >> 3\n",
        ),
        reference: Source::Code(STENCIL_TILED),
        predict: Predict {
            question: Tr::new(
                "Without shared memory, how many global values does one inner cell read to compute its new value?",
                "不用共享内存时，一个内部格算出新值要读几个全局数值？",
            ),
            options: [Tr::new("4", "4"), Tr::new("5", "5"), Tr::new("9", "9")],
            answer: 1,
            why: Tr::new(
                "The four neighbours and the cell itself: five. Not nine, because this stencil ignores the diagonals. Neighbouring threads read most of the same values again.",
                "四个邻居加上它自己：五个。不是九个，因为这个模板不看对角。相邻线程会把大部分相同的值再读一遍。",
            ),
        },
        hints: [
            Tr::new(
                "The ring of border cells has no full set of neighbours. Decide per cell whether it is inner; border cells copy themselves.",
                "边界一圈没有完整的邻居。逐格判断它是不是内部格；边界格复制自己。",
            ),
            Tr::new(
                "Inner means x and y both stay at least one cell away from every edge: min(x, W-1-x, y, H-1-y) ≥ 1. The neighbours sit at index ∓1 and ∓W.",
                "内部格指 x 和 y 都离每条边至少一格：min(x, W-1-x, y, H-1-y) ≥ 1。邻居在下标 ∓1 和 ∓W 处。",
            ),
            Tr::new(
                "For the last star, stage the block's 16 × 16 tile plus a one-cell halo (18 × 18) in shared memory, BAR, and read the neighbours with LDS. Each value is then fetched from global memory about once.",
                "想拿最后一颗星：把块的 16 × 16 格子连同一圈光环（18 × 18）放进共享内存，BAR，再用 LDS 读邻居。每个值大约只从全局内存取一次。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Stencils and halos", "模板与光环"),
            line: Tr::new(
                "A stencil reads the same cells many times over. Loading a tile and its halo once into shared memory turns five global reads per cell into about one.",
                "模板会把同样的格子读很多遍。把一块格子连同光环一次装进共享内存，每格五次全局读取就降到约一次。",
            ),
            reality: Tr::new(
                "Tiling with a halo is the classic optimisation for stencils, convolutions and PDE solvers. The edge of the tile does extra work because it needs cells it does not own.",
                "带光环的分块是模板、卷积和偏微分方程求解器的经典优化。分块的边缘要多做一点工作，因为它需要不属于自己的格子。",
            ),
            myth: Tr::new(
                "Myth: border cells also read four neighbours. Past the edge there is nothing, or worse, the next row.",
                "误解：边界格也读四个邻居。越过边就什么都没有，更糟的是读到了下一行。",
            ),
            simplified: Tr::new(
                "Here the border ring is simply copied. Real solvers choose a boundary rule, and that choice is part of the physics.",
                "这里边界一圈直接复制。真实的求解器要选一条边界规则，而这个选择本身就是物理的一部分。",
            ),
        },
        ..base(
            "the-stencil",
            5,
            Launch::grid2(2, 2, 16, 16),
            FIELD_OUT,
            ATLAS_OPS,
        )
    }
}

// ---------------------------------------------------------------- 10-6

/// Where the gauge readings start in the rom (a whole cache line in).
const GAUGE: usize = 32;

/// rom[0] = n, rom[1] = log2 of the stride the old kernel walks with,
/// rom[32..32 + n] = the readings.
fn gauge(seed: u64, n: usize, k: i32) -> Case {
    let mut rom = vec![0; GAUGE];
    rom[0] = n as i32;
    rom[1] = k;
    rom.extend(values(seed, n, 0, 256));
    case(rom, 0)
}

fn the_analyzer() -> Task {
    Task {
        title: Tr::new("The Analyzer", "分析器"),
        brief: Tr::new(
            "Yinpo's gauge kernel gives the right answers, but it is slow and she cannot say why. rom[0] is the count n (a multiple of 1024) and the readings start at rom[32]. For every reading, out[i] = (in[i]·5 + 3) AND 255.\nThe old kernel launches one thread per reading, 256 threads per block, and every warp is busy. rom[1] only tells the old kernel how far apart it reads; your answer does not need it.\nThe cycle budget is a third of what the old kernel takes. Open the Analyzer and see where the time goes.",
            "银婆的水位内核答案是对的，但是很慢，她说不出为什么。rom[0] 是个数 n（1024 的倍数），读数从 rom[32] 开始。对每个读数：out[i] = (in[i]·5 + 3) AND 255。\n旧内核每个读数一个线程，每块 256 个线程，所有 warp 都满负荷。rom[1] 只告诉旧内核每次隔多远去读；你的答案用不着它。\n周期预算是旧内核用时的三分之一。打开分析器，看看时间花在哪里。",
        ),
        cases: || {
            vec![
                gauge(1051, 4096, 5),
                gauge(1052, 2048, 5),
                gauge(1053, 3072, 5),
            ]
        },
        hidden: || {
            vec![
                gauge(1054, 4096, 3),
                gauge(1055, 4096, 4),
                gauge(1056, 1024, 5),
            ]
        },
        expect: |c| {
            let n = c.rom[0] as usize;
            (0..4096)
                .map(|i| {
                    Some(if i < n {
                        (c.rom[GAUGE + i] * 5 + 3) & 255
                    } else {
                        0
                    })
                })
                .collect()
        },
        grid: Some(|c| Launch::grid((c.rom[0] as usize) / 256, 256)),
        limit: Some(Goal::new(Metric::Cycles, 1650)),
        goals: [Goal::new(Metric::Cycles, 700), Goal::new(Metric::Size, 8)],
        starter: Source::Code(STRIDED),
        reference: Source::Code(
            "MUL r0, bx, bdx\n\
             ADD r0, r0, tx\n\
             ADD r1, r0, 32\n\
             LD r1, [r1]\n\
             MUL r1, r1, 5\n\
             ADD r1, r1, 3\n\
             AND r1, r1, 255\n\
             ST [r0], r1\n",
        ),
        predict: Predict {
            question: Tr::new(
                "In the old kernel, about what share of the time are the SMs just waiting for memory?",
                "旧内核里，各 SM 大约有多少时间只是在等内存？",
            ),
            options: [
                Tr::new("About 10%", "约 10%"),
                Tr::new("About 50%", "约 50%"),
                Tr::new("About 90%", "约 90%"),
            ],
            answer: 2,
            why: Tr::new(
                "Each load makes the 32 lanes fetch 32 different cache lines, and every line is a 400-cycle miss queued behind the others. The arithmetic takes a few cycles; almost everything else is waiting.",
                "每次读取，32 个车道要取 32 条不同的缓存行，每条都是 400 周期的未命中，还要排在别人后面。算术只要几个周期，剩下几乎全是在等。",
            ),
        },
        hints: [
            Tr::new(
                "Deleting arithmetic changes nothing, and the warps are already full. Look at which addresses the 32 lanes of one warp read at the same instant.",
                "删算术没有用，warp 也已经是满的。看看同一个 warp 的 32 个车道在同一时刻读的是哪些地址。",
            ),
            Tr::new(
                "The old kernel makes neighbouring lanes read addresses 32 apart, so each lane needs its own cache line. Every element gets processed exactly once whichever thread takes it.",
                "旧内核让相邻车道读相隔 32 的地址，所以每个车道都要自己的一条缓存行。不管哪个线程来处理，每个元素都只处理一次。",
            ),
            Tr::new(
                "Let thread t take element t: LD rom[t + 32], multiply by 5, add 3, AND 255, ST [t]. Neighbouring lanes then share a single line.",
                "让线程 t 处理元素 t：LD rom[t + 32]，乘 5，加 3，AND 255，ST [t]。相邻车道就共用同一条缓存行。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Find the bottleneck first", "先找瓶颈"),
            line: Tr::new(
                "Time is spent where the warps wait. When they wait on memory, the cure is the access pattern, not fewer instructions.",
                "时间花在 warp 等待的地方。等内存时，药方是访问模式，而不是更少的指令。",
            ),
            reality: Tr::new(
                "Profilers such as Nsight Compute show whether a kernel is limited by memory, compute or latency. Optimising anything but the real limit gains almost nothing.",
                "Nsight Compute 之类的剖析器会告诉你内核受限于内存、计算还是延迟。优化真正瓶颈之外的东西几乎没有收益。",
            ),
            myth: Tr::new(
                "Myth: a slow kernel needs fewer instructions or more threads. Here every warp is already busy and the code is tiny; the wait is the whole cost.",
                "误解：慢内核要更少的指令或更多的线程。这里 warp 已经满了，代码也很短；等待就是全部的代价。",
            ),
            simplified: Tr::new(
                "The Analyzer reports shares of time per SM. A real profiler has hundreds of counters, but it asks the same question first.",
                "分析器按 SM 报告时间的占比。真实的剖析器有成百上千个计数器，但头一个问的也是同一个问题。",
            ),
        },
        ..base("the-analyzer", 6, Launch::grid(16, 256), 4096, ATLAS_OPS)
    }
}

/// The old kernel: element t is read from a scattered place, 2^k apart per lane.
const STRIDED: &str = "; this kernel is correct but slow; open the Analyzer | 这个内核是对的，但很慢；打开分析器\n\
    LD r0, [1]\n\
    MUL r1, bx, bdx\n\
    ADD r1, r1, tx\n\
    AND r2, r1, 1023\n\
    SHR r3, r1, 10\n\
    SHL r3, r3, 10\n\
    MOV r4, 1024\n\
    SHR r4, r4, r0\n\
    SUB r4, r4, 1\n\
    AND r5, r2, r4\n\
    SHL r5, r5, r0\n\
    MOV r6, 10\n\
    SUB r6, r6, r0\n\
    SHR r6, r2, r6\n\
    ADD r5, r5, r6\n\
    ADD r5, r5, r3\n\
    ADD r7, r5, 32\n\
    LD r7, [r7]\n\
    MUL r7, r7, 5\n\
    ADD r7, r7, 3\n\
    AND r7, r7, 255\n\
    ST [r5], r7\n";

// ---------------------------------------------------------------- 10-7

const SIDE: usize = 64;
const CELLS: usize = SIDE * SIDE;

/// rom = the 64 x 64 field, row-major; `frame` is the scheduling seed.
fn jacobi_case(seed: u64, frame: i32) -> Case {
    case(values(seed, CELLS, 0, 1000), frame)
}

/// One Jacobi step: the border ring stays, inside it the mean of the four
/// neighbours (rounded down).
fn jacobi_step(f: &[i32]) -> Vec<i32> {
    let mut g = f.to_vec();
    for y in 1..SIDE - 1 {
        for x in 1..SIDE - 1 {
            let i = y * SIDE + x;
            g[i] = (f[i - 1] + f[i + 1] + f[i - SIDE] + f[i + SIDE]) >> 2;
        }
    }
    g
}

/// Buffer A holds the field, buffer B starts as a copy of it.
fn two_buffers(c: &Case) -> Vec<i32> {
    let mut v = c.rom.clone();
    v.extend_from_slice(&c.rom);
    v
}

/// The sketch: every step updates map A in place, with a grid barrier after it.
const RACE_INPLACE: &str = "; in place on map A, then a barrier: still racy | 在图 A 上原地更新，再加栅栏：仍有竞争\n\
    MUL r0, bx, bdx\n\
    ADD r0, r0, tx\n\
    MUL r1, by, bdy\n\
    ADD r1, r1, ty\n\
    SHL r2, r1, 6\n\
    ADD r2, r2, r0\n\
    XOR r3, r0, 63\n\
    MIN r3, r3, r0\n\
    XOR r4, r1, 63\n\
    MIN r4, r4, r1\n\
    MIN r3, r3, r4\n\
    SETP.GE p0, r3, 1\n\
    LOOP 4\n\
    IF p0\n\
    SUB r3, r2, 1\n\
    LDO r4, [r3]\n\
    ADD r3, r2, 1\n\
    LDO r5, [r3]\n\
    ADD r4, r4, r5\n\
    SUB r3, r2, 64\n\
    LDO r5, [r3]\n\
    ADD r4, r4, r5\n\
    ADD r3, r2, 64\n\
    LDO r5, [r3]\n\
    ADD r4, r4, r5\n\
    SHR r4, r4, 2\n\
    ST [r2], r4\n\
    ENDIF\n\
    GSYNC\n\
    ENDLOOP\n";

/// The fix: read map A, write map B, barrier, swap the two bases.
const RACE_DOUBLE: &str = "MUL r0, bx, bdx\n\
    ADD r0, r0, tx\n\
    MUL r1, by, bdy\n\
    ADD r1, r1, ty\n\
    SHL r2, r1, 6\n\
    ADD r2, r2, r0\n\
    XOR r4, r0, 63\n\
    MIN r4, r4, r0\n\
    XOR r5, r1, 63\n\
    MIN r5, r5, r1\n\
    MIN r4, r4, r5\n\
    SETP.GE p0, r4, 1\n\
    MOV r3, 0\n\
    LOOP 4\n\
    IF p0\n\
    ADD r7, r2, r3\n\
    SUB r6, r7, 1\n\
    LDO r4, [r6]\n\
    ADD r6, r7, 1\n\
    LDO r5, [r6]\n\
    ADD r4, r4, r5\n\
    SUB r6, r7, 64\n\
    LDO r5, [r6]\n\
    ADD r4, r4, r5\n\
    ADD r6, r7, 64\n\
    LDO r5, [r6]\n\
    ADD r4, r4, r5\n\
    SHR r4, r4, 2\n\
    XOR r6, r3, 4096\n\
    ADD r6, r6, r2\n\
    ST [r6], r4\n\
    ENDIF\n\
    XOR r3, r3, 4096\n\
    GSYNC\n\
    ENDLOOP\n";

fn the_race() -> Task {
    Task {
        title: Tr::new("The Race", "抢跑"),
        brief: Tr::new(
            "The wind tower's pressure map is 64 × 64 and settles by Jacobi steps: each inner cell becomes the average of its four neighbours, (n + s + e + w) >> 2, rounded down, while the border ring stays put. Four steps are wanted. The map is rom, and it also sits in the out buffer as map A (cells 0..4095); cells 4096..8191 are an empty copy, map B. Only A is judged at the end.\nThe grid is 16 blocks of 16 × 16 threads, one cell per thread, 128 warps in all. GSYNC (a barrier for the whole grid) only works while every block is on the machine at once, and the four SMs hold 32 warps each only if your kernel uses at most 8 registers; with more, the barrier never completes.\nThis sketch updates A in place and syncs after each step, yet the answers change with the block start order. Find out why.",
            "风塔的压力图是 64 × 64，靠雅可比迭代落定：每个内部格变成四个邻居的平均，(北 + 南 + 东 + 西) >> 2，向下取整，边界一圈不动。要迭代四步。图在 rom 里，也放在 out 缓冲里作为图 A（格子 0..4095）；格子 4096..8191 是一份空白的副本，图 B。最后只判 A。\n网格是 16 个 16 × 16 线程的块，每个线程一个格子，一共 128 个 warp。GSYNC（全网格栅栏）只有在所有块同时驻留在机器上时才管用，而四个 SM 各能容纳 32 个 warp 的前提是内核最多用 8 个寄存器；寄存器更多，栅栏就永远等不齐。\n这份草稿原地更新 A，每步之后都同步，可答案还是随块的启动顺序变化。查查为什么。",
        ),
        cases: || vec![jacobi_case(1071, 1), jacobi_case(1072, 2)],
        hidden: || {
            vec![
                jacobi_case(1073, 3),
                jacobi_case(1074, 4),
                jacobi_case(1075, 5),
            ]
        },
        init: two_buffers,
        expect: |c| {
            let mut f = c.rom.clone();
            for _ in 0..4 {
                f = jacobi_step(&f);
            }
            let mut out: Vec<Option<i32>> = f.into_iter().map(Some).collect();
            out.resize(2 * CELLS, None);
            out
        },
        grid: None,
        goals: [Goal::new(Metric::Cycles, 8200), Goal::new(Metric::Size, 35)],
        starter: Source::Code(RACE_INPLACE),
        reference: Source::Code(RACE_DOUBLE),
        clue: Some(Clue {
            what: Tr::new(
                "the first read of a cell that another block had already overwritten in the same step",
                "第一次读到另一个块在同一步里已经改写过的格子",
            ),
            find: crate::grade::first_race_read,
        }),
        predict: Predict {
            question: Tr::new(
                "In the in-place sketch, a cell on a block's edge reads its east neighbour, which belongs to the next block. What does it see?",
                "在原地更新的草稿里，块边缘的格子要读它东边的邻居，那个格子属于下一个块。它读到的是什么？",
            ),
            options: [
                Tr::new("Always the old value", "总是旧值"),
                Tr::new("Always the new value", "总是新值"),
                Tr::new(
                    "Either, depending on who got there first",
                    "两种都可能，看谁先到",
                ),
            ],
            answer: 2,
            why: Tr::new(
                "The two blocks run side by side with no ordering. If the neighbour has already stored its new value the read sees that; if not, the old one. The next seed may decide differently.",
                "两个块并排运行，没有先后。邻居若已经存下新值，读到的就是新值；否则是旧值。换一个种子可能就是另一个结果。",
            ),
        },
        hints: [
            Tr::new(
                "GSYNC separates the steps, so the trouble is inside one step. Which cells does a thread read, and which does it write?",
                "GSYNC 把各步分开了，所以问题出在一步之内。一个线程读哪些格子，又写哪个格子？",
            ),
            Tr::new(
                "A cell is read by its four neighbours while it is being rewritten. Reads and writes of one step must never touch the same buffer.",
                "一个格子在被改写的同时，还要被它的四个邻居读取。同一步里的读和写不能用同一块缓冲。",
            ),
            Tr::new(
                "Read from A and write to B, GSYNC, then swap the roles. Keep one base register and flip it with XOR 4096 each step; the other buffer is base XOR 4096. Stay within 8 registers. An even number of steps ends in A.",
                "从 A 读、往 B 写，GSYNC，然后对调角色。用一个基址寄存器，每步用 XOR 4096 翻转；另一块缓冲就是基址 XOR 4096。寄存器不要超过 8 个。步数是偶数，所以最后落在 A。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Double buffering", "双缓冲"),
            line: Tr::new(
                "When every cell needs its neighbours' old values, read from one buffer and write to another, then swap. A barrier alone cannot fix an in-place update.",
                "每个格子都需要邻居的旧值时，就从一块缓冲读、往另一块写，然后对调。只靠栅栏救不了原地更新。",
            ),
            reality: Tr::new(
                "Iterative solvers keep two arrays and swap pointers each sweep. A grid-wide barrier between sweeps (or a new kernel launch) makes sure the whole previous sweep is finished first.",
                "迭代求解器保留两个数组，每一轮交换指针。两轮之间用全网格栅栏（或者再发射一次内核），确保上一轮整个结束。",
            ),
            myth: Tr::new(
                "Myth: a smaller step or a better order makes the in-place version safe. The result is still a race; only separate buffers remove it.",
                "误解：时间步更小或顺序更好，原地版本就安全了。结果仍是竞争；只有分开缓冲才能消除它。",
            ),
            simplified: Tr::new(
                "A grid-wide barrier only works while every block is resident at once. Registers decide how many warps fit, so a greedy kernel can make the barrier impossible.",
                "全网格栅栏只在所有块同时驻留时才管用。寄存器决定能装下多少 warp，所以一个贪心的内核会让栅栏无法完成。",
            ),
        },
        ..base(
            "the-race",
            7,
            Launch::grid2(4, 4, 16, 16),
            2 * CELLS,
            ATLAS_OPS,
        )
    }
}

// ---------------------------------------------------------------- 10-8

/// Steps of seepage the commission wants.
const SEEP_STEPS: usize = 40;
/// Map codes: below 1000 is open ground holding its pressure, exactly 1000
/// is a well (held), and 9999 is rock (sealed).
const WELL: i32 = 1000;
const ROCK: i32 = 9999;

/// A map of the given wells and rock cells. The edge of the map is rock, open
/// ground starts at a low, slightly uneven pressure.
fn seep_case(seed: u64, wells: &[(usize, usize)], rock: impl Fn(usize, usize) -> bool) -> Case {
    let mut r = rng(seed);
    let mut m = vec![ROCK; CELLS];
    for y in 1..SIDE - 1 {
        for x in 1..SIDE - 1 {
            m[y * SIDE + x] = if rock(x, y) { ROCK } else { below(&mut r, 40) };
        }
    }
    for &(x, y) in wells {
        m[y * SIDE + x] = WELL;
    }
    case(m, seed as i32 % 7 + 1)
}

/// One seepage step on the map.
fn seep_step(f: &[i32]) -> Vec<i32> {
    let mut g = f.to_vec();
    for i in 0..CELLS {
        let c = f[i];
        if c >= WELL {
            continue;
        }
        let read = |j: usize| if f[j] == ROCK { c } else { f[j] };
        let sum = read(i - 1) + read(i + 1) + read(i - SIDE) + read(i + SIDE);
        g[i] = c + ((sum - 4 * c) >> 3);
    }
    g
}

fn seep_expect(c: &Case) -> Vec<Option<i32>> {
    let mut f = c.rom.clone();
    for _ in 0..SEEP_STEPS {
        f = seep_step(&f);
    }
    let mut out: Vec<Option<i32>> = f.into_iter().map(Some).collect();
    out.resize(2 * CELLS, None);
    out
}

fn seep_single() -> Case {
    seep_case(1081, &[(32, 32)], |_, _| false)
}

fn seep_wall() -> Case {
    seep_case(1082, &[(16, 20), (48, 44)], |x, y| {
        x == 32 && (6..58).contains(&y)
    })
}

fn seep_ring() -> Case {
    seep_case(1083, &[(32, 14), (16, 44), (48, 44)], |_, _| false)
}

fn seep_corner() -> Case {
    seep_case(1084, &[(1, 1)], |_, _| false)
}

fn seep_gap() -> Case {
    seep_case(1085, &[(12, 30), (52, 36)], |x, y| {
        x == 30 && !(28..=33).contains(&y)
    })
}

/// The showcase: the player moves a well; a second well and a rock wall stay.
fn seep_live(_frame: i32, at: [i32; 2]) -> Case {
    let wx = (30 + at[0] * 3).clamp(2, 61) as usize;
    let wy = (30 + at[1] * 3).clamp(2, 61) as usize;
    seep_case(1086, &[(wx, wy), (50, 12)], |x, y| {
        x == 36 && (20..50).contains(&y)
    })
}

/// Reference A: an 18 x 18 tile with a halo in shared memory per block, two
/// passes over eight blocks (grid-stride), two maps swapped each step.
const SEEP_TILED: &str = "MUL r0, bx, bdx\n\
    ADD r0, r0, tx\n\
    MUL r1, by, bdy\n\
    ADD r1, r1, ty\n\
    SHL r1, r1, 6\n\
    ADD r1, r1, r0\n\
    MUL r2, ty, 18\n\
    ADD r2, r2, tx\n\
    ADD r2, r2, 19\n\
    MOV r3, 0\n\
    LOOP 40\n\
    LOOP 2\n\
    ADD r4, r1, r3\n\
    LDO r5, [r4]\n\
    SETP.EQ p0, ty, 0\n\
    SETP.EQ p1, ty, 15\n\
    SETP.EQ p2, tx, 0\n\
    SETP.EQ p3, tx, 15\n\
    IF p0\n\
    SUB r10, r4, 64\n\
    LDO r6, [r10]\n\
    ENDIF\n\
    IF p1\n\
    ADD r10, r4, 64\n\
    LDO r7, [r10]\n\
    ENDIF\n\
    IF p2\n\
    SUB r10, r4, 1\n\
    LDO r8, [r10]\n\
    ENDIF\n\
    IF p3\n\
    ADD r10, r4, 1\n\
    LDO r9, [r10]\n\
    ENDIF\n\
    STS [r2], r5\n\
    IF p0\n\
    SUB r10, r2, 18\n\
    STS [r10], r6\n\
    ENDIF\n\
    IF p1\n\
    ADD r10, r2, 18\n\
    STS [r10], r7\n\
    ENDIF\n\
    IF p2\n\
    SUB r10, r2, 1\n\
    STS [r10], r8\n\
    ENDIF\n\
    IF p3\n\
    ADD r10, r2, 1\n\
    STS [r10], r9\n\
    ENDIF\n\
    BAR\n\
    SETP.LT p0, r5, 1000\n\
    IF p0\n\
    SUB r6, r2, 1\n\
    LDS r7, [r6]\n\
    SETP.GT p1, r7, 1000\n\
    SEL r8, p1, r5, r7\n\
    ADD r6, r2, 1\n\
    LDS r7, [r6]\n\
    SETP.GT p1, r7, 1000\n\
    SEL r7, p1, r5, r7\n\
    ADD r8, r8, r7\n\
    SUB r6, r2, 18\n\
    LDS r7, [r6]\n\
    SETP.GT p1, r7, 1000\n\
    SEL r7, p1, r5, r7\n\
    ADD r8, r8, r7\n\
    ADD r6, r2, 18\n\
    LDS r7, [r6]\n\
    SETP.GT p1, r7, 1000\n\
    SEL r7, p1, r5, r7\n\
    ADD r8, r8, r7\n\
    SHL r6, r5, 2\n\
    SUB r8, r8, r6\n\
    SHR r8, r8, 3\n\
    ADD r8, r8, r5\n\
    XOR r6, r3, 4096\n\
    ADD r6, r6, r1\n\
    ST [r6], r8\n\
    ENDIF\n\
    BAR\n\
    XOR r1, r1, 2048\n\
    ENDLOOP\n\
    XOR r3, r3, 4096\n\
    GSYNC\n\
    ENDLOOP\n";

/// Reference B: no tile, every cell reads its neighbours straight from the
/// map; the same grid-stride, double buffer and GSYNC.
#[cfg(test)]
const SEEP_DIRECT: &str = "MUL r0, bx, bdx\n\
    ADD r0, r0, tx\n\
    MUL r1, by, bdy\n\
    ADD r1, r1, ty\n\
    SHL r1, r1, 6\n\
    ADD r1, r1, r0\n\
    MOV r2, 0\n\
    LOOP 40\n\
    LOOP 2\n\
    ADD r4, r1, r2\n\
    LDO r3, [r4]\n\
    SETP.LT p0, r3, 1000\n\
    IF p0\n\
    SUB r5, r4, 1\n\
    LDO r6, [r5]\n\
    SETP.GT p1, r6, 1000\n\
    SEL r7, p1, r3, r6\n\
    ADD r5, r4, 1\n\
    LDO r6, [r5]\n\
    SETP.GT p1, r6, 1000\n\
    SEL r6, p1, r3, r6\n\
    ADD r7, r7, r6\n\
    SUB r5, r4, 64\n\
    LDO r6, [r5]\n\
    SETP.GT p1, r6, 1000\n\
    SEL r6, p1, r3, r6\n\
    ADD r7, r7, r6\n\
    ADD r5, r4, 64\n\
    LDO r6, [r5]\n\
    SETP.GT p1, r6, 1000\n\
    SEL r6, p1, r3, r6\n\
    ADD r7, r7, r6\n\
    SHL r6, r3, 2\n\
    SUB r7, r7, r6\n\
    SHR r7, r7, 3\n\
    ADD r7, r7, r3\n\
    XOR r5, r2, 4096\n\
    ADD r5, r5, r1\n\
    ST [r5], r7\n\
    ENDIF\n\
    XOR r1, r1, 2048\n\
    ENDLOOP\n\
    XOR r2, r2, 4096\n\
    GSYNC\n\
    ENDLOOP\n";

fn well_seep() -> Task {
    Task {
        title: Tr::new("Seeping Water", "井水渗流"),
        brief: Tr::new(
            "Yinpo is sinking a well and laying down rock, and wants the water pressure map after 40 steps. The map is 64 × 64 and sits in rom; it is also in the out buffer as map A (cells 0..4095), with an identical copy as map B (4096..8191). Only A is judged at the end.\nCodes: a value below 1000 is open ground holding that pressure; exactly 1000 is a well, held at 1000 for ever; 9999 is rock, which never changes. The outer edge of the map is rock.\nEach step, every open cell becomes c + ((n + s + e + w - 4c) >> 3), where a neighbour that is rock counts as c itself (nothing seeps in). Wells and rock keep their value.\nThe grid is 8 blocks of 16 × 16 threads, 64 warps, all resident, so GSYNC is safe. It covers half the map at a time. No starter: you choose the layout. Budget: 300000 cycles. Star 2 asks for 190000 cycles, star 3 for at most 400000 global loads.",
            "银婆在井边钻井、放岩层，想看 40 步之后的水压图。图是 64 × 64，放在 rom 里；out 缓冲里也有一份，作为图 A（格子 0..4095），后面是一模一样的副本，图 B（4096..8191）。最后只判 A。\n编码：小于 1000 的值是可渗透的土地，数值就是水压；恰好 1000 是井口，永远保持 1000；9999 是岩层，永不改变。地图最外一圈是岩层。\n每一步，每个可渗透格变为 c + ((北 + 南 + 东 + 西 - 4c) >> 3)，其中读到岩层的邻居按 c 自己计（水渗不进去）。井口和岩层保持原值。\n网格是 8 个 16 × 16 的线程块，共 64 个 warp，全部驻留，所以 GSYNC 是安全的。一次只能盖住半张图。没有初始方案：布局由你定。预算：300000 周期。★2 要求 190000 周期以内，★3 要求全局读取不超过 400000 次。",
        ),
        init: two_buffers,
        expect: seep_expect,
        cases: || vec![seep_single(), seep_wall(), seep_ring()],
        hidden: || vec![seep_corner(), seep_gap()],
        limit: Some(Goal::new(Metric::Cycles, 300_000)),
        goals: [
            Goal::new(Metric::Cycles, 190_000),
            Goal::new(Metric::Loads, 400_000),
        ],
        starter: Source::Code(
            "; a free machine: no starter | 自由机台：没有初始方案\n\
             ; map A at 0, map B at 4096; read one, write the other, GSYNC, swap | 图 A 在 0，图 B 在 4096；读一块、写另一块，GSYNC，对调\n",
        ),
        live: Some(seep_live),
        steer: Some(Tr::new("MOVE THE WELL", "移动水井")),
        predict: Predict {
            question: Tr::new(
                "Each step, a thread needs its own cell and its four neighbours. If every thread reads them straight from the map, that is 5 global reads per cell. With a 16 × 16 tile plus a one-cell rim in shared memory, how many global reads per cell?",
                "每一步，线程需要自己的格子和四个邻居。如果每个线程都直接从图里读，就是每格 5 次全局读取。换成共享内存里 16 × 16 的块外加一圈边，每格要读几次全局内存？",
            ),
            options: [
                Tr::new("Still 5", "仍是 5 次"),
                Tr::new(
                    "About 1.25: one each, plus a thin rim",
                    "约 1.25 次：每格一次，加上薄薄一圈边",
                ),
                Tr::new("None", "一次也不用"),
            ],
            answer: 1,
            why: Tr::new(
                "The block fetches 16 × 16 = 256 cells plus a rim of 4 × 16 = 64 cells, 320 reads for 256 cells, 1.25 each. The four neighbour reads then happen in shared memory.",
                "一个块取 16 × 16 = 256 个格子，再加一圈 4 × 16 = 64 个，256 个格子读 320 次，每格 1.25 次。四个邻居的读取在共享内存里完成。",
            ),
        },
        hints: [
            Tr::new(
                "Two copies of the map: read this step's from one, write the next into the other, GSYNC, swap the bases. Rock and wells are never written, and both copies start identical, so they stay right in both.",
                "两份地图：这一步从一份读，把下一步写进另一份，GSYNC，再对调基址。岩层和井口从不被写，两份一开始又一样，所以在两份里都是对的。",
            ),
            Tr::new(
                "The grid covers 64 × 32 cells at once. Run every step twice: the second pass is 32 rows lower, 2048 cells further on.",
                "网格一次只盖住 64 × 32 个格子。每一步做两遍：第二遍往下 32 行，也就是往后 2048 个格子。",
            ),
            Tr::new(
                "Reading all four neighbours from the map costs 5 global reads per cell. Load the 16 × 16 patch and a one-cell rim into an 18 × 18 tile in shared memory, BAR, then read the neighbours from the tile. Issue all the global loads before the first STS so their waits overlap, and check the Analyzer: how much of the time is spent waiting on memory?",
                "直接从图里读四个邻居，每格要 5 次全局读取。把 16 × 16 的块和一圈边载入共享内存的 18 × 18 小块，BAR，再从小块里读邻居。先把全局读取全部发出去再做第一个 STS，等待就能重叠；也看看分析器：多少时间花在等内存上？",
            ),
        ],
        insight: Insight {
            title: Tr::new("The whole stencil loop", "完整的模板循环"),
            line: Tr::new(
                "A grid-stride loop, two maps, a grid barrier per step and a shared tile with a rim: together they are the skeleton of every iterative grid solver.",
                "网格步长循环、两张图、每步一次全网格栅栏、带边的共享块：合在一起，就是所有迭代网格求解器的骨架。",
            ),
            reality: Tr::new(
                "Heat, weather and fluid simulations run this loop for thousands of steps. The tile with its rim cuts global traffic by about four times, and the barrier (or a fresh launch) keeps each step from seeing half of the next.",
                "热传导、天气和流体模拟把这个循环跑上成千上万步。带边的小块把全局访存减少大约四倍，栅栏（或者重新发射内核）则让每一步看不到下一步的一半。",
            ),
            myth: Tr::new(
                "Myth: the cache already does the tile's job, so shared memory is redundant. The direct version still issued four times the loads and ran far slower.",
                "误解：缓存已经替你做了小块的工作，共享内存多此一举。直接读取的版本依旧发出了四倍的读取，也慢了许多。",
            ),
            simplified: Tr::new(
                "Fetch a patch once, share it inside the block, and every cell reads its neighbours from there instead of from far away.",
                "把一块数据取来一次，在块内共享，每个格子就从近处而不是远处读邻居。",
            ),
        },
        reference: Source::Code(SEEP_TILED),
        ..base(
            "well-seep",
            8,
            Launch::grid2(4, 2, 16, 16),
            2 * CELLS,
            ATLAS_OPS,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grade::{Verdict, grade};

    /// The plain version earns exactly two stars: right and in time, but not
    /// frugal with global reads.
    fn two_stars(id: &str, code: &'static str) {
        let t = super::super::task(id).unwrap();
        let rep = grade(t, &Source::Code(code).solution()).unwrap();
        assert_eq!(rep.verdict, Verdict::Pass, "{id}: {:?}", rep.measure);
        assert_eq!(rep.stars(), 2, "{id}: {:?} vs {:?}", rep.measure, t.goals);
    }

    #[test]
    fn plain_stencil_misses_the_third_star() {
        two_stars("the-stencil", STENCIL_PLAIN);
    }

    #[test]
    fn direct_seepage_misses_the_third_star() {
        two_stars("well-seep", SEEP_DIRECT);
    }
}
