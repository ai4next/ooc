//! Chapter 6 · Three Dimensions and Rasterization (Prism).
//!
//! Levels 6-3, 6-4, 6-6 and 6-7. The others (6-1, 6-2, 6-5, 6-8) live in
//! `prism.rs`. All four run on Prism with per-unit latencies: multiply-class
//! results (MUL, DOT, RCP, LERP) land 4 beats after issue, loads and EDGE 3,
//! everything else 2.

use oo_base::Tr;

use super::prism::{base, covered, edge, edges, paint, rom_case};
use super::{below, rng};
use crate::isa::{Alu, LANES, OpKind};
use crate::task::*;
use crate::vm::Profile;

const fn alu(a: Alu) -> OpKind {
    OpKind::Alu(a)
}

/// Prism's full set, with DOT.
const DOT_OPS: &[OpKind] = &[
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
    OpKind::Dot,
];

/// The lane-view scalar set plus the reciprocal unit.
const RCP_LANE_OPS: &[OpKind] = &[
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
    OpKind::Rcp,
];

/// Prism's full set, with LERP.
const LERP_OPS: &[OpKind] = &[
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
    OpKind::Lerp,
];

/// Prism's full set, with the reciprocal and LERP units.
const PERSP_OPS: &[OpKind] = &[
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
    OpKind::Rcp,
    OpKind::Lerp,
];

pub fn tasks() -> Vec<Task> {
    vec![the_winding(), behind_you(), barycentric(), persp_correct()]
}

fn lanes(f: impl Fn(usize) -> i32) -> Vec<Option<i32>> {
    (0..LANES).map(|l| Some(f(l))).collect()
}

/// Twice the signed area: positive for a front-facing triangle.
fn area(v: [(i32, i32); 3]) -> i32 {
    (v[1].0 - v[0].0) * (v[2].1 - v[0].1) - (v[2].0 - v[0].0) * (v[1].1 - v[0].1)
}

/// A triangle (half-pixel vertices) and its colour.
type Tri = ([(i32, i32); 3], i32);

/// Two triangles drawn in order. Triangle k sits at rom[16k..16k + 16]: its
/// edges (flipped so that inside is positive whichever way it winds), its
/// colour, then the vertices x0, y0, x1, y1, x2, y2.
fn winding(t: [Tri; 2]) -> Case {
    let mut rom = Vec::new();
    for (v, colour) in t {
        let flip = if area(v) < 0 { -1 } else { 1 };
        rom.extend(edges(v).into_iter().map(|c| c * flip));
        rom.push(colour);
        rom.extend(v.iter().flat_map(|p| [p.0, p.1]));
    }
    rom_case(rom)
}

const WINDINGS: [[Tri; 2]; 5] = [
    [
        ([(2, 2), (30, 6), (8, 30)], 6),
        ([(10, 6), (14, 28), (26, 12)], 13),
    ],
    [
        ([(2, 4), (20, 2), (10, 26)], 5),
        ([(12, 10), (30, 14), (16, 30)], 11),
    ],
    [
        ([(4, 4), (10, 26), (24, 8)], 7),
        ([(30, 30), (26, 4), (8, 20)], 12),
    ],
    [
        ([(2, 2), (30, 4), (14, 30)], 9),
        ([(4, 4), (16, 16), (28, 28)], 14),
    ],
    [
        ([(6, 6), (14, 30), (28, 10)], 3),
        ([(10, 10), (30, 16), (12, 28)], 10),
    ],
];

/// Front-facing triangles only, the later one on top.
fn facing(c: &Case) -> Vec<Option<i32>> {
    paint(c, |r, x, y| {
        let mut px = 0;
        for a in [0, 16] {
            let v = &r[a + 10..a + 16];
            let signed = (v[2] - v[0]) * (v[5] - v[1]) - (v[4] - v[0]) * (v[3] - v[1]);
            if signed > 0 && covered(r, a, x, y) {
                px = r[a + 9];
            }
        }
        px
    })
}

fn the_winding() -> Task {
    Task {
        title: Tr::new("The Winding", "绕向"),
        brief: Tr::new(
            "Huajuan's cardboard-box model has a back as well as a front, and Prism now draws two triangles per picture, one after the other, the later one on top. Triangle k lives at rom[16k..16k+16]: its three edges at +0..+9, its colour at +9, and its vertices x0, y0, x1, y1, x2, y2 at +10..+16.\nThe setup unit has already flipped each triangle's edges so that inside is positive whichever way the vertices run, so the edges alone cannot tell you which side faces the viewer. A triangle faces the viewer when its signed area (x1-x0)·(y2-y0) - (x2-x0)·(y1-y0) is positive.\nPaint only front-facing triangles, and only where they cover the pixel. Back-facing ones are culled. Everything else is 0.\nNew: DOT rd, ra, rb computes r[a]·r[b] + r[a+1]·r[b+1] in one go.",
            "花卷的纸箱模型有正面也有背面。现在 Prism 每张图画两个三角形，先画的在下，后画的在上。第 k 个三角形存在 rom[16k..16k+16]：三条边在 +0..+9，颜色在 +9，顶点 x0, y0, x1, y1, x2, y2 在 +10..+16。\n设置单元已经把每个三角形的边翻转过了：不管顶点怎么转，内部都是正的。所以只看边，分不出哪一面朝向观察者。有向面积 (x1-x0)·(y2-y0) - (x2-x0)·(y1-y0) 为正，三角形才朝向观察者。\n只涂朝向观察者的三角形，并且只涂它盖住的像素；背面的三角形一律剔除。其余为 0。\n新指令：DOT rd, ra, rb 一次算出 r[a]·r[b] + r[a+1]·r[b+1]。",
        ),
        cases: || WINDINGS[..3].iter().map(|t| winding(*t)).collect(),
        hidden: || WINDINGS[3..].iter().map(|t| winding(*t)).collect(),
        expect: facing,
        goals: [Goal::new(Metric::Cycles, 288), Goal::new(Metric::Size, 32)],
        starter: Source::Code(
            "; draw whatever covers the pixel, front or back | 只要盖住像素就画，不分正反\nEDGE r0, [0]\nEDGE r1, [3]\nEDGE r2, [6]\nEDGE r4, [16]\nEDGE r5, [19]\nEDGE r6, [22]\nLD r3, [9]\nLD r7, [25]\nMIN r0, r0, r1\nMIN r4, r4, r5\nMIN r0, r0, r2\nMIN r4, r4, r6\nSETP.GT p0, r0, 0\nSETP.GT p1, r4, 0\nSEL r3, p0, r3, 0\nNOP\nSEL r3, p1, r7, r3\nNOP\nPIX r3\n",
        ),
        reference: Source::Code(
            "LD r4, [1]\nLD r5, [7]\nLD r6, [6]\nLD r7, [0]\nLD r8, [17]\nLD r9, [23]\nSUB r7, 0, r7\nLD r10, [22]\nDOT r12, r4, r6\nLD r11, [16]\nEDGE r0, [0]\nEDGE r1, [3]\nSUB r11, 0, r11\nEDGE r2, [6]\nDOT r13, r8, r10\nEDGE r4, [16]\nEDGE r5, [19]\nEDGE r6, [22]\nLD r3, [9]\nLD r7, [25]\nMIN r0, r0, r1\nMIN r4, r4, r5\nMIN r0, r0, r2\nMIN r4, r4, r6\nMIN r0, r0, r12\nMIN r4, r4, r13\nSETP.GT p0, r0, 0\nSEL r3, p0, r3, 0\nSETP.GT p1, r4, 0\nSEL r3, p1, r7, r3\nNOP\nPIX r3\n",
        ),
        predict: Predict {
            question: Tr::new(
                "Swap two of a triangle's vertices, so it winds the other way. What happens to its signed area?",
                "把三角形的两个顶点对调，让它的顶点顺序反过来。它的有向面积会怎样？",
            ),
            options: [
                Tr::new("Nothing: area is area", "不变：面积就是面积"),
                Tr::new("It changes sign", "符号翻转"),
                Tr::new("It becomes 0", "变成 0"),
            ],
            answer: 1,
            why: Tr::new(
                "The area is a cross product, and swapping two of its inputs negates it. The size stays the same; only the sign says which way the triangle winds.",
                "有向面积是一个叉积，把两个输入对调就会变号。大小不变，只有符号告诉你三角形朝哪边转。",
            ),
        },
        hints: [
            Tr::new(
                "The edge test is happy with a back-facing triangle: it covers pixels, and being drawn later it hides the front one. Work out each triangle's signed area first and keep the pixel only if the area is positive.",
                "边测试并不嫌弃背面三角形：它一样盖住像素，又因为后画，会把正面的盖掉。先算每个三角形的有向面积，面积为正才留下这个像素。",
            ),
            Tr::new(
                "DOT multiplies two pairs and adds them. With dx1 = x1-x0, dx2 = x0-x2, dy2 = y2-y0, dy1 = y1-y0 the area is DOT((dx1, dx2), (dy2, dy1)). Each pair must sit in consecutive registers. The MIN of the three edges and the area is positive only for covered front faces.",
                "DOT 把两对数相乘再相加。令 dx1 = x1-x0，dx2 = x0-x2，dy2 = y2-y0，dy1 = y1-y0，面积就是 DOT((dx1, dx2), (dy2, dy1))。每一对必须放在连续的寄存器里。三条边和面积四者取最小，只有被盖住的正面才为正。",
            ),
            Tr::new(
                "A program holds at most 40 instructions, NOPs included, and the vertex route costs 11 per triangle. The edge coefficients already are edge vectors, scaled by ±2: b0·a2 - a0·b2 (edges 0 and 2) has the sign of the area, and flipping every sign cancels in a product. Four loads, one SUB and one DOT.",
                "程序最多 40 条指令（NOP 也算），而走顶点这条路每个三角形要 11 条。其实边的系数本身就是边向量，只差一个 ±2 的倍数：边 0 和边 2 的 b0·a2 - a0·b2 与面积同号，而整体翻转符号在乘积里会抵消。四次 LD、一次 SUB、一次 DOT。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Culling by winding", "按绕向剔除"),
            line: Tr::new(
                "Which way a triangle winds on screen says whether it faces you. One signed area per triangle tells you, before a single pixel is touched.",
                "三角形在屏幕上朝哪个方向绕，就说明它是否朝向你。每个三角形算一次有向面积，不必碰任何像素就能知道。",
            ),
            reality: Tr::new(
                "Triangle setup computes exactly this determinant. Back-face culling throws away about half the triangles of a closed mesh before rasterisation, and the same determinant normalises the edge functions.",
                "三角形设置阶段算的正是这个行列式。背面剔除能在光栅化之前丢掉封闭网格里大约一半的三角形，同一个行列式还用来归一化边函数。",
            ),
            myth: Tr::new(
                "Myth: draw the back faces anyway, the front ones will hide them. Not when the back face is drawn later, as here, and never for free: every hidden pixel is still shaded.",
                "误解：背面照画就行，反正正面会盖住它。像这里背面后画，就盖不住；而且从来不是免费的：每个被遮住的像素照样要着色。",
            ),
            simplified: Tr::new(
                "Real GPUs let you choose which winding is front and whether to cull front, back or neither; Prism fixes positive area as front and always culls the back.",
                "真实 GPU 可以选择哪种绕向算正面，以及剔除正面、背面还是都不剔；Prism 固定有向面积为正的是正面，总是剔除背面。",
            ),
        },
        profile: Profile::PRISM_UNITS,
        ..base("the-winding", 3, View::Screen, DOT_OPS)
    }
}

/// 32 vertices, one per lane: x' at rom[0..32], depth w at rom[32..64]. `back`
/// of them sit at or behind the eye; with `zero` one of those is exactly 0.
fn vertices(seed: u64, back: usize, zero: bool) -> Case {
    let mut r = rng(seed);
    let mut order: Vec<usize> = (0..LANES).collect();
    for i in (1..LANES).rev() {
        order.swap(i, below(&mut r, i as i32 + 1) as usize);
    }
    let mut rom: Vec<i32> = (0..LANES).map(|_| below(&mut r, 19) - 9).collect();
    let mut w: Vec<i32> = (0..LANES).map(|_| 3 + below(&mut r, 38)).collect();
    for (k, &l) in order[..back].iter().enumerate() {
        w[l] = if zero && k == 0 {
            0
        } else {
            below(&mut r, 7) - 6
        };
    }
    rom.extend(w);
    rom_case(rom)
}

/// Exact projection of what is in front of the eye; 255 for the rest.
fn ahead(c: &Case) -> Vec<Option<i32>> {
    lanes(|l| {
        let (x, w) = (c.rom[l], c.rom[32 + l]);
        if w > 0 {
            (8.0 + x as f64 * 16.0 / w as f64).round() as i32
        } else {
            255
        }
    })
}

fn behind_you() -> Task {
    Task {
        title: Tr::new("Behind You", "身后"),
        brief: Tr::new(
            "Huajuan has spun the cardboard tower so that some corners swing past the viewer's whiskers. Vertex l is lane l: its x' is rom[l] and its depth w = 16 + z' is rom[32 + l], from -6 to 40. A vertex with w ≤ 0 is level with the eye or behind it.\nWrite out[l] = 8 + x'·16 / w for every vertex in front of the eye (w > 0), to within 1. A vertex at or behind the eye is not drawn: write 255.\nNew: RCP rd, src puts 65536 / src in rd (the largest number there is, for 0). The unit also raises a flag whenever a lane with a value ≤ 0 goes through it, and the replay will show you where.",
            "花卷把纸箱塔转了个身，有几个角甩到了观察者的胡须后面。顶点 l 就是车道 l：它的 x' 在 rom[l]，深度 w = 16 + z' 在 rom[32 + l]，范围 -6 到 40。w ≤ 0 的顶点与眼睛齐平，或者在眼睛后面。\n对眼睛前方（w > 0）的每个顶点，写入 out[l] = 8 + x'·16 / w，误差在 1 以内。眼睛处或眼睛后方的顶点不画：写 255。\n新指令：RCP rd, src 把 65536 / src 放进 rd（src 为 0 时是最大的数）。只要有 ≤ 0 的车道经过这个单元，它就会举起一面旗，回放会告诉你是在哪里。",
        ),
        cases: || {
            vec![
                vertices(601, 4, false),
                vertices(602, 12, false),
                vertices(603, 0, false),
            ]
        },
        hidden: || vec![vertices(604, 3, true), vertices(605, 32, false)],
        expect: ahead,
        tolerance: 1,
        goals: [Goal::new(Metric::Cycles, 22), Goal::new(Metric::Size, 19)],
        starter: Source::Code(
            "; projects every vertex, in front or not | 每个顶点都照除，不管在不在前面\nADD r1, lane, 32\nLD r0, [lane]\nLD r1, [r1]\nNOP\nSHL r0, r0, 4\nRCP r2, r1\nSHL r3, r1, 3\nNOP\nADD r0, r0, r3\nNOP\nMUL r2, r0, r2\nNOP\nNOP\nNOP\nSHR r2, r2, 16\nNOP\nST [lane], r2\n",
        ),
        reference: Source::Code(
            "ADD r1, lane, 32\nLD r0, [lane]\nLD r1, [r1]\nNOP\nSHL r0, r0, 4\nRCP r2, r1\nSHL r3, r1, 3\nSETP.LE p0, r1, 0\nADD r0, r0, r3\nNOP\nMUL r2, r0, r2\nNOP\nNOP\nNOP\nSHR r2, r2, 16\nNOP\nSEL r2, p0, 255, r2\nNOP\nST [lane], r2\n",
        ),
        predict: Predict {
            question: Tr::new(
                "A vertex sits behind the eye at w = -4. What does RCP of it give, 65536 / -4?",
                "一个顶点在眼睛后面，w = -4。对它做 RCP，也就是 65536 / -4，会得到什么？",
            ),
            options: [
                Tr::new(
                    "A negative number, so the vertex lands mirrored on the screen",
                    "一个负数，于是顶点被镜像着落到屏幕上",
                ),
                Tr::new("Zero, so it quietly vanishes", "零，于是它悄悄消失"),
                Tr::new("The same as for w = 4", "和 w = 4 时一样"),
            ],
            answer: 0,
            why: Tr::new(
                "The sign survives the division: 65536 / -4 = -16384. The corner behind you is drawn as a ghost on the opposite side of the picture.",
                "符号会留在结果里：65536 / -4 = -16384。身后的那个角，被画成了画面另一侧的一个幽灵。",
            ),
        },
        hints: [
            Tr::new(
                "Nothing is wrong with the arithmetic; it is the vertices that should never have been divided. The replay flags the first RCP that saw a lane with w ≤ 0.",
                "算术没有错，错在那些本来就不该被除的顶点。回放会标出第一次让 w ≤ 0 的车道经过的 RCP。",
            ),
            Tr::new(
                "Test w ≤ 0 with SETP.LE, let every lane do the projection anyway, and replace the behind lanes with 255 using SEL right before the store.",
                "用 SETP.LE 判断 w ≤ 0，让所有车道照常投影，最后在写出之前用 SEL 把身后的车道换成 255。",
            ),
            Tr::new(
                "Fewer beats: 8 + 16·x'/w = (16·x' + 8·w)/w, and the numerator can be built while RCP is busy. RCP and MUL each take 4 beats, so three instructions must separate each from its reader.",
                "再省几拍：8 + 16·x'/w = (16·x' + 8·w)/w，分子可以在 RCP 忙着的时候算好。RCP 和 MUL 都要 4 拍，它们和读结果的指令之间必须隔开三条指令。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Clipping", "裁剪"),
            line: Tr::new(
                "A perspective divide only makes sense in front of the eye. Behind it, w changes sign and the picture folds over itself.",
                "透视除法只在眼睛前方才有意义。到了身后，w 变号，画面会翻折到自己身上。",
            ),
            reality: Tr::new(
                "GPUs clip triangles against the near plane in homogeneous space, before the divide by w, and cut new vertices where an edge crosses it.",
                "GPU 在除以 w 之前，就在齐次空间里把三角形对近平面做裁剪，边穿过近平面的地方会切出新的顶点。",
            ),
            myth: Tr::new(
                "Myth: a wider field of view fixes it. The field of view scales x and y; it never changes the sign of w.",
                "误解：把视野放宽就好了。视野角缩放的是 x 和 y，改变不了 w 的符号。",
            ),
            simplified: Tr::new(
                "Prism only marks a whole vertex as not drawn. Real clipping keeps the part of the triangle in front of the eye.",
                "Prism 只是把整个顶点标成不画。真实的裁剪会保留三角形在眼睛前方的那一部分。",
            ),
        },
        clue: Some(Clue {
            what: Tr::new(
                "the first reciprocal taken of a vertex at or behind the eye (w ≤ 0)",
                "第一次对眼睛处或身后的顶点（w ≤ 0）取倒数",
            ),
            find: crate::grade::first_behind,
        }),
        profile: Profile::PRISM_UNITS,
        ..base("behind-you", 4, View::Lanes, RCP_LANE_OPS)
    }
}

/// A shaded triangle: edge i is opposite vertex i (positive inside), the three
/// colours at rom[9..12], and rom[12] = 65536 / (e0 + e1 + e2), the same at
/// every pixel.
fn shaded(v: [(i32, i32); 3], c: [i32; 3]) -> Case {
    let (v, c) = if area(v) < 0 {
        ([v[0], v[2], v[1]], [c[0], c[2], c[1]])
    } else {
        (v, c)
    };
    let mut rom = edges([v[1], v[2], v[0]]);
    rom.extend(c);
    rom.push(65536 / area(v));
    rom_case(rom)
}

/// Exact barycentric blend inside the triangle, 0 outside.
fn blended(c: &Case) -> Vec<Option<i32>> {
    paint(c, |r, x, y| {
        let e = [edge(r, 0, x, y), edge(r, 3, x, y), edge(r, 6, x, y)];
        if e.iter().all(|&v| v > 0) {
            let sum: i32 = e.iter().sum();
            let top: i32 = (0..3).map(|i| e[i] * r[9 + i]).sum();
            (top as f64 / sum as f64).round() as i32
        } else {
            0
        }
    })
}

fn barycentric() -> Task {
    Task {
        title: Tr::new("Barycentric", "重心坐标"),
        brief: Tr::new(
            "Huajuan wants her paper lantern shaded smoothly: every corner of the triangle has its own colour and the colour in between must blend. rom[0..9] are the three edges (edge i is the one opposite vertex i, positive inside), rom[9..12] are the colours of vertices 0, 1, 2, and rom[12] is 65536 / (e0 + e1 + e2), the same for every pixel: the three edges always add up to twice the triangle's area.\nVertex i owns e_i / (e0 + e1 + e2) of a pixel: its barycentric coordinate. Paint every pixel strictly inside the triangle with the weighted sum of the three colours, to within 1. Everything else is 0.\nNew: LERP rd, ra, rb computes (r[a]·r[b] + r[a+1]·r[b+1] + r[a+2]·r[b+2]) >> 16 over three consecutive registers each: three weights, where 65536 means one, times three values.",
            "花卷想让纸灯笼的颜色平滑过渡：三角形每个角都有自己的颜色，中间必须渐变。rom[0..9] 是三条边（边 i 是顶点 i 对面的那条，内部为正），rom[9..12] 是顶点 0、1、2 的颜色，rom[12] 是 65536 / (e0 + e1 + e2)，每个像素都一样：三条边之和永远等于三角形面积的两倍。\n顶点 i 在一个像素里占 e_i / (e0 + e1 + e2)，这就是它的重心坐标。把严格在三角形内部的每个像素涂成三种颜色的加权和，误差在 1 以内。其余为 0。\n新指令：LERP rd, ra, rb 计算 (r[a]·r[b] + r[a+1]·r[b+1] + r[a+2]·r[b+2]) >> 16，两边各取三个连续寄存器：三个权重（65536 代表 1）乘上三个值。",
        ),
        cases: || {
            vec![
                shaded([(4, 4), (28, 8), (10, 28)], [3, 9, 14]),
                shaded([(8, 2), (30, 16), (2, 26)], [12, 2, 7]),
                shaded([(2, 2), (30, 4), (14, 30)], [15, 6, 1]),
            ]
        },
        hidden: || {
            vec![
                shaded([(12, 2), (20, 6), (14, 30)], [2, 3, 14]),
                shaded([(6, 4), (30, 12), (10, 30)], [1, 15, 8]),
            ]
        },
        expect: blended,
        tolerance: 1,
        goals: [Goal::new(Metric::Cycles, 200), Goal::new(Metric::Size, 19)],
        starter: Source::Code(
            "; the old scanline fill: c0 on the left to c1 on the right | 旧扫描线填充：从左边的 c0 渐变到右边的 c1\nEDGE r0, [0]\nEDGE r1, [3]\nEDGE r2, [6]\nLD r4, [9]\nLD r5, [10]\nMIN r3, r0, r1\nNOP\nSUB r5, r5, r4\nMIN r3, r3, r2\nMUL r5, r5, x\nSETP.GT p0, r3, 0\nNOP\nNOP\nSHR r5, r5, 4\nNOP\nADD r5, r5, r4\nNOP\nSEL r5, p0, r5, 0\nNOP\nPIX r5\n",
        ),
        reference: Source::Code(
            "LD r7, [12]\nEDGE r0, [0]\nEDGE r1, [3]\nEDGE r2, [6]\nLD r4, [9]\nLD r5, [10]\nMUL r8, r0, r7\nMUL r9, r1, r7\nMUL r10, r2, r7\nLD r6, [11]\nMIN r3, r0, r1\nNOP\nLERP r11, r8, r4\nMIN r3, r3, r2\nNOP\nSETP.GT p0, r3, 0\nSEL r11, p0, r11, 0\nNOP\nPIX r11\n",
        ),
        predict: Predict {
            question: Tr::new(
                "Add the three edge values e0 + e1 + e2 at two different pixels inside the same triangle. How do the two sums compare?",
                "在同一个三角形内部的两个不同像素上，各把三条边的值 e0 + e1 + e2 加起来。两个和有什么关系？",
            ),
            options: [
                Tr::new("The same number at every pixel", "每个像素都是同一个数"),
                Tr::new("Larger nearer the middle", "越靠中心越大"),
                Tr::new("Unrelated", "毫无关系"),
            ],
            answer: 0,
            why: Tr::new(
                "The three corner triangles a pixel cuts out always tile the whole triangle, so their areas, the edge values, always add up to the same total. That is why one reciprocal serves every pixel.",
                "一个像素把三角形切成三个小三角形，它们总能拼回整个三角形，所以面积（也就是边的值）之和永远是同一个总数。这就是一个倒数可以供所有像素使用的原因。",
            ),
        },
        hints: [
            Tr::new(
                "Colour = w0·c0 + w1·c1 + w2·c2, where w_i = e_i / (e0 + e1 + e2). The weights are non-negative and add up to one, so the result always lies between the corner colours.",
                "颜色 = w0·c0 + w1·c1 + w2·c2，其中 w_i = e_i / (e0 + e1 + e2)。权重非负、加起来是 1，所以结果总落在三个角的颜色之间。",
            ),
            Tr::new(
                "Multiply each edge by rom[12] to get a weight where 65536 means one. LERP then multiplies the weights by the colours, adds the three products and shifts the 65536 away.",
                "把每条边乘以 rom[12]，得到以 65536 代表 1 的权重。LERP 再把权重乘上颜色、三项相加，并把 65536 移掉。",
            ),
            Tr::new(
                "LERP wants the weights in three consecutive registers and the colours in three others, and keep the raw edges for the inside test. Issue the loads and EDGEs first and let their beats overlap.",
                "LERP 要求权重在三个连续寄存器里，颜色在另外三个里；原始的边留给内部判断用。先把 LD 和 EDGE 发出去，让它们的等待重叠起来。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Barycentric coordinates", "重心坐标"),
            line: Tr::new(
                "Three edge values divided by their sum are three weights that add up to one. They say how much of a pixel each corner owns, and they blend anything: colour, depth, texture coordinates.",
                "三条边的值除以它们的和，就是加起来为 1 的三个权重。它们说明每个角占这个像素的多少，什么都能混合：颜色、深度、纹理坐标。",
            ),
            reality: Tr::new(
                "Every value a GPU interpolates across a triangle is a barycentric blend, and the weights come straight from the edge functions the rasteriser already evaluates. Gouraud shading in 1971 was the first use.",
                "GPU 在三角形上插值的每一个量都是重心混合，权重直接来自光栅器本来就在算的边函数。1971 年的 Gouraud 着色是最早的应用。",
            ),
            myth: Tr::new(
                "Myth: blend along each scanline from the left edge to the right edge. That only works when colour changes along x alone; on a tall thin triangle the seams show.",
                "误解：沿着每条扫描线从左边缘到右边缘渐变就行。只有颜色仅沿 x 变化时才对；遇到又高又细的三角形，接缝就露出来了。",
            ),
            simplified: Tr::new(
                "Real hardware does the blend in floating point, once per attribute; Prism uses weights in 16.16 fixed point and a single shared reciprocal.",
                "真实硬件用浮点数、对每个属性各做一次混合；Prism 用 16.16 定点权重和一个共用的倒数。",
            ),
        },
        profile: Profile::PRISM_UNITS,
        ..base("barycentric", 6, View::Screen, LERP_OPS)
    }
}

/// A triangle on a slope: edge i is opposite vertex i (positive inside), the
/// colours at rom[9..12], and the depths w (bigger is farther) at rom[12..15].
fn tilted(v: [(i32, i32); 3], c: [i32; 3], w: [i32; 3]) -> Case {
    let (v, c, w) = if area(v) < 0 {
        ([v[0], v[2], v[1]], [c[0], c[2], c[1]], [w[0], w[2], w[1]])
    } else {
        (v, c, w)
    };
    let mut rom = edges([v[1], v[2], v[0]]);
    rom.extend(c);
    rom.extend(w);
    rom.push(0);
    rom_case(rom)
}

/// The correct blend: weights e_i / w_i, normalised. Strictly inside only.
fn foreshortened(c: &Case) -> Vec<Option<i32>> {
    paint(c, |r, x, y| {
        let e = [edge(r, 0, x, y), edge(r, 3, x, y), edge(r, 6, x, y)];
        if e.iter().all(|&v| v > 0) {
            let top: f64 = (0..3)
                .map(|i| e[i] as f64 * r[9 + i] as f64 / r[12 + i] as f64)
                .sum();
            let sum: f64 = (0..3).map(|i| e[i] as f64 / r[12 + i] as f64).sum();
            (top / sum).round() as i32
        } else {
            0
        }
    })
}

fn persp_correct() -> Task {
    Task {
        title: Tr::new("Perspective-Correct", "透视校正"),
        brief: Tr::new(
            "Huajuan's lantern now hangs on a long slanted wall, one corner right under her nose and another far down the corridor. The rasteriser's colours look wrong in the middle of the triangle: the blend runs too fast near the far corner, and the lantern looks bent.\nThe edges are in rom[0..9] (edge i is opposite vertex i, positive inside), the colours in rom[9..12], and the depths w of the three vertices in rom[12..15]; a bigger w is farther away. Blending by plain barycentric weights is only right when all three w are equal. On a slope the colour is Σ(e_i·c_i / w_i) / Σ(e_i / w_i).\nPaint every pixel strictly inside the triangle with that colour, to within 1. Everything else is 0. Find the first pixel that goes wrong.",
            "花卷的灯笼现在挂在一面长长的斜墙上：一个角就在她鼻子底下，另一个角远在走廊尽头。光栅器画出来的三角形中段颜色不对：靠近远处那个角时渐变走得太快，灯笼看起来是弯的。\n三条边在 rom[0..9]（边 i 是顶点 i 对面的那条，内部为正），颜色在 rom[9..12]，三个顶点的深度 w 在 rom[12..15]；w 越大越远。直接用重心权重混合，只有三个 w 相等时才对。在斜面上，颜色应当是 Σ(e_i·c_i / w_i) / Σ(e_i / w_i)。\n把严格在三角形内部的每个像素涂成这个颜色，误差在 1 以内。其余为 0。找出第一个出错的像素。",
        ),
        cases: || {
            vec![
                tilted([(4, 4), (28, 8), (10, 28)], [3, 9, 14], [16, 16, 16]),
                tilted([(8, 2), (30, 16), (2, 26)], [12, 2, 7], [16, 16, 64]),
                tilted([(2, 2), (30, 4), (14, 30)], [15, 6, 1], [16, 32, 128]),
            ]
        },
        hidden: || {
            vec![
                tilted([(6, 4), (30, 12), (10, 30)], [1, 15, 8], [16, 128, 128]),
                tilted([(2, 6), (26, 2), (18, 30)], [4, 14, 15], [32, 32, 128]),
            ]
        },
        expect: foreshortened,
        tolerance: 1,
        goals: [Goal::new(Metric::Cycles, 304), Goal::new(Metric::Size, 35)],
        starter: Source::Code(
            "; the plain barycentric blend from the last level | 上一关的重心混合\nEDGE r0, [0]\nEDGE r1, [3]\nEDGE r2, [6]\nLD r4, [9]\nLD r5, [10]\nLD r6, [11]\nADD r7, r0, r1\nMIN r3, r0, r1\nADD r7, r7, r2\nMIN r3, r3, r2\nRCP r7, r7\nSETP.GT p0, r3, 0\nNOP\nNOP\nMUL r8, r0, r7\nMUL r9, r1, r7\nMUL r10, r2, r7\nNOP\nNOP\nNOP\nLERP r11, r8, r4\nNOP\nNOP\nNOP\nSEL r11, p0, r11, 0\nNOP\nPIX r11\n",
        ),
        reference: Source::Code(
            "LD r4, [12]\nLD r5, [13]\nLD r6, [14]\nRCP r4, r4\nRCP r5, r5\nRCP r6, r6\nEDGE r0, [0]\nEDGE r1, [3]\nEDGE r2, [6]\nLD r8, [9]\nLD r9, [10]\nLD r10, [11]\nSHL r4, r4, 4\nSHL r5, r5, 4\nSHL r6, r6, 4\nMUL r8, r8, r4\nLERP r7, r0, r4\nMUL r9, r9, r5\nMUL r10, r10, r6\nMIN r3, r0, r1\nRCP r7, r7\nMIN r3, r3, r2\nLERP r11, r0, r8\nSETP.GT p0, r3, 0\nNOP\nNOP\nMUL r11, r11, r7\nNOP\nNOP\nNOP\nSHR r11, r11, 16\nNOP\nSEL r11, p0, r11, 0\nNOP\nPIX r11\n",
        ),
        predict: Predict {
            question: Tr::new(
                "A triangle's corner A is very near (w = 16) and corner B is very far (w = 128). On screen, the pixel exactly halfway between A and B is closer in colour to which corner?",
                "三角形的角 A 很近（w = 16），角 B 很远（w = 128）。在屏幕上正好位于 A 和 B 中点的像素，颜色更接近哪个角？",
            ),
            options: [
                Tr::new("Neither, exactly in the middle", "都不接近，正好在中间"),
                Tr::new("The near corner A", "近处的角 A"),
                Tr::new("The far corner B", "远处的角 B"),
            ],
            answer: 1,
            why: Tr::new(
                "Near things take up more of the screen, so the halfway pixel on screen sits much closer to A in the real world, and its colour leans to A. The weight of a corner is e / w, so a bigger w counts for less.",
                "近处的东西在屏幕上占的地方更大，所以屏幕上的中点其实离 A 近得多，颜色偏向 A。角的权重是 e / w，w 越大占的比重越小。",
            ),
        },
        hints: [
            Tr::new(
                "Plain weights e_i / (e0 + e1 + e2) assume the corners are equally deep. On a slope the pixel owns e_i / w_i of the corner i, so use those as the weights and divide by their sum.",
                "普通权重 e_i / (e0 + e1 + e2) 假定三个角一样深。在斜面上，角 i 在像素里占的是 e_i / w_i，用它们当权重，再除以它们的和。",
            ),
            Tr::new(
                "Take q_i = RCP(w_i) once per vertex. Then both sums are LERPs with the raw edges as one operand: Σ e_i·q_i for the denominator and Σ e_i·(c_i·q_i) for the numerator. One more RCP of the denominator and a MUL finish the division.",
                "每个顶点只取一次 q_i = RCP(w_i)。然后两个和都是 LERP，一边是原始的边：分母 Σ e_i·q_i，分子 Σ e_i·(c_i·q_i)。再对分母取一次 RCP、做一次 MUL，除法就完成了。",
            ),
            Tr::new(
                "RCP gives 65536 / w, so shift it left by 4 before use: that keeps the denominator large enough to survive the final reciprocal. The numerator and the denominator LERPs both need three consecutive registers on each side; start the loads and reciprocals first, because they take longest.",
                "RCP 给出 65536 / w，用之前先左移 4 位：这样分母足够大，经得起最后一次倒数。分子和分母的 LERP 两边都要三个连续寄存器；先发出 LD 和 RCP，因为它们最慢。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Perspective-correct interpolation", "透视校正插值"),
            line: Tr::new(
                "On screen, colour does not change at a steady rate across a slanted triangle, but 1/w does. So blend c/w and 1/w linearly, then divide.",
                "在斜着的三角形上，颜色在屏幕上不是匀速变化的，但 1/w 是。所以对 c/w 和 1/w 做线性混合，最后再相除。",
            ),
            reality: Tr::new(
                "The original PlayStation interpolated textures linearly in screen space and had no per-pixel divide, so floors and walls swam and bent as the camera moved. Every GPU since divides once per pixel, and it is one of the reasons the reciprocal unit exists.",
                "初代 PlayStation 在屏幕空间里线性插值纹理，没有逐像素除法，镜头一动，地面和墙壁就扭曲摇摆。此后的每一块 GPU 都对每个像素做一次除法，倒数单元的存在也有这个原因。",
            ),
            myth: Tr::new(
                "Myth: cut the triangle into many small ones and the bend goes away. The error only shrinks, never vanishes, and the instruction count multiplies.",
                "误解：把三角形切成许多小三角形，弯曲就消失了。误差只会缩小，不会消除，而且指令数成倍增加。",
            ),
            simplified: Tr::new(
                "Prism does one 16.16 reciprocal per vertex and one per pixel with rounding; real hardware keeps floating-point attributes and a dedicated interpolator.",
                "Prism 对每个顶点和每个像素各做一次 16.16 倒数并有舍入误差；真实硬件使用浮点属性和专用的插值器。",
            ),
        },
        clue: Some(Clue {
            what: Tr::new(
                "the first pixel whose colour is off by more than the tolerance",
                "第一个颜色偏差超过容差的像素",
            ),
            find: crate::grade::first_wrong_write,
        }),
        profile: Profile::PRISM_UNITS,
        ..base("persp-correct", 7, View::Screen, PERSP_OPS)
    }
}
