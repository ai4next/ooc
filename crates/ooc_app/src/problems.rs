//! Open problems: the hub's entry row, the bench's score pills and brief,
//! and the histogram that places a passing score among other solutions.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_base::Lang;
use oo_kit::theme::mix;
use oo_kit::ui::{ButtonSpec, Tone, Ui};
use ooc_core::content::{self, open::Open, open::opens};
use ooc_core::save::SaveData;

use crate::state::{Act, Screen, t};
use crate::{Ctx, View, bench};

/// Solutions in the histogram besides the player's.
const POPULATION: usize = 480;
/// Bars across the histogram.
const BINS: usize = 28;

/// Scores standing in for other solutions, spread around the problem's
/// known approaches: few match the best, most sit near the later ones, and
/// a tail runs past the slowest. The same `seed` gives the same population.
pub fn population(marks: &[u32], seed: u64) -> Vec<u32> {
    let mut x = seed | 1;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x >> 11) as f32 / (1u64 << 53) as f32
    };
    let marks: Vec<f32> = if marks.is_empty() {
        vec![1.0]
    } else {
        marks.iter().map(|&m| m.max(1) as f32).collect()
    };
    let (best, worst) = (marks[0], marks[marks.len() - 1]);
    // Later approaches are the more common ones: weights 1, 4, 9…
    let weights: Vec<f32> = (1..=marks.len()).map(|i| (i * i) as f32).collect();
    let total: f32 = weights.iter().sum();
    (0..POPULATION)
        .map(|_| {
            let v = if next() < 0.22 {
                worst * (next() * 1.1).exp()
            } else {
                let mut r = next() * total;
                let i = weights
                    .iter()
                    .position(|w| {
                        r -= w;
                        r < 0.0
                    })
                    .unwrap_or(marks.len() - 1);
                // Roughly normal, sigma about 8 %.
                let g = next() + next() + next() - 1.5;
                marks[i] * (g * 0.16).exp()
            };
            (v.round() as u32).max(best as u32)
        })
        .collect()
}

/// The problem's population, seeded by its id.
pub fn population_of(o: &Open) -> Vec<u32> {
    let seed = o
        .task
        .id
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
    population(o.marks, seed)
}

/// Percent of `pop` that `score` beats (lower is better); ties count half.
pub fn better_than(pop: &[u32], score: u32) -> u32 {
    if pop.is_empty() {
        return 100;
    }
    let worse = pop.iter().filter(|&&p| p > score).count() as f32;
    let tied = pop.iter().filter(|&&p| p == score).count() as f32;
    ((worse + tied / 2.0) / pop.len() as f32 * 100.0).floor() as u32
}

/// Histogram bars over a score range: one bar per value when the range is
/// narrow, else equal steps on a log scale so a long tail stays readable.
#[derive(Debug)]
pub struct Bins {
    /// `edges[i]..edges[i + 1]` is bar `i`.
    pub edges: Vec<f32>,
    pub counts: Vec<u32>,
}

impl Bins {
    pub fn new(pop: &[u32], extra: &[u32], n: usize) -> Self {
        let all = || pop.iter().chain(extra).copied();
        let lo = all().min().unwrap_or(1).max(1) as f32;
        let hi = all().max().unwrap_or(1).max(1) as f32;
        let edges: Vec<f32> = if hi - lo + 1.0 <= n as f32 {
            (0..=(hi - lo) as usize + 1).map(|i| lo - 0.5 + i as f32).collect()
        } else {
            let (a, b) = ((lo * 0.97).ln(), (hi * 1.03).ln());
            (0..=n).map(|i| (a + (b - a) * i as f32 / n as f32).exp()).collect()
        };
        let mut b = Bins {
            counts: vec![0; edges.len() - 1],
            edges,
        };
        for &p in pop {
            let i = b.index(p);
            b.counts[i] += 1;
        }
        b
    }

    pub fn len(&self) -> usize {
        self.counts.len()
    }

    /// The bar `v` falls in (clamped to the ends).
    pub fn index(&self, v: u32) -> usize {
        let i = self.edges.partition_point(|&e| e <= v as f32);
        i.saturating_sub(1).min(self.len() - 1)
    }

    /// Where `v` sits across the histogram, 0 at the left edge, 1 at the right.
    pub fn at(&self, v: u32) -> f32 {
        let i = self.index(v);
        let (a, b) = (self.edges[i], self.edges[i + 1]);
        let f = ((v as f32 - a) / (b - a)).clamp(0.0, 1.0);
        (i as f32 + f) / self.len() as f32
    }
}

/// A chapter's open problems unlock with its commission (slot 8).
pub fn chapter_open(ch: u8, save: &SaveData) -> bool {
    content::in_chapter(ch)
        .find(|t| t.slot == 8)
        .is_some_and(|t| save.campaign.is_solved(t.id))
}

/// The problems of chapter `ch`, with their indices into `opens()`.
fn in_chapter(ch: u8) -> impl Iterator<Item = (usize, &'static Open)> {
    opens().iter().enumerate().filter(move |(_, o)| o.task.chapter == ch)
}

/// The score of the session's current report, if it passed and is shown.
fn this_run(v: &View, o: &Open) -> Option<u32> {
    let s = v.s;
    s.report
        .as_ref()
        .filter(|_| s.play.revealed)
        .and_then(|r| o.score_of(r))
}

fn upper(s: &str, l: Lang) -> String {
    if l == Lang::En { s.to_uppercase() } else { s.to_string() }
}

pub fn act(cx: &mut Ctx, a: Act) {
    match a {
        Act::Problem(k) => {
            bench::open_problem(&mut cx.s, &cx.game, k);
            cx.go(Screen::Bench);
        }
        // From the histogram: the chapter's next problem, else the hall.
        Act::Next => {
            let next = cx.s.open.map_or(0, |k| k + 1);
            let ch = cx.s.task().chapter;
            match opens().get(next).filter(|o| o.task.chapter == ch) {
                Some(_) => {
                    bench::open_problem(&mut cx.s, &cx.game, next);
                    cx.go(Screen::Bench);
                }
                None => cx.go(Screen::Hub),
            }
        }
        _ => {}
    }
    cx.dirty();
}

/// The hub's row of the chapter's three problems, below the chapter info;
/// `at` is the row's top-left.
pub fn draw_hub_row(ui: &mut Ui, v: &View, ch: u8, accent: Color, at: Vec2) {
    let pal = ui.pal;
    let l = v.lang;
    let save = &v.game.save;
    let open = chapter_open(ch, save);
    let caption = if open {
        t(l, "OPEN PROBLEMS · NO STARS, JUST A SCORE", "开放题 · 不计星，只看分数").to_string()
    } else {
        format!(
            "{} · {} {ch}-8",
            t(l, "OPEN PROBLEMS", "开放题"),
            t(l, "OPENS AFTER", "通过后开放：")
        )
    };
    ui.caption(
        caption,
        at,
        if open { accent } else { pal.text_faint },
        Anchor::CENTER_LEFT,
    );
    let (w, pitch) = (132.0, 140.0);
    for (n, (k, o)) in in_chapter(ch).enumerate() {
        let p = Vec2::new(at.x + w / 2.0 + n as f32 * pitch, at.y - 28.0);
        let title = o.task.title.get(l);
        let max = if l == Lang::En { 17 } else { 8 };
        let label: String = if title.chars().count() > max {
            title.chars().take(max - 1).chain(['…']).collect()
        } else {
            title.to_string()
        };
        let best = save.open_best.get(o.task.id);
        let unit = o.label();
        let tip = match best {
            Some(b) => format!(
                "{title} · {} {b} {} · ORI {}",
                t(l, "best", "最好"),
                unit.get(l),
                o.marks[0]
            ),
            None => format!("{title} · ORI {} {}", o.marks[0], unit.get(l)),
        };
        ui.button(
            ButtonSpec::new(p, Vec2::new(w, 28.0), label, Act::Problem(k))
                .tone(Tone::Ghost)
                .font(12.0)
                .enabled(open)
                .tip(tip),
        );
        if best.is_some() {
            ui.dot(p + Vec2::new(w / 2.0 - 8.0, 9.0), 7.0, pal.good, 12.0);
        }
    }
}

/// Bench top bar: the best score so far and this run's.
pub fn draw_pills(ui: &mut Ui, v: &View, k: usize, y: f32) {
    let pal = ui.pal;
    let l = v.lang;
    let o = &opens()[k];
    let unit = upper(o.label().get(l), l);
    let best = v.game.save.open_best.get(o.task.id);
    ui.pill(
        Vec2::new(110.0, y),
        232.0,
        &format!("{} · {unit}", t(l, "BEST", "最好")),
        best.map_or("—".into(), |b| b.to_string()),
        pal.good,
    );
    let run = this_run(v, o);
    ui.pill(
        Vec2::new(350.0, y),
        232.0,
        &format!("{} · {unit}", t(l, "THIS RUN", "本次")),
        run.map_or("—".into(), |r| r.to_string()),
        if run.is_some() { pal.secondary } else { pal.text_dim },
    );
}

/// The Brief tab of an open problem: the task, how it is scored, the best.
pub fn draw_brief(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let Some(k) = v.s.open else { return };
    let o = &opens()[k];
    let unit = o.label().get(l);
    ui.wrapped(
        o.task.brief.get(l),
        Vec2::new(-612.0, -134.0),
        700.0,
        15.0,
        pal.text_dim,
        Anchor::TOP_LEFT,
    );
    ui.caption(
        t(l, "SCORING", "计分"),
        Vec2::new(150.0, -140.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    ui.wrapped(
        format!(
            "{} {unit}{}",
            t(l, "Ranked by", "按"),
            t(
                l,
                ", lower is better. No stars: every passing run lands on a histogram of other solutions.",
                "排名，越低越好。不计星：每次通过都会落在其他解法的分布图上。",
            )
        ),
        Vec2::new(150.0, -158.0),
        240.0,
        13.0,
        pal.text_dim,
        Anchor::TOP_LEFT,
    );
    ui.caption(
        t(l, "YOUR BEST", "你的最好成绩"),
        Vec2::new(420.0, -140.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let best = v.game.save.open_best.get(o.task.id);
    ui.text(
        best.map_or("—".into(), |b| format!("{b} {unit}")),
        Vec2::new(420.0, -172.0),
        22.0,
        if best.is_some() { pal.good } else { pal.text_faint },
        Anchor::CENTER_LEFT,
    );
    ui.caption(
        format!("{} {} {unit}", t(l, "ORI's own:", "ORI 的解："), o.marks[0]),
        Vec2::new(420.0, -204.0),
        pal.warn,
        Anchor::CENTER_LEFT,
    );
    if o.task.starter.solution() == *v.s.work() {
        ui.caption(
            t(l, "No starter: the board is yours.", "没有初始作品：从空白开始。"),
            Vec2::new(420.0, -228.0),
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
    }
}

/// The verdict line of a passing run: the score and where it ranks.
pub fn draw_verdict(ui: &mut Ui, v: &View, k: usize, at: Vec2) {
    let pal = ui.pal;
    let l = v.lang;
    let o = &opens()[k];
    let Some(score) = this_run(v, o) else { return };
    let pct = better_than(&population_of(o), score);
    ui.text(
        format!(
            "✓ {score} {} · {} {pct}%",
            o.label().get(l),
            t(l, "better than", "胜过"),
        ),
        at,
        14.0,
        pal.good,
        Anchor::CENTER_LEFT,
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(548.0, at.y),
            Vec2::new(120.0, 30.0),
            t(l, "Histogram ›", "分布图 ›"),
            Act::Next,
        )
        .primary()
        .font(14.0),
    );
}

/// After a passing run: the score, the best, and the histogram.
pub fn draw_results(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let Some(k) = s.open else { return };
    let o = &opens()[k];
    let task = &o.task;
    let unit = o.label().get(l);
    let score = this_run(v, o);
    let best = v.game.save.open_best.get(task.id).copied();
    let pop = population_of(o);
    let x0 = -600.0;
    ui.caption(
        format!(
            "{} · {} · {}",
            task.number(),
            content::chapter(task.chapter).machine.get(l),
            t(l, "OPEN PROBLEM", "开放题")
        ),
        Vec2::new(x0, 300.0),
        pal.secondary,
        Anchor::CENTER_LEFT,
    );
    ui.text(task.title.get(l), Vec2::new(x0, 266.0), 30.0, pal.text, Anchor::CENTER_LEFT);
    if let Some(sc) = score {
        let at = Vec2::new(x0, 206.0);
        ui.text(format!("{sc}"), at, 46.0, pal.good, Anchor::CENTER_LEFT);
        let w = 28.0 * sc.to_string().len() as f32;
        ui.text(unit, at + Vec2::new(w + 12.0, -6.0), 18.0, pal.text_dim, Anchor::CENTER_LEFT);
        if s.improved {
            crate::fx::echo(ui, at + Vec2::new(w / 2.0, 0.0), 0.6);
        }
        ui.text(
            format!(
                "{} {}% {}",
                t(l, "Better than", "胜过"),
                better_than(&pop, sc),
                t(l, "of the solutions on record.", "的已存档解法。")
            ),
            Vec2::new(x0, 154.0),
            16.0,
            pal.text,
            Anchor::CENTER_LEFT,
        );
    }
    let line = match (s.improved, best) {
        (true, _) => t(l, "A new best for this problem.", "这道题的新纪录。").to_string(),
        (false, Some(b)) => format!("{} {b} {unit}", t(l, "Your best stays at", "最好成绩仍是")),
        (false, None) => String::new(),
    };
    let color = if s.improved { pal.secondary } else { pal.text_dim };
    ui.text(line, Vec2::new(x0, 126.0), 14.0, color, Anchor::CENTER_LEFT);
    ui.caption(
        format!("{} {} {unit}", t(l, "ORI's own:", "ORI 的解："), o.marks[0]),
        Vec2::new(600.0, 206.0),
        pal.warn,
        Anchor::CENTER_RIGHT,
    );
    draw_histogram(ui, v, o, &pop, score, best);
    let y = -300.0;
    ui.button(
        ButtonSpec::new(
            Vec2::new(-470.0, y),
            Vec2::new(160.0, 40.0),
            t(l, "‹ Bench", "‹ 工作台"),
            Act::Goto(Screen::Bench),
        )
        .tone(Tone::Ghost),
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(-300.0, y),
            Vec2::new(160.0, 40.0),
            t(l, "Grid Hall", "栅厅"),
            Act::Back,
        )
        .tone(Tone::Ghost),
    );
    let more = opens()
        .get(k + 1)
        .is_some_and(|n| n.task.chapter == task.chapter);
    ui.button(
        ButtonSpec::new(
            Vec2::new(500.0, y),
            Vec2::new(220.0, 48.0),
            if more {
                t(l, "Next problem ›", "下一题 ›")
            } else {
                t(l, "Back to the hall ›", "回到栅厅 ›")
            },
            Act::Next,
        )
        .primary()
        .font(17.0),
    );
}

/// Bars of the population; this run's bar lit, the known approaches and
/// the player's best marked on the axis.
fn draw_histogram(
    ui: &mut Ui,
    v: &View,
    o: &Open,
    pop: &[u32],
    score: Option<u32>,
    best: Option<u32>,
) {
    let pal = ui.pal;
    let l = v.lang;
    let center = Vec2::new(0.0, -60.0);
    ui.card(center, Vec2::new(1240.0, 300.0), pal.secondary);
    ui.caption(
        t(l, "HOW OTHER SOLUTIONS SCORED", "其他解法的得分分布"),
        Vec2::new(-596.0, 70.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let extra: Vec<u32> = score.into_iter().chain(best).chain(o.marks.iter().copied()).collect();
    let bins = Bins::new(pop, &extra, BINS);
    let (x0, w, base, h) = (-560.0, 1120.0, -140.0, 160.0);
    let x_of = |v: u32| x0 + bins.at(v) * w;
    let pitch = w / bins.len() as f32;
    let top = bins.counts.iter().copied().max().unwrap_or(1).max(1) as f32;
    let mine = score.map(|s| bins.index(s));
    for (i, &c) in bins.counts.iter().enumerate() {
        let color = match mine {
            Some(m) if i == m => pal.secondary,
            Some(m) if i < m => mix(pal.good, pal.bg, 0.45),
            _ => mix(pal.text_dim, pal.bg, 0.55),
        };
        let bh = (c as f32 / top * h).max(if c > 0 { 3.0 } else { 0.0 });
        if bh > 0.0 {
            let x = x0 + (i as f32 + 0.5) * pitch;
            ui.rect(
                Vec2::new(x, base + bh / 2.0),
                Vec2::new((pitch - 3.0).max(2.0), bh),
                color,
                5.0,
            );
        }
    }
    ui.rect(Vec2::new(0.0, base - 1.0), Vec2::new(w + 16.0, 1.0), pal.line, 5.1);
    // Known approaches: ORI's in amber, the rest as plain ticks.
    for (i, &m) in o.marks.iter().enumerate() {
        let color = if i == 0 { pal.warn } else { pal.text_faint };
        ui.dot(Vec2::new(x_of(m), base - 12.0), 7.0, color, 5.2);
        ui.caption(m.to_string(), Vec2::new(x_of(m), base - 28.0), color, Anchor::CENTER);
    }
    if let Some(b) = best.filter(|b| Some(*b) != score) {
        let x = x_of(b);
        ui.rect(Vec2::new(x, base + h / 2.0), Vec2::new(1.0, h), pal.good.with_alpha(0.6), 5.3);
        ui.caption(
            format!("{} {b}", t(l, "BEST", "最好")),
            Vec2::new(x, base + h + 12.0),
            pal.good,
            Anchor::CENTER,
        );
    }
    if let Some(s) = score {
        let x = x_of(s);
        ui.glow(Vec2::new(x, base + h / 2.0), 90.0, pal.secondary.with_alpha(0.12), 4.9);
        ui.rect(Vec2::new(x, base + h / 2.0 + 8.0), Vec2::new(2.0, h + 16.0), pal.secondary, 5.4);
        ui.caption(
            format!("{} · {s}", t(l, "YOU", "你")),
            Vec2::new(x, base + h + 30.0),
            pal.secondary,
            Anchor::CENTER,
        );
    }
    let axis = format!(
        "{} · {}",
        upper(o.label().get(l), l),
        t(l, "LOWER IS BETTER ←", "越低越好 ←")
    );
    ui.caption(axis, Vec2::new(0.0, base - 54.0), pal.text_dim, Anchor::CENTER);
    let lo = bins.edges[0].ceil().max(1.0) as u32;
    let hi = bins.edges[bins.len()].floor() as u32;
    ui.caption(lo.to_string(), Vec2::new(x0, base - 54.0), pal.text_faint, Anchor::CENTER_LEFT);
    ui.caption(hi.to_string(), Vec2::new(x0 + w, base - 54.0), pal.text_faint, Anchor::CENTER_RIGHT);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn population_is_stable_and_never_beats_the_best() {
        for o in opens() {
            let a = population_of(o);
            assert_eq!(a, population_of(o), "{}", o.task.id);
            assert_eq!(a.len(), POPULATION);
            assert!(a.iter().all(|&p| p >= o.marks[0].max(1)), "{}", o.task.id);
        }
    }

    #[test]
    fn the_reference_ranks_high_and_the_slowest_low() {
        for o in opens() {
            let pop = population_of(o);
            assert!(better_than(&pop, o.marks[0]) >= 85, "{}", o.task.id);
            let worst = *pop.iter().max().unwrap();
            assert_eq!(better_than(&pop, worst + 1), 0);
        }
    }

    #[test]
    fn bins_hold_every_score() {
        let narrow = Bins::new(&[4, 5, 5, 6], &[3], BINS);
        assert_eq!(narrow.len(), 4);
        assert_eq!(narrow.counts, vec![0, 1, 2, 1]);
        let wide = population(&[509, 2122, 6642], 7);
        let b = Bins::new(&wide, &[100], BINS);
        assert_eq!(b.len(), BINS);
        assert_eq!(b.counts.iter().sum::<u32>() as usize, wide.len());
        assert_eq!(b.index(100), 0);
        assert!(b.at(100) >= 0.0 && b.at(*wide.iter().max().unwrap()) <= 1.0);
    }
}
