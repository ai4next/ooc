//! Microcode back to a blueprint: the other half of the chapter-1 switch.
//! Straight-line code made only of blueprint parts becomes one node per
//! value, wired by which instruction last wrote each register.

use oo_base::{Lang, Tr};

use super::{Graph, MAX_NODES, Node, NodeKind, Port};
use crate::isa::*;

/// Why a program cannot be drawn as a blueprint. Lines are instruction
/// indices, 0-based.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unfit {
    /// IF/ELSE/ENDIF/LOOP/ENDLOOP: a blueprint has no control flow.
    Branch(usize),
    /// An instruction with no blueprint part (MOV, OR, LD, ...).
    NoPart(usize, OpKind),
    /// A special register the blueprint has no input for (lane, warp, ...).
    NoSource(usize, Special),
    /// A register read before any instruction wrote it.
    Unset(usize, u8),
    /// A SEL on a predicate no SETP has set.
    NoPred(usize, u8),
    /// No PIX: the blueprint's PIXEL would be left unwired.
    NoOutput,
    /// A second PIX; a blueprint has one PIXEL.
    TwoOutputs(usize),
    /// More parts than a board holds.
    TooMany(usize),
}

impl Unfit {
    /// One sentence for the player, saying what stops the switch.
    pub fn text(self, l: Lang) -> String {
        match self {
            Unfit::Branch(i) => Tr::new(
                "Line {}: the blueprint has no parts for IF or LOOP.",
                "第 {} 行：蓝图没有 IF 或 LOOP 的部件。",
            )
            .fill(l, &[&(i + 1)]),
            Unfit::NoPart(i, op) => Tr::new(
                "Line {}: {} has no blueprint part.",
                "第 {} 行：{} 没有对应的蓝图部件。",
            )
            .fill(l, &[&(i + 1), &op.mnemonic()]),
            Unfit::NoSource(i, s) => Tr::new(
                "Line {}: the blueprint has no {} input.",
                "第 {} 行：蓝图没有 {} 输入。",
            )
            .fill(l, &[&(i + 1), &s.name()]),
            Unfit::Unset(i, r) => Tr::new(
                "Line {}: r{} is read before anything writes it.",
                "第 {} 行：r{} 还没被写入就被读取。",
            )
            .fill(l, &[&(i + 1), &r]),
            Unfit::NoPred(i, p) => Tr::new(
                "Line {}: p{} is used before a SETP sets it.",
                "第 {} 行：p{} 在 SETP 设置之前就被使用。",
            )
            .fill(l, &[&(i + 1), &p]),
            Unfit::NoOutput => Tr::new(
                "The program never writes its pixel (PIX).",
                "程序没有写出像素（PIX）。",
            )
            .get(l)
            .to_string(),
            Unfit::TwoOutputs(i) => Tr::new(
                "Line {}: a second PIX; a blueprint has one PIXEL.",
                "第 {} 行：第二个 PIX；蓝图只有一个 PIXEL。",
            )
            .fill(l, &[&(i + 1)]),
            Unfit::TooMany(n) => Tr::new(
                "This needs {} parts; a board holds at most {}.",
                "这需要 {} 个部件，电路板最多 {} 个。",
            )
            .fill(l, &[&n, &MAX_NODES]),
        }
    }
}

/// The microcode ops that match a level's blueprint parts: the same
/// instructions [`Graph::compile`] emits for them.
pub fn code_ops(kinds: &[NodeKind]) -> Vec<OpKind> {
    let mut ops = Vec::new();
    for k in kinds {
        let more: &[OpKind] = match k {
            NodeKind::Add => &[OpKind::Alu(Alu::Add)],
            NodeKind::Sub => &[OpKind::Alu(Alu::Sub)],
            NodeKind::Mul => &[OpKind::Alu(Alu::Mul)],
            NodeKind::And => &[OpKind::Alu(Alu::And)],
            NodeKind::Shr => &[OpKind::Alu(Alu::Shr)],
            NodeKind::Lt | NodeKind::Ge | NodeKind::Eq | NodeKind::Mux => {
                &[OpKind::SetP, OpKind::Sel]
            }
            _ => &[],
        };
        for op in more {
            if !ops.contains(op) {
                ops.push(*op);
            }
        }
    }
    ops.push(OpKind::Pix);
    ops
}

fn part(op: Alu) -> Option<NodeKind> {
    Some(match op {
        Alu::Add => NodeKind::Add,
        Alu::Sub => NodeKind::Sub,
        Alu::Mul => NodeKind::Mul,
        Alu::And => NodeKind::And,
        Alu::Shr => NodeKind::Shr,
        _ => return None,
    })
}

fn source_kind(s: Special) -> Option<NodeKind> {
    Some(match s {
        Special::X => NodeKind::X,
        Special::Y => NodeKind::Y,
        Special::Tid => NodeKind::Tid,
        Special::Frame => NodeKind::Frame,
        _ => return None,
    })
}

/// What a predicate holds: the SETP's comparison with its operands as they
/// were when it ran, and the compare node made for it so far (one per kind).
struct Pred {
    cmp: Cmp,
    a: Port,
    b: Port,
    nodes: Vec<(NodeKind, usize)>,
}

fn imm(v: i32) -> Port {
    Port { from: None, value: v }
}

fn wire(n: usize) -> Port {
    Port {
        from: Some(n),
        value: 0,
    }
}

fn is_imm(p: Port, v: i32) -> bool {
    p.from.is_none() && p.value == v
}

struct Builder {
    g: Graph,
    regs: [Option<usize>; REGS as usize],
    sources: Vec<(Special, usize)>,
}

impl Builder {
    fn port(&mut self, line: usize, s: Src) -> Result<Port, Unfit> {
        match s {
            Src::Imm(v) => Ok(imm(v)),
            Src::Reg(r) => self.regs[r as usize]
                .map(wire)
                .ok_or(Unfit::Unset(line, r)),
            Src::Sp(sp) => {
                if let Some(&(_, n)) = self.sources.iter().find(|(s, _)| *s == sp) {
                    return Ok(wire(n));
                }
                let kind = source_kind(sp).ok_or(Unfit::NoSource(line, sp))?;
                let n = self.g.push(Node::new(kind, [0.0, 0.0]));
                self.sources.push((sp, n));
                Ok(wire(n))
            }
        }
    }

    fn node(&mut self, kind: NodeKind, inputs: Vec<Port>) -> usize {
        let mut n = Node::new(kind, [0.0, 0.0]);
        n.inputs = inputs;
        self.g.push(n)
    }

    /// The node for `pred` read as `kind(a, b)`, shared by every SEL of it.
    fn compare(&mut self, pred: &mut Pred, kind: NodeKind, a: Port, b: Port) -> usize {
        if let Some(&(_, n)) = pred.nodes.iter().find(|(k, _)| *k == kind) {
            return n;
        }
        let n = self.node(kind, vec![a, b]);
        pred.nodes.push((kind, n));
        n
    }

    /// `SEL d, p, x, y`: the node whose value is `p ? x : y`.
    fn select(&mut self, pred: &mut Pred, x: Port, y: Port) -> usize {
        let (a, b) = (pred.a, pred.b);
        let one_zero = is_imm(x, 1) && is_imm(y, 0);
        // The forms the blueprint compiles to, read back as written.
        match pred.cmp {
            Cmp::Lt if one_zero => return self.compare(pred, NodeKind::Lt, a, b),
            Cmp::Ge if one_zero => return self.compare(pred, NodeKind::Ge, a, b),
            Cmp::Eq if one_zero => return self.compare(pred, NodeKind::Eq, a, b),
            Cmp::Ne if is_imm(b, 0) => return self.node(NodeKind::Mux, vec![a, x, y]),
            _ => {}
        }
        // Other comparisons as one of the three compare parts: `Gt` is `Lt`
        // with the operands swapped, `Le` is `Ge` swapped, and `Ne` is `Eq`
        // with the arms swapped.
        let (kind, a, b, x, y) = match pred.cmp {
            Cmp::Lt => (NodeKind::Lt, a, b, x, y),
            Cmp::Ge => (NodeKind::Ge, a, b, x, y),
            Cmp::Eq => (NodeKind::Eq, a, b, x, y),
            Cmp::Gt => (NodeKind::Lt, b, a, x, y),
            Cmp::Le => (NodeKind::Ge, b, a, x, y),
            Cmp::Ne => (NodeKind::Eq, a, b, y, x),
        };
        if is_imm(x, 1) && is_imm(y, 0) {
            return self.compare(pred, kind, a, b);
        }
        if is_imm(x, 0) && is_imm(y, 1) && kind != NodeKind::Eq {
            let flip = if kind == NodeKind::Lt {
                NodeKind::Ge
            } else {
                NodeKind::Lt
            };
            return self.compare(pred, flip, a, b);
        }
        if kind == NodeKind::Eq && (is_imm(a, 0) || is_imm(b, 0)) {
            let v = if is_imm(b, 0) { a } else { b };
            return self.node(NodeKind::Mux, vec![v, y, x]);
        }
        let c = self.compare(pred, kind, a, b);
        self.node(NodeKind::Mux, vec![wire(c), x, y])
    }
}

/// Reads straight-line code made of blueprint parts back into a blueprint:
/// one node per value, wires from the instruction that last wrote each
/// register, constants as port values, laid out in columns by depth.
pub fn decompile(code: &[Instr]) -> Result<Graph, Unfit> {
    let mut b = Builder {
        g: Graph::new([0.0, 0.0]),
        regs: [None; REGS as usize],
        sources: Vec::new(),
    };
    let mut preds: Vec<Option<Pred>> = (0..PREDS).map(|_| None).collect();
    let mut out = false;
    for (i, ins) in code.iter().enumerate() {
        match *ins {
            Instr::Alu { op, d, a, b: rhs } => {
                let kind = part(op).ok_or(Unfit::NoPart(i, ins.kind()))?;
                let inputs = vec![b.port(i, a)?, b.port(i, rhs)?];
                let n = b.node(kind, inputs);
                b.regs[d as usize] = Some(n);
            }
            Instr::SetP { cmp, p, a, b: rhs } => {
                let (a, rhs) = (b.port(i, a)?, b.port(i, rhs)?);
                preds[p as usize] = Some(Pred {
                    cmp,
                    a,
                    b: rhs,
                    nodes: Vec::new(),
                });
            }
            Instr::Sel { d, p, a, b: rhs } => {
                let (x, y) = (b.port(i, a)?, b.port(i, rhs)?);
                let pred = preds[p as usize].as_mut().ok_or(Unfit::NoPred(i, p))?;
                let n = b.select(pred, x, y);
                b.regs[d as usize] = Some(n);
            }
            Instr::Pix { v } => {
                if out {
                    return Err(Unfit::TwoOutputs(i));
                }
                out = true;
                b.g.nodes[0].inputs[0] = b.port(i, v)?;
            }
            Instr::If { .. } | Instr::Else | Instr::EndIf | Instr::Loop { .. } | Instr::EndLoop => {
                return Err(Unfit::Branch(i));
            }
            _ => return Err(Unfit::NoPart(i, ins.kind())),
        }
    }
    if !out {
        return Err(Unfit::NoOutput);
    }
    let mut g = b.g;
    if g.nodes.len() > MAX_NODES {
        return Err(Unfit::TooMany(g.nodes.len()));
    }
    layout(&mut g);
    Ok(g)
}

/// Where the PIXEL sits, as in the chapter-1 boards.
const OUT_X: f32 = 340.0;
/// Leftmost column and the widest gap between columns (graph units).
const LEFT_X: f32 = -420.0;
const MAX_PITCH: f32 = 170.0;
/// Rows stay within ±`HALF_H` so the tallest part fits the canvas.
const HALF_H: f32 = 105.0;
const ROW_PITCH: f32 = 90.0;

/// Columns by depth (sources first, PIXEL last), rows ordered by where
/// their inputs sit, crowded columns staggered.
fn layout(g: &mut Graph) {
    let n = g.nodes.len();
    // Nodes are created after their inputs, so one pass in index order
    // settles every depth; the PIXEL (node 0) goes last.
    let mut depth = vec![0usize; n];
    for i in (1..n).chain([0]) {
        depth[i] = g.nodes[i]
            .inputs
            .iter()
            .filter_map(|p| p.from)
            .map(|f| depth[f] + 1)
            .max()
            .unwrap_or(usize::from(!g.nodes[i].kind.is_source()));
    }
    let last = depth.iter().copied().max().unwrap_or(0).max(1);
    depth[0] = last;
    for d in depth.iter_mut().skip(1) {
        *d = (*d).min(last - 1);
    }
    let pitch = ((OUT_X - LEFT_X) / last as f32).min(MAX_PITCH);
    let mut ys = vec![0.0f32; n];
    for col in 0..=last {
        let mut members: Vec<usize> = (0..n).filter(|&i| depth[i] == col).collect();
        let centre = |i: usize, ys: &[f32]| {
            let from: Vec<f32> = g.nodes[i]
                .inputs
                .iter()
                .filter_map(|p| p.from)
                .map(|f| ys[f])
                .collect();
            if from.is_empty() {
                0.0
            } else {
                from.iter().sum::<f32>() / from.len() as f32
            }
        };
        // Highest inputs first; sources keep their order of first use.
        members.sort_by(|&a, &b| centre(b, &ys).total_cmp(&centre(a, &ys)));
        let k = members.len();
        let step = if k > 1 {
            (2.0 * HALF_H / (k - 1) as f32).min(ROW_PITCH)
        } else {
            0.0
        };
        let crowded = step < ROW_PITCH * 0.8;
        let x = OUT_X - (last - col) as f32 * pitch;
        for (r, &i) in members.iter().enumerate() {
            let y = if col == last {
                0.0
            } else {
                (k - 1) as f32 * step / 2.0 - r as f32 * step
            };
            ys[i] = y;
            let nudge = if crowded && col != last {
                if r % 2 == 0 { -pitch * 0.22 } else { pitch * 0.22 }
            } else {
                0.0
            };
            g.nodes[i].pos = [(x + nudge).max(LEFT_X), y];
        }
    }
}

impl Graph {
    /// The board as microcode, each part's first instruction headed by a
    /// comment naming the part.
    pub fn source(&self) -> Result<String, super::GraphError> {
        let c = self.compile()?;
        let code = &c.program.code;
        let mut notes: Vec<Option<String>> = code
            .iter()
            .map(|i| matches!(i, Instr::Pix { .. }).then(|| NodeKind::Out.label().to_string()))
            .collect();
        // Each part is labelled at its first instruction: the SETP for compares.
        for (n, r) in c.reg.iter().enumerate() {
            let Some(r) = *r else { continue };
            let Some(at) = code.iter().position(|i| i.dest() == Some(r)) else {
                continue;
            };
            let at = match (code[at], at.checked_sub(1).map(|p| code[p])) {
                (Instr::Sel { .. }, Some(Instr::SetP { .. })) => at - 1,
                _ => at,
            };
            notes[at] = Some(self.nodes[n].kind.label().to_string());
        }
        let doc = crate::edit::Doc {
            code: c.program.code.clone(),
            notes,
            ..Default::default()
        };
        Ok(doc.source())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::assemble;
    use crate::grade::{BuildError, grade};
    use crate::task::{Editor, Solution};

    fn back(src: &str) -> Result<Graph, Unfit> {
        decompile(&assemble(src).unwrap().code)
    }

    #[test]
    fn every_chapter_one_board_round_trips() {
        let tasks: Vec<_> = crate::content::tasks()
            .iter()
            .filter(|t| matches!(t.editor, Editor::Graph(_)))
            .collect();
        assert!(tasks.len() >= 8);
        for t in tasks {
            for (what, src) in [("reference", t.reference), ("starter", t.starter)] {
                let Solution::Graph(g) = src.solution() else {
                    panic!("{}: {what} is not a board", t.id)
                };
                let Ok(code) = g.source() else { continue };
                let again = back(&code).unwrap_or_else(|e| panic!("{} {what}: {e:?}", t.id));
                let r = grade(t, &Solution::Graph(g.clone()));
                let forms = [Solution::Code(code.clone()), Solution::Graph(again)];
                for form in forms {
                    let s = grade(t, &form);
                    match (&r, &s) {
                        (Ok(a), Ok(b)) => {
                            assert_eq!(a.verdict, b.verdict, "{} {what}", t.id);
                            assert_eq!(a.measure, b.measure, "{} {what}", t.id);
                        }
                        (a, b) => assert_eq!(
                            a.as_ref().err(),
                            b.as_ref().err(),
                            "{} {what}",
                            t.id
                        ),
                    }
                }
            }
        }
    }

    #[test]
    fn reads_other_comparisons_as_the_three_parts() {
        // x > 8 is 8 < x; x != 3 ? 5 : 9 is x = 3 ? 9 : 5.
        let g = back("SETP.GT p0, x, 8\nSEL r0, p0, 1, 0\nPIX r0").unwrap();
        assert_eq!(g.nodes[2].kind, NodeKind::Lt);
        assert_eq!(g.nodes[2].inputs[0], imm(8));
        let g = back("SETP.NE p1, y, 3\nSEL r2, p1, 5, 9\nPIX r2").unwrap();
        let kinds: Vec<_> = g.nodes.iter().map(|n| n.kind).collect();
        assert_eq!(kinds, [NodeKind::Out, NodeKind::Y, NodeKind::Eq, NodeKind::Mux]);
        assert_eq!(g.nodes[3].inputs[1], imm(9));
        // Two SELs of one predicate share its compare part.
        let g = back("SETP.LT p0, x, 4\nSEL r0, p0, 2, 3\nSEL r1, p0, 6, 7\nADD r2, r0, r1\nPIX r2")
            .unwrap();
        assert_eq!(g.nodes.iter().filter(|n| n.kind == NodeKind::Lt).count(), 1);
        assert_eq!(g.compile().unwrap().used, 4);
    }

    #[test]
    fn says_why_code_cannot_be_a_board() {
        let cases = [
            ("IF p0\nENDIF\nPIX 1", Unfit::Branch(0)),
            ("MOV r0, x\nPIX r0", Unfit::NoPart(0, OpKind::Mov)),
            ("OR r0, x, 1\nPIX r0", Unfit::NoPart(0, OpKind::Alu(Alu::Or))),
            ("ADD r0, lane, 1\nPIX r0", Unfit::NoSource(0, Special::Lane)),
            ("ADD r0, x, 1\nPIX r3", Unfit::Unset(1, 3)),
            ("SEL r0, p2, 1, 0\nPIX r0", Unfit::NoPred(0, 2)),
            ("ADD r0, x, 1", Unfit::NoOutput),
            ("PIX 1\nPIX 2", Unfit::TwoOutputs(1)),
        ];
        for (src, want) in cases {
            assert_eq!(back(src).unwrap_err(), want, "{src}");
            assert!(!want.text(Lang::Zh).is_empty() && !want.text(Lang::En).contains("{}"));
        }
        let long: String = (0..20).map(|i| format!("ADD r{i}, x, {i}\n")).collect();
        assert_eq!(back(&(long + "PIX r0")).unwrap_err(), Unfit::TooMany(22));
    }

    #[test]
    fn layout_keeps_parts_on_the_canvas() {
        let t = crate::content::task("tide-board").unwrap();
        let Solution::Graph(g) = t.reference.solution() else { panic!() };
        let g = back(&g.source().unwrap()).unwrap();
        assert_eq!(g.nodes[0].pos, [OUT_X, 0.0]);
        for n in &g.nodes {
            assert!((-440.0..=OUT_X).contains(&n.pos[0]) && n.pos[1].abs() <= HALF_H, "{n:?}");
        }
    }

    #[test]
    fn microcode_grades_on_a_board_level_with_its_parts_only() {
        let t = crate::content::task("first-light").unwrap();
        let code = |s: &str| grade(t, &Solution::Code(s.into())).map(|r| r.measure.size);
        assert_eq!(code("SETP.EQ p0, x, 8\nSEL r0, p0, 1, 0\nPIX r0").ok(), Some(1));
        let refused = [
            ("SETP.LT p0, x, 8\nSEL r0, p0, 1, 0\nPIX r0", BuildError::LockedNode(NodeKind::Lt)),
            ("ADD r0, frame, 1\nPIX r0", BuildError::LockedNode(NodeKind::Frame)),
            ("SHR r0, x, 1\nPIX r0", BuildError::LockedOp(OpKind::Alu(Alu::Shr))),
            ("ADD r0, r1, 1\nPIX r0", BuildError::Blueprint(Unfit::Unset(0, 1))),
        ];
        for (src, want) in refused {
            assert_eq!(code(src).unwrap_err(), want, "{src}");
        }
    }
}
