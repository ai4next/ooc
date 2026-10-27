//! Chapter 5 · Pipelines and Scheduling (Prism). From 5-2 on every unit has
//! its own latency: multiply 4 beats, load 3, everything else 2.

use oo_base::Tr;

use super::prism::{BEAT_OPS, base, paint, rom_case};
use super::{below, rng};
use crate::isa::LANES;
use crate::task::*;
use crate::vm::Profile;

pub fn tasks() -> Vec<Task> {
    vec![
        unit_latency(),
        interleave(),
        unroll(),
        q_format(),
        fixed_overflow(),
        ripple_tank(),
    ]
}

/// One value per lane.
fn lanes(f: impl Fn(usize) -> i32) -> Vec<Option<i32>> {
    (0..LANES).map(|l| Some(f(l))).collect()
}

/// `n` banks of 32 readings in `0..=top`.
fn banks(seed: u64, n: usize, top: i32) -> Vec<i32> {
    let mut r = rng(seed);
    (0..n * LANES).map(|_| below(&mut r, top + 1)).collect()
}

/// Like `banks`, but the first lanes read 0 in every bank and the next lanes
/// read `top` in every bank.
fn extremes(seed: u64, n: usize, top: i32) -> Vec<i32> {
    let mut rom = banks(seed, n, top);
    for k in 0..n {
        for l in 0..4 {
            rom[k * LANES + l] = 0;
            rom[k * LANES + 4 + l] = top;
        }
    }
    rom
}

/// The reading of bank `k` on a lane.
fn at(r: &[i32], k: usize, l: usize) -> i32 {
    r[k * LANES + l]
}

fn unit_latency() -> Task {
    Task {
        profile: Profile::PRISM_UNITS,
        title: Tr::new("A Beat Each", "各有各的拍"),
        brief: Tr::new(
            "Huajuan's ledger has four banks of 32 readings, each 0 to 9: a = rom[0..32], b = rom[32..64], c = rom[64..96], d = rom[96..128]. out[lane] = a×b + c − d.\nPrism's new revision gives every unit its own latency: MUL lands 4 beats after issue, LD 3, and ADD, SUB and the rest 2. Still no interlock.\nThe program below was scheduled for \"three beats everywhere\". Make it right, then make it short.",
            "花卷的账本有四组读数，每组 32 个，取值 0 到 9：a = rom[0..32]，b = rom[32..64]，c = rom[64..96]，d = rom[96..128]。out[lane] = a×b + c − d。\nPrism 的新版本让每个单元有自己的延迟：MUL 发出 4 拍后落地，LD 是 3 拍，ADD、SUB 等是 2 拍。依然没有互锁。\n下面的程序是按“处处三拍”排的。先让它正确，再让它尽量短。",
        ),
        cases: || [151, 152, 153].map(|s| rom_case(banks(s, 4, 9))).to_vec(),
        hidden: || [154, 155].map(|s| rom_case(extremes(s, 4, 9))).to_vec(),
        expect: |c| {
            let r = &c.rom;
            lanes(|l| at(r, 0, l) * at(r, 1, l) + at(r, 2, l) - at(r, 3, l))
        },
        goals: [Goal::new(Metric::Cycles, 17), Goal::new(Metric::Size, 14)],
        starter: Source::Code(
            "; scheduled for three beats everywhere | 按“处处三拍”排的\nADD r1, lane, 32\nADD r2, lane, 64\nADD r3, lane, 96\nLD r0, [lane]\nLD r1, [r1]\nLD r2, [r2]\nLD r3, [r3]\nNOP\nMUL r0, r0, r1\nNOP\nNOP\nADD r0, r0, r2\nNOP\nNOP\nSUB r0, r0, r3\nNOP\nNOP\nST [lane], r0\n",
        ),
        reference: Source::Code(
            "ADD r1, lane, 32\nLD r0, [lane]\nLD r1, [r1]\nADD r2, lane, 64\nADD r3, lane, 96\nMUL r0, r0, r1\nLD r2, [r2]\nLD r3, [r3]\nNOP\nADD r0, r0, r2\nNOP\nSUB r0, r0, r3\nNOP\nST [lane], r0\n",
        ),
        predict: Predict {
            question: Tr::new(
                "Compared with an ADD's result, a MUL's result lands…",
                "和 ADD 的结果相比，MUL 的结果落地得……",
            ),
            options: [
                Tr::new("Sooner", "更早"),
                Tr::new("At the same time", "一样"),
                Tr::new("Later", "更晚"),
            ],
            answer: 2,
            why: Tr::new(
                "MUL needs 4 beats, ADD only 2. The multiplier is the bigger circuit, so its answer arrives later, and the program has to know it.",
                "MUL 要 4 拍，ADD 只要 2 拍。乘法器是更大的电路，答案来得更晚，程序得自己记着这一点。",
            ),
        },
        hints: [
            Tr::new(
                "Run it and open the Pipeline tab. The red read is the ADD right after MUL: it came three beats after, and MUL now needs four.",
                "运行一次，打开“流水线”页。红格是 MUL 后面那条 ADD：它在三拍后读，而 MUL 现在要四拍。",
            ),
            Tr::new(
                "After a MUL, leave three beats of other work before using its result. After an ADD, one is enough. The address sums and the other loads fit the gaps exactly.",
                "MUL 之后，隔三拍的别的工作再用它的结果；ADD 之后，隔一拍就够了。地址计算和其余的加载正好填满这些空拍。",
            ),
            Tr::new(
                "Start the MUL as early as its two loads allow, then issue the loads for c and d while it works. NOP only where nothing else is ready.",
                "两个加载一到位就发出 MUL，然后趁它工作的时候发 c 和 d 的加载。只有实在没有别的可做时才放 NOP。",
            ),
        ],
        insight: Insight {
            title: Tr::new("A latency for every unit", "每个单元各有延迟"),
            line: Tr::new(
                "Every unit has its own latency, so every result needs its own gap. Schedule against the slowest link of the chain, not the average.",
                "每个单元有各自的延迟，每个结果就需要各自的空档。要按链上最慢的一环来排，而不是按平均值。",
            ),
            reality: Tr::new(
                "Real pipelines differ the same way: an integer add takes a few cycles, a multiply a few more, a memory load far more. Compilers keep a latency table per instruction class.",
                "真实的流水线也是这样：整数加法几个周期，乘法多几个，访存多得多。编译器为每类指令都维护一张延迟表。",
            ),
            myth: Tr::new(
                "Myth: all instructions take the same time. A schedule that is right for \"three beats\" reads stale products here.",
                "误解：所有指令一样快。按“三拍”排对的程序，放到这里会读到乘法的旧值。",
            ),
            simplified: Tr::new(
                "Real latencies also move with caches and contention. Prism keeps them fixed so you can plan to the beat.",
                "真实延迟还会随缓存和争用而变化。Prism 把它们固定下来，让你能精确到拍去规划。",
            ),
        },
        ..base("unit-latency", 2, View::Lanes, BEAT_OPS)
    }
}

fn interleave() -> Task {
    Task {
        profile: Profile::PRISM_UNITS,
        title: Tr::new("Interleave", "交错"),
        brief: Tr::new(
            "Huajuan runs two looms side by side. Six banks of 32 readings, each 0 to 9, sit at rom[0..32], rom[32..64], and so on up to rom[160..192]: call them a to f. out[lane] = (a×b)×c + (d×e)×f.\nThe two products have nothing to do with each other. Latencies as before: MUL 4, LD 3, ADD 2.\nThe starter finishes one chain, then the other, and fills every wait with NOP. It is right, but the frame budget is ≤ 24 cycles.",
            "花卷让两台织机并排工作。六组读数，每组 32 个，取值 0 到 9，依次在 rom[0..32]、rom[32..64]，一直到 rom[160..192]：记作 a 到 f。out[lane] = (a×b)×c + (d×e)×f。\n两个乘积互不相干。延迟照旧：MUL 4 拍，LD 3 拍，ADD 2 拍。\nstarter 先把一条链写完，再写另一条，每个空拍都用 NOP 填。结果是对的，但一帧的预算是 ≤ 24 个周期。",
        ),
        cases: || [161, 162, 163].map(|s| rom_case(banks(s, 6, 9))).to_vec(),
        hidden: || [164, 165].map(|s| rom_case(extremes(s, 6, 9))).to_vec(),
        expect: |c| {
            let r = &c.rom;
            lanes(|l| {
                at(r, 0, l) * at(r, 1, l) * at(r, 2, l) + at(r, 3, l) * at(r, 4, l) * at(r, 5, l)
            })
        },
        limit: Some(Goal::new(Metric::Cycles, 24)),
        goals: [Goal::new(Metric::Cycles, 23), Goal::new(Metric::Size, 22)],
        starter: Source::Code(
            "; chain one, then chain two | 先写第一条链，再写第二条\nADD r1, lane, 32\nLD r0, [lane]\nLD r1, [r1]\nADD r2, lane, 64\nNOP\nMUL r0, r0, r1\nLD r2, [r2]\nNOP\nNOP\nMUL r0, r0, r2\nNOP\nNOP\nNOP\nADD r1, lane, 96\nADD r2, lane, 128\nLD r1, [r1]\nLD r2, [r2]\nADD r3, lane, 160\nNOP\nMUL r1, r1, r2\nLD r3, [r3]\nNOP\nNOP\nMUL r1, r1, r3\nNOP\nNOP\nNOP\nADD r0, r0, r1\nNOP\nST [lane], r0\n",
        ),
        reference: Source::Code(
            "ADD r1, lane, 32\nADD r2, lane, 128\nADD r3, lane, 96\nLD r1, [r1]\nLD r2, [r2]\nLD r3, [r3]\nLD r0, [lane]\nADD r4, lane, 64\nADD r5, lane, 160\nMUL r2, r2, r3\nMUL r0, r0, r1\nLD r4, [r4]\nLD r5, [r5]\nNOP\nMUL r0, r0, r4\nMUL r2, r2, r5\nNOP\nNOP\nNOP\nADD r0, r0, r2\nNOP\nST [lane], r0\n",
        ),
        predict: Predict {
            question: Tr::new(
                "Two multiply chains are independent. Issue their instructions interleaved, and the whole kernel takes…",
                "两条乘法链互不依赖。把它们的指令交错发射，整个内核需要……",
            ),
            options: [
                Tr::new(
                    "Fewer cycles, by about one chain's idle beats",
                    "更少的周期，约少一条链的空拍数",
                ),
                Tr::new("The same number of cycles", "一样多的周期"),
                Tr::new("More cycles", "更多的周期"),
            ],
            answer: 0,
            why: Tr::new(
                "While one chain waits for its MUL, an instruction of the other chain issues in the gap. The waits overlap instead of adding up.",
                "一条链在等乘法的时候，另一条链的指令就在空档里发出。等待是重叠的，而不是叠加的。",
            ),
        },
        hints: [
            Tr::new(
                "Count the NOPs in the starter. Each one is a beat where the other chain could have done real work.",
                "数一数 starter 里的 NOP。每一个都是另一条链本可以做正事的一拍。",
            ),
            Tr::new(
                "Both chains start with address sums and loads. Do those for both first, so that both first MULs are on their way before you wait for anything.",
                "两条链都以地址计算和加载开头。先把两条链的这部分都做了，这样在开始等待之前，两个第一次乘法都已经上路。",
            ),
            Tr::new(
                "Addresses, then the loads for b, e, d and a, MUL d×e and a×b, then the loads for c and f. The two last MULs come right behind, and one ADD joins them.",
                "先算地址，再加载 b、e、d、a，然后做 d×e 和 a×b 两次乘法，再加载 c 和 f。最后两次乘法紧随其后，一个 ADD 把它们合起来。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Interleaving", "交错发射"),
            line: Tr::new(
                "Independent chains can share the same waiting. One chain's idle beats are the other's working beats.",
                "互不依赖的链可以共用同一段等待。一条链的空拍，正是另一条链的工作拍。",
            ),
            reality: Tr::new(
                "VLIW and DSP compilers do exactly this with software pipelining. Out-of-order CPUs do it in hardware, picking whichever instruction is ready.",
                "VLIW 和 DSP 的编译器用软件流水做的就是这件事；乱序 CPU 则在硬件里做，哪条指令就绪就发哪条。",
            ),
            myth: Tr::new(
                "Myth: tidy code, one job at a time, is fast code. Tidy code leaves every wait empty.",
                "误解：一件做完再做下一件，整整齐齐的代码就是快的代码。整齐的代码把每一次等待都空着。",
            ),
            simplified: Tr::new(
                "A real GPU picks from many independent warps in hardware. Here you do the picking by hand, inside a single warp.",
                "真实的 GPU 在硬件里从许多互不相干的 warp 里挑选。这里你在单个 warp 里亲手挑。",
            ),
        },
        ..base("interleave", 3, View::Lanes, BEAT_OPS)
    }
}

fn unroll() -> Task {
    Task {
        profile: Profile::PRISM_UNITS,
        title: Tr::new("Unroll", "展开"),
        brief: Tr::new(
            "Eight banks of 32 readings, each 0 to 99, at rom[0..32], rom[32..64], and so on up to rom[224..256]. out[lane] is the sum of the eight readings of that lane.\nThe starter loops eight times. Each round loads one reading and then waits three beats for it, so the NOPs add up. The frame budget is ≤ 30 cycles.\nLOOP's count is a plain number, so unrolling needs no new kind of instruction: just write the rounds out.",
            "八组读数，每组 32 个，取值 0 到 99，依次在 rom[0..32]、rom[32..64]，一直到 rom[224..256]。out[lane] 是这条车道八个读数之和。\nstarter 用循环转八圈。每圈读一个数，然后等它三拍，NOP 就这样一圈圈累积起来。一帧的预算是 ≤ 30 个周期。\nLOOP 的圈数是个普通的数，所以展开不需要新的指令种类：把每一圈直接写出来就行。",
        ),
        cases: || [171, 172, 173].map(|s| rom_case(banks(s, 8, 99))).to_vec(),
        hidden: || [174, 175].map(|s| rom_case(extremes(s, 8, 99))).to_vec(),
        expect: |c| lanes(|l| (0..8).map(|k| at(&c.rom, k, l)).sum()),
        limit: Some(Goal::new(Metric::Cycles, 30)),
        goals: [Goal::new(Metric::Cycles, 27), Goal::new(Metric::Size, 25)],
        starter: Source::Code(
            "; one reading per round, then wait for it | 每圈读一个数，然后等它\nMOV r0, 0\nMOV r1, lane\nLOOP 8\nLD r2, [r1]\nADD r1, r1, 32\nNOP\nADD r0, r0, r2\nENDLOOP\nST [lane], r0\n",
        ),
        reference: Source::Code(
            "ADD r1, lane, 32\nADD r2, lane, 64\nLD r0, [lane]\nLD r1, [r1]\nLD r2, [r2]\nADD r3, lane, 96\nADD r4, lane, 128\nLD r3, [r3]\nADD r0, r0, r1\nLD r4, [r4]\nADD r5, lane, 160\nADD r6, lane, 192\nADD r0, r0, r2\nLD r5, [r5]\nLD r6, [r6]\nADD r7, lane, 224\nADD r0, r0, r3\nLD r7, [r7]\nADD r8, r4, r5\nADD r0, r0, r6\nADD r8, r8, r7\nNOP\nADD r0, r0, r8\nNOP\nST [lane], r0\n",
        ),
        predict: Predict {
            question: Tr::new(
                "A loop body loads one reading, then waits 3 beats for it. Unrolled, what can sit between two neighbouring loads?",
                "循环体读一个数，然后要等它 3 拍。展开之后，相邻两次读取之间可以放什么？",
            ),
            options: [
                Tr::new(
                    "Nothing: loads must be back to back",
                    "什么都不行：读取必须紧挨着",
                ),
                Tr::new(
                    "Independent instructions: the next address, the next load, an add",
                    "互不依赖的指令：下一个地址、下一次读取、一次加法",
                ),
                Tr::new("Only NOPs", "只能放 NOP"),
            ],
            answer: 1,
            why: Tr::new(
                "Once the rounds sit side by side, the wait of one load is covered by the work of the others. NOPs are only for what is left over.",
                "各圈并排摆开后，一次读取的等待就被其他读取的工作盖住了。NOP 只用来补剩下的空。",
            ),
        },
        hints: [
            Tr::new(
                "Look at the starter's loop body: one useful add, one address step, a load, and a NOP. Most of every round is waiting.",
                "看看 starter 的循环体：一次有用的加法、一步地址、一次读取，外加一个 NOP。每一圈的大半时间都在等。",
            ),
            Tr::new(
                "Write the eight loads out one after another. While one is in flight, issue the next address sum or the next load, and add what has already landed.",
                "把八次读取一个接一个写出来。一次读取在途的时候，发下一个地址计算或下一次读取，再把已经落地的数加起来。",
            ),
            Tr::new(
                "Compute the addresses early and keep two partial sums: the early readings go into one, the late ones into the other. Merge them at the very end.",
                "地址早点算好，并维护两个部分和：先到的读数加进一个，后到的加进另一个，最后才合并。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Unrolling", "循环展开"),
            line: Tr::new(
                "A loop waits for itself every round. Unrolled, the wait of one round is covered by the work of the next.",
                "循环每一圈都在等自己。展开之后，一圈的等待被下一圈的工作盖住。",
            ),
            reality: Tr::new(
                "Compilers unroll loops for exactly this reason, trading code size for fewer stalls and less loop overhead. GPU compilers do it too.",
                "编译器展开循环就是出于这个原因：用代码体积换更少的停顿和更少的循环开销。GPU 编译器也这么做。",
            ),
            myth: Tr::new(
                "Myth: a shorter program is a faster program. Here the longer one wins, and Prism only has room for 40 instructions.",
                "误解：程序越短越快。这里更长的那个反而赢，而 Prism 一共只放得下 40 条指令。",
            ),
            simplified: Tr::new(
                "A real compiler picks the unroll factor from a size and speed model. Here you feel the trade directly: cycles and size pull against each other.",
                "真实的编译器靠体积与速度模型选展开倍数。这里你能直接感觉到这个权衡：周期和指令数互相拉扯。",
            ),
        },
        ..base("unroll", 5, View::Lanes, BEAT_OPS)
    }
}

/// rom[0..32]: Q8 readings, rom[32]: the gain in Q8. With `extremes`, lane 0
/// reads 0 and lane 1 reads the top value.
fn gauge(seed: u64, gain: i32, extremes: bool) -> Case {
    let mut r = rng(seed);
    let mut rom: Vec<i32> = (0..LANES).map(|_| below(&mut r, 255 * 256 + 1)).collect();
    if extremes {
        rom[0] = 0;
        rom[1] = 255 * 256;
    }
    rom.push(gain);
    rom_case(rom)
}

fn q_format() -> Task {
    Task {
        profile: Profile::PRISM_UNITS,
        title: Tr::new("Fixed Point", "定点数"),
        brief: Tr::new(
            "Huajuan's gauge reports in Q8 fixed point: the real value times 256, stored as a whole number. rom[0..32] holds one reading per lane (0 to 255 × 256), and rom[32] holds the gain, also Q8: 0.75 is stored as 192.\nout[lane] = reading × gain, again in Q8, rounded down.\nThe starter multiplies the two as plain integers. Fix the format.",
            "花卷的量表用 Q8 定点数报读数：真值乘 256，存成整数。rom[0..32] 是每条车道一个读数（0 到 255 × 256），rom[32] 是增益，同样是 Q8：0.75 存成 192。\nout[lane] = 读数 × 增益，结果同样是 Q8，向下取整。\nstarter 把两个数当普通整数直接相乘。把格式改对。",
        ),
        cases: || {
            [(181, 128), (182, 192), (183, 384)]
                .map(|(s, g)| gauge(s, g, false))
                .to_vec()
        },
        hidden: || {
            [(184, 576), (185, 3)]
                .map(|(s, g)| gauge(s, g, true))
                .to_vec()
        },
        expect: |c| lanes(|l| (c.rom[l] * c.rom[32]) >> 8),
        goals: [Goal::new(Metric::Cycles, 13), Goal::new(Metric::Size, 11)],
        starter: Source::Code(
            "; the gain as a plain integer | 把增益当整数直接乘\nLD r1, [32]\nLD r0, [lane]\nNOP\nNOP\nMUL r0, r0, r1\nNOP\nNOP\nNOP\nNOP\nST [lane], r0\n",
        ),
        reference: Source::Code(
            "LD r1, [32]\nLD r0, [lane]\nNOP\nNOP\nMUL r0, r0, r1\nNOP\nNOP\nNOP\nSHR r0, r0, 8\nNOP\nST [lane], r0\n",
        ),
        predict: Predict {
            question: Tr::new(
                "A Q8 reading times a Q8 gain (0.75 is stored as 192). In what format is the product?",
                "Q8 的读数乘以 Q8 的增益（0.75 存成 192）。乘积是什么格式？",
            ),
            options: [
                Tr::new("Q8", "Q8"),
                Tr::new("Q16", "Q16"),
                Tr::new("A plain integer", "普通整数"),
            ],
            answer: 1,
            why: Tr::new(
                "Each factor carries a factor of 256, so the product carries 256 × 256 = 65536. Shift right by 8 to get back to Q8.",
                "每个因子都带着一个 256，乘积就带着 256 × 256 = 65536。右移 8 位才回到 Q8。",
            ),
        },
        hints: [
            Tr::new(
                "Run the starter: every output is 256 times too big. Rounding the gain 0.75 to 0 or 1 would lose everything, so it has to stay 192.",
                "运行 starter：每个输出都大了 256 倍。把增益 0.75 取整成 0 或 1 会丢掉一切，所以它必须保持 192。",
            ),
            Tr::new(
                "Q8 × Q8 = Q16. One right shift by 8 turns it back into Q8.",
                "Q8 × Q8 = Q16。右移 8 位就变回 Q8。",
            ),
            Tr::new(
                "MUL needs 4 beats, so the SHR must wait for it. Load the gain once with LD r1, [32]; where nothing else is ready, use NOP.",
                "MUL 要 4 拍，所以 SHR 得等它。用 LD r1, [32] 读一次增益；没有别的可做的地方，就放 NOP。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Fixed point", "定点数"),
            line: Tr::new(
                "A fixed-point number is a whole number with an agreed scale. A product carries both scales, so shift one back out.",
                "定点数就是约定了比例的整数。乘积带着两个比例，所以要把其中一个移出去。",
            ),
            reality: Tr::new(
                "Early 3D consoles and DSPs had no floating-point unit and did all their maths this way. Fixed point is still common in audio and embedded chips.",
                "早期的 3D 游戏机和 DSP 没有浮点单元，所有运算都这样做。定点数在音频和嵌入式芯片里至今常见。",
            ),
            myth: Tr::new(
                "Myth: to multiply by 0.75, store 0.75. A whole number cannot hold a fraction, so scale it.",
                "误解：要乘 0.75，就存 0.75。整数装不下小数，得给它加比例。",
            ),
            simplified: Tr::new(
                "Real chips have float hardware. Q8 is the same idea made visible: you decide where the point sits.",
                "真实的芯片有浮点硬件。Q8 是同一个想法的显形：小数点放在哪里，由你决定。",
            ),
        },
        ..base("q-format", 6, View::Lanes, BEAT_OPS)
    }
}

/// rom[0..32] and rom[32..64]: Q16 readings (0 to 1.0, so 0..=65536). With
/// `spiked`, some lanes have both readings near 1.0 and some both near 0.
fn q16(seed: u64, spiked: bool) -> Case {
    let mut r = rng(seed);
    let mut rom: Vec<i32> = (0..2 * LANES).map(|_| below(&mut r, 65537)).collect();
    if spiked {
        for l in 0..8 {
            rom[l] = 65536 - below(&mut r, 100);
            rom[LANES + l] = 65536 - below(&mut r, 100);
            rom[8 + l] = below(&mut r, 2000);
            rom[LANES + 8 + l] = below(&mut r, 2000);
        }
    }
    rom_case(rom)
}

fn fixed_overflow() -> Task {
    Task {
        profile: Profile::PRISM_UNITS,
        title: Tr::new("Fixed-Point Overflow", "定点溢出"),
        brief: Tr::new(
            "Huajuan's gauges now report in Q16: a real value from 0 to 1, times 65536. Two banks of 32: a = rom[0..32] and b = rom[32..64]. out[lane] should be a × b, in Q16.\nThe kernel multiplies them and shifts right by 16. Some lanes come out negative or wild.\nFind the beat where a multiply wrapped around 32 bits. Point at it in the Warp River, then fix it. Being off by up to 512 (under a hundredth of 1.0) is fine.",
            "花卷的量表现在用 Q16 报数：真值在 0 到 1 之间，乘 65536。两组各 32 个：a = rom[0..32]，b = rom[32..64]。out[lane] 应当是 a × b，格式仍是 Q16。\n内核先把两个数相乘，再右移 16 位。有些车道算出来是负数，或者一团乱。\n找到那一拍：某次乘法的结果回绕了 32 位。先在 Warp River 里指出它，再修好它。允许差到 512（不到 1.0 的百分之一）。",
        ),
        cases: || [191, 192, 193].map(|s| q16(s, false)).to_vec(),
        hidden: || [194, 195].map(|s| q16(s, true)).to_vec(),
        expect: |c| lanes(|l| ((i64::from(c.rom[l]) * i64::from(c.rom[LANES + l])) >> 16) as i32),
        tolerance: 512,
        goals: [Goal::new(Metric::Cycles, 14), Goal::new(Metric::Size, 12)],
        starter: Source::Code(
            "; Q16 × Q16, then back to Q16 | Q16 乘 Q16，再回到 Q16\nADD r1, lane, 32\nLD r0, [lane]\nLD r1, [r1]\nNOP\nNOP\nMUL r0, r0, r1\nNOP\nNOP\nNOP\nSHR r0, r0, 16\nNOP\nST [lane], r0\n",
        ),
        reference: Source::Code(
            "ADD r1, lane, 32\nLD r0, [lane]\nLD r1, [r1]\nNOP\nSHR r0, r0, 8\nSHR r1, r1, 8\nNOP\nMUL r0, r0, r1\nNOP\nNOP\nNOP\nST [lane], r0\n",
        ),
        predict: Predict {
            question: Tr::new(
                "Two Q16 readings of 1.0 (65536 each) are multiplied as 32-bit whole numbers. What does the result hold?",
                "两个 Q16 的 1.0（各是 65536）按 32 位整数相乘。结果里是什么？",
            ),
            options: [
                Tr::new("4294967296, the exact product", "4294967296，精确的乘积"),
                Tr::new("A wrapped-around value: here 0", "回绕之后的值：这里是 0"),
                Tr::new("The largest 32-bit number", "最大的 32 位数"),
            ],
            answer: 1,
            why: Tr::new(
                "65536 × 65536 = 2³² does not fit in 32 bits. The multiplier keeps the low 32 bits, here all zero, and never complains.",
                "65536 × 65536 = 2³² 装不进 32 位。乘法器只留下低 32 位，这里全是 0，而且一声不吭。",
            ),
        },
        hints: [
            Tr::new(
                "The wrong lanes are the big ones. A Q16 × Q16 product is Q32: for values near 1.0 it is near 2³², far too big for a 32-bit register.",
                "出错的都是大数的车道。Q16 × Q16 的乘积是 Q32：值接近 1.0 时它接近 2³²，32 位寄存器远远装不下。",
            ),
            Tr::new(
                "Shifting after the multiply is too late. Make room before it: shrink both operands, then multiply.",
                "乘完再右移已经晚了。要在乘之前腾出空间：先缩小两个操作数，再相乘。",
            ),
            Tr::new(
                "SHR both readings by 8 first: Q16 becomes Q8, and Q8 × Q8 is Q16 again, needing at most 17 bits. No shift after. Q4 would round the small readings to 0.",
                "先把两个读数各 SHR 8：Q16 变成 Q8，而 Q8 × Q8 又是 Q16，最多只要 17 位，之后不用再移。改成 Q4 会让小读数被舍成 0。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Overflow in the middle", "中间结果溢出"),
            line: Tr::new(
                "A product needs twice the bits of its factors. Make room before multiplying, not after.",
                "乘积需要因子两倍的位数。要在乘之前腾出空间，而不是乘完再说。",
            ),
            reality: Tr::new(
                "Fixed-point audio and graphics code shifts its operands first or keeps a wider accumulator, because a 16-bit by 16-bit multiply needs 32 bits.",
                "定点的音频和图形代码要么先移位再乘，要么用更宽的累加器，因为 16 位乘 16 位需要 32 位。",
            ),
            myth: Tr::new(
                "Myth: if the answer fits, the middle fits too. The product can overflow even when the answer is small.",
                "误解：答案装得下，中间结果就装得下。答案很小，乘积照样会溢出。",
            ),
            simplified: Tr::new(
                "Real chips offer widening multiplies and saturating arithmetic. Prism's MUL keeps only the low 32 bits, silently.",
                "真实的芯片有加宽乘法和饱和运算。Prism 的 MUL 只留低 32 位，而且悄无声息。",
            ),
        },
        clue: Some(Clue {
            what: Tr::new(
                "the beat where a multiply overflowed 32 bits",
                "乘法结果溢出 32 位的那一拍",
            ),
            find: crate::grade::first_overflow,
        }),
        ..base("fixed-overflow", 7, View::Lanes, BEAT_OPS)
    }
}

/// rom[0..2]: where the water was tapped; rom[2..18]: wave shape W; rom[18..34]:
/// fade D. Both tables are Q8.
fn ripple(w: [i32; 16], fade: [i32; 16], tap: (i32, i32), frame: i32) -> Case {
    let mut rom = vec![tap.0, tap.1];
    rom.extend(w);
    rom.extend(fade);
    Case { rom, frame }
}

/// A smooth wave that fades with distance.
fn pond(tap: (i32, i32), frame: i32) -> Case {
    let wave = |i: usize| {
        let phase = i as f64 * std::f64::consts::TAU / 16.0;
        (128.0 + 127.0 * phase.sin()).round() as i32
    };
    let fade = |i: usize| (255.0 * 0.8f64.powi(i as i32)).round() as i32;
    ripple(
        std::array::from_fn(wave),
        std::array::from_fn(fade),
        tap,
        frame,
    )
}

/// Any legal tables at all.
fn choppy(seed: u64, tap: (i32, i32), frame: i32) -> Case {
    let mut r = rng(seed);
    let w = std::array::from_fn(|_| below(&mut r, 256));
    let fade = std::array::from_fn(|_| below(&mut r, 256));
    ripple(w, fade, tap, frame)
}

/// Colour = ((W[(d − frame) AND 15] × D[min(d >> 1, 15)]) >> 8) >> 5.
fn rings(c: &Case) -> Vec<Option<i32>> {
    paint(c, |r, x, y| {
        let d = (x - r[0]).abs() + (y - r[1]).abs();
        let wave = r[2 + ((d - c.frame) & 15) as usize];
        let fade = r[18 + (d >> 1).min(15) as usize];
        ((wave * fade) >> 8) >> 5
    })
}

fn ripple_tank() -> Task {
    Task {
        profile: Profile::PRISM_UNITS,
        title: Tr::new("The Ripple Tank", "水波盘"),
        brief: Tr::new(
            "Huajuan wants the school's Prism to be a ripple tank: a student taps the water and rings spread out from that spot.\nrom[0] and rom[1] hold the tap point (cx, cy). rom[2..18] is a table of 16 wave heights W and rom[18..34] a table of 16 fade factors D, both Q8 (0 to 255).\nFor every pixel: d = |x − cx| + |y − cy|, phase = (d − frame) AND 15, amplitude = (W[phase] × D[min(d >> 1, 15)]) >> 8, colour = amplitude >> 5.\nThe whole frame has to fit in 300 cycles. Latencies as in 5-2. There is no starting program.",
            "花卷想让学校那台 Prism 变成水波盘：学生在水面上点一下，一圈圈波纹从那里扩散开。\nrom[0] 和 rom[1] 是点击位置 (cx, cy)。rom[2..18] 是 16 项波形高度 W，rom[18..34] 是 16 项衰减系数 D，都是 Q8（0 到 255）。\n每个像素：d = |x − cx| + |y − cy|，相位 = (d − frame) AND 15，幅度 = (W[相位] × D[min(d >> 1, 15)]) >> 8，颜色 = 幅度 >> 5。\n整帧要在 300 个周期之内完成。延迟和 5-2 一样。没有初始方案。",
        ),
        cases: || {
            [((8, 8), 0), ((15, 0), 3), ((0, 11), 9), ((5, 10), 40)]
                .map(|(tap, f)| pond(tap, f))
                .to_vec()
        },
        hidden: || {
            [(196, (0, 0), 17), (197, (15, 15), 60), (198, (3, 12), 29)]
                .map(|(s, tap, f)| choppy(s, tap, f))
                .to_vec()
        },
        expect: rings,
        limit: Some(Goal::new(Metric::Cycles, 300)),
        goals: [Goal::new(Metric::Cycles, 232), Goal::new(Metric::Size, 28)],
        starter: Source::Code(
            "; one pixel per thread | 每个线程一个像素\n; MUL lands after 4 beats, LD after 3, the rest after 2 | MUL 4 拍落地，LD 3 拍，其余 2 拍\n",
        ),
        reference: Source::Code(
            "LD r0, [0]\nLD r1, [1]\nNOP\nSUB r2, r0, x\nSUB r0, x, r0\nSUB r3, r1, y\nSUB r1, y, r1\nMAX r0, r0, r2\nMAX r1, r1, r3\nNOP\nADD r0, r0, r1\nNOP\nSUB r2, r0, frame\nSHR r1, r0, 1\nAND r2, r2, 15\nADD r1, r1, 18\nADD r2, r2, 2\nLD r1, [r1]\nLD r2, [r2]\nNOP\nNOP\nMUL r1, r1, r2\nNOP\nNOP\nNOP\nSHR r1, r1, 13\nNOP\nPIX r1\n",
        ),
        // The students tap the water; the rings also keep moving on their own.
        live: Some(|frame, at| {
            let tap = ((at[0] + 4).clamp(0, 15), (at[1] + 4).clamp(0, 15));
            pond(tap, frame / 3)
        }),
        steer: Some(Tr::new("TAP THE WATER", "点一下水面")),
        predict: Predict {
            question: Tr::new(
                "Two table reads feed the final multiply: W by phase, D by distance. How should they be issued?",
                "两次查表喂给最后的乘法：按相位查 W，按距离查 D。该怎么发出？",
            ),
            options: [
                Tr::new(
                    "One, then wait with NOPs, then the other",
                    "先发一个，用 NOP 等着，再发另一个",
                ),
                Tr::new(
                    "Back to back, so both are in flight together",
                    "紧挨着发出，让两个同时在途",
                ),
                Tr::new("Only one of them is needed", "只需要其中一个"),
            ],
            answer: 1,
            why: Tr::new(
                "Neither address depends on the other read, so the second wait hides under the first. The MUL starts as soon as the later one lands.",
                "两个地址都不依赖对方的读取，所以第二次等待藏在第一次底下。较晚的那个一落地，MUL 就能开始。",
            ),
        },
        hints: [
            Tr::new(
                "Find the independent chains: |x − cx| and |y − cy| have nothing to do with each other, and the phase and the fade index both come from d, side by side.",
                "找出互不依赖的链：|x − cx| 和 |y − cy| 互不相干，而相位和衰减下标都从 d 出发，并排计算。",
            ),
            Tr::new(
                "|v| is MAX(v, −v): subtract both ways, then MAX. Do both axes together. The loads of cx and cy take 3 beats, so start them first.",
                "|v| 就是 MAX(v, −v)：两个方向各减一次，再取 MAX。两根轴一起做。加载 cx 和 cy 要 3 拍，所以最先发出。",
            ),
            Tr::new(
                "Phase: SUB frame, AND 15, ADD 2. Fade: SHR 1, ADD 18. Issue both LDs back to back, then MUL, then one SHR by 13 (8 for the Q8 product, 5 for the colour), then PIX.",
                "相位：减 frame，AND 15，加 2。衰减：SHR 1，加 18。两个 LD 紧挨着发，然后 MUL，再一次 SHR 13（8 位给 Q8 乘积，5 位给颜色），最后 PIX。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Putting it together", "全部合在一起"),
            line: Tr::new(
                "Fixed-point maths, per-unit latencies and independent chains pay off in one kernel. The schedule is the program.",
                "定点运算、分单元延迟、互不依赖的链，都在同一个内核里兑现。排程本身就是程序。",
            ),
            reality: Tr::new(
                "Water in old demos was often a table lookup on distance, exactly like this: a wave table indexed by d − time, a fade table indexed by d.",
                "老式演示程序里的水波常常就是按距离查表：用 d − 时间 去查波形表，用 d 去查衰减表。",
            ),
            myth: Tr::new(
                "Myth: a faster program needs a cleverer algorithm. Often it is the same algorithm in a better order.",
                "误解：更快的程序需要更聪明的算法。常常是同一个算法，换一个更好的顺序。",
            ),
            simplified: Tr::new(
                "A real ripple solves a wave equation over time. This one is a distance function through two tables, cheap enough for a classroom.",
                "真实的水波要随时间求解波动方程。这里只是把距离函数过两张表，便宜得足够在教室里跑。",
            ),
        },
        ..base("ripple-tank", 8, View::Screen, BEAT_OPS)
    }
}
