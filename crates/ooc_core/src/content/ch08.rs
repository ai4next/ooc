//! Vault chapter 8: coalescing and shared memory.

use oo_base::Tr;

use super::prism::rom_case;
use super::vault::{ALL_WARPS, CACHE_OPS, SMEM_OPS, VAULT_OPS, base};
use super::{below, rng};
use crate::task::*;
use crate::vm::Launch;

fn cells(v: impl IntoIterator<Item = i32>) -> Vec<Option<i32>> {
    v.into_iter().map(Some).collect()
}

pub fn tasks() -> Vec<Task> {
    vec![
        side_by_side(),
        the_aos(),
        the_neighbors(),
        column_walk(),
        the_banks(),
        the_histogram(),
        the_label(),
        post_tally(),
    ]
}

// ---------------------------------------------------------------- 8-1

/// The ledger: 32 rows of 64 numbers; `zero_rows` blanks some of them.
fn ledger(seed: u64, zero_rows: bool) -> Vec<i32> {
    let mut r = rng(seed);
    (0..2048)
        .map(|i| {
            let v = below(&mut r, 201) - 100;
            if zero_rows && matches!(i / 64, 0 | 13 | 31) {
                0
            } else {
                v
            }
        })
        .collect()
}

fn row_sums(c: &Case) -> Vec<Option<i32>> {
    cells((0..32).map(|r| c.rom[r * 64..r * 64 + 64].iter().sum()))
}

fn side_by_side() -> Task {
    Task {
        title: Tr::new("Side by Side", "并肩取数"),
        brief: Tr::new(
            "Douya keeps a ledger: 32 rows of 64 numbers at rom[0..2048]. Row r starts at rom[64r]. out[r] is the sum of row r.\nThe Vault launches 16 warps. Give each warp two rows. Read a row the way it is stored, neighbouring lanes on neighbouring words, then add the lanes' partial sums together with SHFL.\nA transaction is one 32-word line touched by one warp's read. Stay within 80 of them.",
            "豆芽有一本账：32 行，每行 64 个数，放在 rom[0..2048]。第 r 行从 rom[64r] 开始。out[r] 是第 r 行的和。\nVault 一次发出 16 个 warp。每个 warp 管两行。按它存放的样子去读，相邻车道取相邻的字，再用 SHFL 把各车道的部分和加起来。\n一个事务，就是一个 warp 的一次读摸到的一条 32 字的缓存行。别超过 80 个。",
        ),
        cases: || [301, 302, 303].map(|s| rom_case(ledger(s, false))).to_vec(),
        hidden: || [304, 305].map(|s| rom_case(ledger(s, true))).to_vec(),
        expect: row_sums,
        limit: Some(Goal::new(Metric::Txns, 80)),
        goals: [Goal::new(Metric::Cycles, 1200), Goal::new(Metric::Txns, 64)],
        starter: Source::Code(
            "; one thread per row: simple, and every read scatters | 一个线程一行：简单，但每次读都是散的\nSETP.LT p0, tid, 32\nIF p0\n  MUL r1, tid, 64\n  LOOP 64\n    LD r2, [r1]\n    ADD r0, r0, r2\n    ADD r1, r1, 1\n  ENDLOOP\n  ST [tid], r0\nENDIF\n",
        ),
        reference: Source::Code(
            "SHL r1, warp, 7\nADD r1, r1, lane\nLD r2, [r1]\nADD r3, r1, 32\nLD r3, [r3]\nADD r4, r1, 64\nLD r4, [r4]\nADD r5, r1, 96\nLD r5, [r5]\nADD r2, r2, r3\nADD r4, r4, r5\nMOV r6, 16\nLOOP 5\n  XOR r7, lane, r6\n  SHFL r8, r2, r7\n  ADD r2, r2, r8\n  SHFL r9, r4, r7\n  ADD r4, r4, r9\n  SHR r6, r6, 1\nENDLOOP\nSHL r1, warp, 1\nST [r1], r2\nADD r1, r1, 1\nST [r1], r4\n",
        ),
        predict: Predict {
            question: Tr::new(
                "All 32 lanes of one warp read 32 neighbouring words at the same moment. How many global memory transactions is that?",
                "同一个 warp 的 32 条车道同时读 32 个相邻的字，要几次全局内存事务？",
            ),
            options: [
                Tr::new(
                    "One (two at most, depending on alignment)",
                    "1 次（至多 2 次，视对齐）",
                ),
                Tr::new("Thirty-two", "32 次"),
                Tr::new("It depends on the numbers read", "取决于读到的数值"),
            ],
            answer: 0,
            why: Tr::new(
                "A line holds 32 words and a read costs one transaction per line it touches. 32 neighbours sit in one line, or in two if they straddle a boundary.",
                "一条缓存行装 32 个字，一次读摸到几条行就付几次事务。32 个相邻的字在同一行里，跨了边界也只是两行。",
            ),
        },
        hints: [
            Tr::new(
                "Row r starts at word 64r, a multiple of 32. Lanes 0..31 reading rom[64r + lane] share one line; rom[64r + 32 + lane] is the next line.",
                "第 r 行从第 64r 个字开始，是 32 的倍数。车道 0..31 读 rom[64r + lane] 共用一条行；rom[64r + 32 + lane] 是下一条。",
            ),
            Tr::new(
                "Per warp: four loads (two rows, two lines each), add the pairs, then fold the 32 lane sums. SHFL d, v, partner with partner = lane XOR k for k = 16, 8, 4, 2, 1 leaves the full sum in every lane.",
                "每个 warp：四次读（两行，每行两条线），成对相加，再折叠 32 个车道的和。SHFL d, v, partner，其中 partner = lane XOR k，k 取 16、8、4、2、1，做完每条车道都持有完整的和。",
            ),
            Tr::new(
                "XOR r7, lane, r6 / SHFL r8, r2, r7 / ADD r2, r2, r8, in a LOOP 5 that halves r6 each turn. Then every lane may ST the same sum to out[2 * warp].",
                "XOR r7, lane, r6 / SHFL r8, r2, r7 / ADD r2, r2, r8，放进 LOOP 5，每圈把 r6 减半。之后每条车道都可以把同一个和 ST 到 out[2 * warp]。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Coalescing", "合并访问"),
            line: Tr::new(
                "A warp's read costs one transaction per line its lanes touch, not one per lane. Neighbours on neighbouring words touch one line.",
                "一次 warp 读的代价，是它的车道摸到几条缓存行，而不是几条车道在读。相邻车道读相邻的字，只摸一条。",
            ),
            reality: Tr::new(
                "A real GPU merges the loads of a warp into 32, 64 or 128-byte memory transactions; 32 consecutive 4-byte reads fit one 128-byte segment.",
                "真实的 GPU 会把一个 warp 的读合并成 32、64 或 128 字节的内存事务；32 个连续的 4 字节读正好装进一个 128 字节的段。",
            ),
            myth: Tr::new(
                "\"One thread per row is natural, and fewer readers means fewer reads.\" The bill is per line touched: a thread that strides pays a whole line for every word.",
                "“一个线程一行最自然，读的人少，读取就少。”账是按摸到的行算的：跨着步子读的线程，每个字都要付一整条行。",
            ),
            simplified: Tr::new(
                "OC-1 has one line size, 32 words, and only counts reads. Real hardware also merges stores and splits lines into sectors.",
                "OC-1 只有一种行宽，32 个字，也只统计读。真实硬件写入也会合并，行内还分扇区。",
            ),
        },
        ..base("side-by-side", 1, ALL_WARPS, 32, CACHE_OPS)
    }
}

// ---------------------------------------------------------------- 8-2

/// 512 sparks as records (rom[0..2048]) and again field by field (rom[2048..4096]).
fn sparks(seed: u64, signed: bool) -> Vec<i32> {
    let mut r = rng(seed);
    let f: Vec<[i32; 4]> = (0..512)
        .map(|_| {
            let mut v = [0; 4];
            for x in &mut v {
                *x = below(&mut r, 1000) - if signed { 500 } else { 0 };
            }
            v
        })
        .collect();
    let mut rom: Vec<i32> = f.iter().flatten().copied().collect();
    for k in 0..4 {
        rom.extend(f.iter().map(|p| p[k]));
    }
    rom
}

fn the_aos() -> Task {
    Task {
        title: Tr::new("Array of Records", "结构数组"),
        brief: Tr::new(
            "Douya's workshop tracks 512 sparks. rom[0..2048] stores them as records of 4 words: x, y, vx, vy, spark i at rom[4i].\nrom[2048..4096] holds the same data field by field: every x, then every y, vx, vy. So x_i is at rom[2048 + i] and vx_i at rom[3072 + i].\nOne thread per spark, 16 warps: out[i] = x_i + vx_i. Which copy you read decides how many lines each read touches. Stay within 64 read transactions.",
            "豆芽的作坊里有 512 颗火星。rom[0..2048] 把它们存成结构数组，每颗 4 个字：x、y、vx、vy，第 i 颗从 rom[4i] 起。\nrom[2048..4096] 是同一份数据按字段连续存放：先是全部的 x，再是 y、vx、vy。所以 x_i 在 rom[2048 + i]，vx_i 在 rom[3072 + i]。\n每个线程一颗火星，16 个 warp：out[i] = x_i + vx_i。读哪一份，决定了每次读摸到几条行。读事务数别超过 64。",
        ),
        cases: || [311, 312, 313].map(|s| rom_case(sparks(s, false))).to_vec(),
        hidden: || [314, 315].map(|s| rom_case(sparks(s, true))).to_vec(),
        expect: |c| cells((0..512).map(|i| c.rom[4 * i] + c.rom[4 * i + 2])),
        limit: Some(Goal::new(Metric::Txns, 64)),
        goals: [Goal::new(Metric::Cycles, 560), Goal::new(Metric::Txns, 32)],
        starter: Source::Code(
            "; reads the records: every spark's x sits 4 words from the next | 读的是结构数组：相邻两颗火星的 x 相距 4 个字\nSHL r1, tid, 2\nLD r2, [r1]\nADD r1, r1, 2\nLD r3, [r1]\nADD r2, r2, r3\nST [tid], r2\n",
        ),
        reference: Source::Code(
            "ADD r1, tid, 2048\nADD r2, tid, 3072\nLD r3, [r1]\nLD r4, [r2]\nADD r3, r3, r4\nST [tid], r3\n",
        ),
        predict: Predict {
            question: Tr::new(
                "In the array of records, how far apart in memory are the x values of two neighbouring sparks?",
                "在结构数组里，相邻两颗火星的 x 在内存里相距多远？",
            ),
            options: [
                Tr::new("1 word", "1 个字"),
                Tr::new("4 words", "4 个字"),
                Tr::new("32 words", "32 个字"),
            ],
            answer: 1,
            why: Tr::new(
                "Each record is 4 words, so x repeats every 4. A warp reading x spans 128 words, which is 4 lines, and uses one word in four of each.",
                "每条记录 4 个字，x 每隔 4 个字出现一次。一个 warp 读 x 要跨 128 个字，也就是 4 条行，每条行里只用了四分之一。",
            ),
        },
        hints: [
            Tr::new(
                "Lane l of a warp reads rom[4i]: neighbours are 4 words apart, so 32 lanes span 128 words. Count the lines in the memory view.",
                "车道 l 读 rom[4i]：相邻车道相距 4 个字，32 条车道跨 128 个字。到内存视图里数一数有几条行。",
            ),
            Tr::new(
                "The second copy puts each x next to the next x. Read x and vx from there, one word per lane, neighbours on neighbours.",
                "第二份把每个 x 紧挨着下一个 x 放。从那里读 x 和 vx，每条车道一个字，相邻车道读相邻的字。",
            ),
            Tr::new(
                "ADD r1, tid, 2048 / ADD r2, tid, 3072 / LD r3, [r1] / LD r4, [r2] / ADD / ST [tid]. Issue both loads before you use either.",
                "ADD r1, tid, 2048 / ADD r2, tid, 3072 / LD r3, [r1] / LD r4, [r2] / ADD / ST [tid]。两次读都发出之后，再去用它们。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Structure of arrays", "数组结构"),
            line: Tr::new(
                "What a warp reads together should sit together. Same-kind fields stored side by side coalesce; the fields of one record, interleaved, stride.",
                "一个 warp 一起读的东西，该放在一起。同类字段挨着放，能合并；一条记录的几个字段交错放，读起来就是跨步。",
            ),
            reality: Tr::new(
                "GPU code usually keeps data as a structure of arrays for this reason. An array of structs is easier to write and 4 to 16 times harder on the memory system.",
                "GPU 代码通常把数据存成「数组结构」，就是这个原因。结构数组写起来顺手，但对内存系统要费上几倍的力气。",
            ),
            myth: Tr::new(
                "\"A record belongs together in memory.\" It does for one thread; a warp is 32 threads, and they all want the same field.",
                "“一条记录就该放在一起。”对一个线程是这样；可一个 warp 有 32 个线程，它们要的是同一个字段。",
            ),
            simplified: Tr::new(
                "OC-1 gives you both layouts in rom. A real program has to choose its layout up front or repack, which costs a pass.",
                "OC-1 把两种布局都放进了 rom。真实程序得事先选好布局，或者重排一遍，那也要花一趟。",
            ),
        },
        ..base("the-aos", 2, ALL_WARPS, 512, CACHE_OPS)
    }
}

// ---------------------------------------------------------------- 8-3

/// A 32 x 16 grey picture. Kinds: 0 noise, 1 slope with grain, 2 noise with a
/// bright spot, 3 one flat colour, 4 checkerboard of 0 and 255.
fn picture32(seed: u64, kind: u8) -> Vec<i32> {
    let mut r = rng(seed);
    let flat = below(&mut r, 256);
    (0..512)
        .map(|i| {
            let (x, y) = (i % 32, i / 32);
            match kind {
                0 => below(&mut r, 256),
                1 => (x * 6 + y * 9 + below(&mut r, 24)).min(255),
                2 if (x - 20).abs() + (y - 5).abs() < 3 => 255,
                2 => below(&mut r, 90),
                3 => flat,
                _ => 255 * ((x + y) & 1),
            }
        })
        .collect()
}

fn blur3(c: &Case) -> Vec<Option<i32>> {
    let at = |x: i32, y: i32| c.rom[(y.clamp(0, 15) * 32 + x.clamp(0, 31)) as usize];
    cells((0..512).map(|i| {
        let (x, y) = (i % 32, i / 32);
        let s: i32 = (-1..=1)
            .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
            .map(|(dx, dy)| at(x + dx, y + dy))
            .sum();
        s / 9
    }))
}

fn the_neighbors() -> Task {
    Task {
        title: Tr::new("The Neighbours", "邻居"),
        brief: Tr::new(
            "Douya's window shows a grey picture, 32 wide and 16 tall, at rom[0..512]: pixel (x, y) is rom[32y + x], 0 to 255. Make it soft. out[32y + x] is the sum of the 3 by 3 pixels around (x, y), divided by 9 and rounded down. Past an edge, use the nearest edge pixel.\nOne thread per pixel, 16 warps in one block: x = lane, y = warp. There is no divide; for these sums, s / 9 rounded down is (s * 7282) >> 16.\nRead every pixel from global memory once and share it. BAR makes each warp wait until all of them have written. Stay within 32 read transactions.",
            "豆芽的窗上有一幅灰度图，32 宽 16 高，放在 rom[0..512]：像素 (x, y) 是 rom[32y + x]，值 0 到 255。把它变柔和。out[32y + x] 是 (x, y) 周围 3×3 个像素之和除以 9，向下取整。越过图边的地方，用最近的边缘像素。\n每个线程一个像素，16 个 warp 在同一个块里：x = lane，y = warp。没有除法；对这些和来说，s ÷ 9 向下取整就是 (s * 7282) >> 16。\n每个像素只从全局内存读一次，再共享出去。BAR 让每个 warp 等到所有 warp 都写完。读事务数别超过 32。",
        ),
        cases: || {
            [(321, 0), (322, 1), (323, 2)]
                .map(|(s, k)| rom_case(picture32(s, k)))
                .to_vec()
        },
        hidden: || {
            [(324, 3), (325, 4)]
                .map(|(s, k)| rom_case(picture32(s, k)))
                .to_vec()
        },
        expect: blur3,
        limit: Some(Goal::new(Metric::Txns, 32)),
        goals: [Goal::new(Metric::Cycles, 900), Goal::new(Metric::Txns, 20)],
        starter: Source::Code(
            "; nine reads per pixel, every one from global memory | 每个像素读九次，全部来自全局内存\nSUB r1, warp, 1\nLOOP 3\n  MAX r2, r1, 0\n  MIN r2, r2, 15\n  SHL r2, r2, 5\n  SUB r3, lane, 1\n  LOOP 3\n    MAX r4, r3, 0\n    MIN r4, r4, 31\n    ADD r4, r4, r2\n    LD r5, [r4]\n    ADD r0, r0, r5\n    ADD r3, r3, 1\n  ENDLOOP\n  ADD r1, r1, 1\nENDLOOP\nMUL r0, r0, 7282\nSHR r0, r0, 16\nST [tid], r0\n",
        ),
        reference: Source::Code(
            "LD r0, [tid]\nSTS [tid], r0\nBAR\nSUB r2, warp, 1\nMAX r2, r2, 0\nSHL r2, r2, 5\nADD r2, r2, lane\nLDS r3, [r2]\nADD r5, warp, 1\nMIN r5, r5, 15\nSHL r5, r5, 5\nADD r5, r5, lane\nLDS r6, [r5]\nADD r3, r3, r0\nADD r3, r3, r6\nSUB r7, lane, 1\nMAX r7, r7, 0\nADD r8, lane, 1\nMIN r8, r8, 31\nSHFL r9, r3, r7\nSHFL r10, r3, r8\nADD r3, r3, r9\nADD r3, r3, r10\nMUL r3, r3, 7282\nSHR r3, r3, 16\nST [tid], r3\n",
        ),
        predict: Predict {
            question: Tr::new(
                "In a 3 by 3 blur, how many output pixels use one given input pixel?",
                "在 3×3 模糊里，一个输入像素会被几个输出像素用到？",
            ),
            options: [
                Tr::new("1", "1 个"),
                Tr::new("3", "3 个"),
                Tr::new("9", "9 个"),
            ],
            answer: 2,
            why: Tr::new(
                "The pixel is a neighbour of the 8 around it and of itself: 9 outputs read it. Fetching it 9 times from global memory pays for the same line 9 times.",
                "它是周围 8 个像素的邻居，也是自己的邻居：9 个输出都要读它。从全局内存取 9 次，就是同一条行付了 9 次。",
            ),
        },
        hints: [
            Tr::new(
                "Each thread loads its own pixel once, STS-es it to shared memory at the same index, and waits at BAR. After BAR the neighbours are one LDS away.",
                "每个线程只读自己的像素一次，用 STS 放进共享内存的同一个下标，然后在 BAR 处等。BAR 之后，邻居只隔一次 LDS。",
            ),
            Tr::new(
                "The rows above and below belong to other warps: that is what shared memory is for. The pixels left and right are in your own warp: SHFL reaches them. At the edges, clamp the row or lane with MAX and MIN.",
                "上下两行属于别的 warp：这正是共享内存的用处。左右的像素就在你自己的 warp 里：SHFL 够得着。到了图边，用 MAX 和 MIN 夹住行号或车道号。",
            ),
            Tr::new(
                "Column sum = LDS above + own + LDS below. Then add SHFL of lane - 1 and lane + 1 (clamped). Without BAR, a warp that started early reads rows its neighbours have not written yet.",
                "列和 = 上方 LDS + 自己 + 下方 LDS。再加上车道 lane - 1 与 lane + 1（夹住）的 SHFL。没有 BAR，先开始的 warp 会读到邻居还没写的行。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Tiles and barriers", "分块与栅栏"),
            line: Tr::new(
                "Fetch a pixel once, park it in shared memory, and let its neighbours read it there. Put a barrier between the writers and the readers.",
                "像素只取一次，存进共享内存，让邻居在那里读。写的人和读的人之间，放一道栅栏。",
            ),
            reality: Tr::new(
                "Stencil and filter kernels load a tile with a halo into shared memory, call __syncthreads(), then compute from the tile. Clamped edges make the halo a copy of the border.",
                "模板和滤波内核会把带光环的一块图装进共享内存，调用 __syncthreads()，再从块里算。边缘夹住时，光环只是边框像素的副本。",
            ),
            myth: Tr::new(
                "\"The threads run side by side, so a neighbour has surely written by now.\" Warps drift apart; only BAR makes them wait.",
                "“线程是并肩跑的，邻居肯定已经写完了。”各个 warp 会错开；只有 BAR 能让它们等。",
            ),
            simplified: Tr::new(
                "OC-1 has one block of 16 warps, so BAR always means everyone. Real kernels have many blocks that never wait for each other.",
                "OC-1 只有一个 16 个 warp 的块，所以 BAR 总是等所有人。真实的内核有许多块，块与块之间互不相等。",
            ),
        },
        ..base("the-neighbors", 3, ALL_WARPS, 512, SMEM_OPS)
    }
}

// ---------------------------------------------------------------- 8-4

fn column_walk() -> Task {
    Task {
        title: Tr::new("Down the Columns", "按列走"),
        brief: Tr::new(
            "Douya's brightening machine turns every number of a 32 by 16 picture into twice itself plus one: out[i] = 2 * in[i] + 1, the picture at rom[0..512] in row-major order. 16 warps, one thread per number.\nThe answers come out right, yet the Vault says it spends 16 times the memory traffic it should. Find the read that scatters, then fix the kernel.\nStay within 16 read transactions: one per warp.",
            "豆芽的增亮机把一幅 32×16 的图里每个数变成两倍加一：out[i] = 2 * in[i] + 1，图按行主序放在 rom[0..512]。16 个 warp，每个数一个线程。\n答案是对的，可 Vault 说它花了应有的 16 倍内存流量。找出那次散开的读，再把内核修好。\n读事务数别超过 16：每个 warp 一次。",
        ),
        cases: || [331, 332, 333].map(|s| rom_case(picture32(s, 0))).to_vec(),
        hidden: || [334, 335].map(|s| rom_case(picture32(s, 1))).to_vec(),
        expect: |c| cells(c.rom.iter().map(|v| 2 * v + 1)),
        limit: Some(Goal::new(Metric::Txns, 16)),
        goals: [Goal::new(Metric::Cycles, 480), Goal::new(Metric::Size, 5)],
        starter: Source::Code(
            "; x = tid / 16, y = tid mod 16 | 线程号映射：x = tid ÷ 16，y = tid 对 16 取余\nSHR r1, tid, 4\nAND r2, tid, 15\nSHL r2, r2, 5\nADD r3, r2, r1\nLD r4, [r3]\nSHL r4, r4, 1\nADD r4, r4, 1\nST [r3], r4\n",
        ),
        reference: Source::Code("LD r0, [tid]\nSHL r0, r0, 1\nADD r0, r0, 1\nST [tid], r0\n"),
        predict: Predict {
            question: Tr::new(
                "In the starter, neighbouring lanes read words 32 apart (they walk down a column). How many lines does one warp's read touch?",
                "起初的内核里，相邻车道读的字相距 32（沿着一列往下走）。一个 warp 的一次读摸到几条行？",
            ),
            options: [
                Tr::new("1", "1 条"),
                Tr::new("2", "2 条"),
                Tr::new("16", "16 条"),
            ],
            answer: 2,
            why: Tr::new(
                "A line is 32 words, so words 32 apart are in different lines. The 32 lanes cover 16 rows of the picture, and each row is its own line.",
                "一条行是 32 个字，相距 32 的字就落在不同的行里。32 条车道覆盖图的 16 行，每一行都是单独的一条。",
            ),
        },
        hints: [
            Tr::new(
                "Open the memory view at the first LD. Which words do lanes 0, 1 and 2 read, and which line is each one in?",
                "在第一条 LD 处打开内存视图。车道 0、1、2 各读哪个字？各在哪一条行里？",
            ),
            Tr::new(
                "Neighbouring lanes should read neighbouring words. The kernel derives x and y from tid: which of the two changes when the lane goes up by one?",
                "相邻车道应该读相邻的字。内核从 tid 算出 x 和 y：车道加一时，哪一个在变？",
            ),
            Tr::new(
                "Number the threads along a row: x = tid AND 31, y = tid >> 5. Then y * 32 + x is just tid, and the load and the store both become one line per warp.",
                "沿着一行给线程编号：x = tid AND 31，y = tid >> 5。这样 y * 32 + x 就是 tid，读和写都是每个 warp 一条行。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Who is the neighbour", "谁是邻居"),
            line: Tr::new(
                "Lanes that are neighbours in the warp must be neighbours in memory. How threads are numbered over the data decides it.",
                "在 warp 里相邻的车道，在内存里也得相邻。线程怎样编号去对应数据，决定了这一点。",
            ),
            reality: Tr::new(
                "In CUDA the first rule of memory performance is that threadIdx.x indexes the contiguous dimension. Swapping x and y is a classic 10 to 30 times slowdown.",
                "CUDA 里内存性能的第一条规则，是让 threadIdx.x 去对应连续的那一维。把 x 和 y 弄反，是经典的慢上十几二十倍。",
            ),
            myth: Tr::new(
                "\"Make the loop faster.\" The loop is not the cost. The bill is the lines touched, and a faster loop touches the same 16.",
                "“把循环写快一点。”代价不在循环里。账是按摸到的行算的，循环再快，摸到的还是那 16 条。",
            ),
            simplified: Tr::new(
                "OC-1 prices only reads and uses one line size. Real memory also scatters writes, which cost the same way.",
                "OC-1 只给读标价，行宽也只有一种。真实内存里散开的写入同样要付这笔钱。",
            ),
        },
        clue: Some(Clue {
            what: Tr::new(
                "the first read that lands in 16 different lines",
                "第一次落在 16 条不同缓存行上的读",
            ),
            find: crate::grade::first_uncoalesced,
        }),
        ..base("column-walk", 4, ALL_WARPS, 512, CACHE_OPS)
    }
}

// ---------------------------------------------------------------- 8-5

/// A 32 x 32 table. Kinds: 0 random, 1 symmetric (hides a transpose written
/// the wrong way round), 2 one value everywhere.
fn table32(seed: u64, kind: u8) -> Vec<i32> {
    let mut r = rng(seed);
    let mut t: Vec<i32> = (0..1024).map(|_| below(&mut r, 9000) + 1).collect();
    let flat = t[0];
    match kind {
        1 => {
            for i in 0..32 {
                for j in 0..i {
                    t[i * 32 + j] = t[j * 32 + i];
                }
            }
        }
        2 => t.iter_mut().for_each(|v| *v = flat),
        _ => {}
    }
    t
}

fn transpose32(c: &Case) -> Vec<Option<i32>> {
    cells((0..1024).map(|i| c.rom[(i % 32) * 32 + i / 32]))
}

fn the_banks() -> Task {
    Task {
        title: Tr::new("Thirty-Two Pigeonholes", "三十二个格口"),
        brief: Tr::new(
            "Douya's sorting room has a 32 by 32 table at rom[0..1024]. Turn it on its side: out[c * 32 + r] = in[r * 32 + c]. One warp, a loop of 32 rounds.\nRead the table by rows from global memory, neighbours on neighbours, park it in shared memory, then read it back by columns. Shared memory has 32 banks: word a lives in bank a mod 32, and lanes that want different words of one bank are served one after another.\nShared memory is big enough for 32 by 33. A load takes 400 cycles to come back, so issue several before you use any. Finish within 3800 cycles.",
            "豆芽的分拣室里有一张 32×32 的表，放在 rom[0..1024]。把它翻个身：out[c * 32 + r] = in[r * 32 + c]。一个 warp，循环 32 圈。\n从全局内存按行读表，相邻车道读相邻的字，存进共享内存，再按列读回来。共享内存有 32 个 bank：第 a 个字住在 bank a 对 32 取余，想要同一个 bank 里不同字的车道，只能一个接一个地被服务。\n共享内存放得下 32×33。一次读要 400 个周期才回来，所以先发出几次再去用。在 3800 个周期内完成。",
        ),
        cases: || {
            [(341, 0), (342, 0), (343, 0)]
                .map(|(s, k)| rom_case(table32(s, k)))
                .to_vec()
        },
        hidden: || {
            [(344, 1), (345, 2)]
                .map(|(s, k)| rom_case(table32(s, k)))
                .to_vec()
        },
        expect: transpose32,
        limit: Some(Goal::new(Metric::Cycles, 3800)),
        goals: [
            Goal::new(Metric::Cycles, 3600),
            Goal::new(Metric::Smem, 1024),
        ],
        starter: Source::Code(
            "MOV r8, lane\nMOV r9, lane\nLOOP 8\n  LD r0, [r8]\n  ADD r8, r8, 32\n  LD r1, [r8]\n  ADD r8, r8, 32\n  LD r2, [r8]\n  ADD r8, r8, 32\n  LD r3, [r8]\n  ADD r8, r8, 32\n  STS [r9], r0\n  ADD r9, r9, 32\n  STS [r9], r1\n  ADD r9, r9, 32\n  STS [r9], r2\n  ADD r9, r9, 32\n  STS [r9], r3\n  ADD r9, r9, 32\nENDLOOP\nMUL r10, lane, 32\nMOV r11, lane\nLOOP 32\n  LDS r4, [r10]\n  ADD r10, r10, 1\n  ST [r11], r4\n  ADD r11, r11, 32\nENDLOOP\n",
        ),
        reference: Source::Code(
            "SHL r12, lane, 5\nMOV r8, lane\nMOV r13, 0\nLOOP 8\n  LD r0, [r8]\n  ADD r8, r8, 32\n  LD r1, [r8]\n  ADD r8, r8, 32\n  LD r2, [r8]\n  ADD r8, r8, 32\n  LD r3, [r8]\n  ADD r8, r8, 32\n  XOR r9, r13, lane\n  ADD r9, r9, r12\n  STS [r9], r0\n  XOR r10, r9, 1\n  STS [r10], r1\n  XOR r10, r9, 2\n  STS [r10], r2\n  XOR r10, r9, 3\n  STS [r10], r3\n  ADD r13, r13, 4\nENDLOOP\nMOV r13, 0\nMOV r14, 0\nLOOP 32\n  XOR r9, lane, r13\n  ADD r9, r9, r14\n  LDS r4, [r9]\n  ADD r11, r14, lane\n  ST [r11], r4\n  ADD r13, r13, 1\n  ADD r14, r14, 32\nENDLOOP\n",
        ),
        predict: Predict {
            question: Tr::new(
                "The 32 lanes of a warp read shared-memory words 0, 32, 64, ... 992 at the same moment. How many banks do they land in?",
                "一个 warp 的 32 条车道同时读共享内存的第 0、32、64……992 个字，它们落在几个 bank？",
            ),
            options: [
                Tr::new("32 banks", "32 个"),
                Tr::new("1 bank", "1 个"),
                Tr::new("2 banks", "2 个"),
            ],
            answer: 1,
            why: Tr::new(
                "Bank = address mod 32, and every one of these addresses is a multiple of 32: all bank 0. Thirty-two different words from one bank are served one per pass, 32 passes.",
                "bank = 地址对 32 取余，这些地址全是 32 的倍数：全在 bank 0。同一个 bank 里的 32 个不同的字一次只服务一个，要 32 趟。",
            ),
        },
        hints: [
            Tr::new(
                "Watch the bank column in the memory view while the columns are read back. Which bank does each lane land in?",
                "按列读回来的时候，看内存视图的 bank 列。每条车道落在哪个 bank？",
            ),
            Tr::new(
                "With a row pitch of 32 words, a whole column of the table sits in one bank. Give each row a pitch of 33 instead: the 32 words of a column then fall into 32 different banks.",
                "行距是 32 个字时，表的一整列都在同一个 bank 里。把每行的行距改成 33：一列的 32 个字就落进 32 个不同的 bank。",
            ),
            Tr::new(
                "Rows go in at tile[r * 33 + lane]; round c reads tile[lane * 33 + c]. For the third star, spend no spare words at all: keep 32 per row and move element (r, c) to column c XOR r.",
                "行按 tile[r * 33 + lane] 存进去；第 c 圈读 tile[lane * 33 + c]。要拿第三颗星，一个多余的字都不要花：每行仍是 32 个，把元素 (r, c) 挪到第 c XOR r 列。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Banks", "bank 与排队"),
            line: Tr::new(
                "Shared memory serves one word per bank per pass. Lanes that want different words of one bank queue; a column of a 32-wide table is all one bank.",
                "共享内存每个 bank 每趟只服务一个字。想要同一个 bank 里不同字的车道要排队；32 宽的表，一整列都在同一个 bank。",
            ),
            reality: Tr::new(
                "NVIDIA shared memory has 32 banks of 4 bytes. Padding each row by one word, tile[32][33], is the textbook fix; an XOR swizzle does it for free.",
                "NVIDIA 的共享内存有 32 个 4 字节的 bank。每行补一个字，tile[32][33]，是教科书里的解法；用 XOR 打乱列号则不花任何空间。",
            ),
            myth: Tr::new(
                "\"Shared memory is always fast.\" It is fast when the lanes of a warp spread over the banks, and 32 times slower when they pile onto one.",
                "“共享内存永远很快。”车道散在各个 bank 上时它才快；全挤进同一个 bank，就慢 32 倍。",
            ),
            simplified: Tr::new(
                "OC-1 has 32 banks of one word each and one shared pipe. Real hardware has wider modes, and conflicts only count within a warp.",
                "OC-1 有 32 个每个一字宽的 bank，和一条共享通道。真实硬件还有更宽的模式，冲突只在同一个 warp 内计算。",
            ),
        },
        ..base("the-banks", 5, Launch::flat(32), 1024, SMEM_OPS)
    }
}

// ---------------------------------------------------------------- 8-6

/// 2048 pixels of 16 grey levels. Kinds: 0 random, 1 ramp, 2 two colours,
/// 3 nearly one colour (about 1 pixel in 100 differs), 4 one colour.
fn tones(seed: u64, kind: u8) -> Vec<i32> {
    let mut r = rng(seed);
    let (a, b) = (below(&mut r, 16), below(&mut r, 16));
    (0..2048)
        .map(|i| match kind {
            0 => below(&mut r, 16),
            1 => ((i % 64) / 4 + i / 64) % 16,
            2 => [a, b][below(&mut r, 2) as usize],
            3 if below(&mut r, 100) == 0 => below(&mut r, 16),
            _ => a,
        })
        .collect()
}

fn the_histogram() -> Task {
    Task {
        title: Tr::new("The Tally", "直方图"),
        brief: Tr::new(
            "Douya's film has 2048 pixels at rom[0..2048], each one of 16 grey levels, 0 to 15. Count them: out[v] is how many pixels have level v.\n16 warps, 512 threads, four pixels each: thread t takes pixels t, t + 512, t + 1024 and t + 1536.\nATOM d, [a], v adds v to the word at out[a] and hands back the old value; ATOMS does the same in shared memory. Lanes that add to the same word wait their turn, one after another. Finish within 3000 cycles.",
            "豆芽的胶片有 2048 个像素，放在 rom[0..2048]，每个是 16 级灰度之一，0 到 15。数一数：out[v] 是灰度为 v 的像素个数。\n16 个 warp，512 个线程，每个线程四个像素：线程 t 取第 t、t + 512、t + 1024、t + 1536 个。\nATOM d, [a], v 把 v 加到 out[a] 这个字上，并交回旧值；ATOMS 在共享内存里做同样的事。往同一个字上加的车道要排队，一个接一个。在 3000 个周期内完成。",
        ),
        cases: || {
            [(351, 0), (352, 1), (353, 2)]
                .map(|(s, k)| rom_case(tones(s, k)))
                .to_vec()
        },
        hidden: || {
            [(354, 3), (355, 4)]
                .map(|(s, k)| rom_case(tones(s, k)))
                .to_vec()
        },
        expect: |c| cells((0..16).map(|v| c.rom.iter().filter(|&&p| p == v).count() as i32)),
        limit: Some(Goal::new(Metric::Cycles, 3000)),
        goals: [Goal::new(Metric::Cycles, 2500), Goal::new(Metric::Size, 12)],
        starter: Source::Code(
            "; every pixel goes straight to the counters in global memory | 每个像素都直接加到全局内存里的计数器上\nMOV r1, tid\nLOOP 4\n  LD r0, [r1]\n  ATOM r2, [r0], 1\n  ADD r1, r1, 512\nENDLOOP\n",
        ),
        reference: Source::Code(
            "MOV r1, tid\nLOOP 4\n  LD r0, [r1]\n  ATOMS r2, [r0], 1\n  ADD r1, r1, 512\nENDLOOP\nBAR\nSETP.LT p0, tid, 16\nIF p0\n  LDS r12, [tid]\n  ST [tid], r12\nENDIF\n",
        ),
        predict: Predict {
            question: Tr::new(
                "Nearly every pixel has the same colour, and every thread adds to the same global counter. What happens?",
                "几乎每个像素都是同一种颜色，所有线程都往全局的同一个计数器上加。会怎样？",
            ),
            options: [
                Tr::new(
                    "They queue up; the work is nearly serial",
                    "排队，几乎变成串行",
                ),
                Tr::new(
                    "All done at once, as fast as for a random picture",
                    "同时完成，和随机图一样快",
                ),
                Tr::new("The total comes out wrong", "总数会出错"),
            ],
            answer: 0,
            why: Tr::new(
                "Atomics keep the total right by taking turns: lanes on one address are served one after another. A flat picture puts all 32 lanes of every warp on the same word.",
                "原子操作靠轮流来保证总数正确：同一个地址上的车道一个接一个地被服务。一幅单色的图，让每个 warp 的 32 条车道都落在同一个字上。",
            ),
        },
        hints: [
            Tr::new(
                "Each ATOM waits behind the others on its word. Run a one-colour picture and watch the queue in the memory view.",
                "每个 ATOM 都要排在同一个字上的别人后面。跑一幅单色图，在内存视图里看那条队。",
            ),
            Tr::new(
                "Count in a 16-word histogram in shared memory instead, where an atomic is served faster. Wait for every warp at BAR, then copy the 16 counters to out once.",
                "改在共享内存的 16 个字里数，那里的原子操作服务得更快。在 BAR 处等所有 warp，再把 16 个计数一次写到 out。",
            ),
            Tr::new(
                "ATOMS r2, [r0], 1 for each of the four pixels; BAR; then threads 0..15 do LDS r12, [tid] and ST [tid], r12.",
                "四个像素各做一次 ATOMS r2, [r0], 1；BAR；然后线程 0..15 做 LDS r12, [tid] 和 ST [tid], r12。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Contention", "争用"),
            line: Tr::new(
                "Atomics on one address are serial. Count in a fast place and merge once; the fewer lanes share a counter, the shorter the queue.",
                "同一个地址上的原子操作是串行的。在快的地方数，最后只合并一次；共用一个计数器的车道越少，队就越短。",
            ),
            reality: Tr::new(
                "GPU histograms count in shared memory per block, often per warp, and merge into global memory with one atomic per bin at the end.",
                "GPU 上的直方图在每个块的共享内存里数，常常还一个 warp 一份，最后每个桶只对全局内存做一次原子加。",
            ),
            myth: Tr::new(
                "\"The hardware does atomics, so they are free.\" They are correct for free; the waiting is paid for in cycles.",
                "“原子操作是硬件做的，所以不花钱。”正确性是白送的，排队的时间要用周期来付。",
            ),
            simplified: Tr::new(
                "OC-1 shares one 16-word histogram for the whole block. Real kernels keep a copy per warp as well, to spread the contention across warps.",
                "OC-1 让整个块共用一份 16 个字的直方图。真实的内核常常每个 warp 还有一份，把争用分摊到各个 warp 上。",
            ),
        },
        ..base("the-histogram", 6, ALL_WARPS, 16, VAULT_OPS)
    }
}

// ---------------------------------------------------------------- 8-7

fn the_label() -> Task {
    Task {
        title: Tr::new("A Wrong Label", "译注有误"),
        brief: Tr::new(
            "The same table as before: 32 by 32 at rom[0..1024], out[c * 32 + r] = in[r * 32 + c], one warp, staged through shared memory.\nThe translator's note on the machine says \"shared memory has 4 banks\", so the kernel was laid out for that, rows padded to 48 words. The answers are right, and the kernel is still slow: the column reads queue up.\nFind the first read that waits in a queue, measure the machine yourself (CLOCK before and after one column read, for a few row pitches), and fix the kernel. Finish within 3650 cycles.",
            "还是那张表：32×32，在 rom[0..1024]，out[c * 32 + r] = in[r * 32 + c]，一个 warp，经共享内存中转。\n这台机器的译注写着“共享内存有 4 个 bank”，所以内核就照这个排版，每行补到 48 个字。答案是对的，可内核还是慢：按列读时在排队。\n找出第一次要排队的读；自己去量这台机器（用 CLOCK 包住一次按列读，换几种行距各量一次）；然后把内核修好。在 3650 个周期内完成。",
        ),
        cases: || {
            [(361, 0), (362, 0), (363, 0)]
                .map(|(s, k)| rom_case(table32(s, k)))
                .to_vec()
        },
        hidden: || {
            [(364, 1), (365, 2)]
                .map(|(s, k)| rom_case(table32(s, k)))
                .to_vec()
        },
        expect: transpose32,
        limit: Some(Goal::new(Metric::Cycles, 3650)),
        goals: [
            Goal::new(Metric::Cycles, 3500),
            Goal::new(Metric::Smem, 1056),
        ],
        starter: Source::Code(
            "MOV r8, lane\nMOV r9, lane\nLOOP 8\n  LD r0, [r8]\n  ADD r8, r8, 32\n  LD r1, [r8]\n  ADD r8, r8, 32\n  LD r2, [r8]\n  ADD r8, r8, 32\n  LD r3, [r8]\n  ADD r8, r8, 32\n  STS [r9], r0\n  ADD r9, r9, 48\n  STS [r9], r1\n  ADD r9, r9, 48\n  STS [r9], r2\n  ADD r9, r9, 48\n  STS [r9], r3\n  ADD r9, r9, 48\nENDLOOP\nMUL r10, lane, 48\nMOV r11, lane\nLOOP 32\n  LDS r4, [r10]\n  ADD r10, r10, 1\n  ST [r11], r4\n  ADD r11, r11, 32\nENDLOOP\n",
        ),
        reference: Source::Code(
            "MOV r8, lane\nMOV r9, lane\nLOOP 8\n  LD r0, [r8]\n  ADD r8, r8, 32\n  LD r1, [r8]\n  ADD r8, r8, 32\n  LD r2, [r8]\n  ADD r8, r8, 32\n  LD r3, [r8]\n  ADD r8, r8, 32\n  STS [r9], r0\n  ADD r9, r9, 33\n  STS [r9], r1\n  ADD r9, r9, 33\n  STS [r9], r2\n  ADD r9, r9, 33\n  STS [r9], r3\n  ADD r9, r9, 33\nENDLOOP\nMUL r10, lane, 33\nMOV r11, lane\nLOOP 32\n  LDS r4, [r10]\n  ADD r10, r10, 1\n  ST [r11], r4\n  ADD r11, r11, 32\nENDLOOP\n",
        ),
        predict: Predict {
            question: Tr::new(
                "With rows padded to 48 words, the 32 lanes read a column: words 48 * lane + c. How many different banks do they land in?",
                "行距补到 48 个字，32 条车道按列读：第 48 * lane + c 个字。它们落在几个不同的 bank？",
            ),
            options: [
                Tr::new("32 banks, no queue", "32 个，不用排队"),
                Tr::new("2 banks, 16 lanes in each", "2 个，每个 bank 16 条车道"),
                Tr::new("1 bank, 32 lanes", "1 个，32 条车道"),
            ],
            answer: 1,
            why: Tr::new(
                "48 * lane mod 32 is 0 for even lanes and 16 for odd lanes: two banks. Each serves 16 different words one per pass, so the read takes 16 passes.",
                "48 * lane 对 32 取余：偶数车道得 0，奇数车道得 16，只有两个 bank。每个 bank 要一趟一个地服务 16 个不同的字，这次读要 16 趟。",
            ),
        },
        hints: [
            Tr::new(
                "Do not trust the note. Time one column read with CLOCK before and after it, once for each row pitch you try.",
                "别信译注。用 CLOCK 包住一次按列读，每试一种行距就量一次。",
            ),
            Tr::new(
                "The number of lanes that pile onto one bank depends on how many banks a pitch reaches: pitch p reaches 32 / gcd(p, 32) of them. Only an odd pitch reaches all 32.",
                "多少条车道挤在同一个 bank，取决于行距能碰到几个 bank：行距 p 能碰到 32 / gcd(p, 32) 个。只有奇数行距能碰到全部 32 个。",
            ),
            Tr::new(
                "Change the three constants: ADD r9, r9, 33 four times in the store loop, and MUL r10, lane, 33 before the column loop.",
                "改三处常数：存入循环里四个 ADD r9, r9, 33，以及按列循环之前的 MUL r10, lane, 33。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Measure, don't trust", "实测，别轻信"),
            line: Tr::new(
                "A document can be wrong about a machine; a clock cannot. The bank count is a fact you can measure from the timing of one strided read.",
                "文档可能把机器写错，时钟不会。bank 数是一个事实，量一次跨步读的耗时就能知道。",
            ),
            reality: Tr::new(
                "Programmers microbenchmark real GPUs for exactly this: cache sizes, bank widths and latencies are often undocumented or change between generations.",
                "程序员真的会对 GPU 做微基准测试，量的正是这些：缓存大小、bank 宽度、延迟，文档里常常没有，换一代还会变。",
            ),
            myth: Tr::new(
                "\"Any padding fixes bank conflicts.\" Only a pitch that shares no factor with the bank count does; 36 or 48 words still pile lanes up.",
                "“随便补几个字就能消除 bank 冲突。”只有与 bank 数互质的行距才行；补到 36 或 48 个字，车道照样堆在一起。",
            ),
            simplified: Tr::new(
                "OC-1 has exactly 32 banks of one word. Real parts differ between architectures, and sometimes in wider modes.",
                "OC-1 恰好是 32 个一字宽的 bank。真实硬件随架构而异，有时还有更宽的模式。",
            ),
        },
        clue: Some(Clue {
            what: Tr::new(
                "the first shared-memory read that waits in a bank queue",
                "第一次在 bank 里排队的共享内存读",
            ),
            find: crate::grade::first_bank_conflict,
        }),
        ..base("the-label", 7, Launch::flat(32), 1024, SMEM_OPS)
    }
}

// ---------------------------------------------------------------- 8-8

/// 2048 letters of two words: destination 0..31 and weight class 0..7.
/// Kinds: 0 random, 1 skewed (4 in 5 go to three destinations), 2 nearly
/// all to one destination, 3 all to one destination. In kinds 2 and 3 nine
/// letters in ten are also of one weight class.
fn letters(seed: u64, kind: u8) -> Vec<i32> {
    let mut r = rng(seed);
    let hot = [below(&mut r, 32), below(&mut r, 32), below(&mut r, 32)];
    (0..2048)
        .flat_map(|_| {
            let dest = match kind {
                0 => below(&mut r, 32),
                1 if below(&mut r, 5) < 4 => hot[below(&mut r, 3) as usize],
                1 => below(&mut r, 32),
                2 if below(&mut r, 50) != 0 => hot[0],
                2 => below(&mut r, 32),
                _ => hot[0],
            };
            let class = if kind >= 2 && below(&mut r, 10) != 0 {
                hot[1] % 8
            } else {
                below(&mut r, 8)
            };
            [dest, class]
        })
        .collect()
}

fn pigeonholes(c: &Case) -> Vec<Option<i32>> {
    let mut t = [0; 256];
    for l in c.rom.chunks(2) {
        t[(l[1] * 32 + l[0]) as usize] += 1;
    }
    cells(t)
}

fn post_tally() -> Task {
    Task {
        title: Tr::new("The Sorting Board", "信件统计板"),
        brief: Tr::new(
            "Douya wants the post office's pigeonhole table. rom[0..4096] holds 2048 letters of two words each: destination 0..31, then weight class 0..7.\nWrite out[g * 32 + d] = how many letters of weight class g go to destination d. 8 rows of 32 cells.\nYou choose how many warps to launch: write .warps N (1 to 16) as the first line. Counting in shared memory, reading the letters side by side, and the queues of the atomics are all yours to weigh.\nThe hard budget is what a plain global-atomic kernel needs. Every cell must match on every mail bag.",
            "豆芽要一张邮局的格口表。rom[0..4096] 里是 2048 封信，每封两个字：目的地 0..31，然后是重量级 0..7。\n写出 out[g * 32 + d] = 重量级为 g、寄往 d 的信有几封。8 行 × 32 格。\n启动多少个 warp 由你定：第一行写 .warps N（1 到 16）。在共享内存里数、让相邻车道读相邻的信、原子操作的排队，都由你自己权衡。\n硬预算是只用全局原子操作的朴素内核所需的周期。每一袋邮件的每一个格口都必须对。",
        ),
        cases: || {
            [(371, 0), (372, 1), (373, 2)]
                .map(|(s, k)| rom_case(letters(s, k)))
                .to_vec()
        },
        hidden: || {
            [(374, 0), (375, 1), (376, 3)]
                .map(|(s, k)| rom_case(letters(s, k)))
                .to_vec()
        },
        expect: pigeonholes,
        limit: Some(Goal::new(Metric::Cycles, 4200)),
        goals: [
            Goal::new(Metric::Cycles, 2400),
            Goal::new(Metric::Smem, 256),
        ],
        starter: Source::Code(
            ".warps 1\n; no starter: the mail is waiting | 没有初始方案：信还在等\n",
        ),
        reference: Source::Code(REF_A),
        live: Some(|frame, at| {
            let k = at[0].rem_euclid(4);
            Case {
                frame,
                ..rom_case(letters(380 + k as u64, k as u8))
            }
        }),
        steer: Some(Tr::new("PICK THE MAIL BAG", "挑一袋邮件")),
        predict: Predict {
            question: Tr::new(
                "A bag where nearly every letter goes to the same pigeonhole. The 256 counters move from global to shared memory. How long does the count take?",
                "一袋信几乎全寄往同一个格口。把 256 个计数器从全局内存搬到共享内存，数完要多久？",
            ),
            options: [
                Tr::new("About the same", "差不多"),
                Tr::new("About half as long", "大约一半"),
                Tr::new("Longer: shared memory is smaller", "更久：共享内存更小"),
            ],
            answer: 1,
            why: Tr::new(
                "The lanes still queue on one counter, but the shared queue is served about twice as fast as the global atomic unit. Counting there and writing out 256 cells once costs less than it saves.",
                "车道还是在同一个计数器上排队，但共享内存里的队比全局原子单元服务得快一倍左右。在那里数、最后只写一遍 256 格，花的比省的少。",
            ),
        },
        hints: [
            Tr::new(
                "Start with the plain kernel: one ATOM on out[key] per letter, key = class * 32 + destination. It is correct. Run the skewed bag and see where the cycles go.",
                "先写朴素的内核：每封信对 out[key] 做一次 ATOM，key = 重量级 * 32 + 目的地。它是对的。跑一遍偏斜的邮袋，看周期花在哪。",
            ),
            Tr::new(
                "Letter i is at rom[2i] and rom[2i + 1], so thread t reads letters t, t + 512, ... with 16 warps. Count in shared memory with ATOMS, then BAR.",
                "第 i 封信在 rom[2i] 和 rom[2i + 1]，所以 16 个 warp 时线程 t 读第 t、t + 512……封。用 ATOMS 在共享内存里数，然后 BAR。",
            ),
            Tr::new(
                "After BAR, threads 0..255 each do LDS r6, [tid] and ST [tid], r6. Row-major order means the table comes out already in place.",
                "BAR 之后，线程 0..255 各做一次 LDS r6, [tid] 和 ST [tid], r6。行主序的写出，格口表本身就摆好了。",
            ),
        ],
        insight: Insight {
            title: Tr::new("The Whole Hall", "整座大厅"),
            line: Tr::new(
                "Read side by side, count in a fast place, merge once. Which pieces to use depends on the data, and the data decides how long the queues are.",
                "并肩去读，在快的地方数，最后合并一次。用哪几块要看数据，队有多长也是数据说了算。",
            ),
            reality: Tr::new(
                "Real histogram and scatter kernels combine all of this: coalesced loads, per-block counters in shared memory, a final merge to global memory.",
                "真实的直方图和散射内核把这些全用上：合并的读、每个块在共享内存里的计数、最后合并到全局内存。",
            ),
            myth: Tr::new(
                "\"More warps is always faster.\" More warps hide waiting, but on one hot counter they only make the queue longer.",
                "“warp 越多越快。”多一些 warp 能盖住等待，可全挤在一个热点计数器上，只会让队更长。",
            ),
            simplified: Tr::new(
                "OC-1 has one shared counter table per block and one global atomic unit; real chips have many, and tricks like per-warp copies.",
                "OC-1 每个块只有一张共享计数表，全局只有一个原子单元；真实芯片有很多个，还有每个 warp 一份副本之类的办法。",
            ),
        },
        free_warps: true,
        ..base("post-tally", 8, ALL_WARPS, 256, VAULT_OPS)
    }
}

const REF_A: &str = ".warps 16\nSHL r1, tid, 1\nLOOP 4\n  LD r2, [r1]\n  ADD r3, r1, 1\n  LD r4, [r3]\n  SHL r4, r4, 5\n  ADD r2, r2, r4\n  ATOMS r5, [r2], 1\n  ADD r1, r1, 1024\nENDLOOP\nBAR\nSETP.LT p0, tid, 256\nIF p0\n  LDS r6, [tid]\n  ST [tid], r6\nENDIF\n";

// __LEVELS__
