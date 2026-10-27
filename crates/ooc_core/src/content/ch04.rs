//! Chapter 4 · Lumen at full load: lanes that disagree.

use oo_base::Tr;

use super::warp::{lanes, rom_cases, values};
use super::{SCALAR_OPS, WARP_OPS};
use crate::isa::FB_LEN;
use crate::task::*;
use crate::vm::Launch;

fn base(id: &'static str, slot: u8, ops: &'static [crate::isa::OpKind]) -> Task {
    super::warp::base(4, id, slot, ops)
}

pub fn tasks() -> Vec<Task> {
    vec![
        the_mask(),
        the_else(),
        if_or_sel(),
        the_fork(),
        the_uniform(),
        escape_time(),
        ghost_lane(),
        weather_machine(),
    ]
}

/// What the three-tier ledger does to a reading.
fn tiers(v: i32) -> i32 {
    if v < 300 {
        v + 100
    } else if v < 700 {
        v * 2
    } else {
        v - 100
    }
}

fn the_else() -> Task {
    Task {
        title: Tr::new("The Else", "否则"),
        brief: Tr::new(
            "The ledger sorts 32 readings from rom[0..32] into three tiers: below 300 gain 100; from 300 up to 699 are doubled; 700 and above lose 100. Results go to out[lane].\nThe old clerk only knows the first tier. Teach it the other two — the lanes that miss one tier must wait while the others take theirs.",
            "账本把 rom[0..32] 的 32 个读数分成三档：低于 300 的加 100；300 到 699 的翻倍；700 及以上的减 100。结果写入 out[lane]。\n旧办事员只认得第一档。把另外两档教给它——没落在某一档的车道，要等其他车道走完自己那一档。",
        ),
        cases: || rom_cases(|s| values(s, 32, 0, 700), &[201, 202, 203]),
        hidden: || {
            rom_cases(
                |s| {
                    let mut v = values(s, 32, 0, 1000);
                    v[..4].copy_from_slice(&[300, 299, 699, 700]);
                    v
                },
                &[204, 205],
            )
        },
        expect: |c| lanes(|l| tiers(c.rom[l])),
        goals: [Goal::new(Metric::Cycles, 13), Goal::new(Metric::Size, 13)],
        starter: Source::Code(
            "LD r0, [lane]\nSETP.LT p0, r0, 300\nIF p0\n  ADD r0, r0, 100\nENDIF\nST [lane], r0\n",
        ),
        reference: Source::Code(
            "LD r0, [lane]\nSETP.LT p0, r0, 300\nIF p0\n  ADD r0, r0, 100\nELSE\n  SETP.LT p1, r0, 700\n  IF p1\n    SHL r0, r0, 1\n  ELSE\n    SUB r0, r0, 100\n  ENDIF\nENDIF\nST [lane], r0\n",
        ),
        predict: Predict {
            question: Tr::new(
                "Lanes land on both sides of an IF/ELSE. The IF part has 3 instructions, the ELSE part has 2. How many instructions does the warp issue for the two parts together?",
                "车道分落在 IF/ELSE 的两侧。IF 部分有 3 条指令，ELSE 部分有 2 条。warp 为这两部分一共要发射多少条指令？",
            ),
            options: [Tr::new("2", "2"), Tr::new("3", "3"), Tr::new("5", "5")],
            answer: 2,
            why: Tr::new(
                "The warp has one instruction stream. It runs the IF part with one group of lanes on, then the ELSE part with the other group on: 3 + 2.",
                "warp 只有一条指令流。它先开着一组车道跑 IF 部分，再开着另一组车道跑 ELSE 部分：3 + 2。",
            ),
        },
        hints: [
            Tr::new(
                "The starter handles lanes below 300 and leaves everybody else untouched. Those other lanes need an ELSE.",
                "初始代码处理了低于 300 的车道，其他车道原样不动。这些车道需要一个 ELSE。",
            ),
            Tr::new(
                "ELSE flips the mask to the lanes that did not take the IF. Inside it you can test again, with a second predicate and a second IF/ELSE.",
                "ELSE 把掩码翻转给没进 IF 的车道。在它里面可以再次判断，用第二个谓词和第二层 IF/ELSE。",
            ),
            Tr::new(
                "IF p0 {+100} ELSE { SETP.LT p1, r0, 700 / IF p1 {SHL r0, r0, 1} ELSE {SUB r0, r0, 100} ENDIF } ENDIF, then one ST at the end.",
                "IF p0 {+100} ELSE { SETP.LT p1, r0, 700 / IF p1 {SHL r0, r0, 1} ELSE {SUB r0, r0, 100} ENDIF } ENDIF，最后只写一次 ST。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Divergence costs both sides", "分岔要付两边的钱"),
            line: Tr::new(
                "When lanes disagree, the warp runs each side in turn. The cost is the sum of the sides, not the larger.",
                "车道意见不一时，warp 把每一边依次跑一遍。代价是各边之和，而不是较大的那一边。",
            ),
            reality: Tr::new(
                "This is warp divergence, the best known way to lose GPU performance. Nested ifs multiply the effect.",
                "这就是 warp 分歧，也是损失 GPU 性能最广为人知的方式。嵌套的 if 会让效果成倍放大。",
            ),
            myth: Tr::new(
                "Myth: an if/else runs only the side that is true. For a warp that has lanes on both sides, both run.",
                "误解：if/else 只执行为真的那一边。对于两边都有车道的 warp，两边都会执行。",
            ),
            simplified: Tr::new(
                "Real hardware reconverges lanes with a stack or per-thread program counters; OC-1 always rejoins at ENDIF.",
                "真实硬件用栈或每线程的程序计数器让车道重新汇合；OC-1 一律在 ENDIF 处汇合。",
            ),
        },
        ..base("the-else", 2, SCALAR_OPS)
    }
}

/// rom[0..32] scores, rom[32..64] flags (0 or 1).
fn flagged(seed: u64) -> Vec<i32> {
    let mut rom = values(seed, 32, 1, 500);
    rom.extend(values(seed + 1, 32, 0, 2));
    rom
}

fn the_mask() -> Task {
    Task {
        title: Tr::new("The Mask", "掩码"),
        brief: Tr::new(
            "The scoreboard in out[0..32] already shows rom[0..32]. Flagged players earned a double.\nrom[32 + lane] is 1 for a flagged player, 0 otherwise.\nDouble only the flagged scores; every other score must stay exactly as it is.",
            "out[0..32] 里的记分板已经显示着 rom[0..32]。被标记的玩家赢得了双倍。\nrom[32 + lane] 为 1 表示被标记，否则为 0。\n只把被标记的分数翻倍；其余分数必须原样保留。",
        ),
        cases: || rom_cases(flagged, &[111, 112, 113]),
        hidden: || rom_cases(flagged, &[114, 115]),
        init: |c| c.rom[..32].to_vec(),
        expect: |c| {
            lanes(|l| {
                if c.rom[32 + l] != 0 {
                    c.rom[l] * 2
                } else {
                    c.rom[l]
                }
            })
        },
        goals: [Goal::new(Metric::Cycles, 8), Goal::new(Metric::Size, 8)],
        starter: Source::Code(
            "; everyone gets doubled — too generous | 人人都翻倍——太大方了\nLD r1, [lane]\nSHL r1, r1, 1\nST [lane], r1\n",
        ),
        reference: Source::Code(
            "ADD r0, lane, 32\nLD r0, [r0]\nSETP.NE p0, r0, 0\nIF p0\n  LD r1, [lane]\n  SHL r1, r1, 1\n  ST [lane], r1\nENDIF\n",
        ),
        predict: Predict {
            question: Tr::new(
                "Inside IF p0, only flagged lanes are active. What do the unflagged lanes do meanwhile?",
                "在 IF p0 里，只有被标记的车道处于活动状态。没被标记的车道这时在做什么？",
            ),
            options: [
                Tr::new("Skip ahead to ENDIF", "直接跳到 ENDIF"),
                Tr::new("Sit masked off, waiting", "被屏蔽，原地等待"),
                Tr::new("Run the same code on garbage", "用垃圾数据跑同样的代码"),
            ],
            answer: 1,
            why: Tr::new(
                "They share one instruction stream, so they cannot leave. The active mask switches their writes off until ENDIF.",
                "它们共用一条指令流，所以走不开。活动掩码会关掉它们的写入，直到 ENDIF。",
            ),
        },
        hints: [
            Tr::new(
                "Watch the Lanes view: which cells should have stayed the same?",
                "看车道视图：哪些格子本应保持不变？",
            ),
            Tr::new(
                "SETP sets a predicate per lane; IF p0 masks off the lanes where it is false. Stores from masked lanes never happen.",
                "SETP 为每条车道设置一个谓词；IF p0 会屏蔽谓词为假的车道。被屏蔽车道的写入根本不会发生。",
            ),
            Tr::new(
                "Load the flag from [lane + 32], SETP.NE p0 against 0, then wrap the starter's three lines in IF p0 … ENDIF.",
                "从 [lane + 32] 读出标记，SETP.NE p0 与 0 比较，然后用 IF p0 … ENDIF 包住初始代码的三行。",
            ),
        ],
        insight: Insight {
            title: Tr::new("The active mask", "活动掩码"),
            line: Tr::new(
                "An IF in a warp does not jump. It switches lanes off, and the warp keeps marching.",
                "warp 里的 IF 并不跳转。它只是关掉一些车道，warp 继续前进。",
            ),
            reality: Tr::new(
                "Every GPU keeps a per-warp execution mask in hardware; predicated stores are how kernels handle ragged edges.",
                "每块 GPU 都在硬件里为每个 warp 保存执行掩码；带谓词的写入正是内核处理不规则边缘的方式。",
            ),
            myth: Tr::new(
                "Myth: an IF that skips most lanes saves time. The warp still spends the cycles — just with fewer lanes busy.",
                "误解：跳过大多数车道的 IF 能省时间。warp 照样要花那些周期——只是忙碌的车道少了。",
            ),
            simplified: Tr::new(
                "OC-1 skips a branch only when no lane takes it; real GPUs do the same, plus finer tricks we leave out.",
                "OC-1 只有在没有车道进入分支时才跳过它；真实 GPU 也这么做，还有我们省略掉的更精细的技巧。",
            ),
        },
        ..base("the-mask", 2, SCALAR_OPS)
    }
}

fn if_or_sel() -> Task {
    Task {
        title: Tr::new("If or Select", "分支还是挑选"),
        brief: Tr::new(
            "A turnstile counts people: a reading below 500 means someone walked in (+1), otherwise someone walked out (−1). Write the new count to out[lane].\nThe old program branches with IF/ELSE and is correct, but it spends 8 cycles. The turnstile allows 6.\nWhen both answers are cheap, compute both and keep one.",
            "旋转门在数人：读数低于 500 表示有人进来（+1），否则有人出去（−1）。把新的计数写入 out[lane]。\n旧程序用 IF/ELSE 分支，结果是对的，但花了 8 个周期。旋转门只允许 6 个。\n当两个答案都很便宜时，两个都算出来，留下一个。",
        ),
        cases: || rom_cases(|s| values(s, 32, 0, 1000), &[211, 212, 213]),
        hidden: || rom_cases(|s| values(s, 32, 0, 1000), &[214, 215]),
        expect: |c| {
            lanes(|l| {
                if c.rom[l] < 500 {
                    c.rom[l] + 1
                } else {
                    c.rom[l] - 1
                }
            })
        },
        limit: Some(Goal::new(Metric::Cycles, 6)),
        goals: [Goal::new(Metric::Cycles, 6), Goal::new(Metric::Size, 6)],
        starter: Source::Code(
            "LD r0, [lane]\nSETP.LT p0, r0, 500\nIF p0\n  ADD r0, r0, 1\nELSE\n  SUB r0, r0, 1\nENDIF\nST [lane], r0\n",
        ),
        reference: Source::Code(
            "LD r0, [lane]\nSETP.LT p0, r0, 500\nADD r1, r0, 1\nSUB r2, r0, 1\nSEL r0, p0, r1, r2\nST [lane], r0\n",
        ),
        predict: Predict {
            question: Tr::new(
                "Compared with the IF/ELSE, which instructions does the SEL version drop?",
                "与 IF/ELSE 相比，SEL 版本去掉了哪些指令？",
            ),
            options: [
                Tr::new("The ADD and the SUB", "ADD 和 SUB"),
                Tr::new(
                    "IF, ELSE and ENDIF — both sums still run",
                    "IF、ELSE 和 ENDIF——两个算式照样都跑",
                ),
                Tr::new("The SETP", "SETP"),
            ],
            answer: 1,
            why: Tr::new(
                "With lanes on both sides the warp ran the ADD and the SUB anyway. SEL keeps both and replaces three bookkeeping instructions with one.",
                "两边都有车道时，warp 本来就把 ADD 和 SUB 都跑了。SEL 保留了两者，只是用一条指令换掉了三条管理指令。",
            ),
        },
        hints: [
            Tr::new(
                "Count what the IF, the ELSE and the ENDIF do for the answer. They compute nothing.",
                "数一数 IF、ELSE 和 ENDIF 为答案做了什么。它们什么也没算。",
            ),
            Tr::new(
                "Compute both candidates into two registers, with no branch at all, and let SEL pick per lane with the predicate.",
                "完全不要分支，把两个候选值分别算进两个寄存器，再用 SEL 按谓词为每条车道挑一个。",
            ),
            Tr::new(
                "LD r0 / SETP.LT p0, r0, 500 / ADD r1, r0, 1 / SUB r2, r0, 1 / SEL r0, p0, r1, r2 / ST [lane], r0.",
                "LD r0 / SETP.LT p0, r0, 500 / ADD r1, r0, 1 / SUB r2, r0, 1 / SEL r0, p0, r1, r2 / ST [lane], r0。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Predication", "谓词化"),
            line: Tr::new(
                "For short branches, computing both sides and selecting skips the mask bookkeeping, and nothing diverges.",
                "对于很短的分支，两边都算再挑选，省去了掩码的管理开销，也不会有任何分歧。",
            ),
            reality: Tr::new(
                "GPU compilers turn small if/else blocks into predicated or select instructions on their own.",
                "GPU 编译器会自己把小的 if/else 块改写成带谓词的或者挑选类的指令。",
            ),
            myth: Tr::new(
                "Myth: branches are always the cheap way to choose. For a warp that is split anyway, a select is cheaper.",
                "误解：分支永远是最省的选择方式。对一个反正会被劈开的 warp 来说，挑选更省。",
            ),
            simplified: Tr::new(
                "It stops paying off when the branches are long: you would run both of them for every lane.",
                "分支一长就不划算了：你得让每条车道把两条都跑一遍。",
            ),
        },
        ..base("if-or-sel", 3, SCALAR_OPS)
    }
}

fn collatz(v: i32) -> i32 {
    if v & 1 == 1 { 3 * v + 1 } else { v >> 1 }
}

fn the_fork() -> Task {
    Task {
        title: Tr::new("The Fork", "岔路"),
        brief: Tr::new(
            "The Collatz job takes one step for each of 32 numbers in rom[0..32].\nOdd numbers become 3n + 1, even numbers become n / 2. Results go to out[lane].\nHalf the board stays dark. The arithmetic is right — something else is not.",
            "考拉兹任务要对 rom[0..32] 中的 32 个数各走一步。\n奇数变成 3n + 1，偶数变成 n / 2。结果写入 out[lane]。\n记分板有一半是黑的。算术没错——错的是别的地方。",
        ),
        cases: || rom_cases(|s| values(s, 32, 1, 10_000), &[121, 122, 123]),
        hidden: || rom_cases(|s| values(s, 32, 1, 10_000), &[124, 125]),
        expect: |c| lanes(|l| collatz(c.rom[l])),
        goals: [Goal::new(Metric::Cycles, 10), Goal::new(Metric::Size, 10)],
        starter: Source::Code(
            "LD r0, [lane]\nAND r1, r0, 1\nSETP.NE p0, r1, 0\nIF p0\n  MUL r0, r0, 3\n  ADD r0, r0, 1\nELSE\n  SHR r0, r0, 1\n  ST [lane], r0\nENDIF\n",
        ),
        reference: Source::Code(
            "LD r0, [lane]\nAND r1, r0, 1\nSETP.NE p0, r1, 0\nIF p0\n  MUL r0, r0, 3\n  ADD r0, r0, 1\nELSE\n  SHR r0, r0, 1\nENDIF\nST [lane], r0\n",
        ),
        predict: Predict {
            question: Tr::new(
                "The warp has both odd and even numbers. How many of IF's and ELSE's instructions does it issue?",
                "warp 里既有奇数也有偶数。IF 和 ELSE 两边的指令它会发出多少？",
            ),
            options: [
                Tr::new("Only the odd side", "只有奇数那边"),
                Tr::new("Whichever side has more lanes", "车道多的那一边"),
                Tr::new("Both sides, one after the other", "两边都发，一前一后"),
            ],
            answer: 2,
            why: Tr::new(
                "The odd lanes run first while the even ones wait, then the mask flips. Divergence costs the sum of both paths.",
                "奇数车道先跑，偶数车道等待，然后掩码翻转。分歧的代价是两条路径之和。",
            ),
        },
        hints: [
            Tr::new(
                "Which lanes stay dark: the odd ones or the even ones? Which side of the fork are they on?",
                "黑着的是奇数车道还是偶数车道？它们在岔路的哪一边？",
            ),
            Tr::new(
                "The ST runs only while the ELSE mask is active. Results must be stored after both sides have finished.",
                "ST 只在 ELSE 的掩码有效时执行。结果必须在两边都结束之后再写入。",
            ),
            Tr::new(
                "Move ST [lane], r0 from inside the ELSE to just after ENDIF.",
                "把 ST [lane], r0 从 ELSE 里面移到 ENDIF 之后。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Divergence and reconvergence", "分歧与重聚"),
            line: Tr::new(
                "A warp forks by masking, runs both paths, and rejoins at ENDIF. Work after the join runs for everyone.",
                "warp 靠掩码分叉，两条路都走，在 ENDIF 重新汇合。汇合之后的工作对所有车道生效。",
            ),
            reality: Tr::new(
                "GPU compilers insert reconvergence points just like ENDIF; misplaced work inside a branch is a classic kernel bug.",
                "GPU 编译器会像 ENDIF 一样插入重聚点；把工作错放在分支里，是经典的内核 bug。",
            ),
            myth: Tr::new(
                "Myth: divergent lanes run in parallel on different paths. They take turns, on the same hardware.",
                "误解：分歧的车道在不同路径上并行运行。它们其实是在同一套硬件上轮流执行。",
            ),
            simplified: Tr::new(
                "OC-1 forbids jumps, so every branch reconverges structurally. Real code can be far messier.",
                "OC-1 禁止跳转，所以每个分支都会结构化地重聚。真实代码可能乱得多。",
            ),
        },
        clue: Some(Clue {
            what: Tr::new(
                "the store that issued while half the warp was masked off",
                "半个 warp 被屏蔽时发出的那条写入",
            ),
            find: crate::grade::first_masked_store,
        }),
        ..base("the-fork", 5, SCALAR_OPS)
    }
}

/// rom[0..32] readings; rom[32] is the mode every lane has to follow.
fn moded(seed: u64, mode: i32) -> Case {
    let mut rom = values(seed, 32, 0, 100);
    rom.push(mode);
    Case { rom, frame: 0 }
}

fn the_uniform() -> Task {
    Task {
        title: Tr::new("The Uniform Branch", "一致的分支"),
        brief: Tr::new(
            "The signal tower runs in one of two modes, written in rom[32]. In mode 0 every reading in rom[0..32] goes through six rounds of x → 3x + 1; in mode 1, six rounds of x → 2x + 3. Results go to out[lane].\nThe old program computes both modes for everyone and picks one at the end. Every lane reads the same mode — so only one branch should ever run.",
            "信号塔有两种模式，写在 rom[32]。模式 0 下，rom[0..32] 的每个读数都做六轮 x → 3x + 1；模式 1 下，做六轮 x → 2x + 3。结果写入 out[lane]。\n旧程序为所有人把两种模式都算一遍，最后再挑一个。每条车道读到的模式都一样——所以永远只该跑一个分支。",
        ),
        cases: || vec![moded(221, 0), moded(222, 0), moded(223, 1), moded(224, 1)],
        hidden: || vec![moded(225, 1), moded(226, 0), moded(227, 1)],
        expect: |c| {
            lanes(|l| {
                (0..6).fold(
                    c.rom[l],
                    |x, _| {
                        if c.rom[32] == 0 { 3 * x + 1 } else { 2 * x + 3 }
                    },
                )
            })
        },
        limit: Some(Goal::new(Metric::Cycles, 40)),
        goals: [Goal::new(Metric::Cycles, 26), Goal::new(Metric::Size, 15)],
        starter: Source::Code(concat!(
            "LD r0, [lane]\nLD r1, [32]\nMOV r2, r0\nMOV r3, r0\n",
            "LOOP 6\n  MUL r2, r2, 3\n  ADD r2, r2, 1\nENDLOOP\n",
            "LOOP 6\n  SHL r3, r3, 1\n  ADD r3, r3, 3\nENDLOOP\n",
            "SETP.EQ p0, r1, 0\nSEL r0, p0, r2, r3\nST [lane], r0\n",
        )),
        reference: Source::Code(concat!(
            "LD r0, [lane]\nLD r1, [32]\nSETP.EQ p0, r1, 0\n",
            "IF p0\n  LOOP 6\n    MUL r0, r0, 3\n    ADD r0, r0, 1\n  ENDLOOP\n",
            "ELSE\n  LOOP 6\n    SHL r0, r0, 1\n    ADD r0, r0, 3\n  ENDLOOP\nENDIF\n",
            "ST [lane], r0\n",
        )),
        predict: Predict {
            question: Tr::new(
                "All 32 lanes read the same mode, so the branch condition is the same in every lane. How much of the IF/ELSE does the warp issue?",
                "32 条车道读到的是同一个模式，所以每条车道的分支条件都一样。warp 会发射 IF/ELSE 的多少内容？",
            ),
            options: [
                Tr::new("Both sides", "两边都发射"),
                Tr::new(
                    "Only the side the lanes agree on",
                    "只发射车道一致同意的那一边",
                ),
                Tr::new("Neither side", "两边都不发射"),
            ],
            answer: 1,
            why: Tr::new(
                "A branch costs extra only when lanes disagree. When the whole warp agrees, the other side has no active lane and is skipped.",
                "分支只有在车道意见不一时才有额外代价。整个 warp 一致时，另一边没有活动车道，会被直接跳过。",
            ),
        },
        hints: [
            Tr::new(
                "The starter's SEL forces every lane through both loops. Does any lane ever need the loop it throws away?",
                "初始代码里的 SEL 逼着每条车道跑完两个循环。有哪条车道真的需要它扔掉的那个循环吗？",
            ),
            Tr::new(
                "Branch on the mode instead of selecting on it. If no lane takes a side, the warp skips that side completely.",
                "按模式去分支，而不是按模式去挑选。如果没有车道进入某一边，warp 就整个跳过那一边。",
            ),
            Tr::new(
                "LD r0, LD r1 / SETP.EQ p0, r1, 0 / IF p0 {LOOP 6: MUL r0, r0, 3; ADD r0, r0, 1} ELSE {LOOP 6: SHL r0, r0, 1; ADD r0, r0, 3} ENDIF / ST.",
                "LD r0、LD r1 / SETP.EQ p0, r1, 0 / IF p0 {LOOP 6: MUL r0, r0, 3; ADD r0, r0, 1} ELSE {LOOP 6: SHL r0, r0, 1; ADD r0, r0, 3} ENDIF / ST。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Uniform branches are free", "一致的分支是免费的"),
            line: Tr::new(
                "A branch whose condition is the same across the warp does not split it. Only disagreement costs.",
                "条件在整个 warp 里都相同的分支不会把它劈开。只有意见不一才有代价。",
            ),
            reality: Tr::new(
                "Kernel parameters, block-wide flags and loop bounds are uniform; GPUs run branches on them at full speed.",
                "内核参数、整个线程块共有的标志以及循环次数都是一致的；GPU 在它们上面的分支跑得和平常一样快。",
            ),
            myth: Tr::new(
                "Myth: every if on a GPU is slow. A uniform if is as cheap as on any processor.",
                "误解：GPU 上的每个 if 都很慢。一致的 if 和在任何处理器上一样便宜。",
            ),
            simplified: Tr::new(
                "Real hardware must detect uniformity at run time, which OC-1 does for free by looking at the mask.",
                "真实硬件需要在运行时检测一致性；OC-1 看一眼掩码就白送了。",
            ),
        },
        ..base("the-uniform", 5, SCALAR_OPS)
    }
}

/// Collatz steps until the value reaches 1.
fn escape(mut v: i32) -> i32 {
    let mut n = 0;
    while v != 1 {
        v = collatz(v);
        n += 1;
    }
    n
}

fn escape_time() -> Task {
    Task {
        title: Tr::new("Escape Time", "逃逸时间"),
        brief: Tr::new(
            "Each lane holds a number from 1 to 9 in rom[lane]. Count how many Collatz steps it takes to reach 1 (odd → 3n + 1, even → n / 2) and write the count to out[lane]. A number already at 1 takes 0 steps.\nNo number up here needs more than 20. The old program counts all 20 rounds in every lane — including the rounds after a lane has arrived.",
            "每条车道在 rom[lane] 里有一个 1 到 9 的数。数一数它要走多少步考拉兹变换才到 1（奇数 → 3n + 1，偶数 → n / 2），把步数写入 out[lane]。已经是 1 的数要走 0 步。\n这里没有哪个数需要超过 20 步。旧程序让每条车道把 20 轮全数了一遍——包括到达之后的那些轮。",
        ),
        cases: || rom_cases(|s| values(s, 32, 1, 10), &[231, 232, 233]),
        hidden: || {
            let slow = |n: usize| {
                let mut v = vec![2; 32];
                v[n] = 9;
                Case { rom: v, frame: 0 }
            };
            vec![
                slow(13),
                Case {
                    rom: vec![9; 32],
                    frame: 0,
                },
                Case {
                    rom: vec![1; 32],
                    frame: 0,
                },
            ]
        },
        expect: |c| lanes(|l| escape(c.rom[l])),
        goals: [Goal::new(Metric::Cycles, 237), Goal::new(Metric::Size, 17)],
        starter: Source::Code(concat!(
            "LD r0, [lane]\nMOV r1, 0\nLOOP 20\n",
            "  AND r2, r0, 1\n  SETP.EQ p1, r2, 0\n  IF p1\n    SHR r0, r0, 1\n  ELSE\n    MUL r0, r0, 3\n    ADD r0, r0, 1\n  ENDIF\n",
            "  ADD r1, r1, 1\nENDLOOP\nST [lane], r1\n",
        )),
        reference: Source::Code(concat!(
            "LD r0, [lane]\nMOV r1, 0\nLOOP 20\n",
            "  SETP.NE p0, r0, 1\n  IF p0\n",
            "    AND r2, r0, 1\n    SETP.EQ p1, r2, 0\n    IF p1\n      SHR r0, r0, 1\n    ELSE\n      MUL r0, r0, 3\n      ADD r0, r0, 1\n    ENDIF\n",
            "    ADD r1, r1, 1\n  ENDIF\nENDLOOP\nST [lane], r1\n",
        )),
        predict: Predict {
            question: Tr::new(
                "Lane 0 reaches 1 after 3 steps, but the loop runs 20 rounds for the whole warp. What does lane 0 do in the other 17 rounds?",
                "0 号车道走了 3 步就到了 1，可是循环对整个 warp 要跑 20 轮。0 号车道在其余 17 轮里做什么？",
            ),
            options: [
                Tr::new("It leaves the loop early", "提前离开循环"),
                Tr::new("It sits masked off, waiting", "被屏蔽，原地等待"),
                Tr::new("It keeps counting", "继续数下去"),
            ],
            answer: 1,
            why: Tr::new(
                "The loop belongs to the warp, not to a lane. A lane that has finished can only be masked off while the slowest lane keeps going.",
                "循环属于整个 warp，而不属于某条车道。完成的车道只能被屏蔽，等最慢的那一条继续走完。",
            ),
        },
        hints: [
            Tr::new(
                "Once a lane has reached 1 it should stop counting — but a lane cannot leave the loop on its own.",
                "一条车道到达 1 之后就该停止计数——可是车道自己没法离开循环。",
            ),
            Tr::new(
                "Inside the loop, test r0 against 1 and put both the step and the count inside an IF on that test. Finished lanes are masked off.",
                "在循环里拿 r0 与 1 比较，把变换和计数都放进以该比较为条件的 IF 里。已完成的车道会被屏蔽。",
            ),
            Tr::new(
                "LOOP 20 { SETP.NE p0, r0, 1 / IF p0 { the even/odd IF-ELSE, then ADD r1, r1, 1 } ENDIF } ENDLOOP, then ST [lane], r1.",
                "LOOP 20 { SETP.NE p0, r0, 1 / IF p0 { 奇偶的 IF-ELSE，然后 ADD r1, r1, 1 } ENDIF } ENDLOOP，最后 ST [lane], r1。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Waiting for the slowest", "等最慢的那一个"),
            line: Tr::new(
                "A loop with a different trip count per lane runs as long as its longest lane. Early finishers are masked, not freed.",
                "每条车道循环次数不同的循环，会一直跑到最长的那条车道结束。早完成的车道只是被屏蔽，并没有被释放。",
            ),
            reality: Tr::new(
                "Fractal escape times, ray marching and iterative solvers all have this shape; one slow pixel holds up its whole warp.",
                "分形的逃逸时间、光线步进和迭代求解器都是这种形状：一个慢像素会拖住它整个 warp。",
            ),
            myth: Tr::new(
                "Myth: threads that finish early let the others run faster. They just wait.",
                "误解：早完成的线程会让其他线程跑得更快。它们只是在等。",
            ),
            simplified: Tr::new(
                "Real GPUs may retire a warp whose lanes are all done; one live lane keeps it running.",
                "真实 GPU 可以在 warp 的所有车道都完成后将其撤下；只要还有一条车道活着，它就得继续跑。",
            ),
        },
        ..base("escape-time", 6, SCALAR_OPS)
    }
}

/// `flagged`, with lane 0 and lane 31 forced to the given flags.
fn ends(seed: u64, first: i32, last: i32) -> Case {
    let mut rom = flagged(seed);
    (rom[32], rom[63]) = (first, last);
    Case { rom, frame: 0 }
}

fn ghost_lane() -> Task {
    Task {
        title: Tr::new("The Ghost Lane", "幽灵车道"),
        brief: Tr::new(
            "Fault report: the relay ring has 32 posts. A flagged post (rom[32 + lane] = 1) reports its own reading plus its next neighbour's, but only if that neighbour is flagged too; otherwise just its own. Unflagged posts report nothing: their out cell stays 0.\nThe relay program adds a reading that should not be there. Find who is lending it.",
            "故障报告：中继环有 32 个岗哨。被标记的岗哨（rom[32 + lane] = 1）报告自己的读数，加上下一个邻居的读数——但只有邻居也被标记时才加；否则只报自己的。没被标记的岗哨什么也不报：它们的 out 格保持 0。\n中继程序加进了一个不该出现的读数。查一查是谁借给它的。",
        ),
        cases: || rom_cases(flagged, &[241, 242, 243]),
        hidden: || vec![ends(244, 0, 1), ends(245, 1, 1), ends(246, 1, 0)],
        expect: |c| {
            let on = |l: usize| c.rom[32 + l % 32] != 0;
            lanes(|l| match (on(l), on(l + 1)) {
                (false, _) => 0,
                (true, false) => c.rom[l],
                (true, true) => c.rom[l] + c.rom[(l + 1) % 32],
            })
        },
        goals: [Goal::new(Metric::Cycles, 12), Goal::new(Metric::Size, 12)],
        starter: Source::Code(concat!(
            "ADD r5, lane, 32\nLD r4, [r5]\nSETP.NE p0, r4, 0\nLD r0, [lane]\n",
            "IF p0\n  ADD r2, lane, 1\n  SHFL r1, r0, r2\n  ADD r0, r0, r1\n  ST [lane], r0\nENDIF\n",
        )),
        reference: Source::Code(concat!(
            "ADD r5, lane, 32\nLD r4, [r5]\nLD r0, [lane]\n",
            "ADD r2, lane, 1\nSHFL r1, r0, r2\nSHFL r3, r4, r2\nMUL r1, r1, r3\nADD r0, r0, r1\n",
            "SETP.NE p0, r4, 0\nIF p0\n  ST [lane], r0\nENDIF\n",
        )),
        predict: Predict {
            question: Tr::new(
                "Inside IF p0 an unflagged neighbour is masked off. A lane asks it for r0 with SHFL. What does it get?",
                "在 IF p0 里，一个没被标记的邻居被屏蔽了。有一条车道用 SHFL 向它要 r0。会拿到什么？",
            ),
            options: [
                Tr::new("Zero, because that lane is off", "零，因为那条车道关着"),
                Tr::new(
                    "The value it still holds in its register",
                    "它寄存器里仍然留着的值",
                ),
                Tr::new("An error", "一个错误"),
            ],
            answer: 1,
            why: Tr::new(
                "The mask switches off a lane's writes, not its registers. SHFL reads them anyway, so a masked-off lane hands over a ghost of its old value.",
                "掩码关掉的是车道的写入，而不是它的寄存器。SHFL 照样能读到，所以被屏蔽的车道会交出旧值的幽灵。",
            ),
        },
        hints: [
            Tr::new(
                "Which lanes does the starter's SHFL ask? Are they all lanes that the IF has kept switched on?",
                "初始代码里的 SHFL 向哪些车道要数据？这些车道都是被 IF 保持开着的吗？",
            ),
            Tr::new(
                "Shuffle both the reading and the neighbour's flag, outside the IF where everybody is awake. Then keep the neighbour's reading only if its flag is 1.",
                "在 IF 之外、人人都醒着的地方，同时洗牌读数和邻居的标记。然后只有邻居的标记为 1 时才保留它的读数。",
            ),
            Tr::new(
                "SHFL r1, r0, r2 and SHFL r3, r4, r2, then MUL r1, r1, r3 (flags are 0 or 1) / ADD r0, r0, r1; only the ST goes inside IF p0.",
                "SHFL r1, r0, r2 和 SHFL r3, r4, r2，然后 MUL r1, r1, r3（标记只有 0 或 1）/ ADD r0, r0, r1；只有 ST 放进 IF p0。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Masked lanes still hold values", "被屏蔽的车道仍有值"),
            line: Tr::new(
                "A mask stops a lane from writing, not from being read. Shuffle from a masked lane and you get stale data.",
                "掩码只能阻止车道写入，不能阻止它被读取。从被屏蔽的车道上洗牌，拿到的是过时的数据。",
            ),
            reality: Tr::new(
                "CUDA's shuffle functions take an explicit participation mask because reading from an inactive lane gives undefined results.",
                "CUDA 的洗牌函数需要显式传入参与掩码，因为从不活动的车道读取的结果是未定义的。",
            ),
            myth: Tr::new(
                "Myth: inside an IF, only the active lanes exist. The others are still there, with their old registers.",
                "误解：在 IF 里只有活动车道存在。其余车道仍在那里，带着它们旧的寄存器。",
            ),
            simplified: Tr::new(
                "OC-1 hands you the ghost value; real hardware may return anything at all, which is worse.",
                "OC-1 会把幽灵值交给你；真实硬件可能返回任何东西，那更糟。",
            ),
        },
        clue: Some(Clue {
            what: Tr::new(
                "the first SHFL that read a lane the mask had switched off",
                "第一个读取了被掩码关掉的车道的 SHFL",
            ),
            find: crate::grade::first_ghost_shfl,
        }),
        ..base("ghost-lane", 7, WARP_OPS)
    }
}

/// rom[t] = row·16 + column of particle t.
fn particles(seed: u64) -> Vec<i32> {
    values(seed, FB_LEN, 0, FB_LEN as i32)
}

fn weather(c: &Case) -> Vec<Option<i32>> {
    let mut out = vec![0; FB_LEN];
    for &p in &c.rom {
        let (x, y) = (p & 15, p >> 4);
        let (dx, col) = if y < 8 { (c.frame, 12) } else { (-c.frame, 6) };
        out[(y * 16 + ((x + dx) & 15)) as usize] = col;
    }
    out.into_iter().map(Some).collect()
}

fn weather_machine() -> Task {
    Task {
        title: Tr::new("The Weather Machine", "天气机"),
        brief: Tr::new(
            "256 particles, one per thread: particle tid sits at rom[tid] = row·16 + column.\nWind blows right in rows 0..7 (colour 12) and left in rows 8..15 (colour 6), one column per frame, wrapping.\nPaint each particle where it is now. Last year's kernel argued with itself and dropped frames; this one must finish in ≤ 100 cycles.",
            "256 个粒子，每个线程一个：粒子 tid 位于 rom[tid] = 行·16 + 列。\n0..7 行风向右吹（颜色 12），8..15 行风向左吹（颜色 6），每帧移动一列，循环往复。\n把每个粒子画在它此刻的位置。去年的内核总在和自己争吵，掉了帧；这次必须在 ≤ 100 个周期内画完。",
        ),
        view: View::Screen,
        launch: Launch::SCREEN,
        out_len: FB_LEN,
        cases: || {
            [(161, 0), (162, 3), (163, 9), (164, 30)]
                .map(|(s, f)| Case {
                    rom: particles(s),
                    frame: f,
                })
                .to_vec()
        },
        hidden: || {
            [(165, 5), (166, 17)]
                .map(|(s, f)| Case {
                    rom: particles(s),
                    frame: f,
                })
                .to_vec()
        },
        init: screen_blank,
        expect: weather,
        limit: Some(Goal::new(Metric::Cycles, 100)),
        goals: [Goal::new(Metric::Cycles, 88), Goal::new(Metric::Size, 11)],
        starter: Source::Code("; one particle per thread | 每个线程一个粒子\n"),
        reference: Source::Code(
            "LD r0, [tid]\nAND r1, r0, 15\nSETP.LT p0, r0, 128\nSUB r4, 0, frame\nSEL r4, p0, frame, r4\nADD r1, r1, r4\nAND r1, r1, 15\nSEL r3, p0, 12, 6\nAND r2, r0, 240\nADD r2, r2, r1\nST [r2], r3\n",
        ),
        live: Some(|frame, _| Case {
            rom: particles(161),
            frame,
        }),
        predict: Predict {
            question: Tr::new(
                "Particles are scattered at random, so almost every warp has some in each half. What does the IF cost?",
                "粒子随机散布，几乎每个 warp 都有粒子落在上下两半。这个 IF 代价如何？",
            ),
            options: [
                Tr::new(
                    "Nothing — each lane takes one side",
                    "没有代价——每条车道只走一边",
                ),
                Tr::new("Every warp issues both sides", "每个 warp 两边都要发射"),
                Tr::new(
                    "Only warps near row 8 pay",
                    "只有靠近第 8 行的 warp 付出代价",
                ),
            ],
            answer: 1,
            why: Tr::new(
                "Which half a particle is in has nothing to do with which warp it belongs to, so all eight warps diverge.",
                "粒子在哪一半，和它属于哪个 warp 毫无关系，所以全部八个 warp 都会分歧。",
            ),
        },
        hints: [
            Tr::new(
                "An IF on the row is a fine first draft. Then open the Warp River: count the slots each warp spends inside the IF and the ELSE.",
                "先按行写一个 IF 也没关系。然后打开 Warp River：数一数每个 warp 在 IF 和 ELSE 里各花了多少个发射槽。",
            ),
            Tr::new(
                "Both sides only pick numbers: a step of +frame or −frame, a colour of 12 or 6. SEL picks without diverging.",
                "两边做的只是挑数字：步长是 +frame 还是 −frame，颜色是 12 还是 6。SEL 能在不分歧的情况下挑选。",
            ),
            Tr::new(
                "SUB r4, 0, frame / SEL r4, p0, frame, r4 / ADD r1, r1, r4 / SEL r3, p0, 12, 6. For three stars: row·16 is just r0 AND 240, and the row test is r0 < 128.",
                "SUB r4, 0, frame / SEL r4, p0, frame, r4 / ADD r1, r1, r4 / SEL r3, p0, 12, 6。想拿三星：行·16 就是 r0 AND 240，行测试就是 r0 < 128。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Divergence has a price", "分歧是有代价的"),
            line: Tr::new(
                "When the data decides the branch, every warp pays for both sides. Selection made the choice free.",
                "当数据决定分支时，每个 warp 都要为两边买单。选择指令让这个决定变得免费。",
            ),
            reality: Tr::new(
                "Particle and physics kernels sort or bin their data by behaviour so whole warps agree, or rewrite branches as selects.",
                "粒子和物理内核会按行为对数据排序或分桶，让整个 warp 意见一致，或者把分支改写成选择。",
            ),
            myth: Tr::new(
                "Myth: branches are always bad on GPUs. Uniform branches are cheap — it is disagreement inside a warp that costs.",
                "误解：GPU 上分支总是不好的。统一的分支很便宜——代价来自 warp 内部的分歧。",
            ),
            simplified: Tr::new(
                "When two particles land on one cell, the last store wins. Real GPUs give no ordering guarantee at all.",
                "两个粒子落到同一格时，后写的那个胜出。真实 GPU 对此根本不保证顺序。",
            ),
        },
        ..base("weather-machine", 6, SCALAR_OPS)
    }
}
