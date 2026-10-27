//! The block editor for OC-1 code: one row per instruction, operands as
//! chips, a palette of the machine's opcodes and a picker for the selected slot.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_kit::audio::Cue;
use oo_kit::ui::{ButtonSpec, Tone, Ui};
use ooc_core::edit::{Doc, Operand, operands, set_operand};
use ooc_core::isa::{Alu, BASE_REGS, Cmp, Cvt, MAX_LOOP, OpKind, PREDS, Program, Special, Src};
use ooc_core::task::{Editor, Solution};

use crate::state::{Act, Sel, Session, t};
use crate::{Ctx, View};

/// Visible rows of the listing.
pub const ROWS: usize = 14;
const ROW_H: f32 = 22.0;
const TOP: f32 = 262.0;
const X0: f32 = -584.0;
const INDENT: f32 = 14.0;
const CHIP: f32 = 58.0;
/// Centre of the picker column.
const PX: f32 = 40.0;

pub fn row_y(r: usize) -> f32 {
    TOP - r as f32 * ROW_H
}

/// A display row: a note above an instruction, or the instruction itself.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Row {
    Note(usize),
    Line(usize),
}

fn rows(doc: &Doc) -> Vec<Row> {
    let mut out = Vec::new();
    for i in 0..doc.code.len() {
        if doc.notes.get(i).is_some_and(Option::is_some) {
            out.push(Row::Note(i));
        }
        out.push(Row::Line(i));
    }
    out
}

pub fn doc(s: &Session) -> Option<Doc> {
    match s.work.as_ref()? {
        Solution::Code(src) => Doc::parse(src).ok(),
        Solution::Graph(_) => None,
    }
}

/// Screen y of instruction `line` if it is scrolled into view.
pub fn line_y(s: &Session, line: usize) -> Option<f32> {
    let d = doc(s)?;
    let r = rows(&d).iter().position(|r| *r == Row::Line(line))?;
    let r = r.checked_sub(s.scroll).filter(|r| *r < ROWS)?;
    Some(row_y(r))
}

/// The selected immediate or loop count, for typing digits.
pub fn number_at(s: &Session, line: usize, k: usize) -> Option<i32> {
    let d = doc(s)?;
    match operands(d.code.get(line)?).get(k)? {
        Operand::Src(Src::Imm(v)) => Some(*v),
        Operand::Count(n) => Some(*n),
        _ => None,
    }
}

fn sel_line(s: &Session) -> Option<usize> {
    match s.sel {
        Sel::Line(i) | Sel::Operand(i, _) => Some(i),
        _ => None,
    }
}

fn commit(cx: &mut Ctx, d: &Doc) {
    *cx.s.edit() = Solution::Code(d.source());
}

/// Scrolls so instruction `line` is visible.
fn show_line(s: &mut Session, d: &Doc, line: usize) {
    let Some(r) = rows(d).iter().position(|r| *r == Row::Line(line)) else {
        return;
    };
    if r < s.scroll {
        s.scroll = r.saturating_sub(1);
    } else if r >= s.scroll + ROWS {
        s.scroll = r + 1 - ROWS;
    }
}

pub fn act(cx: &mut Ctx, a: Act) {
    let Some(mut d) = doc(&cx.s) else { return };
    let l = cx.lang();
    let line = sel_line(&cx.s);
    match a {
        Act::AddOp(kind) => {
            let extra = if matches!(kind, OpKind::If | OpKind::Loop) {
                2
            } else {
                1
            };
            let room = cx.s.task().profile.max_program();
            if d.code.len() + extra > room {
                cx.toasts.push(
                    t(l, "Program memory is full", "程序存储已满"),
                    format!(
                        "{} {room}",
                        t(l, "OC-1 holds at most", "OC-1 最多容纳指令条数：")
                    ),
                );
                cx.cues.play(Cue::Error);
                return;
            }
            let after = line.or(d.code.len().checked_sub(1));
            let at = d.insert(after, kind);
            commit(cx, &d);
            cx.s.sel = Sel::Line(at);
            show_line(&mut cx.s, &d, at);
            cx.cues.play(Cue::Place);
        }
        Act::SelLine(i) => {
            cx.s.sel = if cx.s.sel == Sel::Line(i) {
                Sel::None
            } else {
                Sel::Line(i)
            };
            cx.s.typing = false;
        }
        Act::SelOperand(i, k) => {
            let s = Sel::Operand(i, k);
            cx.s.sel = if cx.s.sel == s { Sel::Line(i) } else { s };
            cx.s.typing = false;
        }
        Act::SetOperand(op) => {
            if let Sel::Operand(i, k) = cx.s.sel
                && let Some(ins) = d.code.get_mut(i)
                && operands(ins).get(k) != Some(&op)
                && set_operand(ins, k, op)
            {
                commit(cx, &d);
            }
            cx.s.typing = false;
        }
        Act::Bump(delta) => {
            let Sel::Operand(i, k) = cx.s.sel else { return };
            let Some(ins) = d.code.get_mut(i) else { return };
            let next = match operands(ins).get(k) {
                Some(Operand::Src(Src::Imm(v))) => {
                    Operand::Src(Src::Imm((v + delta).clamp(-9999, 9999)))
                }
                Some(Operand::Count(n)) => Operand::Count((n + delta).clamp(0, MAX_LOOP)),
                _ => return,
            };
            if operands(ins).get(k) != Some(&next) && set_operand(ins, k, next) {
                commit(cx, &d);
            }
        }
        Act::Warps(delta) => {
            if !cx.s.task().free_warps {
                return;
            }
            let now = d.warps.unwrap_or(1) as i32;
            let next = (now + delta).clamp(1, 16) as u8;
            if next != now as u8 {
                d.warps = Some(next);
                commit(cx, &d);
            }
        }
        Act::Block(delta) => {
            if !cx.s.task().free_block {
                return;
            }
            let now = d.block.unwrap_or(32) as i32;
            let next = (now + delta * 32).clamp(32, 512) as u16;
            if next != now as u16 {
                d.block = Some(next);
                commit(cx, &d);
            }
        }
        Act::Smem(delta) => {
            let now = d.smem.unwrap_or(0) as i32;
            let next = (now + delta * 32).clamp(0, 8192) as u16;
            if next != now as u16 {
                d.smem = (next > 0).then_some(next);
                commit(cx, &d);
            }
        }
        Act::SmemScale(up) => {
            let now = d.smem.unwrap_or(0);
            let next = if up { (now.max(16) * 2).min(8192) } else { now / 2 };
            if next != now {
                d.smem = (next > 0).then_some(next);
                commit(cx, &d);
            }
        }
        Act::DeleteSel => {
            if let Some(i) = line.filter(|i| *i < d.code.len()) {
                d.remove(i);
                commit(cx, &d);
                cx.s.sel = Sel::None;
                let max = rows(&d).len().saturating_sub(ROWS);
                cx.s.scroll = cx.s.scroll.min(max);
            }
        }
        Act::MoveLine(up) => {
            if let Some(i) = line
                && let Some(j) = d.shift(i, up)
            {
                commit(cx, &d);
                cx.s.sel = match cx.s.sel {
                    Sel::Operand(_, k) => Sel::Operand(j, k),
                    _ => Sel::Line(j),
                };
                show_line(&mut cx.s, &d, j);
            }
        }
        Act::Scroll(dy) => {
            let max = rows(&d).len().saturating_sub(ROWS) as i32;
            cx.s.scroll = (cx.s.scroll as i32 + dy).clamp(0, max) as usize;
        }
        _ => {}
    }
}

fn op_color(ui: &Ui, k: OpKind) -> Color {
    let pal = ui.pal;
    match k {
        OpKind::If | OpKind::Else | OpKind::EndIf | OpKind::Loop | OpKind::EndLoop => pal.tertiary,
        OpKind::Ld | OpKind::St | OpKind::Pix | OpKind::Edge | OpKind::Rcp => pal.secondary,
        OpKind::Dot | OpKind::Lerp | OpKind::Mma => pal.secondary,
        OpKind::Ldo
        | OpKind::Lds
        | OpKind::Sts
        | OpKind::Ldl
        | OpKind::Stl
        | OpKind::Dma
        | OpKind::Send
        | OpKind::Recv
        | OpKind::Atom
        | OpKind::AtomS => pal.secondary,
        OpKind::Bar | OpKind::Gsync | OpKind::Fence => pal.tertiary,
        OpKind::Shfl | OpKind::Vote => pal.warn,
        _ => pal.primary,
    }
}

/// Listing, palette and picker inside the editor card.
pub fn draw(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let Editor::Code(ops) = s.task().editor else {
        return;
    };
    let Some(d) = doc(s) else {
        ui.text(
            t(
                l,
                "This program cannot be shown as blocks.",
                "这段程序无法显示为积木。",
            ),
            Vec2::new(-600.0, 240.0),
            15.0,
            pal.bad,
            Anchor::CENTER_LEFT,
        );
        ui.button(ButtonSpec::new(
            Vec2::new(-500.0, 190.0),
            Vec2::new(200.0, 34.0),
            t(l, "Restore the starter", "恢复初始程序"),
            Act::Restore,
        ));
        return;
    };
    ui.caption(
        format!(
            "{}  {} / {}",
            t(l, "PROGRAM", "程序"),
            d.code.len(),
            v.s.task().profile.max_program()
        ),
        Vec2::new(-612.0, 288.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    if s.task().free_warps {
        let n = d.warps.unwrap_or(1);
        ui.caption(
            t(l, "WARPS", "启动 warp 数"),
            Vec2::new(-470.0, 288.0),
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
        for (x, label, dw) in [(-348.0, "−", -1), (-290.0, "+", 1)] {
            ui.button(
                ButtonSpec::new(
                    Vec2::new(x, 288.0),
                    Vec2::new(26.0, 20.0),
                    label.to_string(),
                    Act::Warps(dw),
                )
                .font(13.0),
            );
        }
        ui.text(
            n.to_string(),
            Vec2::new(-319.0, 288.0),
            15.0,
            pal.secondary,
            Anchor::CENTER,
        );
    }
    if s.task().free_block {
        let n = d.block.unwrap_or(32);
        ui.caption(
            t(l, "BLOCK", "块线程数"),
            Vec2::new(-470.0, 288.0),
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
        for (x, label, dw) in [(-348.0, "−", -1), (-270.0, "+", 1)] {
            ui.button(
                ButtonSpec::new(
                    Vec2::new(x, 288.0),
                    Vec2::new(26.0, 20.0),
                    label.to_string(),
                    Act::Block(dw),
                )
                .font(13.0),
            );
        }
        ui.text(
            n.to_string(),
            Vec2::new(-309.0, 288.0),
            15.0,
            pal.secondary,
            Anchor::CENTER,
        );
    }
    if ops.contains(&OpKind::Lds) {
        ui.caption(
            ".SMEM",
            Vec2::new(-150.0, 288.0),
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
        let buttons: [(f32, &str, Act); 4] = [
            (-78.0, "÷2", Act::SmemScale(false)),
            (-48.0, "−", Act::Smem(-1)),
            (28.0, "+", Act::Smem(1)),
            (58.0, "×2", Act::SmemScale(true)),
        ];
        for (x, label, act) in buttons {
            ui.button(
                ButtonSpec::new(
                    Vec2::new(x, 288.0),
                    Vec2::new(26.0, 20.0),
                    label.to_string(),
                    act,
                )
                .font(12.0),
            );
        }
        ui.text(
            d.smem.map_or("—".to_string(), |n| n.to_string()),
            Vec2::new(-10.0, 288.0),
            15.0,
            pal.secondary,
            Anchor::CENTER,
        );
    }
    let all = rows(&d);
    let depths = Program::depths(&d.code);
    let selected = sel_line(s);
    let probe = probe_values(s);
    // The bar behind the line playback last issued; moved by `river::sync_markers`.
    let bar = ui.rect(
        Vec2::new(-385.0, -1000.0),
        Vec2::new(470.0, ROW_H - 2.0),
        pal.primary.with_alpha(0.09),
        5.5,
    );
    ui.commands
        .entity(bar)
        .insert((crate::river::PcBar, Visibility::Hidden));
    for (r, row) in all.iter().enumerate().skip(s.scroll).take(ROWS) {
        let y = row_y(r - s.scroll);
        let i = match *row {
            Row::Note(i) => {
                let x = X0 + depths[i] as f32 * INDENT;
                let note = ooc_core::edit::note_text(
                    d.notes[i].as_deref().unwrap_or(""),
                    l == oo_base::Lang::Zh,
                );
                ui.text(
                    format!("; {note}"),
                    Vec2::new(x, y),
                    12.0,
                    pal.text_faint,
                    Anchor::CENTER_LEFT,
                );
                continue;
            }
            Row::Line(i) => i,
        };
        let ins = &d.code[i];
        let x = X0 + depths[i] as f32 * INDENT;
        if selected == Some(i) {
            ui.rect(
                Vec2::new(-385.0, y),
                Vec2::new(470.0, ROW_H - 2.0),
                pal.text.with_alpha(0.06),
                5.4,
            );
        }
        ui.text(
            format!("{:>2}", i + 1),
            Vec2::new(-604.0, y),
            11.0,
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
        let kind = ins.kind();
        let c = op_color(ui, kind);
        let mw = 62.0;
        let label = match kind {
            OpKind::SetP | OpKind::Vote | OpKind::Alu(_) => kind_family(kind),
            k => k.mnemonic(),
        };
        let b = ButtonSpec::new(
            Vec2::new(x + mw / 2.0, y),
            Vec2::new(mw, ROW_H - 4.0),
            label,
            Act::SelLine(i),
        )
        .font(12.0)
        .selected(selected == Some(i))
        .tone(if selected == Some(i) {
            Tone::Selected
        } else {
            Tone::Ghost
        });
        ui.button(b);
        ui.rect(Vec2::new(x + 1.5, y), Vec2::new(3.0, ROW_H - 8.0), c, 11.5);
        for (k, op) in operands(ins).into_iter().enumerate() {
            let cx = x + mw + 6.0 + CHIP / 2.0 + k as f32 * (CHIP + 4.0);
            let on = s.sel == Sel::Operand(i, k);
            let b = ButtonSpec::new(
                Vec2::new(cx, y),
                Vec2::new(CHIP, ROW_H - 4.0),
                op.text(),
                Act::SelOperand(i, k),
            )
            .font(12.0)
            .tone(if on { Tone::Selected } else { Tone::Secondary });
            ui.button(b);
        }
        if let (Some(p), Some(reg)) = (&probe, written_reg(ins)) {
            ui.text(
                format!("= {}", p.regs[reg as usize]),
                Vec2::new(-152.0, y),
                12.0,
                pal.secondary,
                Anchor::CENTER_RIGHT,
            );
        }
    }
    if all.len() > ROWS {
        let h = ROWS as f32 * ROW_H;
        let frac = ROWS as f32 / all.len() as f32;
        let top = TOP + ROW_H / 2.0;
        let off = s.scroll as f32 / all.len() as f32 * h;
        ui.rect(
            Vec2::new(-626.0, top - h / 2.0),
            Vec2::new(3.0, h),
            pal.line,
            6.0,
        );
        ui.rect(
            Vec2::new(-626.0, top - off - frac * h / 2.0),
            Vec2::new(3.0, frac * h),
            pal.primary,
            6.1,
        );
    }
    draw_palette(ui, ops, l);
    draw_side(ui, v, &d, ops, probe);
}

fn kind_family(k: OpKind) -> &'static str {
    match k {
        OpKind::SetP => "SETP",
        OpKind::Vote => "VOTE",
        _ => "ALU",
    }
}

/// The register an instruction writes, for probe annotations.
fn written_reg(i: &ooc_core::isa::Instr) -> Option<u8> {
    use ooc_core::isa::Instr as I;
    match *i {
        I::Mov { d, .. }
        | I::Alu { d, .. }
        | I::Sel { d, .. }
        | I::Ld { d, .. }
        | I::Ldo { d, .. }
        | I::Lds { d, .. }
        | I::Ldl { d, .. }
        | I::Atom { d, .. }
        | I::Edge { d, .. }
        | I::Dot { d, .. }
        | I::Rcp { d, .. }
        | I::Cvt { d, .. }
        | I::Mma { d, .. }
        | I::Lerp { d, .. }
        | I::Shfl { d, .. } => Some(d),
        _ => None,
    }
}

fn probe_values(s: &Session) -> Option<ooc_core::grade::Probe> {
    let rep = s.report.as_ref()?;
    let th = s.probe?;
    let case = &rep.runs[s.case.min(rep.runs.len() - 1)].case;
    ooc_core::grade::probe(s.task(), &rep.program, case, th)
}

fn draw_palette(ui: &mut Ui, ops: &[OpKind], l: oo_base::Lang) {
    let pal = ui.pal;
    ui.caption(
        t(l, "ADD AN INSTRUCTION", "添加指令"),
        Vec2::new(PX - 158.0, 288.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let list: Vec<OpKind> = ops
        .iter()
        .copied()
        .filter(|k| !matches!(k, OpKind::EndIf | OpKind::EndLoop))
        .collect();
    // Five rows fit above the side panel; wider levels get a fifth column.
    let cols = if list.len() > 20 { 5 } else { 4 };
    let pitch = 320.0 / cols as f32;
    let w = pitch - 4.0;
    for (n, k) in list.into_iter().enumerate() {
        let (c, r) = ((n % cols) as f32, (n / cols) as f32);
        let p = Vec2::new(PX - 160.0 + pitch / 2.0 + c * pitch, 262.0 - r * 27.0);
        ui.button(
            ButtonSpec::new(p, Vec2::new(w, 24.0), k.mnemonic(), Act::AddOp(k))
                .font(12.0)
                .tone(Tone::Ghost)
                .tip(op_tip(k, l)),
        );
        let color = op_color(ui, k);
        ui.rect(
            p + Vec2::new(3.0 - w / 2.0, 0.0),
            Vec2::new(3.0, 14.0),
            color,
            11.5,
        );
    }
}

fn op_tip(k: OpKind, l: oo_base::Lang) -> String {
    let s = match k {
        OpKind::Mov => t(
            l,
            "MOV d, a — copy a value into register d.",
            "MOV d, a —— 把一个值复制到寄存器 d。",
        ),
        OpKind::Alu(Alu::Add) => t(l, "ADD d, a, b — d = a + b.", "ADD d, a, b —— d = a + b。"),
        OpKind::Alu(Alu::Sub) => t(l, "SUB d, a, b — d = a − b.", "SUB d, a, b —— d = a − b。"),
        OpKind::Alu(Alu::Mul) => t(l, "MUL d, a, b — d = a × b.", "MUL d, a, b —— d = a × b。"),
        OpKind::Alu(Alu::And) => t(l, "AND d, a, b — bitwise and.", "AND d, a, b —— 按位与。"),
        OpKind::Alu(Alu::Or) => t(l, "OR d, a, b — bitwise or.", "OR d, a, b —— 按位或。"),
        OpKind::Alu(Alu::Xor) => t(
            l,
            "XOR d, a, b — bitwise exclusive or.",
            "XOR d, a, b —— 按位异或。",
        ),
        OpKind::Alu(Alu::Shl) => t(
            l,
            "SHL d, a, b — shift left: a × 2^b.",
            "SHL d, a, b —— 左移：a × 2^b。",
        ),
        OpKind::Alu(Alu::Shr) => t(
            l,
            "SHR d, a, b — shift right: a ÷ 2^b, rounding down.",
            "SHR d, a, b —— 右移：a ÷ 2^b，向下取整。",
        ),
        OpKind::Alu(Alu::Min) => t(
            l,
            "MIN d, a, b — the smaller one.",
            "MIN d, a, b —— 取较小者。",
        ),
        OpKind::Alu(Alu::Max) => t(
            l,
            "MAX d, a, b — the larger one.",
            "MAX d, a, b —— 取较大者。",
        ),
        OpKind::Alu(Alu::HAdd) => t(
            l,
            "HADD d, a, b — FP16 add, rounded to half precision.",
            "HADD d, a, b —— FP16 加法，结果舍入到半精度。",
        ),
        OpKind::Alu(Alu::HSub) => t(
            l,
            "HSUB d, a, b — FP16 subtract.",
            "HSUB d, a, b —— FP16 减法。",
        ),
        OpKind::Alu(Alu::HMul) => t(
            l,
            "HMUL d, a, b — FP16 multiply. Past 65504 it becomes infinity.",
            "HMUL d, a, b —— FP16 乘法。超过 65504 就变成无穷。",
        ),
        OpKind::Alu(Alu::HMax) => t(
            l,
            "HMAX d, a, b — the larger FP16 value.",
            "HMAX d, a, b —— 取较大的 FP16 值。",
        ),
        OpKind::Alu(Alu::FAdd) => t(l, "FADD d, a, b — FP32 add.", "FADD d, a, b —— FP32 加法。"),
        OpKind::Alu(Alu::FSub) => t(
            l,
            "FSUB d, a, b — FP32 subtract.",
            "FSUB d, a, b —— FP32 减法。",
        ),
        OpKind::Alu(Alu::FMul) => t(
            l,
            "FMUL d, a, b — FP32 multiply.",
            "FMUL d, a, b —— FP32 乘法。",
        ),
        OpKind::Alu(Alu::FMax) => t(
            l,
            "FMAX d, a, b — the larger FP32 value.",
            "FMAX d, a, b —— 取较大的 FP32 值。",
        ),
        OpKind::Mma => t(
            l,
            "MMA d, a, b, c — tensor core: each half-warp is a 4×4 tile. Lane (row·4+col) holds a, b (FP16) and c, d (FP32). d = a·b + c.",
            "MMA d, a, b, c —— 张量核：每半个 warp 是一块 4×4。车道（行·4+列）持有 a、b（FP16）和 c、d（FP32）。d = a·b + c。",
        ),
        OpKind::Cvt(c) => match c {
            Cvt::HI => t(
                l,
                "CVT.H.I d, a — integer to FP16.",
                "CVT.H.I d, a —— 整数转 FP16。",
            ),
            Cvt::FI => t(
                l,
                "CVT.F.I d, a — integer to FP32.",
                "CVT.F.I d, a —— 整数转 FP32。",
            ),
            Cvt::FH => t(
                l,
                "CVT.F.H d, a — FP16 widened to FP32 (exact).",
                "CVT.F.H d, a —— FP16 扩成 FP32（无损）。",
            ),
            Cvt::HF => t(
                l,
                "CVT.H.F d, a — FP32 narrowed to FP16: rounds, may overflow to infinity.",
                "CVT.H.F d, a —— FP32 收窄成 FP16：会舍入，可能溢出为无穷。",
            ),
            Cvt::IF => t(
                l,
                "CVT.I.F d, a — FP32 to the nearest integer.",
                "CVT.I.F d, a —— FP32 取最近整数。",
            ),
            Cvt::IH => t(
                l,
                "CVT.I.H d, a — FP16 to the nearest integer.",
                "CVT.I.H d, a —— FP16 取最近整数。",
            ),
            Cvt::BI => t(
                l,
                "CVT.B.I d, a — integer to INT8, clamped to ±127.",
                "CVT.B.I d, a —— 整数转 INT8，夹在 ±127 内。",
            ),
            Cvt::BF => t(
                l,
                "CVT.B.F d, a — FP32 to INT8, rounded and clamped to ±127.",
                "CVT.B.F d, a —— FP32 转 INT8，四舍五入后夹在 ±127 内。",
            ),
            Cvt::HExp => t(
                l,
                "HEXP d, a — FP16 e^a. Big inputs overflow to infinity.",
                "HEXP d, a —— FP16 的 e^a。输入大了会溢出为无穷。",
            ),
            Cvt::HRcp => t(l, "HRCP d, a — FP16 1/a.", "HRCP d, a —— FP16 的 1/a。"),
        },
        OpKind::SetP => t(
            l,
            "SETP.cmp p, a, b — set predicate p to the comparison, per lane.",
            "SETP.cmp p, a, b —— 每条车道把比较结果写入谓词 p。",
        ),
        OpKind::Sel => t(
            l,
            "SEL d, p, a, b — d = p ? a : b. No branch, no divergence.",
            "SEL d, p, a, b —— d = p ? a : b。不分支，不发散。",
        ),
        OpKind::If => t(
            l,
            "IF p … ENDIF — lanes where p is false sit idle. The warp still issues every line.",
            "IF p … ENDIF —— p 为假的车道闲置，但整个 warp 仍要发出每一行。",
        ),
        OpKind::Else => t(
            l,
            "ELSE — the other lanes run now; the first group waits.",
            "ELSE —— 轮到另一组车道，前一组等待。",
        ),
        OpKind::Loop => t(
            l,
            "LOOP n … ENDLOOP — repeat the body n times.",
            "LOOP n … ENDLOOP —— 循环体重复 n 次。",
        ),
        OpKind::Ld => t(
            l,
            "LD d, [addr] — read the machine's ROM.",
            "LD d, [addr] —— 读取机器的 ROM。",
        ),
        OpKind::St => t(
            l,
            "ST [addr], v — write the output buffer.",
            "ST [addr], v —— 写入输出缓冲。",
        ),
        OpKind::Ldo => t(
            l,
            "LDO d, [addr] — read the output buffer (global memory, 400 cycles on a miss).",
            "LDO d, [addr] —— 读输出缓冲（全局内存，未命中要等 400 周期）。",
        ),
        OpKind::Lds => t(
            l,
            "LDS d, [addr] — read shared memory. Lanes on the same bank queue up.",
            "LDS d, [addr] —— 读共享内存。落在同一个 bank 的车道要排队。",
        ),
        OpKind::Sts => t(
            l,
            "STS [addr], v — write shared memory.",
            "STS [addr], v —— 写共享内存。",
        ),
        OpKind::Ldl => t(
            l,
            "LDL d, [addr] — read this thread's local memory (spilled values). Slow.",
            "LDL d, [addr] —— 读本线程的本地内存（溢出的值）。很慢。",
        ),
        OpKind::Stl => t(
            l,
            "STL [addr], v — write this thread's local memory.",
            "STL [addr], v —— 写本线程的本地内存。",
        ),
        OpKind::Dma => t(
            l,
            "DMA dst, src, n — copy n input words into shared memory in the background. BAR waits for it.",
            "DMA dst, src, n —— 在后台把输入的 n 个字拷进共享内存。BAR 会等它完成。",
        ),
        OpKind::Send => t(
            l,
            "SEND peer, buf, n — queue n shared words for the neighbouring SM. The link reads them later; BAR waits for it.",
            "SEND peer, buf, n —— 把共享内存里 n 个字排进发往相邻 SM 的队列。链路稍后才读缓冲;BAR 会等它发完。",
        ),
        OpKind::Recv => t(
            l,
            "RECV peer, buf, n — wait for the next message from that SM and store it at buf.",
            "RECV peer, buf, n —— 等那个 SM 的下一条消息,存进 buf。",
        ),
        OpKind::Bar => t(
            l,
            "BAR — wait until every warp of the block has arrived.",
            "BAR —— 等本块所有 warp 都到齐再往下走。",
        ),
        OpKind::Gsync => t(
            l,
            "GSYNC — wait until every block of the grid has arrived. Costs a fixed stall.",
            "GSYNC —— 等整个网格的所有块都到齐再往下走。固定停顿一段时间。",
        ),
        OpKind::Fence => t(
            l,
            "FENCE — make this warp's earlier writes visible before the later ones.",
            "FENCE —— 让本 warp 之前的写入先于之后的写入可见。",
        ),
        OpKind::Atom => t(
            l,
            "ATOM d, [addr], v — add v to the output cell atomically; d gets the old value.",
            "ATOM d, [addr], v —— 原子地给输出单元加 v，d 得到旧值。",
        ),
        OpKind::AtomS => t(
            l,
            "ATOMS d, [addr], v — the same on shared memory.",
            "ATOMS d, [addr], v —— 同上，作用于共享内存。",
        ),
        OpKind::Pix => t(
            l,
            "PIX v — write this thread's own pixel.",
            "PIX v —— 写入本线程自己的像素。",
        ),
        OpKind::Shfl => t(
            l,
            "SHFL d, a, lane — read register a from another lane of the warp.",
            "SHFL d, a, lane —— 读取同一 warp 中另一条车道的寄存器 a。",
        ),
        OpKind::Vote => t(
            l,
            "VOTE.ANY/ALL p, q — one answer for the whole warp.",
            "VOTE.ANY/ALL p, q —— 整个 warp 得到同一个答案。",
        ),
        OpKind::EndIf | OpKind::EndLoop => t(l, "Closes a block.", "结束一个块。"),
        OpKind::Edge => t(
            l,
            "EDGE d, [a] — Prism's raster unit: d = rom[a]·x + rom[a+1]·y + rom[a+2] at this pixel.",
            "EDGE d, [a] —— Prism 的光栅单元：在本像素处算 d = rom[a]·x + rom[a+1]·y + rom[a+2]。",
        ),
        OpKind::Dot => t(
            l,
            "DOT d, a, b — d = r[a]·r[b] + r[a+1]·r[b+1]: a dot product of two register pairs.",
            "DOT d, a, b —— d = r[a]·r[b] + r[a+1]·r[b+1]：两对相邻寄存器的点积。",
        ),
        OpKind::Rcp => t(
            l,
            "RCP d, a — d = 65536 / a, a Q16 reciprocal. A value of 0 gives a huge number.",
            "RCP d, a —— d = 65536 / a，Q16 倒数。输入 0 会得到一个极大的数。",
        ),
        OpKind::Lerp => t(
            l,
            "LERP d, a, b — d = (r[a]·r[b] + r[a+1]·r[b+1] + r[a+2]·r[b+2]) >> 16: three values blended by three Q16 weights.",
            "LERP d, a, b —— d = (r[a]·r[b] + r[a+1]·r[b+1] + r[a+2]·r[b+2]) >> 16：用三个 Q16 权重混合三个值。",
        ),
        OpKind::Nop => t(
            l,
            "NOP — waits one beat. Honest, but every NOP is a wasted cycle.",
            "NOP —— 空等一拍。老实，但每个 NOP 都是浪费的一个周期。",
        ),
    };
    s.to_string()
}

const PICK_W: f32 = 74.0;

fn pick(ui: &mut Ui, n: usize, cols: usize, top: f32, label: String, op: Operand, on: bool) {
    let (c, r) = ((n % cols) as f32, (n / cols) as f32);
    let w = (4.0 * (PICK_W + 4.0)) / cols as f32 - 4.0;
    let x = PX - 2.0 * (PICK_W + 4.0) + w / 2.0 + 2.0 + c * (w + 4.0);
    let b = ButtonSpec::new(
        Vec2::new(x, top - r * 24.0),
        Vec2::new(w, 21.0),
        label,
        Act::SetOperand(op),
    )
    .font(12.0)
    .selected(on);
    ui.button(b);
}

fn bumps(ui: &mut Ui, y: f32, value: String) {
    let pal = ui.pal;
    for (n, d) in [-10, -1, 1, 10].into_iter().enumerate() {
        let x = PX + [-132.0, -82.0, 82.0, 132.0][n];
        let label = if d > 0 {
            format!("+{d}")
        } else {
            format!("−{}", -d)
        };
        ui.button(
            ButtonSpec::new(Vec2::new(x, y), Vec2::new(46.0, 21.0), label, Act::Bump(d)).font(12.0),
        );
    }
    ui.text(value, Vec2::new(PX, y), 15.0, pal.secondary, Anchor::CENTER);
}

/// The picker for the selected operand, or line actions for a selected line.
fn draw_side(
    ui: &mut Ui,
    v: &View,
    d: &Doc,
    ops: &[OpKind],
    _probe: Option<ooc_core::grade::Probe>,
) {
    let pal = ui.pal;
    let l = v.lang;
    let top = 104.0;
    let head = Vec2::new(PX - 158.0, 128.0);
    let (line, k) = match v.s.sel {
        Sel::Operand(i, k) => (i, Some(k)),
        Sel::Line(i) => (i, None),
        _ => {
            ui.caption(
                t(l, "EDIT", "编辑"),
                head,
                pal.text_faint,
                Anchor::CENTER_LEFT,
            );
            ui.wrapped(
                t(l, "Pick an instruction above to append it. Click a line to select it, a chip to change it.\nDigits type a number · Alt+↑/↓ moves a line\nDelete removes it.", "点上方的指令把它加到程序里。点一行选中它，点一个格子修改它。\n数字键输入数值 · Alt+↑/↓ 移动一行\nDelete 删除所选行。"),
                Vec2::new(PX - 158.0, top + 6.0), 310.0, 13.0, pal.text_dim, Anchor::TOP_LEFT,
            );
            return;
        }
    };
    let Some(ins) = d.code.get(line) else { return };
    let Some(k) = k else {
        ui.caption(
            format!("{} {}", t(l, "LINE", "第几行："), line + 1),
            head,
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
        ui.wrapped(
            op_tip(ins.kind(), l),
            Vec2::new(PX - 158.0, top + 6.0),
            310.0,
            13.0,
            pal.text_dim,
            Anchor::TOP_LEFT,
        );
        let y = top - 76.0;
        ui.button(
            ButtonSpec::new(
                Vec2::new(PX - 112.0, y),
                Vec2::new(86.0, 26.0),
                t(l, "↑ Up", "↑ 上移"),
                Act::MoveLine(true),
            )
            .font(13.0),
        );
        ui.button(
            ButtonSpec::new(
                Vec2::new(PX - 18.0, y),
                Vec2::new(86.0, 26.0),
                t(l, "↓ Down", "↓ 下移"),
                Act::MoveLine(false),
            )
            .font(13.0),
        );
        ui.button(
            ButtonSpec::new(
                Vec2::new(PX + 90.0, y),
                Vec2::new(110.0, 26.0),
                t(l, "Delete", "删除"),
                Act::DeleteSel,
            )
            .font(13.0),
        );
        if ins.opens() || ins.closes() {
            ui.wrapped(
                t(
                    l,
                    "Deleting a block keeps its body.",
                    "删除一个块会保留里面的内容。",
                ),
                Vec2::new(PX - 158.0, y - 22.0),
                310.0,
                12.0,
                pal.text_faint,
                Anchor::TOP_LEFT,
            );
        }
        return;
    };
    let Some(cur) = operands(ins).get(k).copied() else {
        return;
    };
    ui.caption(
        format!(
            "{} {} · {}",
            t(l, "LINE", "第几行："),
            line + 1,
            slot_name(ins, k, l)
        ),
        head,
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let nreg = v.s.task().profile.max_regs();
    let rcols = if nreg > BASE_REGS { 6 } else { 4 };
    let sp_base = (nreg as usize).div_ceil(6) * 6 + 6;
    match cur {
        Operand::Reg(r) => {
            for i in 0..nreg {
                pick(
                    ui,
                    i as usize,
                    rcols,
                    top,
                    format!("r{i}"),
                    Operand::Reg(i),
                    r == i,
                );
            }
        }
        Operand::Pred(p) => {
            for i in 0..PREDS {
                pick(
                    ui,
                    i as usize,
                    4,
                    top,
                    format!("p{i}"),
                    Operand::Pred(i),
                    p == i,
                );
            }
        }
        Operand::Cmp(c) => {
            for (i, x) in Cmp::ALL.into_iter().enumerate() {
                pick(ui, i, 3, top, cmp_label(x).into(), Operand::Cmp(x), c == x);
            }
        }
        Operand::Mode(m) => {
            pick(ui, 0, 2, top, "ANY".into(), Operand::Mode(false), !m);
            pick(ui, 1, 2, top, "ALL".into(), Operand::Mode(true), m);
        }
        Operand::Op(o) => {
            let alus = ops.iter().filter_map(|k| match k {
                OpKind::Alu(a) => Some(*a),
                _ => None,
            });
            for (i, a) in alus.enumerate() {
                pick(ui, i, 4, top, a.name().into(), Operand::Op(a), o == a);
            }
        }
        Operand::Count(n) => {
            bumps(ui, top, n.to_string());
            ui.text(
                t(
                    l,
                    "Times the body repeats (0–64).",
                    "循环体重复的次数（0–64）。",
                ),
                Vec2::new(PX - 158.0, top - 30.0),
                12.0,
                pal.text_faint,
                Anchor::CENTER_LEFT,
            );
        }
        Operand::Src(src) => {
            for i in 0..nreg {
                pick(
                    ui,
                    i as usize,
                    rcols,
                    top,
                    format!("r{i}"),
                    Operand::Src(Src::Reg(i)),
                    src == Src::Reg(i),
                );
            }
            for (i, sp) in Special::ALL.into_iter().enumerate() {
                pick(
                    ui,
                    sp_base + i,
                    6,
                    top - 2.0,
                    sp.name().into(),
                    Operand::Src(Src::Sp(sp)),
                    src == Src::Sp(sp),
                );
            }
            let prof = v.s.task().profile;
            let vault = prof.vault;
            let grid_n = if prof.atlas { Special::GRID.len() } else { 0 };
            for (i, sp) in Special::GRID.into_iter().take(grid_n).enumerate() {
                pick(
                    ui,
                    sp_base + 6 + i,
                    6,
                    top - 2.0,
                    sp.name().into(),
                    Operand::Src(Src::Sp(sp)),
                    src == Src::Sp(sp),
                );
            }
            if vault {
                pick(
                    ui,
                    sp_base + 6 + grid_n,
                    6,
                    top - 2.0,
                    Special::Clock.name().into(),
                    Operand::Src(Src::Sp(Special::Clock)),
                    src == Src::Sp(Special::Clock),
                );
            }
            let rows = if vault {
                (sp_base + 6 + grid_n) / 6 + 1
            } else {
                sp_base / 6 + 1
            };
            let y = top - rows as f32 * 24.0 - 4.0;
            match src {
                Src::Imm(v) => bumps(ui, y, v.to_string()),
                _ => {
                    let b = ButtonSpec::new(
                        Vec2::new(PX, y),
                        Vec2::new(PICK_W * 2.0, 21.0),
                        t(l, "0-9 a number", "0-9 一个数"),
                        Act::SetOperand(Operand::Src(Src::Imm(0))),
                    )
                    .font(12.0);
                    ui.button(b);
                }
            }
        }
    }
}

fn cmp_label(c: Cmp) -> &'static str {
    match c {
        Cmp::Lt => "LT  <",
        Cmp::Le => "LE  ≤",
        Cmp::Eq => "EQ  =",
        Cmp::Ne => "NE  ≠",
        Cmp::Ge => "GE  ≥",
        Cmp::Gt => "GT  >",
    }
}

/// What operand slot `k` of `ins` means, in words.
fn slot_name(ins: &ooc_core::isa::Instr, k: usize, l: oo_base::Lang) -> &'static str {
    use ooc_core::isa::Instr as I;
    match (ins, k) {
        (I::Alu { .. }, 0) => t(l, "OPERATION", "运算"),
        (I::SetP { .. }, 0) => t(l, "COMPARISON", "比较"),
        (I::Vote { .. }, 0) => t(l, "MODE", "模式"),
        (I::Loop { .. }, 0) => t(l, "COUNT", "次数"),
        (I::St { .. }, 0) | (I::Ld { .. }, 1) => t(l, "ADDRESS", "地址"),
        (I::Shfl { .. }, 2) => t(l, "FROM LANE", "来源车道"),
        (I::Send { .. } | I::Recv { .. }, 0) => t(l, "PEER SM", "对端 SM"),
        (I::Send { .. } | I::Recv { .. }, 1) => t(l, "BUFFER", "缓冲地址"),
        (I::Send { .. } | I::Recv { .. }, 2) => t(l, "WORDS", "字数"),
        (I::Mov { .. } | I::Sel { .. } | I::Ld { .. } | I::Shfl { .. }, 0) | (I::Alu { .. }, 1) => {
            t(l, "DESTINATION", "目标寄存器")
        }
        (I::SetP { .. } | I::Sel { .. }, 1) | (I::If { .. }, 0) | (I::Vote { .. }, _) => {
            t(l, "PREDICATE", "谓词")
        }
        _ => t(l, "VALUE", "数值"),
    }
}
