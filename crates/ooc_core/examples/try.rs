//! Grades a source file against a task: `cargo run --example try -- <id> <file>`.
//! `<file>` may be `ref` or `starter` for the shipped solutions.

use ooc_core::{content, grade::grade, task::Solution};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let t = content::task(&args[0]).expect("unknown task");
    let sol = match args.get(1).map(String::as_str) {
        Some("ref") | None => t.reference.solution(),
        Some("starter") => t.starter.solution(),
        Some(path) => Solution::Code(std::fs::read_to_string(path).expect("read")),
    };
    let rep = match grade(t, &sol) {
        Ok(r) => r,
        Err(e) => return println!("build error: {e:?}"),
    };
    println!("{:?} {:?} stars={}", rep.verdict, rep.measure, rep.stars());
    for (i, r) in rep.runs.iter().enumerate() {
        let first = r.run.trace.iter().position(|s| s.stale);
        println!(
            "case {i}{}: wrong={} stale={} first_stale={:?}",
            if rep.is_hidden(i) { " (hidden)" } else { "" },
            r.wrong.len(),
            r.run.stats.stale,
            first.map(|i| (i, r.run.trace[i].pc + 1)),
        );
    }
}
