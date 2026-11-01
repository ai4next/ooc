use super::river_tab::draw_river;
use super::text_tabs::{draw_hints, draw_listing};
use super::verdict::draw_verdict;
use super::*;

pub(super) const LEFT: (Vec2, Vec2) = (Vec2::new(-216.0, 120.0), Vec2::new(832.0, 372.0));
pub(super) const RIGHT: (Vec2, Vec2) = (Vec2::new(421.0, 120.0), Vec2::new(422.0, 372.0));
pub(super) const STRIP: (Vec2, Vec2) = (Vec2::new(0.0, -216.0), Vec2::new(1264.0, 264.0));
pub(super) const TAB_Y: f32 = -106.0;

pub fn draw(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let task = s.task();
    draw_top(ui, v);
    ui.card(LEFT.0, LEFT.1, pal.primary);
    ui.card(RIGHT.0, RIGHT.1, pal.secondary);
    ui.card(STRIP.0, STRIP.1, pal.tertiary);
    crate::blueprint::draw_editor(ui, v);
    river::draw_display(ui, v);
    draw_cases(ui, v);
    draw_transport(ui, v);
    let tabs = bench_tabs(s);
    let (pitch, width) = tab_geometry(tabs.len());
    let x0 = -612.0 + width / 2.0;
    for (i, tab) in tabs.into_iter().enumerate() {
        let label = tab_label(tab, l);
        let p = Vec2::new(x0 + i as f32 * pitch, TAB_Y);
        let mut b = ButtonSpec::new(p, Vec2::new(width, 28.0), label, Act::Tab(tab))
            .font(if tabs_dense(width) { 12.0 } else { 13.0 })
            .selected(s.tab == tab);
        if tab != s.tab {
            b = b.tone(Tone::Ghost);
        }
        ui.button(b);
        if tab == Tab::Hints
            && s.hints
                .next()
                .is_some_and(|n| s.hints.is_available(n) && n != HintTier::Direction)
        {
            ui.dot(p + Vec2::new(width / 2.0 - 8.0, 9.0), 8.0, pal.secondary, 12.0);
        }
    }
    draw_verdict(ui, v);
    let y = LEFT.0.y - LEFT.1.y / 2.0 + 16.0;
    ui.button(
        ButtonSpec::new(
            Vec2::new(122.0, y),
            Vec2::new(52.0, 20.0),
            t(l, "Undo", "撤销"),
            Act::Undo,
        )
        .font(11.0)
        .tone(Tone::Ghost)
        .enabled(!s.undo.is_empty())
        .tip("Ctrl+Z"),
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(178.0, y),
            Vec2::new(52.0, 20.0),
            t(l, "Reset", "还原"),
            Act::Restore,
        )
        .font(11.0)
        .tone(Tone::Ghost)
        .tip(t(
            l,
            "Put the starter back. Undo brings yours back.",
            "恢复初始作品，可以撤销。",
        )),
    );
    match s.tab {
        Tab::River => draw_river(ui, v),
        Tab::Brief if s.open.is_some() => crate::problems::draw_brief(ui, v),
        Tab::Brief => {
            ui.wrapped(
                task.brief.get(l),
                Vec2::new(-612.0, -134.0),
                700.0,
                15.0,
                pal.text_dim,
                Anchor::TOP_LEFT,
            );
            let met = s
                .report
                .as_ref()
                .filter(|r| r.passed())
                .map(|r| r.goals_met);
            if let Some(cat) = content::chapter(task.chapter).client {
                let at = Vec2::new(-562.0, -282.0);
                ui.well(at, Vec2::splat(84.0));
                oo_kit::portrait::spawn(ui, cat, at, 80.0, 20.0);
                ui.caption(
                    t(l, "ASKED BY", "委托人"),
                    Vec2::new(-508.0, -266.0),
                    pal.text_faint,
                    Anchor::CENTER_LEFT,
                );
                ui.text(
                    format!("{} · {}", cat.name().get(l), cat.role().get(l)),
                    Vec2::new(-508.0, -290.0),
                    14.0,
                    hub::chapter_accent(content::chapter(task.chapter), &pal),
                    Anchor::CENTER_LEFT,
                );
            }
            ui.caption(
                t(l, "LEVEL STANDARDS", "本关标准"),
                Vec2::new(150.0, -140.0),
                pal.text_faint,
                Anchor::CENTER_LEFT,
            );
            hub::draw_goals(ui, task, Vec2::new(150.0, -168.0), l, met);
            ui.caption(
                t(l, "YOUR PREDICTION", "你的预测"),
                Vec2::new(420.0, -140.0),
                pal.text_faint,
                Anchor::CENTER_LEFT,
            );
            ui.wrapped(
                task.predict.why.get(l),
                Vec2::new(420.0, -158.0),
                196.0,
                12.0,
                pal.text_dim,
                Anchor::TOP_LEFT,
            );
        }
        Tab::Hints => draw_hints(ui, v),
        Tab::Listing => draw_listing(ui, v),
        Tab::Pipeline => crate::pipe_view::draw(ui, v),
        Tab::Memory => crate::machine::mem::draw(ui, v),
        Tab::Analyzer => crate::machine::analyzer::draw(ui, v),
        Tab::Tensor => crate::machine::tensor::draw(ui, v),
        Tab::Room => crate::machine::room::draw(ui, v),
        Tab::Lab => crate::lab::panel::draw_panel(ui, v),
    }
}

/// The tab row. The free bench has no brief: its panel takes that place;
/// open problems have no hints.
pub(super) fn bench_tabs(s: &Session) -> Vec<Tab> {
    let mut tabs = crate::machine::tabs(s.task());
    if s.lab {
        tabs.iter_mut().filter(|t| **t == Tab::Brief).for_each(|t| *t = Tab::Lab);
    }
    if s.open.is_some() {
        tabs.retain(|t| *t != Tab::Hints);
    }
    tabs
}

/// Pitch and width of the tab buttons: up to nine share the row left of
/// the verdict.
fn tab_geometry(n: usize) -> (f32, f32) {
    if n > 6 { (78.0, 74.0) } else { (102.0, 96.0) }
}

/// Where the verdict line starts: just right of the last tab.
pub(super) fn verdict_x(n: usize) -> f32 {
    let (pitch, width) = tab_geometry(n);
    -612.0 + n.saturating_sub(1) as f32 * pitch + width + 16.0
}

fn tabs_dense(width: f32) -> bool {
    width < 90.0
}

fn tab_label(tab: Tab, l: oo_base::Lang) -> &'static str {
    match tab {
        Tab::River => t(l, "Warp River", "Warp River"),
        Tab::Pipeline => t(l, "Pipeline", "流水线"),
        Tab::Memory => t(l, "Memory", "内存"),
        Tab::Analyzer => t(l, "Analyzer", "分析器"),
        Tab::Tensor => t(l, "Tensor", "张量"),
        Tab::Room => t(l, "Room", "机房"),
        Tab::Brief => t(l, "Brief", "任务"),
        Tab::Hints => t(l, "Hints", "提示"),
        Tab::Listing => t(l, "Listing", "汇编"),
        Tab::Lab => t(l, "Lab", "实验台"),
    }
}

fn draw_top(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let task = s.task();
    let y = 332.0;
    ui.button(
        ButtonSpec::new(
            Vec2::new(-586.0, y),
            Vec2::new(96.0, 28.0),
            t(l, "‹ Grid Hall", "‹ 栅厅"),
            Act::Back,
        )
        .tone(Tone::Ghost)
        .font(13.0),
    );
    ui.caption(
        format!(
            "{} · {} · {}",
            task.number(),
            content::chapter(task.chapter).machine.get(l),
            if s.open.is_some() {
                t(l, "OPEN PROBLEM", "开放题")
            } else {
                hub::kind_label(task.kind(), l)
            }
        ),
        Vec2::new(-520.0, y + 12.0),
        pal.text_dim,
        Anchor::CENTER_LEFT,
    );
    ui.text(
        task.title.get(l),
        Vec2::new(-520.0, y - 9.0),
        17.0,
        pal.text,
        Anchor::CENTER_LEFT,
    );
    let measure = s.report.as_ref().map(|r| r.measure);
    if s.lab {
        crate::lab::panel::draw_kernels(ui, v, y);
        let frame = crate::lab::frame_cycles(s).map(|c| c.to_string());
        ui.pill(
            Vec2::new(274.0, y),
            220.0,
            t(l, "FRAME · CYCLES", "一帧 · 周期"),
            frame.unwrap_or_else(|| "—".into()),
            pal.secondary,
        );
    }
    if let Some(k) = s.open {
        crate::problems::draw_pills(ui, v, k, y);
    }
    let scored = !s.lab && s.open.is_none();
    for (i, g) in task.goals.into_iter().enumerate().filter(|_| scored) {
        let value = measure.map_or("—".into(), |m| m.get(g.metric).to_string());
        let met = measure.is_some_and(|m| m.meets(g)) && s.play.revealed;
        let accent = if met { pal.good } else { pal.text_dim };
        let x = 110.0 + i as f32 * 240.0;
        let label = format!("★{} {}", i + 2, hub::goal_text(g, l));
        ui.pill(Vec2::new(x, y), 232.0, &label, value, accent);
    }
    if let Some(g) = task.limit.filter(|_| scored) {
        let value = measure.map_or("—".into(), |m| m.get(g.metric).to_string());
        ui.pill(
            Vec2::new(-130.0, y),
            232.0,
            &format!("{} {}", t(l, "LIMIT", "上限"), hub::goal_text(g, l)),
            value,
            pal.warn,
        );
    }
    ui.button(
        ButtonSpec::new(
            Vec2::new(520.0, y),
            Vec2::new(72.0, 28.0),
            l.switch_label(),
            Act::ToggleLang,
        )
        .font(13.0),
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(592.0, y),
            Vec2::new(52.0, 28.0),
            "≡",
            Act::Goto(Screen::Settings),
        )
        .font(15.0),
    );
}

/// Case buttons; after the verdict each shows whether it passed.
fn draw_cases(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let s = v.s;
    let d = river::display(s.task().view);
    let n = s.cases.len();
    ui.caption(
        t(v.lang, "CASE", "用例"),
        d.cases,
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let hidden = (s.task().hidden)().len();
    // Many cases shrink to fit the card, leaving room for the hidden count.
    let right = RIGHT.0.x + RIGHT.1.x / 2.0 - 16.0 - if hidden > 0 { 76.0 } else { 0.0 };
    let pitch = ((right - d.cases.x - 44.0) / n.max(1) as f32).clamp(22.0, 34.0);
    for i in 0..n {
        let p = d.cases + Vec2::new(58.0 + i as f32 * pitch, 0.0);
        let on = s.case == i;
        let b = ButtonSpec::new(p, Vec2::new(pitch - 4.0, 24.0), (i + 1).to_string(), Act::Case(i))
            .font(12.0)
            .selected(on)
            .tone(if on { Tone::Selected } else { Tone::Ghost });
        ui.button(b);
        if let Some(r) = s.report.as_ref().filter(|_| s.play.revealed) {
            let ok = r.runs.get(i).is_some_and(|c| c.ok());
            ui.dot(
                p + Vec2::new(pitch / 2.0 - 6.0, 9.0),
                6.0,
                if ok { pal.good } else { pal.bad },
                12.0,
            );
        }
    }
    if hidden > 0 {
        let p = d.cases + Vec2::new(58.0 + n as f32 * pitch - pitch / 2.0 + 8.0, 0.0);
        let state = s
            .report
            .as_ref()
            .filter(|_| s.play.revealed)
            .map(|r| r.runs[r.public..].iter().all(|c| c.ok()));
        let color = match state {
            Some(true) => pal.good,
            Some(false) => pal.bad,
            None => pal.text_faint,
        };
        ui.caption(
            format!("+{hidden} {}", t(v.lang, "HIDDEN", "隐藏")),
            p,
            color,
            Anchor::CENTER_LEFT,
        );
    }
}

/// Run, playback controls, probe readout, undo and restore.
fn draw_transport(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let ran = s.report.is_some();
    let y = -38.0;
    ui.button(
        ButtonSpec::new(
            Vec2::new(282.0, y),
            Vec2::new(118.0, 34.0),
            t(l, "Run", "运行"),
            Act::Run,
        )
        .primary()
        .font(15.0)
        .tip(t(
            l,
            "Build and test every case. Enter / F5",
            "构建并测试所有用例。Enter / F5",
        )),
    );
    let play = if s.play.playing {
        t(l, "Pause", "暂停")
    } else if s.play.revealed {
        t(l, "Replay", "重放")
    } else {
        t(l, "Play", "播放")
    };
    let speed = format!("×{}", crate::state::SPEEDS[s.play.speed] as i32);
    let controls = [
        (play, Act::Pause, t(l, "Space", "空格")),
        (
            t(l, "Step", "单步"),
            Act::Step,
            t(
                l,
                "One warp-instruction. Period key",
                "前进一条 warp 指令。句号键",
            ),
        ),
        (
            t(l, "Rewind", "倒回"),
            Act::Rewind,
            t(l, "Back to the first step", "回到第一步"),
        ),
        (
            t(l, "Skip", "跳过"),
            Act::Skip,
            t(l, "Jump to the verdict", "直接看结果"),
        ),
    ];
    for (i, (label, act, tip)) in controls.into_iter().enumerate() {
        let p = Vec2::new(372.0 + i as f32 * 58.0, y);
        ui.button(
            ButtonSpec::new(p, Vec2::new(54.0, 30.0), label, act)
                .font(12.0)
                .enabled(ran)
                .tip(tip),
        );
    }
    ui.button(
        ButtonSpec::new(
            Vec2::new(604.0, y),
            Vec2::new(46.0, 30.0),
            speed,
            Act::Speed,
        )
        .font(12.0)
        .tone(Tone::Ghost)
        .tip(t(
            l,
            "Playback speed, steps per second",
            "回放速度（每秒步数）",
        )),
    );
    // Hovering names the cell's thread and its last write; a click pins it.
    let shown = s.hover.and_then(|c| river::thread_of(s, c).map(|th| (c, th)));
    if let Some((cell, th)) = shown {
        let mut what = format!(
            "{} {th} · {}",
            t(l, "THREAD", "线程"),
            cell_name(s.task(), cell, l)
        );
        match river::last_write(s, cell) {
            Some((i, st)) => {
                let value = st.writes.iter().rev().find(|w| w.0 as usize == cell).map_or(0, |w| w.1);
                what = format!(
                    "{what} · {} {} · {} {} = {value}",
                    t(l, "STEP", "步"),
                    i + 1,
                    t(l, "LINE", "行"),
                    st.pc + 1
                );
                if s.task().profile.vault {
                    what = format!("{what} · {} {}", t(l, "CYCLE", "周期"), st.cycle);
                }
            }
            None => what = format!("{what} · {}", t(l, "NOT WRITTEN YET", "还没写入")),
        }
        ui.caption(what, Vec2::new(232.0, -4.0), pal.text_dim, Anchor::CENTER_LEFT);
    } else if let Some(th) = s.probe {
        let fb_w = crate::fb::screen_w(s.task());
        let what = if s.task().view == TaskView::Lanes {
            format!("{} {th}", t(l, "PROBE · LANE", "探针 · 车道"))
        } else if s.task().view == TaskView::Cells {
            format!("{} {th}", t(l, "PROBE · THREAD", "探针 · 线程"))
        } else {
            format!(
                "{} {th} · x {} · y {}",
                t(l, "PROBE · THREAD", "探针 · 线程"),
                th % fb_w,
                th / fb_w
            )
        };
        ui.caption(what, Vec2::new(232.0, -4.0), pal.secondary, Anchor::CENTER_LEFT);
    } else if ran {
        let tip = t(
            l,
            "Hover a cell to see who wrote it; click to probe its thread.",
            "悬停格子看是谁写的；点击探测它的线程。",
        );
        ui.caption(tip, Vec2::new(232.0, -4.0), pal.text_faint, Anchor::CENTER_LEFT);
    }
    if s.probe.is_some() {
        ui.button(
            ButtonSpec::new(
                Vec2::new(598.0, -4.0),
                Vec2::new(60.0, 20.0),
                t(l, "Clear", "清除"),
                Act::ClearProbe,
            )
            .font(11.0)
            .tone(Tone::Ghost),
        );
    }
}

/// Where a cell sits, in words: lane, pixel, or front/back buffer pixel.
pub(super) fn cell_name(task: &ooc_core::task::Task, cell: usize, l: oo_base::Lang) -> String {
    let w = crate::fb::screen_w(task);
    let len = ooc_core::isa::FB_LEN;
    match task.view {
        TaskView::Lanes => format!("{} {cell}", t(l, "lane", "车道")),
        TaskView::Cells => {
            let side = crate::fb::grid_side(task);
            format!("{} {cell} ({}, {})", t(l, "cell", "格"), cell % side, cell / side)
        }
        TaskView::Buffers => {
            let which = if cell >= len {
                t(l, "back", "后缓冲")
            } else {
                t(l, "front", "前缓冲")
            };
            let c = cell % len;
            format!("{which} ({}, {})", c % w, c / w)
        }
        TaskView::Screen => format!("{} ({}, {})", t(l, "pixel", "像素"), cell % w, cell / w),
    }
}
