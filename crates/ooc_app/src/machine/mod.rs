//! The machine tabs of the bench strip: Memory (Vault on), Analyzer (Atlas
//! on), Tensor (Lyra on) and the machine room (Origin). Each reads the
//! selected case's run; the replay position picks the moment.

pub mod analyzer;
pub mod mem;
pub mod room;
pub mod tensor;

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_kit::ui::Ui;
use ooc_core::grade::{CaseRun, Report};
use ooc_core::vm::Step;

use crate::state::{Session, Tab};

/// Left edge of the strip's content and its title row.
pub const X0: f32 = -612.0;
pub const TITLE_Y: f32 = -136.0;

/// The tabs this task's machine has, in tab-row order.
pub fn tabs(task: &ooc_core::task::Task) -> Vec<Tab> {
    let p = task.profile;
    let mut v = vec![Tab::River];
    if p.latency > 1 {
        v.push(Tab::Pipeline);
    }
    if p.vault {
        v.push(Tab::Memory);
    }
    if p.atlas {
        v.push(Tab::Analyzer);
    }
    if p.tensor {
        v.push(Tab::Tensor);
    }
    if p.link.is_some() {
        v.push(Tab::Room);
    }
    v.extend([Tab::Brief, Tab::Hints, Tab::Listing]);
    v
}

/// Tabs whose drawing follows the replay step.
pub fn follows_replay(tab: Tab) -> bool {
    matches!(
        tab,
        Tab::Pipeline | Tab::Memory | Tab::Analyzer | Tab::Tensor | Tab::Room
    )
}

/// The report and the run of the selected case.
pub fn run(s: &Session) -> Option<(&Report, &CaseRun)> {
    let rep = s.report.as_ref()?;
    Some((rep, &rep.runs[s.case.min(rep.runs.len() - 1)]))
}

/// The replay's latest step.
pub fn latest(s: &Session) -> Option<(usize, &Step)> {
    let (_, r) = run(s)?;
    let i = s.play.step.checked_sub(1)?;
    r.run.trace.get(i).map(|st| (i, st))
}

/// The cycle the replay has reached (Vault and later machines count cycles).
pub fn cycle_now(s: &Session) -> u32 {
    latest(s).map_or(0, |(_, st)| st.cycle)
}

/// Title caption plus a help paragraph, for a tab with nothing run yet.
pub fn empty(ui: &mut Ui, title: &str, help: &str) {
    let pal = ui.pal;
    ui.caption(
        title,
        Vec2::new(X0, TITLE_Y),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    ui.wrapped(
        help,
        Vec2::new(X0, TITLE_Y - 22.0),
        900.0,
        14.0,
        pal.text_dim,
        Anchor::TOP_LEFT,
    );
}

/// A small legend swatch with its label; returns the x after it.
pub fn swatch(ui: &mut Ui, at: Vec2, color: Color, label: &str) -> f32 {
    let pal = ui.pal;
    ui.chip(at, Vec2::splat(10.0), color, 9.0);
    ui.text(
        label,
        at + Vec2::new(10.0, 0.0),
        12.0,
        pal.text_dim,
        Anchor::CENTER_LEFT,
    );
    at.x + 24.0 + label.chars().map(|c| if c.is_ascii() { 6.6 } else { 12.0 }).sum::<f32>()
}

/// Percent with no decimals.
pub fn pct(f: f32) -> String {
    format!("{:.0}%", (f * 100.0).clamp(0.0, 100.0))
}
