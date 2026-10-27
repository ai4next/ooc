//! Prints every reference's measure next to its goals: `cargo run --example measure`.

use ooc_core::{content, grade::grade};

fn main() {
    for t in content::tasks() {
        let rep = grade(t, &t.reference.solution()).expect("reference builds");
        let starter = grade(t, &t.starter.solution()).expect("starter builds");
        println!(
            "{:>4} {:<18} {:?} ref={:?} stale={} limit={:?} goals={:?} starter_pass={}",
            t.number(),
            t.id,
            rep.verdict,
            rep.measure,
            rep.runs
                .iter()
                .map(|r| r.run.stats.stale)
                .max()
                .unwrap_or(0),
            t.limit,
            t.goals,
            starter.passed()
        );
    }
}
