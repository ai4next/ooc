//! Chapter 4 · Prism (1996): into three dimensions. Every register result
//! lands three beats after issue and nothing waits for it.

use oo_base::Tr;

use super::{below, rng, stub};
use crate::isa::{Alu, FB_LEN, FB_W, LANES, OpKind};
use crate::task::*;
use crate::vm::{Launch, Profile};

const fn alu(a: Alu) -> OpKind {
    OpKind::Alu(a)
}

/// Lumen's scalar set plus the beat you can waste.
pub(super) const BEAT_OPS: &[OpKind] = &[
    OpKind::Mov,
    alu(Alu::Add),
    alu(Alu::Sub),
    alu(Alu::Mul),
    alu(Alu::And),
    alu(Alu::Or),
    alu(Alu::Shl),
    alu(Alu::Shr),
    alu(Alu::Min),
    alu(Alu::Max),
    OpKind::SetP,
    OpKind::Sel,
    OpKind::If,
    OpKind::Else,
    OpKind::EndIf,
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::Ld,
    OpKind::St,
    OpKind::Pix,
    OpKind::Nop,
];

/// Everything Prism offers, raster unit included.
pub(super) const PRISM_OPS: &[OpKind] = &[
    OpKind::Mov,
    alu(Alu::Add),
    alu(Alu::Sub),
    alu(Alu::Mul),
    alu(Alu::And),
    alu(Alu::Or),
    alu(Alu::Shl),
    alu(Alu::Shr),
    alu(Alu::Min),
    alu(Alu::Max),
    OpKind::SetP,
    OpKind::Sel,
    OpKind::If,
    OpKind::Else,
    OpKind::EndIf,
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::Ld,
    OpKind::St,
    OpKind::Pix,
    OpKind::Nop,
    OpKind::Edge,
];

pub(super) fn base(id: &'static str, slot: u8, view: View, ops: &'static [OpKind]) -> Task {
    let (launch, out_len) = match view {
        View::Lanes => (Launch::ONE_WARP, LANES),
        _ => (Launch::SCREEN, FB_LEN),
    };
    Task {
        profile: Profile::PRISM,
        init: if view == View::Lanes {
            lanes_blank
        } else {
            screen_blank
        },
        ..stub(id, 4, slot, view, Editor::Code(ops), launch, out_len)
    }
}

pub fn tasks() -> Vec<Task> {
    vec![
        three_beats(),
        the_triangle(),
        the_depth(),
        the_turn(),
        the_stale(),
        the_lighthouse(),
    ]
}

pub(super) fn rom_case(rom: Vec<i32>) -> Case {
    Case { rom, frame: 0 }
}

/// The value of the edge (or depth plane) stored at `rom[a..a+3]` at a pixel,
/// exactly as EDGE computes it.
pub(super) fn edge(rom: &[i32], a: usize, x: i32, y: i32) -> i32 {
    rom[a]
        .wrapping_mul(x)
        .wrapping_add(rom[a + 1].wrapping_mul(y))
        .wrapping_add(rom[a + 2])
}

/// Coverage as the raster unit sees it: all three edges strictly on one side.
pub(super) fn covered(rom: &[i32], a: usize, x: i32, y: i32) -> bool {
    let e = [
        edge(rom, a, x, y),
        edge(rom, a + 3, x, y),
        edge(rom, a + 6, x, y),
    ];
    e.iter().all(|&v| v > 0) || e.iter().all(|&v| v < 0)
}

/// Edge coefficients of a triangle given in half-pixel units, so vertices can
/// sit on pixel corners: `e = a·x + b·y + c` with x, y whole pixels.
pub(super) fn edges(v: [(i32, i32); 3]) -> Vec<i32> {
    let mut out = Vec::new();
    for i in 0..3 {
        let (p, q) = (v[i], v[(i + 1) % 3]);
        let (a, b) = (p.1 - q.1, q.0 - p.0);
        // Pixel centre (x, y) is (2x + 1, 2y + 1) in half-pixel units.
        out.extend([2 * a, 2 * b, a + b + p.0 * q.1 - q.0 * p.1]);
    }
    out
}

/// A screen from a per-pixel rule over the case's ROM.
pub(super) fn paint(c: &Case, f: impl Fn(&[i32], i32, i32) -> i32) -> Vec<Option<i32>> {
    (0..FB_LEN as i32)
        .map(|t| Some(f(&c.rom, t % FB_W, t / FB_W)))
        .collect()
}

fn lanes(f: impl Fn(usize) -> i32) -> Vec<Option<i32>> {
    (0..LANES).map(|l| Some(f(l))).collect()
}

/// Four banks of 32 sensor readings.
fn banks(seed: u64) -> Vec<i32> {
    let mut r = rng(seed);
    (0..128).map(|_| below(&mut r, 250)).collect()
}

fn three_beats() -> Task {
    Task {
        title: Tr::new("Three Beats Later", "三拍之后"),
        brief: Tr::new(
            "The keeper's log has four banks of 32 readings: rom[0..32], rom[32..64], rom[64..96], rom[96..128]. out[lane] is the sum of the four readings for that lane.\nOn Prism a result lands three beats after its instruction issues, and nothing waits for it: read a register sooner and you get its old value.\nThe program below was right on Lumen. Make it right on Prism in ≤ 20 cycles.",
            "看守人的日志有四组读数，每组 32 个：rom[0..32]、rom[32..64]、rom[64..96]、rom[96..128]。out[lane] 是这条车道四个读数之和。\n在 Prism 上，指令发出三拍之后结果才落地，而且没有任何东西会等它：提前读寄存器，读到的是旧值。\n下面的程序在 Lumen 上是对的。让它在 Prism 上也对，并且 ≤ 20 个周期。",
        ),
        cases: || [141, 142, 143].map(|s| rom_case(banks(s))).to_vec(),
        hidden: || [144, 145].map(|s| rom_case(banks(s))).to_vec(),
        expect: |c| {
            let r = &c.rom;
            lanes(|l| r[l] + r[32 + l] + r[64 + l] + r[96 + l])
        },
        limit: Some(Goal::new(Metric::Cycles, 20)),
        goals: [Goal::new(Metric::Cycles, 18), Goal::new(Metric::Size, 16)],
        starter: Source::Code(
            "; written for Lumen | 为 Lumen 写的\nLD r0, [lane]\nADD r1, lane, 32\nLD r1, [r1]\nADD r0, r0, r1\nADD r1, lane, 64\nLD r1, [r1]\nADD r0, r0, r1\nADD r1, lane, 96\nLD r1, [r1]\nADD r0, r0, r1\nST [lane], r0\n",
        ),
        reference: Source::Code(
            "ADD r1, lane, 32\nADD r2, lane, 64\nADD r3, lane, 96\nLD r0, [lane]\nLD r1, [r1]\nLD r2, [r2]\nLD r3, [r3]\nADD r0, r0, r1\nNOP\nADD r2, r2, r3\nNOP\nNOP\nADD r0, r0, r2\nNOP\nNOP\nST [lane], r0\n",
        ),
        predict: Predict {
            question: Tr::new(
                "LD r0 issues on beat 1. From which beat can an instruction of the same warp read the loaded r0?",
                "LD r0 在第 1 拍发出。同一个 warp 的指令从第几拍起才能读到加载进来的 r0？",
            ),
            options: [
                Tr::new("Beat 2, the very next one", "第 2 拍，紧接着的下一拍"),
                Tr::new("Beat 4, three beats later", "第 4 拍，三拍之后"),
                Tr::new("Whenever: the warp waits for it", "随时都行：warp 会等它"),
            ],
            answer: 1,
            why: Tr::new(
                "The result lands on beat 4. Prism has no interlock: beats 2 and 3 run anyway and see whatever r0 held before.",
                "结果在第 4 拍落地。Prism 没有互锁：第 2、3 拍照样执行，看到的是 r0 原来的值。",
            ),
        },
        hints: [
            Tr::new(
                "Run it and open the Pipeline tab: red cells are reads that came too early. NOPs would fix them, but each NOP is a cycle.",
                "运行一次，打开“流水线”页：红格是读得太早的地方。NOP 能修好，但每个 NOP 都是一个周期。",
            ),
            Tr::new(
                "Fill the gap with work that does not need the result: the other addresses and the other loads.",
                "用不依赖这个结果的工作去填空档：别的地址、别的加载。",
            ),
            Tr::new(
                "Compute all three addresses first, issue all four loads, then add in pairs: (a + b) + (c + d).",
                "先算出三个地址，再一口气发出四个加载，然后两两相加：(a + b) + (c + d)。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Hiding latency", "隐藏延迟"),
            line: Tr::new(
                "With no interlock, the program has to schedule around every result's latency. Independent work fills the gap for free.",
                "没有互锁，程序就得绕着每个结果的延迟排指令。不相关的工作可以免费填满空档。",
            ),
            reality: Tr::new(
                "Early MIPS chips had a load delay slot, and VLIW DSPs and GPUs exposed latency to the compiler. Instruction schedulers still reorder code to hide it.",
                "早期 MIPS 芯片有加载延迟槽，VLIW 的 DSP 和 GPU 也把延迟交给编译器处理。今天的指令调度器仍在重排代码来隐藏延迟。",
            ),
            myth: Tr::new(
                "Myth: NOPs make it correct, so they are fine. Each NOP is a whole warp doing nothing for a cycle.",
                "误解：NOP 能让结果正确，所以无所谓。每个 NOP 都是整个 warp 空转一个周期。",
            ),
            simplified: Tr::new(
                "Modern GPUs interlock in hardware and hide latency by switching to another warp. Prism shows you the latency they hide.",
                "现代 GPU 在硬件里做互锁，靠切换到别的 warp 来隐藏延迟。Prism 让你看见它们藏起来的东西。",
            ),
        },
        ..base("three-beats", 1, View::Lanes, BEAT_OPS)
    }
}

/// Triangles in half-pixel units (pixel corners are even), with colours.
/// Clockwise, counter-clockwise, a sliver, a degenerate line, then hidden ones.
const TRIANGLES: [([(i32, i32); 3], i32); 6] = [
    ([(4, 4), (28, 8), (10, 28)], 12),
    ([(26, 4), (4, 14), (22, 30)], 14),
    ([(2, 2), (30, 10), (6, 8)], 11),
    ([(4, 4), (16, 16), (28, 28)], 9),
    ([(30, 2), (18, 30), (2, 12)], 10),
    ([(6, 26), (28, 20), (12, 2)], 8),
];

/// rom[0..9] the three edges, rom[9] the colour, rom[10..14] the bounding
/// box in whole pixels (min x, min y, max x, max y) that the old rasteriser used.
fn triangle(k: usize) -> Case {
    let (v, colour) = TRIANGLES[k];
    let mut rom = edges(v);
    rom.push(colour);
    let lo = |f: fn(&(i32, i32)) -> i32| v.iter().map(f).min().unwrap_or(0) / 2;
    let hi = |f: fn(&(i32, i32)) -> i32| (v.iter().map(f).max().unwrap_or(0) - 1) / 2;
    rom.extend([lo(|p| p.0), lo(|p| p.1), hi(|p| p.0), hi(|p| p.1)]);
    rom_case(rom)
}

fn the_triangle() -> Task {
    Task {
        title: Tr::new("The Triangle", "三角形"),
        brief: Tr::new(
            "Prism draws in triangles. A triangle is three edges, and each edge is a line a·x + b·y + c stored in rom: edge 0 at rom[0..3], edge 1 at rom[3..6], edge 2 at rom[6..9].\nEDGE r, [a] evaluates the line at rom[a] for this thread's pixel: positive on one side, negative on the other.\nPaint rom[9] on every pixel strictly inside the triangle and 0 elsewhere. Some triangles wind the other way; one is flat as a line.",
            "Prism 用三角形画画。三角形就是三条边，每条边是一条直线 a·x + b·y + c，存在 rom 里：边 0 在 rom[0..3]，边 1 在 rom[3..6]，边 2 在 rom[6..9]。\nEDGE r, [a] 在本线程的像素处计算 rom[a] 那条直线：一侧为正，另一侧为负。\n把严格位于三角形内部的像素涂成 rom[9]，其余为 0。有的三角形顶点顺序是反的；还有一个扁成了一条线。",
        ),
        cases: || (0..4).map(triangle).collect(),
        hidden: || (4..6).map(triangle).collect(),
        expect: |c| paint(c, |r, x, y| if covered(r, 0, x, y) { r[9] } else { 0 }),
        limit: Some(Goal::new(Metric::Cycles, 160)),
        goals: [Goal::new(Metric::Cycles, 112), Goal::new(Metric::Size, 14)],
        starter: Source::Code(
            "; the old rasteriser: inside the bounding box | 旧光栅器：包围盒以内\nLD r0, [10]\nLD r2, [12]\nLD r1, [11]\nLD r3, [13]\nLD r5, [9]\nSETP.GE p0, x, r0\nSETP.LE p1, x, r2\nSETP.GE p2, y, r1\nSETP.LE p3, y, r3\nIF p0\n  IF p1\n    IF p2\n      IF p3\n        PIX r5\n      ENDIF\n    ENDIF\n  ENDIF\nENDIF\n",
        ),
        reference: Source::Code(
            "EDGE r0, [0]\nEDGE r1, [3]\nEDGE r2, [6]\nLD r5, [9]\nMUL r3, r0, r1\nMUL r4, r1, r2\nNOP\nSETP.GT p0, r3, 0\nIF p0\n  SETP.GT p0, r4, 0\n  IF p0\n    PIX r5\n  ENDIF\nENDIF\n",
        ),
        predict: Predict {
            question: Tr::new(
                "A pixel sits outside the triangle but inside its bounding box. What do the three edges say?",
                "一个像素在三角形外面，却在它的包围盒里面。三条边会怎么说？",
            ),
            options: [
                Tr::new("All three agree", "三条边意见一致"),
                Tr::new(
                    "At least one disagrees with the others",
                    "至少有一条和另外两条不一致",
                ),
                Tr::new("All three are zero", "三条边都是 0"),
            ],
            answer: 1,
            why: Tr::new(
                "Inside means the same side of all three edges. Every corner of the box that the triangle misses lies across at least one edge.",
                "在内部，意味着在三条边的同一侧。包围盒里三角形没盖住的角落，至少跨过了一条边。",
            ),
        },
        hints: [
            Tr::new(
                "Evaluate all three edges. Inside is \"all positive\" for one winding and \"all negative\" for the other.",
                "把三条边都算出来。顶点顺序一种时，内部是“全为正”；反过来时是“全为负”。",
            ),
            Tr::new(
                "Two numbers have the same strict sign exactly when their product is positive. e0·e1 > 0 and e1·e2 > 0 covers both windings.",
                "两个数严格同号，当且仅当乘积为正。e0·e1 > 0 且 e1·e2 > 0 两种顺序都能覆盖。",
            ),
            Tr::new(
                "Issue the three EDGEs back to back so their beats overlap, then nest two IFs around PIX instead of computing a minimum.",
                "把三个 EDGE 背靠背发出去，让它们的等待重叠；然后用两层 IF 包住 PIX，省掉求最小值。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Edge functions", "边函数"),
            line: Tr::new(
                "A triangle is the set of pixels on the same side of three lines. Each pixel asks the question by itself, in parallel.",
                "三角形就是位于三条直线同一侧的像素集合。每个像素独自、并行地问这个问题。",
            ),
            reality: Tr::new(
                "Pineda's 1988 paper described edge functions for parallel rasterisation; every GPU raster unit still evaluates them, in hardware, for tiles of pixels.",
                "Pineda 在 1988 年的论文里提出用边函数做并行光栅化；今天每个 GPU 的光栅单元仍在硬件里对成块的像素计算它们。",
            ),
            myth: Tr::new(
                "Myth: the bounding box is a good enough approximation. It doubles the area of a typical triangle and fills a sliver's whole rectangle.",
                "误解：包围盒近似就够了。它通常让面积翻倍，遇到细长三角形更是把整个矩形都涂满。",
            ),
            simplified: Tr::new(
                "Real rasterisers use a tie-break \"top-left\" rule so shared edges are drawn exactly once. Prism simply leaves pixels on an edge empty.",
                "真实光栅器用“左上”规则处理恰好落在边上的像素，让公共边只画一次。Prism 干脆让边上的像素空着。",
            ),
        },
        ..base("the-triangle", 2, View::Screen, PRISM_OPS)
    }
}

/// Two triangles that pass through each other: vertices (half-pixel units),
/// depth plane `z = a·x + b·y + c` and colour of each.
type Face = ([(i32, i32); 3], [i32; 3], i32);

const PAIRS: [(Face, Face); 6] = [
    (
        ([(2, 4), (28, 2), (16, 30)], [4, 2, 0], 12),
        ([(4, 28), (30, 26), (14, 2)], [-4, -2, 85], 8),
    ),
    (
        ([(2, 2), (30, 6), (4, 30)], [6, 0, 0], 11),
        ([(30, 2), (28, 30), (2, 18)], [-6, 0, 73], 14),
    ),
    (
        ([(4, 6), (30, 14), (8, 30)], [0, 4, 2], 10),
        ([(2, 10), (26, 2), (22, 28)], [2, -4, 41], 12),
    ),
    (
        ([(6, 2), (28, 20), (2, 24)], [4, -2, 40], 9),
        ([(30, 8), (4, 4), (18, 30)], [-2, 4, 41], 11),
    ),
    (
        ([(2, 14), (24, 2), (30, 30)], [-2, -4, 90], 14),
        ([(4, 30), (10, 2), (30, 18)], [2, 4, -1], 10),
    ),
    (
        ([(4, 2), (30, 28), (2, 26)], [2, 6, 0], 8),
        ([(28, 2), (30, 30), (2, 10)], [-4, -2, 93], 12),
    ),
];

/// Face A at rom[0..13] (edges, depth plane, colour), face B at rom[16..29].
fn pair(k: usize) -> Case {
    let mut rom = Vec::new();
    for (i, (v, z, colour)) in [PAIRS[k].0, PAIRS[k].1].into_iter().enumerate() {
        rom.resize(16 * i, 0);
        rom.extend(edges(v));
        rom.extend(z);
        rom.push(colour);
    }
    rom_case(rom)
}

/// The nearer covered face at a pixel (smaller depth wins), or 0.
fn nearest(r: &[i32], x: i32, y: i32) -> i32 {
    [0, 16]
        .into_iter()
        .filter(|&a| covered(r, a, x, y))
        .min_by_key(|&a| edge(r, a + 9, x, y))
        .map_or(0, |a| r[a + 12])
}

fn the_depth() -> Task {
    Task {
        title: Tr::new("Front and Back", "前后"),
        brief: Tr::new(
            "Two faces of the wind-tower model pass through each other. Face A: edges at rom[0..9], depth plane at rom[9..12], colour rom[12]. Face B: the same at rom[16..29].\nA depth plane is a line like an edge: EDGE r, [9] gives face A's depth at this pixel. Smaller is nearer.\nEvery pixel shows the nearest face that covers it, or 0. Drawing A and then B is not enough: each is in front somewhere.",
            "风塔模型的两个面互相穿插。面 A：边在 rom[0..9]，深度平面在 rom[9..12]，颜色 rom[12]。面 B：同样的布局在 rom[16..29]。\n深度平面和边一样是一条直线：EDGE r, [9] 给出面 A 在本像素的深度。越小越近。\n每个像素显示覆盖它的最近的面，没有就是 0。先画 A 再画 B 是不够的：两个面各有一部分在前面。",
        ),
        cases: || (0..4).map(pair).collect(),
        hidden: || (4..6).map(pair).collect(),
        expect: |c| paint(c, nearest),
        limit: Some(Goal::new(Metric::Cycles, 280)),
        goals: [Goal::new(Metric::Cycles, 216), Goal::new(Metric::Size, 27)],
        starter: Source::Code(
            "; painter: A first, then B on top | 画家算法：先画 A，再把 B 盖上去\nEDGE r0, [0]\nEDGE r1, [3]\nEDGE r2, [6]\nEDGE r3, [16]\nEDGE r4, [19]\nMUL r0, r0, r1\nEDGE r5, [22]\nMUL r1, r1, r2\nMUL r3, r3, r4\nMUL r4, r4, r5\nMIN r0, r0, r1\nLD r8, [12]\nMIN r3, r3, r4\nSETP.GT p0, r0, 0\nLD r9, [28]\nSEL r8, p0, r8, 0\nSETP.GT p1, r3, 0\nNOP\nSEL r8, p1, r9, r8\nNOP\nNOP\nPIX r8\n",
        ),
        reference: Source::Code(
            "EDGE r0, [0]\nEDGE r1, [3]\nEDGE r2, [6]\nEDGE r3, [16]\nEDGE r4, [19]\nMUL r0, r0, r1\nEDGE r5, [22]\nMUL r1, r1, r2\nMUL r3, r3, r4\nMUL r4, r4, r5\nMIN r0, r0, r1\nEDGE r6, [9]\nLD r8, [12]\nMIN r3, r3, r4\nSETP.GT p0, r0, 0\nEDGE r7, [25]\nSEL r6, p0, r6, 9999\nLD r9, [28]\nSEL r8, p0, r8, 0\nSETP.GT p1, r3, 0\nIF p1\n  SETP.LT p2, r7, r6\n  SEL r8, p2, r9, r8\nENDIF\nNOP\nNOP\nPIX r8\n",
        ),
        predict: Predict {
            question: Tr::new(
                "Face B is drawn after face A. Where A is nearer, what does the painter's starter show?",
                "面 B 在面 A 之后画。在 A 更近的地方，画家算法的初始程序会显示什么？",
            ),
            options: [
                Tr::new("A, because it is nearer", "A，因为它更近"),
                Tr::new("B, because it came last", "B，因为它最后画"),
                Tr::new("A mix of both", "两者混在一起"),
            ],
            answer: 1,
            why: Tr::new(
                "Draw order knows nothing about distance. Faces that pass through each other have no order that is right everywhere.",
                "绘制顺序对距离一无所知。互相穿插的面，不存在一个处处正确的先后顺序。",
            ),
        },
        hints: [
            Tr::new(
                "Find coverage for both faces first, as in the last level. Then ask the depth question per pixel, not per face.",
                "先像上一关那样算出两个面的覆盖。然后按像素问深度问题，而不是按面。",
            ),
            Tr::new(
                "Give an uncovered face a depth farther than anything: SEL r6, p0, r6, 9999. Then the nearer depth decides the colour.",
                "没覆盖的面给它一个比什么都远的深度：SEL r6, p0, r6, 9999。之后由更近的深度决定颜色。",
            ),
            Tr::new(
                "Where B covers: SETP.LT p2, depthB, depthA / SEL colour, p2, colourB, colourA. Interleave the two faces' work so each result has three beats to land.",
                "在 B 覆盖的地方：SETP.LT p2, 深度B, 深度A / SEL 颜色, p2, 颜色B, 颜色A。把两个面的工作交错排开，让每个结果都有三拍时间落地。",
            ),
        ],
        insight: Insight {
            title: Tr::new("The depth buffer", "深度缓冲"),
            line: Tr::new(
                "Visibility is a per-pixel question. Keep the nearest depth seen so far and order stops mattering.",
                "可见性是逐像素的问题。记住目前见过的最近深度，绘制顺序就不再重要。",
            ),
            reality: Tr::new(
                "Ed Catmull described the Z-buffer in 1974; by the late 1990s every consumer 3D card had one in hardware, with early-Z rejecting hidden pixels before shading.",
                "Ed Catmull 在 1974 年提出了 Z 缓冲；到 1990 年代末，每块消费级 3D 卡都在硬件里有它，还能在着色前用 early-Z 丢掉看不见的像素。",
            ),
            myth: Tr::new(
                "Myth: sort the faces back to front and the painter's algorithm works. Not when faces intersect or overlap in a cycle.",
                "误解：把面从后往前排好序，画家算法就能用。面一旦相交或循环遮挡，就不行了。",
            ),
            simplified: Tr::new(
                "Here both faces live in one kernel, so the buffer is just a register. Real GPUs keep depth in memory and test each fragment against it.",
                "这里两个面在同一个内核里，深度缓冲只是一个寄存器。真实 GPU 把深度放在显存里，每个片元都要和它比较。",
            ),
        },
        ..base("the-depth", 3, View::Screen, PRISM_OPS)
    }
}

/// The wind tower's 32 outline vertices: four rings around the tower's axis
/// at x = 8, as (x, depth z).
fn tower() -> [(i32, i32); 32] {
    let mut v = [(0, 0); 32];
    for (k, p) in v.iter_mut().enumerate() {
        let (ring, step) = (k / 8, k % 8);
        let a = std::f64::consts::TAU * step as f64 / 8.0 + ring as f64 * 0.4;
        let r = [6.0, 5.0, 4.0, 3.0][ring];
        *p = (
            8 + (r * a.cos()).round() as i32,
            (r * a.sin()).round() as i32,
        );
    }
    v
}

/// Where a wind tower turned by `deg` degrees projects each vertex on screen.
/// rom[0..32] x, rom[32..64] z, rom[64] cos·64, rom[65] sin·64, rom[66] the
/// angle, rom[96..117] the divide table 1024 / (16 + z') for z' = −10..10.
fn turned(deg: i32) -> Case {
    let a = (deg as f64).to_radians();
    let v = tower();
    let mut rom: Vec<i32> = v.iter().map(|p| p.0).collect();
    rom.extend(v.iter().map(|p| p.1));
    rom.extend([
        (64.0 * a.cos()).round() as i32,
        (64.0 * a.sin()).round() as i32,
        deg,
    ]);
    rom.resize(96, 0);
    rom.extend((-10..=10).map(|z| 1024 / (16 + z)));
    rom_case(rom)
}

/// Exact projection, focal length and eye distance 16.
fn projected(c: &Case) -> Vec<Option<i32>> {
    let a = (c.rom[66] as f64).to_radians();
    lanes(|l| {
        let (x, z) = ((c.rom[l] - 8) as f64, c.rom[32 + l] as f64);
        let (xr, zr) = (x * a.cos() - z * a.sin(), x * a.sin() + z * a.cos());
        (8.0 + xr * 16.0 / (16.0 + zr)).round() as i32
    })
}

fn the_turn() -> Task {
    Task {
        title: Tr::new("Make It Turn", "转起来"),
        brief: Tr::new(
            "The wind-tower model is 32 vertices, one per lane: x at rom[lane] and depth z at rom[32 + lane]. Its axis stands at x = 8, z = 0.\nTurn it by the angle whose cos·64 and sin·64 are rom[64] and rom[65]: x' = x·cos − z·sin, z' = x·sin + z·cos. Then project: out[lane] = 8 + x'·16 / (16 + z').\nPrism cannot divide. rom[106 + z'] holds 1024 / (16 + z') for z' from −10 to 10. Within 1 pixel counts.",
            "风塔模型有 32 个顶点，每条车道一个：x 在 rom[lane]，深度 z 在 rom[32 + lane]。它的中轴立在 x = 8、z = 0。\n按角度旋转它，cos·64 和 sin·64 分别在 rom[64] 和 rom[65]：x' = x·cos − z·sin，z' = x·sin + z·cos。然后投影：out[lane] = 8 + x'·16 / (16 + z')。\nPrism 不会除法。rom[106 + z'] 存着 1024 / (16 + z')，z' 从 −10 到 10。误差在 1 像素以内都算对。",
        ),
        cases: || [0, 30, 75, 110, 160, 215].map(turned).to_vec(),
        hidden: || [250, 310].map(turned).to_vec(),
        expect: projected,
        tolerance: 1,
        goals: [Goal::new(Metric::Cycles, 40), Goal::new(Metric::Size, 33)],
        starter: Source::Code(
            "; turns, but about the wrong point | 转了，但转错了中心\nLD r0, [lane]\nADD r1, lane, 32\nLD r2, [64]\nLD r3, [65]\nLD r1, [r1]\nMUL r4, r0, r2\nMUL r6, r0, r3\nMUL r7, r1, r2\nMUL r5, r1, r3\nNOP\nADD r6, r6, r7\nSUB r4, r4, r5\nNOP\nSHR r6, r6, 6\nNOP\nNOP\nADD r6, r6, 106\nNOP\nNOP\nLD r6, [r6]\nNOP\nNOP\nMUL r4, r4, r6\nNOP\nNOP\nSHR r4, r4, 12\nNOP\nNOP\nST [lane], r4\n",
        ),
        reference: Source::Code(
            "LD r0, [lane]\nADD r1, lane, 32\nLD r2, [64]\nLD r3, [65]\nLD r1, [r1]\nSUB r0, r0, 8\nNOP\nMUL r7, r1, r2\nMUL r6, r0, r3\nMUL r4, r0, r2\nMUL r5, r1, r3\nADD r6, r6, r7\nNOP\nSUB r4, r4, r5\nSHR r6, r6, 6\nNOP\nNOP\nADD r6, r6, 106\nNOP\nNOP\nLD r6, [r6]\nNOP\nNOP\nMUL r4, r4, r6\nNOP\nNOP\nSHR r4, r4, 12\nNOP\nNOP\nADD r4, r4, 8\nNOP\nNOP\nST [lane], r4\n",
        ),
        predict: Predict {
            question: Tr::new(
                "The starter rotates x as it is stored, 0..15. At angle 0 it is still wrong. Why?",
                "初始程序直接旋转存着的 x（0..15）。角度为 0 时它照样是错的。为什么？",
            ),
            options: [
                Tr::new("The divide table is off by one", "除法表差了一位"),
                Tr::new(
                    "Perspective scales x toward 0, the screen's corner, not the axis",
                    "透视把 x 朝 0 缩放，也就是屏幕的角，而不是中轴",
                ),
                Tr::new("cos 0 is not exactly 64", "cos 0 不正好是 64"),
            ],
            answer: 1,
            why: Tr::new(
                "Turning and scaling both pivot on the origin. Move the axis to the origin first (x − 8), and move it back last (+ 8).",
                "旋转和缩放都以原点为支点。先把中轴移到原点（x − 8），最后再移回去（+ 8）。",
            ),
        },
        hints: [
            Tr::new(
                "Every transform here pivots on 0. Where is 0, and where is the tower's axis?",
                "这里每一步变换都以 0 为支点。0 在哪里？风塔的中轴又在哪里？",
            ),
            Tr::new(
                "Subtract 8 from x right after loading it, and add 8 back just before the store.",
                "x 一加载就减 8，存之前再加回 8。",
            ),
            Tr::new(
                "x'·64 = x·cos − z·sin. z' = (x·sin + z·cos) >> 6. Then x'·64 × rom[106 + z'] >> 12 is x'·16 / (16 + z').",
                "x'·64 = x·cos − z·sin。z' = (x·sin + z·cos) >> 6。于是 x'·64 × rom[106 + z'] >> 12 就是 x'·16 / (16 + z')。",
            ),
        ],
        insight: Insight {
            title: Tr::new("The vertex transform", "顶点变换"),
            line: Tr::new(
                "Move to the origin, turn, push away, divide by depth, move back. One thread per vertex does it all in lock step.",
                "移到原点、旋转、推远、除以深度、移回去。每个顶点一个线程，步调一致地全部做完。",
            ),
            reality: Tr::new(
                "Late-1990s consoles and cards did this per vertex in fixed point, often with reciprocal tables. Hardware transform and lighting arrived on PC cards in 1999.",
                "1990 年代末的游戏机和显卡就是这样逐顶点用定点数算的，常常借助倒数表。PC 显卡在 1999 年才有了硬件变换与光照。",
            ),
            myth: Tr::new(
                "Myth: rotation is just the sin/cos formula. Without moving the pivot first it turns the model about the screen's corner.",
                "误解：旋转就是那个 sin/cos 公式。不先移动支点，模型就会绕着屏幕的角转。",
            ),
            simplified: Tr::new(
                "Real pipelines use 4×4 matrices and floating point, and divide after clipping. Prism keeps one axis and a lookup table.",
                "真实管线用 4×4 矩阵和浮点数，裁剪之后才做除法。Prism 只保留一根轴和一张查找表。",
            ),
        },
        ..base("the-turn", 4, View::Lanes, BEAT_OPS)
    }
}

/// Face of the wind tower model under each pixel (0 = sky): a narrow lamp
/// room on rows 2..4 and the tower on rows 5..15, five facets across each.
fn facet(x: i32, y: i32) -> i32 {
    let w = match y {
        2 | 3 => 3,
        5..=14 => 5,
        _ => return 0,
    };
    if x < 8 - w || x >= 8 + w {
        return 0;
    }
    (x - 8 + w) * 5 / (2 * w) + 1
}

/// rom[0..256] facet per pixel, rom[256..262] facet normal x·64,
/// rom[264..270] normal z·64, rom[272..274] the light, rom[280..288] the
/// colour ramp from unlit to full.
fn lit(light: (i32, i32)) -> Case {
    let mut rom: Vec<i32> = (0..FB_LEN as i32)
        .map(|t| facet(t % FB_W, t / FB_W))
        .collect();
    let deg = [-72.0f64, -36.0, 0.0, 36.0, 72.0];
    for f in [f64::sin, f64::cos] {
        rom.push(0);
        rom.extend(deg.map(|d| (64.0 * f(d.to_radians())).round() as i32));
        rom.extend([0, 0]);
    }
    rom.extend([light.0, light.1]);
    rom.resize(280, 0);
    rom.extend([0, 1, 2, 4, 9, 15, 7, 10]);
    rom_case(rom)
}

fn shade(c: &Case) -> Vec<Option<i32>> {
    paint(c, |r, x, y| {
        let f = r[(y * FB_W + x) as usize] as usize;
        let d = r[256 + f] * r[272] + r[264 + f] * r[273];
        r[280 + (d.max(0) >> 9).min(7) as usize]
    })
}

fn the_stale() -> Task {
    Task {
        title: Tr::new("Black Stripes", "黑纹"),
        brief: Tr::new(
            "The lighting kernel shades each facet by how much it faces the light: brightness = (normal · light) >> 9, from 0 to 7, then a colour from the ramp at rom[280..288].\nIt was scheduled for Prism by hand. Since the last edit, the facets facing the visitors have gone black.\nFind the beat where an instruction read a register before its result had landed. Point at it in the Warp River, then fix it.",
            "光照内核按每个面朝向光的程度着色：亮度 = (法线 · 光) >> 9，取 0 到 7，再到 rom[280..288] 的色带里取颜色。\n它是手工为 Prism 排过的。上次改动之后，朝向参观者的那些面变成了黑的。\n找到那一拍：某条指令在结果落地之前就读了寄存器。先在 Warp River 里指出它，再修好它。",
        ),
        cases: || [(-30, 50), (40, 45), (-55, 25), (20, 60)].map(lit).to_vec(),
        hidden: || [(-10, 64), (50, -20)].map(lit).to_vec(),
        expect: shade,
        goals: [Goal::new(Metric::Cycles, 256), Goal::new(Metric::Size, 32)],
        starter: Source::Code(
            "LD r0, [tid]\nLD r5, [272]\nNOP\nADD r1, r0, 256\nADD r3, r0, 264\nNOP\nLD r2, [r1]\nLD r3, [r3]\n; light z, added in the last edit | 光的 z，上次改动加的\nLD r6, [273]\nMUL r2, r2, r5\nMUL r3, r3, r6\nNOP\nNOP\nADD r2, r2, r3\nNOP\nNOP\nMAX r2, r2, 0\nNOP\nNOP\nSHR r2, r2, 9\nNOP\nNOP\nMIN r2, r2, 7\nNOP\nNOP\nADD r2, r2, 280\nNOP\nNOP\nLD r2, [r2]\nNOP\nNOP\nPIX r2\n",
        ),
        reference: Source::Code(
            "LD r0, [tid]\nLD r5, [272]\nLD r6, [273]\nADD r1, r0, 256\nADD r3, r0, 264\nNOP\nLD r2, [r1]\nLD r3, [r3]\nNOP\nMUL r2, r2, r5\nMUL r3, r3, r6\nNOP\nNOP\nADD r2, r2, r3\nNOP\nNOP\nMAX r2, r2, 0\nNOP\nNOP\nSHR r2, r2, 9\nNOP\nNOP\nMIN r2, r2, 7\nNOP\nNOP\nADD r2, r2, 280\nNOP\nNOP\nLD r2, [r2]\nNOP\nNOP\nPIX r2\n",
        ),
        predict: Predict {
            question: Tr::new(
                "An instruction reads r6 two beats after the load into r6 issued. What does it get?",
                "某条指令在加载 r6 发出两拍之后读 r6。它读到的是什么？",
            ),
            options: [
                Tr::new("The loaded value", "加载进来的值"),
                Tr::new("Whatever r6 held before: here, 0", "r6 原来的值：这里是 0"),
                Tr::new("Half of the loaded value", "加载值的一半"),
            ],
            answer: 1,
            why: Tr::new(
                "The load lands on its third beat. One beat early, r6 still holds its old value, 0, so the light's z part drops out.",
                "加载要到第三拍才落地。早一拍读，r6 还是旧值 0，于是光的 z 分量整个消失了。",
            ),
        },
        hints: [
            Tr::new(
                "The formula is fine. Facets that face you depend most on the light's z part. Which register holds it, and when does it land?",
                "公式没问题。正对你的面最依赖光的 z 分量。它在哪个寄存器里？什么时候落地？",
            ),
            Tr::new(
                "Step through the Pipeline tab: one read of r6 is marked red, two beats after LD r6 issued.",
                "在“流水线”页里逐步看：有一次读 r6 被标成红色，就在 LD r6 发出后的第二拍。",
            ),
            Tr::new(
                "Move LD r6, [273] up into the empty beat after LD r5. Everything else can stay.",
                "把 LD r6, [273] 挪到 LD r5 后面那个空拍里。其他都不用动。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Read after write", "写后读冒险"),
            line: Tr::new(
                "A hazard is a read that comes before the write it depends on. Without an interlock, it fails silently with an old value.",
                "冒险就是读走在它依赖的写前面。没有互锁，它就悄无声息地拿到旧值，不报任何错。",
            ),
            reality: Tr::new(
                "Hazards like this hid in hand-scheduled DSP and console code for years: right on one chip revision, black stripes on the next.",
                "这类冒险在手工排程的 DSP 和游戏机代码里藏了很多年：在这一版芯片上是对的，换一版就出黑纹。",
            ),
            myth: Tr::new(
                "Myth: wrong colours mean wrong lighting maths. Here the maths is right and only the timing is wrong.",
                "误解：颜色错了，就是光照公式错了。这里公式是对的，错的只是时序。",
            ),
            simplified: Tr::new(
                "Real shaders interpolate normals across a face. Prism's facets are flat, one normal each.",
                "真实着色器会在面上插值法线。Prism 的面是平的，每个面一条法线。",
            ),
        },
        clue: Some(Clue {
            what: Tr::new(
                "the beat that read a register too early",
                "读寄存器读得太早的那一拍",
            ),
            find: crate::grade::first_stale_read,
        }),
        ..base("the-stale", 5, View::Screen, BEAT_OPS)
    }
}

/// Half-width of the wind tower on each row in half-pixels (0 = sky): lamp
/// room, gallery, then the tower widening to its foot.
const HALF: [i32; 16] = [0, 7, 7, 7, 12, 9, 9, 10, 10, 10, 11, 11, 12, 12, 13, 13];
/// Light from the well side, in 64ths of a turn.
const SUN: f64 = -6.0;

/// rom[4y..4y+4]: the row's screen-to-surface edge and its colour band;
/// rom[64..97] light per surface column; rom[128..288] screen offset to
/// surface column (100 = sky); rom[300..] the colour bands.
fn lighthouse(turn: i32) -> Case {
    let mut rom = vec![0; 412];
    for (y, &h) in HALF.iter().enumerate() {
        let band = match y {
            1..=3 => 316,
            4 => 332,
            _ => 300,
        };
        let row = match h {
            0 => [0, 0, 128 * 16, 300],
            h => {
                let s = (512.0 / h as f64).round() as i32;
                [2 * s, 0, -15 * s + 208 * 16 + 8, band]
            }
        };
        rom[4 * y..4 * y + 4].copy_from_slice(&row);
    }
    let tau = std::f64::consts::TAU;
    for v in 0..=32 {
        let phi = (v - 16) as f64 * tau / 64.0;
        rom[64 + v as usize] = (1.0 + 6.0 * (phi - SUN * tau / 64.0).cos())
            .round()
            .clamp(0.0, 7.0) as i32;
    }
    rom[100] = 64;
    for (i, cell) in rom.iter_mut().enumerate().take(288).skip(128) {
        let u = i as f64 - 208.0;
        *cell = if u.abs() <= 32.0 {
            80 + ((u / 32.0).asin() * 64.0 / tau).round() as i32
        } else {
            100
        };
    }
    rom[300..348].copy_from_slice(&[
        1, 2, 2, 8, 8, 8, 14, 14, 1, 5, 13, 13, 6, 6, 7, 7, // red and white
        4, 9, 9, 9, 10, 10, 10, 10, 9, 10, 10, 7, 7, 7, 7, 7, // the lamp
        0, 1, 5, 5, 13, 13, 6, 6, 0, 1, 5, 5, 13, 13, 6, 6, // the gallery
    ]);
    rom[364..412].fill(1);
    Case { rom, frame: turn }
}

fn tower_view(c: &Case) -> Vec<Option<i32>> {
    paint(c, |r, x, y| {
        let row = 4 * y as usize;
        let v = r[(edge(r, row, x, y) >> 4) as usize];
        let stripe = (v + c.frame + y) & 8;
        r[(r[row + 3] + stripe + r[v as usize]) as usize]
    })
}

fn the_lighthouse() -> Task {
    Task {
        title: Tr::new("The Turning Wind Tower", "会转的风塔"),
        brief: Tr::new(
            "Huajuan wants the school's Prism to show the wind tower turning, so the children can drag it round and see why the far side goes dark.\nEach row of the tower is a slice of a cylinder. EDGE r, [4y] gives that row's position ×16 into the table at rom[128..288], which turns a screen offset into a surface column v (100 = sky).\nrom[v] is the light on that column, 0 to 7. The paint is red and white bands 8 columns wide that turn with the frame: the band is (v + frame + y) AND 8.\nThe colour is rom[rom[4y+3] + band + light]. Sky cells read colour 1 the same way. Keep it inside a frame of 240 cycles.",
            "花卷想让学校那台 Prism 显示风塔转起来，让孩子们拖着它转，看清背光面为什么会暗。\n塔的每一行是圆柱的一片。EDGE r, [4y] 给出这一行的位置 ×16，它指向 rom[128..288] 的表，把屏幕偏移换成表面列 v（100 表示天空）。\nrom[v] 是这一列的光照，0 到 7。塔身漆着 8 列宽的红白条纹，随帧转动：条纹 = (v + frame + y) AND 8。\n颜色是 rom[rom[4y+3] + 条纹 + 光照]。天空格子用同样的方式读到颜色 1。要在 240 个周期的帧预算之内。",
        ),
        cases: || [0, 3, 6, 9, 12, 14].map(lighthouse).to_vec(),
        hidden: || [5, 11].map(lighthouse).to_vec(),
        expect: tower_view,
        limit: Some(Goal::new(Metric::Cycles, 240)),
        goals: [Goal::new(Metric::Cycles, 208), Goal::new(Metric::Size, 26)],
        starter: Source::Code(
            "; one pixel per thread; results land three beats later | 每个线程一个像素；结果三拍后才落地\n",
        ),
        reference: Source::Code(
            "SHL r9, y, 2\nADD r2, y, frame\nNOP\nEDGE r0, [r9]\nADD r9, r9, 3\nNOP\nSHR r0, r0, 4\nLD r8, [r9]\nNOP\nLD r1, [r0]\nNOP\nNOP\nLD r5, [r1]\nADD r3, r1, r2\nNOP\nADD r5, r5, r8\nAND r3, r3, 8\nNOP\nNOP\nADD r3, r3, r5\nNOP\nNOP\nLD r6, [r3]\nNOP\nNOP\nPIX r6\n",
        ),
        // The children drag it round; it also turns on its own.
        live: Some(|frame, at| lighthouse(frame / 3 + at[0] * 4)),
        steer: Some(Tr::new("TURN THE TOWER", "转动风塔")),
        predict: Predict {
            question: Tr::new(
                "The kernel is a chain of four loads, each needing the last one's result. What sets its length on Prism?",
                "这个内核是一串四次加载，每次都要用上一次的结果。在 Prism 上，什么决定了它有多长？",
            ),
            options: [
                Tr::new("How many instructions it has", "它有多少条指令"),
                Tr::new(
                    "The chain: three beats per link, however much else there is",
                    "那条链：每一环三拍，不管别的指令有多少",
                ),
                Tr::new("How many pixels are lit", "有多少像素被点亮"),
            ],
            answer: 1,
            why: Tr::new(
                "Independent work can hide in the gaps, but a dependent chain can't be shortened by scheduling. Only by making the chain shorter.",
                "互不依赖的指令可以藏进空拍里，但一条依赖链没法靠排程变短。只能把链本身缩短。",
            ),
        },
        hints: [
            Tr::new(
                "Write it in order first, with NOPs wherever a result has not landed yet. Then pull independent work up into those NOPs.",
                "先按顺序写，在结果还没落地的地方都放 NOP。然后把互不依赖的指令往上挪，填进这些 NOP。",
            ),
            Tr::new(
                "The chain is EDGE → SHR → LD v → LD light → ADD → LD colour. The row's band and y + frame don't depend on it: start them early.",
                "链是 EDGE → SHR → LD v → LD 光照 → ADD → LD 颜色。这一行的色带和 y + frame 都不依赖它：早点开始算。",
            ),
            Tr::new(
                "SHL r9, y, 2 / EDGE r0, [r9] / SHR r0, r0, 4 / LD r1, [r0] / LD r5, [r1]. Load the band from [r9 + 3] and add y + frame while waiting.",
                "SHL r9, y, 2 / EDGE r0, [r9] / SHR r0, r0, 4 / LD r1, [r0] / LD r5, [r1]。等待的时候从 [r9 + 3] 读色带，再算 y + frame。",
            ),
        ],
        insight: Insight {
            title: Tr::new("The latency chain", "延迟链"),
            line: Tr::new(
                "A kernel is as long as its longest chain of dependent results. Scheduling fills the gaps around it; only a shorter chain makes it faster.",
                "一个内核有多长，取决于它最长的那条依赖链。排程能填满链周围的空拍，只有更短的链才能让它更快。",
            ),
            reality: Tr::new(
                "Late-90s 3D cards rendered cylinders, skies and sprites through lookup tables exactly like these, because a table read was cheaper than a sine.",
                "九十年代末的 3D 卡就是用这样的查找表画圆柱、天空和精灵的：读一次表比算一次正弦便宜。",
            ),
            myth: Tr::new(
                "Myth: fewer instructions is always faster. On Prism a NOP-filled kernel can beat a shorter one that waits in the wrong places.",
                "误解：指令越少越快。在 Prism 上，塞满 NOP 的内核可能比一个更短、却在错误地方等待的内核还快。",
            ),
            simplified: Tr::new(
                "The tower is drawn as a cylinder from tables. A real renderer would transform and raster every triangle, as in 4-2 to 4-4.",
                "这座塔是用查表画出的圆柱。真正的渲染器会像 4-2 到 4-4 那样变换并光栅化每个三角形。",
            ),
        },
        ..base("the-lighthouse", 6, View::Screen, PRISM_OPS)
    }
}
