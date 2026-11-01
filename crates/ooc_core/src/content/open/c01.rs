//! Chapter 1 open problems: blueprint circuits on Grandpa Juye's station sign.

use oo_base::Tr;

use super::Open;
use crate::content::{frames, screen, stub};
use crate::graph::{Graph, Node, NodeKind as N};
use crate::isa::FB_LEN;
use crate::task::*;
use crate::vm::Launch;

const OUT: [f32; 2] = [340.0, 0.0];

const NODES: &[N] = &[
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

/// A node row: kind and `(port, constant)` pairs; positions are laid out in a row.
type Row<'a> = (N, &'a [(usize, i32)]);

/// Builds a graph from node rows and `(from, to, port)` wires.
/// Row indices start at 1; node 0 is the output.
fn circuit(nodes: &[Row], wires: &[(usize, usize, usize)]) -> Graph {
    let mut g = Graph::new(OUT);
    for (i, (kind, consts)) in nodes.iter().enumerate() {
        let pos = [
            -420.0 + 70.0 * i as f32,
            if i % 2 == 0 { -60.0 } else { 60.0 },
        ];
        let mut n = Node::new(*kind, pos);
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

fn empty() -> Graph {
    Graph { nodes: Vec::new() }
}

fn base(id: &'static str, slot: u8) -> Task {
    Task {
        starter: Source::Graph(empty),
        ..stub(
            id,
            1,
            slot,
            View::Screen,
            Editor::Graph(NODES),
            Launch::SCREEN,
            FB_LEN,
        )
    }
}

pub fn problems() -> Vec<Open> {
    vec![drift_stripes(), camel_walk(), sign_tiles()]
}

fn drift_stripes() -> Open {
    Open {
        task: Task {
            title: Tr::new("Drifting Stripes", "流动的斜纹"),
            brief: Tr::new(
                "Grandpa Juye wants diagonal stripes that drift with the clock.\nA bulb shows colour 6 when (X + Y + CLOCK) AND 3 is 0, and 0 otherwise.\nScore: compute nodes in your circuit. Fewer is better.",
                "橘爷想要一排随时钟流动的斜纹。\n当 (X + Y + CLOCK) AND 3 等于 0 时灯泡亮颜色 6，否则为 0。\n得分：电路里的计算节点数，越少越好。",
            ),
            cases: || frames(&[0, 1, 6]),
            hidden: || frames(&[3, 13, 42]),
            expect: |c| screen(|x, y| if (x + y + c.frame) & 3 == 0 { 6 } else { 0 }),
            reference: STRIPES_MUX,
            ..base("drift-stripes", 9)
        },
        metric: Metric::Size,
        score: None,
        score_label: None,
        marks: &[4, 5],
    }
}

/// MUX picks 0 for a non-zero remainder and 6 otherwise.
const STRIPES_MUX: Source = Source::Graph(|| {
    circuit(
        &[
            (N::X, &[]),
            (N::Y, &[]),
            (N::Frame, &[]),
            (N::Add, &[]),
            (N::Add, &[]),
            (N::And, &[(1, 3)]),
            (N::Mux, &[(1, 0), (2, 6)]),
        ],
        &[
            (1, 4, 0),
            (2, 4, 1),
            (4, 5, 0),
            (3, 5, 1),
            (5, 6, 0),
            (6, 7, 0),
            (7, 0, 0),
        ],
    )
});

/// A=B then MUL: one node more than the MUX.
#[cfg(test)]
const STRIPES_EQ: Source = Source::Graph(|| {
    circuit(
        &[
            (N::X, &[]),
            (N::Y, &[]),
            (N::Frame, &[]),
            (N::Add, &[]),
            (N::Add, &[]),
            (N::And, &[(1, 3)]),
            (N::Eq, &[(1, 0)]),
            (N::Mul, &[(1, 6)]),
        ],
        &[
            (1, 4, 0),
            (2, 4, 1),
            (4, 5, 0),
            (3, 5, 1),
            (5, 6, 0),
            (6, 7, 0),
            (7, 8, 0),
            (8, 0, 0),
        ],
    )
});

/// Whether the camel covers bulb (x, y) at this frame.
fn camel(frame: i32, x: i32, y: i32) -> bool {
    (x - frame) & 12 == 0 && y & 12 == 4
}

fn camel_walk() -> Open {
    Open {
        task: Task {
            title: Tr::new("The Walking Camel", "走路的骆驼"),
            brief: Tr::new(
                "A 4×4 camel walks along rows 4 to 7 of the sign, one bulb per frame, and wraps around.\nIt covers the bulbs where (X − CLOCK) AND 12 is 0 and Y AND 12 is 4. Camel bulbs are colour 10, the rest 0.\nScore: cycles. Every instruction runs on all 8 warps, and a compare or MUX costs two.",
                "一只 4×4 的骆驼沿着招牌的第 4 到 7 行走，每帧一格，走到头再绕回来。\n它覆盖 (X − CLOCK) AND 12 为 0 且 Y AND 12 为 4 的灯泡。骆驼是颜色 10，其余为 0。\n得分：周期数。每条指令都要在 8 个 warp 上各跑一次，比较和 MUX 各算两条。",
            ),
            cases: || frames(&[0, 5, 14]),
            hidden: || frames(&[7, 19, 61]),
            expect: |c| screen(|x, y| if camel(c.frame, x, y) { 10 } else { 0 }),
            reference: CAMEL_KEY,
            ..base("camel-walk", 10)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[64, 72, 80],
    }
}

/// Both tests fold into one key: (X − CLOCK) AND 12 plus (ADDR − 64) AND 192 is 0
/// only on the camel.
const CAMEL_KEY: Source = Source::Graph(|| {
    circuit(
        &[
            (N::X, &[]),
            (N::Frame, &[]),
            (N::Tid, &[]),
            (N::Sub, &[]),
            (N::And, &[(1, 12)]),
            (N::Sub, &[(1, 64)]),
            (N::And, &[(1, 192)]),
            (N::Add, &[]),
            (N::Mux, &[(1, 0), (2, 10)]),
        ],
        &[
            (1, 4, 0),
            (2, 4, 1),
            (4, 5, 0),
            (3, 6, 0),
            (6, 7, 0),
            (5, 8, 0),
            (7, 8, 1),
            (8, 9, 0),
            (9, 0, 0),
        ],
    )
});

/// Two MUXes, one per test.
#[cfg(test)]
const CAMEL_MUXES: Source = Source::Graph(|| {
    circuit(
        &[
            (N::X, &[]),
            (N::Frame, &[]),
            (N::Y, &[]),
            (N::Sub, &[]),
            (N::And, &[(1, 12)]),
            (N::And, &[(1, 12)]),
            (N::Sub, &[(1, 4)]),
            (N::Mux, &[(1, 0), (2, 10)]),
            (N::Mux, &[(1, 0)]),
        ],
        &[
            (1, 4, 0),
            (2, 4, 1),
            (4, 5, 0),
            (3, 6, 0),
            (6, 7, 0),
            (7, 8, 0),
            (5, 9, 0),
            (8, 9, 2),
            (9, 0, 0),
        ],
    )
});

/// Two A=B tests multiplied together, then by the colour.
#[cfg(test)]
const CAMEL_EQ: Source = Source::Graph(|| {
    circuit(
        &[
            (N::X, &[]),
            (N::Frame, &[]),
            (N::Y, &[]),
            (N::Sub, &[]),
            (N::And, &[(1, 12)]),
            (N::Eq, &[(1, 0)]),
            (N::And, &[(1, 12)]),
            (N::Eq, &[(1, 4)]),
            (N::Mul, &[]),
            (N::Mul, &[(1, 10)]),
        ],
        &[
            (1, 4, 0),
            (2, 4, 1),
            (4, 5, 0),
            (5, 6, 0),
            (3, 7, 0),
            (7, 8, 0),
            (6, 9, 0),
            (8, 9, 1),
            (9, 10, 0),
            (10, 0, 0),
        ],
    )
});

fn sign_tiles() -> Open {
    Open {
        task: Task {
            title: Tr::new("Rolling Tiles", "滚动的方格"),
            brief: Tr::new(
                "The sign shows a checkerboard of 4×4 tiles that rolls down one row per frame.\nA bulb is colour 14 when (X SHR 2) + ((Y + CLOCK) SHR 2) is odd, and colour 2 when it is even.\nScore: compute nodes in your circuit. Fewer is better.",
                "招牌上是一张 4×4 方格的棋盘，每帧向下滚一行。\n当 (X SHR 2) + ((Y + CLOCK) SHR 2) 为奇数时灯泡是颜色 14，为偶数时是颜色 2。\n得分：电路里的计算节点数，越少越好。",
            ),
            cases: || frames(&[0, 2, 5]),
            hidden: || frames(&[11, 30, 57]),
            expect: |c| {
                screen(|x, y| {
                    if ((x >> 2) + ((y + c.frame) >> 2)) & 1 == 1 {
                        14
                    } else {
                        2
                    }
                })
            },
            reference: TILES_BITS,
            ..base("sign-tiles", 11)
        },
        metric: Metric::Size,
        score: None,
        score_label: None,
        marks: &[5, 6],
    }
}

/// Bit 2 of X against bit 2 of Y + CLOCK: the difference is non-zero when they differ.
const TILES_BITS: Source = Source::Graph(|| {
    circuit(
        &[
            (N::X, &[]),
            (N::Y, &[]),
            (N::Frame, &[]),
            (N::Add, &[]),
            (N::And, &[(1, 4)]),
            (N::And, &[(1, 4)]),
            (N::Sub, &[]),
            (N::Mux, &[(1, 14), (2, 2)]),
        ],
        &[
            (2, 4, 0),
            (3, 4, 1),
            (1, 5, 0),
            (4, 6, 0),
            (5, 7, 0),
            (6, 7, 1),
            (7, 8, 0),
            (8, 0, 0),
        ],
    )
});

/// The formula as written: two shifts, a sum and its low bit.
#[cfg(test)]
const TILES_SHR: Source = Source::Graph(|| {
    circuit(
        &[
            (N::X, &[]),
            (N::Y, &[]),
            (N::Frame, &[]),
            (N::Add, &[]),
            (N::Shr, &[(1, 2)]),
            (N::Shr, &[(1, 2)]),
            (N::Add, &[]),
            (N::And, &[(1, 1)]),
            (N::Mux, &[(1, 14), (2, 2)]),
        ],
        &[
            (2, 4, 0),
            (3, 4, 1),
            (1, 5, 0),
            (4, 6, 0),
            (5, 7, 0),
            (6, 7, 1),
            (7, 8, 0),
            (8, 9, 0),
            (9, 0, 0),
        ],
    )
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grade::grade;

    #[test]
    fn alternatives_pass_and_score_on_marks() {
        let alts: [&[Source]; 3] = [&[STRIPES_EQ], &[CAMEL_MUXES, CAMEL_EQ], &[TILES_SHR]];
        for (o, alts) in problems().iter().zip(alts) {
            for a in alts {
                let rep = grade(&o.task, &a.solution()).unwrap();
                let s = o.score_of(&rep);
                assert!(
                    s.is_some_and(|s| o.marks.contains(&s)),
                    "{}: {s:?}",
                    o.task.id
                );
            }
        }
    }
}
