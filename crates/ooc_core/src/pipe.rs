//! The pipeline view's model: one warp's recent beats, when each result
//! lands, and which reads came before the result they wanted.

use crate::isa::{Instr, Program};
use crate::vm::{Profile, Step};

/// One issued instruction of the shown warp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    /// Index into the trace.
    pub step: usize,
    /// The warp's own beat count when it issued (1 = its first instruction).
    pub beat: u32,
    pub pc: u16,
    pub instr: Instr,
    /// The beat its register result can first be read, if it writes one.
    pub lands: Option<u32>,
    /// A register read before its result landed, and the beat that result
    /// was issued on.
    pub early: Option<(u8, u32)>,
}

/// The last `rows` instructions the warp of `trace[upto - 1]` issued, up to
/// and including that step. `None` before the first step.
pub fn window(
    prog: &Program,
    trace: &[Step],
    upto: usize,
    profile: Profile,
    rows: usize,
) -> Option<(u16, Vec<Row>)> {
    let upto = upto.min(trace.len());
    let warp = trace.get(upto.checked_sub(1)?)?.warp;
    let lat = profile.latency.max(1);
    let mut all: Vec<Row> = Vec::new();
    for (step, st) in trace[..upto].iter().enumerate() {
        if st.warp != warp {
            continue;
        }
        let instr = prog.code[st.pc as usize];
        let beat = all.len() as u32 + 1;
        // Same rule as the machine: a result issued on beat j is pending
        // until beat j + latency.
        let early = instr.reads().find_map(|r| {
            all.iter()
                .rev()
                .find(|p| p.instr.dest() == Some(r) && p.beat + lat > beat)
                .map(|p| (r, p.beat))
        });
        all.push(Row {
            step,
            beat,
            pc: st.pc,
            instr,
            lands: instr.dest().map(|_| beat + lat),
            early,
        });
    }
    let skip = all.len().saturating_sub(rows);
    Some((warp, all.split_off(skip)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asm::assemble;
    use crate::vm::{Launch, run_frame};

    #[test]
    fn early_reads_match_the_machine() {
        let p = assemble("MOV r0, 5\nST [lane], r0\nMOV r1, 0\nMOV r1, 0\nST [lane], r0").unwrap();
        let mut out = vec![0; 64];
        let r = run_frame(&p, &[], Launch::ONE_WARP, Profile::PRISM, 0, &mut out);
        let (w, rows) = window(&p, &r.trace, r.trace.len(), Profile::PRISM, 8).unwrap();
        assert_eq!(w, 0);
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[0].lands, Some(4));
        assert_eq!(rows[1].early, Some((0, 1)));
        assert_eq!(rows[4].early, None);
        for (row, st) in rows.iter().zip(&r.trace) {
            assert_eq!(row.early.is_some(), st.stale);
        }
    }

    #[test]
    fn shows_only_the_latest_warp_and_the_last_rows() {
        let p = assemble("ADD r0, x, y\nNOP\nNOP\nPIX r0").unwrap();
        let mut out = vec![0; 256];
        let r = run_frame(&p, &[], Launch::SCREEN, Profile::PRISM, 0, &mut out);
        // Round-robin: step 10 is warp 2's second instruction.
        let (w, rows) = window(&p, &r.trace, 11, Profile::PRISM, 8).unwrap();
        assert_eq!(w, 2);
        assert_eq!(rows.iter().map(|r| r.beat).collect::<Vec<_>>(), [1, 2]);
        let (_, rows) = window(&p, &r.trace, r.trace.len(), Profile::PRISM, 3).unwrap();
        assert_eq!(rows.iter().map(|r| r.beat).collect::<Vec<_>>(), [2, 3, 4]);
        assert!(window(&p, &r.trace, 0, Profile::PRISM, 8).is_none());
    }
}
