//! What a Grid Hall task is: the machine shape, the test cases, what counts as
//! correct, the mastery goals, and the teaching text around it.

use oo_base::{Cat, LevelKind, Tr};
use serde::{Deserialize, Serialize};

use crate::graph::{Graph, NodeKind};
use crate::isa::{FB_LEN, OpKind};
use crate::vm::Launch;

/// Chapters in the game, and levels in each (bible §3).
pub const CHAPTERS: u8 = 16;
pub const LEVELS_PER_CHAPTER: u8 = 8;

/// Kind of the `slot`-th level (0-based): 3 puzzles, a fault, 2 puzzles, a
/// fault, then the commission.
pub fn kind_of_slot(slot: u8) -> LevelKind {
    match slot {
        3 | 6 => LevelKind::Fault,
        7 => LevelKind::Commission,
        _ => LevelKind::Puzzle,
    }
}

/// One chapter of the course; a machine spans one to three chapters.
#[derive(Clone, Copy, Debug)]
pub struct Chapter {
    pub number: u8,
    /// The machine being rebuilt.
    pub machine: Tr,
    pub title: Tr,
    pub tagline: Tr,
    /// Who commissions the chapter's last level; `None` is the whole town.
    pub client: Option<Cat>,
}

impl Chapter {
    pub fn client_name(&self) -> Tr {
        self.client
            .map(Cat::name)
            .unwrap_or(Tr::new("The whole town", "全镇"))
    }
}

/// How the output buffer is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    /// 16×16 colour screen.
    Screen,
    /// Two screens side by side: front and back buffer (`out[0..512]`).
    Buffers,
    /// One row of 32 numbered cells, one per lane.
    Lanes,
    /// Vault: the out buffer as a grid of values, 32 to a row.
    Cells,
}

/// Lower-is-better measurements of a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    /// Warp-instructions issued (worst case across test cases).
    Cycles,
    /// Instructions in the program, or compute nodes in a circuit.
    Size,
    /// Lane loads from ROM (worst case).
    Loads,
    /// Lane stores (worst case).
    Stores,
    /// Registers the kernel touches (highest index + 1).
    Regs,
    /// Vault: global read transactions, cache lines touched per read.
    Txns,
    /// Vault: L1 misses.
    Misses,
    /// Vault: shared-memory words used.
    Smem,
    /// Atlas: lanes that performed a global ATOM.
    Atoms,
    /// Lyra: the largest gap between an output cell and its expectation.
    Err,
    /// Origin: words sent over all links.
    LinkWords,
    /// Origin: words over the busiest single link.
    HotLink,
    /// Origin: idle share of the idlest SM, percent.
    Idle,
    /// Origin: share of all SM-cycles that issued nothing, percent.
    Bubble,
}

impl Metric {
    pub fn label(self) -> Tr {
        match self {
            Metric::Cycles => Tr::new("cycles", "周期"),
            Metric::Size => Tr::new("size", "规模"),
            Metric::Loads => Tr::new("loads", "读取"),
            Metric::Stores => Tr::new("stores", "写入"),
            Metric::Regs => Tr::new("registers", "寄存器"),
            Metric::Txns => Tr::new("read transactions", "读事务"),
            Metric::Misses => Tr::new("cache misses", "未命中"),
            Metric::Smem => Tr::new("shared words", "共享字数"),
            Metric::Atoms => Tr::new("global atomics", "全局原子操作"),
            Metric::Err => Tr::new("max error", "最大误差"),
            Metric::LinkWords => Tr::new("link words", "链路字数"),
            Metric::HotLink => Tr::new("busiest link", "最忙链路"),
            Metric::Idle => Tr::new("max idle %", "最大空闲率 %"),
            Metric::Bubble => Tr::new("bubble %", "气泡率 %"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Goal {
    pub metric: Metric,
    pub max: u32,
}

impl Goal {
    pub const fn new(metric: Metric, max: u32) -> Self {
        Self { metric, max }
    }
}

/// Which editor the task uses and which parts it offers.
#[derive(Clone, Copy, Debug)]
pub enum Editor {
    Graph(&'static [NodeKind]),
    Code(&'static [OpKind]),
}

/// One test input: the ROM image and the frame counter.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Case {
    pub rom: Vec<i32>,
    pub frame: i32,
}

impl Case {
    pub fn frame(frame: i32) -> Self {
        Self {
            rom: Vec::new(),
            frame,
        }
    }
}

/// A player's work on a task, as saved.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Solution {
    /// Assembly source.
    Code(String),
    Graph(Graph),
}

#[derive(Clone, Copy, Debug)]
pub enum Source {
    Code(&'static str),
    Graph(fn() -> Graph),
}

impl Source {
    pub fn solution(self) -> Solution {
        match self {
            Source::Code(s) => Solution::Code(s.to_string()),
            Source::Graph(f) => Solution::Graph(f()),
        }
    }
}

/// The Predict step: commit to an answer before running anything.
#[derive(Clone, Copy, Debug)]
pub struct Predict {
    pub question: Tr,
    pub options: [Tr; 3],
    pub answer: usize,
    pub why: Tr,
}

/// The card earned on a solve: what the player just touched, honestly framed.
#[derive(Clone, Copy, Debug)]
pub struct Insight {
    pub title: Tr,
    pub line: Tr,
    /// How real hardware does it.
    pub reality: Tr,
    /// A common misconception, corrected.
    pub myth: Tr,
    /// What OC-1 simplifies away.
    pub simplified: Tr,
}

#[derive(Clone, Debug)]
pub struct Task {
    pub id: &'static str,
    /// 1..=8; see [`crate::content::chapter`].
    pub chapter: u8,
    /// 1..=6 within the chapter: slots 1–4 are puzzles, 5 the fault, 6 the
    /// commission.
    pub slot: u8,
    pub view: View,
    pub title: Tr,
    /// What the hall wants, in ORI's voice. Lines separated by `\n`.
    pub brief: Tr,
    pub editor: Editor,
    pub launch: Launch,
    /// The machine the kernel runs on.
    pub profile: crate::vm::Profile,
    pub out_len: usize,
    pub cases: fn() -> Vec<Case>,
    /// Same distribution, other seeds: run with every submission but never
    /// shown, so a solution has to understand the rule, not the examples.
    pub hidden: fn() -> Vec<Case>,
    pub init: fn(&Case) -> Vec<i32>,
    /// Expected buffer after the run; `None` cells are not checked.
    pub expect: fn(&Case) -> Vec<Option<i32>>,
    /// How far a cell may be from its expectation and still count (0 = exact).
    pub tolerance: i32,
    /// Must be met to count as solved (e.g. "faster than the old machine").
    pub limit: Option<Goal>,
    /// Second and third star.
    pub goals: [Goal; 2],
    pub starter: Source,
    pub reference: Source,
    /// Showcase input: rewrites a case from the frame and a player-moved point.
    pub live: Option<fn(i32, [i32; 2]) -> Case>,
    /// What moving the showcase point does, as a caption (`None`: the hero).
    pub steer: Option<Tr>,
    pub predict: Predict,
    pub hints: [Tr; 3],
    pub insight: Insight,
    /// Fault levels: the moment to point at on the Warp River before the
    /// fix counts.
    pub clue: Option<Clue>,
    pub requires: &'static [&'static str],
    /// Vault: the player chooses the warp count with `.warps N` (1–16);
    /// `launch` is then only the default.
    pub free_warps: bool,
    /// Atlas: the grid depends on the case (its size varies); `launch` is
    /// the default.
    pub grid: Option<fn(&Case) -> Launch>,
    /// Atlas: the player picks threads per block with `.block N`; the grid
    /// is then rebuilt from `launch`/`grid` with that block size.
    pub free_block: bool,
    /// Atlas: most registers a thread may use; more is refused.
    pub reg_cap: Option<u8>,
    /// Origin: bit flips planned for a case's messages.
    pub flips: Option<fn(&Case) -> Vec<crate::vm::Flip>>,
}

/// A fault's evidence, found in the actual replay of a failing case.
#[derive(Clone, Copy, Debug)]
pub struct Clue {
    /// What the player is looking for, e.g. "the first wrong bulb being written".
    pub what: Tr,
    /// The trace window (step indices) and warp that count as pointing at it.
    pub find: fn(&crate::isa::Program, &crate::grade::CaseRun) -> Option<oo_base::Evidence>,
}

impl Task {
    /// The launch for `prog`: a free-warps task honours its `.warps N` line.
    pub fn launch_for(&self, prog: &crate::isa::Program, case: &Case) -> Launch {
        let base = self.grid.map_or(self.launch, |g| g(case));
        match (prog.warps, prog.block) {
            (Some(n), _) if self.free_warps => Launch::flat(n as usize * crate::isa::LANES),
            (_, Some(b)) if self.free_block => {
                Launch::grid(base.threads.div_ceil(b as usize), b as usize)
            }
            _ => base,
        }
    }

    pub fn kind(&self) -> LevelKind {
        kind_of_slot(self.slot - 1)
    }

    /// "3-5": chapter and slot, as printed on the level label.
    pub fn number(&self) -> String {
        format!("{}-{}", self.chapter, self.slot)
    }
}

/// Default `init`: an all-black buffer of the task's length.
pub fn blank(len: usize) -> Vec<i32> {
    vec![0; len]
}

pub fn screen_blank(_: &Case) -> Vec<i32> {
    blank(FB_LEN)
}

pub fn lanes_blank(_: &Case) -> Vec<i32> {
    blank(crate::isa::LANES)
}
