//! Seals: a kernel the player wrote for a solved level, kept under a name and
//! pasted into later programs by a `USE name` line.
//!
//! Expansion is plain text substitution at assembly time. The sealed block
//! shares the host's registers, so its card lists what it reads and what it
//! leaves behind. Its instructions count toward length, size and cycles like
//! any other code, and must be allowed in the host's task.

use std::collections::{BTreeMap, BTreeSet};

use oo_base::{Lang, Tr};

use crate::asm::{self, AsmError, AsmErrorKind};
use crate::isa::{Instr, Program};
use crate::save::SaveData;
use crate::task::{Editor, Task};

/// Sealed bodies by name, as assembly source without `USE` lines.
pub type Seals = BTreeMap<String, String>;

/// A kernel that can be sealed once its level is solved.
#[derive(Clone, Copy, Debug)]
pub struct Sealable {
    pub name: &'static str,
    /// The level whose passing work becomes the body.
    pub task: &'static str,
    /// What the block does, as the level asked for it.
    pub card: Tr,
}

pub const CATALOGUE: &[Sealable] = &[
    Sealable {
        name: "reduce32",
        task: "the-reduction",
        card: Tr::new(
            "Sums rom[0..32] across one warp's lanes and writes the total to out[0].",
            "把 rom[0..32] 在一个 warp 的车道间加总，写到 out[0]。",
        ),
    },
    Sealable {
        name: "tile_load",
        task: "the-neighbors",
        card: Tr::new(
            "Loads a 32×16 picture into shared memory once, then each thread sums its 3×3 and writes out.",
            "把 32×16 的图读进共享内存一次，再由每个线程求 3×3 之和并写出。",
        ),
    },
    Sealable {
        name: "bilerp",
        task: "the-bilinear",
        card: Tr::new(
            "Bilinear sample: four clamped neighbours, two LERPs across and one down.",
            "双线性取样：四个钳位的邻居，横向两次 LERP，纵向一次。",
        ),
    },
    Sealable {
        name: "mip2x",
        task: "the-mip",
        card: Tr::new(
            "Shrinks a 64×64 picture to 8×8 by averaging 2×2 blocks three times.",
            "连做三次 2×2 平均，把 64×64 的图缩成 8×8。",
        ),
    },
    Sealable {
        name: "scan32",
        task: "the-prefix",
        card: Tr::new(
            "Running total across one warp: out[lane] = rom[0] + … + rom[lane].",
            "一个 warp 内的累计和：out[lane] = rom[0] + … + rom[lane]。",
        ),
    },
    Sealable {
        name: "compact",
        task: "the-compact",
        card: Tr::new(
            "Scans a wet mask into a dense list of wet cells, then updates only those.",
            "把湿格掩码扫描成紧凑的湿格列表，再只更新这些格。",
        ),
    },
    Sealable {
        name: "gemm_tile",
        task: "the-matmul",
        card: Tr::new(
            "64×64 matrix product with 16×16 tiles staged through shared memory.",
            "64×64 矩阵乘，16×16 的块经共享内存装入。",
        ),
    },
    Sealable {
        name: "softmax16",
        task: "overflow",
        card: Tr::new(
            "Half-precision softmax over one warp's 32 scores, maximum subtracted first.",
            "对一个 warp 的 32 个半精度分数做 softmax，先减去最大值。",
        ),
    },
    Sealable {
        name: "ring_allreduce",
        task: "all-reduce",
        card: Tr::new(
            "Ring all-reduce over four SMs: three steps of reduce-scatter, three of all-gather.",
            "四个 SM 上的环形 all-reduce：三步 reduce-scatter，三步 all-gather。",
        ),
    },
    Sealable {
        name: "attn_head",
        task: "the-attention",
        card: Tr::new(
            "One causal attention head: Q·Kᵀ, masked softmax, then times V.",
            "一个因果注意力头：Q·Kᵀ，带掩码的 softmax，再乘 V。",
        ),
    },
];

/// The seal a level's passing work can become.
pub fn sealable(task: &str) -> Option<&'static Sealable> {
    CATALOGUE.iter().find(|s| s.task == task)
}

pub fn find(name: &str) -> Option<&'static Sealable> {
    CATALOGUE.iter().find(|s| s.name == name)
}

/// Seals open with chapter 10.
pub fn is_open(save: &SaveData) -> bool {
    crate::content::tasks()
        .iter()
        .filter(|t| t.chapter == 10)
        .min_by_key(|t| t.slot)
        .is_some_and(|t| save.is_unlocked(t))
}

/// One `USE` line after expansion: its code is `start..end`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub name: String,
    pub start: usize,
    pub end: usize,
    /// The `USE` line in the source (0-based).
    pub line: usize,
}

impl Span {
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    pub fn contains(&self, pc: usize) -> bool {
        (self.start..self.end).contains(&pc)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SealError {
    /// No seal of this name has been made.
    Unknown(String),
    /// A sealed body that itself says `USE`.
    Nested(String),
    /// A sealed body that no longer assembles.
    Broken(String),
}

impl SealError {
    pub fn text(&self, l: Lang) -> String {
        let (tr, name) = match self {
            SealError::Unknown(n) => (
                Tr::new(
                    "No seal named `{}`. Seal a solved level's work from its results first.",
                    "没有名为 `{}` 的封印。先在已解关卡的结算页封印它的程序。",
                ),
                n,
            ),
            SealError::Nested(n) => (
                Tr::new(
                    "Seal `{}` uses another seal; a sealed block cannot USE.",
                    "封印 `{}` 里又用了封印；封印块内不能再 USE。",
                ),
                n,
            ),
            SealError::Broken(n) => (
                Tr::new(
                    "Seal `{}` no longer assembles. Seal its level again.",
                    "封印 `{}` 已无法汇编。请重新封印它的关卡。",
                ),
                n,
            ),
        };
        tr.fill(l, &[name])
    }
}

/// Reads a `USE name` line (comments already stripped). `None` when the line
/// is something else; an error when `USE` has no name or more than one.
pub fn use_of(text: &str) -> Option<Result<String, AsmErrorKind>> {
    let text = text.trim();
    let (op, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    if !op.eq_ignore_ascii_case("USE") {
        return None;
    }
    let args: Vec<&str> = rest.split_whitespace().collect();
    Some(match args[..] {
        [name] if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') => {
            Ok(name.to_ascii_lowercase())
        }
        [bad] => Err(AsmErrorKind::BadOperand(bad.to_string())),
        _ => Err(AsmErrorKind::Arity {
            want: 1,
            got: args.len(),
        }),
    })
}

fn code_part(raw: &str) -> &str {
    raw.split([';', '#']).next().unwrap_or("")
}

fn is_directive(raw: &str) -> bool {
    code_part(raw).trim_start().starts_with('.')
}

/// The instructions of a sealed body. Bodies are leaves: a `USE` inside one
/// is an error, so expansion is one level deep.
fn body(seals: &Seals, name: &str) -> Result<Vec<Instr>, SealError> {
    let src = seals
        .get(name)
        .ok_or_else(|| SealError::Unknown(name.to_string()))?;
    let mut code = Vec::new();
    for raw in src.lines().filter(|r| !is_directive(r)) {
        let text = code_part(raw);
        if use_of(text).is_some() {
            return Err(SealError::Nested(name.to_string()));
        }
        match asm::parse_line(text) {
            Ok(Some(i)) => code.push(i),
            Ok(None) => {}
            Err(_) => return Err(SealError::Broken(name.to_string())),
        }
    }
    Ok(code)
}

/// Instructions, the source line of each, and the expanded spans.
pub type Parsed = (Vec<Instr>, Vec<usize>, Vec<Span>);

/// Like [`asm::parse`], with each `USE name` replaced by the sealed body.
/// Expanded instructions report the `USE` line as their source line.
pub fn parse(source: &str, seals: &Seals) -> Result<Parsed, AsmError> {
    let mut code = Vec::new();
    let mut lines = Vec::new();
    let mut spans = Vec::new();
    for (line, raw) in source.lines().enumerate() {
        if is_directive(raw) {
            continue;
        }
        let text = code_part(raw);
        let err = |kind| AsmError { line, kind };
        match use_of(text) {
            Some(Ok(name)) => {
                let block = body(seals, &name).map_err(|e| err(AsmErrorKind::Seal(e)))?;
                let start = code.len();
                lines.extend(std::iter::repeat_n(line, block.len()));
                code.extend(block);
                spans.push(Span {
                    name,
                    start,
                    end: code.len(),
                    line,
                });
            }
            Some(Err(kind)) => return Err(err(kind)),
            None => match asm::parse_line(text) {
                Ok(Some(i)) => {
                    code.push(i);
                    lines.push(line);
                }
                Ok(None) => {}
                Err(kind) => return Err(err(kind)),
            },
        }
    }
    Ok((code, lines, spans))
}

/// Assembles source that may `USE` seals. Without `USE` lines this equals
/// [`asm::assemble`].
pub fn assemble(source: &str, seals: &Seals) -> Result<Program, AsmError> {
    let (code, lines, spans) = parse(source, seals)?;
    let mut prog = asm::finish(source, code, lines)?;
    prog.seals = spans;
    Ok(prog)
}

/// The source with every `USE` line replaced by its body's code (directives
/// in the body are dropped). This is what gets stored when sealing, so a
/// stored body never says `USE`.
pub fn flatten(source: &str, seals: &Seals) -> Result<String, AsmError> {
    let mut out = String::new();
    for (line, raw) in source.lines().enumerate() {
        match use_of(code_part(raw)) {
            Some(Ok(name)) => {
                body(seals, &name).map_err(|e| AsmError {
                    line,
                    kind: AsmErrorKind::Seal(e),
                })?;
                for b in seals[&name].lines().filter(|r| !is_directive(r)) {
                    out.push_str(b);
                    out.push('\n');
                }
            }
            Some(Err(kind)) => return Err(AsmError { line, kind }),
            None => {
                out.push_str(raw);
                out.push('\n');
            }
        }
    }
    Ok(out)
}

/// Instruction count of a seal's body, if it expands.
pub fn body_len(seals: &Seals, name: &str) -> Option<usize> {
    body(seals, name).ok().map(|b| b.len())
}

/// What a sealed block expects from and leaves to the code around it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Contract {
    /// Registers read before the block writes them, so the host sets them.
    pub reads: Vec<u8>,
    /// Registers the block writes, so the host loses what was there.
    pub writes: Vec<u8>,
    /// `.smem N` the original program asked for.
    pub smem: Option<u16>,
    pub len: usize,
}

/// The register contract of a seal, read in program order.
pub fn contract(seals: &Seals, name: &str) -> Option<Contract> {
    let code = body(seals, name).ok()?;
    let mut reads = BTreeSet::new();
    let mut writes = BTreeSet::new();
    for i in &code {
        reads.extend(i.reads().filter(|r| !writes.contains(r)));
        writes.extend(i.dest());
    }
    let smem = seals[name].lines().find_map(|raw| {
        code_part(raw)
            .trim()
            .strip_prefix(".smem")?
            .trim()
            .parse()
            .ok()
    });
    Some(Contract {
        reads: reads.into_iter().collect(),
        writes: writes.into_iter().collect(),
        smem,
        len: code.len(),
    })
}

/// The editor line of `pc` when each span shows as one `USE` line: a pc
/// inside a span maps to that span's line.
pub fn line_of(spans: &[Span], pc: usize) -> usize {
    let mut line = pc as isize;
    for s in spans {
        if s.contains(pc) {
            return (line - (pc - s.start) as isize).max(0) as usize;
        }
        if s.end <= pc {
            line += 1 - s.len() as isize;
        }
    }
    line.max(0) as usize
}

/// Seals whose every instruction this task allows, in catalogue order.
pub fn usable(task: &Task, seals: &Seals) -> Vec<&'static Sealable> {
    let Editor::Code(ops) = task.editor else {
        return Vec::new();
    };
    CATALOGUE
        .iter()
        .filter(|s| {
            body(seals, s.name).is_ok_and(|code| code.iter().all(|i| ops.contains(&i.kind())))
        })
        .collect()
}

/// Stores `work` as the body of `seal`. Returns `false` when the stored
/// body was already the same.
pub fn store(seals: &mut Seals, seal: &Sealable, work: &str) -> Result<bool, AsmError> {
    let flat = flatten(work, seals)?;
    if seals.get(seal.name) == Some(&flat) {
        return Ok(false);
    }
    seals.insert(seal.name.to_string(), flat);
    Ok(true)
}

/// Every seal made from its level's reference solution (for tests and the
/// `OOC_SEAL` debug switch).
pub fn from_references() -> Seals {
    let mut seals = Seals::new();
    for s in CATALOGUE {
        let Some(t) = crate::content::task(s.task) else {
            continue;
        };
        if let crate::task::Solution::Code(src) = t.reference.solution() {
            let _ = store(&mut seals, s, &src);
        }
    }
    seals
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content;
    use crate::grade::{grade, grade_with};
    use crate::task::Solution;

    fn seals(pairs: &[(&str, &str)]) -> Seals {
        pairs
            .iter()
            .map(|(n, s)| (n.to_string(), s.to_string()))
            .collect()
    }

    #[test]
    fn catalogue_names_real_code_levels() {
        let mut names = BTreeSet::new();
        for s in CATALOGUE {
            let t = content::task(s.task).unwrap_or_else(|| panic!("{}", s.task));
            assert!(matches!(t.editor, Editor::Code(_)), "{}", s.task);
            assert!(s.card.is_complete());
            assert!(names.insert(s.name), "{} twice", s.name);
            assert_eq!(find(s.name).unwrap().task, s.task);
            assert_eq!(sealable(s.task).unwrap().name, s.name);
        }
        let all = from_references();
        assert_eq!(all.len(), CATALOGUE.len());
        for s in CATALOGUE {
            let t = content::task(s.task).unwrap();
            assert!(usable(t, &all).iter().any(|u| u.name == s.name), "{}", s.name);
        }
    }

    #[test]
    fn use_expands_in_place_and_records_spans() {
        let s = seals(&[("two", ".smem 64\n; note\nMOV r1, 2\nADD r0, r0, r1\n")]);
        let src = "MOV r0, 1\nUSE two ; again\nuse TWO\nST [0], r0\n";
        let p = assemble(src, &s).unwrap();
        let hand = asm::assemble(
            "MOV r0, 1\nMOV r1, 2\nADD r0, r0, r1\nMOV r1, 2\nADD r0, r0, r1\nST [0], r0",
        )
        .unwrap();
        assert_eq!(p.code, hand.code);
        assert_eq!(p.smem, None, "body directives stay in the body");
        let span = |start, end, line| Span {
            name: "two".into(),
            start,
            end,
            line,
        };
        assert_eq!(p.seals, [span(1, 3, 1), span(3, 5, 2)]);
        assert_eq!(asm::assemble("PIX 1").unwrap().seals, []);
        assert_eq!(
            contract(&s, "two").unwrap(),
            Contract {
                reads: vec![0],
                writes: vec![0, 1],
                smem: Some(64),
                len: 2,
            }
        );
    }

    #[test]
    fn bad_uses_are_errors_on_their_line() {
        let s = seals(&[("loop", "USE loop\n"), ("junk", "FROB r0\n")]);
        let kind = |src: &str| assemble(src, &s).unwrap_err();
        let e = kind("PIX 1\nUSE nothing");
        assert_eq!(e.line, 1);
        assert_eq!(e.kind, AsmErrorKind::Seal(SealError::Unknown("nothing".into())));
        assert_eq!(
            kind("USE loop").kind,
            AsmErrorKind::Seal(SealError::Nested("loop".into()))
        );
        assert_eq!(
            kind("USE junk").kind,
            AsmErrorKind::Seal(SealError::Broken("junk".into()))
        );
        assert!(matches!(kind("USE").kind, AsmErrorKind::Arity { want: 1, got: 0 }));
        assert!(matches!(kind("USE a b").kind, AsmErrorKind::Arity { want: 1, got: 2 }));
        assert!(matches!(kind("USE [r0]").kind, AsmErrorKind::BadOperand(_)));
        let msg = SealError::Unknown("scan32".into());
        assert!(msg.text(Lang::En).contains("`scan32`"));
        assert!(msg.text(Lang::Zh).contains("`scan32`"));
        // Plain assembly does not know USE at all.
        assert!(matches!(
            asm::assemble("USE two").unwrap_err().kind,
            AsmErrorKind::UnknownOp(_)
        ));
    }

    #[test]
    fn blocks_inside_a_seal_must_balance_with_the_host() {
        let s = seals(&[("open", "LOOP 2\n")]);
        assert!(assemble("USE open\nENDLOOP", &s).is_ok());
        let e = assemble("PIX 1\nUSE open", &s).unwrap_err();
        assert_eq!(e.line, 1);
        assert!(matches!(e.kind, AsmErrorKind::Shape(_)));
    }

    #[test]
    fn sealed_code_costs_what_pasted_code_costs() {
        let task = content::task("the-prefix").unwrap();
        let all = from_references();
        let used = Solution::Code("USE scan32\n".into());
        let rep = grade_with(task, &used, &all).unwrap();
        let by_hand = grade(task, &task.reference.solution()).unwrap();
        assert!(rep.passed());
        assert_eq!(rep.measure, by_hand.measure);
        assert_eq!(rep.stars(), by_hand.stars());
        // Plain grading has no seals.
        assert!(grade(task, &used).is_err());
        let src = "MOV r9, 0\nUSE scan32\nNOP\n";
        let flat = flatten(src, &all).unwrap();
        assert!(!flat.contains("USE"));
        let a = assemble(src, &all).unwrap();
        assert_eq!(a.code, asm::assemble(&flat).unwrap().code);
        let n = body_len(&all, "scan32").unwrap();
        assert_eq!(a.seals[0].len(), n);
        assert_eq!(a.len(), n + 2);
    }

    #[test]
    fn expanded_ops_obey_the_host_task() {
        let all = from_references();
        // The prefix kernel shuffles; chapter 2 has no SHFL.
        let lumen = content::task("the-sprite").unwrap();
        let e = grade_with(lumen, &Solution::Code("USE scan32\n".into()), &all).unwrap_err();
        assert!(matches!(e, crate::grade::BuildError::LockedOp(_)), "{e:?}");
        assert!(usable(lumen, &all).is_empty());
    }

    #[test]
    fn storing_flattens_and_reports_changes() {
        let mut s = seals(&[("two", "MOV r1, 2\n")]);
        let scan = find("scan32").unwrap();
        assert!(store(&mut s, scan, "USE two\nPIX r1\n").unwrap());
        assert_eq!(s["scan32"], "MOV r1, 2\nPIX r1\n");
        assert!(!store(&mut s, scan, "USE two\nPIX r1\n").unwrap());
        assert!(store(&mut s, scan, "USE gone\n").is_err());
    }

    #[test]
    fn editor_lines_fold_spans() {
        let s = seals(&[("three", "NOP\nNOP\nNOP\n"), ("none", "; nothing\n")]);
        let p = assemble("PIX 1\nUSE three\nPIX 2\nUSE none\nPIX 3\n", &s).unwrap();
        let lines: Vec<usize> = (0..p.len()).map(|pc| line_of(&p.seals, pc)).collect();
        assert_eq!(lines, [0, 1, 1, 1, 2, 4]);
        let d = crate::edit::Doc::parse("PIX 1\nUSE three\nPIX 2\nUSE none\nPIX 3\n").unwrap();
        assert_eq!(d.code.len(), 5);
        assert_eq!(d.use_at(1), Some("three"));
        assert_eq!(d.use_at(0), None);
    }

    #[test]
    fn docs_keep_use_lines() {
        use crate::edit::Doc;
        let src = "; first\nPIX 1\nUSE scan32\nPIX 2\n";
        let mut d = Doc::parse(src).unwrap();
        assert_eq!(Doc::parse(&d.source()).unwrap(), d);
        assert!(d.source().contains("USE scan32\n"));
        let at = d.insert_use(Some(0), "reduce32");
        assert_eq!(at, 1);
        assert_eq!(d.uses.len(), d.code.len());
        assert_eq!(d.shift(1, false), Some(2));
        assert_eq!((d.use_at(1), d.use_at(2)), (Some("scan32"), Some("reduce32")));
        assert_eq!(d.remove(1), 1);
        assert_eq!(d.use_at(1), Some("reduce32"));
        let mut plain = Doc::parse("PIX 1").unwrap();
        assert!(plain.uses.is_empty());
        plain.insert_use(None, "scan32");
        assert_eq!(plain.source(), "USE scan32\nPIX 1\n");
        assert!(Doc::parse("USE").is_err());
    }
}
