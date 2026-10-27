//! The run view: the machine screen, the lanes, the Warp River, and the
//! systems that keep them in step with playback.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_kit::ui::{Cursor, Ui};
use ooc_core::isa::{FB_LEN, LANES};
use ooc_core::task::View as TaskView;

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
    let Some(i) = step_at_col(cx.s.play.step, col) else {
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
fn thread_of(s: &Session, cell: usize) -> Option<usize> {
    let threads = s.task().launch.threads;
    let th = match s.task().view {
        TaskView::Buffers => cell % FB_LEN,
        _ => cell,
    };
    (th < threads).then_some(th)
}

/// Clicking a cell probes its thread.
pub fn probe_input(cursor: Res<Cursor>, mut cx: Ctx) {
    if !cursor.pressed || cursor.over_button || cx.s.report.is_none() {
        return;
    }
    let Some(p) = cursor.world else { return };
    let view = cx.s.task().view;
    let cell = if view == TaskView::Lanes {
        lane_at(p)
    } else if view == TaskView::Cells {
        let d = display(view);
        fb::grid_at(p, d.out.0, d.out.1)
    } else {
        let d = display(view);
        fb::cell_at(p, d.out.0, d.out.1, d.wide)
    };
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
    let (out, target) = fbi.pair(s.task().view);
    if let Some(mut img) = images.get_mut(out) {
        fb::paint_values(&mut img, &buf);
    }
    let expect = expected(&s);
    if new_case && let Some(mut img) = images.get_mut(target) {
        fb::paint(&mut img, &expect);
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
    if let Some(mut img) = images.get_mut(&fbi.river) {
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
                    format!("warp {w} · {} {} · {ins}", t(l, "line", "行"), pc + 1)
                }
                _ => String::new(),
            },
        };
    }
    for (mut tf, mut vis) in &mut bar {
        match latest.and_then(|(_, pc, _)| code_ed::line_y(&s, pc)) {
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
fn paint_river(
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
    let (img, timg) = v.fb.pair(view);
    screen_frame(ui, d.out.0, d.out.1, img.clone(), pal.line_bright);
    screen_frame(ui, d.target.0, d.target.1, timg.clone(), pal.line);
    let out_top = d.out.0.y + d.out.1.y / 2.0;
    let label = if d.wide {
        t(l, "OUTPUT · FRONT | BACK", "输出 · 前缓冲 | 后缓冲")
    } else if view == TaskView::Cells {
        t(l, "OUTPUT · 32 CELLS PER ROW", "输出 · 每行 32 格")
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
    ui.caption(
        t(l, "TARGET", "目标"),
        tl,
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let place = |c: usize| {
        if view == TaskView::Cells {
            (
                fb::grid_pos(c, d.out.0, d.out.1),
                d.out.1.x / fb::GRID_W as f32,
            )
        } else {
            (
                fb::cell_pos(c, d.out.0, d.out.1, d.wide),
                d.out.1.x / if d.wide { 33.0 } else { 16.0 },
            )
        }
    };
    if s.play.revealed {
        for &c in wrong_cells(s).iter().take(96) {
            let (p, pitch) = place(c);
            ui.rect(p, Vec2::splat(pitch * 0.36), pal.bad, 7.5);
        }
    }
    if let Some(th) = s.probe {
        let (p, pitch) = place(th);
        ui.glow(p, pitch * 3.0, pal.secondary.with_alpha(0.5), 7.6);
        ui.chip(p, Vec2::splat(pitch * 0.5), pal.secondary, 7.7);
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
