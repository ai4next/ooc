//! Open problems (design §4.2): a goal and a scoring function, no stars and
//! no starting program. Three per chapter; the score is placed on a histogram.

use std::sync::LazyLock;

use oo_base::Tr;

use crate::grade::Report;
use crate::task::{Metric, Task};

mod c01;
mod c02;
mod c03;
mod c04;
mod c05;
mod c06;
mod c07;
mod c08;
mod c09;
mod c10;
mod c11;
mod c12;
mod c13;
mod c14;
mod c15;
mod c16;

/// Open problems per chapter.
pub const PER_CHAPTER: usize = 3;

/// One open problem. `task` carries the machine, cases, expectation and a
/// reference; its `goals` and `limit` are not used, and `starter` is empty.
/// `task.chapter` is the chapter and `task.slot` is 9, 10 or 11.
#[derive(Clone, Debug)]
pub struct Open {
    pub task: Task,
    /// What the histogram ranks, lower is better.
    pub metric: Metric,
    /// Optional score computed from a passing report instead of `metric`
    /// (e.g. an error in tenths of a percent). Lower is better.
    pub score: Option<fn(&Report) -> u32>,
    /// What `score` counts, shown on the histogram axis; `None` uses the
    /// metric's label.
    pub score_label: Option<Tr>,
    /// Scores of known approaches, best first; `marks[0]` is the reference's
    /// score. The histogram's population is spread around them.
    pub marks: &'static [u32],
}

impl Open {
    /// The axis label of the histogram.
    pub fn label(&self) -> Tr {
        self.score_label.unwrap_or_else(|| self.metric.label())
    }

    /// The score of a passing report; `None` if the run did not pass.
    pub fn score_of(&self, rep: &Report) -> Option<u32> {
        if !rep.passed() {
            return None;
        }
        Some(match self.score {
            Some(f) => f(rep),
            None => rep.measure.get(self.metric),
        })
    }
}

static OPENS: LazyLock<Vec<Open>> = LazyLock::new(|| {
    let mut v = Vec::new();
    for f in [
        c01::problems,
        c02::problems,
        c03::problems,
        c04::problems,
        c05::problems,
        c06::problems,
        c07::problems,
        c08::problems,
        c09::problems,
        c10::problems,
        c11::problems,
        c12::problems,
        c13::problems,
        c14::problems,
        c15::problems,
        c16::problems,
    ] {
        v.extend(f());
    }
    v.sort_by_key(|o| (o.task.chapter, o.task.slot));
    v
});

/// Every open problem, in chapter order.
pub fn opens() -> &'static [Open] {
    &OPENS
}

/// The open problem with this id.
pub fn open(id: &str) -> Option<&'static Open> {
    OPENS.iter().find(|o| o.task.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::tasks;
    use crate::grade::grade;
    use crate::task::Source;

    #[test]
    fn three_per_chapter_in_slots_9_to_11() {
        for ch in 1..=16u8 {
            let slots: Vec<u8> = opens()
                .iter()
                .filter(|o| o.task.chapter == ch)
                .map(|o| o.task.slot)
                .collect();
            assert_eq!(slots, vec![9, 10, 11], "chapter {ch}");
        }
        assert_eq!(opens().len(), 16 * PER_CHAPTER);
    }

    #[test]
    fn ids_are_unique_across_levels_and_opens() {
        let mut ids: Vec<&str> = tasks().iter().map(|t| t.id).collect();
        ids.extend(opens().iter().map(|o| o.task.id));
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }

    #[test]
    fn every_reference_scores_and_starters_are_empty() {
        for o in opens() {
            let t = &o.task;
            match t.starter {
                Source::Code(s) => assert!(s.trim().is_empty(), "{} starter", t.id),
                Source::Graph(f) => assert!(f().nodes.is_empty(), "{} starter", t.id),
            }
            let rep =
                grade(t, &t.reference.solution()).unwrap_or_else(|e| panic!("{}: {e:?}", t.id));
            let score = o.score_of(&rep);
            assert!(score.is_some(), "{}: {:?}", t.id, rep.verdict);
            assert_eq!(score, o.marks.first().copied(), "{} marks[0]", t.id);
            assert!(o.marks.is_sorted(), "{} marks ascending", t.id);
        }
    }
}
