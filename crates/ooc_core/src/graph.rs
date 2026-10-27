//! Blueprint graphs: the Grid Hall's wiring editor. A graph is a per-pixel
//! datapath — every pixel runs the same circuit with its own `x`, `y` —
//! and it compiles to OC-1, so the player can see their wiring as code.

use serde::{Deserialize, Serialize};

use crate::isa::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    X,
    Y,
    Tid,
    Frame,
    Add,
    Sub,
    Mul,
    And,
    Shr,
    Lt,
    Ge,
    Eq,
    Mux,
    Out,
}

impl NodeKind {
    pub const PALETTE: [NodeKind; 13] = [
        NodeKind::X,
        NodeKind::Y,
        NodeKind::Tid,
        NodeKind::Frame,
        NodeKind::Add,
        NodeKind::Sub,
        NodeKind::Mul,
        NodeKind::And,
        NodeKind::Shr,
        NodeKind::Lt,
        NodeKind::Ge,
        NodeKind::Eq,
        NodeKind::Mux,
    ];

    pub fn arity(self) -> usize {
        match self {
            NodeKind::X | NodeKind::Y | NodeKind::Tid | NodeKind::Frame => 0,
            NodeKind::Out => 1,
            NodeKind::Mux => 3,
            _ => 2,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            NodeKind::X => "X",
            NodeKind::Y => "Y",
            NodeKind::Tid => "ADDR",
            NodeKind::Frame => "CLOCK",
            NodeKind::Add => "ADD",
            NodeKind::Sub => "SUB",
            NodeKind::Mul => "MUL",
            NodeKind::And => "AND",
            NodeKind::Shr => "SHR",
            NodeKind::Lt => "A<B",
            NodeKind::Ge => "A≥B",
            NodeKind::Eq => "A=B",
            NodeKind::Mux => "MUX",
            NodeKind::Out => "PIXEL",
        }
    }

    /// Short port names, top to bottom.
    pub fn ports(self) -> &'static [&'static str] {
        match self {
            NodeKind::Mux => &["?", "1", "0"],
            NodeKind::Out => &["C"],
            k if k.arity() == 2 => &["A", "B"],
            _ => &[],
        }
    }

    pub fn is_source(self) -> bool {
        self.arity() == 0
    }

    fn special(self) -> Option<Special> {
        match self {
            NodeKind::X => Some(Special::X),
            NodeKind::Y => Some(Special::Y),
            NodeKind::Tid => Some(Special::Tid),
            NodeKind::Frame => Some(Special::Frame),
            _ => None,
        }
    }
}

/// An input port: wired to another node, or a constant when unwired.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Port {
    pub from: Option<usize>,
    pub value: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub kind: NodeKind,
    pub pos: [f32; 2],
    pub inputs: Vec<Port>,
}

impl Node {
    pub fn new(kind: NodeKind, pos: [f32; 2]) -> Self {
        Self {
            kind,
            pos,
            inputs: vec![Port::default(); kind.arity()],
        }
    }

    pub fn with(mut self, port: usize, value: i32) -> Self {
        self.inputs[port].value = value;
        self
    }
}

/// Node 0 is always the single `Out` node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Graph {
    pub nodes: Vec<Node>,
}

pub const MAX_NODES: usize = 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraphError {
    Cycle(usize),
    TooManyNodes,
    Missing,
}

/// Compiled graph: the program plus which register each node landed in.
#[derive(Clone, Debug)]
pub struct Compiled {
    pub program: Program,
    pub reg: Vec<Option<u8>>,
    /// Compute nodes reachable from the output (the "size" of the circuit).
    pub used: usize,
}

impl Graph {
    pub fn new(out_pos: [f32; 2]) -> Self {
        Self {
            nodes: vec![Node::new(NodeKind::Out, out_pos)],
        }
    }

    pub fn push(&mut self, node: Node) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    /// Wires `from`'s output into `to`'s port. Refuses self-loops and cycles.
    pub fn connect(&mut self, from: usize, to: usize, port: usize) -> bool {
        if from >= self.nodes.len() || to >= self.nodes.len() || port >= self.nodes[to].inputs.len()
        {
            return false;
        }
        if self.nodes[from].kind == NodeKind::Out || self.reaches(to, from) {
            return false;
        }
        self.nodes[to].inputs[port].from = Some(from);
        true
    }

    /// Does `a` feed (transitively) into `b`? `reaches(x, x)` is true.
    pub fn reaches(&self, a: usize, b: usize) -> bool {
        let mut stack = vec![b];
        let mut seen = vec![false; self.nodes.len()];
        while let Some(n) = stack.pop() {
            if n == a {
                return true;
            }
            if std::mem::replace(&mut seen[n], true) {
                continue;
            }
            stack.extend(self.nodes[n].inputs.iter().filter_map(|p| p.from));
        }
        false
    }

    /// Removes a node (never the output) and every wire touching it.
    pub fn remove(&mut self, idx: usize) {
        if idx == 0 || idx >= self.nodes.len() {
            return;
        }
        self.nodes.remove(idx);
        for n in &mut self.nodes {
            for p in &mut n.inputs {
                p.from = match p.from {
                    Some(f) if f == idx => None,
                    Some(f) if f > idx => Some(f - 1),
                    other => other,
                };
            }
        }
    }

    pub fn compile(&self) -> Result<Compiled, GraphError> {
        if self.nodes.first().map(|n| n.kind) != Some(NodeKind::Out) {
            return Err(GraphError::Missing);
        }
        if self.nodes.len() > MAX_NODES {
            return Err(GraphError::TooManyNodes);
        }
        // Post-order DFS from the output; 1 = visiting, 2 = done.
        let mut state = vec![0u8; self.nodes.len()];
        let mut order = Vec::new();
        fn visit(
            g: &Graph,
            n: usize,
            state: &mut [u8],
            order: &mut Vec<usize>,
        ) -> Result<(), GraphError> {
            match state[n] {
                2 => return Ok(()),
                1 => return Err(GraphError::Cycle(n)),
                _ => {}
            }
            state[n] = 1;
            for p in &g.nodes[n].inputs {
                if let Some(f) = p.from {
                    visit(g, f, state, order)?;
                }
            }
            state[n] = 2;
            order.push(n);
            Ok(())
        }
        visit(self, 0, &mut state, &mut order)?;

        let mut reg = vec![None; self.nodes.len()];
        let mut code = Vec::new();
        let mut next = 0u8;
        let src = |p: &Port, reg: &[Option<u8>]| -> Src {
            match p.from {
                Some(f) => match self.nodes[f].kind.special() {
                    Some(s) => Src::Sp(s),
                    None => Src::Reg(reg[f].unwrap_or(0)),
                },
                None => Src::Imm(p.value),
            }
        };
        for &n in &order {
            let node = &self.nodes[n];
            if node.kind.is_source() {
                continue;
            }
            let ins: Vec<Src> = node.inputs.iter().map(|p| src(p, &reg)).collect();
            if node.kind == NodeKind::Out {
                code.push(Instr::Pix { v: ins[0] });
                continue;
            }
            if next >= REGS {
                return Err(GraphError::TooManyNodes);
            }
            let d = next;
            next += 1;
            reg[n] = Some(d);
            let alu = |op| Instr::Alu {
                op,
                d,
                a: ins[0],
                b: ins[1],
            };
            let cmp = |cmp| Instr::SetP {
                cmp,
                p: 0,
                a: ins[0],
                b: ins[1],
            };
            let one_zero = Instr::Sel {
                d,
                p: 0,
                a: Src::Imm(1),
                b: Src::Imm(0),
            };
            match node.kind {
                NodeKind::Add => code.push(alu(Alu::Add)),
                NodeKind::Sub => code.push(alu(Alu::Sub)),
                NodeKind::Mul => code.push(alu(Alu::Mul)),
                NodeKind::And => code.push(alu(Alu::And)),
                NodeKind::Shr => code.push(alu(Alu::Shr)),
                NodeKind::Lt => code.extend([cmp(Cmp::Lt), one_zero]),
                NodeKind::Ge => code.extend([cmp(Cmp::Ge), one_zero]),
                NodeKind::Eq => code.extend([cmp(Cmp::Eq), one_zero]),
                NodeKind::Mux => code.extend([
                    Instr::SetP {
                        cmp: Cmp::Ne,
                        p: 0,
                        a: ins[0],
                        b: Src::Imm(0),
                    },
                    Instr::Sel {
                        d,
                        p: 0,
                        a: ins[1],
                        b: ins[2],
                    },
                ]),
                _ => unreachable!("sources and output handled above"),
            }
        }
        let used = next as usize;
        let program = Program::new(code).map_err(|_| GraphError::TooManyNodes)?;
        Ok(Compiled { program, reg, used })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::{Launch, Profile, run_frame};

    fn eval(g: &Graph, frame: i32) -> Vec<i32> {
        let c = g.compile().unwrap();
        let mut out = vec![0; FB_LEN];
        run_frame(
            &c.program,
            &[],
            Launch::SCREEN,
            Profile::LUMEN,
            frame,
            &mut out,
        );
        out
    }

    #[test]
    fn unwired_output_is_a_constant() {
        let mut g = Graph::new([0.0, 0.0]);
        g.nodes[0].inputs[0].value = 9;
        assert!(eval(&g, 0).iter().all(|v| *v == 9));
    }

    #[test]
    fn compiles_arithmetic_compares_and_mux() {
        let mut g = Graph::new([0.0, 0.0]);
        let x = g.push(Node::new(NodeKind::X, [0.0, 0.0]));
        let lt = g.push(Node::new(NodeKind::Lt, [0.0, 0.0]).with(1, 8));
        let mux = g.push(Node::new(NodeKind::Mux, [0.0, 0.0]).with(1, 3).with(2, 5));
        assert!(g.connect(x, lt, 0) && g.connect(lt, mux, 0) && g.connect(mux, 0, 0));
        let c = g.compile().unwrap();
        assert_eq!(c.used, 2);
        assert_eq!(c.reg[x], None, "sources read specials, not registers");
        let out = eval(&g, 0);
        assert_eq!((out[7], out[8], out[16 + 2]), (3, 5, 3));
    }

    #[test]
    fn refuses_cycles_and_rewires_on_remove() {
        let mut g = Graph::new([0.0, 0.0]);
        let a = g.push(Node::new(NodeKind::Add, [0.0, 0.0]));
        let b = g.push(Node::new(NodeKind::Add, [0.0, 0.0]));
        assert!(g.connect(a, b, 0));
        assert!(!g.connect(b, a, 0), "would close a loop");
        assert!(!g.connect(a, a, 1), "self-loop");
        assert!(!g.connect(0, a, 0), "output has no output");
        assert!(g.connect(b, 0, 0));
        g.remove(a);
        assert_eq!(g.nodes.len(), 2);
        assert_eq!(g.nodes[1].inputs[0].from, None);
        assert_eq!(g.nodes[0].inputs[0].from, Some(1));
    }

    #[test]
    fn unreachable_nodes_cost_nothing() {
        let mut g = Graph::new([0.0, 0.0]);
        g.push(Node::new(NodeKind::Mul, [0.0, 0.0]));
        assert_eq!(g.compile().unwrap().used, 0);
    }

    #[test]
    fn serialises() {
        let mut g = Graph::new([1.0, 2.0]);
        let y = g.push(Node::new(NodeKind::Y, [0.0, 0.0]));
        g.connect(y, 0, 0);
        let back: Graph = serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
        assert_eq!(back, g);
    }
}
