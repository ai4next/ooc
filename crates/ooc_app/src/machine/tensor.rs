//! The Tensor tab: the tiles of the replay's latest MMA, one half-warp's
//! A × B + C = D, with each cell shaded by its value and FP16 overflow in red.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_kit::ui::Ui;
use ooc_core::grade::{MmaTile, mma_tile};
use ooc_core::isa::Instr;

use super::{TITLE_Y, X0, empty, run};
use crate::View;
use crate::state::{Act, t};

const CELL: f32 = 34.0;
const TILE_Y: f32 = -242.0;
/// Centres of the four tiles and the operators between them.
const TILES: [f32; 4] = [-540.0, -350.0, -160.0, 70.0];
const PILL_X: f32 = 470.0;
/// FP16's largest finite value.
const H_MAX: f32 = 65504.0;

/// The newest MMA at or before the replay's latest step.
fn latest_mma(v: &View) -> Option<(usize, usize)> {
    let (rep, r) = run(v.s)?;
    let end = v.s.play.step.min(r.run.trace.len());
    let mut count = 0;
    let mut last = None;
    for (i, st) in r.run.trace.iter().enumerate() {
        if matches!(rep.program.code.get(st.pc as usize), Some(Instr::Mma { .. })) {
            if i < end {
                last = Some(i);
            }
            count += 1;
        }
    }
    last.map(|i| (i, count))
}

pub fn draw(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let title = t(l, "TENSOR CORE · ONE MMA", "张量核 · 一条 MMA");
    let help = t(
        l,
        "MMA multiplies two 4×4 tiles in one instruction: each half-warp holds a tile, one value per lane (lane = row × 4 + column). A and B are FP16; the sum C + A × B is kept in FP32. Run a kernel that uses MMA, then step the replay to one.",
        "MMA 一条指令乘完两块 4×4：每个半 warp 持有一块，每条车道一格（车道 = 行 × 4 + 列）。A 和 B 是 FP16，C + A × B 用 FP32 累加。运行一个用到 MMA 的内核，再把回放走到它。",
    );
    let Some((rep, r)) = run(s) else {
        empty(ui, title, help);
        return;
    };
    let Some((step, count)) = latest_mma(v) else {
        empty(ui, title, help);
        draw_flags(ui, v);
        return;
    };
    let Some(tile) = mma_tile(s.task(), &rep.program, r, step) else {
        empty(ui, title, help);
        return;
    };
    ui.caption(
        format!(
            "{title} · {} {} · warp {} · {} {}",
            t(l, "STEP", "步"),
            step + 1,
            tile.warp,
            t(l, "LANES", "车道"),
            if tile.half == 0 { "0–15" } else { "16–31" }
        ),
        Vec2::new(X0, TITLE_Y),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let names = ["A · FP16", "B · FP16", "C · FP32", "D · FP32"];
    let tiles = [&tile.a, &tile.b, &tile.c, &tile.d];
    let span = tiles
        .iter()
        .flat_map(|t| t.iter())
        .filter(|x| x.is_finite())
        .fold(1e-6f32, |m, x| m.max(x.abs()));
    for (k, (name, vals)) in names.iter().zip(tiles).enumerate() {
        draw_tile(ui, TILES[k], name, vals, span, &tile);
    }
    for (k, op) in ["×", "+", "="].into_iter().enumerate() {
        let x = if k < 2 {
            (TILES[k] + TILES[k + 1]) / 2.0
        } else {
            (TILES[2] + TILES[3]) / 2.0
        };
        ui.text(op, Vec2::new(x, TILE_Y), 26.0, pal.text_dim, Anchor::CENTER);
    }
    if count > 1 {
        ui.button(
            oo_kit::ui::ButtonSpec::new(
                Vec2::new(240.0, TILE_Y),
                Vec2::new(150.0, 26.0),
                t(l, "Next MMA ›", "下一条 MMA ›"),
                Act::Seek(next_mma(v, step) + 1),
            )
            .font(11.0)
            .tone(oo_kit::ui::Tone::Ghost),
        );
    }
    draw_flags(ui, v);
}

/// The trace index of the next MMA after `step`, or `step` if none.
fn next_mma(v: &View, step: usize) -> usize {
    run(v.s)
        .and_then(|(rep, r)| {
            r.run
                .trace
                .iter()
                .enumerate()
                .skip(step + 1)
                .find(|(_, st)| {
                    matches!(rep.program.code.get(st.pc as usize), Some(Instr::Mma { .. }))
                })
                .map(|(i, _)| i)
        })
        .unwrap_or(step)
}

/// Short form of a tile value.
fn short(x: f32) -> String {
    if !x.is_finite() {
        return if x.is_nan() { "NaN".into() } else { "inf".into() };
    }
    let a = x.abs();
    if a >= 10000.0 {
        format!("{:.0}k", x / 1000.0)
    } else if a >= 100.0 || x.fract() == 0.0 {
        format!("{x:.0}")
    } else if a >= 10.0 {
        format!("{x:.1}")
    } else {
        format!("{x:.2}")
    }
}

fn draw_tile(
    ui: &mut Ui,
    cx: f32,
    name: &str,
    vals: &[f32; 16],
    span: f32,
    tile: &MmaTile,
) {
    let pal = ui.pal;
    let size = Vec2::splat(CELL * 4.0 + 10.0);
    ui.well(Vec2::new(cx, TILE_Y), size);
    ui.caption(
        name,
        Vec2::new(cx, TILE_Y + size.y / 2.0 + 10.0),
        pal.text_faint,
        Anchor::CENTER,
    );
    let fp16 = name.contains("16");
    for (i, &x) in vals.iter().enumerate() {
        let (r, c) = ((i / 4) as f32, (i % 4) as f32);
        let p = Vec2::new(cx + (c - 1.5) * CELL, TILE_Y + (1.5 - r) * CELL);
        let live = tile.live & (1 << i) != 0;
        let bad = !x.is_finite() || (fp16 && x.abs() > H_MAX);
        let f = (x.abs() / span).clamp(0.0, 1.0).sqrt();
        let base = if bad {
            pal.bad
        } else if x < 0.0 {
            pal.tertiary
        } else {
            pal.primary
        };
        let fill = if live {
            base.with_alpha(if bad { 0.9 } else { 0.12 + 0.7 * f })
        } else {
            pal.line.with_alpha(0.15)
        };
        ui.chip(p, Vec2::splat(CELL - 3.0), fill, 7.0);
        if live {
            let ink = if f > 0.6 || bad { pal.bg } else { pal.text };
            ui.text(short(x), p, 10.0, ink, Anchor::CENTER);
        }
    }
}

/// Lyra's failure counters for the whole run, on the right.
fn draw_flags(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let Some((rep, r)) = run(v.s) else { return };
    let tr = &r.run.trace;
    let mmas = tr
        .iter()
        .filter(|st| matches!(rep.program.code.get(st.pc as usize), Some(Instr::Mma { .. })))
        .count();
    let inf = tr.iter().filter(|st| st.inf).count();
    let sat = tr.iter().filter(|st| st.sat).count();
    let reset = tr.iter().filter(|st| st.reset).count();
    let warn = |n: usize| if n > 0 { pal.bad } else { pal.good };
    let rows: [(&str, String, Color); 5] = [
        (t(l, "MMAs issued", "发出的 MMA"), mmas.to_string(), pal.primary),
        (t(l, "Steps with inf / NaN", "出现 inf / NaN 的步"), inf.to_string(), warn(inf)),
        (t(l, "INT8 clamps", "INT8 截断"), sat.to_string(), warn(sat)),
        (t(l, "Accumulators zeroed", "累加器被清零"), reset.to_string(), warn(reset)),
        (
            t(l, "Largest error", "最大误差"),
            format!("{} / {}", r.err().min(999_999), r.tolerance),
            if r.err() > r.tolerance { pal.bad } else { pal.good },
        ),
    ];
    for (k, (label, value, c)) in rows.into_iter().enumerate() {
        ui.pill(Vec2::new(PILL_X, -152.0 - k as f32 * 34.0), 270.0, label, value, c);
    }
}
