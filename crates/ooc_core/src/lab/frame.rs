//! A lab frame: one to four kernels run back to back on the same ROM.
//! Kernel k + 1 starts from the buffer kernel k left behind, and the
//! frame's cycles are the kernels' cycles added up.

use crate::grade::{BuildError, CaseRun, Measure, Report, Verdict, build};
use crate::isa::Program;
use crate::task::{Solution, Task};
use crate::vm::{Profile, Step, run_frame_with};

use super::input::{Input, side};

/// Most kernels a frame may chain.
pub const MAX_KERNELS: usize = 4;

#[derive(Clone, Debug)]
pub struct Frame {
    /// One report per kernel, in order; every one has the same cases.
    pub kernels: Vec<Report>,
}

impl Frame {
    /// Cycles of the whole frame: per case the kernels' cycles summed,
    /// then the slowest case.
    pub fn cycles(&self) -> u32 {
        let cases = self.kernels.first().map_or(0, |r| r.runs.len());
        (0..cases)
            .map(|c| self.kernels.iter().map(|r| r.runs[c].run.stats.cycles).sum())
            .max()
            .unwrap_or(0)
    }

    /// Cycles of case `c`, kernel by kernel.
    pub fn split(&self, c: usize) -> Vec<u32> {
        self.kernels
            .iter()
            .map(|r| r.runs.get(c).map_or(0, |run| run.run.stats.cycles))
            .collect()
    }

    /// The first kernel that hit the watchdog, if any.
    pub fn timeout(&self) -> Option<usize> {
        self.kernels.iter().position(|r| r.verdict != Verdict::Pass)
    }
}

/// Builds every kernel, then runs them in order on each case of `input`.
/// A build error names the kernel it came from.
pub fn run(task: &Task, kernels: &[Solution], input: &Input) -> Result<Frame, (usize, BuildError)> {
    let built = kernels
        .iter()
        .enumerate()
        .map(|(k, sol)| build(task, sol).map_err(|e| (k, e)))
        .collect::<Result<Vec<_>, _>>()?;
    let cases = input.cases(task, side(task));
    // Imported or built-in ROMs are not shaped like the level's, so the
    // launch and the blank buffer come from the level's first case.
    let shape = match input {
        Input::Level => None,
        _ => Some((task.cases)().into_iter().next().unwrap_or_default()),
    };
    let mut bufs: Vec<Vec<i32>> = cases
        .iter()
        .map(|c| {
            let mut init = (task.init)(shape.as_ref().unwrap_or(c));
            init.resize(task.out_len, 0);
            init
        })
        .collect();
    let mut reports = Vec::with_capacity(built.len());
    for b in built {
        let runs: Vec<CaseRun> = cases
            .iter()
            .zip(&mut bufs)
            .map(|(case, buf)| {
                let init = buf.clone();
                let launch = task.launch_for(&b.program, shape.as_ref().unwrap_or(case));
                let run = run_frame_with(&b.program, &case.rom, launch, task.profile, case.frame, &[], buf);
                CaseRun {
                    case: case.clone(),
                    init,
                    out: buf.clone(),
                    expect: Vec::new(),
                    tolerance: 0,
                    run,
                    wrong: Vec::new(),
                }
            })
            .collect();
        let mut measure = Measure {
            size: b.size as u32,
            regs: b.program.regs_used() as u32,
            ..Default::default()
        };
        for r in &runs {
            let s = &r.run.stats;
            measure.cycles = measure.cycles.max(s.cycles);
            measure.loads = measure.loads.max(s.loads);
            measure.stores = measure.stores.max(s.stores);
            measure.txns = measure.txns.max(s.txns);
            measure.misses = measure.misses.max(s.misses);
            measure.smem = measure.smem.max(s.smem);
            measure.atoms = measure.atoms.max(s.atoms);
            measure.link = measure.link.max(s.link_words);
            measure.hot = measure.hot.max(s.hot_link);
        }
        let verdict = runs
            .iter()
            .position(|r| r.run.timeout)
            .map_or(Verdict::Pass, |case| Verdict::Timeout { case });
        reports.push(Report {
            program: b.program,
            public: runs.len(),
            runs,
            verdict,
            measure,
            goals_met: [true; 2],
        });
    }
    Ok(Frame { kernels: reports })
}

/// What teaching mode writes over one step of the Warp River.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// The SM issued nothing for this many cycles before this step.
    Wait(u32),
    /// Prism: read a register this many beats before its result landed.
    Early(u32),
}

/// Teaching marks for a trace, one entry per step that has one.
pub fn marks(prog: &Program, trace: &[Step], profile: Profile) -> Vec<(usize, Mark)> {
    let mut out = Vec::new();
    if profile.vault {
        let mut next: Vec<u32> = Vec::new();
        for (i, st) in trace.iter().enumerate() {
            let sm = st.sm as usize;
            if next.len() <= sm {
                next.resize(sm + 1, 0);
            }
            let gap = st.cycle.saturating_sub(next[sm]);
            if gap > 0 {
                out.push((i, Mark::Wait(gap)));
            }
            next[sm] = st.cycle + 1;
        }
    } else if profile.latency > 1 || profile.units {
        // Per warp: beat of each instruction issued so far, and when its
        // result lands.
        let mut due: Vec<Vec<(u8, u32)>> = Vec::new();
        let mut beats: Vec<u32> = Vec::new();
        for (i, st) in trace.iter().enumerate() {
            let w = st.warp as usize;
            if beats.len() <= w {
                beats.resize(w + 1, 0);
                due.resize(w + 1, Vec::new());
            }
            beats[w] += 1;
            let beat = beats[w];
            let Some(&ins) = prog.code.get(st.pc as usize) else {
                continue;
            };
            due[w].retain(|&(_, d)| d > beat);
            let early = ins
                .reads()
                .flat_map(|r| due[w].iter().filter(move |p| p.0 == r).map(|p| p.1 - beat))
                .max();
            if let Some(n) = early {
                out.push((i, Mark::Early(n)));
            }
            if let Some(d) = ins.dest() {
                due[w].push((d, beat + profile.lat(&ins)));
            }
        }
    }
    out
}
