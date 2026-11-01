//! The Showcase: a solved commission's kernel driving the machine in town.
//! Townsfolk swap the input, the player turns the kernel's constants, and a
//! corner readout shows the frame time and a Warp River thumbnail. A kernel
//! over the frame budget drops frames; that is the feedback, not a failure.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_kit::ui::{ButtonSpec, Cursor, Tone, Ui};
use ooc_core::edit::{Operand, operands, set_operand};
use ooc_core::grade::build_with;
use ooc_core::isa::{Instr, LANES, Program, Src};
use ooc_core::task::{Case, Metric, Task};
use ooc_core::vm::{Step, run_frame};

use crate::fb::{self, FbImages};
use crate::river::{RIVER_FULL, RIVER_SPLIT, RIVER_STALE};
use crate::state::{Act, Session, t};
use crate::{Ctx, View};

/// Frames per second of the showcase clock.
const FPS: f32 = 8.0;
/// The town's display refresh the frame budget is measured against.
const REFRESH_MS: f32 = 1000.0 / 60.0;
/// Columns of the corner Warp River.
pub const THUMB_W: u32 = 96;
/// Where the showcase screen sits.
const SHOW: (Vec2, Vec2) = (Vec2::new(-150.0, 10.0), Vec2::splat(520.0));
/// Constants the player can turn, at most.
const KNOBS: usize = 5;

/// One constant of the kernel the player can turn.
#[derive(Clone, Debug)]
pub struct Knob {
    pub pc: usize,
    pub slot: usize,
    pub base: i32,
    pub value: i32,
}

/// Showcase state beyond the clock and the hero's point.
#[derive(Clone, Debug, Default)]
pub struct Show {
    /// 0: the live scene; `n`: the task's n-th test case (public, then hidden).
    pub input: usize,
    pub knobs: Vec<Knob>,
    /// Bumped when a knob turns, so the kernel is rebuilt.
    pub rev: u32,
    /// The kernel as solved, before any knob turned.
    pub base: Option<Program>,
}

/// Live readout under the showcase title.
#[derive(Component)]
pub struct ShowStat;

/// Frame time in the screen's corner.
#[derive(Component)]
pub struct ShowHud;

pub fn act(cx: &mut Ctx, a: Act) {
    let show = &mut cx.s.show;
    match a {
        Act::ShowInput(i) => show.input = i,
        Act::Knob(k, d) => {
            if let Some(n) = show.knobs.get_mut(k) {
                n.value = n.value.saturating_add(d);
                show.rev += 1;
            }
        }
        Act::KnobReset => {
            for n in &mut show.knobs {
                n.value = n.base;
            }
            show.rev += 1;
        }
        _ => return,
    }
    cx.dirty();
}

/// Rewinds the showcase clock, puts the hero back at its start and finds the
/// kernel's constants.
pub fn start(s: &mut Session, seals: &ooc_core::seal::Seals) {
    s.clock = 0.0;
    s.point = [4, 4];
    let base = program(s, seals);
    s.show = Show {
        knobs: base.as_ref().map(knobs).unwrap_or_default(),
        base,
        ..default()
    };
}

/// Whether the showcase case depends on the player-moved point.
pub fn uses_point(task: &Task) -> bool {
    task.live.is_some_and(|f| f(0, [0, 0]) != f(0, [5, 5]))
}

/// The kernel the showcase runs: the graded one, or the work rebuilt.
fn program(s: &Session, seals: &ooc_core::seal::Seals) -> Option<Program> {
    match &s.report {
        Some(r) => Some(r.program.clone()),
        None => build_with(s.task(), s.work.as_ref()?, seals)
            .ok()
            .map(|b| b.program),
    }
}

/// Immediates worth turning: arithmetic constants and loop counts, not
/// addresses (an address knob only scrambles the picture).
pub fn knobs(p: &Program) -> Vec<Knob> {
    let mut out = Vec::new();
    for (pc, ins) in p.code.iter().enumerate() {
        let tunable = matches!(
            ins,
            Instr::Alu { .. } | Instr::SetP { .. } | Instr::Sel { .. } | Instr::Mov { .. } | Instr::Loop { .. }
        );
        if !tunable {
            continue;
        }
        for (slot, o) in operands(ins).into_iter().enumerate() {
            let v = match o {
                Operand::Src(Src::Imm(v)) | Operand::Count(v) => v,
                _ => continue,
            };
            out.push(Knob {
                pc,
                slot,
                base: v,
                value: v,
            });
        }
    }
    // Zeros and ones are usually identities; prefer the telling constants.
    out.sort_by_key(|k| (k.base == 0 || k.base == 1, k.pc));
    out.truncate(KNOBS);
    out.sort_by_key(|k| (k.pc, k.slot));
    out
}

/// `p` with each knob's value written back.
pub fn tuned(p: &Program, knobs: &[Knob]) -> Program {
    let mut p = p.clone();
    for k in knobs {
        let Some(ins) = p.code.get_mut(k.pc) else {
            continue;
        };
        let v = match operands(ins).get(k.slot) {
            Some(Operand::Count(_)) => Operand::Count(k.value.clamp(0, 256)),
            _ => Operand::Src(Src::Imm(k.value)),
        };
        set_operand(ins, k.slot, v);
    }
    p
}

/// Cycles one frame may take at the town's 60 Hz: the level's hard cycle
/// limit, or a quarter over its cycle goal. `None`: the frame is never late.
pub fn budget(task: &Task) -> Option<u32> {
    let cycles = |g: &ooc_core::task::Goal| (g.metric == Metric::Cycles).then_some(g.max);
    task.limit
        .as_ref()
        .and_then(cycles)
        .or_else(|| task.goals.iter().find_map(cycles).map(|c| c + c / 4))
}

/// Refreshes one frame spans: 1 on budget, 2 at up to twice over, and so on.
pub fn span(cycles: u32, budget: Option<u32>) -> u32 {
    budget.map_or(1, |b| cycles.div_ceil(b.max(1)).max(1))
}

/// The case the showcase feeds the kernel.
fn input(task: &Task, s: &Session, frame: i32) -> Option<Case> {
    match s.show.input {
        0 => task.live.map(|f| f(frame, s.point)),
        i => (task.cases)()
            .into_iter()
            .chain((task.hidden)())
            .nth(i - 1),
    }
}

const THUMB_EMPTY: [u8; 4] = [7, 10, 18, 235];

/// Squeezes a run's trace into the corner river: each column is a run of
/// steps, each row a lane, brighter where the lane was busy more often.
pub fn paint_thumb(img: &mut Image, trace: &[Step]) {
    let w = THUMB_W as usize;
    let Some(data) = img.data.as_mut() else {
        return;
    };
    let n = trace.len();
    for c in 0..w {
        // Short runs keep one column per step, flowing in from the right.
        let (a, b) = if n <= w {
            match (c + n).checked_sub(w) {
                Some(i) => (i, i + 1),
                None => (0, 0),
            }
        } else {
            (c * n / w, (c + 1) * n / w)
        };
        let steps = &trace[a.min(n)..b.min(n)];
        let split = steps.iter().filter(|s| s.mask != u32::MAX).count();
        let stale = steps.iter().any(|s| s.stale);
        let hue = if stale {
            RIVER_STALE
        } else if split * 2 > steps.len() {
            RIVER_SPLIT
        } else {
            RIVER_FULL
        };
        for lane in 0..LANES {
            let on = steps.iter().filter(|s| s.mask & (1 << lane) != 0).count();
            let k = if steps.is_empty() {
                0.0
            } else {
                on as f32 / steps.len() as f32
            };
            let px: [u8; 4] = std::array::from_fn(|i| {
                let (e, h) = (THUMB_EMPTY[i] as f32, hue[i] as f32);
                if i == 3 { 235 } else { (e + (h - e) * k) as u8 }
            });
            // Alternate columns a shade darker, as on the full river.
            let px = if c % 2 == 1 { px.map(|v| if v == 235 { v } else { (v as f32 * 0.8) as u8 }) } else { px };
            let at = (lane * w + c) * 4;
            if at + 4 <= data.len() {
                data[at..at + 4].copy_from_slice(&px);
            }
        }
    }
}

/// The frame-time line and its colour.
fn hud_text(cycles: u32, budget: Option<u32>, l: oo_base::Lang) -> (String, bool) {
    // The budget is one 60 Hz refresh; a slower kernel holds a frame longer.
    let Some(b) = budget else {
        return (format!("{cycles} {}", t(l, "cyc", "周期")), true);
    };
    let ms = cycles as f32 / b as f32 * REFRESH_MS;
    let k = span(cycles, budget);
    let fps = 60 / k;
    (format!("{ms:.1} ms · {fps} fps"), k == 1)
}

/// How far one press turns a knob that started at `base`.
fn knob_step(base: i32) -> i32 {
    (base.unsigned_abs() / 8).max(1) as i32
}

pub fn draw(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let task = s.task();
    let (at, size) = SHOW;
    let era = fb::era(task);
    ui.chip(at, size + Vec2::splat(14.0), pal.secondary.with_alpha(0.35), 6.0);
    ui.glow(at, size.x * 1.6, pal.secondary.with_alpha(0.08), 5.5);
    ui.chip(at, size + Vec2::splat(10.0), pal.bg_deep, 6.2);
    ui.commands.spawn((
        Sprite {
            image: v.fb.pair(task).0.clone(),
            custom_size: Some(size),
            ..default()
        },
        Transform::from_xyz(at.x, at.y, 7.0),
        oo_kit::ui::ScreenEntity,
    ));
    if task.view != ooc_core::task::View::Cells {
        fb::crt(ui, v.fb, at, size, era);
    }
    draw_corner(ui, v);
    let x0 = 160.0;
    ui.caption(
        t(l, "SHOWCASE · LIVE", "展示 · 实时"),
        Vec2::new(x0, 262.0),
        pal.secondary,
        Anchor::CENTER_LEFT,
    );
    ui.text(task.title.get(l), Vec2::new(x0, 230.0), 28.0, pal.text, Anchor::CENTER_LEFT);
    let body = t(
        l,
        "Your kernel, running every frame. Each bulb is one thread; each thread ran your program alone and never saw its neighbours.",
        "你的内核在逐帧运行。每个灯泡是一个线程；每个线程都独自运行你的程序，从没看见过邻居。",
    );
    ui.wrapped(body, Vec2::new(x0, 204.0), 440.0, 15.0, pal.text_dim, Anchor::TOP_LEFT);
    let e = ui.text("", Vec2::new(x0, 136.0), 15.0, pal.secondary, Anchor::CENTER_LEFT);
    ui.commands.entity(e).insert(ShowStat);
    draw_inputs(ui, v, x0, 100.0);
    if s.show.input == 0 && uses_point(task) {
        ui.caption(
            task.steer.map_or(t(l, "MOVE THE HERO", "移动主角"), |c| c.get(l)),
            Vec2::new(x0, 28.0),
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
        // A steered level shows the turn itself, not a coordinate.
        let at_text = match task.steer {
            Some(_) => format!("{:+}", s.point[0] - 4),
            None => format!("({}, {})", s.point[0], s.point[1]),
        };
        ui.text(at_text, Vec2::new(x0 + 440.0, 28.0), 16.0, pal.text, Anchor::CENTER_RIGHT);
        let help = t(
            l,
            "Arrow keys, or click the screen. Only the ROM changes — the kernel is the same.",
            "方向键，或点击屏幕。变的只有 ROM——内核还是同一个。",
        );
        ui.wrapped(help, Vec2::new(x0, 14.0), 440.0, 14.0, pal.text_dim, Anchor::TOP_LEFT);
    }
    let steered = s.show.input == 0 && uses_point(task);
    draw_knobs(ui, v, x0, if steered { -62.0 } else { 20.0 });
    ui.button(
        ButtonSpec::new(
            Vec2::new(x0 + 110.0, -300.0),
            Vec2::new(220.0, 44.0),
            t(l, "‹ Back to results", "‹ 回到结果"),
            Act::Back,
        )
        .tone(Tone::Ghost)
        .font(15.0),
    );
}

/// Frame time and the Warp River thumbnail, hung under the screen's corner.
fn draw_corner(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let (at, size) = SHOW;
    let w = Vec2::new(300.0, 62.0);
    let c = Vec2::new(at.x + size.x / 2.0 - w.x / 2.0 + 7.0, at.y - size.y / 2.0 - w.y / 2.0 - 16.0);
    ui.chip(c, w, pal.bg_deep.with_alpha(0.9), 6.0);
    ui.chip(c, w - Vec2::splat(2.0), pal.bg.with_alpha(0.6), 6.05);
    let tw = Vec2::new(150.0, 48.0);
    let tc = c + Vec2::new(w.x / 2.0 - tw.x / 2.0 - 7.0, 0.0);
    ui.commands.spawn((
        Sprite {
            image: v.fb.thumb.clone(),
            custom_size: Some(tw),
            ..default()
        },
        Transform::from_xyz(tc.x, tc.y, 6.3),
        oo_kit::ui::ScreenEntity,
    ));
    let lx = c.x - w.x / 2.0 + 12.0;
    ui.caption(
        t(v.lang, "FRAME TIME", "帧时间"),
        Vec2::new(lx, c.y + 14.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let e = ui.text("", Vec2::new(lx, c.y - 10.0), 14.0, pal.good, Anchor::CENTER_LEFT);
    ui.commands.entity(e).insert(ShowHud);
}

/// The townsfolk's inputs: the live scene, then every test case.
fn draw_inputs(ui: &mut Ui, v: &View, x0: f32, y: f32) {
    let pal = ui.pal;
    let l = v.lang;
    let task = v.s.task();
    let n = (task.cases)().len() + (task.hidden)().len();
    ui.caption(
        t(l, "THE TOWNSFOLK'S INPUT", "镇民换输入"),
        Vec2::new(x0, y),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let row = y - 30.0;
    let live = task.live.is_some();
    let mut x = x0;
    if live {
        ui.button(
            ButtonSpec::new(Vec2::new(x + 40.0, row), Vec2::new(80.0, 30.0), t(l, "Scene", "现场"), Act::ShowInput(0))
                .selected(v.s.show.input == 0)
                .font(14.0)
                .tip(t(l, "The scene in town, moving.", "镇上的现场画面，会动。")),
        );
        x += 88.0;
    }
    for i in 1..=n.min(8) {
        ui.button(
            ButtonSpec::new(Vec2::new(x + 19.0, row), Vec2::new(38.0, 30.0), i.to_string(), Act::ShowInput(i))
                .selected(v.s.show.input == i)
                .font(14.0)
                .tip(t(l, "A test case the town sent in.", "镇上送来的一组测试输入。")),
        );
        x += 44.0;
    }
}

/// The kernel's constants, each with − and + buttons.
fn draw_knobs(ui: &mut Ui, v: &View, x0: f32, y: f32) {
    let pal = ui.pal;
    let l = v.lang;
    let knobs = &v.s.show.knobs;
    ui.caption(
        t(l, "TURN THE CONSTANTS", "实时改参数"),
        Vec2::new(x0, y),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    if knobs.is_empty() {
        ui.text(
            t(l, "This kernel has no constants to turn.", "这个内核没有可调的常数。"),
            Vec2::new(x0, y - 28.0),
            14.0,
            pal.text_dim,
            Anchor::CENTER_LEFT,
        );
        return;
    }
    let moved = knobs.iter().any(|k| k.value != k.base);
    ui.button(
        ButtonSpec::new(Vec2::new(x0 + 410.0, y), Vec2::new(60.0, 24.0), t(l, "Reset", "复原"), Act::KnobReset)
            .tone(Tone::Ghost)
            .font(13.0)
            .enabled(moved),
    );
    let code = v.s.show.base.as_ref().map(|p| &p.code);
    for (i, k) in knobs.iter().enumerate() {
        let ry = y - 32.0 - i as f32 * 32.0;
        ui.chip(Vec2::new(x0 + 220.0, ry), Vec2::new(440.0, 28.0), pal.bg_deep.with_alpha(0.7), 6.0);
        let line = code
            .and_then(|c| c.get(k.pc))
            .map(|ins| {
                let mut ins = *ins;
                let o = match operands(&ins).get(k.slot) {
                    Some(Operand::Count(_)) => Operand::Count(k.value),
                    _ => Operand::Src(Src::Imm(k.value)),
                };
                set_operand(&mut ins, k.slot, o);
                ins.to_string()
            })
            .unwrap_or_default();
        ui.text(
            format!("{:>2}  {line}", k.pc + 1),
            Vec2::new(x0 + 10.0, ry),
            13.0,
            if k.value == k.base { pal.text_dim } else { pal.secondary },
            Anchor::CENTER_LEFT,
        );
        let d = knob_step(k.base);
        for (dx, label, by) in [(352.0, "−", -d), (400.0, "+", d)] {
            ui.button(
                ButtonSpec::new(Vec2::new(x0 + dx, ry), Vec2::new(40.0, 24.0), label, Act::Knob(i, by))
                    .tone(Tone::Ghost)
                    .font(15.0),
            );
        }
    }
    ui.text(
        t(
            l,
            "Only this showcase changes. Your saved work and score stay as they are.",
            "只改这次展演，存档里的作品和成绩不变。",
        ),
        Vec2::new(x0, y - 32.0 - knobs.len() as f32 * 32.0 - 4.0),
        13.0,
        pal.text_faint,
        Anchor::TOP_LEFT,
    );
}

/// What the last showcase frame ran, so unchanged frames are skipped.
#[derive(Default)]
pub struct Last {
    key: Option<(i32, [i32; 2], usize, usize, u32)>,
    program: Option<(u64, u32, Option<Program>)>,
    /// Refreshes the last frame took; the clock holds a frame that long.
    span: u32,
    /// The previous picture, faded into the next as phosphor afterglow.
    glow: Vec<u8>,
}

/// Runs the kernel on the showcase input whenever the shown frame changes.
#[allow(clippy::too_many_arguments)]
pub fn tick(
    time: Res<Time>,
    mut s: ResMut<Session>,
    fbi: Res<FbImages>,
    mut images: ResMut<Assets<Image>>,
    mut stat: Query<&mut Text2d, (With<ShowStat>, Without<ShowHud>)>,
    mut hud: Query<(&mut Text2d, &mut TextColor), With<ShowHud>>,
    game: Res<crate::state::Game>,
    reduce: Res<oo_kit::scene::ReduceMotion>,
    pal: Res<oo_kit::theme::Palette>,
    mut heard: ResMut<crate::sound::ShowTrace>,
    mut last: Local<Last>,
) {
    s.clock += time.delta_secs();
    let task = s.task();
    // An over-budget kernel holds each picture for several refreshes.
    let tick = (s.clock * FPS) as i32;
    let hold = last.span.max(1) as i32;
    let frame = tick - tick.rem_euclid(hold);
    let key = (frame, s.point, s.show.input, stat.iter().count(), s.show.rev);
    if last.key == Some(key) {
        return;
    }
    last.key = Some(key);
    let rev = (s.generation, s.show.rev);
    if last.program.as_ref().is_none_or(|c| (c.0, c.1) != rev) {
        let p = s
            .show
            .base
            .clone()
            .or_else(|| program(&s, &game.save.seals))
            .map(|p| tuned(&p, &s.show.knobs));
        last.program = Some((rev.0, rev.1, p));
    }
    let Some((_, _, Some(prog))) = last.program.as_ref() else {
        return;
    };
    let Some(case) = input(task, &s, frame) else {
        return;
    };
    let mut out = (task.init)(&case);
    out.resize(task.out_len, 0);
    let run = run_frame(prog, &case.rom, task.launch_for(prog, &case), task.profile, case.frame, &mut out);
    let budget = budget(task);
    last.span = span(run.stats.cycles, budget);
    if let Some(mut img) = images.get_mut(fbi.pair(task).0) {
        let shade = if task.view == ooc_core::task::View::Cells {
            fb::Shade::of(out.iter())
        } else {
            fb::Shade::Palette
        };
        fb::paint_values(&mut img, &out, shade);
        if fb::era(task) == fb::Era::Amber && task.view != ooc_core::task::View::Cells {
            fb::amber_tint(&mut img, 0.85);
        }
        if !reduce.0 && task.view != ooc_core::task::View::Cells {
            afterglow(&mut img, &mut last.glow);
        }
    }
    if let Some(mut img) = images.get_mut(&fbi.thumb) {
        paint_thumb(&mut img, &run.trace);
    }
    let l = game.lang();
    let (line, on_time) = hud_text(run.stats.cycles, budget, l);
    for (mut text, mut color) in &mut hud {
        text.0 = line.clone();
        color.0 = if on_time { pal.good } else { pal.warn };
    }
    for mut text in &mut stat {
        text.0 = if on_time {
            format!(
                "{} {frame} · {} {} · {} {:.0}%",
                t(l, "frame", "帧"),
                run.stats.cycles,
                t(l, "cycles", "周期"),
                t(l, "lanes busy", "车道忙碌"),
                run.utilization() * 100.0
            )
        } else {
            format!(
                "{} {} {} · {}",
                t(l, "frame", "帧"),
                run.stats.cycles,
                t(l, "cycles", "周期"),
                t(l, "over budget, dropping frames", "超出预算，正在掉帧")
            )
        };
    }
    crate::sound::show_trace(&mut heard, run.trace);
}

/// Lit pixels of the last frame fade instead of vanishing.
fn afterglow(img: &mut Image, prev: &mut Vec<u8>) {
    let Some(data) = img.data.as_mut() else {
        return;
    };
    if prev.len() == data.len() {
        for (px, old) in data.iter_mut().zip(prev.iter()) {
            *px = (*px).max((*old as f32 * 0.45) as u8);
        }
    }
    prev.clear();
    prev.extend_from_slice(data);
}

/// Arrow keys or a click move the point the live case is built around.
pub fn input_sys(keys: Res<ButtonInput<KeyCode>>, cursor: Res<Cursor>, mut cx: Ctx) {
    if cx.s.show.input != 0 || !uses_point(cx.s.task()) {
        return;
    }
    let mut p = cx.s.point;
    for (k, d) in [
        (KeyCode::ArrowLeft, [-1, 0]),
        (KeyCode::ArrowRight, [1, 0]),
        (KeyCode::ArrowUp, [0, -1]),
        (KeyCode::ArrowDown, [0, 1]),
    ] {
        if keys.just_pressed(k) {
            p = [p[0] + d[0], p[1] + d[1]];
        }
    }
    let sw = fb::screen_w(cx.s.task());
    if cursor.pressed
        && !cursor.over_button
        && let Some(w) = cursor.world
        && let Some(c) = fb::cell_at(w, sw, SHOW.0, SHOW.1, false)
    {
        // The click lands on the sprite's centre.
        p = [(c % sw) as i32 - 4, (c / sw) as i32 - 4];
    }
    let hi = sw as i32 - 1;
    let p = [p[0].clamp(-7, hi), p[1].clamp(-7, hi)];
    if p != cx.s.point {
        cx.s.point = p;
        cx.dirty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_drop_in_whole_refreshes() {
        assert_eq!(span(800, Some(960)), 1);
        assert_eq!(span(960, Some(960)), 1);
        assert_eq!(span(961, Some(960)), 2);
        assert_eq!(span(3000, Some(960)), 4);
        assert_eq!(span(5, None), 1);
    }

    #[test]
    fn every_commission_has_a_budget_it_meets() {
        for task in ooc_core::content::tasks().iter().filter(|t| t.live.is_some()) {
            let Ok(rep) = ooc_core::grade::grade(task, &task.reference.solution()) else {
                continue;
            };
            assert_eq!(span(rep.measure.cycles, budget(task)), 1, "{}", task.id);
        }
    }

    #[test]
    fn knobs_write_back() {
        let task = ooc_core::content::task("tide-board").unwrap();
        let rep = ooc_core::grade::grade(task, &task.reference.solution()).unwrap();
        let mut ks = knobs(&rep.program);
        assert!(!ks.is_empty() && ks.len() <= KNOBS);
        assert_eq!(tuned(&rep.program, &ks), rep.program);
        ks[0].value += 1;
        assert_ne!(tuned(&rep.program, &ks), rep.program);
    }
}
