//! Free experiments use the campaign's real machines and replay engine.
//! Only the objective, grading budget, injected fault and hidden cases are removed.

use std::sync::LazyLock;

use crate::content;
use crate::isa::{FB_LEN, FB_W, WIDE_LEN, WIDE_W};
use crate::task::{Case, Editor, Goal, Metric, Task, View};
use crate::vm::Launch;

pub mod code;
pub mod frame;
pub mod input;
pub mod slot;

static LABS: LazyLock<Vec<Task>> = LazyLock::new(|| {
    content::tasks()
        .iter()
        .enumerate()
        .map(|(index, source)| {
            let mut task = source.clone();
            task.editor = match source.editor {
                Editor::Code(_) => {
                    let mut ops = Vec::new();
                    for prior in &content::tasks()[..=index] {
                        if let Editor::Code(available) = prior.editor {
                            ops.extend_from_slice(available);
                        }
                    }
                    ops.sort();
                    ops.dedup();
                    Editor::Code(Box::leak(ops.into_boxed_slice()))
                }
                Editor::Graph(_) => {
                    let mut nodes = Vec::new();
                    for prior in &content::tasks()[..=index] {
                        if let Editor::Graph(available) = prior.editor {
                            for &node in available {
                                if !nodes.contains(&node) {
                                    nodes.push(node);
                                }
                            }
                        }
                    }
                    Editor::Graph(Box::leak(nodes.into_boxed_slice()))
                }
            };
            task.expect = no_target;
            task.hidden = no_cases;
            task.limit = None;
            task.clue = None;
            task.flips = None;
            task.reg_cap = None;
            task.goals = [
                Goal::new(Metric::Cycles, u32::MAX),
                Goal::new(Metric::Size, u32::MAX),
            ];
            task
        })
        .collect()
});

fn no_target(_: &Case) -> Vec<Option<i32>> {
    Vec::new()
}

fn no_cases() -> Vec<Case> {
    Vec::new()
}

/// Screen levels again with the other screen size: 32 × 32 for a 16 × 16
/// machine and the other way round. Same machine, one thread per pixel.
static RESIZED: LazyLock<Vec<Option<Task>>> = LazyLock::new(|| {
    LABS.iter()
        .map(|lab| {
            if lab.view != View::Screen || lab.launch.threads != lab.out_len {
                return None;
            }
            let mut t = lab.clone();
            let (w, len) = if lab.profile.fb_w == WIDE_W {
                (FB_W, FB_LEN)
            } else {
                (WIDE_W, WIDE_LEN)
            };
            t.profile.fb_w = w;
            t.launch = Launch::flat(len);
            t.out_len = len;
            Some(t)
        })
        .collect()
});

/// A persistent template. Its index and save key match the source task.
pub fn task(index: usize) -> &'static Task {
    &LABS[index]
}

/// The template with a `width`-pixel-wide screen, where the level has a
/// screen that can change size; otherwise the same as [`task`].
pub fn sized(index: usize, width: usize) -> &'static Task {
    let lab = task(index);
    if lab.profile.fb_w as usize == width {
        return lab;
    }
    RESIZED[index].as_ref().filter(|_| width == 16 || width == 32).unwrap_or(lab)
}

/// Whether the level's screen can be switched between 16 and 32 wide.
pub fn resizable(index: usize) -> bool {
    RESIZED[index].is_some()
}

/// The position of task `id` in the campaign.
pub fn index(id: &str) -> Option<usize> {
    content::tasks().iter().position(|t| t.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grade::{Verdict, grade};
    use crate::task::Solution;

    #[test]
    fn experiments_preserve_the_machine_and_have_no_answer_to_match() {
        let index = content::tasks()
            .iter()
            .position(|t| t.id == "first-light")
            .unwrap();
        let source = &content::tasks()[index];
        let lab = task(index);
        assert_eq!(lab.profile, source.profile);
        assert_eq!(lab.launch, source.launch);
        assert!(lab.limit.is_none() && lab.clue.is_none());
        let report = grade(lab, &source.starter.solution()).unwrap();
        assert_eq!(report.verdict, Verdict::Pass);
        assert_eq!(report.runs.len(), report.public);
        assert!(report.runs.iter().all(|r| r.expect.is_empty()));
    }

    #[test]
    fn sandbox_still_checks_programs_and_hardware_limits() {
        let index = content::tasks()
            .iter()
            .position(|t| matches!(t.editor, Editor::Code(_)))
            .unwrap();
        assert!(grade(task(index), &Solution::Code("NOT_AN_INSTRUCTION".into())).is_err());
        let source = &content::tasks()[index];
        assert!(grade(task(index), &source.reference.solution()).is_ok());
    }

    #[test]
    fn screens_switch_between_16_and_32() {
        let i = index("the-lut").unwrap();
        assert!(resizable(i));
        let wide = sized(i, 32);
        assert_eq!(wide.profile.fb_w, 32);
        assert_eq!((wide.launch.threads, wide.out_len), (1024, 1024));
        assert!(std::ptr::eq(sized(i, 16), task(i)));
        let lanes = content::tasks().iter().position(|t| t.view == View::Lanes).unwrap();
        assert!(!resizable(lanes));
        assert!(std::ptr::eq(sized(lanes, 32), task(lanes)));
        // X and Y follow the wider screen: x = 31 on the last column.
        let sol = Solution::Code("PIX x".into());
        let f = frame::run(wide, &[sol], &input::Input::Level).unwrap();
        let out = &f.kernels[0].runs[0].out;
        assert_eq!((out[31], out[32], out[1023]), (31, 0, 31));
    }

    #[test]
    fn built_in_sets_are_fixed_and_fit_the_screen() {
        for s in input::Set::ALL {
            for side in [16, 32] {
                let a = s.cases(side);
                assert_eq!(a, s.cases(side), "{s:?} is deterministic");
                assert!(!a.is_empty());
                for c in &a {
                    assert_eq!(c.rom.len(), side * side, "{s:?}");
                    assert!(c.rom.iter().all(|v| (0..=s.max()).contains(v)), "{s:?}");
                    // Not blank: every set shows something.
                    assert!(c.rom.iter().any(|&v| v != c.rom[0]), "{s:?} {side}");
                }
            }
        }
        assert_eq!(input::Set::Digits.cases(16).len(), 10);
    }

    #[test]
    fn imports_read_csv_and_pictures() {
        let d = input::parse_csv("t.csv", "1, 2,3\n4,5,6\n7\n").unwrap();
        assert_eq!((d.w, d.values.len(), d.h()), (3, 7, 3));
        assert!(input::parse_csv("x", "1,two").is_err());
        assert!(input::parse_csv("x", "\n").is_err());
        // A 2 x 1 picture: pure red and pure blue.
        let rgba = [255, 0, 0, 255, 0, 0, 255, 255];
        let pal = [[0, 0, 0], [250, 10, 10], [10, 10, 250]];
        let q = input::quantise("p", 2, 1, &rgba, 2, &pal);
        assert_eq!(q.values, vec![1, 2, 1, 2]);
        let l = input::luminance("p", 2, 1, &rgba);
        assert_eq!((l.w, l.values.clone()), (2, vec![76, 29]));
    }

    #[test]
    fn kernels_chain_and_cycles_add_up() {
        let i = index("the-palette").unwrap();
        let t = task(i);
        let first = Solution::Code("PIX 3".into());
        let second = Solution::Code("LDO r0, [tid]\nADD r0, r0, 1\nPIX r0".into());
        let one = frame::run(t, std::slice::from_ref(&first), &input::Input::Level).unwrap();
        let two = frame::run(t, &[first, second.clone()], &input::Input::Level).unwrap();
        assert_eq!(two.kernels.len(), 2);
        let k1 = &two.kernels[1].runs[0];
        assert!(k1.init.iter().all(|&v| v == 3));
        assert!(k1.out.iter().all(|&v| v == 4));
        assert_eq!(two.cycles(), one.cycles() + two.kernels[1].measure.cycles);
        assert_eq!(two.split(0).iter().sum::<u32>(), two.cycles());
        let bad = frame::run(t, &[second, Solution::Code("NOPE".into())], &input::Input::Level);
        assert_eq!(bad.err().map(|e| e.0), Some(1));
    }

    #[test]
    fn sets_feed_the_rom() {
        let i = index("the-lut").unwrap();
        let sol = Solution::Code("LD r0, [tid]\nPIX r0".into());
        let input = input::Input::Set(input::Set::Pictures);
        let f = frame::run(task(i), &[sol], &input).unwrap();
        assert_eq!(f.kernels[0].runs.len(), 4);
        assert_eq!(f.kernels[0].runs[0].out, input::Set::Pictures.cases(16)[0].rom);
    }

    #[test]
    fn share_codes_round_trip() {
        let slot = slot::LabSlot {
            name: String::new(),
            machine: "the-lut".into(),
            kernels: vec![Solution::Code("PIX 7 ; 中".into())],
            input: input::Input::Data(input::Data {
                name: "a.csv".into(),
                w: 2,
                values: vec![1, -2, 300, 4],
            }),
            screen: 32,
        };
        let code = code::encode(&slot, 12);
        assert!(code.starts_with("OOC-"));
        let back = code::decode(&code).unwrap();
        assert_eq!((back.slot, back.cycles, back.cut), (slot.clone(), 12, false));
        let mut big = slot.clone();
        big.input = input::Input::Data(input::Data {
            name: "big".into(),
            w: 64,
            values: vec![1000; 4096],
        });
        let back = code::decode(&code::encode(&big, 5)).unwrap();
        assert!(back.cut && back.slot.input == input::Input::Level);
        let mut typo = code.clone().into_bytes();
        let n = typo.len() - 6;
        typo[n] = if typo[n] == b'A' { b'B' } else { b'A' };
        assert!(code::decode(&String::from_utf8(typo).unwrap()).is_err());
        assert!(code::decode("OOE-AAAA-AAAA").is_err());
    }

    #[test]
    fn teaching_marks_count_the_wait() {
        let prog = crate::asm::assemble("MOV r0, 5\nADD r1, r0, 1\nST [lane], r1").unwrap();
        let mut out = vec![0; 32];
        let r = crate::vm::run_frame(&prog, &[], Launch::ONE_WARP, crate::vm::Profile::PRISM, 0, &mut out);
        let m = frame::marks(&prog, &r.trace, crate::vm::Profile::PRISM);
        assert_eq!(m, vec![(1, frame::Mark::Early(2)), (2, frame::Mark::Early(2))]);
        let prog = crate::asm::assemble("LD r0, [lane]\nADD r0, r0, 1\nST [lane], r0").unwrap();
        let rom = vec![1; 32];
        let r = crate::vm::run_frame(&prog, &rom, Launch::ONE_WARP, crate::vm::Profile::VAULT, 0, &mut out);
        let m = frame::marks(&prog, &r.trace, crate::vm::Profile::VAULT);
        assert!(matches!(m.first(), Some((1, frame::Mark::Wait(n))) if *n > 100), "{m:?}");
        assert!(frame::marks(&prog, &r.trace, crate::vm::Profile::LUMEN).is_empty());
    }
}
