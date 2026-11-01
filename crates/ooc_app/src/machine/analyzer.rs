//! The Analyzer tab: how each SM spent the run, how full it was, and where
//! the kernel sits under the roofline.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_kit::ui::Ui;
use ooc_core::isa::LANES;
use ooc_core::mem::{FILL_GAP, LINE};
use ooc_core::vm::{SM_WARPS, SmUse};

use super::{TITLE_Y, X0, cycle_now, empty, pct, run, swatch};
use crate::View;
use crate::state::t;

const BAR_X: f32 = -560.0;
const BAR_W: f32 = 430.0;
const ROW_Y: f32 = -168.0;
const ROW_H: f32 = 30.0;
/// The roofline plot (centre, size).
const ROOF: (Vec2, Vec2) = (Vec2::new(150.0, -232.0), Vec2::new(250.0, 160.0));
const PILL_X: f32 = 470.0;

pub fn draw(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let title = t(l, "ANALYZER · WHERE THE CYCLES WENT", "分析器 · 周期都去哪了");
    let Some((_, r)) = run(s) else {
        empty(
            ui,
            title,
            t(
                l,
                "Run the program. Each SM gets a bar: the share of cycles it issued an instruction, and the share it sat waiting on memory, at a barrier or on a result, or had nothing to run.",
                "运行程序。每个 SM 一条横条：它有多少周期在发射指令，多少周期在等内存、等同步、等结果，或者根本没活干。",
            ),
        );
        return;
    };
    let a = &r.run.analysis;
    ui.caption(
        format!("{title} · {} {}", a.total, t(l, "CYCLES", "周期")),
        Vec2::new(X0, TITLE_Y),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let parts = |u: &SmUse| {
        [
            (u.issue, pal.primary),
            (u.mem, pal.secondary),
            (u.sync, pal.tertiary),
            (u.board, pal.warn),
            (u.idle, pal.text_faint.with_alpha(0.3)),
        ]
    };
    for (i, u) in a.sms.iter().enumerate() {
        let y = ROW_Y - i as f32 * ROW_H;
        ui.text(
            format!("SM {i}"),
            Vec2::new(X0, y),
            13.0,
            pal.text_dim,
            Anchor::CENTER_LEFT,
        );
        ui.chip(
            Vec2::new(BAR_X + BAR_W / 2.0, y),
            Vec2::new(BAR_W + 4.0, 20.0),
            pal.bg_deep,
            7.0,
        );
        let total: u32 = parts(u).iter().map(|p| p.0).sum::<u32>().max(1);
        let mut x = BAR_X;
        for (n, c) in parts(u) {
            let w = n as f32 / total as f32 * BAR_W;
            if w >= 0.5 {
                ui.rect(Vec2::new(x + w / 2.0, y), Vec2::new(w, 16.0), c, 8.0);
            }
            x += w;
        }
        // Average resident warps against what the SM can hold.
        let occ = u.warp_cycles as f32 / (a.total.max(1) as f32 * SM_WARPS as f32);
        ui.text(
            pct(u.issue as f32 / a.total.max(1) as f32),
            Vec2::new(BAR_X + BAR_W + 12.0, y),
            13.0,
            pal.primary,
            Anchor::CENTER_LEFT,
        );
        ui.text(
            format!("{:.1} {}", occ * SM_WARPS as f32, t(l, "warps", "warp")),
            Vec2::new(BAR_X + BAR_W + 56.0, y),
            12.0,
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
    }
    // The replay's moment, shared with the river and the other tabs: a
    // cursor across the bars.
    if !s.play.revealed && a.total > 0 {
        let now = cycle_now(s).min(a.total);
        let x = BAR_X + now as f32 / a.total as f32 * BAR_W;
        let rows = a.sms.len() as f32 * ROW_H;
        ui.rect(
            Vec2::new(x, ROW_Y - rows / 2.0 + ROW_H / 2.0),
            Vec2::new(2.0, rows),
            pal.text.with_alpha(0.8),
            8.5,
        );
        ui.caption(
            format!("{} {now}", t(l, "REPLAY AT CYCLE", "回放到周期")),
            Vec2::new(x, ROW_Y + ROW_H / 2.0 + 6.0),
            pal.text,
            Anchor::CENTER,
        );
    }
    let y = ROW_Y - a.sms.len() as f32 * ROW_H - 4.0;
    let mut x = X0 + 4.0;
    for (c, label) in [
        (pal.primary, t(l, "issuing", "发射")),
        (pal.secondary, t(l, "waiting on memory", "等内存")),
        (pal.tertiary, t(l, "at a barrier", "等同步")),
        (pal.warn, t(l, "waiting on a result", "等结果")),
        (pal.text_faint.with_alpha(0.3), t(l, "nothing to run", "没活干")),
    ] {
        x = swatch(ui, Vec2::new(x, y), c, label);
    }
    draw_roofline(ui, v);
    let st = &r.run.stats;
    let rows: [(&str, String, Color); 5] = [
        (t(l, "Busy", "忙碌"), pct(a.busy()), pal.primary),
        (t(l, "Waiting on memory", "等内存"), pct(a.waiting_mem()), pal.secondary),
        (t(l, "At barriers", "等同步"), pct(a.waiting_sync()), pal.tertiary),
        (t(l, "Occupancy", "占用率"), pct(a.occupancy()), pal.primary),
        (
            t(l, "Warps one SM can hold", "每个 SM 能容纳的 warp"),
            format!("{} / {SM_WARPS}", a.cap),
            if a.cap < SM_WARPS { pal.warn } else { pal.good },
        ),
    ];
    for (k, (label, value, c)) in rows.into_iter().enumerate() {
        ui.pill(Vec2::new(PILL_X, -152.0 - k as f32 * 34.0), 270.0, label, value, c);
    }
    if st.spills > 0 {
        ui.text(
            format!("{} {}", st.spills, t(l, "lane accesses spilled to local memory", "次车道访问溢出到局部内存")),
            Vec2::new(PILL_X - 135.0, -322.0),
            12.0,
            pal.bad,
            Anchor::CENTER_LEFT,
        );
    }
}

/// Log-log roofline: lane operations per cycle against lane operations per
/// word fetched from memory. The slope is what memory can feed, the flat
/// top what the lanes can do.
fn draw_roofline(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let Some((_, r)) = run(v.s) else { return };
    let (at, size) = ROOF;
    ui.well(at, size);
    let sms = r.run.analysis.sms.len().max(1) as f32;
    let peak = sms * LANES as f32;
    let bw = sms * LINE as f32 / FILL_GAP as f32;
    // Axes in log2: intensity 1/16 … 256, throughput 1/4 … 2× peak.
    let (x0, x1) = (-4.0f32, 8.0f32);
    let (y0, y1) = (-2.0f32, peak.log2() + 1.0);
    let map = |i: f32, p: f32| {
        let fx = (i.max(1e-6).log2().clamp(x0, x1) - x0) / (x1 - x0);
        let fy = (p.max(1e-6).log2().clamp(y0, y1) - y0) / (y1 - y0);
        at - size / 2.0 + Vec2::new(fx * size.x, fy * size.y)
    };
    let ridge = peak / bw;
    let lo = 2f32.powf(x0);
    let start = map(lo, bw * lo);
    let knee = map(ridge, peak);
    let end = map(2f32.powf(x1), peak);
    ui.line(start, knee, 2.0, pal.secondary.with_alpha(0.8), 6.0);
    ui.line(knee, end, 2.0, pal.primary.with_alpha(0.8), 6.0);
    ui.line(
        Vec2::new(knee.x, at.y - size.y / 2.0),
        knee,
        1.0,
        pal.line,
        5.8,
    );
    ui.caption(
        t(l, "MEMORY-BOUND", "受限于内存"),
        Vec2::new(at.x - size.x / 2.0 + 8.0, at.y - size.y / 2.0 + 12.0),
        pal.secondary.with_alpha(0.8),
        Anchor::CENTER_LEFT,
    );
    ui.caption(
        t(l, "COMPUTE-BOUND", "受限于计算"),
        Vec2::new(at.x + size.x / 2.0 - 8.0, at.y - size.y / 2.0 + 12.0),
        pal.primary.with_alpha(0.8),
        Anchor::CENTER_RIGHT,
    );
    let st = &r.run.stats;
    let words = st.misses as f32 * LINE as f32;
    let intensity = if words > 0.0 {
        st.lane_ops as f32 / words
    } else {
        2f32.powf(x1)
    };
    let achieved = st.lane_ops as f32 / st.cycles.max(1) as f32;
    let p = map(intensity, achieved);
    ui.glow(p, 34.0, pal.warn.with_alpha(0.45), 6.5);
    ui.dot(p, 9.0, pal.warn, 7.0);
    let roof = peak.min(bw * intensity);
    ui.text(
        format!(
            "{:.1} {} · {} {}",
            achieved,
            t(l, "lane-ops / cycle", "车道运算 / 周期"),
            pct(achieved / roof.max(1e-6)),
            t(l, "of the roof", "触顶")
        ),
        Vec2::new(at.x, at.y - size.y / 2.0 - 12.0),
        12.0,
        pal.warn,
        Anchor::CENTER,
    );
    ui.caption(
        t(l, "ROOFLINE", "屋顶线"),
        Vec2::new(at.x - size.x / 2.0, TITLE_Y),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    ui.text(
        t(l, "ops per word fetched →", "每取一个字做几次运算 →"),
        Vec2::new(at.x + size.x / 2.0, TITLE_Y),
        11.0,
        pal.text_faint,
        Anchor::CENTER_RIGHT,
    );
}
