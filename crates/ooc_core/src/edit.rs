//! Structured editing of OC-1 code for the block editor: operands as typed
//! slots, and insert/remove that keep blocks balanced.

use crate::isa::*;

/// One editable operand of an instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operand {
    /// Destination register.
    Reg(u8),
    Pred(u8),
    Src(Src),
    Cmp(Cmp),
    /// LOOP trip count.
    Count(i32),
    /// VOTE mode: `true` = ALL, `false` = ANY.
    Mode(bool),
    /// ALU operation, editable in place.
    Op(Alu),
}

/// The instruction's operands in display order (the ALU op comes first).
pub fn operands(i: &Instr) -> Vec<Operand> {
    use Operand as O;
    match *i {
        Instr::Mov { d, a } => vec![O::Reg(d), O::Src(a)],
        Instr::Alu { op, d, a, b } => vec![O::Op(op), O::Reg(d), O::Src(a), O::Src(b)],
        Instr::SetP { cmp, p, a, b } => vec![O::Cmp(cmp), O::Pred(p), O::Src(a), O::Src(b)],
        Instr::Sel { d, p, a, b } => vec![O::Reg(d), O::Pred(p), O::Src(a), O::Src(b)],
        Instr::If { p } => vec![O::Pred(p)],
        Instr::Loop { n } => vec![O::Count(n)],
        Instr::Ld { d, addr }
        | Instr::Edge { d, addr }
        | Instr::Ldo { d, addr }
        | Instr::Lds { d, addr }
        | Instr::Ldl { d, addr } => vec![O::Reg(d), O::Src(addr)],
        Instr::St { addr, v } | Instr::Sts { addr, v } | Instr::Stl { addr, v } => {
            vec![O::Src(addr), O::Src(v)]
        }
        Instr::Dma { dst, src, n } => vec![O::Src(dst), O::Src(src), O::Src(n)],
        Instr::Send { peer, buf, n } | Instr::Recv { peer, buf, n } => {
            vec![O::Src(peer), O::Src(buf), O::Src(n)]
        }
        Instr::Atom { d, addr, v, .. } => vec![O::Reg(d), O::Src(addr), O::Src(v)],
        Instr::Pix { v } => vec![O::Src(v)],
        Instr::Shfl { d, a, lane } => vec![O::Reg(d), O::Src(a), O::Src(lane)],
        Instr::Vote { all, p, q } => vec![O::Mode(all), O::Pred(p), O::Pred(q)],
        Instr::Rcp { d, a } | Instr::Cvt { d, a, .. } => vec![O::Reg(d), O::Src(a)],
        Instr::Mma { d, a, b, c } => vec![O::Reg(d), O::Reg(a), O::Reg(b), O::Reg(c)],
        Instr::Dot { d, a, b } | Instr::Lerp { d, a, b } => vec![O::Reg(d), O::Reg(a), O::Reg(b)],
        Instr::Else
        | Instr::EndIf
        | Instr::EndLoop
        | Instr::Nop
        | Instr::Bar
        | Instr::Gsync
        | Instr::Fence => Vec::new(),
    }
}

/// Replaces operand `idx`. Returns `false` (and changes nothing) when the
/// slot does not exist or holds a different kind of operand.
pub fn set_operand(i: &mut Instr, idx: usize, v: Operand) -> bool {
    use Operand as O;
    let Some(old) = operands(i).get(idx).copied() else {
        return false;
    };
    if std::mem::discriminant(&old) != std::mem::discriminant(&v) {
        return false;
    }
    let ok = match v {
        O::Reg(r) => r < REGS,
        O::Pred(p) => p < PREDS,
        O::Src(Src::Reg(r)) => r < REGS,
        O::Count(n) => (0..=MAX_LOOP).contains(&n),
        _ => true,
    };
    if !ok {
        return false;
    }
    match (i, idx, v) {
        (Instr::Mov { d, .. }, 0, O::Reg(r)) => *d = r,
        (Instr::Mov { a, .. }, 1, O::Src(s)) => *a = s,
        (Instr::Alu { op, .. }, 0, O::Op(o)) => *op = o,
        (Instr::Alu { d, .. }, 1, O::Reg(r)) => *d = r,
        (Instr::Alu { a, .. }, 2, O::Src(s)) => *a = s,
        (Instr::Alu { b, .. }, 3, O::Src(s)) => *b = s,
        (Instr::SetP { cmp, .. }, 0, O::Cmp(c)) => *cmp = c,
        (Instr::SetP { p, .. }, 1, O::Pred(q)) => *p = q,
        (Instr::SetP { a, .. }, 2, O::Src(s)) => *a = s,
        (Instr::SetP { b, .. }, 3, O::Src(s)) => *b = s,
        (Instr::Sel { d, .. }, 0, O::Reg(r)) => *d = r,
        (Instr::Sel { p, .. }, 1, O::Pred(q)) => *p = q,
        (Instr::Sel { a, .. }, 2, O::Src(s)) => *a = s,
        (Instr::Sel { b, .. }, 3, O::Src(s)) => *b = s,
        (Instr::If { p }, 0, O::Pred(q)) => *p = q,
        (Instr::Loop { n }, 0, O::Count(c)) => *n = c,
        (
            Instr::Ld { d, .. }
            | Instr::Ldo { d, .. }
            | Instr::Lds { d, .. }
            | Instr::Ldl { d, .. },
            0,
            O::Reg(r),
        ) => *d = r,
        (
            Instr::Ld { addr, .. }
            | Instr::Ldo { addr, .. }
            | Instr::Lds { addr, .. }
            | Instr::Ldl { addr, .. },
            1,
            O::Src(s),
        ) => *addr = s,
        (Instr::Atom { d, .. }, 0, O::Reg(r)) => *d = r,
        (Instr::Atom { addr, .. }, 1, O::Src(s)) => *addr = s,
        (Instr::Atom { v, .. }, 2, O::Src(s)) => *v = s,
        (Instr::Edge { d, .. }, 0, O::Reg(r)) => *d = r,
        (Instr::Edge { addr, .. }, 1, O::Src(s)) => *addr = s,
        (Instr::Rcp { d, .. } | Instr::Cvt { d, .. }, 0, O::Reg(r)) => *d = r,
        (Instr::Rcp { a, .. } | Instr::Cvt { a, .. }, 1, O::Src(s)) => *a = s,
        (Instr::Mma { d, .. }, 0, O::Reg(r)) => *d = r,
        (Instr::Mma { a, .. }, 1, O::Reg(r)) => *a = r,
        (Instr::Mma { b, .. }, 2, O::Reg(r)) => *b = r,
        (Instr::Mma { c, .. }, 3, O::Reg(r)) => *c = r,
        (Instr::Dot { d, .. } | Instr::Lerp { d, .. }, 0, O::Reg(r)) => *d = r,
        (Instr::Dot { a, .. } | Instr::Lerp { a, .. }, 1, O::Reg(r)) => *a = r,
        (Instr::Dot { b, .. } | Instr::Lerp { b, .. }, 2, O::Reg(r)) => *b = r,
        (
            Instr::St { addr, .. } | Instr::Sts { addr, .. } | Instr::Stl { addr, .. },
            0,
            O::Src(s),
        ) => *addr = s,
        (Instr::St { v, .. } | Instr::Sts { v, .. } | Instr::Stl { v, .. }, 1, O::Src(s)) => *v = s,
        (Instr::Dma { dst, .. }, 0, O::Src(s)) => *dst = s,
        (Instr::Dma { src, .. }, 1, O::Src(s)) => *src = s,
        (Instr::Dma { n, .. }, 2, O::Src(s)) => *n = s,
        (Instr::Send { peer, .. } | Instr::Recv { peer, .. }, 0, O::Src(s)) => *peer = s,
        (Instr::Send { buf, .. } | Instr::Recv { buf, .. }, 1, O::Src(s)) => *buf = s,
        (Instr::Send { n, .. } | Instr::Recv { n, .. }, 2, O::Src(s)) => *n = s,
        (Instr::Pix { v }, 0, O::Src(s)) => *v = s,
        (Instr::Shfl { d, .. }, 0, O::Reg(r)) => *d = r,
        (Instr::Shfl { a, .. }, 1, O::Src(s)) => *a = s,
        (Instr::Shfl { lane, .. }, 2, O::Src(s)) => *lane = s,
        (Instr::Vote { all, .. }, 0, O::Mode(m)) => *all = m,
        (Instr::Vote { p, .. }, 1, O::Pred(q)) => *p = q,
        (Instr::Vote { q, .. }, 2, O::Pred(r)) => *q = r,
        _ => return false,
    }
    true
}

impl Operand {
    /// Assembly spelling of the operand.
    pub fn text(self) -> String {
        match self {
            Operand::Reg(r) => format!("r{r}"),
            Operand::Pred(p) => format!("p{p}"),
            Operand::Src(s) => s.to_string(),
            Operand::Cmp(c) => c.name().to_string(),
            Operand::Count(n) => n.to_string(),
            Operand::Mode(all) => if all { "ALL" } else { "ANY" }.to_string(),
            Operand::Op(o) => o.name().to_string(),
        }
    }
}

/// Inserts a fresh instruction of `kind` after line `after` (or at the top
/// when `None`). Openers get their closer so the block stays balanced.
/// Returns the index of the new line.
pub fn insert(code: &mut Vec<Instr>, after: Option<usize>, kind: OpKind) -> usize {
    let at = after.map_or(0, |a| (a + 1).min(code.len()));
    let ins = kind.template();
    code.insert(at, ins);
    match kind {
        OpKind::If => code.insert(at + 1, Instr::EndIf),
        OpKind::Loop => code.insert(at + 1, Instr::EndLoop),
        _ => {}
    }
    at
}

/// The lines that form the block structure `line` belongs to: an opener,
/// its ELSE and its closer. Falls back to just `line` when unbalanced.
pub fn block_of(code: &[Instr], line: usize) -> Vec<usize> {
    let mut stack: Vec<Vec<usize>> = Vec::new();
    for (i, ins) in code.iter().enumerate() {
        match ins {
            Instr::If { .. } | Instr::Loop { .. } => stack.push(vec![i]),
            Instr::Else => match stack.last_mut() {
                Some(b) => b.push(i),
                None => return vec![line],
            },
            Instr::EndIf | Instr::EndLoop => {
                let Some(mut b) = stack.pop() else {
                    return vec![line];
                };
                b.push(i);
                if b.contains(&line) {
                    return b;
                }
            }
            _ => {}
        }
    }
    vec![line]
}

/// The lines a removal of `line` takes: a block opener or closer takes its
/// whole frame (opener, ELSE, closer) but keeps the body; an ELSE goes alone.
pub fn removal(code: &[Instr], line: usize) -> Vec<usize> {
    if line >= code.len() {
        return Vec::new();
    }
    let mut lines = match code[line] {
        Instr::If { .. } | Instr::Loop { .. } | Instr::EndIf | Instr::EndLoop => {
            block_of(code, line)
        }
        _ => vec![line],
    };
    lines.sort_unstable();
    lines
}

/// Removes a line (see [`removal`]). Returns the removed count.
pub fn remove(code: &mut Vec<Instr>, line: usize) -> usize {
    let lines = removal(code, line);
    for &l in lines.iter().rev() {
        code.remove(l);
    }
    lines.len()
}

/// Code with the comments that precede each instruction, so editing a
/// listing keeps the Grid Hall's annotations attached to their lines.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Doc {
    pub code: Vec<Instr>,
    pub notes: Vec<Option<String>>,
    /// `.warps N`, for machines where the player picks the launch.
    pub warps: Option<u8>,
    /// `.smem N`: shared-memory words the block asks for.
    pub smem: Option<u16>,
    /// `.block N`: threads per block.
    pub block: Option<u16>,
}

/// Bilingual notes are written `english | 中文`; several comment lines that
/// belong to one instruction keep their halves apart when merged.
fn join_notes(parts: &[String]) -> String {
    if !parts.iter().any(|p| p.contains(" | ")) {
        return parts.join(" ");
    }
    let half = |zh: bool| {
        parts
            .iter()
            .map(|p| note_text(p, zh))
            .collect::<Vec<_>>()
            .join(" ")
    };
    format!("{} | {}", half(false), half(true))
}

/// The half of a note for the chosen language.
pub fn note_text(note: &str, zh: bool) -> &str {
    match note.split_once(" | ") {
        Some((en, cn)) => {
            if zh {
                cn
            } else {
                en
            }
        }
        None => note,
    }
}

impl Doc {
    pub fn parse(src: &str) -> Result<Self, crate::asm::AsmError> {
        let mut doc = Doc::default();
        let mut pending: Vec<String> = Vec::new();
        for (line, raw) in src.lines().enumerate() {
            if let Some(rest) = raw.trim().strip_prefix(".warps") {
                doc.warps = rest.trim().parse().ok().filter(|n| (1..=16).contains(n));
                continue;
            }
            if let Some(rest) = raw.trim().strip_prefix(".smem") {
                doc.smem = rest.trim().parse().ok().filter(|n| (1..=8192).contains(n));
                continue;
            }
            if let Some(rest) = raw.trim().strip_prefix(".block") {
                doc.block = rest
                    .trim()
                    .parse()
                    .ok()
                    .filter(|n| (32..=512).contains(n) && n % 32 == 0);
                continue;
            }
            let (body, note) = match raw.find([';', '#']) {
                Some(i) => (&raw[..i], Some(raw[i + 1..].trim())),
                None => (raw, None),
            };
            let ins =
                crate::asm::parse_line(body).map_err(|kind| crate::asm::AsmError { line, kind })?;
            if let Some(n) = note.filter(|n| !n.is_empty()) {
                pending.push(n.to_string());
            }
            if let Some(i) = ins {
                doc.code.push(i);
                doc.notes
                    .push((!pending.is_empty()).then(|| join_notes(&pending)));
                pending.clear();
            }
        }
        Ok(doc)
    }

    /// Assembly source: notes as comment lines, blocks indented.
    pub fn source(&self) -> String {
        let depths = Program::depths(&self.code);
        let mut out = String::new();
        if let Some(n) = self.warps {
            out.push_str(&format!(".warps {n}\n"));
        }
        if let Some(n) = self.smem {
            out.push_str(&format!(".smem {n}\n"));
        }
        if let Some(n) = self.block {
            out.push_str(&format!(".block {n}\n"));
        }
        for (i, ins) in self.code.iter().enumerate() {
            let pad = "  ".repeat(depths[i]);
            if let Some(Some(n)) = self.notes.get(i) {
                out.push_str(&format!("{pad}; {n}\n"));
            }
            out.push_str(&format!("{pad}{ins}\n"));
        }
        out
    }

    pub fn insert(&mut self, after: Option<usize>, kind: OpKind) -> usize {
        let before = self.code.len();
        let at = insert(&mut self.code, after, kind);
        for _ in before..self.code.len() {
            self.notes.insert(at.min(self.notes.len()), None);
        }
        at
    }

    pub fn remove(&mut self, line: usize) -> usize {
        let lines = removal(&self.code, line);
        for &l in lines.iter().rev() {
            self.code.remove(l);
            if l < self.notes.len() {
                self.notes.remove(l);
            }
        }
        lines.len()
    }

    /// Swaps a line with its neighbour. Returns the line's new index.
    pub fn shift(&mut self, line: usize, up: bool) -> Option<usize> {
        let other = if up { line.checked_sub(1)? } else { line + 1 };
        if other >= self.code.len() || line >= self.code.len() {
            return None;
        }
        self.code.swap(line, other);
        self.notes.resize(self.code.len(), None);
        self.notes.swap(line, other);
        Some(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::assemble;

    fn code(src: &str) -> Vec<Instr> {
        assemble(src).unwrap().code
    }

    #[test]
    fn operands_round_trip_through_set() {
        for kind in [
            OpKind::Mov,
            OpKind::Alu(Alu::Add),
            OpKind::SetP,
            OpKind::Sel,
            OpKind::If,
            OpKind::Loop,
            OpKind::Ld,
            OpKind::St,
            OpKind::Pix,
            OpKind::Shfl,
            OpKind::Vote,
            OpKind::Edge,
            OpKind::Dot,
            OpKind::Rcp,
            OpKind::Cvt(crate::isa::Cvt::HI),
            OpKind::Mma,
            OpKind::Lerp,
            OpKind::Ldo,
            OpKind::Lds,
            OpKind::Sts,
            OpKind::Ldl,
            OpKind::Stl,
            OpKind::Dma,
            OpKind::Send,
            OpKind::Recv,
            OpKind::Atom,
            OpKind::AtomS,
        ] {
            let mut i = kind.template();
            for (n, o) in operands(&i).into_iter().enumerate() {
                assert!(set_operand(&mut i, n, o), "{kind:?} slot {n}");
            }
            assert_eq!(i, kind.template());
        }
    }

    #[test]
    fn smem_and_block_directives_survive_a_round_trip() {
        let d = Doc::parse(".smem 512\n.block 64\nLD r0, [lane]\nST [lane], r0").unwrap();
        assert_eq!((d.smem, d.block), (Some(512), Some(64)));
        let again = Doc::parse(&d.source()).unwrap();
        assert_eq!((again.smem, again.block), (Some(512), Some(64)));
        assert_eq!(again.code.len(), 2);
    }

    #[test]
    fn warps_directive_survives_a_round_trip() {
        let d = Doc::parse(".warps 4\nLD r0, [lane]\nST [lane], r0").unwrap();
        assert_eq!(d.warps, Some(4));
        assert!(d.source().starts_with(".warps 4\n"));
        assert_eq!(Doc::parse(&d.source()).unwrap().warps, Some(4));
        assert_eq!(Doc::parse("LD r0, [lane]").unwrap().warps, None);
    }

    #[test]
    fn set_operand_checks_kind_and_range() {
        let mut i = Instr::Alu {
            op: Alu::Add,
            d: 0,
            a: Src::Reg(0),
            b: Src::Imm(1),
        };
        assert!(!set_operand(&mut i, 1, Operand::Pred(0)));
        assert!(!set_operand(&mut i, 1, Operand::Reg(REGS)));
        assert!(!set_operand(&mut i, 9, Operand::Reg(1)));
        assert!(set_operand(&mut i, 0, Operand::Op(Alu::Mul)));
        assert!(set_operand(&mut i, 3, Operand::Src(Src::Sp(Special::Lane))));
        assert_eq!(i.to_string(), "MUL r0, r0, lane");
        let mut l = Instr::Loop { n: 4 };
        assert!(!set_operand(&mut l, 0, Operand::Count(MAX_LOOP + 1)));
    }

    #[test]
    fn insert_balances_blocks() {
        let mut c = code("PIX 1");
        let at = insert(&mut c, Some(0), OpKind::If);
        assert_eq!(at, 1);
        assert!(Program::new(c.clone()).is_ok());
        insert(&mut c, Some(at), OpKind::Loop);
        assert!(Program::new(c.clone()).is_ok());
        assert_eq!(insert(&mut c, None, OpKind::Pix), 0);
        assert_eq!(c.len(), 6);
    }

    #[test]
    fn remove_takes_the_whole_frame() {
        let src = "IF p0\nPIX 1\nELSE\nPIX 2\nENDIF\nPIX 3";
        let mut c = code(src);
        assert_eq!(remove(&mut c, 4), 3);
        assert_eq!(c, code("PIX 1\nPIX 2\nPIX 3"));
        let mut c = code(src);
        assert_eq!(remove(&mut c, 2), 1);
        assert!(Program::new(c.clone()).is_ok());
        let mut c = code("LOOP 2\nIF p0\nPIX 1\nENDIF\nENDLOOP");
        assert_eq!(remove(&mut c, 0), 2);
        assert_eq!(c, code("IF p0\nPIX 1\nENDIF"));
        assert_eq!(remove(&mut c, 7), 0);
    }

    #[test]
    fn bilingual_notes_pick_a_language() {
        let d = Doc::parse("; one | 一\n; two | 二\nPIX 1\n").unwrap();
        let n = d.notes[0].as_deref().unwrap();
        assert_eq!(note_text(n, false), "one two");
        assert_eq!(note_text(n, true), "一 二");
        assert_eq!(note_text("plain", true), "plain");
    }

    #[test]
    fn docs_keep_notes_on_their_lines() {
        let src = "; first\nPIX 1\n\nLOOP 2 ; twice\n  ; body\n  ADD r0, r0, 1\nENDLOOP\n";
        let mut d = Doc::parse(src).unwrap();
        assert_eq!(d.code.len(), 4);
        assert_eq!(d.notes[0].as_deref(), Some("first"));
        assert_eq!(d.notes[1].as_deref(), Some("twice"));
        assert_eq!(d.notes[2].as_deref(), Some("body"));
        let back = Doc::parse(&d.source()).unwrap();
        assert_eq!(back, d);
        assert_eq!(d.shift(0, true), None);
        assert_eq!(d.shift(0, false), Some(1));
        assert_eq!(d.notes[1].as_deref(), Some("first"));
        d.insert(Some(3), OpKind::If);
        assert_eq!((d.code.len(), d.notes.len()), (6, 6));
        assert_eq!(d.remove(4), 2);
        assert_eq!((d.code.len(), d.notes.len()), (4, 4));
        assert!(Doc::parse("BOGUS").is_err());
    }
}
