//! Chapter 1 · Lumen: blueprint circuits. Each pixel runs the same wiring.

use oo_base::Tr;

use super::{frames, screen, stub};
use crate::graph::{Graph, Node, NodeKind as N};
use crate::isa::FB_LEN;
use crate::task::*;
use crate::vm::Launch;

const OUT: [f32; 2] = [340.0, 0.0];

/// A node row: kind, position, and `(port, constant)` pairs.
type Row<'a> = (N, [f32; 2], &'a [(usize, i32)]);

/// Builds a graph from node rows and `(from, to, port)` wires.
/// Row indices start at 1; node 0 is the output.
fn circuit(nodes: &[Row], wires: &[(usize, usize, usize)]) -> Graph {
    let mut g = Graph::new(OUT);
    for (kind, pos, consts) in nodes {
        let mut n = Node::new(*kind, *pos);
        for &(p, v) in *consts {
            n = n.with(p, v);
        }
        g.push(n);
    }
    for &(from, to, port) in wires {
        assert!(g.connect(from, to, port), "bad wire {from}->{to}:{port}");
    }
    g
}

fn base(id: &'static str, slot: u8, nodes: &'static [N]) -> Task {
    stub(
        id,
        1,
        slot,
        View::Screen,
        Editor::Graph(nodes),
        Launch::SCREEN,
        FB_LEN,
    )
}

pub fn tasks() -> Vec<Task> {
    vec![
        first_light(),
        the_line(),
        the_frame(),
        opening_night(),
        the_disc(),
        upside_down(),
        the_seam(),
        tide_board(),
    ]
}

const ALL_NODES: &[N] = &[
    N::X,
    N::Y,
    N::Tid,
    N::Frame,
    N::Add,
    N::Sub,
    N::Mul,
    N::And,
    N::Shr,
    N::Lt,
    N::Ge,
    N::Eq,
    N::Mux,
];

/// Whether a bulb sits on the outermost ring of the 16×16 sign.
fn on_border(x: i32, y: i32) -> bool {
    x == 0 || y == 0 || x == 15 || y == 15
}

fn first_light() -> Task {
    Task {
        title: Tr::new("A Single Light", "一盏灯"),
        brief: Tr::new(
            "The Grid Hall sign has 256 bulbs and only one circuit.\nEvery bulb runs the same wiring, fed its own X, Y and ADDR.\nLight only bulb (5,3), in colour 7.",
            "栅厅的招牌有 256 个灯泡，却只有一套电路。\n每个灯泡运行同样的接线，只是各自拿到自己的 X、Y 和 ADDR。\n只点亮 (5,3) 这一盏，颜色 7。",
        ),
        expect: |_| screen(|x, y| if (x, y) == (5, 3) { 7 } else { 0 }),
        goals: [Goal::new(Metric::Cycles, 32), Goal::new(Metric::Size, 2)],
        starter: Source::Graph(|| circuit(&[(N::Tid, [-300.0, 40.0], &[])], &[])),
        reference: Source::Graph(|| {
            circuit(
                &[
                    (N::Tid, [-300.0, 40.0], &[]),
                    (N::Eq, [-60.0, 20.0], &[(1, 53)]),
                    (N::Mul, [140.0, 0.0], &[(1, 7)]),
                ],
                &[(1, 2, 0), (2, 3, 0), (3, 0, 0)],
            )
        }),
        predict: Predict {
            question: Tr::new(
                "Bulbs are numbered row by row, 16 per row. What is the ADDR of bulb (5,3)?",
                "灯泡按行编号，每行 16 个。灯泡 (5,3) 的 ADDR 是多少？",
            ),
            options: [Tr::new("8", "8"), Tr::new("35", "35"), Tr::new("53", "53")],
            answer: 2,
            why: Tr::new(
                "ADDR = y·16 + x = 3·16 + 5 = 53. Memory is one long line; the screen folds it into rows.",
                "ADDR = y·16 + x = 3·16 + 5 = 53。内存是一条长线，屏幕只是把它折成一行行。",
            ),
        },
        hints: [
            Tr::new(
                "Every bulb asks the same question. Which question is true only for (5,3)?",
                "每个灯泡都在问同一个问题。什么问题只对 (5,3) 成立？",
            ),
            Tr::new(
                "A=B outputs 1 when its inputs match and 0 otherwise. Compare ADDR with 53.",
                "A=B 在两个输入相等时输出 1，否则输出 0。把 ADDR 和 53 比较。",
            ),
            Tr::new(
                "ADDR → A=B (B = 53) → MUL (B = 7) → PIXEL. The chosen bulb gets 1×7 = 7; all others 0×7 = 0.",
                "ADDR → A=B（B = 53）→ MUL（B = 7）→ PIXEL。被选中的灯泡得到 1×7 = 7，其余都是 0×7 = 0。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Data parallelism", "数据并行"),
            line: Tr::new(
                "You never wrote a loop over 256 pixels. You wrote one pixel's rule, and the machine ran it everywhere.",
                "你没有写遍历 256 个像素的循环，只写了一个像素的规则，机器把它铺满了全部像素。",
            ),
            reality: Tr::new(
                "A GPU launches thousands of threads with the same kernel; each reads its built-in index to find the data it owns.",
                "GPU 用同一个内核启动成千上万个线程，每个线程读取自己的内建编号来找到要处理的数据。",
            ),
            myth: Tr::new(
                "Myth: a GPU is a CPU with more cores. Its threads are cheap and identical — one program, different indices.",
                "误解：GPU 只是核更多的 CPU。它的线程廉价且相同——同一份程序，不同的编号。",
            ),
            simplified: Tr::new(
                "OC-1 gives each pixel exactly one thread on a 16×16 grid. Real launches pick grid and block shapes freely.",
                "OC-1 让每个像素恰好对应一个线程、网格固定为 16×16。真实的启动可以自由选择网格与线程块形状。",
            ),
        },
        ..base(
            "first-light",
            1,
            &[N::X, N::Y, N::Tid, N::Add, N::Mul, N::Eq, N::Mux],
        )
    }
}

fn the_line() -> Task {
    Task {
        title: Tr::new("Raster Line", "扫描线"),
        brief: Tr::new(
            "The Grid Hall clock ticks once per frame and never stops.\nDraw a line in colour 10 on row CLOCK mod 16, so it sweeps down and wraps.\nSeveral frames are checked — a fixed row will not do.",
            "栅厅的时钟每帧走一格，永不停止。\n在第 CLOCK mod 16 行画一条颜色 10 的线，让它向下扫过并循环。\n会检查多个帧——固定的一行可过不了关。",
        ),
        cases: || frames(&[0, 5, 13, 22, 47]),
        hidden: || frames(&[9, 31, 64]),
        expect: |c| screen(|_, y| if y == c.frame & 15 { 10 } else { 0 }),
        goals: [Goal::new(Metric::Cycles, 40), Goal::new(Metric::Size, 3)],
        starter: Source::Graph(|| {
            circuit(
                &[
                    (N::Y, [-300.0, 40.0], &[]),
                    (N::Eq, [-60.0, 20.0], &[]),
                    (N::Mul, [140.0, 0.0], &[(1, 10)]),
                ],
                &[(1, 2, 0), (2, 3, 0), (3, 0, 0)],
            )
        }),
        reference: Source::Graph(|| {
            circuit(
                &[
                    (N::Frame, [-320.0, -80.0], &[]),
                    (N::And, [-160.0, -60.0], &[(1, 15)]),
                    (N::Y, [-320.0, 80.0], &[]),
                    (N::Eq, [-10.0, 20.0], &[]),
                    (N::Mul, [160.0, 0.0], &[(1, 10)]),
                ],
                &[(1, 2, 0), (3, 4, 0), (2, 4, 1), (4, 5, 0), (5, 0, 0)],
            )
        }),
        predict: Predict {
            question: Tr::new(
                "At frame 21, which row should be lit?",
                "第 21 帧时，应该点亮哪一行？",
            ),
            options: [
                Tr::new("Row 21 — off the screen", "第 21 行——在屏幕外"),
                Tr::new("Row 5", "第 5 行"),
                Tr::new("Row 1", "第 1 行"),
            ],
            answer: 1,
            why: Tr::new(
                "21 AND 15 = 5. Keeping the low four bits is mod 16, so the clock wraps for free.",
                "21 AND 15 = 5。只保留低四位就是对 16 取模，时钟自然循环。",
            ),
        },
        hints: [
            Tr::new(
                "The starter lights row 0 forever. Which input changes from one frame to the next?",
                "初始电路永远点亮第 0 行。哪个输入会随帧变化？",
            ),
            Tr::new(
                "CLOCK grows past 15. AND with 15 keeps only its last four bits — always a row 0..15.",
                "CLOCK 会超过 15。与 15 做 AND 只保留最后四位——结果总在 0..15 行之间。",
            ),
            Tr::new(
                "CLOCK → AND (B = 15) → into A=B's B port, Y into its A port → MUL (B = 10) → PIXEL.",
                "CLOCK → AND（B = 15）→ 接到 A=B 的 B 口，Y 接 A 口 → MUL（B = 10）→ PIXEL。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Frames and the clock", "帧与时钟"),
            line: Tr::new(
                "The same circuit draws a new picture each frame because one input — time — changed.",
                "同一套电路每帧画出不同的画面，只因为一个输入——时间——变了。",
            ),
            reality: Tr::new(
                "Games hand every shader a time or frame value each frame; animation is the same kernel re-run with new inputs.",
                "游戏每帧都把时间或帧号传给所有着色器；动画就是同一个内核用新输入重新运行。",
            ),
            myth: Tr::new(
                "Myth: animation means storing every frame. Mostly it means recomputing from a few changing numbers.",
                "误解：动画就是把每一帧都存下来。多数时候它只是根据几个变化的数重新计算。",
            ),
            simplified: Tr::new(
                "Real displays refresh at 60–240 Hz and shaders use floating-point time; OC-1 counts whole frames.",
                "真实显示器以 60–240 Hz 刷新，着色器使用浮点时间；OC-1 只数整数帧。",
            ),
        },
        ..base(
            "the-line",
            2,
            &[
                N::X,
                N::Y,
                N::Tid,
                N::Frame,
                N::Add,
                N::Mul,
                N::And,
                N::Eq,
                N::Mux,
            ],
        )
    }
}

fn the_frame() -> Task {
    Task {
        title: Tr::new("The Frame", "画框"),
        brief: Tr::new(
            "Grandpa Juye wants a frame around the sign: colour 9 on the outermost ring of bulbs, colour 2 everywhere inside.\nEach bulb writes PIXEL exactly once, so one rule has to cover all four edges at the same time.",
            "橘爷想给招牌加一圈画框：最外一圈灯泡是颜色 9，里面全是颜色 2。\n每个灯泡只写一次 PIXEL，所以一条规则要同时管住四条边。",
        ),
        expect: |_| screen(|x, y| if on_border(x, y) { 9 } else { 2 }),
        goals: [Goal::new(Metric::Cycles, 96), Goal::new(Metric::Size, 6)],
        starter: Source::Graph(|| {
            circuit(
                &[
                    (N::X, [-300.0, 40.0], &[]),
                    (N::Eq, [-60.0, 20.0], &[]),
                    (N::Mux, [160.0, 0.0], &[(1, 9), (2, 2)]),
                ],
                &[(1, 2, 0), (2, 3, 0), (3, 0, 0)],
            )
        }),
        reference: Source::Graph(|| {
            circuit(
                &[
                    (N::X, [-380.0, -80.0], &[]),
                    (N::Add, [-230.0, -80.0], &[(1, 1)]),
                    (N::And, [-90.0, -80.0], &[(1, 14)]),
                    (N::Y, [-380.0, 70.0], &[]),
                    (N::Add, [-230.0, 70.0], &[(1, 1)]),
                    (N::And, [-90.0, 70.0], &[(1, 14)]),
                    (N::Mul, [50.0, 0.0], &[]),
                    (N::Mux, [190.0, 0.0], &[(1, 2), (2, 9)]),
                ],
                &[
                    (1, 2, 0),
                    (2, 3, 0),
                    (4, 5, 0),
                    (5, 6, 0),
                    (3, 7, 0),
                    (6, 7, 1),
                    (7, 8, 0),
                    (8, 0, 0),
                ],
            )
        }),
        predict: Predict {
            question: Tr::new(
                "Bulb (0,0) lies on the top edge and on the left edge. How many times does it write PIXEL?",
                "灯泡 (0,0) 既在上边又在左边。它会写几次 PIXEL？",
            ),
            options: [
                Tr::new("Twice — once for each edge", "两次——每条边一次"),
                Tr::new(
                    "Once — one rule must already know both",
                    "一次——一条规则必须同时知道两条边",
                ),
                Tr::new("Four times — once per edge test", "四次——每条边测试一次"),
            ],
            answer: 1,
            why: Tr::new(
                "A circuit has exactly one PIXEL. There is no 'draw the top, then draw the left': every condition must be merged before the single write.",
                "电路只有一个 PIXEL。没有“先画上边，再画左边”：所有条件都必须在那唯一一次写入之前合并好。",
            ),
        },
        hints: [
            Tr::new(
                "The starter only frames the left edge. MUX's ? counts any non-zero as yes — so adding two 0/1 tests already means 'either'.",
                "初始电路只框住了左边。MUX 的 ? 把任何非零都当作“是”——所以两个 0/1 测试相加就已经是“任一成立”。",
            ),
            Tr::new(
                "Fewer parts: ask the opposite. (X + 1) AND 14 is 0 only for columns 0 and 15. Inside means both X's and Y's version are non-zero.",
                "更省部件：反过来问。(X + 1) AND 14 只在第 0 列和第 15 列为 0。“在里面”就是 X 和 Y 的这个值都非零。",
            ),
            Tr::new(
                "X → ADD (B = 1) → AND (B = 14); the same for Y; MUL the two → MUX ?, with 1 → 2 and 0 → 9 → PIXEL.",
                "X → ADD（B = 1）→ AND（B = 14）；Y 同样处理；两者 MUL → MUX 的 ?，1 口 = 2，0 口 = 9 → PIXEL。",
            ),
        ],
        insight: Insight {
            title: Tr::new("One write per thread", "每个线程只写一次"),
            line: Tr::new(
                "You could not draw four edges one after another. Each bulb merged every condition into one answer and wrote it once.",
                "你没法一条边接一条边地画。每个灯泡把所有条件合并成一个答案，只写一次。",
            ),
            reality: Tr::new(
                "A pixel shader returns one colour per pixel. Shapes are combined inside the shader — with min, max, multiply and select — not by drawing over them.",
                "像素着色器每个像素只返回一个颜色。形状是在着色器内部用 min、max、乘法和选择组合起来的，而不是一层层盖上去。",
            ),
            myth: Tr::new(
                "Myth: AND and OR need special parts. With 0/1 flags, multiply is AND and add is OR — the selector only asks 'is it zero?'.",
                "误解：AND 和 OR 需要专门的部件。对 0/1 标志来说，乘法就是 AND，加法就是 OR——选择器只问“是不是零”。",
            ),
            simplified: Tr::new(
                "Real GPUs can blend several draws into the same pixel in order; OC-1's sign keeps one write per bulb per frame.",
                "真实 GPU 可以按顺序把多次绘制混合进同一个像素；OC-1 的招牌每个灯泡每帧只写一次。",
            ),
        },
        ..base(
            "the-frame",
            3,
            &[
                N::X,
                N::Y,
                N::Tid,
                N::Add,
                N::Sub,
                N::Mul,
                N::And,
                N::Lt,
                N::Ge,
                N::Eq,
                N::Mux,
            ],
        )
    }
}

/// The sprite's left edge at a frame: it slides right and restarts every 8 frames.
fn sprite_x(frame: i32) -> i32 {
    frame & 7
}

fn opening_night() -> Task {
    Task {
        title: Tr::new("Opening Night", "开幕之夜"),
        brief: Tr::new(
            "Tonight the sign must move. Paint the wall colour 1.\nA 4×4 block in colour 14 covers rows 6..9 and columns (CLOCK AND 7) to +3.\nFive different frames are checked, so the block has to follow the clock.",
            "今晚招牌要动起来。墙面涂成颜色 1。\n一个 4×4 的色块（颜色 14）占据第 6..9 行，列从 (CLOCK AND 7) 开始共 4 列。\n会检查五个不同的帧，所以色块必须跟着时钟走。",
        ),
        cases: || frames(&[0, 3, 7, 12, 29]),
        hidden: || frames(&[5, 18, 100]),
        expect: |c| {
            let x0 = sprite_x(c.frame);
            screen(|x, y| {
                if (x0..x0 + 4).contains(&x) && (6..10).contains(&y) {
                    14
                } else {
                    1
                }
            })
        },
        goals: [Goal::new(Metric::Cycles, 80), Goal::new(Metric::Size, 7)],
        starter: Source::Graph(|| {
            let mut g = Graph::new(OUT);
            g.nodes[0].inputs[0].value = 1;
            g
        }),
        reference: Source::Graph(|| {
            circuit(
                &[
                    (N::Frame, [-420.0, -120.0], &[]),
                    (N::And, [-280.0, -110.0], &[(1, 7)]),
                    (N::X, [-420.0, -20.0], &[]),
                    (N::Sub, [-140.0, -60.0], &[]),
                    (N::Shr, [0.0, -60.0], &[(1, 2)]),
                    (N::Y, [-420.0, 120.0], &[]),
                    (N::Sub, [-140.0, 110.0], &[(1, 6)]),
                    (N::Shr, [0.0, 110.0], &[(1, 2)]),
                    (N::Mux, [140.0, 90.0], &[(1, 1), (2, 14)]),
                    (N::Mux, [250.0, 0.0], &[(1, 1)]),
                ],
                &[
                    (1, 2, 0),
                    (3, 4, 0),
                    (2, 4, 1),
                    (4, 5, 0),
                    (6, 7, 0),
                    (7, 8, 0),
                    (8, 9, 0),
                    (5, 10, 0),
                    (9, 10, 2),
                    (10, 0, 0),
                ],
            )
        }),
        live: Some(|frame, _| Case::frame(frame)),
        predict: Predict {
            question: Tr::new(
                "Let d = X − left edge. For which d is a pixel inside the block's columns?",
                "设 d = X − 左边界。d 取哪些值时像素落在色块的列里？",
            ),
            options: [
                Tr::new("d = 4 only", "只有 d = 4"),
                Tr::new(
                    "0 ≤ d < 4, i.e. d SHR 2 = 0",
                    "0 ≤ d < 4，也就是 d SHR 2 = 0",
                ),
                Tr::new("Any d < 4", "任何 d < 4"),
            ],
            answer: 1,
            why: Tr::new(
                "Shifting right by 2 divides by 4. Only 0..3 become 0; negatives stay negative, 4 and up stay positive.",
                "右移 2 位就是除以 4。只有 0..3 会变成 0；负数仍为负，4 及以上仍为正。",
            ),
        },
        hints: [
            Tr::new(
                "Split it: a pixel is in the block when its column test AND its row test both pass.",
                "拆开看：像素在色块里，当且仅当列测试和行测试同时通过。",
            ),
            Tr::new(
                "MUX picks input 1 when '?' is non-zero. (X − edge) SHR 2 is non-zero exactly when the column is outside.",
                "MUX 在“?”非零时选输入 1。(X − 边界) SHR 2 恰好在列越界时非零。",
            ),
            Tr::new(
                "edge = CLOCK AND 7. Inner MUX: ? = (Y − 6) SHR 2, 1 → 1, 0 → 14. Outer MUX: ? = (X − edge) SHR 2, 1 → 1, 0 → inner.",
                "边界 = CLOCK AND 7。内层 MUX：? = (Y − 6) SHR 2，1 口 = 1，0 口 = 14。外层 MUX：? = (X − 边界) SHR 2，1 口 = 1，0 口 = 内层。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Branch-free selection", "无分支选择"),
            line: Tr::new(
                "Your circuit never asked 'if'. Every pixel computed both answers and a selector picked one.",
                "你的电路从不问“如果”。每个像素都算出两个答案，再由选择器挑一个。",
            ),
            reality: Tr::new(
                "Shader compilers turn short ifs into select instructions so neighbouring threads never take different paths.",
                "着色器编译器会把短小的 if 变成选择指令，让相邻线程永远不走不同路径。",
            ),
            myth: Tr::new(
                "Myth: computing both sides is always wasteful. When both are cheap, it beats making threads disagree.",
                "误解：两边都算总是浪费。当两边都很便宜时，它比让线程分道扬镳更划算。",
            ),
            simplified: Tr::new(
                "Real sprites are textured quads with blending; here the sprite is a rectangle test per pixel.",
                "真实的精灵是带混合的贴图四边形；这里只是每个像素做一次矩形测试。",
            ),
        },
        ..base(
            "opening-night",
            4,
            &[
                N::X,
                N::Y,
                N::Tid,
                N::Frame,
                N::Add,
                N::Sub,
                N::Mul,
                N::And,
                N::Shr,
                N::Lt,
                N::Ge,
                N::Eq,
                N::Mux,
            ],
        )
    }
}

/// Radius of the disc at a frame.
fn radius(frame: i32) -> i32 {
    2 + (frame & 3)
}

fn the_disc() -> Task {
    Task {
        title: Tr::new("The Disc", "圆盘"),
        brief: Tr::new(
            "The Grid Hall wants a round window: a disc of colour 11 on a field of colour 1, centred on bulb (8,8).\nIts radius is 2 + (CLOCK AND 3), so it breathes with the frames. A bulb belongs to the disc when its distance to the centre is at most the radius.\nThe blueprint has no square root. You will not need one.",
            "栅厅想要一扇圆窗：颜色 1 的底上有一个颜色 11 的圆盘，圆心在灯泡 (8,8)。\n半径是 2 + (CLOCK AND 3)，随帧数一胀一缩。灯泡到圆心的距离不超过半径，就算在圆盘里。\n蓝图里没有开方。你也用不着它。",
        ),
        cases: || frames(&[0, 1, 2]),
        hidden: || frames(&[3, 11, 30]),
        expect: |c| {
            let r = radius(c.frame);
            screen(|x, y| {
                if (x - 8) * (x - 8) + (y - 8) * (y - 8) <= r * r {
                    11
                } else {
                    1
                }
            })
        },
        goals: [Goal::new(Metric::Cycles, 156), Goal::new(Metric::Size, 10)],
        starter: Source::Graph(|| {
            circuit(
                &[
                    (N::X, [-300.0, 40.0], &[]),
                    (N::Sub, [-190.0, 40.0], &[(1, 8)]),
                    (N::Mul, [-80.0, 40.0], &[]),
                    (N::Ge, [30.0, 20.0], &[(0, 9)]),
                    (N::Mux, [150.0, 0.0], &[(1, 11), (2, 1)]),
                ],
                &[
                    (1, 2, 0),
                    (2, 3, 0),
                    (2, 3, 1),
                    (3, 4, 1),
                    (4, 5, 0),
                    (5, 0, 0),
                ],
            )
        }),
        reference: Source::Graph(|| {
            circuit(
                &[
                    // 1..3: (X − 8)²
                    (N::X, [-440.0, 120.0], &[]),
                    (N::Sub, [-330.0, 120.0], &[(1, 8)]),
                    (N::Mul, [-220.0, 120.0], &[]),
                    // 4..6: (Y − 8)²
                    (N::Y, [-440.0, 40.0], &[]),
                    (N::Sub, [-330.0, 40.0], &[(1, 8)]),
                    (N::Mul, [-220.0, 40.0], &[]),
                    // 7: squared distance
                    (N::Add, [-110.0, 80.0], &[]),
                    // 8..11: r²
                    (N::Frame, [-440.0, -60.0], &[]),
                    (N::And, [-330.0, -60.0], &[(1, 3)]),
                    (N::Add, [-220.0, -60.0], &[(1, 2)]),
                    (N::Mul, [-110.0, -60.0], &[]),
                    // 12..13: inside?
                    (N::Ge, [10.0, 0.0], &[]),
                    (N::Mux, [150.0, 0.0], &[(1, 11), (2, 1)]),
                ],
                &[
                    (1, 2, 0),
                    (2, 3, 0),
                    (2, 3, 1),
                    (4, 5, 0),
                    (5, 6, 0),
                    (5, 6, 1),
                    (3, 7, 0),
                    (6, 7, 1),
                    (8, 9, 0),
                    (9, 10, 0),
                    (10, 11, 0),
                    (10, 11, 1),
                    (11, 12, 0),
                    (7, 12, 1),
                    (12, 13, 0),
                    (13, 0, 0),
                ],
            )
        }),
        predict: Predict {
            question: Tr::new(
                "A bulb sits 3 columns right and 4 rows down from the centre, and the radius is 5. Where is it?",
                "一个灯泡在圆心右边 3 列、下面 4 行，半径是 5。它在哪里？",
            ),
            options: [
                Tr::new("Inside the disc", "圆内"),
                Tr::new("Exactly on the rim", "恰在圆上"),
                Tr::new("Outside the disc", "圆外"),
            ],
            answer: 1,
            why: Tr::new(
                "3² + 4² = 25 = 5². Its squared distance equals the squared radius, so the test must say 'at most', not 'less than'.",
                "3² + 4² = 25 = 5²。它的距离平方恰好等于半径平方，所以判断必须是“不超过”，而不是“小于”。",
            ),
        },
        hints: [
            Tr::new(
                "For non-negative numbers, d ≤ r holds exactly when d² ≤ r². Compare squares and the root disappears.",
                "对非负数来说，d ≤ r 恰好等价于 d² ≤ r²。比较平方，开方就消失了。",
            ),
            Tr::new(
                "Wire one node into both ports of a MUL to square it. Square (X − 8) and (Y − 8) and add them; build r from CLOCK AND 3, plus 2, and square that too.",
                "把同一个节点接到 MUL 的两个口就是平方。把 (X − 8) 和 (Y − 8) 各自平方后相加；r 由 CLOCK AND 3 再加 2 得到，也平方一下。",
            ),
            Tr::new(
                "A≥B with r² on A and the squared distance on B, then MUX: 1 → 11, 0 → 1. Use ≥, not >: bulbs on the rim are part of the disc.",
                "A≥B 的 A 口接 r²，B 口接距离平方，再接 MUX：1 口 = 11，0 口 = 1。要用 ≥ 而不是 >：圆边上的灯泡也属于圆盘。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Compare squares", "比平方，不开方"),
            line: Tr::new(
                "'Inside the circle?' has the same answer if you square both sides. The square root was never needed.",
                "“在圆里吗？”两边同时平方，答案不变。开方从头到尾都用不着。",
            ),
            reality: Tr::new(
                "Shaders and collision code compare length² with radius² in their hottest loops; square roots are the instruction they avoid.",
                "着色器和碰撞检测在最热的循环里比较长度平方与半径平方；开方是它们竭力避开的指令。",
            ),
            myth: Tr::new(
                "Myth: distance means a square root. To compare distances, squares are enough.",
                "误解：距离就得开方。比较距离的大小，平方就够了。",
            ),
            simplified: Tr::new(
                "Real shaders use fractions and fade the rim for smooth edges; OC-1's rim is a hard integer line.",
                "真实的着色器用小数，并让圆边渐隐来得到平滑边缘；OC-1 的圆边是一条硬邦邦的整数线。",
            ),
        },
        ..base("the-disc", 6, ALL_NODES)
    }
}

/// The water level on Grandpa Juye's gauge at a frame: 0..15 rows of water.
fn tide(frame: i32) -> i32 {
    frame & 15
}

fn upside_down() -> Task {
    Task {
        title: Tr::new("Upside-Down Water Level", "倒挂的水位"),
        brief: Tr::new(
            "The station's well gauge fills the sign with water from the bottom: the lowest (CLOCK AND 15) rows are colour 12, the rest colour 1.\nTonight the water hangs from the ceiling.\nFirst point at the moment the first wrong bulb is written, then fix the circuit.",
            "驿站的井水水位计从招牌底部往上灌水：最下面 (CLOCK AND 15) 行是颜色 12，其余是颜色 1。\n今晚，水挂在了天花板上。\n先指出第一个错误灯泡被写入的那一拍，再修好电路。",
        ),
        cases: || frames(&[3, 9, 14, 21, 40]),
        hidden: || frames(&[6, 27, 55]),
        expect: |c| {
            let h = tide(c.frame);
            screen(|_, y| if y + h >= 16 { 12 } else { 1 })
        },
        goals: [Goal::new(Metric::Cycles, 56), Goal::new(Metric::Size, 4)],
        starter: Source::Graph(|| {
            circuit(
                &[
                    (N::Frame, [-360.0, -80.0], &[]),
                    (N::And, [-210.0, -70.0], &[(1, 15)]),
                    (N::Y, [-360.0, 80.0], &[]),
                    (N::Lt, [-40.0, 20.0], &[]),
                    (N::Mux, [140.0, 0.0], &[(1, 12), (2, 1)]),
                ],
                &[(1, 2, 0), (3, 4, 0), (2, 4, 1), (4, 5, 0), (5, 0, 0)],
            )
        }),
        reference: Source::Graph(|| {
            circuit(
                &[
                    (N::Frame, [-360.0, -80.0], &[]),
                    (N::And, [-210.0, -70.0], &[(1, 15)]),
                    (N::Y, [-360.0, 80.0], &[]),
                    (N::Add, [-60.0, 20.0], &[]),
                    (N::Ge, [80.0, 20.0], &[(1, 16)]),
                    (N::Mux, [200.0, 0.0], &[(1, 12), (2, 1)]),
                ],
                &[
                    (1, 2, 0),
                    (3, 4, 0),
                    (2, 4, 1),
                    (4, 5, 0),
                    (5, 6, 0),
                    (6, 0, 0),
                ],
            )
        }),
        predict: Predict {
            question: Tr::new(
                "Rows are numbered from the top, 0 to 15. With 3 rows of water, which rows should be blue?",
                "行号从上往下数，0 到 15。水深 3 行时，哪几行应该是蓝的？",
            ),
            options: [
                Tr::new("Rows 0, 1, 2", "第 0、1、2 行"),
                Tr::new("Rows 13, 14, 15", "第 13、14、15 行"),
                Tr::new("Rows 14, 15, 16", "第 14、15、16 行"),
            ],
            answer: 1,
            why: Tr::new(
                "Screen y grows downward, and the last row is 15, not 16. Blue means y ≥ 16 − 3 = 13.",
                "屏幕的 y 向下增长，最后一行是 15 而不是 16。蓝色就是 y ≥ 16 − 3 = 13。",
            ),
        },
        hints: [
            Tr::new(
                "Y < h lights the first h rows from the top. Which rows does the station want?",
                "Y < h 点亮的是从顶部数起的 h 行。驿站要的是哪几行？",
            ),
            Tr::new(
                "Flipping to 16 − Y loses a row: row 15 becomes 1, not 0. Ask instead whether Y + h reaches 16.",
                "翻成 16 − Y 会少一行：第 15 行变成了 1 而不是 0。不如问 Y + h 是否达到 16。",
            ),
            Tr::new(
                "Replace A<B with ADD (Y, h) → A≥B (B = 16) → MUX ?, 1 → 12, 0 → 1.",
                "把 A<B 换成 ADD（Y, h）→ A≥B（B = 16）→ MUX 的 ?，1 口 = 12，0 口 = 1。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Screen space and off-by-one", "屏幕坐标与差一"),
            line: Tr::new(
                "The circuit was right about the water and wrong about which way is up. Screens count rows downward from 0.",
                "电路算对了水，却搞错了哪边是上。屏幕的行从 0 开始向下数。",
            ),
            reality: Tr::new(
                "Graphics APIs disagree: OpenGL puts y = 0 at the bottom, Direct3D and most image formats at the top. Ported shaders flip upside down all the time.",
                "图形 API 各不相同：OpenGL 的 y = 0 在底部，Direct3D 和大多数图片格式在顶部。移植过来的着色器经常上下颠倒。",
            ),
            myth: Tr::new(
                "Myth: flipping is 'size minus y'. With rows 0..15 it is 15 − y; 16 − y is one row off.",
                "误解：翻转就是“尺寸减 y”。行号是 0..15 时应该是 15 − y；16 − y 差了一行。",
            ),
            simplified: Tr::new(
                "Real pipelines flip in the viewport transform with fractional pixel centres; OC-1's bulbs are whole cells.",
                "真实管线在视口变换里翻转，像素中心还带着小数；OC-1 的灯泡是整格。",
            ),
        },
        clue: Some(Clue {
            what: Tr::new(
                "the first wrong bulb being written, on its warp",
                "第一个错误灯泡被写入的那一拍，以及它所在的 warp",
            ),
            find: crate::grade::first_wrong_write,
        }),
        ..base(
            "upside-down",
            5,
            &[
                N::X,
                N::Y,
                N::Tid,
                N::Frame,
                N::Add,
                N::Sub,
                N::Mul,
                N::And,
                N::Shr,
                N::Lt,
                N::Ge,
                N::Eq,
                N::Mux,
            ],
        )
    }
}

fn the_seam() -> Task {
    Task {
        title: Tr::new("The Seam", "接缝"),
        brief: Tr::new(
            "A stripe of light sweeps the Grid Hall sign: two columns of colour 10 on black, one column to the right per frame, wrapping round the edge.\nThe old wiring keeps its counter at 15 and the stripe tears at the seam: a column goes missing.\nFirst point at the first wrong bulb, then mend the circuit.",
            "栅厅的招牌上有一道光带扫过：黑底上两列颜色 10，每帧右移一列，到边缘就绕回另一头。\n旧接线把计数器卡在 15，光带过接缝时撕开了：少了一列。\n先指出第一个错误的灯泡，再修好电路。",
        ),
        cases: || frames(&[0, 9, 15, 20, 31]),
        hidden: || frames(&[14, 47, 79]),
        expect: |c| screen(|x, _| if (x - c.frame) & 15 < 2 { 10 } else { 0 }),
        goals: [Goal::new(Metric::Cycles, 84), Goal::new(Metric::Size, 4)],
        starter: Source::Graph(|| {
            circuit(
                &[
                    (N::Frame, [-420.0, -80.0], &[]),
                    (N::And, [-300.0, -80.0], &[(1, 15)]),
                    (N::X, [-420.0, 80.0], &[]),
                    (N::Sub, [-190.0, 20.0], &[]),
                    (N::Shr, [-80.0, 20.0], &[(1, 1)]),
                    (N::Eq, [30.0, 20.0], &[(1, 0)]),
                    (N::Mul, [150.0, 0.0], &[(1, 10)]),
                ],
                &[
                    (1, 2, 0),
                    (3, 4, 0),
                    (2, 4, 1),
                    (4, 5, 0),
                    (5, 6, 0),
                    (6, 7, 0),
                    (7, 0, 0),
                ],
            )
        }),
        reference: Source::Graph(|| {
            circuit(
                &[
                    (N::Frame, [-420.0, -80.0], &[]),
                    (N::X, [-420.0, 80.0], &[]),
                    (N::Sub, [-300.0, 20.0], &[]),
                    (N::And, [-190.0, 20.0], &[(1, 15)]),
                    (N::Lt, [-80.0, 20.0], &[(1, 2)]),
                    (N::Mux, [150.0, 0.0], &[(1, 10), (2, 0)]),
                ],
                &[
                    (2, 3, 0),
                    (1, 3, 1),
                    (3, 4, 0),
                    (4, 5, 0),
                    (5, 6, 0),
                    (6, 0, 0),
                ],
            )
        }),
        predict: Predict {
            question: Tr::new(
                "At frame 15 the stripe's head is on column 15 and the stripe is two columns wide. Which columns should be lit?",
                "第 15 帧，光带的头在第 15 列，光带两列宽。哪几列应该亮？",
            ),
            options: [
                Tr::new("Column 15 only", "只有第 15 列"),
                Tr::new("Columns 15 and 0", "第 15 列和第 0 列"),
                Tr::new("Columns 15 and 16", "第 15 列和第 16 列"),
            ],
            answer: 1,
            why: Tr::new(
                "There is no column 16: the stripe wraps round to column 0, where the next frame will put its head.",
                "没有第 16 列：光带绕回第 0 列，下一帧它的头就会出现在那里。",
            ),
        },
        hints: [
            Tr::new(
                "Watch frame 15: the stripe should cover columns 15 and 0, but column 0 stays dark. What does the circuit do when X − counter goes negative?",
                "看第 15 帧：光带应该盖住第 15 列和第 0 列，可第 0 列是黑的。X − 计数 变成负数时，电路在做什么？",
            ),
            Tr::new(
                "Wrap after the subtraction: (X − CLOCK) AND 15 is how far a column trails the head, whatever the clock says. Widening the counter only hides the stripe for 16 frames.",
                "回绕放在减法之后：(X − CLOCK) AND 15 是这一列落后光带头多远，不管时钟走到多少。把计数器加宽，只会让光带有 16 帧不在屏幕上。",
            ),
            Tr::new(
                "X − CLOCK → AND (B = 15) → A<B (B = 2) → MUX: 1 → 10, 0 → 0.",
                "X − CLOCK → AND（B = 15）→ A<B（B = 2）→ MUX：1 口 = 10，0 口 = 0。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Wrapping round", "绕回去"),
            line: Tr::new(
                "A counter that wraps needs a picture that wraps with it. Take the remainder after the subtraction, not before.",
                "会回绕的计数器，需要一幅同样会回绕的画面。取余要放在减法之后，而不是之前。",
            ),
            reality: Tr::new(
                "Scrolling backgrounds, ring buffers and texture repeat modes all rely on the same masked subtraction. A seam that tears is a classic scrolling bug.",
                "滚动背景、环形缓冲区和纹理重复模式，靠的都是同样的带掩码的减法。接缝撕裂是滚动画面的经典 bug。",
            ),
            myth: Tr::new(
                "Myth: a bigger counter fixes wrapping. It only moves the seam off screen for a while.",
                "误解：把计数器加宽就能解决回绕。它只是让接缝暂时跑到屏幕外面去。",
            ),
            simplified: Tr::new(
                "AND 15 is a free modulo only because 16 is a power of two; other sizes need a real division, which OC-1 does not have.",
                "AND 15 能白送一次取模，只因为 16 是 2 的幂；其他尺寸需要真正的除法，OC-1 没有。",
            ),
        },
        clue: Some(Clue {
            what: Tr::new(
                "the first bulb that should light but stays dark at the seam",
                "接缝处第一个该亮却没亮的灯泡",
            ),
            find: crate::grade::first_wrong_write,
        }),
        ..base("the-seam", 7, ALL_NODES)
    }
}

/// The water board at a frame: border 9, the camel lamp 10 when lit, water 12
/// in the bottom rows, sky 1.
fn board(frame: i32, x: i32, y: i32) -> i32 {
    let h = frame & 7;
    let lamp = (x & 14) == 2 && (y & 14) == 2 && frame & 8 != 0;
    if on_border(x, y) {
        9
    } else if lamp {
        10
    } else if y + h >= 14 {
        12
    } else {
        1
    }
}

fn tide_board() -> Task {
    Task {
        title: Tr::new("The Water Board", "水位牌"),
        brief: Tr::new(
            "Grandpa Juye's commission: the station sign, all in one circuit.\nA colour-9 frame on the outer ring. Inside, rows with Y + (CLOCK AND 7) ≥ 14 are water (12), the rest sky (1).\nThe camel lamp — bulbs (2..3, 2..3) — shows colour 10 whenever CLOCK AND 8 is not 0. The frame wins over the lamp, the lamp over the water.",
            "橘爷的委托：驿站信号牌，一套电路全部搞定。\n最外一圈是颜色 9 的画框。框内满足 Y + (CLOCK AND 7) ≥ 14 的行是水（12），其余是天（1）。\n驼灯——灯泡 (2..3, 2..3)——在 CLOCK AND 8 不为 0 时亮成颜色 10。画框盖过驼灯，驼灯盖过水。",
        ),
        cases: || frames(&[0, 5, 8, 13, 22, 47]),
        hidden: || frames(&[3, 11, 30, 61]),
        expect: |c| screen(|x, y| board(c.frame, x, y)),
        goals: [Goal::new(Metric::Cycles, 168), Goal::new(Metric::Size, 15)],
        starter: Source::Graph(|| Graph::new(OUT)),
        reference: Source::Graph(|| {
            circuit(
                &[
                    // 1..7: inside the frame
                    (N::X, [-432.0, 117.0], &[]),
                    (N::Add, [-320.0, 117.0], &[(1, 1)]),
                    (N::And, [-208.0, 117.0], &[(1, 14)]),
                    (N::Y, [-432.0, 39.0], &[]),
                    (N::Add, [-320.0, 39.0], &[(1, 1)]),
                    (N::And, [-208.0, 39.0], &[(1, 14)]),
                    (N::Mul, [-96.0, 78.0], &[]),
                    // 8..11: water
                    (N::Frame, [-432.0, -39.0], &[]),
                    (N::And, [-320.0, -39.0], &[(1, 7)]),
                    (N::Add, [-208.0, -39.0], &[]),
                    (N::Ge, [-96.0, -30.0], &[(1, 14)]),
                    // 12..16: the lamp
                    (N::Tid, [-432.0, -117.0], &[]),
                    (N::And, [-320.0, -117.0], &[(1, 0xEE)]),
                    (N::Eq, [-208.0, -117.0], &[(1, 0x22)]),
                    (N::And, [-96.0, -117.0], &[(1, 8)]),
                    (N::Mul, [16.0, -117.0], &[]),
                    // 17..19: compose, bottom layer first
                    (N::Mux, [16.0, -30.0], &[(1, 12), (2, 1)]),
                    (N::Mux, [128.0, -60.0], &[(1, 10)]),
                    (N::Mux, [236.0, 20.0], &[(2, 9)]),
                ],
                &[
                    (1, 2, 0),
                    (2, 3, 0),
                    (4, 5, 0),
                    (5, 6, 0),
                    (3, 7, 0),
                    (6, 7, 1),
                    (8, 9, 0),
                    (4, 10, 0),
                    (9, 10, 1),
                    (10, 11, 0),
                    (12, 13, 0),
                    (13, 14, 0),
                    (8, 15, 0),
                    (14, 16, 0),
                    (15, 16, 1),
                    (11, 17, 0),
                    (16, 18, 0),
                    (17, 18, 2),
                    (7, 19, 0),
                    (18, 19, 1),
                    (19, 0, 0),
                ],
            )
        }),
        live: Some(|frame, _| Case::frame(frame)),
        predict: Predict {
            question: Tr::new(
                "ADDR AND 0xEE keeps every bit of y·16 + x except the lowest bit of each. Which bulbs give 0x22?",
                "ADDR AND 0xEE 保留 y·16 + x 的所有位，只去掉 x 和 y 各自的最低位。哪些灯泡会得到 0x22？",
            ),
            options: [
                Tr::new("Only (2,2)", "只有 (2,2)"),
                Tr::new("x and y both 2 or 3", "x 和 y 都是 2 或 3"),
                Tr::new("Every bulb in row 2", "第 2 行的所有灯泡"),
            ],
            answer: 1,
            why: Tr::new(
                "Dropping the low bit merges 2 and 3. One AND and one compare test a 2×2 square — no separate row and column tests.",
                "去掉最低位就把 2 和 3 合成了一个。一次 AND 加一次比较就测出一个 2×2 方块——不用分别测行和列。",
            ),
        },
        hints: [
            Tr::new(
                "Build it in layers, like the sign itself: a sky-or-water colour first, then the lamp over it, then the frame over everything.",
                "像招牌本身一样分层搭：先选天还是水，再把驼灯盖上去，最后用画框盖住一切。",
            ),
            Tr::new(
                "Each layer is one MUX whose 0 port takes the layer below. The frame test from The Frame and the water test from Upside-Down Water Level both still work.",
                "每一层是一个 MUX，它的 0 口接下面那层。“画框”的边框测试和“倒挂的水位”的水位测试都能直接用。",
            ),
            Tr::new(
                "Water: (Y + (CLOCK AND 7)) ≥ 14. Lamp: (ADDR AND 0xEE) = 0x22, times (CLOCK AND 8). Inside: ((X+1) AND 14) × ((Y+1) AND 14).",
                "水：(Y + (CLOCK AND 7)) ≥ 14。驼灯：(ADDR AND 0xEE) = 0x22，乘以 (CLOCK AND 8)。框内：((X+1) AND 14) × ((Y+1) AND 14)。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Layers in one pass", "一遍画完所有图层"),
            line: Tr::new(
                "Frame, lamp, water and sky never overlap on screen because every bulb chose its layer before its single write.",
                "画框、驼灯、水和天在屏幕上从不打架，因为每个灯泡在唯一一次写入之前就选好了自己的图层。",
            ),
            reality: Tr::new(
                "Signage, HUDs and UI shaders compose dozens of layers in one pixel shader; the order of the selects is the draw order.",
                "招牌、HUD 和界面着色器在一个像素着色器里合成几十层；选择的先后就是绘制的先后。",
            ),
            myth: Tr::new(
                "Myth: a blinking light needs a timer per bulb. One shared clock, tested by every bulb, is enough.",
                "误解：闪烁的灯需要每个灯泡一个计时器。一个共享的时钟，每个灯泡自己去测，就够了。",
            ),
            simplified: Tr::new(
                "A real water-level board would read sensor data from memory; Lumen's blueprints only see X, Y, ADDR and CLOCK.",
                "真正的水位牌会从内存读传感器数据；Lumen 的蓝图只能看到 X、Y、ADDR 和 CLOCK。",
            ),
        },
        ..base(
            "tide-board",
            6,
            &[
                N::X,
                N::Y,
                N::Tid,
                N::Frame,
                N::Add,
                N::Sub,
                N::Mul,
                N::And,
                N::Shr,
                N::Lt,
                N::Ge,
                N::Eq,
                N::Mux,
            ],
        )
    }
}
