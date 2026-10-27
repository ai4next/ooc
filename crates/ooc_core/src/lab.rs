//! Free experiments use the campaign's real machines and replay engine.
//! Only the objective, grading budget, injected fault and hidden cases are removed.

use std::sync::LazyLock;

use crate::content;
use crate::task::{Case, Editor, Goal, Metric, Task};

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

/// A persistent template. Its index and save key match the source task.
pub fn task(index: usize) -> &'static Task {
    &LABS[index]
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
}
