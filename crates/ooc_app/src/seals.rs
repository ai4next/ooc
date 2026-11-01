//! Seals in the app: the results button that makes one, the palette group
//! and `USE` rows in the block editor, and folded bands on the Warp River.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_kit::audio::Cue;
use oo_kit::ui::{ButtonSpec, Tone, Ui};
use ooc_core::edit::Doc;
use ooc_core::save::SaveData;
use ooc_core::seal::{self, Sealable, Span};
use ooc_core::task::Solution;
use ooc_core::vm::Step;

use crate::fb::RIVER_COLS;
use crate::state::{Act, Screen, Session, t};
use crate::{Ctx, View, river};

/// Sealed blocks are violet everywhere: editor rows, palette, river bands.
pub const SEAL_RGB: [u8; 4] = [176, 132, 255, 255];
const SEAL_DARK: [u8; 4] = [70, 48, 120, 255];
const RIVER_EMPTY: [u8; 4] = [7, 10, 18, 255];

pub fn colour() -> Color {
    river::river_colour(SEAL_RGB)
}

// ---- results ---------------------------------------------------------------

/// The seal this visit can make: a sealable level, passed, with seals open.
fn offer(s: &Session, save: &SaveData) -> Option<&'static Sealable> {
    if s.lab || !s.report.as_ref().is_some_and(|r| r.passed()) {
        return None;
    }
    let sl = seal::sealable(s.task().id)?;
    seal::is_open(save).then_some(sl)
}

fn work_src(s: &Session) -> Option<&str> {
    match s.work.as_ref()? {
        Solution::Code(src) => Some(src),
        Solution::Graph(_) => None,
    }
}

/// The "Seal as `name`" button on a sealable level's results.
pub fn draw_button(ui: &mut Ui, v: &View) {
    let l = v.lang;
    let save = &v.game.save;
    let Some(sl) = offer(v.s, save) else { return };
    let same = work_src(v.s)
        .and_then(|src| seal::flatten(src, &save.seals).ok())
        .is_some_and(|flat| save.seals.get(sl.name) == Some(&flat));
    let label = if same {
        format!("{} {} ✓", t(l, "Sealed", "已封印"), sl.name)
    } else {
        format!("{} {}", t(l, "Seal as", "封印为"), sl.name)
    };
    let tip = format!(
        "{}\n{}",
        sl.card.get(l),
        t(
            l,
            "Later levels and the free bench can paste it with USE. It costs its full cycles.",
            "之后的关卡和自由工作台可以用 USE 粘进来。周期照实计算。",
        )
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(-120.0, -290.0),
            Vec2::new(180.0, 40.0),
            label,
            Act::Seal,
        )
        .tone(if same { Tone::Ghost } else { Tone::Secondary })
        .font(14.0)
        .tip(tip),
    );
}

/// Stores the passing work as the level's seal.
pub fn seal_work(cx: &mut Ctx) {
    let l = cx.lang();
    let Some(sl) = offer(&cx.s, &cx.game.save) else {
        return;
    };
    let Some(src) = work_src(&cx.s).map(str::to_string) else {
        return;
    };
    match seal::store(&mut cx.game.save.seals, sl, &src) {
        Ok(changed) => {
            let title = if changed {
                format!("{} {}", t(l, "Sealed", "已封印"), sl.name)
            } else {
                format!("{} {}", sl.name, t(l, "is already this code", "已经是这段代码"))
            };
            cx.toasts.push(
                title,
                format!(
                    "{} USE {}",
                    t(l, "In later levels, add it from the palette or type", "之后的关卡里，从指令面板添加，或写"),
                    sl.name
                ),
            );
            cx.game.touch();
            cx.cues.play(Cue::Place);
        }
        Err(e) => {
            cx.toasts.push(
                t(l, "Could not seal", "无法封印"),
                crate::bench::build_error_text(&ooc_core::grade::BuildError::Asm(e), l),
            );
            cx.cues.play(Cue::Error);
        }
    }
    cx.dirty();
}

// ---- block editor ----------------------------------------------------------

/// Seals the editor offers: every one whose ops this task allows, from
/// chapter 10 on and in the free bench.
pub fn offered(v: &View) -> Vec<&'static Sealable> {
    let task = v.s.task();
    if !v.s.lab && task.chapter < 10 {
        return Vec::new();
    }
    seal::usable(task, &v.game.save.seals)
}

/// The palette's SEALS group under the instructions; returns the new bottom.
pub fn palette(ui: &mut Ui, v: &View, bottom: f32, px: f32) -> f32 {
    let list = offered(v);
    if list.is_empty() {
        return bottom;
    }
    let pal = ui.pal;
    let l = v.lang;
    ui.caption(
        t(l, "SEALS · ADD AS USE", "封印 · 以 USE 添加"),
        Vec2::new(px - 158.0, bottom - 6.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let cols = 3;
    let pitch = 320.0 / cols as f32;
    let y0 = bottom - 28.0;
    for (n, sl) in list.iter().enumerate() {
        let (c, r) = ((n % cols) as f32, (n / cols) as f32);
        let p = Vec2::new(px - 160.0 + pitch / 2.0 + c * pitch, y0 - r * 25.0);
        let at = seal::CATALOGUE
            .iter()
            .position(|s| s.name == sl.name)
            .unwrap_or(0);
        ui.button(
            ButtonSpec::new(p, Vec2::new(pitch - 4.0, 22.0), sl.name, Act::UseSeal(at))
                .font(12.0)
                .tone(Tone::Ghost)
                .tip(card(v, sl.name)),
        );
        ui.rect(
            p + Vec2::new(3.0 - (pitch - 4.0) / 2.0, 0.0),
            Vec2::new(3.0, 13.0),
            colour(),
            11.5,
        );
    }
    y0 - (list.len().div_ceil(cols) - 1) as f32 * 25.0 - 12.5
}

/// A seal's card: what it does and its register contract.
pub fn card(v: &View, name: &str) -> String {
    let l = v.lang;
    let seals = &v.game.save.seals;
    let Some(c) = seal::contract(seals, name) else {
        let e = match seals.contains_key(name) {
            true => seal::SealError::Broken(name.to_string()),
            false => seal::SealError::Unknown(name.to_string()),
        };
        return e.text(l);
    };
    let regs = |r: &[u8]| match r {
        [] => "—".to_string(),
        r => r.iter().map(|r| format!("r{r}")).collect::<Vec<_>>().join(" "),
    };
    let what = seal::find(name).map_or("", |s| s.card.get(l));
    let mut out = format!(
        "USE {name} — {what}\n{} {} · {} {}\n{} {}",
        t(l, "Reads", "读取"),
        regs(&c.reads),
        t(l, "writes", "写入"),
        regs(&c.writes),
        c.len,
        t(l, "instructions, counted in full.", "条指令，全数计入。"),
    );
    if let Some(n) = c.smem {
        out.push_str(&format!(
            "\n{} .smem {n}",
            t(l, "Its level asked for", "它的关卡用了")
        ));
    }
    out
}

/// Program length with every `USE` expanded, as the machine counts it.
pub fn doc_len(v: &View, d: &Doc) -> usize {
    (0..d.code.len())
        .map(|i| match d.use_at(i) {
            Some(n) => seal::body_len(&v.game.save.seals, n).unwrap_or(1),
            None => 1,
        })
        .sum()
}

/// Inserts `USE name` after the selected line; returns its line.
pub fn insert(d: &mut Doc, after: Option<usize>, n: usize) -> Option<usize> {
    let sl = seal::CATALOGUE.get(n)?;
    Some(d.insert_use(after, sl.name))
}

/// One `USE name` row of the block editor.
pub fn use_row(ui: &mut Ui, v: &View, i: usize, name: &str, x: f32, y: f32, on: bool) {
    let pal = ui.pal;
    let (mw, h) = (62.0, 18.0);
    ui.button(
        ButtonSpec::new(
            Vec2::new(x + mw / 2.0, y),
            Vec2::new(mw, h),
            "USE",
            Act::SelLine(i),
        )
        .font(12.0)
        .selected(on)
        .tone(if on { Tone::Selected } else { Tone::Ghost }),
    );
    ui.rect(Vec2::new(x + 1.5, y), Vec2::new(3.0, h - 4.0), colour(), 11.5);
    let w = 120.0;
    ui.button(
        ButtonSpec::new(
            Vec2::new(x + mw + 6.0 + w / 2.0, y),
            Vec2::new(w, h),
            name.to_string(),
            Act::SelLine(i),
        )
        .font(12.0)
        .tone(Tone::Secondary)
        .tip(card(v, name)),
    );
    let note = match seal::body_len(&v.game.save.seals, name) {
        Some(n) => (format!("{n} {}", t(v.lang, "lines", "条")), colour()),
        None => (t(v.lang, "not sealed", "未封印").to_string(), pal.bad),
    };
    ui.text(
        note.0,
        Vec2::new(x + mw + w + 14.0, y),
        12.0,
        note.1,
        Anchor::CENTER_LEFT,
    );
}

// ---- Warp River ------------------------------------------------------------

/// Columns a folded band takes, whatever its length.
const BAND_MIN: usize = 10;
const BAND_MAX: usize = 18;
/// Labels the River tab spawns; more bands than this stay unlabelled.
const LABELS: usize = 8;

/// What one river column shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Col {
    Empty,
    Step(usize),
    /// Steps `first..=last` all ran inside folded span `span`.
    Band { span: usize, first: usize, last: usize },
}

fn folded(spans: &[Span], open: &[(usize, usize)], pc: usize) -> Option<usize> {
    spans
        .iter()
        .position(|s| s.contains(pc) && !open.contains(&(s.start, s.end)))
}

/// The river's columns, newest at the right. Runs of steps inside one folded
/// span collapse into a band.
pub fn layout(trace: &[Step], step: usize, spans: &[Span], open: &[(usize, usize)]) -> Vec<Col> {
    let w = RIVER_COLS as usize;
    let mut cols = Vec::with_capacity(w);
    let mut i = step.min(trace.len());
    while cols.len() < w && i > 0 {
        let last = i - 1;
        match folded(spans, open, trace[last].pc as usize) {
            Some(k) => {
                let mut first = last;
                while first > 0 && folded(spans, open, trace[first - 1].pc as usize) == Some(k) {
                    first -= 1;
                }
                let n = (last + 1 - first)
                    .clamp(BAND_MIN, BAND_MAX)
                    .min(w - cols.len());
                cols.extend(std::iter::repeat_n(Col::Band { span: k, first, last }, n));
                i = first;
            }
            None => {
                cols.push(Col::Step(last));
                i = last;
            }
        }
    }
    cols.resize(w, Col::Empty);
    cols.reverse();
    cols
}

/// The replay's program spans and the selected case's trace.
fn run_of(s: &Session) -> Option<(&[Span], &[Step])> {
    let rep = s.report.as_ref()?;
    let run = &rep.runs[s.case.min(rep.runs.len() - 1)].run;
    Some((&rep.program.seals, &run.trace))
}

/// Paints the river with folded spans; `false` when nothing is sealed, so
/// the plain painter runs instead.
pub fn paint(img: &mut Image, s: &Session, mark: Option<usize>) -> bool {
    let Some((spans, trace)) = run_of(s).filter(|(sp, tr)| !sp.is_empty() && !tr.is_empty()) else {
        return false;
    };
    let cols = layout(trace, s.play.step, spans, &s.unfolded);
    let shown: Vec<Step> = cols
        .iter()
        .map(|c| match *c {
            Col::Step(i) => trace[i].clone(),
            _ => trace[0].clone(),
        })
        .collect();
    let mark = mark.and_then(|m| cols.iter().position(|c| *c == Col::Step(m)));
    river::paint_river(img, Some(&shown), shown.len(), mark);
    let w = RIVER_COLS as usize;
    let Some(data) = img.data.as_mut() else {
        return true;
    };
    for (c, col) in cols.iter().enumerate() {
        let edge = matches!(*col, Col::Band { .. })
            && (c == 0 || cols[c - 1] != *col || cols.get(c + 1) != Some(col));
        let px = match col {
            Col::Step(_) => continue,
            Col::Empty => RIVER_EMPTY,
            Col::Band { .. } if edge => SEAL_RGB,
            Col::Band { .. } => SEAL_DARK,
        };
        for lane in 0..ooc_core::isa::LANES {
            let i = (lane * w + c) * 4;
            if i + 4 <= data.len() {
                data[i..i + 4].copy_from_slice(&px);
            }
        }
    }
    true
}

/// The editor line of `pc`, 1-based, with the seal and its own line when
/// `pc` runs inside one: `1 (scan32 +27)`.
pub fn line_text(spans: &[Span], pc: usize) -> String {
    let line = seal::line_of(spans, pc) + 1;
    match spans.iter().find(|s| s.contains(pc)) {
        Some(s) => format!("{line} ({} +{})", s.name, pc - s.start + 1),
        None => line.to_string(),
    }
}

/// Band at a river column: (span, first, last, first column, last column).
type BandAt = (usize, usize, usize, usize, usize);

fn bands(cols: &[Col]) -> Vec<BandAt> {
    let mut out: Vec<BandAt> = Vec::new();
    for (c, col) in cols.iter().enumerate() {
        if let Col::Band { span, first, last } = *col {
            match out.last_mut() {
                Some(b) if b.4 + 1 == c && (b.0, b.1, b.2) == (span, first, last) => b.4 = c,
                _ => out.push((span, first, last, c, c)),
            }
        }
    }
    out
}

/// What clicking river column `col` means when seals fold the river: the
/// step to rewind to, or `None` after toggling a band. Outer `None`: no
/// folding here, use the plain mapping.
pub fn click(cx: &mut Ctx, col: usize) -> Option<Option<usize>> {
    let (spans, trace) = run_of(&cx.s).filter(|(sp, _)| !sp.is_empty())?;
    let cols = layout(trace, cx.s.play.step, spans, &cx.s.unfolded);
    match cols.get(col)? {
        Col::Step(i) => Some(Some(*i)),
        Col::Empty => Some(None),
        Col::Band { span, .. } => {
            fold(cx, *span);
            Some(None)
        }
    }
}

/// Folds or unfolds span `k` of the report's program.
pub fn fold(cx: &mut Ctx, k: usize) {
    let Some(sp) = cx.s.report.as_ref().and_then(|r| r.program.seals.get(k)) else {
        return;
    };
    let key = (sp.start, sp.end);
    match cx.s.unfolded.iter().position(|u| *u == key) {
        Some(i) => {
            cx.s.unfolded.remove(i);
        }
        None => cx.s.unfolded.push(key),
    }
    cx.s.generation += 1;
    cx.dirty();
}

/// Text over one folded band.
#[derive(Component)]
pub struct BandLabel(pub usize);

/// The River tab's band labels and the fold toggles beside the stats.
pub fn draw_river(ui: &mut Ui, v: &View) {
    for k in 0..LABELS {
        let e = ui.text("", Vec2::new(0.0, -1000.0), 11.0, ui.pal.text, Anchor::CENTER);
        ui.commands.entity(e).insert((BandLabel(k), Visibility::Hidden));
    }
    let Some(rep) = &v.s.report else { return };
    let spans = &rep.program.seals;
    if spans.is_empty() {
        return;
    }
    let pal = ui.pal;
    let l = v.lang;
    let x = 440.0;
    ui.caption(
        t(l, "SEALS · CLICK TO FOLD", "封印 · 点击折叠"),
        Vec2::new(x, -208.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    for (k, sp) in spans.iter().enumerate().take(3) {
        let open = v.s.unfolded.contains(&(sp.start, sp.end));
        let label = format!("{} {}", if open { "▾" } else { "▸" }, sp.name);
        ui.button(
            ButtonSpec::new(
                Vec2::new(x + 80.0, -236.0 - k as f32 * 28.0),
                Vec2::new(160.0, 24.0),
                label,
                Act::Fold(k),
            )
            .font(12.0)
            .tone(if open { Tone::Ghost } else { Tone::Secondary })
            .tip(format!(
                "{} {}–{}",
                t(l, "Lines", "行"),
                sp.start + 1,
                sp.end
            )),
        );
    }
}

/// Moves the band labels to the bands playback has reached.
pub fn sync_labels(
    s: Res<Session>,
    screen: Res<State<Screen>>,
    game: Res<crate::state::Game>,
    mut labels: Query<(&BandLabel, &mut Text2d, &mut Transform, &mut Visibility)>,
    mut last: Local<Option<(u64, usize, usize)>>,
) {
    if *screen.get() != Screen::Bench {
        *last = None;
        return;
    }
    let key = (s.generation, s.play.step, labels.iter().count());
    if *last == Some(key) {
        return;
    }
    *last = Some(key);
    let l = game.lang();
    let found = run_of(&s)
        .filter(|(sp, _)| !sp.is_empty())
        .map(|(spans, trace)| {
            let cols = layout(trace, s.play.step, spans, &s.unfolded);
            (spans, trace, bands(&cols))
        });
    let (at, size) = river::RIVER;
    let pitch = size.x / RIVER_COLS as f32;
    let left = at.x - size.x / 2.0;
    for (label, mut text, mut tf, mut vis) in &mut labels {
        let Some((spans, trace, list)) = &found else {
            *vis = Visibility::Hidden;
            continue;
        };
        let Some(&(span, first, last, c0, c1)) = list.get(label.0) else {
            *vis = Visibility::Hidden;
            continue;
        };
        let (a, b) = (&trace[first], &trace[last]);
        // Vault onwards steps carry their issue cycle; before that, one
        // step is one cycle of the single issuing warp.
        let cost = if b.cycle > 0 {
            format!("{} {}", b.cycle - a.cycle + 1, t(l, "cyc", "周期"))
        } else {
            format!("{} {}", last + 1 - first, t(l, "steps", "步"))
        };
        text.0 = format!("{}\n{cost}", spans[span].name);
        tf.translation.x = left + (c0 + c1 + 1) as f32 / 2.0 * pitch;
        tf.translation.y = at.y;
        tf.translation.z = 7.5;
        *vis = Visibility::Inherited;
    }
}
