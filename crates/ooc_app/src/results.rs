//! After a solve: the verdict, the insight card, and the Showcase where the
//! finished kernel drives the machine live.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_kit::ui::{ButtonSpec, Cursor, Tone, Ui};
use ooc_core::content;
use ooc_core::grade::build;
use ooc_core::isa::Program;
use ooc_core::task::Task;
use ooc_core::vm::run_frame;

use crate::fb::{self, FbImages};
use crate::state::{Act, Screen, Session, t};
use crate::{Ctx, View, bench, hub};

/// Frames per second of the showcase clock.
const FPS: f32 = 8.0;

pub fn act(cx: &mut Ctx, a: Act) {
    match a {
        Act::Next => {
            let save = &cx.game.save;
            let tasks = content::tasks();
            let open =
                |i: &usize| save.is_unlocked(&tasks[*i]) && !save.campaign.is_solved(tasks[*i].id);
            let after = (cx.s.task + 1..tasks.len()).find(open);
            match after.or_else(|| hub::next_task(save)) {
                Some(i) => {
                    bench::open(&mut cx.s, &cx.game, i);
                    cx.go(Screen::Brief);
                }
                None => cx.go(Screen::Hub),
            }
        }
        Act::Showcase if cx.s.task().live.is_some() => {
            start_showcase(&mut cx.s);
            cx.go(Screen::Showcase);
        }
        _ => {}
    }
    cx.dirty();
}

/// Rewinds the showcase clock and puts the hero back at its start.
pub fn start_showcase(s: &mut Session) {
    s.clock = 0.0;
    s.point = [4, 4];
}

/// Whether the showcase case depends on the player-moved point.
fn uses_point(task: &Task) -> bool {
    task.live.is_some_and(|f| f(0, [0, 0]) != f(0, [5, 5]))
}

/// The kernel the showcase runs: the graded one, or the work rebuilt.
fn program(s: &Session) -> Option<Program> {
    match &s.report {
        Some(r) => Some(r.program.clone()),
        None => build(s.task(), s.work.as_ref()?).ok().map(|b| b.program),
    }
}

pub fn draw_results(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let task = s.task();
    let rec = v
        .game
        .save
        .campaign
        .get(task.id)
        .cloned()
        .unwrap_or_default();
    // Left: what the run achieved.
    let left = Vec2::new(-330.0, 20.0);
    ui.card(left, Vec2::new(560.0, 540.0), pal.good);
    let x0 = left.x - 250.0;
    if let Some(cat) = ooc_core::content::chapter(task.chapter).client {
        let at = Vec2::new(left.x + 170.0, -122.0);
        ui.well(at, Vec2::splat(100.0));
        oo_kit::portrait::spawn(ui, cat, at, 96.0, 20.0);
        ui.caption(
            t(l, "ASKED BY", "委托人"),
            Vec2::new(at.x, at.y - 64.0),
            pal.text_faint,
            Anchor::CENTER,
        );
        ui.text(
            cat.name().get(l).to_string(),
            Vec2::new(at.x, at.y - 84.0),
            14.0,
            pal.text_dim,
            Anchor::CENTER,
        );
    }
    ui.caption(
        format!(
            "{} · {}",
            hub::chapter_word(task.chapter, l),
            t(l, "MACHINE REBUILT", "机器已复刻")
        ),
        Vec2::new(x0, 262.0),
        pal.good,
        Anchor::CENTER_LEFT,
    );
    ui.text(
        task.title.get(l),
        Vec2::new(x0, 226.0),
        30.0,
        pal.text,
        Anchor::CENTER_LEFT,
    );
    let stars = s.report.as_ref().map_or(0, |r| r.stars());
    crate::fx::stars(ui, Vec2::new(left.x, 160.0), stars, 22.0);
    if s.improved && stars > 0 {
        crate::fx::echo(
            ui,
            Vec2::new(left.x, 160.0),
            0.35 + (stars - 1) as f32 * 0.28,
        );
    }
    let (msg, color) = if s.improved {
        (
            t(l, "A new best for this level.", "这一关的新纪录。"),
            pal.secondary,
        )
    } else if rec.stars > stars {
        (
            t(
                l,
                "Your earlier solution earned more stars — that record stays.",
                "你之前的解法星更多——纪录保留。",
            ),
            pal.text_dim,
        )
    } else {
        (t(l, "Solved.", "已解决。"), pal.text_dim)
    };
    ui.text(msg, Vec2::new(left.x, 116.0), 15.0, color, Anchor::CENTER);
    if rec.assisted {
        ui.text(
            t(
                l,
                "Solved with the walkthrough: at most ★★ here.",
                "看过完整解法：这里最多 ★★。",
            ),
            Vec2::new(left.x, 94.0),
            12.0,
            pal.text_faint,
            Anchor::CENTER,
        );
    }
    if let Some(rep) = &s.report {
        ui.caption(
            t(l, "THIS RUN", "本次运行"),
            Vec2::new(x0, 58.0),
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
        let rows = task
            .limit
            .into_iter()
            .map(|g| (t(l, "LIMIT", "上限").to_string(), g))
            .chain(
                task.goals
                    .into_iter()
                    .enumerate()
                    .map(|(i, g)| (format!("★{}", i + 2), g)),
            );
        for (i, (label, g)) in rows.enumerate() {
            let y = 28.0 - i as f32 * 38.0;
            let got = rep.measure.get(g.metric);
            let met = rep.measure.meets(g);
            let accent = if met { pal.good } else { pal.warn };
            ui.chip(
                Vec2::new(left.x, y),
                Vec2::new(500.0, 32.0),
                pal.glass.with_alpha(0.8),
                8.0,
            );
            ui.dot(Vec2::new(x0 + 12.0, y), 8.0, accent, 9.0);
            ui.text(
                format!("{label}  {}", hub::goal_text(g, l)),
                Vec2::new(x0 + 26.0, y),
                14.0,
                pal.text,
                Anchor::CENTER_LEFT,
            );
            ui.text(
                got.to_string(),
                Vec2::new(left.x + 236.0, y),
                17.0,
                accent,
                Anchor::CENTER_RIGHT,
            );
        }
        let util = rep
            .runs
            .iter()
            .map(|r| r.run.stats.utilization())
            .fold(1.0f32, f32::min);
        let line = format!(
            "{} {:.0}%",
            t(l, "Worst-case lane utilization:", "最差用例的车道利用率："),
            util * 100.0
        );
        ui.text(
            line,
            Vec2::new(x0, -96.0),
            13.0,
            pal.text_dim,
            Anchor::CENTER_LEFT,
        );
    }
    if let Some(best) = rec.best {
        let line = format!(
            "{} {} {best}",
            t(l, "Best ever,", "历史最佳，"),
            task.goals[0].metric.label().get(l)
        );
        ui.text(
            line,
            Vec2::new(x0, -118.0),
            13.0,
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
    }
    if !s.unlocked.is_empty() {
        ui.caption(
            t(l, "NOW OPEN", "新开放"),
            Vec2::new(x0, -150.0),
            pal.secondary,
            Anchor::CENTER_LEFT,
        );
        let names: Vec<&str> = s
            .unlocked
            .iter()
            .map(|i| content::tasks()[*i].title.get(l))
            .collect();
        ui.wrapped(
            names.join(" · "),
            Vec2::new(x0, -164.0),
            500.0,
            15.0,
            pal.text,
            Anchor::TOP_LEFT,
        );
    }
    draw_insight(ui, task, l);
    let y = -290.0;
    ui.button(
        ButtonSpec::new(
            Vec2::new(500.0, y),
            Vec2::new(220.0, 48.0),
            t(l, "Next level ›", "下一关 ›"),
            Act::Next,
        )
        .primary()
        .font(17.0),
    );
    if task.live.is_some() {
        ui.button(
            ButtonSpec::new(
                Vec2::new(272.0, y),
                Vec2::new(200.0, 48.0),
                t(l, "Showcase ▶", "展示 ▶"),
                Act::Showcase,
            )
            .tone(Tone::Secondary)
            .font(16.0)
            .tip(t(
                l,
                "Watch your kernel drive the machine live.",
                "看你的内核实时驱动这台机器。",
            )),
        );
    }
    ui.button(
        ButtonSpec::new(
            Vec2::new(-470.0, y),
            Vec2::new(160.0, 40.0),
            t(l, "‹ Bench", "‹ 工作台"),
            Act::Goto(Screen::Bench),
        )
        .tone(Tone::Ghost)
        .tip(t(l, "Chase the stars you missed.", "去拿漏掉的星。")),
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
}

/// The card the solve earns: the idea, how real GPUs do it, a myth, and what OC-1 leaves out.
fn draw_insight(ui: &mut Ui, task: &Task, l: oo_base::Lang) {
    let pal = ui.pal;
    let i = &task.insight;
    let at = Vec2::new(300.0, 20.0);
    ui.card(at, Vec2::new(640.0, 540.0), pal.secondary);
    let x0 = at.x - 290.0;
    ui.caption(
        t(l, "INSIGHT CARD", "洞见卡"),
        Vec2::new(x0, 262.0),
        pal.secondary,
        Anchor::CENTER_LEFT,
    );
    ui.text(
        i.title.get(l),
        Vec2::new(x0, 230.0),
        24.0,
        pal.text,
        Anchor::CENTER_LEFT,
    );
    ui.wrapped(
        i.line.get(l),
        Vec2::new(x0, 204.0),
        580.0,
        16.0,
        pal.text,
        Anchor::TOP_LEFT,
    );
    let parts = [
        (t(l, "ON A REAL GPU", "真实 GPU 上"), i.reality, pal.primary),
        (t(l, "A COMMON MYTH", "常见误解"), i.myth, pal.warn),
        (
            t(l, "WHAT OC-1 SIMPLIFIES", "OC-1 简化了什么"),
            i.simplified,
            pal.text_faint,
        ),
    ];
    for (n, (head, body, color)) in parts.into_iter().enumerate() {
        let y = 110.0 - n as f32 * 112.0;
        ui.rect(
            Vec2::new(x0 + 1.5, y - 38.0),
            Vec2::new(3.0, 86.0),
            color,
            8.0,
        );
        ui.caption(head, Vec2::new(x0 + 14.0, y), color, Anchor::CENTER_LEFT);
        ui.wrapped(
            body.get(l),
            Vec2::new(x0 + 14.0, y - 14.0),
            566.0,
            14.0,
            pal.text_dim,
            Anchor::TOP_LEFT,
        );
    }
}

/// Where the showcase screen sits.
const SHOW: (Vec2, Vec2) = (Vec2::new(-150.0, 10.0), Vec2::splat(520.0));

/// Live readout under the showcase screen.
#[derive(Component)]
pub struct ShowStat;

pub fn draw_showcase(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let task = s.task();
    let (at, size) = SHOW;
    ui.chip(
        at,
        size + Vec2::splat(14.0),
        pal.secondary.with_alpha(0.35),
        6.0,
    );
    ui.glow(at, size.x * 1.6, pal.secondary.with_alpha(0.08), 5.5);
    ui.chip(at, size + Vec2::splat(10.0), ui.pal.bg_deep, 6.2);
    let image = v.fb.pair(task.view).0.clone();
    ui.commands.spawn((
        Sprite {
            image,
            custom_size: Some(size),
            ..default()
        },
        Transform::from_xyz(at.x, at.y, 7.0),
        oo_kit::ui::ScreenEntity,
    ));
    let x0 = 160.0;
    ui.caption(
        t(l, "SHOWCASE · LIVE", "展示 · 实时"),
        Vec2::new(x0, 262.0),
        pal.secondary,
        Anchor::CENTER_LEFT,
    );
    ui.text(
        task.title.get(l),
        Vec2::new(x0, 230.0),
        28.0,
        pal.text,
        Anchor::CENTER_LEFT,
    );
    let body = t(
        l,
        "Your kernel, running every frame. Each bulb is one thread; each thread ran your program alone and never saw its neighbours.",
        "你的内核在逐帧运行。每个灯泡是一个线程；每个线程都独自运行你的程序，从没看见过邻居。",
    );
    ui.wrapped(
        body,
        Vec2::new(x0, 200.0),
        440.0,
        15.0,
        pal.text_dim,
        Anchor::TOP_LEFT,
    );
    let e = ui.text(
        "",
        Vec2::new(x0, 80.0),
        15.0,
        pal.secondary,
        Anchor::CENTER_LEFT,
    );
    ui.commands.entity(e).insert(ShowStat);
    if uses_point(task) {
        ui.caption(
            task.steer
                .map_or(t(l, "MOVE THE HERO", "移动主角"), |c| c.get(l)),
            Vec2::new(x0, 30.0),
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
        let help = t(
            l,
            "Arrow keys, or click the screen. Only the ROM changes — the kernel is the same.",
            "方向键，或点击屏幕。变的只有 ROM——内核还是同一个。",
        );
        ui.wrapped(
            help,
            Vec2::new(x0, 14.0),
            440.0,
            14.0,
            pal.text_dim,
            Anchor::TOP_LEFT,
        );
        // A steered level shows the turn itself, not a coordinate.
        ui.text(
            match task.steer {
                Some(_) => format!("{:+}", s.point[0] - 4),
                None => format!("({}, {})", s.point[0], s.point[1]),
            },
            Vec2::new(x0, -46.0),
            18.0,
            pal.text,
            Anchor::CENTER_LEFT,
        );
    }
    ui.button(
        ButtonSpec::new(
            Vec2::new(x0 + 110.0, -280.0),
            Vec2::new(220.0, 46.0),
            t(l, "‹ Back to results", "‹ 回到结果"),
            Act::Back,
        )
        .tone(Tone::Ghost)
        .font(15.0),
    );
}

/// Runs the kernel on the live case whenever the frame or point changes.
pub fn tick_showcase(
    time: Res<Time>,
    mut s: ResMut<Session>,
    fbi: Res<FbImages>,
    mut images: ResMut<Assets<Image>>,
    mut stat: Query<&mut Text2d, With<ShowStat>>,
    game: Res<crate::state::Game>,
    mut last: Local<Option<(i32, [i32; 2], usize)>>,
    mut cache: Local<Option<(u64, Option<Program>)>>,
) {
    s.clock += time.delta_secs();
    let task = s.task();
    let Some(live) = task.live else { return };
    let frame = (s.clock * FPS) as i32;
    let key = (frame, s.point, stat.iter().count());
    if *last == Some(key) {
        return;
    }
    *last = Some(key);
    if cache.as_ref().is_none_or(|c| c.0 != s.generation) {
        *cache = Some((s.generation, program(&s)));
    }
    let Some(Some(prog)) = cache.as_ref().map(|c| &c.1) else {
        return;
    };
    let case = live(frame, s.point);
    let mut out = (task.init)(&case);
    out.resize(task.out_len, 0);
    let run = run_frame(
        prog,
        &case.rom,
        task.launch_for(prog, &case),
        task.profile,
        case.frame,
        &mut out,
    );
    let handle = fbi.pair(task.view).0;
    if let Some(mut img) = images.get_mut(handle) {
        fb::paint_values(&mut img, &out);
    }
    let l = game.lang();
    for mut text in &mut stat {
        text.0 = format!(
            "{} {frame} · {} {} · {} {:.0}%",
            t(l, "frame", "帧"),
            run.stats.cycles,
            t(l, "cycles", "周期"),
            t(l, "lanes busy", "车道忙碌"),
            run.stats.utilization() * 100.0
        );
    }
}

/// Arrow keys or a click move the point the live case is built around.
pub fn showcase_input(keys: Res<ButtonInput<KeyCode>>, cursor: Res<Cursor>, mut cx: Ctx) {
    if !uses_point(cx.s.task()) {
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
    if cursor.pressed
        && !cursor.over_button
        && let Some(w) = cursor.world
        && let Some(c) = fb::cell_at(w, SHOW.0, SHOW.1, false)
    {
        // The click lands on the sprite's centre.
        p = [(c % 16) as i32 - 4, (c / 16) as i32 - 4];
    }
    let p = [p[0].clamp(-7, 15), p[1].clamp(-7, 15)];
    if p != cx.s.point {
        cx.s.point = p;
        cx.dirty();
    }
}
