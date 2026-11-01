//! Two editors, one program: the Blueprint | Microcode switch on the
//! bench. In chapter 1 both sides edit the work; from chapter 2 the
//! blueprint is a read-only dataflow drawing of the program.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_kit::audio::Cue;
use oo_kit::ui::{ButtonSpec, Tone, Ui};
use ooc_core::asm::assemble;
use ooc_core::grade::{BuildError, blueprint_of};
use ooc_core::graph::Graph;
use ooc_core::isa::{Instr, OpKind, Program};
use ooc_core::task::{Editor, Solution, Task};

use crate::bench::build_error_text;
use crate::state::{Act, Sel, Session, t};
use crate::{Ctx, View, code_ed, graph_ed};

/// The switch's state for one visit to the bench.
#[derive(Default)]
pub struct Bp {
    /// Code levels: the read-only blueprint is showing.
    pub view: bool,
    /// Chapter 1: the board as it was when the player switched to
    /// microcode, and the text it became. Switching back with the text
    /// unchanged restores the player's own layout.
    stash: Option<(String, Graph)>,
    /// Why the last switch to the blueprint was refused, and the work
    /// generation it describes.
    why: Option<(u64, String)>,
    /// Horizontal scroll of the read-only drawing, in screen pixels.
    scroll: f32,
}

/// The microcode ops the task's editor offers: on a blueprint level, the
/// instructions its parts compile to.
pub fn code_ops(task: &Task) -> Vec<OpKind> {
    match task.editor {
        Editor::Code(ops) => ops.to_vec(),
        Editor::Graph(kinds) => ooc_core::graph::code_ops(kinds),
    }
}

/// The blueprint side of the switch is showing.
pub fn showing(s: &Session) -> bool {
    match s.work.as_ref() {
        Some(Solution::Graph(_)) => true,
        _ => s.bp.view && matches!(s.task().editor, Editor::Code(_)),
    }
}

/// Code level with the read-only drawing up.
fn viewing(s: &Session) -> bool {
    s.bp.view && matches!(s.task().editor, Editor::Code(_))
}

/// Flips the bench to the blueprint (`true`) or microcode. In chapter 1
/// this converts the work (undoable); code that no board can draw stays
/// in microcode with the reason shown.
pub fn switch(cx: &mut Ctx, to_blueprint: bool) {
    let l = cx.lang();
    let kinds = match cx.s.task().editor {
        Editor::Code(_) => {
            cx.s.bp.view = to_blueprint;
            cx.s.sel = Sel::None;
            return;
        }
        Editor::Graph(kinds) => kinds,
    };
    let work = cx.s.work().clone();
    match (work, to_blueprint) {
        (Solution::Graph(g), false) => match g.source() {
            Ok(src) => {
                cx.s.bp.stash = Some((src.clone(), g));
                *cx.s.edit() = Solution::Code(src);
            }
            Err(e) => return refuse(cx, build_error_text(&BuildError::Graph(e), l)),
        },
        (Solution::Code(src), true) => {
            let kept = cx.s.bp.stash.as_ref().filter(|(s, _)| *s == src);
            let g = match kept {
                Some((_, g)) => Ok(g.clone()),
                None => assemble(&src)
                    .map_err(BuildError::Asm)
                    .and_then(|p| blueprint_of(kinds, &p)),
            };
            match g {
                Ok(g) => *cx.s.edit() = Solution::Graph(g),
                Err(e) => return refuse(cx, build_error_text(&e, l)),
            }
        }
        _ => return,
    }
    cx.s.bp.why = None;
    cx.s.sel = Sel::None;
    cx.s.drag = None;
    cx.s.scroll = 0;
    cx.s.typing = false;
    cx.cues.play(Cue::Place);
}

fn refuse(cx: &mut Ctx, why: String) {
    let l = cx.lang();
    cx.toasts.push(
        t(l, "Staying in microcode", "仍留在微码"),
        why.clone(),
    );
    cx.cues.play(Cue::Error);
    cx.s.bp.why = Some((cx.s.generation, why));
}

/// Editor actions, routed by which side is showing. The read-only drawing
/// only scrolls.
pub fn act(cx: &mut Ctx, a: Act) {
    if let Act::Blueprint(on) = a {
        return switch(cx, on);
    }
    if viewing(&cx.s) {
        if let Act::Scroll(d) = a {
            let max = max_scroll(&cx.s);
            cx.s.bp.scroll = (cx.s.bp.scroll + d as f32 * SCROLL_STEP).clamp(0.0, max);
        }
        return;
    }
    match cx.s.work() {
        Solution::Graph(_) => graph_ed::act(cx, a),
        Solution::Code(_) => code_ed::act(cx, a),
    }
}

/// The editor card's contents: whichever side is showing, the switch,
/// and a refused switch's reason.
pub fn draw_editor(ui: &mut Ui, v: &View) {
    let s = v.s;
    let l = v.lang;
    if viewing(s) {
        draw_view(ui, v);
    } else {
        match s.work() {
            Solution::Graph(_) => graph_ed::draw(ui, v),
            Solution::Code(_) => code_ed::draw(ui, v),
        }
    }
    let on = showing(s);
    let code_level = matches!(s.task().editor, Editor::Code(_));
    let sides = [
        (
            true,
            Vec2::new(99.0, 288.0),
            t(l, "Blueprint", "蓝图"),
            if code_level {
                t(l, "See the program as a dataflow drawing (view only) · Tab", "把程序看成数据流图（仅供查看）· Tab")
            } else {
                t(l, "Edit the circuit as parts and wires · Tab", "用部件和连线编辑电路 · Tab")
            },
        ),
        (
            false,
            Vec2::new(163.0, 288.0),
            t(l, "Microcode", "微码"),
            t(l, "Edit the same program as instructions · Tab", "把同一个程序当作指令编辑 · Tab"),
        ),
    ];
    for (side, p, label, tip) in sides {
        let here = side == on;
        ui.button(
            ButtonSpec::new(p, Vec2::new(62.0, 20.0), label, Act::Blueprint(side))
                .font(11.0)
                .selected(here)
                .tone(if here { Tone::Selected } else { Tone::Ghost })
                .tip(tip),
        );
    }
    if let Some((made, why)) = &s.bp.why
        && *made == s.generation
        && !on
    {
        ui.text(
            why.clone(),
            Vec2::new(-612.0, -50.0),
            12.0,
            ui.pal.bad,
            Anchor::CENTER_LEFT,
        );
    }
}

// The read-only drawing ---------------------------------------------------

/// Where the drawing may go (screen centre, size).
const AREA: (Vec2, Vec2) = (Vec2::new(-216.0, 121.0), Vec2::new(820.0, 302.0));
/// Programs longer than this get a notice instead of a drawing.
const MAX_DRAWN: usize = 80;
/// Smallest zoom before the drawing scrolls instead of shrinking.
const MIN_SCALE: f32 = 0.7;
const SCROLL_STEP: f32 = 120.0;
const MARGIN: f32 = 12.0;
/// Block size and gaps at scale 1.
const BW: f32 = 88.0;
const BH: f32 = 30.0;
const COL_GAP: f32 = 20.0;
const ROW_GAP: f32 = 10.0;
const ITEM_GAP: f32 = 18.0;
const PAD: f32 = 10.0;
const HEAD: f32 = 18.0;

/// A program's block structure.
enum Item {
    Op(usize),
    If {
        at: usize,
        then: Vec<Item>,
        /// The ELSE line and its body.
        other: Option<(usize, Vec<Item>)>,
    },
    Loop {
        at: usize,
        body: Vec<Item>,
    },
}

/// Splits valid code into nested items; `i` is the next line to read.
fn items(code: &[Instr], i: &mut usize) -> Vec<Item> {
    let mut out = Vec::new();
    while *i < code.len() {
        let at = *i;
        *i += 1;
        match code[at] {
            Instr::If { .. } => {
                let then = items(code, i);
                let other = match code.get(*i - 1) {
                    Some(Instr::Else) => Some((*i - 1, items(code, i))),
                    _ => None,
                };
                out.push(Item::If { at, then, other });
            }
            Instr::Loop { .. } => {
                let body = items(code, i);
                out.push(Item::Loop { at, body });
            }
            Instr::Else | Instr::EndIf | Instr::EndLoop => return out,
            _ => out.push(Item::Op(at)),
        }
    }
    out
}

/// A framed IF/ELSE/LOOP region, in drawing space (y grows downward).
struct Frame {
    min: Vec2,
    max: Vec2,
    label: String,
    /// ELSE divider: its y and label.
    split: Option<(f32, String)>,
    is_loop: bool,
}

/// Positions at scale 1 relative to the drawing's top-left, y downward.
#[derive(Default)]
struct Laid {
    size: Vec2,
    /// Block centre per instruction line.
    blocks: Vec<(usize, Vec2)>,
    frames: Vec<Frame>,
    /// Where a wire into a frame's predicate lands (IF line → point).
    heads: Vec<(usize, Vec2)>,
}

impl Laid {
    fn shift(mut self, by: Vec2) -> Self {
        for (_, p) in self.blocks.iter_mut().chain(self.heads.iter_mut()) {
            *p += by;
        }
        for f in &mut self.frames {
            f.min += by;
            f.max += by;
            if let Some((y, _)) = &mut f.split {
                *y += by.y;
            }
        }
        self
    }

    fn absorb(&mut self, other: Laid) {
        self.blocks.extend(other.blocks);
        self.frames.extend(other.frames);
        self.heads.extend(other.heads);
    }
}

/// The most blocks stacked in one column before a run spills rightward.
const MAX_ROWS: usize = 5;

/// Lays out a straight-line run in columns by dependency depth.
fn lay_run(code: &[Instr], lines: &[usize]) -> Laid {
    let mut reg = [None::<usize>; 32];
    let mut pred = [None::<usize>; 4];
    let mut rows: Vec<usize> = Vec::new();
    let mut laid = Laid::default();
    for &i in lines {
        let ins = &code[i];
        let need = ins
            .reads()
            .filter_map(|r| reg.get(r as usize).copied().flatten())
            .chain(pred_read(ins).and_then(|p| pred.get(p as usize).copied().flatten()))
            .map(|c| c + 1)
            .max()
            .unwrap_or(0);
        let col = (need..).find(|&c| rows.get(c).copied().unwrap_or(0) < MAX_ROWS).unwrap_or(need);
        if rows.len() <= col {
            rows.resize(col + 1, 0);
        }
        let row = rows[col];
        rows[col] += 1;
        if let Some(d) = ins.dest() {
            reg[d as usize] = Some(col);
        }
        if let Some(p) = pred_write(ins) {
            pred[p as usize] = Some(col);
        }
        let at = Vec2::new(
            col as f32 * (BW + COL_GAP) + BW / 2.0,
            row as f32 * (BH + ROW_GAP) + BH / 2.0,
        );
        laid.blocks.push((i, at));
    }
    let cols = rows.len() as f32;
    let tall = rows.iter().copied().max().unwrap_or(0) as f32;
    laid.size = Vec2::new(
        cols * (BW + COL_GAP) - COL_GAP,
        tall * (BH + ROW_GAP) - ROW_GAP,
    );
    laid
}

/// Lays out items left to right, each centred on the row.
fn lay_seq(code: &[Instr], items: &[Item]) -> Laid {
    let mut parts: Vec<Laid> = Vec::new();
    let mut run: Vec<usize> = Vec::new();
    for item in items {
        if let Item::Op(i) = item {
            run.push(*i);
            continue;
        }
        if !run.is_empty() {
            parts.push(lay_run(code, &std::mem::take(&mut run)));
        }
        parts.push(lay_frame(code, item));
    }
    if !run.is_empty() {
        parts.push(lay_run(code, &run));
    }
    let mut out = Laid::default();
    if parts.is_empty() {
        out.size = Vec2::new(BW * 0.75, BH);
        return out;
    }
    let tall = parts.iter().map(|p| p.size.y).fold(0.0, f32::max);
    let mut x = 0.0;
    for p in parts {
        let size = p.size;
        out.absorb(p.shift(Vec2::new(x, (tall - size.y) / 2.0)));
        x += size.x + ITEM_GAP;
    }
    out.size = Vec2::new(x - ITEM_GAP, tall);
    out
}

/// An IF or LOOP as a framed box around its body.
fn lay_frame(code: &[Instr], item: &Item) -> Laid {
    let (at, first, other, is_loop) = match item {
        Item::If { at, then, other } => (*at, then, other.as_ref(), false),
        Item::Loop { at, body } => (*at, body, None, true),
        Item::Op(_) => unreachable!("runs are laid out by lay_run"),
    };
    let a = lay_seq(code, first);
    let b = other.map(|(e, body)| (*e, lay_seq(code, body)));
    let inner = b.as_ref().map_or(a.size.x, |(_, b)| a.size.x.max(b.size.x)).max(BW);
    let top = HEAD + PAD;
    let mut out = Laid::default();
    let a_h = a.size.y;
    out.absorb(a.shift(Vec2::new(PAD, top)));
    let mut h = top + a_h + PAD;
    let mut split = None;
    if let Some((e, b)) = b {
        split = Some((h, code[e].to_string()));
        let b_h = b.size.y;
        out.absorb(b.shift(Vec2::new(PAD, h + PAD)));
        h += PAD + b_h + PAD;
    }
    out.size = Vec2::new(inner + 2.0 * PAD, h);
    out.heads.push((at, Vec2::new(0.0, HEAD / 2.0)));
    out.frames.push(Frame {
        min: Vec2::ZERO,
        max: out.size,
        label: code[at].to_string(),
        split,
        is_loop,
    });
    out
}

/// The predicate an instruction reads.
fn pred_read(ins: &Instr) -> Option<u8> {
    match *ins {
        Instr::Sel { p, .. } | Instr::If { p } => Some(p),
        Instr::Vote { q, .. } => Some(q),
        _ => None,
    }
}

/// The predicate an instruction writes.
fn pred_write(ins: &Instr) -> Option<u8> {
    match *ins {
        Instr::SetP { p, .. } | Instr::Vote { p, .. } => Some(p),
        _ => None,
    }
}

/// Definitions that may reach a point: per register and per predicate.
#[derive(Clone)]
struct Defs {
    r: Vec<Vec<usize>>,
    p: Vec<Vec<usize>>,
}

impl Defs {
    fn merge(&mut self, other: Defs) {
        for (mine, theirs) in self.r.iter_mut().chain(self.p.iter_mut()).zip(other.r.into_iter().chain(other.p)) {
            for d in theirs {
                if !mine.contains(&d) {
                    mine.push(d);
                }
            }
        }
    }
}

/// Dataflow edges `(from, to, predicate?)`: every write that can reach a
/// read, through either side of an IF. Loop-carried values are left out.
fn edges(code: &[Instr]) -> Vec<(usize, usize, bool)> {
    let mut cur = Defs {
        r: vec![Vec::new(); 32],
        p: vec![Vec::new(); 4],
    };
    // Per open IF/LOOP: the state on entry and the THEN side's end state.
    let mut open: Vec<(Defs, Option<Defs>)> = Vec::new();
    let mut out = Vec::new();
    for (i, ins) in code.iter().enumerate() {
        let mut regs: Vec<u8> = ins.reads().collect();
        regs.sort_unstable();
        regs.dedup();
        for r in regs {
            for &d in cur.r.get(r as usize).into_iter().flatten() {
                out.push((d, i, false));
            }
        }
        if let Some(p) = pred_read(ins) {
            for &d in cur.p.get(p as usize).into_iter().flatten() {
                out.push((d, i, true));
            }
        }
        match ins {
            Instr::If { .. } | Instr::Loop { .. } => open.push((cur.clone(), None)),
            Instr::Else => {
                if let Some((entry, then)) = open.last_mut() {
                    *then = Some(std::mem::replace(&mut cur, entry.clone()));
                }
            }
            Instr::EndIf => {
                if let Some((entry, then)) = open.pop() {
                    cur.merge(then.unwrap_or(entry));
                }
            }
            Instr::EndLoop => {
                open.pop();
            }
            _ => {}
        }
        if let Some(d) = ins.dest() {
            cur.r[d as usize] = vec![i];
        }
        if let Some(p) = pred_write(ins) {
            cur.p[p as usize] = vec![i];
        }
    }
    out
}

/// Why there is no drawing.
enum Blank {
    Empty,
    Broken,
    Large(usize),
}

/// The program, its layout and the zoom that fits it.
fn plan(s: &Session) -> Result<(Program, Laid, f32), Blank> {
    let Solution::Code(src) = s.work() else {
        return Err(Blank::Broken);
    };
    let program = assemble(src).map_err(|_| Blank::Broken)?;
    let code = &program.code;
    if code.is_empty() {
        return Err(Blank::Empty);
    }
    if code.len() > MAX_DRAWN {
        return Err(Blank::Large(code.len()));
    }
    let laid = lay_seq(code, &items(code, &mut 0));
    let room = AREA.1 - Vec2::splat(2.0 * MARGIN);
    let mut scale = (room.y / laid.size.y).min(1.0);
    if laid.size.x * scale > room.x {
        scale = (room.x / laid.size.x).max(MIN_SCALE).min(scale);
    }
    Ok((program, laid, scale))
}

/// How far the drawing scrolls before its right end shows.
fn max_scroll(s: &Session) -> f32 {
    match plan(s) {
        Ok((_, laid, scale)) => (laid.size.x * scale - (AREA.1.x - 2.0 * MARGIN)).max(0.0),
        Err(_) => 0.0,
    }
}

/// The read-only blueprint of a code level's program: blocks per
/// instruction, wires per value, frames per IF and LOOP.
fn draw_view(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    ui.caption(
        t(l, "BLUEPRINT · VIEW ONLY", "蓝图 · 仅供查看"),
        Vec2::new(-612.0, 288.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    for (k, (label, c)) in [
        (t(l, "register", "寄存器"), pal.primary),
        (t(l, "predicate", "谓词"), pal.warn),
    ]
    .into_iter()
    .enumerate()
    {
        let x = -380.0 + k as f32 * 96.0;
        ui.line(Vec2::new(x, 288.0), Vec2::new(x + 18.0, 288.0), 2.0, c, 12.0);
        ui.text(label, Vec2::new(x + 24.0, 288.0), 11.0, pal.text_dim, Anchor::CENTER_LEFT);
    }
    ui.well(AREA.0, AREA.1);
    ui.wrapped(
        t(
            l,
            "View only: from chapter 2 the blueprint draws your program. Edit it in Microcode.",
            "仅供查看：从第 2 章起，蓝图只画出你的程序。修改请切到微码。",
        ),
        Vec2::new(-612.0, -50.0),
        690.0,
        12.0,
        pal.text_dim,
        Anchor::CENTER_LEFT,
    );
    let (program, laid, scale) = match plan(s) {
        Ok(p) => p,
        Err(blank) => {
            let msg = match blank {
                Blank::Empty => t(
                    l,
                    "Nothing to draw yet. Add instructions in Microcode.",
                    "还没有可画的内容。先在微码里添加指令。",
                )
                .to_string(),
                Blank::Broken => t(
                    l,
                    "Fix the microcode first: the blueprint draws only a program that assembles.",
                    "先修好微码：蓝图只画能汇编的程序。",
                )
                .to_string(),
                Blank::Large(n) => oo_base::Tr::new(
                    "Too large to draw: {} lines, the blueprint draws up to {}. Read it in Microcode.",
                    "太大，画不下：共 {} 行，蓝图最多画 {} 行。请在微码里查看。",
                )
                .fill(l, &[&n, &MAX_DRAWN]),
            };
            ui.wrapped(msg, AREA.0, 560.0, 14.0, pal.text_dim, Anchor::CENTER);
            return;
        }
    };
    let code = &program.code;
    let room = AREA.1 - Vec2::splat(2.0 * MARGIN);
    let left = AREA.0.x - AREA.1.x / 2.0 + MARGIN;
    let right = left + room.x;
    let top = AREA.0.y + AREA.1.y / 2.0 - MARGIN;
    let wide = laid.size.x * scale;
    let max = (wide - room.x).max(0.0);
    let scroll = s.bp.scroll.min(max);
    let dx = if max > 0.0 { -scroll } else { (room.x - wide) / 2.0 };
    let dy = (room.y - laid.size.y * scale) / 2.0;
    let to = |p: Vec2| Vec2::new(left + dx + p.x * scale, top - dy - p.y * scale);
    let inside = |x: f32| (left - 0.5..=right + 0.5).contains(&x);
    draw_frames(ui, &laid, scale, &to, (left, right));
    // Wires under the blocks, clipped to the well.
    let mut at = vec![None; code.len()];
    for &(i, p) in laid.blocks.iter().chain(laid.heads.iter()) {
        at[i] = Some(p);
    }
    let half = Vec2::new(BW / 2.0, 0.0);
    for (from, into, pred) in edges(code) {
        let (Some(a), Some(b)) = (at[from], at[into]) else {
            continue;
        };
        let a = to(a + half);
        let b = if matches!(code[into], Instr::If { .. }) { to(b) } else { to(b - half) };
        let c = if pred {
            pal.warn.with_alpha(0.7)
        } else {
            pal.primary.with_alpha(0.45)
        };
        let k = ((b.x - a.x).abs() * 0.5).max(24.0 * scale);
        let (c1, c2) = (a + Vec2::new(k, 0.0), b - Vec2::new(k, 0.0));
        let point = |t: f32| {
            let u = 1.0 - t;
            a * u * u * u + c1 * 3.0 * u * u * t + c2 * 3.0 * u * t * t + b * t * t * t
        };
        let mut prev = a;
        for n in 1..=8 {
            let p = point(n as f32 / 8.0);
            if inside(prev.x) && inside(p.x) {
                ui.line(prev, p, 1.5, c, 6.0);
            }
            prev = p;
        }
    }
    let size = Vec2::new(BW, BH) * scale;
    for &(i, p) in &laid.blocks {
        let c = to(p);
        if !inside(c.x - size.x / 2.0) || !inside(c.x + size.x / 2.0) {
            continue;
        }
        draw_block(ui, &code[i], i, c, size, scale);
    }
    if max > 0.0 {
        draw_scroll(ui, scroll, max, wide, (left, room.x));
    }
}

/// IF and LOOP boxes, clipped to the well; a header scrolled off the left
/// keeps its label at the edge.
fn draw_frames(ui: &mut Ui, laid: &Laid, scale: f32, to: &impl Fn(Vec2) -> Vec2, clip: (f32, f32)) {
    let pal = ui.pal;
    let (left, right) = clip;
    for f in &laid.frames {
        let (a, b) = (to(f.min), to(f.max));
        let (x0, x1) = (a.x.max(left), b.x.min(right));
        if x1 <= x0 + 1.0 {
            continue;
        }
        let c = if f.is_loop { pal.tertiary } else { pal.warn };
        let edge = c.with_alpha(0.55);
        let w = x1 - x0;
        let mid = (x0 + x1) / 2.0;
        let head = HEAD * scale;
        ui.rect(Vec2::new(mid, (a.y + b.y) / 2.0), Vec2::new(w, a.y - b.y), c.with_alpha(0.03), 5.3);
        ui.rect(Vec2::new(mid, a.y - head / 2.0), Vec2::new(w, head), c.with_alpha(0.11), 5.35);
        for y in [a.y, b.y] {
            ui.line(Vec2::new(x0, y), Vec2::new(x1, y), 1.0, edge, 5.6);
        }
        for x in [a.x, b.x] {
            if (left..=right).contains(&x) {
                ui.line(Vec2::new(x, a.y), Vec2::new(x, b.y), 1.0, edge, 5.6);
            }
        }
        ui.text(
            f.label.clone(),
            Vec2::new(x0 + 6.0 * scale, a.y - head / 2.0),
            11.0 * scale,
            c,
            Anchor::CENTER_LEFT,
        );
        if let Some((y, label)) = &f.split {
            let y = to(Vec2::new(0.0, *y)).y;
            let from = x0 + 44.0 * scale;
            if from < x1 {
                ui.line(Vec2::new(from, y), Vec2::new(x1, y), 1.0, edge, 5.6);
            }
            ui.text(label.clone(), Vec2::new(x0 + 6.0 * scale, y), 10.0 * scale, c, Anchor::CENTER_LEFT);
        }
    }
}

/// One instruction: mnemonic over operands, coloured by kind, with its
/// line number.
fn draw_block(ui: &mut Ui, ins: &Instr, line: usize, c: Vec2, size: Vec2, scale: f32) {
    let pal = ui.pal;
    let color = code_ed::op_color(ui, ins.kind());
    ui.chip(c, size + Vec2::splat(2.0), color.with_alpha(0.45), 7.0);
    ui.chip(c, size, pal.glass_raised, 7.1);
    ui.rect(
        Vec2::new(c.x - size.x / 2.0 + 2.5 * scale, c.y),
        Vec2::new(2.5 * scale, size.y - 8.0 * scale),
        color,
        7.2,
    );
    let text = ins.to_string();
    let (head, rest) = text.split_once(' ').unwrap_or((&text, ""));
    let x = c.x - size.x / 2.0 + 9.0 * scale;
    if rest.is_empty() {
        ui.text(head.to_string(), Vec2::new(x, c.y), 11.5 * scale, pal.text, Anchor::CENTER_LEFT);
    } else {
        ui.text(head.to_string(), Vec2::new(x, c.y + 6.5 * scale), 11.0 * scale, pal.text, Anchor::CENTER_LEFT);
        ui.text(rest.to_string(), Vec2::new(x, c.y - 7.0 * scale), 9.5 * scale, pal.text_dim, Anchor::CENTER_LEFT);
    }
    ui.text(
        (line + 1).to_string(),
        Vec2::new(c.x + size.x / 2.0 - 5.0 * scale, c.y + 7.0 * scale),
        8.0 * scale,
        pal.text_faint,
        Anchor::CENTER_RIGHT,
    );
}

/// Scroll buttons by the switch and a position bar under the well.
fn draw_scroll(ui: &mut Ui, scroll: f32, max: f32, wide: f32, track: (f32, f32)) {
    let pal = ui.pal;
    for (x, label, d, on) in [(22.0, "‹", -1, scroll > 0.0), (48.0, "›", 1, scroll < max)] {
        ui.button(
            ButtonSpec::new(Vec2::new(x, 288.0), Vec2::new(22.0, 20.0), label, Act::Scroll(d))
                .font(13.0)
                .tone(Tone::Ghost)
                .enabled(on),
        );
    }
    let (left, w) = track;
    let y = AREA.0.y - AREA.1.y / 2.0 + 4.0;
    ui.rect(Vec2::new(left + w / 2.0, y), Vec2::new(w, 2.0), pal.line.with_alpha(0.3), 5.6);
    let thumb = (w * w / wide).max(24.0);
    let x = left + (w - thumb) * scroll / max + thumb / 2.0;
    ui.rect(Vec2::new(x, y), Vec2::new(thumb, 2.0), pal.text_dim, 5.7);
}
