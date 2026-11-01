//! Every diagnostic `why` (design §5.1) has a fault level whose starter the
//! evidence finder catches at the moment syllabus.md names, and whose
//! reference gives it nothing to point at (or, where the reference still
//! does the risky thing safely, no failing case to ask about). One test per
//! fault level, named after the why.

use oo_base::Evidence;
use ooc_core::content::task;
use ooc_core::grade::{self, CaseRun, Report, grade};
use ooc_core::isa::{OpKind, Program};
use ooc_core::task::Task;
use ooc_core::vm::Step;

type Find = fn(&Program, &CaseRun) -> Option<Evidence>;

/// A fault starter's run: its first failing case and the evidence on it.
struct Caught {
    rep: Report,
    case: usize,
    ev: Evidence,
}

impl Caught {
    fn run(&self) -> &CaseRun {
        &self.rep.runs[self.case]
    }

    /// The trace step the evidence starts at.
    fn step(&self) -> &Step {
        &self.run().run.trace[self.ev.from as usize]
    }

    /// Opcode family of the instruction at the evidence step.
    fn kind(&self) -> OpKind {
        self.rep.program.code[self.step().pc as usize].kind()
    }
}

/// The task's own clue finder.
fn clue(id: &str) -> Find {
    task(id).unwrap().clue.expect("fault has a clue").find
}

/// Grades the starter of `id`, finds evidence with `find` on the first
/// failing case, and checks it is the one the bench would show: same as the
/// task's clue, inside the trace, on the warp of its first step.
fn caught(id: &str, find: Find) -> Caught {
    let t: &Task = task(id).unwrap();
    let rep = grade(t, &t.starter.solution()).unwrap();
    let case = rep
        .runs
        .iter()
        .position(|r| r.fails(t))
        .unwrap_or_else(|| panic!("{id}: starter fails no case"));
    let run = &rep.runs[case];
    let ev = find(&rep.program, run).unwrap_or_else(|| panic!("{id}: no evidence"));
    assert_eq!((t.clue.unwrap().find)(&rep.program, run), Some(ev), "{id}");
    assert!(ev.from <= ev.to, "{id}");
    assert!(
        (ev.to as usize) < run.run.trace.len(),
        "{id}: window past the trace"
    );
    let warp = run.run.trace[ev.from as usize].warp as u32;
    assert!(ev.accepts(ev.from, Some(warp)), "{id}");
    Caught { rep, case, ev }
}

/// The reference of `id` gives `find` nothing to point at in any case.
fn clean(id: &str, find: Find) {
    let t = task(id).unwrap();
    let rep = grade(t, &t.reference.solution()).unwrap();
    assert!(rep.passed(), "{id}");
    for (i, r) in rep.runs.iter().enumerate() {
        assert_eq!(
            find(&rep.program, r),
            None,
            "{id}: evidence in reference case {i}"
        );
    }
}

/// The reference of `id` may still do what `find` looks for (a guarded
/// SHFL past the edge, an RCP whose result is discarded), but no case fails,
/// so the bench never asks for evidence.
fn quiet(id: &str) -> Report {
    let t = task(id).unwrap();
    let rep = grade(t, &t.reference.solution()).unwrap();
    assert!(rep.runs.iter().all(|r| !r.fails(t)), "{id}");
    rep
}

/// The evidence step writes a cell the case still has wrong at the end.
fn writes_a_wrong_cell(c: &Caught) -> bool {
    let run = c.run();
    c.step()
        .writes
        .iter()
        .any(|&(a, _)| run.wrong.contains(&(a as usize)))
}

/// The evidence step writes a value outside tolerance of what the case wants.
fn writes_a_bad_value(c: &Caught) -> bool {
    let run = c.run();
    c.step().writes.iter().any(|&(a, v)| {
        run.expect
            .get(a as usize)
            .copied()
            .flatten()
            .is_some_and(|w| w.abs_diff(v) > run.tolerance)
    })
}

/// Wrong-output faults: the finder points at the first bad write, and that
/// write lands on a cell the case gets wrong.
fn wrong_write(id: &str) -> Caught {
    let c = caught(id, grade::first_wrong_write);
    assert!(writes_a_bad_value(&c), "{id}");
    assert!(
        writes_a_wrong_cell(&c),
        "{id}: first bad write is overwritten later"
    );
    c
}

// 坐标 (coordinates)
#[test]
fn coord_upside_down() {
    wrong_write("upside-down");
    clean("upside-down", grade::first_wrong_write);
}

// 溢出 (overflow): the counter wraps at 15, so the first bad frame is past it.
#[test]
fn overflow_the_seam() {
    let c = wrong_write("the-seam");
    assert!(c.run().case.frame >= 15, "frame {}", c.run().case.frame);
    clean("the-seam", grade::first_wrong_write);
}

// 寻址 (addressing)
#[test]
fn addressing_the_skew() {
    wrong_write("the-skew");
    clean("the-skew", grade::first_wrong_write);
}

// 撕裂 (tearing): the bad write goes into the buffer the beam is showing.
#[test]
fn tearing_the_flicker() {
    let c = wrong_write("the-flicker");
    let front = (c.run().case.frame & 1) as usize;
    assert!(
        c.step()
            .writes
            .iter()
            .all(|&(a, _)| a as usize / 256 == front),
        "writes outside the front buffer"
    );
    clean("the-flicker", grade::first_wrong_write);
}

// 串行 (serial): a load of the loop's second round fetching again.
#[test]
fn serial_the_crawl() {
    let c = caught("the-crawl", grade::first_repeat_load);
    assert_eq!(c.kind(), OpKind::Ld);
    let (w, pc) = (c.step().warp, c.step().pc);
    let trace = &c.run().run.trace[..c.ev.from as usize];
    assert!(
        trace.iter().any(|s| (s.warp, s.pc) == (w, pc)),
        "no first round"
    );
    clean("the-crawl", grade::first_repeat_load);
}

// 寻址 (addressing): a SHFL past lane 0 or 31 that wrapped round.
#[test]
fn addressing_edge_lane() {
    let c = caught("edge-lane", grade::first_wrapped_shfl);
    assert_eq!(c.kind(), OpKind::Shfl);
    assert!(c.step().wrapped);
    quiet("edge-lane");
}

// 发散 (divergence): a store with lanes 16–31 masked off.
#[test]
fn divergence_the_fork() {
    let c = caught("the-fork", grade::first_masked_store);
    assert_eq!(c.kind(), OpKind::St);
    let m = c.step().mask;
    assert!(m != u32::MAX && m != 0, "mask {m:#x}");
    clean("the-fork", grade::first_masked_store);
}

// 发散 (divergence): a SHFL reading a lane the mask switched off.
#[test]
fn divergence_ghost_lane() {
    let c = caught("ghost-lane", grade::first_ghost_shfl);
    assert_eq!(c.kind(), OpKind::Shfl);
    assert_ne!(c.step().mask, u32::MAX);
    clean("ghost-lane", grade::first_ghost_shfl);
}

// 冒险读旧值 (hazard, stale read)
#[test]
fn stale_read_the_stale() {
    let c = caught("the-stale", grade::first_stale_read);
    assert!(c.step().stale);
    clean("the-stale", grade::first_stale_read);
}

// 溢出 (overflow): a multiply that wrapped 32 bits.
#[test]
fn overflow_fixed_overflow() {
    let c = caught("fixed-overflow", grade::first_overflow);
    assert!(c.step().overflow);
    clean("fixed-overflow", grade::first_overflow);
}

// 裁剪 (clipping): RCP of a vertex at or behind w = 0.
#[test]
fn clipping_behind_you() {
    let c = caught("behind-you", grade::first_behind);
    assert_eq!(c.kind(), OpKind::Rcp);
    quiet("behind-you");
}

// 插值 (interpolation)
#[test]
fn interpolation_persp_correct() {
    wrong_write("persp-correct");
    clean("persp-correct", grade::first_wrong_write);
}

// 缓存缺失 (cache miss): the read whose line left L1 with one word used.
#[test]
fn cache_miss_the_stride() {
    let c = caught("the-stride", grade::first_wasted_line);
    assert_eq!(c.kind(), OpKind::Ld);
    let mem = &c.run().run.mem;
    assert!(
        mem.evictions
            .iter()
            .any(|e| e.fill_step == c.ev.from && e.used <= 1),
        "no wasted line filled here"
    );
    clean("the-stride", grade::first_wasted_line);
}

// 缓存冲突 (cache conflict): a read bringing back a line its set threw out.
#[test]
fn cache_conflict_the_alias() {
    let c = caught("the-alias", grade::first_refill);
    assert_eq!(c.kind(), OpKind::Ld);
    let mem = &c.run().run.mem;
    let f = mem.fills.iter().find(|f| f.refill).unwrap();
    assert_eq!(f.step, c.ev.from);
    assert!(
        mem.evictions
            .iter()
            .any(|e| (e.set, e.tag) == (f.set, f.tag) && e.cycle <= f.cycle),
        "line was never evicted from set {}",
        f.set
    );
    clean("the-alias", grade::first_refill);
}

// 未合并 (uncoalesced): a read scattered over 16 or more lines.
#[test]
fn uncoalesced_column_walk() {
    let c = caught("column-walk", grade::first_uncoalesced);
    assert_eq!(c.kind(), OpKind::Ld);
    assert!(c.step().lines >= 16, "{} lines", c.step().lines);
    clean("column-walk", grade::first_uncoalesced);
}

// bank 冲突 (bank conflict): a shared read that needed several passes.
#[test]
fn bank_conflict_the_label() {
    let c = caught("the-label", grade::first_bank_conflict);
    assert_eq!(c.kind(), OpKind::Lds);
    assert!(c.step().passes >= 2);
    clean("the-label", grade::first_bank_conflict);
}

// 寻址 (addressing): a sample half a texel off.
#[test]
fn addressing_half_texel() {
    wrong_write("half-texel");
    clean("half-texel", grade::first_wrong_write);
}

// 寻址 (addressing): a sample past the texture edge.
#[test]
fn addressing_wrap_mode() {
    wrong_write("wrap-mode");
    clean("wrap-mode", grade::first_wrong_write);
}

// 竞争 (race): two blocks write the same cell before a GSYNC.
#[test]
fn race_the_collide() {
    let c = caught("the-collide", grade::first_collide);
    assert!(c.step().race_w);
    assert!(!c.step().writes.is_empty(), "the collision is a write");
    clean("the-collide", grade::first_collide);
}

// 竞争 (race): a read of a neighbour another block already updated.
#[test]
fn race_the_race() {
    let c = caught("the-race", grade::first_race_read);
    assert!(c.step().race_r);
    clean("the-race", grade::first_race_read);
}

// 竞争 (race): a higher warp appended before a lower one.
#[test]
fn race_shuffled_order() {
    let c = caught("shuffled-order", grade::first_out_of_order);
    assert_eq!(c.kind(), OpKind::Atom);
    let (w, prog) = (c.step().warp, &c.rep.program);
    let later = &c.run().run.trace[c.ev.from as usize + 1..];
    assert!(
        later
            .iter()
            .any(|s| s.warp < w && prog.code[s.pc as usize].kind() == OpKind::Atom),
        "no lower warp appends later"
    );
    clean("shuffled-order", grade::first_out_of_order);
}

// 负载不均 (load imbalance): one warp left running on the whole machine.
#[test]
fn imbalance_the_hub() {
    let c = caught("the-hub", grade::first_lonely);
    assert!(c.step().lonely);
    let w = c.step().warp;
    let after = &c.run().run.trace[c.ev.from as usize..];
    assert!(
        after.iter().all(|s| s.warp == w),
        "another warp issues later"
    );
    // Every run ends with a last warp; the reference's lone tail is shorter.
    let r = quiet("the-hub");
    let tail = |run: &CaseRun, ev: Option<Evidence>| {
        ev.map_or(0, |e| run.run.trace.len() - e.from as usize)
    };
    let (ref_run, ours) = (&r.runs[c.case], after.len());
    let theirs = tail(ref_run, grade::first_lonely(&r.program, ref_run));
    assert!(
        theirs < ours,
        "reference alone for {theirs} steps, starter {ours}"
    );
}

// 占用率 (occupancy): the first register spill to local memory.
#[test]
fn occupancy_the_spill() {
    let c = caught("the-spill", grade::first_spill);
    assert!(
        matches!(c.kind(), OpKind::Ldl | OpKind::Stl),
        "{:?}",
        c.kind()
    );
    clean("the-spill", grade::first_spill);
}

// 乱序可见 (out-of-order visibility): flag visible, data not yet.
#[test]
fn reorder_the_fence() {
    let c = caught("the-fence", grade::first_reordered);
    assert!(c.step().reorder);
    clean("the-fence", grade::first_reordered);
}

// 寻址 (addressing): a load past the end of B, into the guard zone.
#[test]
fn addressing_the_ragged() {
    let c = caught("the-ragged", clue("the-ragged"));
    let r = &c.run().case.rom;
    let end = 32 + r[0] * r[2] + r[2] * r[1];
    assert!(c.step().rmax >= end, "read {} < end {end}", c.step().rmax);
    let before = &c.run().run.trace[..c.ev.from as usize];
    assert!(
        before.iter().all(|s| s.rmax < end),
        "an earlier read was out"
    );
    clean("the-ragged", clue("the-ragged"));
}

// 累加 (accumulation): an accumulator holding a partial sum set back to 0.
#[test]
fn accumulate_acc_reset() {
    let c = caught("acc-reset", grade::first_acc_reset);
    assert!(c.step().reset);
    assert_ne!(c.kind(), OpKind::Mma);
    clean("acc-reset", grade::first_acc_reset);
}

// 溢出 (overflow): an FP16 result that went to infinity.
#[test]
fn overflow_overflow() {
    let c = caught("overflow", grade::first_inf);
    assert!(c.step().inf);
    clean("overflow", grade::first_inf);
}

// 饱和 (saturation): an INT8 conversion clamped at the limit.
#[test]
fn saturation_the_clipped() {
    let c = caught("the-clipped", grade::first_clamp);
    assert!(matches!(c.kind(), OpKind::Cvt(_)), "{:?}", c.kind());
    assert!(c.step().sat);
    clean("the-clipped", grade::first_clamp);
}

// 互等 (mutual wait): every SM blocked; the evidence is on one of them.
#[test]
fn mutual_wait_deadlock() {
    let c = caught("deadlock", grade::first_deadlock);
    let d = c.run().run.deadlock.as_ref().expect("starter deadlocks");
    assert!(
        d.sms.contains(&c.step().sm),
        "SM {} is not waiting",
        c.step().sm
    );
    clean("deadlock", grade::first_deadlock);
}

// 链路争用 (link contention): a SEND on the busiest link. Every run has a
// busiest link, so the reference is checked for a cooler one instead.
#[test]
fn link_contention_hot_link() {
    let find = clue("hot-link");
    let c = caught("hot-link", find);
    let msgs = &c.run().run.msgs;
    let mut words = [[0u32; 4]; 4];
    for m in msgs {
        words[m.from as usize % 4][m.to as usize % 4] += m.len as u32;
    }
    let hot = words.iter().flatten().copied().max().unwrap();
    let sm = c.step().sm as usize;
    assert!(
        words[sm].contains(&hot),
        "SM {sm} does not send on the busiest link"
    );
    let t = task("hot-link").unwrap();
    let r = grade(t, &t.reference.solution()).unwrap();
    assert!(r.measure.hot < c.rep.measure.hot);
}

// 静默错误 (silent error): the read of a message with a flipped bit. The
// flip hits the reference too; it must survive it, so no case fails.
#[test]
fn silent_error_the_bitflip() {
    let c = caught("the-bitflip", grade::first_flip);
    let m = c
        .run()
        .run
        .msgs
        .iter()
        .find(|m| m.flipped.is_some())
        .unwrap();
    assert_eq!(c.step().sm, m.to);
    assert!(c.step().cycle >= m.arrive);
    let t = task("the-bitflip").unwrap();
    let r = grade(t, &t.reference.solution()).unwrap();
    assert!(r.runs.iter().all(|run| !run.fails(t)));
    assert!(
        r.runs
            .iter()
            .any(|run| grade::first_flip(&r.program, run).is_some())
    );
}

// 负载不均 (load imbalance): SM 2 working alone while the others wait.
#[test]
fn imbalance_the_straggler() {
    let find = clue("the-straggler");
    let c = caught("the-straggler", find);
    assert!(c.ev.to > c.ev.from);
    let window = &c.run().run.trace[c.ev.from as usize..=c.ev.to as usize];
    assert!(
        window.iter().all(|s| s.sm == 2),
        "another SM issues in the window"
    );
    clean("the-straggler", find);
}

/// Every fault level is covered above.
#[test]
fn every_fault_has_a_why_test() {
    let src = include_str!("why.rs");
    for t in ooc_core::content::tasks() {
        if t.clue.is_some() {
            assert!(
                src.contains(&format!("(\"{}\"", t.id)),
                "{} has no why test",
                t.id
            );
        }
    }
}
