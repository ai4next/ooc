//! The Memory tab: every cache line L1 fetched, against the cycle it came
//! in and how long it stayed, plus what the latest memory step cost.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_kit::ui::Ui;
use ooc_core::grade::CaseRun;
use ooc_core::mem::{GLOBAL_LAT, HIT_LAT};

use super::{X0, TITLE_Y, empty, latest, run, swatch};
use crate::View;
use crate::fb::FbImages;
use crate::river::{RIVER_SPLIT, RIVER_STALE, river_colour};
use crate::state::{Screen, Session, Tab, t};

/// Pixels of the line × cycle image.
pub const MEM_W: u32 = 300;
pub const MEM_H: u32 = 104;
/// Where the image sits on the strip.
const PLOT: (Vec2, Vec2) = (Vec2::new(-312.0, -194.0), Vec2::new(600.0, 92.0));
const BARS: usize = 96;
const BAR_BASE: f32 = -338.0;
/// Bar height per line or pass, and the most a bar shows.
const BAR_UNIT: f32 = 5.0;
const BAR_CAP: u8 = 11;
const BAR_Y: f32 = -272.0;
const PILL_X: f32 = 470.0;

const BG: [u8; 4] = [7, 10, 18, 255];
const LIFE: [u8; 4] = [18, 62, 60, 255];
const FILL: [u8; 4] = [53, 230, 181, 255];

/// Repaints the image when the run or the case changes.
pub fn sync(
    s: Res<Session>,
    screen: Res<State<Screen>>,
    fbi: Res<FbImages>,
    mut images: ResMut<Assets<Image>>,
    mut last: Local<Option<u64>>,
) {
    if *screen.get() != Screen::Bench || s.tab != Tab::Memory {
        *last = None;
        return;
    }
    if *last == Some(s.generation) {
        return;
    }
    *last = Some(s.generation);
    if let Some(mut img) = images.get_mut(&fbi.mem) {
        paint(&mut img, run(&s).map(|(_, r)| r));
    }
}

/// One row per distinct line (lowest address on top), one column per slice
/// of the run. A line glows where it was fetched, stays dimly lit while it
/// lives in L1, and ends red if it was thrown out after one word was used.
fn paint(img: &mut Image, run: Option<&CaseRun>) {
    let (w, h) = (MEM_W as usize, MEM_H as usize);
    let Some(data) = img.data.as_mut() else {
        return;
    };
    for px in data.chunks_exact_mut(4) {
        px.copy_from_slice(&BG);
    }
    let Some(r) = run else { return };
    let log = &r.run.mem;
    let mut tags: Vec<u32> = log.fills.iter().map(|f| f.tag).collect();
    tags.sort_unstable();
    tags.dedup();
    if tags.is_empty() {
        return;
    }
    let n = tags.len();
    let thick = (h / n).clamp(1, 4);
    let row = |tag: u32| tags.binary_search(&tag).map_or(0, |i| i * h / n);
    let total = r.run.stats.cycles.max(1) as u64;
    let col = |c: u32| ((c as u64 * w as u64 / total) as usize).min(w - 1);
    let ends: HashMap<(u32, u32), (u32, bool)> = log
        .evictions
        .iter()
        .map(|e| ((e.tag, e.fill_step), (e.cycle, e.used <= 1)))
        .collect();
    let mut put = |x: usize, y0: usize, c: [u8; 4]| {
        for y in y0..(y0 + thick).min(h) {
            let i = (y * w + x) * 4;
            data[i..i + 4].copy_from_slice(&c);
        }
    };
    for f in &log.fills {
        let (end, _) = ends
            .get(&(f.tag, f.step))
            .copied()
            .unwrap_or((total as u32, false));
        let y = row(f.tag);
        for x in col(f.cycle)..=col(end) {
            put(x, y, LIFE);
        }
    }
    for f in &log.fills {
        let y = row(f.tag);
        let c = if f.refill { RIVER_SPLIT } else { FILL };
        put(col(f.cycle), y, c);
        if let Some(&(end, true)) = ends.get(&(f.tag, f.step)) {
            put(col(end), y, RIVER_STALE);
        }
    }
}

pub fn draw(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let title = t(l, "MEMORY · CACHE LINES × CYCLES", "内存 · 缓存行 × 周期");
    let Some((_, r)) = run(s) else {
        empty(
            ui,
            title,
            t(
                l,
                "Run the program. Each row will be one cache line of 32 words, each column a slice of the run's cycles. A bright mark is the cycle L1 fetched the line from memory (400 cycles away); the dim trail is how long it stayed before something pushed it out.",
                "运行程序。每一行是一条 32 字的缓存行，每一列是一段周期。亮点是 L1 从内存取回这一行的那一拍（要等 400 周期）；暗色的尾巴是它在缓存里待了多久，直到被别的行挤出去。",
            ),
        );
        return;
    };
    let lines = {
        let mut tags: Vec<u32> = r.run.mem.fills.iter().map(|f| f.tag).collect();
        tags.sort_unstable();
        tags.dedup();
        tags.len()
    };
    ui.caption(
        format!(
            "{title} · {lines} {}",
            t(l, "LINES FETCHED", "条缓存行被取回")
        ),
        Vec2::new(X0, TITLE_Y),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let (at, size) = PLOT;
    ui.chip(at, size + Vec2::splat(6.0), pal.line.with_alpha(0.6), 6.0);
    ui.commands.spawn((
        Sprite {
            image: v.fb.mem.clone(),
            custom_size: Some(size),
            ..default()
        },
        Transform::from_xyz(at.x, at.y, 7.0),
        oo_kit::ui::ScreenEntity,
    ));
    let total = r.run.stats.cycles.max(1);
    // The replay's cycle, across the whole run.
    if let Some((_, st)) = latest(s) {
        let x = at.x - size.x / 2.0 + st.cycle.min(total) as f32 / total as f32 * size.x;
        ui.rect(
            Vec2::new(x, at.y),
            Vec2::new(2.0, size.y + 8.0),
            pal.secondary.with_alpha(0.8),
            8.0,
        );
    }
    let y = at.y - size.y / 2.0 - 13.0;
    ui.caption("0", Vec2::new(X0, y), pal.text_faint, Anchor::CENTER_LEFT);
    ui.caption(
        format!("{total} {}", t(l, "CYCLES", "周期")),
        Vec2::new(X0 + size.x, y),
        pal.text_faint,
        Anchor::CENTER_RIGHT,
    );
    let mut x = X0 + 40.0;
    for (c, label) in [
        (river_colour(FILL), t(l, "fetched", "取回")),
        (river_colour(LIFE), t(l, "in L1", "在 L1 中")),
        (river_colour(RIVER_SPLIT), t(l, "fetched again", "再次取回")),
        (river_colour(RIVER_STALE), t(l, "evicted, 1 word used", "被挤出，只用了 1 个字")),
    ] {
        x = swatch(ui, Vec2::new(x, y), c, label);
    }
    draw_bars(ui, v);
    draw_step(ui, v, r);
}

/// What each of the replay's last steps cost in memory, aligned with the
/// Warp River: lines a read touched (misses at the bottom), or passes a
/// shared-memory access took.
fn draw_bars(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let Some((_, r)) = run(s) else { return };
    ui.caption(
        t(
            l,
            "PER STEP · LINES A READ TOUCHED · SHARED-MEMORY PASSES",
            "每一步 · 一次读取碰到的缓存行 · 共享内存的轮数",
        ),
        Vec2::new(X0, BAR_Y),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let pitch = PLOT.1.x / BARS as f32;
    let unit = BAR_UNIT;
    ui.rect(
        Vec2::new(X0 + PLOT.1.x / 2.0, BAR_BASE - 1.0),
        Vec2::new(PLOT.1.x, 1.0),
        pal.line,
        8.0,
    );
    let end = s.play.step.min(r.run.trace.len());
    for i in end.saturating_sub(BARS)..end {
        let st = &r.run.trace[i];
        let col = BARS - (end - i);
        let x = X0 + (col as f32 + 0.5) * pitch;
        let bar = |ui: &mut Ui, from: f32, n: u8, c: Color| {
            if n == 0 {
                return;
            }
            let hgt = n.min(BAR_CAP) as f32 * unit;
            ui.rect(
                Vec2::new(x, BAR_BASE + from + hgt / 2.0),
                Vec2::new(pitch - 1.5, hgt),
                c,
                8.5,
            );
        };
        if st.lines > 0 {
            bar(ui, 0.0, st.misses, pal.secondary);
            let shown = st.misses.min(BAR_CAP);
            bar(
                ui,
                shown as f32 * unit,
                (st.lines - st.misses).min(BAR_CAP - shown),
                pal.primary.with_alpha(0.8),
            );
        } else if st.passes > 0 {
            let c = if st.passes > 1 {
                pal.bad
            } else {
                pal.tertiary.with_alpha(0.8)
            };
            bar(ui, 0.0, st.passes, c);
        }
    }
    let y = BAR_Y;
    let mut x = X0 + 380.0;
    // Teaching mode (free bench): each colour's cost in cycles.
    let costs = crate::lab::panel::mem_costs(s, l);
    if costs.is_some() {
        x = X0 + 330.0;
    }
    for (i, (c, label)) in [
        (pal.secondary, t(l, "miss", "缺失")),
        (pal.primary, t(l, "hit", "命中")),
        (pal.tertiary, t(l, "shared", "共享")),
        (pal.bad, t(l, "bank conflict", "bank 冲突")),
    ]
    .into_iter()
    .enumerate()
    {
        let label = costs.as_ref().map_or(label.to_string(), |c| format!("{label} {}", c[i]));
        x = swatch(ui, Vec2::new(x, y), c, &label);
    }
}

/// The latest step that touched memory, and the run's totals.
fn draw_step(ui: &mut Ui, v: &View, r: &CaseRun) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let x = 20.0;
    ui.caption(
        t(l, "LATEST MEMORY STEP", "最近一次访存"),
        Vec2::new(x, TITLE_Y),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let end = s.play.step.min(r.run.trace.len());
    let found = (end.saturating_sub(400)..end)
        .rev()
        .map(|i| (i, &r.run.trace[i]))
        .find(|(_, st)| st.lines > 0 || st.passes > 0);
    match found {
        Some((i, st)) => {
            let ins = s
                .report
                .as_ref()
                .and_then(|rep| rep.program.code.get(st.pc as usize))
                .map_or(String::new(), |ins| ins.to_string());
            ui.text(
                format!("{} {} · warp {} · {ins}", t(l, "step", "步"), i + 1, st.warp),
                Vec2::new(x, -160.0),
                14.0,
                pal.text,
                Anchor::CENTER_LEFT,
            );
            let (msg, color) = step_text(st, l, &pal);
            ui.wrapped(
                msg,
                Vec2::new(x, -178.0),
                300.0,
                13.0,
                color,
                Anchor::TOP_LEFT,
            );
        }
        None => {
            ui.wrapped(
                t(
                    l,
                    "No memory step yet. Step the replay forward, or click the Warp River.",
                    "还没有访存。让回放往前走，或者点 Warp River。",
                ),
                Vec2::new(x, -160.0),
                300.0,
                13.0,
                pal.text_dim,
                Anchor::TOP_LEFT,
            );
        }
    }
    let st = &r.run.stats;
    let log = &r.run.mem;
    let hit = 1.0 - st.misses as f32 / st.txns.max(1) as f32;
    let wasted = log.evictions.iter().filter(|e| e.used <= 1).count();
    let refills = log.fills.iter().filter(|f| f.refill).count();
    let rows: [(&str, String, Color); 6] = [
        (t(l, "Transactions", "传输次数"), st.txns.to_string(), pal.primary),
        (t(l, "L1 misses", "L1 缺失"), st.misses.to_string(), pal.secondary),
        (t(l, "L1 hit rate", "L1 命中率"), super::pct(hit), pal.primary),
        (t(l, "Lines wasted", "白取的行"), wasted.to_string(), pal.bad),
        (t(l, "Lines fetched again", "再次取回"), refills.to_string(), pal.secondary),
        (t(l, "Bank conflicts", "bank 冲突"), st.conflicts.to_string(), pal.tertiary),
    ];
    for (k, (label, value, c)) in rows.into_iter().enumerate() {
        ui.pill(Vec2::new(PILL_X, -152.0 - k as f32 * 34.0), 270.0, label, value, c);
    }
}

fn step_text(
    st: &ooc_core::vm::Step,
    l: oo_base::Lang,
    pal: &oo_kit::theme::Palette,
) -> (String, Color) {
    if st.lines == 0 {
        return if st.passes > 1 {
            (
                t(
                    l,
                    "Shared memory took {p} passes: lanes wanted different words in the same bank, so the banks served them one after another.",
                    "共享内存用了 {p} 轮：几条车道要同一个 bank 里的不同字，只能一个接一个地服务。",
                )
                .replace("{p}", &st.passes.to_string()),
                pal.bad,
            )
        } else {
            (
                t(
                    l,
                    "Shared memory in one pass: no two lanes needed the same bank.",
                    "共享内存一轮完成：没有两条车道挤同一个 bank。",
                )
                .into(),
                pal.good,
            )
        };
    }
    let lanes = st.mask.count_ones();
    let hits = st.lines - st.misses;
    let shape = match st.lines {
        1 => t(
            l,
            "Coalesced: the whole warp is one transaction.",
            "合并访问：整个 warp 只算一次传输。",
        ),
        2..=4 => t(l, "Nearly coalesced.", "接近合并。"),
        _ => t(
            l,
            "Scattered: every line is a separate trip.",
            "分散访问：每一行都要单独跑一趟。",
        ),
    };
    let msg = t(
        l,
        "{n} lanes read {k} cache lines. {shape} {m} missed L1 and waited {g} cycles; {h} hit in {c}.",
        "{n} 条车道读了 {k} 条缓存行。{shape}{m} 条没命中 L1，等了 {g} 周期；{h} 条命中，只要 {c}。",
    )
    .replace("{n}", &lanes.to_string())
    .replace("{k}", &st.lines.to_string())
    .replace("{shape}", shape)
    .replace("{m}", &st.misses.to_string())
    .replace("{g}", &GLOBAL_LAT.to_string())
    .replace("{h}", &hits.to_string())
    .replace("{c}", &HIT_LAT.to_string());
    let color = if st.lines > 4 || st.misses > 0 {
        pal.warn
    } else {
        pal.good
    };
    (msg, color)
}
