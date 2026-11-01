//! After a solve: the verdict, the insight card, and the Showcase where the
//! finished kernel drives the machine live.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_kit::ui::{ButtonSpec, Tone, Ui};
use ooc_core::content;
use ooc_core::task::Task;

use crate::state::{Act, Screen, t};
use crate::{Ctx, View, bench, hub};

pub fn act(cx: &mut Ctx, a: Act) {
    if cx.s.open.is_some() {
        return crate::problems::act(cx, a);
    }
    match a {
        Act::Next if crate::notes::ending_open(&cx.game.save) && cx.game.save.ending.is_none() => {
            cx.go(Screen::Ending);
        }
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
        Act::Seal => crate::seals::seal_work(cx),
        Act::Lab(_) => hub::act(cx, a),
        Act::Showcase if cx.s.task().live.is_some() => {
            crate::showcase::start(&mut cx.s, &cx.game.save.seals);
            cx.go(Screen::Showcase);
        }
        _ => {}
    }
    cx.dirty();
}

pub fn draw_results(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    if s.open.is_some() {
        return crate::problems::draw_results(ui, v);
    }
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
            .map(|r| r.run.utilization())
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
            if crate::notes::ending_open(&v.game.save) && v.game.save.ending.is_none() {
                t(l, "Cen Li's offer ›", "岑理的方案 ›")
            } else {
                t(l, "Next level ›", "下一关 ›")
            },
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
    crate::seals::draw_button(ui, v);
}

/// The card the solve earns: the idea, how real GPUs do it, a myth, and what OC-1 leaves out.
/// Index of `task` in the campaign.
fn v_task(task: &Task) -> usize {
    content::tasks()
        .iter()
        .position(|x| x.id == task.id)
        .unwrap_or(0)
}

fn draw_insight(ui: &mut Ui, task: &Task, l: oo_base::Lang) {
    draw_insight_card(ui, &task.insight, Vec2::new(300.0, 20.0), l, Some(v_task(task)));
}

/// A five-part concept card, 640 × 540, centred on `at`.
/// `try_it`: the level whose machine the card's "Try it" button opens on
/// the free bench.
pub fn draw_insight_card(
    ui: &mut Ui,
    i: &ooc_core::task::Insight,
    at: Vec2,
    l: oo_base::Lang,
    try_it: Option<usize>,
) {
    let pal = ui.pal;
    ui.card(at, Vec2::new(640.0, 540.0), pal.secondary);
    let x0 = at.x - 290.0;
    if let Some(k) = try_it {
        ui.button(
            ButtonSpec::new(
                Vec2::new(at.x + 234.0, at.y + 236.0),
                Vec2::new(124.0, 28.0),
                t(l, "Try it ›", "试一试 ›"),
                Act::Lab(k),
            )
            .tone(Tone::Secondary)
            .font(13.0)
            .tip(t(
                l,
                "Open this machine on the free bench and play with the idea. Never scored.",
                "在自由机台上打开这台机器，随手试试这个想法。不计分。",
            )),
        );
    }
    ui.caption(
        t(l, "INSIGHT CARD", "洞见卡"),
        Vec2::new(x0, at.y + 242.0),
        pal.secondary,
        Anchor::CENTER_LEFT,
    );
    ui.text(
        i.title.get(l),
        Vec2::new(x0, at.y + 210.0),
        24.0,
        pal.text,
        Anchor::CENTER_LEFT,
    );
    ui.wrapped(
        i.line.get(l),
        Vec2::new(x0, at.y + 184.0),
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
        let y = at.y + 90.0 - n as f32 * 112.0;
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
