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
use ooc_core::task::{CHAPTERS, Chapter, Goal, LEVELS_PER_CHAPTER, Metric, Task, kind_of_slot};

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
        Act::Problem(_) => crate::problems::act(cx, a),
        Act::Lab(i) => {
            bench::open_lab(&mut cx.s, &cx.game, i);
            cx.go(Screen::Bench);
        }
        Act::Chapter(n) => {
            cx.s.chapter = Some(n);
            cx.s.ori_menu = false;
            cx.dirty();
        }
        Act::OriMenu => {
            cx.s.ori_menu = !cx.s.ori_menu;
            cx.dirty();
        }
        Act::Opening(_) | Act::Line | Act::SkipOpening => crate::hall::opening_act(cx, a),
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
pub fn shown_chapter(v: &View) -> u8 {
    v.s.chapter
        .or_else(|| next_task(&v.game.save).map(|i| content::tasks()[i].chapter))
        .unwrap_or(1)
}

/// A chapter opens with its first implemented level.
pub fn chapter_open(n: u8, save: &SaveData) -> bool {
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
    // The Grid Hall across the top; the stations pick the chapter.
    crate::hall::draw_hall(ui, v, v.s.opening.is_none());
    if v.s.opening.is_some() {
        crate::hall::draw_opening(ui, v);
        return;
    }
    let next = next_task(save);
    let tasks = content::tasks();
    let shown = shown_chapter(v);
    // Below: the chapter, its client and its eight levels.
    let ch = content::chapter(shown);
    let accent = chapter_accent(ch, &pal);
    let center = Vec2::new(0.0, -140.0);
    let size = Vec2::new(1264.0, 290.0);
    ui.card(center, size, accent);
    let top = center.y + size.y / 2.0;
    let bottom = center.y - size.y / 2.0;
    let left = center.x - size.x / 2.0 + 28.0;
    draw_chapter_info(ui, v, ch, accent, Vec2::new(left, top));
    draw_strip(ui, shown, save, l, Vec2::new(left, bottom + 26.0));
    let split = -186.0;
    ui.rect(
        Vec2::new(split, center.y),
        Vec2::new(1.0, size.y - 36.0),
        pal.line.with_alpha(0.5),
        4.6,
    );
    let (cell_w, cell_h) = (376.0, 54.0);
    for slot in 1..=LEVELS_PER_CHAPTER {
        let col = ((slot - 1) / 4) as f32;
        let row = ((slot - 1) % 4) as f32;
        let at = Vec2::new(28.0 + col * 394.0, top - 44.0 - row * 64.0);
        let found = tasks
            .iter()
            .enumerate()
            .find(|(_, t)| t.chapter == ch.number && t.slot == slot);
        draw_level(ui, v, found, ch.number, slot, at, Vec2::new(cell_w, cell_h), next);
    }
    draw_bottom_row(ui, v, ch, next);
}

/// The chapter's caption, title, client, tagline and portrait, from the
/// card's top-left corner `at`.
fn draw_chapter_info(ui: &mut Ui, v: &View, ch: &Chapter, accent: Color, at: Vec2) {
    let pal = ui.pal;
    let l = v.lang;
    let save = &v.game.save;
    let (left, top) = (at.x, at.y);
    ui.caption(
        format!("{} · {}", chapter_word(ch.number, l), ch.machine.get(l)),
        Vec2::new(left, top - 26.0),
        accent,
        Anchor::CENTER_LEFT,
    );
    // Long titles shrink so they stay clear of the client's portrait.
    let title = ch.title.get(l);
    let em: f32 = title.chars().map(|c| if c.is_ascii() { 0.53 } else { 1.0 }).sum();
    let room = if ch.client.is_some() { 285.0 } else { 520.0 };
    ui.text(
        title,
        Vec2::new(left, top - 54.0),
        (room / em.max(1.0)).clamp(16.0, 22.0),
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
        Vec2::new(left + 20.0, top - 86.0),
        14.0,
        accent,
        Anchor::CENTER_LEFT,
    );
    ui.wrapped(
        ch.tagline.get(l),
        Vec2::new(left, top - 104.0),
        if ch.client.is_some() { 300.0 } else { 400.0 },
        13.0,
        pal.text_dim,
        Anchor::TOP_LEFT,
    );
    if let Some(cat) = ch.client {
        let p = Vec2::new(-246.0, top - 78.0);
        ui.well(p, Vec2::splat(96.0));
        oo_kit::portrait::spawn(ui, cat, p, 92.0, 20.0);
    }
    let built: Vec<&Task> = content::in_chapter(ch.number).collect();
    let solved = built.iter().filter(|t| save.campaign.is_solved(t.id)).count();
    let stars: u32 = built.iter().map(|t| save.campaign.stars(t.id) as u32).sum();
    let y = top - 160.0;
    ui.button(
        ButtonSpec::new(
            Vec2::new(left + 58.0, y),
            Vec2::new(116.0, 28.0),
            t(l, "Opening", "开场对白"),
            Act::Opening(ch.number),
        )
        .tone(Tone::Ghost)
        .font(13.0),
    );
    ui.progress(
        Vec2::new(left + 176.0, y - 1.0),
        Vec2::new(80.0, 4.0),
        solved as f32 / LEVELS_PER_CHAPTER as f32,
        accent,
    );
    ui.caption(
        format!("{solved} / {LEVELS_PER_CHAPTER}   ★ {stars}"),
        Vec2::new(left + 226.0, y),
        pal.text_dim,
        Anchor::CENTER_LEFT,
    );
    crate::problems::draw_hub_row(ui, v, ch.number, accent, Vec2::new(left, top - 188.0));
}

/// All sixteen chapters as a strip of small tabs, solved ones marked.
fn draw_strip(ui: &mut Ui, shown: u8, save: &SaveData, l: Lang, at: Vec2) {
    let pal = ui.pal;
    let (w, pitch) = (22.0, 25.0);
    for n in 1..=CHAPTERS {
        let p = Vec2::new(at.x + w / 2.0 + (n - 1) as f32 * pitch, at.y);
        let open = chapter_open(n, save);
        let mut b = ButtonSpec::new(p, Vec2::new(w, 24.0), n.to_string(), Act::Chapter(n))
            .tone(if n == shown { Tone::Selected } else { Tone::Ghost })
            .font(12.0)
            .enabled(open);
        if !open {
            b = b.tip(t(l, "Finish the chapter before it first.", "先完成上一章。"));
        }
        ui.button(b);
        if crate::notes::chapter_done(n, save) {
            ui.rect(p - Vec2::new(0.0, 10.0), Vec2::new(10.0, 2.0), pal.primary, 11.5);
        }
    }
}

/// One level of the chapter as a cell of the 2 × 4 grid.
#[allow(clippy::too_many_arguments)]
fn draw_level(
    ui: &mut Ui,
    v: &View,
    found: Option<(usize, &Task)>,
    chapter: u8,
    slot: u8,
    at: Vec2,
    size: Vec2,
    next: Option<usize>,
) {
    let pal = ui.pal;
    let l = v.lang;
    let save = &v.game.save;
    let kind = kind_of_slot(slot - 1);
    let code = format!("{chapter}-{slot}");
    let left = at.x - size.x / 2.0;
    let stripe = kind_color(kind, &pal);
    let Some((i, task)) = found else {
        ui.well(at, size);
        ui.rect(Vec2::new(left + 8.0, at.y), Vec2::new(3.0, 22.0), mix(stripe, pal.bg, 0.7), 13.0);
        ui.text(code, Vec2::new(left + 20.0, at.y), 14.0, pal.text_faint, Anchor::CENTER_LEFT);
        ui.caption(
            format!("{} · {}", kind_label(kind, l), t(l, "IN RESTORATION", "修复中")),
            Vec2::new(left + 66.0, at.y),
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
        return;
    };
    let open = save.is_unlocked(task);
    let tone = if Some(i) == next { Tone::Primary } else { Tone::Secondary };
    let mut b = ButtonSpec::new(at, size, "", Act::Open(i)).tone(tone).enabled(open);
    if !open {
        b = b.tip(format!(
            "{} {}",
            t(l, "Requires", "需要先完成"),
            requirement_names(task, save, l)
        ));
    }
    ui.button(b);
    ui.rect(
        Vec2::new(left + 8.0, at.y),
        Vec2::new(3.0, 22.0),
        if open { stripe } else { mix(stripe, pal.bg, 0.7) },
        13.0,
    );
    let dim = if open { pal.text_dim } else { pal.text_faint };
    ui.text(code, Vec2::new(left + 20.0, at.y), 14.0, dim, Anchor::CENTER_LEFT);
    let title = clip(task.title.get(l), if l == Lang::En { 30 } else { 15 });
    ui.text(
        title,
        Vec2::new(left + 66.0, at.y + 8.0),
        15.0,
        if open { pal.text } else { pal.text_faint },
        Anchor::CENTER_LEFT,
    );
    let sub = if open { kind_label(kind, l) } else { t(l, "LOCKED", "未开放") };
    ui.caption(
        sub,
        Vec2::new(left + 66.0, at.y - 11.0),
        if open { stripe } else { pal.text_faint },
        Anchor::CENTER_LEFT,
    );
    if open {
        ui.stars(
            Vec2::new(at.x + size.x / 2.0 - 34.0, at.y),
            save.campaign.stars(task.id),
            8.0,
        );
    }
}

/// ORI's line and the hub's buttons. The gap between Bai Yan's notes and
/// Title is kept for one more button.
fn draw_bottom_row(ui: &mut Ui, v: &View, ch: &Chapter, next: Option<usize>) {
    let pal = ui.pal;
    let l = v.lang;
    let save = &v.game.save;
    let tasks = content::tasks();
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
    ui.text(line, Vec2::new(-620.0, -330.0), 14.0, pal.text_dim, Anchor::CENTER_LEFT);
    // The free bench runs the newest machine solved in this chapter.
    let lab = tasks
        .iter()
        .enumerate()
        .rev()
        .find(|(_, t)| t.chapter == ch.number && save.campaign.is_solved(t.id))
        .map(|(i, _)| i);
    let mut b = ButtonSpec::new(
        Vec2::new(130.0, -330.0),
        Vec2::new(150.0, 34.0),
        t(l, "Free bench", "自由机台"),
        Act::Lab(lab.unwrap_or(0)),
    )
    .tone(Tone::Secondary)
    .font(14.0)
    .enabled(lab.is_some())
    .tip(t(
        l,
        "This chapter's machine with nothing to match: every op you have unlocked, the river and the Analyzer. Never scored.",
        "这一章的机器，没有答案要对：已解锁的指令、Warp River 和分析器全开。不计分。",
    ));
    if lab.is_none() {
        b = b.tip(t(l, "Solve a level in this chapter first.", "先在这一章通过一关。"));
    }
    ui.button(b);
    ui.button(
        ButtonSpec::new(
            Vec2::new(447.0, -330.0),
            Vec2::new(110.0, 34.0),
            t(l, "Manual", "手册"),
            Act::Goto(Screen::Manual),
        )
        .tone(Tone::Secondary)
        .font(14.0)
        .tip(t(
            l,
            "ORI's handbook: every op, machine and tool, and the concept cards you have earned.",
            "ORI 的手册：每条指令、每台机器、每个工具，还有你拿到的洞见卡。",
        )),
    );
    let ending = crate::notes::ending_open(save) && save.ending.is_none();
    let mut b = ButtonSpec::new(
        Vec2::new(290.0, -330.0),
        Vec2::new(150.0, 34.0),
        t(l, "Bai Yan's notes", "白砚译注"),
        Act::Goto(Screen::Notes),
    )
    .font(14.0);
    b = if ending { b.primary() } else { b.tone(Tone::Secondary) };
    ui.button(b);
    ui.button(
        ButtonSpec::new(
            Vec2::new(580.0, -330.0),
            Vec2::new(100.0, 34.0),
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
