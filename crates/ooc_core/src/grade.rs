//! Running a solution against every test case and turning it into stars.

use oo_base::{Evidence, Outcome};

use crate::asm::{AsmError, assemble};
use crate::graph::{GraphError, NodeKind};
use crate::isa::{OpKind, PREDS, Program, REGS};
use crate::task::{Case, Editor, Goal, Metric, Solution, Task};
use crate::vm::{FrameRun, Machine, run_frame_with};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildError {
    Asm(AsmError),
    Graph(GraphError),
    /// The solution uses a part this level does not offer.
    LockedOp(OpKind),
    LockedNode(NodeKind),
    /// Graph work submitted to a code task or vice versa.
    WrongEditor,
    /// More instructions than this machine's program memory holds.
    TooLong(usize),
    /// More registers per thread than this machine gives (the cap).
    TooManyRegs(u8),
}

/// A solution turned into a runnable program.
#[derive(Clone, Debug)]
pub struct Built {
    pub program: Program,
    /// Instructions, or compute nodes for circuits.
    pub size: usize,
}

pub fn build(task: &Task, sol: &Solution) -> Result<Built, BuildError> {
    match (task.editor, sol) {
        (Editor::Code(ops), Solution::Code(src)) => {
            let program = assemble(src).map_err(BuildError::Asm)?;
            if let Some(op) = program
                .code
                .iter()
                .map(|i| i.kind())
                .find(|k| !ops.contains(k))
            {
                return Err(BuildError::LockedOp(op));
            }
            let size = program.len();
            let room = task.profile.max_program();
            if size > room {
                return Err(BuildError::TooLong(room));
            }
            if program.regs_used() > task.profile.max_regs() {
                return Err(BuildError::TooManyRegs(task.profile.max_regs()));
            }
            if let Some(cap) = task.reg_cap.filter(|&c| program.regs_used() > c) {
                return Err(BuildError::TooManyRegs(cap));
            }
            Ok(Built { program, size })
        }
        (Editor::Graph(kinds), Solution::Graph(g)) => {
            if let Some(n) = g
                .nodes
                .iter()
                .map(|n| n.kind)
                .find(|k| *k != NodeKind::Out && !kinds.contains(k))
            {
                return Err(BuildError::LockedNode(n));
            }
            let c = g.compile().map_err(BuildError::Graph)?;
            Ok(Built {
                program: c.program,
                size: c.used,
            })
        }
        _ => Err(BuildError::WrongEditor),
    }
}

/// One test case, executed.
#[derive(Clone, Debug)]
pub struct CaseRun {
    pub case: Case,
    pub init: Vec<i32>,
    pub out: Vec<i32>,
    pub expect: Vec<Option<i32>>,
    pub tolerance: u32,
    pub run: FrameRun,
    /// Cells that differ from the expectation.
    pub wrong: Vec<usize>,
}

impl CaseRun {
    pub fn ok(&self) -> bool {
        self.wrong.is_empty() && !self.run.timeout
    }

    /// Largest gap between a checked output cell and its expectation.
    pub fn err(&self) -> u32 {
        self.expect
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                let v = (*e)?;
                Some(self.out.get(i).map_or(u32::MAX, |o| o.abs_diff(v)))
            })
            .max()
            .unwrap_or(0)
    }

    /// Whether this case is one the fix has to change: wrong, timed out, or
    /// correct but past the task's hard limit.
    pub fn fails(&self, task: &Task) -> bool {
        let s = &self.run.stats;
        let over = task.limit.is_some_and(|g| {
            let got = match g.metric {
                Metric::Cycles => s.cycles,
                Metric::Loads => s.loads,
                Metric::Stores => s.stores,
                Metric::Size | Metric::Regs => 0,
                Metric::Txns => s.txns,
                Metric::Misses => s.misses,
                Metric::Smem => s.smem,
                Metric::Atoms => s.atoms,
                Metric::Err => self.err(),
                Metric::LinkWords => s.link_words,
                Metric::HotLink => s.hot_link,
                Metric::Idle => idle_pct(&self.run),
                Metric::Bubble => bubble_pct(&self.run),
            };
            got > g.max
        });
        !self.ok() || over
    }
}

/// Idle share of the idlest SM, in percent.
fn idle_pct(run: &FrameRun) -> u32 {
    let a = &run.analysis;
    if a.total == 0 {
        return 0;
    }
    a.sms
        .iter()
        .map(|s| (a.total.saturating_sub(s.issue) as u64 * 100).div_ceil(a.total as u64) as u32)
        .max()
        .unwrap_or(0)
}

/// Share of all SM-cycles with nothing issued, in percent.
fn bubble_pct(run: &FrameRun) -> u32 {
    ((1.0 - run.analysis.busy()) * 100.0)
        .round()
        .clamp(0.0, 100.0) as u32
}

pub fn run_case(task: &Task, program: &Program, case: Case) -> CaseRun {
    let init = (task.init)(&case);
    let mut out = init.clone();
    out.resize(task.out_len, 0);
    let flips = task.flips.map(|f| f(&case)).unwrap_or_default();
    let run = run_frame_with(
        program,
        &case.rom,
        task.launch_for(program, &case),
        task.profile,
        case.frame,
        &flips,
        &mut out,
    );
    let expect = (task.expect)(&case);
    let wrong = expect
        .iter()
        .enumerate()
        .filter(|(i, e)| {
            e.is_some_and(|v| {
                out.get(*i)
                    .is_none_or(|o| o.abs_diff(v) > task.tolerance as u32)
            })
        })
        .map(|(i, _)| i)
        .collect();
    CaseRun {
        case,
        init,
        out,
        expect,
        tolerance: task.tolerance as u32,
        run,
        wrong,
    }
}

/// One thread's registers and predicates when the frame ends — what a
/// probe on a pixel or lane shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Probe {
    pub regs: [i32; REGS as usize],
    pub preds: [bool; PREDS as usize],
}

pub fn probe(task: &Task, program: &Program, case: &Case, thread: usize) -> Option<Probe> {
    let launch = task.launch_for(program, case);
    if thread >= launch.threads {
        return None;
    }
    let mut out = (task.init)(case);
    out.resize(task.out_len, 0);
    let mut m = Machine::new(program, &case.rom, launch, task.profile, case.frame);
    let mut stats = Default::default();
    while m.step(&mut out, &mut stats).is_some() {
        if m.runaway(&stats) {
            break;
        }
    }
    Some(Probe {
        regs: m.regs[thread],
        preds: m.preds[thread],
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    /// First wrong cell of the first failing case.
    Wrong {
        case: usize,
        cell: usize,
        got: i32,
        want: i32,
    },
    Timeout {
        case: usize,
    },
    /// Correct, but misses the task's hard limit.
    OverLimit {
        goal: Goal,
        got: u32,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Measure {
    pub cycles: u32,
    pub size: u32,
    pub loads: u32,
    pub stores: u32,
    pub regs: u32,
    pub txns: u32,
    pub misses: u32,
    pub smem: u32,
    pub atoms: u32,
    pub err: u32,
    pub link: u32,
    pub hot: u32,
    pub idle: u32,
    pub bubble: u32,
}

impl Measure {
    pub fn get(&self, m: Metric) -> u32 {
        match m {
            Metric::Cycles => self.cycles,
            Metric::Size => self.size,
            Metric::Loads => self.loads,
            Metric::Stores => self.stores,
            Metric::Regs => self.regs,
            Metric::Txns => self.txns,
            Metric::Misses => self.misses,
            Metric::Smem => self.smem,
            Metric::Atoms => self.atoms,
            Metric::Err => self.err,
            Metric::LinkWords => self.link,
            Metric::HotLink => self.hot,
            Metric::Idle => self.idle,
            Metric::Bubble => self.bubble,
        }
    }

    pub fn meets(&self, g: Goal) -> bool {
        self.get(g.metric) <= g.max
    }
}

#[derive(Clone, Debug)]
pub struct Report {
    pub program: Program,
    /// Public cases first, then the hidden ones.
    pub runs: Vec<CaseRun>,
    /// How many of `runs` are public.
    pub public: usize,
    pub verdict: Verdict,
    pub measure: Measure,
    /// Which of the two mastery goals were met (only meaningful on a pass).
    pub goals_met: [bool; 2],
}

impl Report {
    pub fn passed(&self) -> bool {
        self.verdict == Verdict::Pass
    }

    /// Whether run `case` is one the player never sees.
    pub fn is_hidden(&self, case: usize) -> bool {
        case >= self.public
    }

    pub fn stars(&self) -> u8 {
        if !self.passed() {
            return 0;
        }
        1 + self.goals_met.iter().filter(|m| **m).count() as u8
    }

    pub fn outcome(&self, task: &Task) -> Option<Outcome> {
        self.passed().then(|| Outcome {
            stars: self.stars(),
            metric: self.measure.get(task.goals[0].metric) as f64,
            alt: self.measure.get(task.goals[1].metric) as f64,
        })
    }
}

pub fn grade(task: &Task, sol: &Solution) -> Result<Report, BuildError> {
    let built = build(task, sol)?;
    Ok(grade_built(task, built))
}

pub fn grade_built(task: &Task, built: Built) -> Report {
    let public = (task.cases)();
    let shown = public.len();
    let runs: Vec<CaseRun> = public
        .into_iter()
        .chain((task.hidden)())
        .map(|c| run_case(task, &built.program, c))
        .collect();
    let mut measure = Measure {
        size: built.size as u32,
        regs: built.program.regs_used() as u32,
        ..Default::default()
    };
    for r in &runs {
        measure.cycles = measure.cycles.max(r.run.stats.cycles);
        measure.loads = measure.loads.max(r.run.stats.loads);
        measure.stores = measure.stores.max(r.run.stats.stores);
        measure.txns = measure.txns.max(r.run.stats.txns);
        measure.misses = measure.misses.max(r.run.stats.misses);
        measure.smem = measure.smem.max(r.run.stats.smem);
        measure.atoms = measure.atoms.max(r.run.stats.atoms);
        measure.err = measure.err.max(r.err());
        measure.link = measure.link.max(r.run.stats.link_words);
        measure.hot = measure.hot.max(r.run.stats.hot_link);
        measure.idle = measure.idle.max(idle_pct(&r.run));
        measure.bubble = measure.bubble.max(bubble_pct(&r.run));
    }
    let mut verdict = Verdict::Pass;
    for (i, r) in runs.iter().enumerate() {
        if r.run.timeout {
            verdict = Verdict::Timeout { case: i };
            break;
        }
        if let Some(&cell) = r.wrong.first() {
            let want = r.expect[cell].unwrap_or_default();
            verdict = Verdict::Wrong {
                case: i,
                cell,
                got: r.out[cell],
                want,
            };
            break;
        }
    }
    if verdict == Verdict::Pass
        && let Some(goal) = task.limit
        && !measure.meets(goal)
    {
        verdict = Verdict::OverLimit {
            goal,
            got: measure.get(goal.metric),
        };
    }
    let goals_met = task.goals.map(|g| measure.meets(g));
    Report {
        program: built.program,
        runs,
        public: shown,
        verdict,
        measure,
        goals_met,
    }
}

/// Evidence: the first issued instruction that writes a cell the case
/// checks and gets wrong, on that instruction's warp.
pub fn first_wrong_write(_: &Program, run: &CaseRun) -> Option<Evidence> {
    let i = run.run.trace.iter().position(|st| {
        st.writes.iter().any(|&(a, v)| {
            run.expect
                .get(a as usize)
                .copied()
                .flatten()
                .is_some_and(|w| w.abs_diff(v) > run.tolerance)
        })
    })?;
    Some(Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

/// Evidence: the first store issued while part of its warp was masked off.
pub fn first_masked_store(prog: &Program, run: &CaseRun) -> Option<Evidence> {
    let i = run.run.trace.iter().position(|st| {
        st.mask != u32::MAX
            && prog
                .code
                .get(st.pc as usize)
                .is_some_and(|ins| ins.kind() == OpKind::St)
    })?;
    Some(Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

/// Evidence: the first SHFL that asked for a lane beyond either end of the warp.
pub fn first_wrapped_shfl(_: &Program, run: &CaseRun) -> Option<Evidence> {
    let i = run.run.trace.iter().position(|st| st.wrapped)?;
    Some(Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

/// Evidence: the first SHFL that read a lane the mask had switched off.
pub fn first_ghost_shfl(_: &Program, run: &CaseRun) -> Option<Evidence> {
    let i = run.run.trace.iter().position(|st| st.ghost)?;
    Some(Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

/// Evidence: the first load issued a second time by the same instruction
/// of a loop, fetching what neighbouring lanes already fetched.
pub fn first_repeat_load(prog: &Program, run: &CaseRun) -> Option<Evidence> {
    let mut seen = std::collections::HashSet::new();
    let i = run.run.trace.iter().position(|st| {
        prog.code
            .get(st.pc as usize)
            .is_some_and(|ins| ins.kind() == OpKind::Ld)
            && !seen.insert((st.warp, st.pc))
    })?;
    Some(Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

/// Evidence: the first instruction that read a register before its result
/// had been written back (Prism's missing interlock).
pub fn first_stale_read(_: &Program, run: &CaseRun) -> Option<Evidence> {
    let i = run.run.trace.iter().position(|st| st.stale)?;
    Some(Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

/// Evidence: the first FP result that overflowed to infinity or NaN.
pub fn first_inf(_: &Program, run: &CaseRun) -> Option<Evidence> {
    let i = run.run.trace.iter().position(|st| st.inf)?;
    Some(Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

/// Evidence: the first INT8 conversion that had to clamp its input.
pub fn first_clamp(_: &Program, run: &CaseRun) -> Option<Evidence> {
    let i = run.run.trace.iter().position(|st| st.sat)?;
    Some(Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

/// Evidence: the first time an instruction zeroed a register an MMA had
/// been accumulating into.
pub fn first_acc_reset(_: &Program, run: &CaseRun) -> Option<Evidence> {
    let i = run.run.trace.iter().position(|st| st.reset)?;
    Some(Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

/// Evidence: the first multiply whose 32-bit product wrapped around.
pub fn first_overflow(_: &Program, run: &CaseRun) -> Option<Evidence> {
    let i = run.run.trace.iter().position(|st| st.overflow)?;
    Some(Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

/// Evidence: the first reciprocal taken of a value at or behind zero.
pub fn first_behind(_: &Program, run: &CaseRun) -> Option<Evidence> {
    let i = run.run.trace.iter().position(|st| st.behind)?;
    Some(Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

/// Evidence: the first global read that touched 16 or more cache lines at once.
pub fn first_uncoalesced(_: &Program, run: &CaseRun) -> Option<Evidence> {
    let i = run.run.trace.iter().position(|st| st.lines >= 16)?;
    Some(Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

/// Evidence: the first shared-memory access that needed more than one pass.
pub fn first_bank_conflict(prog: &Program, run: &CaseRun) -> Option<Evidence> {
    let i = run.run.trace.iter().position(|st| {
        st.passes >= 2
            && prog
                .code
                .get(st.pc as usize)
                .is_some_and(|ins| ins.kind() == OpKind::Lds)
    })?;
    Some(Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

/// Evidence: the first read that fetched a whole line and had it pushed out
/// again after using a single word of it.
pub fn first_wasted_line(_: &Program, run: &CaseRun) -> Option<Evidence> {
    let e = run
        .run
        .mem
        .evictions
        .iter()
        .filter(|e| e.used <= 1)
        .min_by_key(|e| e.fill_step)?;
    Some(Evidence::at(e.fill_step, 0).on(e.warp as u32))
}

/// Evidence: the first read that brought back a line the set had thrown out.
pub fn first_refill(_: &Program, run: &CaseRun) -> Option<Evidence> {
    let f = run.run.mem.fills.iter().find(|f| f.refill)?;
    Some(Evidence::at(f.step, 0).on(f.warp as u32))
}

/// Evidence: the first store to a cell another block had already written
/// since the last GSYNC.
pub fn first_collide(_: &Program, run: &CaseRun) -> Option<Evidence> {
    let i = run.run.trace.iter().position(|st| st.race_w)?;
    Some(Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

/// Evidence: the first read of a cell another block had already overwritten
/// since the last GSYNC.
pub fn first_race_read(_: &Program, run: &CaseRun) -> Option<Evidence> {
    let i = run.run.trace.iter().position(|st| st.race_r)?;
    Some(Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

/// Evidence: the last thing a deadlocked SM did before it went silent.
pub fn first_deadlock(_: &Program, run: &CaseRun) -> Option<Evidence> {
    let d = run.run.deadlock.as_ref()?;
    let i = run
        .run
        .trace
        .iter()
        .rposition(|st| d.sms.contains(&st.sm))
        .or(run.run.trace.len().checked_sub(1))?;
    Some(Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

/// Evidence: the first message a planned bit flip hit, at the step where
/// its receiver reads it (or the end of the trace if it never did).
pub fn first_flip(_: &Program, run: &CaseRun) -> Option<Evidence> {
    let m = run.run.msgs.iter().find(|m| m.flipped.is_some())?;
    let i = run
        .run
        .trace
        .iter()
        .position(|st| st.sm == m.to && st.cycle >= m.arrive)
        .or(run.run.trace.len().checked_sub(1))?;
    Some(Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

/// Evidence: the first instruction issued while it was the only warp left
/// running on the whole machine.
pub fn first_lonely(_: &Program, run: &CaseRun) -> Option<Evidence> {
    let i = run.run.trace.iter().position(|st| st.lonely)?;
    Some(Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

/// Evidence: the first value sent out to local memory because the
/// registers ran out.
pub fn first_spill(_: &Program, run: &CaseRun) -> Option<Evidence> {
    let i = run.run.trace.iter().position(|st| st.spill)?;
    Some(Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

/// Evidence: the first shared-memory read that got ahead of its DMA.
pub fn first_early_read(_: &Program, run: &CaseRun) -> Option<Evidence> {
    let i = run.run.trace.iter().position(|st| st.early)?;
    Some(Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

/// Evidence: the first read that found a flag visible but the data it
/// guards still on its way.
pub fn first_reordered(_: &Program, run: &CaseRun) -> Option<Evidence> {
    let i = run.run.trace.iter().position(|st| st.reorder)?;
    Some(Evidence::at(i as u32, 0).on(run.run.trace[i].warp as u32))
}

/// Evidence: the first global ATOM that landed after a higher-numbered warp
/// had already appended, so the output order depends on who was fastest.
pub fn first_out_of_order(prog: &Program, run: &CaseRun) -> Option<Evidence> {
    let mut high: Option<(usize, u16)> = None;
    for (i, st) in run.run.trace.iter().enumerate() {
        let is_atom = prog
            .code
            .get(st.pc as usize)
            .is_some_and(|ins| ins.kind() == OpKind::Atom);
        if !is_atom {
            continue;
        }
        match high {
            Some((j, w)) if st.warp < w => {
                return Some(Evidence::at(j as u32, 0).on(w as u32));
            }
            Some((_, w)) if st.warp <= w => {}
            _ => high = Some((i, st.warp)),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::task;
    use crate::graph::Graph;

    #[test]
    fn locked_ops_and_wrong_editor_are_rejected() {
        let t = task("thirty-two").unwrap();
        let shfl = Solution::Code("SHFL r0, r0, lane\nST [lane], r0".into());
        assert_eq!(
            grade(t, &shfl).unwrap_err(),
            BuildError::LockedOp(OpKind::Shfl)
        );
        assert_eq!(
            grade(t, &Solution::Graph(Graph::new([0.0, 0.0]))).unwrap_err(),
            BuildError::WrongEditor
        );
        let g = task("first-light").unwrap();
        let mut circuit = Graph::new([0.0, 0.0]);
        circuit.push(crate::graph::Node::new(NodeKind::Frame, [0.0, 0.0]));
        assert_eq!(
            grade(g, &Solution::Graph(circuit)).unwrap_err(),
            BuildError::LockedNode(NodeKind::Frame)
        );
    }

    #[test]
    fn hardcoding_one_case_fails_the_others() {
        // Draws the line for frame 0 only.
        let t = task("the-line").unwrap();
        let rep = grade(t, &t.starter.solution()).unwrap();
        assert!(matches!(rep.verdict, Verdict::Wrong { case: 1, .. }));
        assert_eq!(rep.stars(), 0);
        assert!(rep.outcome(t).is_none());
    }

    #[test]
    fn stars_count_met_goals() {
        let t = task("thirty-two").unwrap();
        // Correct and under the limit, but one instruction too long for the size star.
        let sol =
            Solution::Code("LD r0, [lane]\nADD r1, r0, r0\nADD r0, r0, r1\nST [lane], r0".into());
        let rep = grade(t, &sol).unwrap();
        assert!(rep.passed());
        assert_eq!(rep.goals_met, [false, false]);
        assert_eq!(rep.stars(), 1);
        let o = rep.outcome(t).unwrap();
        assert_eq!((o.metric, o.alt), (4.0, 4.0));
    }

    #[test]
    fn probes_read_one_thread() {
        let t = task("thirty-two").unwrap();
        let rep = grade(t, &t.reference.solution()).unwrap();
        let case = &rep.runs[0].case;
        let p = probe(t, &rep.program, case, 5).unwrap();
        assert_eq!(p.regs[0], case.rom[5] * 3);
        assert!(probe(t, &rep.program, case, 32).is_none());
    }

    #[test]
    fn timeouts_are_reported() {
        let t = task("thirty-two").unwrap();
        let spin = "LOOP 64\nLOOP 64\nLOOP 64\nADD r0, r0, 1\nENDLOOP\nENDLOOP\nENDLOOP";
        let rep = grade(t, &Solution::Code(spin.into())).unwrap();
        assert_eq!(rep.verdict, Verdict::Timeout { case: 0 });
    }
}
