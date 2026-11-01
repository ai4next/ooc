//! The free bench's own controls: save slots, input sets and imports, the
//! 16 / 32 screen, teaching mode, chained kernels and share codes. The
//! bench still edits one kernel at a time through `Session.work`.

use std::path::PathBuf;

use oo_kit::audio::Cue;
use ooc_core::content;
use ooc_core::grade::Report;
use ooc_core::lab::frame::{self, MAX_KERNELS};
use ooc_core::lab::input::{Input, Set, side};
use ooc_core::lab::slot::{self, LabSlot, SLOTS};
use ooc_core::lab::{self as core_lab, code};
use ooc_core::task::Solution;

use crate::Ctx;
use crate::state::{Game, Play, Sel, Session, Tab, t};

pub mod import;
pub mod panel;
pub mod sync;

pub use sync::{LabImages, drop_files, paste_keys, sync_preview, sync_teach};

/// What the free bench holds besides the kernel being edited.
#[derive(Default)]
pub struct LabState {
    /// The save slot this frame goes back to; `None` = not saved.
    pub slot: Option<usize>,
    /// Every kernel of the frame. `kernels[cur]` is stale while the bench
    /// edits it: the live copy is `Session.work`.
    pub kernels: Vec<Solution>,
    pub cur: usize,
    pub input: Input,
    /// Screen width; 0 until the bench opens (the machine's own).
    pub screen: usize,
    pub teach: bool,
    /// One report per kernel after a run. The shown kernel's sits in
    /// `Session.report` and its entry here is `None`.
    pub reports: Vec<Option<Report>>,
    /// Frame cycles: the slowest case's kernels added up.
    pub total: u32,
    /// Per case, each kernel's cycles.
    pub split: Vec<Vec<u32>>,
    /// The last exported code.
    pub code: String,
    /// The import field.
    pub field: String,
    /// Imported code: the author's cycles, and ours once it ran.
    pub pending: Option<u32>,
    pub check: Option<(u32, u32)>,
    /// Files in the import folder.
    pub files: Vec<PathBuf>,
    /// A line for the panel; `true` = something went wrong.
    pub note: Option<(String, bool)>,
    /// Bumped when the input or screen changes (the preview repaints).
    pub input_gen: u64,
}

impl LabState {
    /// Drops the frame's run: an edit makes every later kernel stale too.
    pub fn clear_run(&mut self) {
        self.reports.clear();
        self.split.clear();
        self.total = 0;
    }

    pub fn ran(&self) -> bool {
        !self.reports.is_empty()
    }
}

/// Whether latencies are spelled out as numbers.
pub fn teaching(s: &Session) -> bool {
    s.lab && s.lab_state.teach
}

/// Frame cycles once the whole frame ran.
pub fn frame_cycles(s: &Session) -> Option<u32> {
    (s.lab && s.lab_state.ran()).then_some(s.lab_state.total)
}

/// Called after the bench opened task `i` as a lab: picks the slot that
/// holds this machine (else the first free one) and loads it.
pub fn open(s: &mut Session, game: &Game) {
    let id = s.task().id;
    let slots = &game.save.lab_slots;
    let slot = slots
        .iter()
        .position(|x| x.machine == id && !x.is_empty())
        .or_else(|| (0..SLOTS).find(|&i| slots.get(i).is_none_or(LabSlot::is_empty)));
    let from = slot.and_then(|i| slots.get(i).cloned()).filter(|x| x.machine == id && !x.is_empty());
    load(s, slot, from);
    s.lab_state.files = import::scan();
    s.tab = Tab::Lab;
}

/// Puts `from` (or the bench's current work) on the bench as slot `slot`.
fn load(s: &mut Session, slot: Option<usize>, from: Option<LabSlot>) {
    let native = core_lab::task(s.task).profile.fb_w as usize;
    let st = &mut s.lab_state;
    st.slot = slot;
    st.cur = 0;
    st.clear_run();
    match from {
        Some(x) => {
            st.screen = x.width();
            st.kernels = x.kernels;
            st.input = x.input;
        }
        None => {
            st.kernels = vec![s.work.clone().unwrap_or_else(|| s_starter(s.task))];
            st.input = Input::Level;
            st.screen = native;
        }
    }
    if !core_lab::resizable(s.task) {
        st.screen = native;
    }
    st.kernels.truncate(MAX_KERNELS);
    s.work = Some(st.kernels[0].clone());
    s.undo.clear();
    s.sel = Sel::None;
    s.invalidate();
    refresh_cases(s);
}

fn s_starter(i: usize) -> Solution {
    core_lab::task(i).starter.solution()
}

/// Rebuilds the cases after the input or screen changed.
fn refresh_cases(s: &mut Session) {
    let task = s.task();
    s.cases = s.lab_state.input.cases(task, side(task));
    s.case = s.case.min(s.cases.len().saturating_sub(1));
    s.lab_state.input_gen += 1;
}

/// The frame as it would be saved.
pub fn current(s: &Session, name: String) -> LabSlot {
    let st = &s.lab_state;
    let mut kernels = st.kernels.clone();
    if let (Some(w), Some(k)) = (&s.work, kernels.get_mut(st.cur)) {
        *k = w.clone();
    }
    LabSlot {
        name,
        machine: s.task().id.to_string(),
        kernels,
        input: st.input.clone(),
        screen: st.screen as u8,
    }
}

/// Writes the frame into its slot, and its first kernel into `labs`.
pub fn save(game: &mut Game, s: &Session) {
    slot::ensure(&mut game.save.lab_slots);
    let id = s.task().id;
    let now = current(s, String::new());
    if let Some(k) = now.kernels.first() {
        game.save.labs.insert(id.to_string(), k.clone());
    }
    if let Some(x) = s.lab_state.slot.and_then(|i| game.save.lab_slots.get_mut(i)) {
        let name = if x.machine == id { std::mem::take(&mut x.name) } else { String::new() };
        *x = LabSlot { name, ..now };
    }
    game.touch();
}

/// Shows kernel `k` in the editor and the replay, keeping the frame's run.
pub fn show(s: &mut Session, k: usize) {
    let st = &mut s.lab_state;
    if k == st.cur || k >= st.kernels.len() {
        return;
    }
    if let Some(w) = s.work.take() {
        st.kernels[st.cur] = w;
    }
    if st.ran() {
        st.reports[st.cur] = s.report.take();
        s.report = st.reports[k].take();
    }
    st.cur = k;
    s.work = Some(st.kernels[k].clone());
    s.undo.clear();
    s.sel = Sel::None;
    s.probe = None;
    s.scroll = 0;
    s.generation += 1;
    s.play = Play {
        speed: s.play.speed,
        ..Default::default()
    };
    if s.report.is_some() {
        crate::bench::restart(s, false);
    }
}

/// At the end of a kernel's replay: moves on to the next one, if any.
pub fn next_kernel(s: &mut Session) -> bool {
    let st = &s.lab_state;
    if !s.lab || !st.ran() || s.play.revealed || st.cur + 1 >= st.kernels.len() {
        return false;
    }
    let playing = s.play.playing;
    show(s, s.lab_state.cur + 1);
    crate::bench::restart(s, playing);
    true
}

/// Replays start from the first kernel.
pub fn rewind(s: &mut Session) {
    if s.lab && s.lab_state.ran() {
        show(s, 0);
    }
}

/// Skipping jumps to the last kernel's end.
pub fn to_last(s: &mut Session) {
    if s.lab && s.lab_state.ran() {
        let last = s.lab_state.kernels.len() - 1;
        show(s, last);
    }
}

/// Builds and runs every kernel of the frame, then replays from the first.
pub fn run(cx: &mut Ctx) {
    let l = cx.lang();
    let task = cx.s.task();
    if let Some(w) = cx.s.work.clone() {
        let cur = cx.s.lab_state.cur;
        cx.s.lab_state.kernels[cur] = w;
    }
    cx.s.invalidate();
    let kernels = cx.s.lab_state.kernels.clone();
    match frame::run(task, &kernels, &cx.s.lab_state.input) {
        Err((k, e)) => {
            show(&mut cx.s, k);
            let what = crate::bench::build_error_text(&e, l);
            cx.s.error = Some(if kernels.len() > 1 {
                format!("{} {}: {what}", t(l, "Kernel", "内核"), k + 1)
            } else {
                what
            });
            cx.s.lab_state.pending = None;
            cx.cues.play(Cue::Error);
        }
        Ok(f) => {
            let cases = cx.s.cases.len();
            let st = &mut cx.s.lab_state;
            st.total = f.cycles();
            st.split = (0..cases).map(|c| f.split(c)).collect();
            st.reports = f.kernels.into_iter().map(Some).collect();
            if let Some(author) = st.pending.take() {
                st.check = Some((author, st.total));
                st.note = None;
            }
            let shown = st.cur;
            cx.s.report = st.reports[shown].take();
            if let Some(rep) = &cx.s.report {
                cx.s.case = cx.s.case.min(rep.public.saturating_sub(1));
            }
            show(&mut cx.s, 0);
            crate::bench::restart(&mut cx.s, true);
        }
    }
    cx.dirty();
}

/// A choice of input on the panel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pick {
    Level,
    Set(Set),
    /// The n-th file of the import folder.
    File(usize),
}

/// Buttons of the lab panel and the top bar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LabAct {
    Kernel(usize),
    AddKernel,
    DropKernel,
    Slot(usize),
    ClearSlot(usize),
    Input(Pick),
    Screen(usize),
    Teach,
    Copy,
    Paste,
    Import,
    ClearField,
    Rescan,
}

pub fn act(cx: &mut Ctx, a: LabAct) {
    let l = cx.lang();
    slot::ensure(&mut cx.game.save.lab_slots);
    match a {
        LabAct::Kernel(k) => {
            show(&mut cx.s, k);
            if cx.s.report.is_some() {
                crate::bench::advance(&mut cx.s, usize::MAX);
                cx.s.play.revealed = true;
            }
        }
        LabAct::AddKernel => {
            let n = cx.s.lab_state.kernels.len();
            if n < MAX_KERNELS {
                let fresh = cx.s.task().starter.solution();
                let s = &mut *cx.s;
                if let Some(w) = s.work.take() {
                    s.lab_state.kernels[s.lab_state.cur] = w;
                }
                s.lab_state.kernels.push(fresh.clone());
                s.lab_state.cur = n;
                s.work = Some(fresh);
                fresh_editor(s);
            }
        }
        LabAct::DropKernel => {
            let st = &mut cx.s.lab_state;
            if st.kernels.len() > 1 {
                st.kernels.remove(st.cur);
                st.cur = st.cur.min(st.kernels.len() - 1);
                cx.s.work = Some(st.kernels[st.cur].clone());
                fresh_editor(&mut cx.s);
            }
        }
        LabAct::Slot(i) => pick_slot(cx, i),
        LabAct::ClearSlot(i) => {
            if let Some(x) = cx.game.save.lab_slots.get_mut(i) {
                *x = LabSlot { screen: 16, ..Default::default() };
                cx.game.touch();
            }
            if cx.s.lab_state.slot == Some(i) {
                cx.s.lab_state.slot = None;
                note(cx, t(l, "Slot freed. Click a free slot to keep this frame.", "已清空存档位。点一个空位来保存这一帧。"), false);
            }
        }
        LabAct::Input(p) => {
            let input = match p {
                Pick::Level => Input::Level,
                Pick::Set(s) => Input::Set(s),
                Pick::File(i) => {
                    let Some(path) = cx.s.lab_state.files.get(i).cloned() else { return };
                    match import::load(&path, cx.s.task(), l) {
                        Ok(d) => Input::Data(d),
                        Err(e) => return note(cx, e, true),
                    }
                }
            };
            set_input(&mut cx.s, input);
        }
        LabAct::Screen(w) => {
            if core_lab::resizable(cx.s.task) && cx.s.lab_state.screen != w {
                cx.s.lab_state.screen = w;
                cx.s.invalidate();
                refresh_cases(&mut cx.s);
            }
        }
        LabAct::Teach => {
            cx.s.lab_state.teach = !cx.s.lab_state.teach;
            cx.s.generation += 1;
        }
        LabAct::Copy => copy_code(cx),
        LabAct::Paste => match import::paste() {
            Some(text) => cx.s.lab_state.field = text.trim().chars().take(4096).collect(),
            None => note(cx, t(l, "The clipboard has no text.", "剪贴板里没有文字。"), true),
        },
        LabAct::Import => import_code(cx),
        LabAct::ClearField => {
            cx.s.lab_state.field.clear();
            cx.s.lab_state.check = None;
            cx.s.lab_state.note = None;
        }
        LabAct::Rescan => {
            cx.s.lab_state.files = import::scan();
            let _ = std::fs::create_dir_all(import::folder());
        }
    }
    cx.dirty();
}

/// After the kernel list changed: the frame's run is gone.
fn fresh_editor(s: &mut Session) {
    s.undo.clear();
    s.sel = Sel::None;
    s.scroll = 0;
    s.invalidate();
}

pub fn note(cx: &mut Ctx, msg: impl Into<String>, bad: bool) {
    cx.s.lab_state.note = Some((msg.into(), bad));
    if bad {
        cx.cues.play(Cue::Error);
    }
    cx.dirty();
}

/// Feeds the frame a new input.
pub fn set_input(s: &mut Session, input: Input) {
    s.lab_state.input = input;
    s.invalidate();
    refresh_cases(s);
}

/// Clicking a slot: a free one keeps the current frame there; a filled one
/// is loaded (the current frame stays in its own slot).
fn pick_slot(cx: &mut Ctx, i: usize) {
    let l = cx.lang();
    if cx.s.lab_state.slot == Some(i) {
        return;
    }
    let Some(x) = cx.game.save.lab_slots.get(i).cloned() else { return };
    if x.is_empty() {
        cx.s.lab_state.slot = Some(i);
        save(&mut cx.game, &cx.s);
        let msg = format!("{} {}.", t(l, "Saved in slot", "已存入存档位"), i + 1);
        return note(cx, msg, false);
    }
    save(&mut cx.game, &cx.s);
    let Some(idx) = core_lab::index(&x.machine) else { return };
    reopen(cx, idx, Some(i), x);
}

/// Opens the bench on machine `idx` with `from` loaded, keeping the
/// panel's own state (teaching, share field, folder listing).
fn reopen(cx: &mut Ctx, idx: usize, slot: Option<usize>, from: LabSlot) {
    let keep = std::mem::take(&mut cx.s.lab_state);
    let tab = cx.s.tab;
    crate::bench::open_lab(&mut cx.s, &cx.game, idx);
    load(&mut cx.s, slot, Some(from));
    let st = &mut cx.s.lab_state;
    st.teach = keep.teach;
    st.field = keep.field;
    st.code = keep.code;
    st.files = keep.files;
    st.pending = keep.pending;
    cx.s.tab = if crate::machine::tabs(cx.s.task()).contains(&tab) || tab == Tab::Lab {
        tab
    } else {
        Tab::Lab
    };
}

/// Exports the frame as a share code (running it first if needed) and puts
/// it on the clipboard.
fn copy_code(cx: &mut Ctx) {
    let l = cx.lang();
    if !cx.s.lab_state.ran() {
        run(cx);
        if !cx.s.lab_state.ran() {
            return note(cx, t(l, "Fix the build error first.", "先修好构建错误。"), true);
        }
    }
    let frame = current(&cx.s, String::new());
    let st = &mut cx.s.lab_state;
    st.code = code::encode(&frame, st.total);
    let cut = code::decode(&st.code).is_ok_and(|d| d.cut);
    let msg = if import::copy(&st.code) {
        t(l, "Code copied to the clipboard.", "分享码已复制到剪贴板。")
    } else {
        t(l, "No clipboard here: copy the code from the box.", "这里用不了剪贴板：请从框里手动复制。")
    };
    let mut msg = msg.to_string();
    if cut {
        msg = format!(
            "{msg} {}",
            t(l, "The imported file is too big to travel; the code uses the level's input.", "导入的文件太大，分享码里改用关卡自带的输入。")
        );
    }
    note(cx, msg, false);
}

/// Reads the import field, loads the frame and re-runs it here.
fn import_code(cx: &mut Ctx) {
    let l = cx.lang();
    let text: String = cx.s.lab_state.field.split_whitespace().collect();
    if text.is_empty() {
        return note(cx, t(l, "Paste a code into the box first.", "先把分享码贴进框里。"), true);
    }
    let shared = match code::decode(&text) {
        Ok(x) => x,
        Err(e) => return note(cx, e.get(l), true),
    };
    let Some(idx) = core_lab::index(&shared.slot.machine) else {
        return note(cx, t(l, "That machine is not in this version of the game.", "这个版本的游戏里没有那台机器。"), true);
    };
    let tasks = content::tasks();
    let ch = tasks[idx].chapter;
    let open = tasks[idx..]
        .iter()
        .any(|t| t.chapter == ch && cx.game.save.campaign.is_solved(t.id));
    if !open {
        let msg = format!(
            "{} {}",
            t(l, "This code runs on a machine you have not reached yet:", "这个分享码用的机器你还没解锁："),
            tasks[idx].title.get(l)
        );
        return note(cx, msg, true);
    }
    save(&mut cx.game, &cx.s);
    let free = cx.game.save.lab_slots.iter().position(LabSlot::is_empty);
    let mut slot = shared.slot;
    slot.name = t(l, "Shared code", "分享的作品").into();
    if let Some(i) = free {
        cx.game.save.lab_slots[i] = slot.clone();
        cx.game.touch();
    }
    cx.s.lab_state.pending = Some(shared.cycles);
    cx.s.lab_state.check = None;
    reopen(cx, idx, free, slot);
    run(cx);
    let mut msgs = Vec::new();
    if free.is_none() {
        msgs.push(t(l, "Every slot is full, so this frame is not saved.", "存档位都满了，这一帧不会保存。"));
    }
    if shared.cut {
        msgs.push(t(l, "The author's imported file was too big to include; this runs on the level's input.", "作者导入的文件太大没有带上，这里用关卡自带的输入。"));
    }
    cx.s.lab_state.note = (!msgs.is_empty()).then(|| (msgs.join(" "), false));
    cx.cues.play(Cue::Success);
}

/// Debug overrides for screenshots (see `boot`).
pub fn debug_env(cx: &mut Ctx, env: fn(&str) -> Option<String>) {
    if let Some(src) = env("OOC_LAB_KERNELS") {
        let ks: Vec<Solution> = src
            .split('|')
            .take(MAX_KERNELS)
            .map(|k| Solution::Code(k.replace("\\n", "\n")))
            .collect();
        if !ks.is_empty() {
            cx.s.work = Some(ks[0].clone());
            cx.s.lab_state.kernels = ks;
            cx.s.lab_state.cur = 0;
            cx.s.invalidate();
        }
    }
    if let Some(w) = env("OOC_LAB_SCREEN").and_then(|v| v.parse().ok()) {
        act(cx, LabAct::Screen(w));
    }
    if let Some(set) = env("OOC_LAB_INPUT").and_then(|v| Set::from_id(&v)) {
        set_input(&mut cx.s, Input::Set(set));
    }
    if let Some(path) = env("OOC_LAB_IMPORT") {
        let l = cx.lang();
        match import::load(std::path::Path::new(&path), cx.s.task(), l) {
            Ok(d) => set_input(&mut cx.s, Input::Data(d)),
            Err(e) => note(cx, e, true),
        }
    }
    if env("OOC_LAB_TEACH").is_some() {
        cx.s.lab_state.teach = true;
    }
    if env("OOC_LAB_COPY").is_some() {
        run(cx);
        let frame = current(&cx.s, String::new());
        let st = &mut cx.s.lab_state;
        st.code = code::encode(&frame, st.total);
        println!("{}", st.code);
    }
    if let Some(c) = env("OOC_LAB_SHARE") {
        cx.s.lab_state.field = c;
        import_code(cx);
    }
}
