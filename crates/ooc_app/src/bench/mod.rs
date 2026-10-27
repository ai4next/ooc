//! The workbench: edit, run, watch, and get judged.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_base::HintTier;
use oo_kit::audio::Cue;
use oo_kit::ui::{Actions, ButtonSpec, Tone, Ui};
use ooc_core::content;
use ooc_core::grade::{BuildError, Report, Verdict, grade};
use ooc_core::save::SaveData;
use ooc_core::task::{Editor, Solution, View as TaskView};

use crate::state::{Act, Game, Play, Screen, Sel, Session, Tab, t};
use crate::{Ctx, View, code_ed, graph_ed, hub, river};

/// Starts a fresh visit to task `i` with the player's saved work.
pub fn open(s: &mut Session, game: &Game, i: usize) {
    let task = &content::tasks()[i];
    *s = Session {
        task: i,
        work: Some(game.save.work_for(task)),
        cases: (task.cases)(),
        settings_return: s.settings_return,
        play: Play {
            speed: 1,
            ..default()
        },
        ..default()
    };
}

pub fn leave(cx: &mut Ctx) {
    save_work(cx);
    cx.go(Screen::Hub);
}

fn save_work(cx: &mut Ctx) {
    let id = cx.s.task().id;
    if let Some(w) = cx.s.work.clone() {
        cx.game.save.work.insert(id.to_string(), w);
        cx.game.touch();
    }
}

pub fn build_error_text(e: &BuildError, l: oo_base::Lang) -> String {
    use ooc_core::asm::AsmErrorKind as K;
    use ooc_core::graph::GraphError as G;
    match e {
        BuildError::Asm(a) => match &a.kind {
            K::Shape(sh) => format!(
                "{} {}: {:?}",
                t(l, "Block structure broken at line", "块结构在第几行出错："),
                a.line + 1,
                sh
            ),
            k => format!("{} {}: {:?}", t(l, "Line", "第"), a.line + 1, k),
        },
        BuildError::Graph(G::Cycle(_)) => {
            t(l, "The circuit feeds back into itself.", "电路形成了回路。").into()
        }
        BuildError::Graph(G::TooManyNodes) => t(
            l,
            "Too many parts: the board has 16 registers.",
            "部件太多：电路板只有 16 个寄存器。",
        )
        .into(),
        BuildError::Graph(G::Missing) => {
            t(l, "The circuit has no output.", "电路没有输出。").into()
        }
        BuildError::LockedOp(op) => format!(
            "{} {}",
            op.mnemonic(),
            t(l, "is not available in this level.", "在这一关里不可用。")
        ),
        BuildError::LockedNode(n) => format!(
            "{} {}",
            n.label(),
            t(l, "is not available in this level.", "在这一关里不可用。")
        ),
        BuildError::TooLong(n) => format!(
            "{} {n}",
            t(
                l,
                "Program memory is full. This machine holds at most",
                "程序存储已满。这台机器最多容纳指令条数："
            )
        ),
        BuildError::TooManyRegs(n) => format!(
            "{} {n}",
            t(
                l,
                "Too many registers per thread. This machine gives at most",
                "每线程寄存器太多。这台机器最多给："
            )
        ),
        BuildError::WrongEditor => t(
            l,
            "This work belongs to another editor.",
            "这份作品属于另一种编辑器。",
        )
        .into(),
    }
}

/// Builds and grades the work, then starts the replay of the first failing
/// case (or the selected one when everything passes).
pub fn run(cx: &mut Ctx) {
    let task = cx.s.task();
    let l = cx.lang();
    cx.s.invalidate();
    cx.s.sel = match cx.s.sel {
        Sel::Line(_) | Sel::Operand(..) | Sel::Node(_) | Sel::Port(..) => cx.s.sel,
        Sel::None => Sel::None,
    };
    match grade(task, cx.s.work()) {
        Err(e) => {
            cx.s.error = Some(build_error_text(&e, l));
            cx.cues.play(Cue::Error);
        }
        Ok(rep) => {
            cx.game.save.campaign.note_attempt(task.id);
            cx.game.touch();
            // Hidden cases are never replayed: stay on a public one.
            cx.s.case = match rep.verdict {
                Verdict::Wrong { case, .. } | Verdict::Timeout { case } if !rep.is_hidden(case) => {
                    case
                }
                _ => cx.s.case.min(rep.public - 1),
            };
            cx.s.report = Some(rep);
            restart(&mut cx.s, true);
        }
    }
    cx.dirty();
}

/// Rewinds the replay of the selected case.
pub fn restart(s: &mut Session, playing: bool) {
    let Some(rep) = &s.report else { return };
    let run = &rep.runs[s.case.min(rep.runs.len() - 1)];
    let mut buf = run.init.clone();
    buf.resize(s.task().out_len, 0);
    s.play.buf = buf;
    s.play.step = 0;
    s.play.acc = 0.0;
    s.play.playing = playing;
    s.generation += 1;
}

/// Applies replay steps up to `to` (clamped to the trace length).
pub fn advance(s: &mut Session, to: usize) {
    let Some(rep) = &s.report else { return };
    let trace = &rep.runs[s.case].run.trace;
    let to = to.min(trace.len());
    for step in &trace[s.play.step.min(to)..to] {
        for &(addr, v) in &step.writes {
            if let Some(c) = s.play.buf.get_mut(addr as usize) {
                *c = v;
            }
        }
    }
    s.play.step = s.play.step.max(to);
}

pub fn trace_len(s: &Session) -> usize {
    s.report
        .as_ref()
        .map_or(0, |r| r.runs[s.case].run.trace.len())
}

/// Shows the verdict; scores the report the first time.
pub fn reveal(cx: &mut Ctx) {
    advance(&mut cx.s, usize::MAX);
    cx.s.play.playing = false;
    cx.s.play.revealed = true;
    if !cx.s.judged {
        cx.s.judged = true;
        judge(cx);
    }
    cx.dirty();
}

fn judge(cx: &mut Ctx) {
    let task = cx.s.task();
    let l = cx.lang();
    let Some(rep) = &cx.s.report else { return };
    let Some(outcome) = rep.outcome(task) else {
        let had =
            cx.s.hints
                .next()
                .is_some_and(|n| cx.s.hints.is_available(n));
        cx.s.hints.fail();
        let has =
            cx.s.hints
                .next()
                .is_some_and(|n| cx.s.hints.is_available(n));
        if has && !had {
            cx.toasts.push(
                t(l, "ORI has a hint", "ORI 有提示"),
                t(
                    l,
                    "Open the Hints tab when you want it.",
                    "需要时打开“提示”标签。",
                ),
            );
        }
        cx.cues.play(Cue::Error);
        return;
    };
    save_work(cx);
    if !cx.game.save.has_evidence(task)
        && let Some(clue) = task.clue
    {
        cx.toasts.push(
            t(l, "Fixed — now show ORI why", "修好了——再给 ORI 指出原因"),
            format!(
                "{} {}",
                t(
                    l,
                    "Reset to the starter, run it, and point on the Warp River at",
                    "还原初始电路并运行，在 Warp River 上指出"
                ),
                clue.what.get(l)
            ),
        );
        cx.cues.play(Cue::Error);
        return;
    }
    let save = &mut cx.game.save;
    let before: Vec<bool> = content::tasks()
        .iter()
        .map(|t| save.is_unlocked(t))
        .collect();
    let assisted = cx.s.hints.used_walkthrough();
    cx.s.improved = save.campaign.record_solve(task.id, outcome, assisted);
    SaveData::note_once(&mut save.insights, task.id);
    cx.s.unlocked = content::tasks()
        .iter()
        .enumerate()
        .filter(|(i, t)| !before[*i] && save.is_unlocked(t))
        .map(|(i, _)| i)
        .collect();
    for &i in &cx.s.unlocked {
        let name = content::tasks()[i].title.get(l);
        cx.toasts.push(t(l, "Level unlocked", "新关卡开放"), name);
    }
    cx.cues.play(Cue::Success);
    if cx.s.improved {
        cx.cues.play(Cue::Star);
    }
    if !cx.s.unlocked.is_empty() {
        cx.cues.play(Cue::Unlock);
    }
    cx.game.touch();
}

/// Where a fault's evidence sits in the selected case's replay, if that
/// case failed.
pub fn clue_at(s: &Session) -> Option<oo_base::Evidence> {
    let clue = s.task().clue?;
    let rep = s.report.as_ref()?;
    let run = rep.runs.get(s.case)?;
    if !run.fails(s.task()) {
        return None;
    }
    (clue.find)(&rep.program, run)
}

/// The player's "it broke here" at trace index `i`. A miss is not punished:
/// the replay jumps to the right moment and ORI says what to look for.
fn point(cx: &mut Ctx, i: usize) {
    let task = cx.s.task();
    let l = cx.lang();
    let Some(clue) = task.clue else { return };
    let Some(ev) = clue_at(&cx.s) else { return };
    let warp =
        cx.s.report
            .as_ref()
            .and_then(|r| r.runs[cx.s.case].run.trace.get(i).map(|st| st.warp as u32));
    let hit = ev.accepts(i as u32, warp);
    cx.s.pointed = Some((i, hit));
    if hit {
        SaveData::note_once(&mut cx.game.save.evidence, task.id);
        cx.game.touch();
        cx.toasts
            .push(t(l, "Evidence found", "找到证据了"), clue.what.get(l));
        cx.cues.play(Cue::Success);
    } else {
        cx.toasts.push(
            t(l, "Not that moment", "不是这一拍"),
            t(
                l,
                "The replay now stops at the right one.",
                "回放已经停在正确的那一拍。",
            ),
        );
        cx.cues.play(Cue::Error);
    }
    // Either way, the replay settles on the evidence.
    seek(&mut cx.s, ev.center() as usize + 1);
}

/// Moves the replay so that `to` steps have been applied.
pub fn seek(s: &mut Session, to: usize) {
    let revealed = s.play.revealed;
    restart(s, false);
    advance(s, to);
    s.play.revealed = revealed;
}

pub fn act(cx: &mut Ctx, a: Act) {
    let l = cx.lang();
    match a {
        Act::Run => run(cx),
        Act::Pause => {
            if cx.s.report.is_some() && !cx.s.play.revealed {
                cx.s.play.playing = !cx.s.play.playing;
            } else if cx.s.report.is_some() {
                restart(&mut cx.s, true);
                cx.s.play.revealed = false;
            }
        }
        Act::Step => {
            if cx.s.report.is_some() {
                cx.s.play.playing = false;
                let n = cx.s.play.step + 1;
                advance(&mut cx.s, n);
                if cx.s.play.step >= trace_len(&cx.s) {
                    reveal(cx);
                }
            }
        }
        Act::Rewind => {
            restart(&mut cx.s, false);
            cx.s.play.revealed = cx.s.judged;
        }
        Act::Skip => {
            if cx.s.report.is_some() {
                reveal(cx);
            }
        }
        Act::Speed => cx.s.play.speed = (cx.s.play.speed + 1) % crate::state::SPEEDS.len(),
        Act::Case(i) => {
            cx.s.case = i;
            if cx.s.report.is_some() {
                restart(&mut cx.s, false);
                advance(&mut cx.s, usize::MAX);
            } else {
                cx.s.generation += 1;
            }
            cx.s.probe = None;
        }
        Act::Tab(tab) => cx.s.tab = tab,
        Act::Hint => {
            if let Some(tier) = cx.s.hints.reveal_next() {
                let id = cx.s.task().id;
                cx.game.save.campaign.note_hint(id, tier as u8);
                cx.game.touch();
                cx.s.tab = Tab::Hints;
                if tier == HintTier::Walkthrough {
                    cx.toasts.push(
                        t(l, "Walkthrough opened", "已打开完整解法"),
                        t(
                            l,
                            "This level can now earn at most two stars.",
                            "本关最多只能获得两颗星。",
                        ),
                    );
                }
            }
        }
        Act::Undo => {
            if let Some(w) = cx.s.undo.pop() {
                cx.s.work = Some(w);
                cx.s.invalidate();
                cx.s.sel = Sel::None;
            }
        }
        Act::Restore => {
            let starter = cx.s.task().starter.solution();
            *cx.s.edit() = starter;
            cx.s.sel = Sel::None;
            cx.s.scroll = 0;
        }
        Act::ClearProbe => cx.s.probe = None,
        Act::Point => {
            let i = cx.s.play.step.saturating_sub(1);
            point(cx, i);
        }
        Act::Seek(to) => {
            if cx.s.report.is_some() {
                let len = trace_len(&cx.s).max(1);
                cx.s.play.playing = false;
                seek(&mut cx.s, to.clamp(1, len));
            }
        }
        Act::Next => {
            if cx.s.report.as_ref().is_some_and(Report::passed)
                && cx.game.save.has_evidence(cx.s.task())
            {
                cx.go(Screen::Results);
            }
        }
        a => match cx.s.task().editor {
            Editor::Graph(_) => graph_ed::act(cx, a),
            Editor::Code(_) => code_ed::act(cx, a),
        },
    }
    cx.dirty();
}

pub fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut actions: ResMut<Actions<Act>>,
    mut cx: Ctx,
    scroll: Res<bevy::input::mouse::AccumulatedMouseScroll>,
) {
    let ctrl = keys.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);
    let alt = keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
    let mut push = |a| actions.push(a);
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::F5) {
        push(Act::Run);
    }
    if keys.just_pressed(KeyCode::Space) {
        push(Act::Pause);
    }
    if ctrl && keys.just_pressed(KeyCode::KeyZ) {
        push(Act::Undo);
    }
    if keys.just_pressed(KeyCode::Period) || keys.just_pressed(KeyCode::ArrowRight) {
        push(Act::Step);
    }
    if (keys.just_pressed(KeyCode::Comma) || keys.just_pressed(KeyCode::ArrowLeft))
        && cx.s.report.is_some()
    {
        push(Act::Seek(cx.s.play.step.saturating_sub(1)));
    }
    if keys.just_pressed(KeyCode::Delete) || (keys.just_pressed(KeyCode::Backspace) && !cx.s.typing)
    {
        push(Act::DeleteSel);
    }
    for (k, up) in [(KeyCode::ArrowUp, true), (KeyCode::ArrowDown, false)] {
        if keys.just_pressed(k) {
            push(if alt {
                Act::MoveLine(up)
            } else {
                Act::Scroll(if up { -1 } else { 1 })
            });
        }
    }
    let wheel = scroll.delta.y;
    if wheel.abs() > 0.5 {
        push(Act::Scroll(if wheel > 0.0 { -1 } else { 1 }));
    }
    // Typing a number edits the selected constant.
    let digits = [
        KeyCode::Digit0,
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    let Some(cur) = selected_number(&cx.s) else {
        cx.s.typing = false;
        return;
    };
    let mut next = None;
    for (d, k) in digits.into_iter().enumerate() {
        if keys.just_pressed(k) {
            let base = if cx.s.typing { cur } else { 0 };
            let sign = if base < 0 { -1 } else { 1 };
            next = Some(base.saturating_mul(10).saturating_add(sign * d as i32));
        }
    }
    if keys.just_pressed(KeyCode::Minus) {
        next = Some(-cur);
    }
    if cx.s.typing && keys.just_pressed(KeyCode::Backspace) {
        next = Some(cur / 10);
    }
    if let Some(n) = next {
        let n = n.clamp(-9999, 9999);
        cx.s.typing = true;
        push(Act::Bump(n - cur));
    }
}

/// The value of the selected constant, immediate or loop count.
fn selected_number(s: &Session) -> Option<i32> {
    match (s.sel, s.work.as_ref()?) {
        (Sel::Port(n, p), Solution::Graph(g)) => {
            let port = g.nodes.get(n)?.inputs.get(p)?;
            port.from.is_none().then_some(port.value)
        }
        (Sel::Operand(line, k), Solution::Code(_)) => code_ed::number_at(s, line, k),
        _ => None,
    }
}

pub fn tick(time: Res<Time>, mut cx: Ctx) {
    let dt = time.delta_secs();
    let before =
        cx.s.hints
            .next()
            .is_some_and(|n| cx.s.hints.is_available(n));
    let wait = cx.s.hints.wait_for_next().map(|w| w.ceil() as i32);
    cx.s.hints.tick(dt);
    let after =
        cx.s.hints
            .next()
            .is_some_and(|n| cx.s.hints.is_available(n));
    let wait_now = cx.s.hints.wait_for_next().map(|w| w.ceil() as i32);
    if before != after || (cx.s.tab == Tab::Hints && wait != wait_now) {
        cx.dirty();
    }
    if !cx.s.play.playing || cx.s.report.is_none() {
        return;
    }
    let speed = crate::state::SPEEDS[cx.s.play.speed];
    cx.s.play.acc += dt * speed;
    let n = cx.s.play.acc.floor() as usize;
    if n > 0 {
        cx.s.play.acc -= n as f32;
        let to = cx.s.play.step + n;
        advance(&mut cx.s, to);
        // The pipeline view is drawn per step, not updated in place.
        if cx.s.tab == Tab::Pipeline {
            cx.dirty();
        }
    }
    if cx.s.play.step >= trace_len(&cx.s) {
        reveal(&mut cx);
    }
}

mod draw;
mod river_tab;
mod text_tabs;
mod verdict;

pub use draw::draw;
