//! The hub (sixteen chapters under six machines, each with its client)
//! and the briefing with its Predict question.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_base::Lang;
use oo_base::LevelKind;
use oo_kit::theme::mix;
use oo_kit::ui::{ButtonSpec, Tone, Ui};
use oo_kit::ui::{TipArea, Tooltip};
use ooc_core::content;
use ooc_core::save::SaveData;
use ooc_core::task::{Chapter, Goal, LEVELS_PER_CHAPTER, Metric, Task, kind_of_slot};

use crate::state::{Act, Screen, t};
use crate::{Ctx, View, bench};

pub fn act(cx: &mut Ctx, a: Act) {
    match a {
        Act::Open(i) => {
            bench::open(&mut cx.s, &cx.game, i);
            cx.go(Screen::Brief);
        }
        Act::Predict(i) => {
            if cx.s.pick.is_none() {
                cx.s.pick = Some(i);
                let task = cx.s.task();
                SaveData::note_once(&mut cx.game.save.predicted, task.id);
                let cue = if i == task.predict.answer {
                    oo_kit::audio::Cue::Success
                } else {
                    oo_kit::audio::Cue::Error
                };
                cx.cues.play(cue);
                cx.game.touch();
                cx.dirty();
            }
        }
        Act::StartBench => cx.go(Screen::Bench),
        Act::Chapter(n) => {
            cx.s.chapter = Some(n);
            cx.dirty();
        }
        _ => {}
    }
}

pub fn kind_label(k: LevelKind, l: Lang) -> &'static str {
    match k {
        LevelKind::Puzzle => t(l, "PUZZLE", "解谜"),
        LevelKind::Fault => t(l, "FAULT", "故障"),
        LevelKind::Commission => t(l, "COMMISSION", "委托"),
    }
}

/// `CHAPTER 3`, `第 3 章`, `3장`.
pub fn chapter_word(n: u8, l: Lang) -> String {
    match l {
        Lang::En => format!("CHAPTER {n}"),
        Lang::Zh | Lang::Ja => format!("第 {n} 章"),
        Lang::Ko => format!("{n}장"),
    }
}

/// `CHAPTER 2 · Lumen + cartridge · 2-5 · FAULT`, for screen captions.
pub fn task_caption(task: &Task, l: Lang) -> String {
    format!(
        "{} · {} · {} · {}",
        chapter_word(task.chapter, l),
        content::chapter(task.chapter).machine.get(l),
        task.number(),
        kind_label(task.kind(), l)
    )
}

fn hex(c: u32) -> Color {
    Color::srgb_u8((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// The chapter's accent: its client's colour, the primary for the whole town.
pub fn chapter_accent(ch: &Chapter, pal: &oo_kit::theme::Palette) -> Color {
    ch.client.map(|c| hex(c.color())).unwrap_or(pal.primary)
}

/// The chapter the hub shows: the one picked, else where ORI points next.
fn shown_chapter(v: &View) -> u8 {
    v.s.chapter
        .or_else(|| next_task(&v.game.save).map(|i| content::tasks()[i].chapter))
        .unwrap_or(1)
}

/// A chapter opens with its first implemented level.
fn chapter_open(n: u8, save: &SaveData) -> bool {
    content::in_chapter(n)
        .next()
        .is_some_and(|t| save.is_unlocked(t))
}

pub fn goal_text(g: Goal, l: Lang) -> String {
    format!("≤ {} {}", g.max, g.metric.label().get(l))
}

/// What a metric measures, for goal tooltips.
pub fn metric_tip(m: Metric, l: Lang) -> &'static str {
    match m {
        Metric::Cycles => t(
            l,
            "Warp-instructions issued, worst test case.",
            "发出的 warp 指令数，取最差测试用例。",
        ),
        Metric::Size => t(
            l,
            "Instructions in the program, or compute nodes in a circuit.",
            "程序的指令条数，或电路中的计算节点数。",
        ),
        Metric::Loads => t(
            l,
            "Lane reads from ROM, worst test case.",
            "车道从 ROM 读取的次数，取最差测试用例。",
        ),
        Metric::Stores => t(
            l,
            "Lane writes to the output, worst test case.",
            "车道写入输出的次数，取最差测试用例。",
        ),
        Metric::Regs => t(
            l,
            "Registers the kernel touches: the highest register used, plus one.",
            "内核用到的寄存器数：用到的最大编号加一。",
        ),
        Metric::Txns => t(
            l,
            "Global read transactions: cache lines touched by each warp read, worst test case.",
            "全局读事务数：每次 warp 读触及的缓存行数之和，取最差测试用例。",
        ),
        Metric::Misses => t(
            l,
            "Reads that missed L1 and went to memory, worst test case.",
            "未命中 L1、要去全局内存取数的读取次数，取最差测试用例。",
        ),
        Metric::Smem => t(
            l,
            "Shared-memory words the kernel uses, worst test case.",
            "内核用到的共享内存字数，取最差测试用例。",
        ),
        Metric::Atoms => t(
            l,
            "Lanes that performed a global atomic add, worst test case.",
            "执行全局原子加的车道次数，取最差测试用例。",
        ),
        Metric::LinkWords => t(
            l,
            "Words sent over all links, worst test case.",
            "所有链路上发送的字数，取最差测试用例。",
        ),
        Metric::HotLink => t(
            l,
            "Words carried by the busiest single link, worst test case.",
            "最忙的那一条链路传过的字数，取最差测试用例。",
        ),
        Metric::Idle => t(
            l,
            "Share of the run the idlest SM issued nothing, in percent, worst test case.",
            "最空闲的那个 SM 在整个运行中没有发射指令的占比（百分数），取最差测试用例。",
        ),
        Metric::Bubble => t(
            l,
            "Share of all SM-cycles that issued nothing, in percent, worst test case.",
            "所有 SM 的周期里没有发射指令的占比（百分数），取最差测试用例。",
        ),
        Metric::Err => t(
            l,
            "Largest gap between an output cell and the exact answer, worst test case.",
            "输出格与精确答案的最大差距，取最差测试用例。",
        ),
    }
}

/// The first unlocked task without a solve: where ORI points next.
pub fn next_task(save: &SaveData) -> Option<usize> {
    content::tasks()
        .iter()
        .position(|t| save.is_unlocked(t) && !save.campaign.is_solved(t.id))
}

fn requirement_names(task: &Task, save: &SaveData, l: Lang) -> String {
    task.requires
        .iter()
        .filter(|r| !save.campaign.is_solved(r))
        .filter_map(|r| content::task(r))
        .map(|t| t.title.get(l))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// The colour that codes a level kind: puzzle teal, fault warning, commission violet.
fn kind_color(k: LevelKind, pal: &oo_kit::theme::Palette) -> Color {
    match k {
        LevelKind::Puzzle => pal.primary,
        LevelKind::Fault => pal.warn,
        LevelKind::Commission => pal.tertiary,
    }
}

pub fn draw_hub(ui: &mut Ui, v: &View) {
    let l = v.lang;
    let pal = ui.pal;
    let save = &v.game.save;
    ui.text_spaced(
        t(l, "THE DEEP WELL", "深井"),
        Vec2::new(-600.0, 318.0),
        24.0,
        pal.text,
        Anchor::CENTER_LEFT,
        3.0,
    );
    let all = content::tasks().len() as u32;
    ui.pill(
        Vec2::new(226.0, 318.0),
        170.0,
        t(l, "RESTORED", "已复刻"),
        format!("{} / {}", save.campaign.solved_count(), all),
        pal.good,
    );
    ui.pill(
        Vec2::new(400.0, 318.0),
        150.0,
        t(l, "STARS", "星"),
        format!("{} / {}", save.campaign.total_stars(), all * 3),
        pal.secondary,
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(520.0, 318.0),
            Vec2::new(76.0, 34.0),
            l.switch_label(),
            Act::ToggleLang,
        )
        .font(14.0),
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(592.0, 318.0),
            Vec2::new(56.0, 34.0),
            "≡",
            Act::Goto(Screen::Settings),
        )
        .font(16.0)
        .tip(t(l, "Settings", "设置").to_string()),
    );
    let next = next_task(save);
    let tasks = content::tasks();
    let shown = shown_chapter(v);
    draw_rail(ui, shown, save, l);
    // Right: the chapter's machine, its client and its eight levels.
    let ch = content::chapter(shown);
    let accent = chapter_accent(ch, &pal);
    let center = Vec2::new(160.0, -18.0);
    let size = Vec2::new(900.0, 560.0);
    ui.card(center, size, accent);
    let top = center.y + size.y / 2.0;
    let left = center.x - size.x / 2.0 + 28.0;
    ui.caption(
        format!("{} · {}", chapter_word(ch.number, l), ch.machine.get(l)),
        Vec2::new(left, top - 26.0),
        accent,
        Anchor::CENTER_LEFT,
    );
    ui.text(
        ch.title.get(l),
        Vec2::new(left, top - 54.0),
        24.0,
        pal.text,
        Anchor::CENTER_LEFT,
    );
    ui.dot(Vec2::new(left + 6.0, top - 86.0), 10.0, accent, 13.0);
    let client = match ch.client {
        Some(c) => format!("{} · {}", c.name().get(l), c.role().get(l)),
        None => ch.client_name().get(l).to_string(),
    };
    ui.text(
        client,
        Vec2::new(left + 22.0, top - 86.0),
        15.0,
        accent,
        Anchor::CENTER_LEFT,
    );
    ui.text(
        ch.tagline.get(l),
        Vec2::new(left, top - 112.0),
        14.0,
        pal.text_dim,
        Anchor::CENTER_LEFT,
    );
    if let Some(cat) = ch.client {
        let at = Vec2::new(center.x + size.x / 2.0 - 86.0, top - 74.0);
        ui.well(at, Vec2::splat(104.0));
        oo_kit::portrait::spawn(ui, cat, at, 100.0, 20.0);
    }
    let row_w = size.x - 40.0;
    for slot in 1..=LEVELS_PER_CHAPTER {
        let y = top - 160.0 - (slot - 1) as f32 * 52.0;
        let found = tasks
            .iter()
            .enumerate()
            .find(|(_, t)| t.chapter == ch.number && t.slot == slot);
        let kind = kind_of_slot(slot - 1);
        let code = format!("{}-{}", ch.number, slot);
        let Some((i, task)) = found else {
            ui.well(Vec2::new(center.x, y), Vec2::new(row_w, 44.0));
            ui.rect(
                Vec2::new(left - 12.0, y),
                Vec2::new(3.0, 22.0),
                mix(kind_color(kind, &pal), pal.bg, 0.7),
                13.0,
            );
            ui.text(
                code,
                Vec2::new(left + 6.0, y),
                14.0,
                pal.text_faint,
                Anchor::CENTER_LEFT,
            );
            ui.caption(
                format!(
                    "{} · {}",
                    kind_label(kind, l),
                    t(l, "IN RESTORATION", "修复中")
                ),
                Vec2::new(left + 66.0, y),
                pal.text_faint,
                Anchor::CENTER_LEFT,
            );
            continue;
        };
        let open = save.is_unlocked(task);
        let tone = if Some(i) == next {
            Tone::Primary
        } else {
            Tone::Secondary
        };
        let mut b = ButtonSpec::new(
            Vec2::new(center.x, y),
            Vec2::new(row_w, 44.0),
            "",
            Act::Open(i),
        )
        .tone(tone)
        .enabled(open);
        if !open {
            b = b.tip(format!(
                "{} {}",
                t(l, "Requires", "需要先完成"),
                requirement_names(task, save, l)
            ));
        }
        ui.button(b);
        let stripe = kind_color(kind, &pal);
        ui.rect(
            Vec2::new(left - 12.0, y),
            Vec2::new(3.0, 22.0),
            if open {
                stripe
            } else {
                mix(stripe, pal.bg, 0.7)
            },
            13.0,
        );
        let dim = if open { pal.text_dim } else { pal.text_faint };
        ui.text(
            code,
            Vec2::new(left + 6.0, y),
            14.0,
            dim,
            Anchor::CENTER_LEFT,
        );
        ui.text(
            task.title.get(l),
            Vec2::new(left + 66.0, y + 8.0),
            16.0,
            if open { pal.text } else { pal.text_faint },
            Anchor::CENTER_LEFT,
        );
        let sub = if open {
            kind_label(kind, l)
        } else {
            t(l, "LOCKED", "未开放")
        };
        ui.caption(
            sub,
            Vec2::new(left + 66.0, y - 11.0),
            if open { stripe } else { pal.text_faint },
            Anchor::CENTER_LEFT,
        );
        if open {
            ui.stars(
                Vec2::new(center.x + row_w / 2.0 - 52.0, y),
                save.campaign.stars(task.id),
                9.0,
            );
        }
    }
    let line = match next {
        Some(i) => format!(
            "ORI · {} “{}”",
            t(l, "Next on the list:", "下一关："),
            tasks[i].title.get(l)
        ),
        None => t(
            l,
            "ORI · Every machine runs again. Go back and chase the stars you missed.",
            "ORI · 每台机器都重新运转了。回去把漏掉的星补上吧。",
        )
        .to_string(),
    };
    ui.text(
        line,
        Vec2::new(-600.0, -330.0),
        14.0,
        pal.text_dim,
        Anchor::CENTER_LEFT,
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(560.0, -330.0),
            Vec2::new(120.0, 34.0),
            t(l, "Title", "标题"),
            Act::Back,
        )
        .tone(Tone::Ghost)
        .font(14.0),
    );
}

/// `s` cut to at most `n` characters, with an ellipsis if it was longer.
fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// The hub's left rail: six machines, with the shown machine's chapters
/// unfolded under its header.
fn draw_rail(ui: &mut Ui, shown: u8, save: &SaveData, l: Lang) {
    let pal = ui.pal;
    let (x, w) = (-470.0, 300.0);
    let current = content::machine_of(shown);
    let mut y = 272.0;
    for (mi, m) in content::machines().iter().enumerate() {
        let chapters = (m.first..=m.last).map(content::chapter);
        let built: Vec<&Task> = (m.first..=m.last).flat_map(content::in_chapter).collect();
        let solved = built
            .iter()
            .filter(|t| save.campaign.is_solved(t.id))
            .count();
        let total = (m.last - m.first + 1) as usize * LEVELS_PER_CHAPTER as usize;
        let open = chapter_open(m.first, save);
        let mut b = ButtonSpec::new(
            Vec2::new(x, y),
            Vec2::new(w, 46.0),
            "",
            Act::Chapter(m.first),
        )
        .tone(if mi == current {
            Tone::Selected
        } else {
            Tone::Secondary
        })
        .enabled(open);
        if !open {
            let tip = if built.is_empty() {
                t(
                    l,
                    "This machine is still being restored.",
                    "这台机器还在修复中。",
                )
            } else {
                t(l, "Finish the chapter before it first.", "先完成上一章。")
            };
            b = b.tip(tip.to_string());
        }
        ui.button(b);
        let left = x - w / 2.0;
        let tone = if open { pal.text } else { pal.text_faint };
        ui.text(
            m.name.get(l),
            Vec2::new(left + 16.0, y + 8.0),
            16.0,
            tone,
            Anchor::CENTER_LEFT,
        );
        let range = if m.first == m.last {
            m.first.to_string()
        } else {
            format!("{}–{}", m.first, m.last)
        };
        ui.caption(
            chapter_word_range(&range, l),
            Vec2::new(left + 16.0, y - 11.0),
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
        ui.progress(
            Vec2::new(x + w / 2.0 - 54.0, y - 11.0),
            Vec2::new(76.0, 4.0),
            solved as f32 / total as f32,
            pal.primary,
        );
        ui.caption(
            format!("{solved} / {total}"),
            Vec2::new(x + w / 2.0 - 16.0, y + 9.0),
            pal.text_dim,
            Anchor::CENTER_RIGHT,
        );
        y -= 52.0;
        if mi == current {
            for ch in chapters {
                draw_chapter_row(
                    ui,
                    ch,
                    Vec2::new(x + 12.0, y),
                    w - 24.0,
                    ch.number == shown,
                    save,
                    l,
                );
                y -= 50.0;
            }
            y -= 4.0;
        }
    }
}

/// `CH 1–4`, `第 1–4 章`, `1–4장`.
fn chapter_word_range(range: &str, l: Lang) -> String {
    match l {
        Lang::En => format!("CH {range}"),
        Lang::Zh | Lang::Ja => format!("第 {range} 章"),
        Lang::Ko => format!("{range}장"),
    }
}

/// One chapter under its machine: number, title, client and progress.
fn draw_chapter_row(
    ui: &mut Ui,
    ch: &Chapter,
    at: Vec2,
    w: f32,
    shown: bool,
    save: &SaveData,
    l: Lang,
) {
    let pal = ui.pal;
    let open = chapter_open(ch.number, save);
    let tasks: Vec<&Task> = content::in_chapter(ch.number).collect();
    let solved = tasks
        .iter()
        .filter(|t| save.campaign.is_solved(t.id))
        .count();
    let stars: u32 = tasks.iter().map(|t| save.campaign.stars(t.id) as u32).sum();
    let accent = chapter_accent(ch, &pal);
    let mut b = ButtonSpec::new(at, Vec2::new(w, 44.0), "", Act::Chapter(ch.number))
        .tone(if shown {
            Tone::Selected
        } else {
            Tone::Secondary
        })
        .enabled(open);
    if !open {
        let tip = if tasks.is_empty() {
            t(l, "Not built yet.", "还没造好。")
        } else {
            t(l, "Finish the chapter before it first.", "先完成上一章。")
        };
        b = b.tip(tip.to_string());
    }
    ui.button(b);
    let left = at.x - w / 2.0;
    ui.text(
        ch.number.to_string(),
        Vec2::new(left + 18.0, at.y),
        20.0,
        if open { accent } else { pal.text_faint },
        Anchor::CENTER,
    );
    let (en, other) = (26, 14);
    let title = clip(ch.title.get(l), if l == Lang::En { en } else { other });
    ui.text(
        title,
        Vec2::new(left + 38.0, at.y + 8.0),
        14.0,
        if open { pal.text } else { pal.text_faint },
        Anchor::CENTER_LEFT,
    );
    ui.caption(
        ch.client_name().get(l),
        Vec2::new(left + 38.0, at.y - 11.0),
        if open { accent } else { pal.text_faint },
        Anchor::CENTER_LEFT,
    );
    if open {
        ui.caption(
            format!("★ {stars}"),
            Vec2::new(at.x + w / 2.0 - 12.0, at.y + 9.0),
            pal.secondary,
            Anchor::CENTER_RIGHT,
        );
        ui.progress(
            Vec2::new(at.x + w / 2.0 - 48.0, at.y - 11.0),
            Vec2::new(60.0, 4.0),
            solved as f32 / tasks.len().max(1) as f32,
            accent,
        );
    }
}

/// Goal rows: the hard limit (if any) then the two star goals.
pub fn draw_goals(ui: &mut Ui, task: &Task, at: Vec2, l: Lang, met: Option<[bool; 2]>) {
    let pal = ui.pal;
    let mut y = at.y;
    let mut row = |ui: &mut Ui, label: String, goal: Goal, color: Color, lit: bool| {
        let e = ui.text(
            format!("{label}  {}", goal_text(goal, l)),
            Vec2::new(at.x + 20.0, y),
            15.0,
            if lit { pal.text } else { pal.text_dim },
            Anchor::CENTER_LEFT,
        );
        ui.commands.entity(e).insert((
            Tooltip(metric_tip(goal.metric, l).into()),
            TipArea(Vec2::new(160.0, 10.0)),
        ));
        ui.dot(
            Vec2::new(at.x + 6.0, y),
            8.0,
            if lit { color } else { pal.text_faint },
            13.0,
        );
        y -= 26.0;
    };
    if let Some(g) = task.limit {
        row(ui, t(l, "Must run", "必须达到").into(), g, pal.warn, true);
    }
    for (i, g) in task.goals.into_iter().enumerate() {
        let lit = met.is_none_or(|m| m[i]);
        row(ui, format!("★{}", i + 2), g, pal.secondary, lit);
    }
}

pub fn draw_brief(ui: &mut Ui, v: &View) {
    let l = v.lang;
    let pal = ui.pal;
    let task = v.s.task();
    let save = &v.game.save;
    // Left: the machine.
    let left = Vec2::new(-300.0, 10.0);
    ui.card(left, Vec2::new(600.0, 560.0), pal.primary);
    let x0 = left.x - 270.0;
    ui.caption(
        task_caption(task, l),
        Vec2::new(x0, 262.0),
        pal.primary,
        Anchor::CENTER_LEFT,
    );
    ui.text(
        task.title.get(l),
        Vec2::new(x0, 228.0),
        30.0,
        pal.text,
        Anchor::CENTER_LEFT,
    );
    ui.wrapped(
        task.brief.get(l),
        Vec2::new(x0, 190.0),
        540.0,
        16.0,
        pal.text_dim,
        Anchor::TOP_LEFT,
    );
    ui.caption(
        t(l, "LEVEL STANDARDS", "本关标准"),
        Vec2::new(x0, -110.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    draw_goals(ui, task, Vec2::new(x0, -140.0), l, None);
    let rec = save.campaign.get(task.id);
    if let Some(r) = rec.filter(|r| r.solved) {
        ui.stars(Vec2::new(left.x + 220.0, 228.0), r.stars, 11.0);
    }
    // Right: Predict.
    let right = Vec2::new(330.0, 10.0);
    ui.card(right, Vec2::new(560.0, 560.0), pal.secondary);
    let rx = right.x - 250.0;
    ui.caption(
        t(l, "PREDICT · BEFORE YOU BUILD", "预测 · 先想再做"),
        Vec2::new(rx, 262.0),
        pal.secondary,
        Anchor::CENTER_LEFT,
    );
    ui.wrapped(
        task.predict.question.get(l),
        Vec2::new(rx, 236.0),
        500.0,
        17.0,
        pal.text,
        Anchor::TOP_LEFT,
    );
    let answered = v.s.pick.or(save
        .predicted
        .iter()
        .any(|p| p == task.id)
        .then_some(usize::MAX));
    for (i, opt) in task.predict.options.iter().enumerate() {
        let y = 110.0 - i as f32 * 64.0;
        let mut b = ButtonSpec::new(
            Vec2::new(right.x, y),
            Vec2::new(500.0, 54.0),
            "",
            Act::Predict(i),
        )
        .enabled(answered.is_none());
        if answered.is_some() && i == task.predict.answer {
            b = b.tone(Tone::Selected);
        }
        ui.button(b);
        let mark = match answered {
            Some(_) if i == task.predict.answer => ("✓", pal.good),
            Some(p) if p == i => ("×", pal.bad),
            _ => (["A", "B", "C"][i], pal.text_faint),
        };
        ui.text(
            mark.0,
            Vec2::new(right.x - 228.0, y),
            16.0,
            mark.1,
            Anchor::CENTER,
        );
        let e = ui.wrapped(
            opt.get(l),
            Vec2::new(right.x - 206.0, y),
            440.0,
            15.0,
            pal.text,
            Anchor::CENTER_LEFT,
        );
        ui.commands
            .entity(e)
            .insert(Transform::from_xyz(right.x - 206.0, y, 11.0));
    }
    if let Some(p) = answered {
        let (head, color) = match p {
            usize::MAX => (
                t(l, "You predicted this before.", "你之前预测过这一题。"),
                pal.text_dim,
            ),
            p if p == task.predict.answer => (t(l, "Right.", "答对了。"), pal.good),
            _ => (
                t(
                    l,
                    "Not quite — and that is what the bench is for.",
                    "不太对——这正是动手要验证的。",
                ),
                pal.warn,
            ),
        };
        ui.text(
            head,
            Vec2::new(rx, -100.0),
            16.0,
            color,
            Anchor::CENTER_LEFT,
        );
        ui.wrapped(
            task.predict.why.get(l),
            Vec2::new(rx, -122.0),
            500.0,
            14.0,
            pal.text_dim,
            Anchor::TOP_LEFT,
        );
    } else {
        ui.text(
            t(
                l,
                "Commit to an answer. You will see why when you run it.",
                "先选定一个答案，运行时你会看到原因。",
            ),
            Vec2::new(rx, -100.0),
            14.0,
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
    }
    let start = if rec.is_some_and(|r| r.solved) {
        t(l, "Back to the bench", "回到工作台")
    } else {
        t(l, "To the bench", "去工作台")
    };
    ui.button(
        ButtonSpec::new(
            Vec2::new(right.x + 120.0, -232.0),
            Vec2::new(240.0, 50.0),
            start,
            Act::StartBench,
        )
        .primary()
        .enabled(answered.is_some())
        .font(17.0),
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(right.x - 160.0, -232.0),
            Vec2::new(150.0, 40.0),
            t(l, "Grid Hall", "栅厅"),
            Act::Back,
        )
        .tone(Tone::Ghost),
    );
}
