//! The run view: the machine screen, the lanes, the Warp River, and the
//! systems that keep them in step with playback.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_kit::ui::{Cursor, Ui};
use ooc_core::isa::{FB_LEN, LANES};
use ooc_core::task::View as TaskView;
use ooc_core::vm::Step;

use crate::fb::{self, FbImages, RIVER_COLS};
use crate::state::{Screen, Session, t};
use crate::{Ctx, View, code_ed};

/// Where the output and target screens sit for each task view.
pub struct Display {
    pub out: (Vec2, Vec2),
    pub target: (Vec2, Vec2),
    pub wide: bool,
    /// Left end of the row of case buttons.
    pub cases: Vec2,
}

pub fn display(view: TaskView) -> Display {
    match view {
        TaskView::Screen => Display {
            out: (Vec2::new(344.0, 166.0), Vec2::splat(224.0)),
            target: (Vec2::new(556.0, 220.0), Vec2::splat(104.0)),
            wide: false,
            cases: Vec2::new(236.0, 26.0),
        },
        TaskView::Buffers => Display {
            out: (Vec2::new(420.0, 190.0), Vec2::new(380.0, 184.0)),
            target: (Vec2::new(292.0, 52.0), Vec2::new(120.0, 58.0)),
            wide: true,
            cases: Vec2::new(372.0, 52.0),
        },
        TaskView::Cells => Display {
            out: (Vec2::new(344.0, 166.0), Vec2::splat(224.0)),
            target: (Vec2::new(556.0, 220.0), Vec2::splat(104.0)),
            wide: false,
            cases: Vec2::new(236.0, 26.0),
        },
        TaskView::Lanes => Display {
            out: (Vec2::ZERO, Vec2::ZERO),
            target: (Vec2::ZERO, Vec2::ZERO),
            wide: false,
            cases: Vec2::new(236.0, 26.0),
        },
    }
}

/// The Warp River image on the bench strip (centre, size).
pub const RIVER: (Vec2, Vec2) = (Vec2::new(-352.0, -222.0), Vec2::new(480.0, 160.0));

/// The trace step shown in river column `col` (0 = left edge), if any.
fn step_at_col(step: usize, col: usize) -> Option<usize> {
    (step + col).checked_sub(RIVER_COLS as usize)
}

/// Clicking a Warp River column rewinds the replay so that step is the latest.
pub fn river_input(cursor: Res<Cursor>, mut cx: Ctx) {
    if !cursor.pressed || cursor.over_button || cx.s.report.is_none() {
        return;
    }
    if cx.s.tab != crate::state::Tab::River {
        return;
    }
    let Some(p) = cursor.world else { return };
    let (c, size) = RIVER;
    let d = p - (c - size / 2.0);
    if d.x < 0.0 || d.y < 0.0 || d.x >= size.x || d.y >= size.y {
        return;
    }
    let col = (d.x / size.x * RIVER_COLS as f32) as usize;
    let at = match crate::seals::click(&mut cx, col) {
        Some(folded) => folded,
        None => step_at_col(cx.s.play.step, col),
    };
    let Some(i) = at else {
        return;
    };
    let len = crate::bench::trace_len(&cx.s);
    if i + 1 == cx.s.play.step || i >= len {
        return;
    }
    cx.s.play.playing = false;
    crate::bench::seek(&mut cx.s, i + 1);
}

/// Lane grid: 8 columns × 4 rows of numbered cells.
pub const LANE_CELL: Vec2 = Vec2::new(44.0, 46.0);

pub fn lane_pos(lane: usize) -> Vec2 {
    let (c, r) = ((lane % 8) as f32, (lane / 8) as f32);
    Vec2::new(257.0 + c * 48.0, 252.0 - r * 52.0)
}

pub fn lane_at(p: Vec2) -> Option<usize> {
    (0..LANES).find(|&l| (p - lane_pos(l)).abs().cmple(LANE_CELL / 2.0).all())
}

/// Live value of one lane cell.
#[derive(Component)]
pub struct LaneText(pub usize);

/// Live playback readouts in the River tab: 0 = step counter, 1 = current line.
#[derive(Component)]
pub struct RiverStat(pub u8);

/// Bar behind the code line the latest step issued.
#[derive(Component)]
pub struct PcBar;

/// The thread a cell of the output belongs to, if the task launched it.
pub fn thread_of(s: &Session, cell: usize) -> Option<usize> {
    let threads = s.task().launch.threads;
    let th = match s.task().view {
        TaskView::Buffers => cell % FB_LEN,
        _ => cell,
    };
    (th < threads).then_some(th)
}

/// The output cell under `p`, if any.
fn cell_under(s: &Session, p: Vec2) -> Option<usize> {
    let view = s.task().view;
    let d = display(view);
    if view == TaskView::Lanes {
        lane_at(p)
    } else if view == TaskView::Cells {
        fb::grid_at(p, fb::grid_side(s.task()), d.out.0, d.out.1)
    } else {
        fb::cell_at(p, fb::screen_w(s.task()), d.out.0, d.out.1, d.wide)
    }
}

/// The step of the selected case's replay, up to the current one, that last
/// wrote `cell`: its index and the step.
pub fn last_write(s: &Session, cell: usize) -> Option<(usize, &Step)> {
    let rep = s.report.as_ref()?;
    let trace = &rep.runs.get(s.case)?.run.trace;
    let upto = s.play.step.min(trace.len());
    trace[..upto]
        .iter()
        .enumerate()
        .rev()
        .find(|(_, st)| st.writes.iter().any(|&(a, _)| a as usize == cell))
}

/// Hovering a cell names its thread and the step that wrote it; clicking
/// pins the probe, whose values the editor shows.
pub fn probe_input(cursor: Res<Cursor>, mut pinned: Local<Option<bool>>, mut cx: Ctx) {
    if cx.s.report.is_none() {
        return;
    }
    let cell = cursor.world.and_then(|p| cell_under(&cx.s, p));
    // Screenshots set the hover by hand (`OOC_HOVER`); the cursor leaves it.
    let pinned = *pinned.get_or_insert_with(|| std::env::var_os("OOC_HOVER").is_some());
    if !pinned && cell != cx.s.hover {
        cx.s.hover = cell;
        cx.dirty();
    }
    if !cursor.pressed || cursor.over_button {
        return;
    }
    if let Some(th) = cell.and_then(|c| thread_of(&cx.s, c)) {
        cx.s.probe = if cx.s.probe == Some(th) {
            None
        } else {
            Some(th)
        };
        cx.dirty();
    }
}

/// The expectation of the selected case.
pub fn expected(s: &Session) -> Vec<Option<i32>> {
    match &s.report {
        Some(r) => r.runs[s.case.min(r.runs.len() - 1)].expect.clone(),
        None => s
            .cases
            .get(s.case)
            .map_or_else(Vec::new, |c| (s.task().expect)(c)),
    }
}

/// What the output shows: the replay buffer, or the initial buffer before a run.
pub fn shown(s: &Session) -> Vec<i32> {
    if s.report.is_some() {
        return s.play.buf.clone();
    }
    let mut b = s
        .cases
        .get(s.case)
        .map_or_else(Vec::new, |c| (s.task().init)(c));
    b.resize(s.task().out_len, 0);
    b
}

/// Repaints the screens and lane cells when playback moves.
pub fn sync_output(
    s: Res<Session>,
    screen: Res<State<Screen>>,
    fbi: Res<FbImages>,
    mut images: ResMut<Assets<Image>>,
    mut lanes: Query<(&LaneText, &mut Text2d, &mut TextColor)>,
    mut last: Local<Option<(u64, usize, usize)>>,
    pal: Res<oo_kit::theme::Palette>,
) {
    if !matches!(*screen.get(), Screen::Bench | Screen::Results) {
        *last = None;
        return;
    }
    let key = (s.generation, s.play.step, lanes.iter().count());
    if *last == Some(key) {
        return;
    }
    let new_case = last.is_none_or(|l| l.0 != key.0);
    *last = Some(key);
    let buf = shown(&s);
    let (out, target) = fbi.pair(s.task());
    let expect = expected(&s);
    let shade = shade_for(&s, &expect);
    let amber = fb::era(s.task()) == fb::Era::Amber && s.task().view != TaskView::Cells;
    if let Some(mut img) = images.get_mut(out) {
        fb::paint_values(&mut img, &buf, shade);
        if amber {
            fb::amber_tint(&mut img, 0.3);
        }
    }
    if new_case && let Some(mut img) = images.get_mut(target) {
        fb::paint_shaded(&mut img, &expect, shade);
        if amber {
            fb::amber_tint(&mut img, 0.3);
        }
    }
    for (lane, mut text, mut color) in &mut lanes {
        let v = buf.get(lane.0).copied().unwrap_or(0);
        text.0 = v.to_string();
        let done = s.play.revealed;
        let want = expect.get(lane.0).copied().flatten();
        color.0 = match want {
            Some(w) if done && w.abs_diff(v) > s.task().tolerance as u32 => pal.bad,
            Some(_) if done => pal.good,
            _ => pal.text,
        };
    }
}

/// Data grids are shaded by the expectation's range so output and target agree.
pub fn shade_for(s: &Session, expect: &[Option<i32>]) -> fb::Shade {
    if s.task().view != TaskView::Cells {
        return fb::Shade::Palette;
    }
    fb::Shade::of(expect.iter().flatten())
}

/// Paints the Warp River and moves the live readouts.
pub fn sync_markers(
    s: Res<Session>,
    screen: Res<State<Screen>>,
    fbi: Res<FbImages>,
    game: Res<crate::state::Game>,
    mut images: ResMut<Assets<Image>>,
    mut stats: Query<(&RiverStat, &mut Text2d)>,
    mut bar: Query<(&mut Transform, &mut Visibility), With<PcBar>>,
    mut last: Local<Option<(u64, usize, usize)>>,
) {
    if *screen.get() != Screen::Bench {
        *last = None;
        return;
    }
    let key = (
        s.generation,
        s.play.step,
        stats.iter().count() + bar.iter().count(),
    );
    if *last == Some(key) {
        return;
    }
    *last = Some(key);
    let l = game.lang();
    let run = s
        .report
        .as_ref()
        .map(|r| (r, &r.runs[s.case.min(r.runs.len() - 1)].run));
    // Fault levels: once pointed at (or found), the evidence column glows.
    let mark = (s.pointed.is_some() || game.save.has_evidence(s.task()))
        .then(|| crate::bench::clue_at(&s))
        .flatten()
        .map(|e| e.center() as usize);
    if let Some(mut img) = images.get_mut(&fbi.river)
        && !crate::seals::paint(&mut img, &s, mark)
    {
        paint_river(&mut img, run.map(|(_, r)| &r.trace[..]), s.play.step, mark);
    }
    let latest = run.and_then(|(rep, r)| {
        let i = s.play.step.checked_sub(1)?;
        let st = r.trace.get(i)?;
        Some((
            st.warp,
            st.pc as usize,
            rep.program.code.get(st.pc as usize).copied(),
        ))
    });
    for (stat, mut text) in &mut stats {
        text.0 = match (stat.0, run) {
            (0, Some((_, r))) => {
                format!("{} {} / {}", t(l, "Step", "步"), s.play.step, r.trace.len())
            }
            (0, None) => t(l, "Not run yet", "尚未运行").into(),
            (_, _) => match latest {
                Some((w, pc, Some(ins))) => {
                    let spans = run.map_or(&[][..], |(rep, _)| &rep.program.seals[..]);
                    let at = crate::seals::line_text(spans, pc);
                    format!("warp {w} · {} {at} · {ins}", t(l, "line", "行"))
                }
                _ => String::new(),
            },
        };
    }
    for (mut tf, mut vis) in &mut bar {
        let line = |pc| run.map_or(pc, |(rep, _)| ooc_core::seal::line_of(&rep.program.seals, pc));
        match latest.and_then(|(_, pc, _)| code_ed::line_y(&s, line(pc))) {
            Some(y) => {
                tf.translation.y = y;
                *vis = Visibility::Inherited;
            }
            None => *vis = Visibility::Hidden,
        }
    }
}

pub const RIVER_FULL: [u8; 4] = [53, 230, 181, 255];
const RIVER_FULL_ODD: [u8; 4] = [34, 170, 135, 255];
pub const RIVER_SPLIT: [u8; 4] = [255, 154, 60, 255];
const RIVER_SPLIT_ODD: [u8; 4] = [214, 122, 40, 255];
pub const RIVER_IDLE: [u8; 4] = [22, 28, 44, 255];
pub const RIVER_STALE: [u8; 4] = [255, 77, 109, 255];
const RIVER_EMPTY: [u8; 4] = [7, 10, 18, 255];
const RIVER_MARK: [u8; 4] = [255, 107, 138, 255];

pub fn river_colour(c: [u8; 4]) -> Color {
    Color::srgb_u8(c[0], c[1], c[2])
}

/// One column per issued instruction (latest at the right), one row per lane.
/// Full warps glow teal; divergent ones show their active lanes in amber.
pub(crate) fn paint_river(
    img: &mut Image,
    trace: Option<&[ooc_core::vm::Step]>,
    step: usize,
    mark: Option<usize>,
) {
    let w = RIVER_COLS as usize;
    let Some(data) = img.data.as_mut() else {
        return;
    };
    let trace = trace.unwrap_or(&[]);
    // Right-aligned: the newest step is always the last column, so the river
    // flows in from the right as playback runs.
    for c in 0..w {
        let i = step_at_col(step, c);
        let st = i.and_then(|i| trace.get(i));
        let marked = i.is_some() && i == mark;
        for lane in 0..LANES {
            let px = match st {
                Some(_) if marked && lane % 4 != 1 => RIVER_MARK,
                None => RIVER_EMPTY,
                // Prism: a read that beat its result to the register.
                Some(st) if st.stale && st.mask & (1 << lane) != 0 => RIVER_STALE,
                Some(st) => {
                    let on = st.mask & (1 << lane) != 0;
                    let full = st.mask == u32::MAX;
                    let odd = st.warp % 2 == 1;
                    match (on, full, odd) {
                        (false, _, _) => RIVER_IDLE,
                        (true, true, false) => RIVER_FULL,
                        (true, true, true) => RIVER_FULL_ODD,
                        (true, false, false) => RIVER_SPLIT,
                        (true, false, true) => RIVER_SPLIT_ODD,
                    }
                }
            };
            let i = (lane * w + c) * 4;
            if i + 4 <= data.len() {
                data[i..i + 4].copy_from_slice(&px);
            }
        }
    }
}

/// Draws the output and target screens (or lane cells) into the run card.
pub fn draw_display(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let view = s.task().view;
    let expect = expected(s);
    if view == TaskView::Lanes {
        ui.caption(
            t(
                l,
                "LANES · INPUT rom[lane] · OUTPUT · EXPECTED",
                "车道 · 输入 rom[lane] · 输出 · 期望",
            ),
            Vec2::new(235.0, 286.0),
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
        let rom = s
            .cases
            .get(s.case)
            .map(|c| c.rom.clone())
            .unwrap_or_default();
        let wrong: Vec<usize> = wrong_cells(s);
        for lane in 0..LANES {
            let p = lane_pos(lane);
            let bad = wrong.contains(&lane);
            let probed = s.probe == Some(lane);
            let edge = if bad {
                pal.bad
            } else if probed {
                pal.secondary
            } else {
                pal.line
            };
            ui.chip(p, LANE_CELL + Vec2::splat(2.0), edge, 6.0);
            ui.chip(p, LANE_CELL, pal.bg_deep, 6.2);
            ui.text(
                lane.to_string(),
                p + Vec2::new(-19.0, 16.0),
                9.0,
                pal.text_faint,
                Anchor::CENTER_LEFT,
            );
            if let Some(r) = rom.get(lane) {
                ui.text(
                    r.to_string(),
                    p + Vec2::new(19.0, 16.0),
                    9.0,
                    pal.tertiary,
                    Anchor::CENTER_RIGHT,
                );
            }
            let e = ui.text("0", p + Vec2::new(0.0, 1.0), 16.0, pal.text, Anchor::CENTER);
            ui.commands.entity(e).insert(LaneText(lane));
            if let Some(Some(want)) = expect.get(lane) {
                ui.text(
                    want.to_string(),
                    p + Vec2::new(0.0, -15.0),
                    9.0,
                    pal.text_dim,
                    Anchor::CENTER,
                );
            }
        }
        return;
    }
    let d = display(view);
    let (img, timg) = v.fb.pair(s.task());
    let side = fb::grid_side(s.task());
    screen_frame(ui, d.out.0, d.out.1, img.clone(), pal.line_bright);
    let crt = view != TaskView::Cells;
    if crt {
        fb::crt(ui, v.fb, d.out.0, d.out.1, fb::era(s.task()));
    }
    // The free bench has no target to show.
    if !s.lab {
        screen_frame(ui, d.target.0, d.target.1, timg.clone(), pal.line);
        if crt {
            fb::crt(ui, v.fb, d.target.0, d.target.1, fb::era(s.task()));
        }
    } else {
        crate::lab::panel::draw_preview(ui, v, d.target.0, d.target.1);
    }
    let out_top = d.out.0.y + d.out.1.y / 2.0;
    let label = if d.wide {
        t(l, "OUTPUT · FRONT | BACK", "输出 · 前缓冲 | 后缓冲")
    } else if view == TaskView::Cells {
        match side {
            32 => t(l, "OUTPUT · 32 CELLS PER ROW", "输出 · 每行 32 格"),
            64 => t(l, "OUTPUT · 64 CELLS PER ROW", "输出 · 每行 64 格"),
            _ => t(l, "OUTPUT · 128 CELLS PER ROW", "输出 · 每行 128 格"),
        }
    } else {
        t(l, "OUTPUT", "输出")
    };
    ui.caption(
        label,
        Vec2::new(d.out.0.x - d.out.1.x / 2.0, out_top + 10.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let tl = Vec2::new(
        d.target.0.x - d.target.1.x / 2.0,
        d.target.0.y + d.target.1.y / 2.0 + 10.0,
    );
    if !s.lab {
        ui.caption(
            t(l, "TARGET", "目标"),
            tl,
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
    }
    let place = |c: usize| {
        if view == TaskView::Cells {
            (
                fb::grid_pos(c, side, d.out.0, d.out.1),
                d.out.1.x / side as f32,
            )
        } else {
            let w = fb::screen_w(s.task());
            (
                fb::cell_pos(c, w, d.out.0, d.out.1, d.wide),
                d.out.1.x / if d.wide { 33.0 } else { w as f32 },
            )
        }
    };
    if s.play.revealed {
        for &c in wrong_cells(s).iter().take(96) {
            let (p, pitch) = place(c);
            ui.rect(p, Vec2::splat((pitch * 0.36).max(2.0)), pal.bad, 7.5);
        }
    }
    if let Some(c) = s.hover.filter(|&c| Some(c) != s.probe && view != TaskView::Lanes) {
        let (p, pitch) = place(c);
        let (e, c) = (pitch.max(4.0) / 2.0 + 1.0, pal.text.with_alpha(0.7));
        for (off, size) in [
            (Vec2::new(0.0, e), Vec2::new(2.0 * e, 1.0)),
            (Vec2::new(0.0, -e), Vec2::new(2.0 * e, 1.0)),
            (Vec2::new(e, 0.0), Vec2::new(1.0, 2.0 * e)),
            (Vec2::new(-e, 0.0), Vec2::new(1.0, 2.0 * e)),
        ] {
            ui.rect(p + off, size, c, 7.65);
        }
    }
    if let Some(th) = s.probe {
        let (p, pitch) = place(th);
        ui.glow(p, (pitch * 3.0).max(12.0), pal.secondary.with_alpha(0.5), 7.6);
        ui.chip(p, Vec2::splat((pitch * 0.5).max(4.0)), pal.secondary, 7.7);
    }
}

fn screen_frame(ui: &mut Ui, at: Vec2, size: Vec2, image: Handle<Image>, edge: Color) {
    ui.chip(at, size + Vec2::splat(8.0), edge.with_alpha(0.5), 6.0);
    ui.chip(at, size + Vec2::splat(6.0), ui.pal.bg_deep, 6.2);
    ui.commands.spawn((
        Sprite {
            image,
            custom_size: Some(size),
            ..default()
        },
        Transform::from_xyz(at.x, at.y, 7.0),
        oo_kit::ui::ScreenEntity,
    ));
}

/// Wrong cells of the selected case (after a run).
pub fn wrong_cells(s: &Session) -> Vec<usize> {
    match (&s.report, s.play.revealed) {
        (Some(r), true) => r.runs[s.case.min(r.runs.len() - 1)].wrong.clone(),
        _ => Vec::new(),
    }
}
