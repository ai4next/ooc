//! Times the design §8 performance targets on reference solutions and prints
//! the median of several runs: one Lumen case, one Atlas frame of 4096
//! threads, and one character of the origin machine's first words.
//! `cargo run -p ooc_core --example perf --release`

use std::time::{Duration, Instant};

use ooc_core::content::{task, tasks};
use ooc_core::grade::{build, run_case};
use ooc_core::task::{Case, Task};

const REPS: usize = 15;

/// Median wall time of `REPS` runs of `case` on `t`'s reference.
fn time(t: &Task, case: &Case) -> (Duration, usize) {
    let built = build(t, &t.reference.solution()).unwrap();
    let threads = t.launch_for(&built.program, case).threads;
    let mut v: Vec<Duration> = (0..REPS)
        .map(|_| {
            let c = case.clone();
            let at = Instant::now();
            let r = run_case(t, &built.program, c);
            let d = at.elapsed();
            assert!(r.ok(), "{}: reference fails", t.id);
            d
        })
        .collect();
    v.sort();
    (v[REPS / 2], threads)
}

fn all_cases(t: &Task) -> Vec<Case> {
    (t.cases)().into_iter().chain((t.hidden)()).collect()
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

/// Median and worst of the per-case medians, with the worst case's task.
fn summary(rows: &mut [(Duration, &'static str)]) -> (f64, f64, &'static str) {
    rows.sort();
    let (worst, id) = rows[rows.len() - 1];
    (ms(rows[rows.len() / 2].0), ms(worst), id)
}

fn main() {
    // Lumen: every case of chapters 1–4.
    let mut lumen: Vec<(Duration, &'static str)> = tasks()
        .iter()
        .filter(|t| t.chapter <= 4)
        .flat_map(|t| all_cases(t).into_iter().map(move |c| (time(t, &c).0, t.id)))
        .collect();
    let first = time(
        task("first-light").unwrap(),
        &(task("first-light").unwrap().cases)()[0],
    )
    .0;
    let (med, worst, id) = summary(&mut lumen);
    println!(
        "Lumen case        target <1 ms   first-light {:.3} ms; {} cases median {med:.3} ms, worst {worst:.3} ms ({id})",
        ms(first),
        lumen.len()
    );

    // Atlas: every case of chapters 10–12, grouped by launch size; frames
    // over the target are listed with their thread and cycle counts.
    let mut atlas: Vec<(Duration, &'static str)> = Vec::new();
    let mut big = 0;
    let mut slow = Vec::new();
    for t in tasks().iter().filter(|t| (10..=12).contains(&t.chapter)) {
        for c in all_cases(t) {
            let (d, n) = time(t, &c);
            if n == 4096 {
                atlas.push((d, t.id));
            } else if n > 4096 {
                big += 1;
            }
            if n >= 4096 && ms(d) > 20.0 {
                let built = build(t, &t.reference.solution()).unwrap();
                let run = run_case(t, &built.program, c).run;
                slow.push((ms(d), t.id, n, run.stats.cycles, run.trace.len()));
            }
        }
    }
    let an = task("the-analyzer").unwrap();
    let an_case = all_cases(an)
        .into_iter()
        .find(|c| c.rom[0] == 4096)
        .unwrap_or_else(|| (an.cases)()[0].clone());
    let (an_t, an_n) = time(an, &an_case);
    let (med, worst, id) = summary(&mut atlas);
    println!(
        "Atlas 4096 frame  target <20 ms  the-analyzer ({an_n} threads) {:.2} ms; {} frames of exactly 4096 threads median {med:.2} ms, worst {worst:.2} ms ({id}); {big} larger frames",
        ms(an_t),
        atlas.len()
    );
    slow.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (d, id, n, cycles, steps) in slow {
        let per = d * 1e6 / steps as f64;
        println!(
            "  over 20 ms: {id:<16} {n:>6} threads {cycles:>7} cycles {steps:>7} warp-steps {d:>8.2} ms ({per:.0} ns/step)"
        );
    }

    // Origin: first-words from the start token says 15 characters; with a
    // 14-character prefix it says one after filling the cache.
    let fw = task("first-words").unwrap();
    let full = (fw.cases)()[0].clone();
    let said = full.rom[0] as u32;
    let (d, _) = time(fw, &full);
    let last = (fw.hidden)().last().unwrap().clone();
    let (d1, _) = time(fw, &last);
    println!(
        "Origin character  target <50 ms  first-words {:.2} ms per character ({said} in {:.2} ms); one character after a 14-character prefix {:.2} ms",
        ms(d) / said as f64,
        ms(d),
        ms(d1)
    );
}
