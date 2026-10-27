//! The Pipeline tab: one warp's recent instructions against the beats they
//! issue and land on. Reads that come before their result are drawn red.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_kit::ui::{ButtonSpec, Tone, Ui};
use ooc_core::pipe::{Row, window};

use crate::View;
use crate::state::{Act, t};

const ROWS: usize = 6;
const ROW_H: f32 = 25.0;
/// Left edge of the note about the newest beat.
const NOTE_X: f32 = 330.0;
const TOP: f32 = -164.0;
/// Left edge of the beat grid and the width of one beat.
const GRID_X: f32 = -300.0;
const BEAT_W: f32 = 50.0;
const BEATS: u32 = 12;

pub fn draw(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let task = s.task();
    let lat = task.profile.latency;
    let shown = s.report.as_ref().and_then(|rep| {
        let run = &rep.runs[s.case.min(rep.runs.len() - 1)].run;
        window(&rep.program, &run.trace, s.play.step, task.profile, ROWS)
    });
    let Some((warp, rows)) = shown else {
        ui.caption(
            t(l, "PIPELINE", "流水线"),
            Vec2::new(-612.0, -136.0),
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
        let help = match lat {
            1 => t(
                l,
                "Every result is ready on the next beat.",
                "每个结果下一拍就能用。",
            ),
            _ => t(
                l,
                "Run the program, then step through it. Each row is one instruction of one warp; each column is one of that warp's beats. A result lands three beats after it issues, and nothing stops a read that comes sooner: it gets the old value.",
                "运行程序，然后逐步查看。每一行是一个 warp 的一条指令，每一列是这个 warp 的一拍。结果在发出三拍后才落地，而没有任何东西会拦住更早的读：它读到的是旧值。",
            ),
        };
        ui.wrapped(
            help,
            Vec2::new(-612.0, -158.0),
            900.0,
            14.0,
            pal.text_dim,
            Anchor::TOP_LEFT,
        );
        return;
    };
    ui.caption(
        format!("{} · WARP {warp}", t(l, "PIPELINE", "流水线")),
        Vec2::new(-612.0, -136.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let last = rows.last().map_or(1, |r| r.beat);
    // Keep the newest beat and its landing in view.
    let first = (last + lat).saturating_sub(BEATS - 1).max(1);
    let col = |beat: u32| GRID_X + (beat - first) as f32 * BEAT_W + BEAT_W / 2.0;
    for b in first..first + BEATS {
        ui.text(
            b.to_string(),
            Vec2::new(col(b), -136.0),
            11.0,
            if b == last {
                pal.secondary
            } else {
                pal.text_faint
            },
            Anchor::CENTER,
        );
    }
    ui.caption(
        t(l, "BEAT", "拍"),
        Vec2::new(GRID_X - 8.0, -136.0),
        pal.text_faint,
        Anchor::CENTER_RIGHT,
    );
    let x = col(last);
    ui.rect(
        Vec2::new(x, TOP - (ROWS as f32 - 1.0) * ROW_H / 2.0),
        Vec2::new(BEAT_W - 4.0, ROWS as f32 * ROW_H + 6.0),
        pal.secondary.with_alpha(0.06),
        6.0,
    );
    for (i, row) in rows.iter().enumerate() {
        draw_row(ui, v, row, &rows, TOP - i as f32 * ROW_H, first);
    }
    draw_legend(ui, l);
    draw_note(ui, v, &rows, lat);
}

/// What the newest beat did, on the right of the grid.
fn draw_note(ui: &mut Ui, v: &View, rows: &[Row], lat: u32) {
    let pal = ui.pal;
    let l = v.lang;
    let Some(row) = rows.last() else { return };
    let x = NOTE_X;
    ui.caption(
        t(l, "THIS BEAT", "这一拍"),
        Vec2::new(x, -164.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    ui.text(
        row.instr.to_string(),
        Vec2::new(x, -190.0),
        16.0,
        pal.text,
        Anchor::CENTER_LEFT,
    );
    let (msg, color) = match row.early {
        Some((reg, from)) => (
            t(
                l,
                "Reads r{r} on beat {b}, but the result from beat {f} lands on beat {w}. Nothing waits: it gets the old value.",
                "在第 {b} 拍读 r{r}，可第 {f} 拍的结果要到第 {w} 拍才落地。没有谁会等：它读到的是旧值。",
            )
            .replace("{r}", &reg.to_string())
            .replace("{b}", &row.beat.to_string())
            .replace("{f}", &from.to_string())
            .replace("{w}", &(from + lat).to_string()),
            pal.bad,
        ),
        None => (
            t(
                l,
                "Every register it reads had already landed.",
                "它读的每个寄存器都已经落地了。",
            )
            .to_string(),
            pal.good,
        ),
    };
    ui.wrapped(
        msg,
        Vec2::new(x, -212.0),
        280.0,
        13.0,
        color,
        Anchor::TOP_LEFT,
    );
}

fn draw_row(ui: &mut Ui, v: &View, row: &Row, rows: &[Row], y: f32, first: u32) {
    let pal = ui.pal;
    let l = v.lang;
    let latest = rows.last() == Some(row);
    let label = format!("{:>2}  {}", row.pc + 1, row.instr);
    ui.button(
        ButtonSpec::new(
            Vec2::new(-458.0, y),
            Vec2::new(300.0, ROW_H - 4.0),
            label,
            Act::Seek(row.step + 1),
        )
        .font(13.0)
        .tone(Tone::Ghost)
        .selected(latest)
        .tip(t(
            l,
            "Rewind the replay to this beat.",
            "把回放倒回到这一拍。",
        )),
    );
    let col = |beat: u32| {
        (beat >= first && beat < first + BEATS)
            .then(|| GRID_X + (beat - first) as f32 * BEAT_W + BEAT_W / 2.0)
    };
    let cell = Vec2::new(BEAT_W - 6.0, ROW_H - 8.0);
    // Issue, then in flight, then the beat the result can be read.
    let issue = if row.early.is_some() {
        pal.bad
    } else {
        pal.primary
    };
    if let Some(x) = col(row.beat) {
        ui.chip(Vec2::new(x, y), cell, issue.with_alpha(0.85), 8.0);
        let name = if row.early.is_some() {
            t(l, "OLD", "旧值")
        } else {
            t(l, "ISSUE", "发射")
        };
        ui.text(name, Vec2::new(x, y), 10.0, pal.bg, Anchor::CENTER);
    }
    let Some(lands) = row.lands else { return };
    for b in row.beat + 1..lands {
        if let Some(x) = col(b) {
            ui.chip(Vec2::new(x, y), cell, pal.primary.with_alpha(0.22), 8.0);
        }
    }
    // A landing someone read too early is outlined in red.
    let missed = rows
        .iter()
        .any(|r| r.early.is_some_and(|(_, from)| from == row.beat));
    if let Some(x) = col(lands) {
        let edge = if missed { pal.bad } else { pal.good };
        ui.chip(Vec2::new(x, y), cell, edge.with_alpha(0.75), 8.0);
        ui.chip(Vec2::new(x, y), cell - Vec2::splat(3.0), pal.bg, 8.5);
        let reg = row.instr.dest().map_or(String::new(), |d| format!("r{d}"));
        ui.text(reg, Vec2::new(x, y), 11.0, edge, Anchor::CENTER);
    }
}

fn draw_legend(ui: &mut Ui, l: oo_base::Lang) {
    let pal = ui.pal;
    let items = [
        (pal.primary, t(l, "issues", "发射")),
        (pal.primary.with_alpha(0.22), t(l, "in flight", "执行中")),
        (pal.good, t(l, "result lands", "结果落地")),
        (pal.bad, t(l, "read before it landed", "落地前就读了")),
    ];
    for (i, (c, label)) in items.into_iter().enumerate() {
        let p = Vec2::new(GRID_X + 10.0 + i as f32 * 150.0, -318.0);
        ui.chip(p, Vec2::splat(10.0), c, 7.0);
        ui.text(
            label,
            p + Vec2::new(11.0, 0.0),
            12.0,
            pal.text_dim,
            Anchor::CENTER_LEFT,
        );
    }
}
