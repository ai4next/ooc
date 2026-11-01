//! ORI's handbook, opened from the hub: the OC-1 ops and blueprint nodes,
//! the six machines, the observation tools, and every level's concept card.
//! What the player has not reached yet stays greyed or locked.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_base::Lang;
use oo_kit::ui::{Actions, ButtonSpec, Tone, Ui};
use ooc_core::content::{self, manual};
use ooc_core::graph::NodeKind;
use ooc_core::isa::OpKind;
use ooc_core::save::SaveData;
use ooc_core::task::{CHAPTERS, Task};
use ooc_core::vm::SMS;

use crate::state::{Act, t};
use crate::{Ctx, View, hub, results};

const SECTIONS: u8 = 4;

/// The page card every section but the concept cards draws on.
const PAGE: (Vec2, Vec2) = (Vec2::new(0.0, -27.0), Vec2::new(1240.0, 530.0));

pub fn act(cx: &mut Ctx, a: Act) {
    let (sec, page) = cx.s.manual;
    cx.s.manual = match a {
        Act::ManualSection(n) => (n.min(SECTIONS - 1), 0),
        Act::ManualPage(d) => (sec, page.saturating_add_signed(d as isize)),
        Act::ManualAt(n) => (sec, n),
        Act::Lab(_) => return crate::hub::act(cx, a),
        _ => return,
    };
    let last = pages(cx.s.manual.0).saturating_sub(1);
    cx.s.manual.1 = cx.s.manual.1.min(last);
    cx.dirty();
}

/// ←/→ and PageUp/PageDown turn pages, 1–4 pick a section, the wheel turns
/// one page per notch (a trackpad flick turns one, not ten).
pub fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    time: Res<Time>,
    mut cool: Local<f32>,
    mut actions: ResMut<Actions<Act>>,
) {
    for (k, d) in [
        (KeyCode::ArrowLeft, -1),
        (KeyCode::PageUp, -1),
        (KeyCode::ArrowRight, 1),
        (KeyCode::PageDown, 1),
    ] {
        if keys.just_pressed(k) {
            actions.push(Act::ManualPage(d));
        }
    }
    for (n, k) in [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
    ]
    .into_iter()
    .enumerate()
    {
        if keys.just_pressed(k) {
            actions.push(Act::ManualSection(n as u8));
        }
    }
    *cool = (*cool - time.delta_secs()).max(0.0);
    let wheel = scroll.delta.y;
    if wheel.abs() > 0.5 && *cool == 0.0 {
        actions.push(Act::ManualPage(if wheel > 0.0 { -1 } else { 1 }));
        *cool = 0.25;
    }
}

/// Pages in section `sec`.
fn pages(sec: u8) -> usize {
    match sec {
        0 => isa_pages().len(),
        1 => content::machines().len(),
        2 => manual::TOOLS.len().div_ceil(TOOLS_PER_PAGE),
        _ => content::tasks().len(),
    }
}

fn section_name(sec: u8, l: Lang) -> &'static str {
    match sec {
        0 => t(l, "Instruction set", "指令集"),
        1 => t(l, "Machines", "机器"),
        2 => t(l, "Observation tools", "观察工具"),
        _ => t(l, "Concept cards", "洞见卡"),
    }
}

/// The level that brings an op, node or tool is open to the player.
fn reached(task: Option<&Task>, save: &SaveData) -> bool {
    task.is_some_and(|t| save.is_unlocked(t))
}

pub fn draw_manual(ui: &mut Ui, v: &View) {
    let l = v.lang;
    let pal = ui.pal;
    let save = &v.game.save;
    let sec = v.s.manual.0.min(SECTIONS - 1);
    let total = pages(sec);
    let page = v.s.manual.1.min(total - 1);
    ui.text_spaced(
        t(l, "ORI'S HANDBOOK", "ORI 手册"),
        Vec2::new(-600.0, 318.0),
        24.0,
        pal.text,
        Anchor::CENTER_LEFT,
        3.0,
    );
    let (label, have, of) = progress(sec, save, l);
    ui.pill(
        Vec2::new(400.0, 318.0),
        200.0,
        label,
        format!("{have} / {of}"),
        pal.primary,
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(560.0, 318.0),
            Vec2::new(76.0, 34.0),
            l.switch_label(),
            Act::ToggleLang,
        )
        .font(14.0),
    );
    for s in 0..SECTIONS {
        let mut b = ButtonSpec::new(
            Vec2::new(-505.0 + s as f32 * 200.0, 262.0),
            Vec2::new(190.0, 30.0),
            format!("{}  {}", s + 1, section_name(s, l)),
            Act::ManualSection(s),
        )
        .font(14.0)
        .selected(s == sec);
        if s != sec {
            b = b.tone(Tone::Ghost);
        }
        ui.button(b);
    }
    match sec {
        0 => draw_isa(ui, v, page),
        1 => draw_machine(ui, v, page),
        2 => draw_tools(ui, v, page),
        _ => draw_concept(ui, v, page),
    }
    // Bottom row: page turning in the middle, the way back on the right.
    ui.text(
        t(
            l,
            "← → or the wheel turn pages · 1–4 pick a section",
            "← → 或滚轮翻页 · 1–4 切换栏目",
        ),
        Vec2::new(-600.0, -330.0),
        13.0,
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(-110.0, -330.0),
            Vec2::new(110.0, 34.0),
            t(l, "‹ Prev", "‹ 上一页"),
            Act::ManualPage(-1),
        )
        .tone(Tone::Secondary)
        .font(14.0)
        .enabled(page > 0),
    );
    ui.text(
        format!("{} / {}", page + 1, total),
        Vec2::new(0.0, -330.0),
        15.0,
        pal.text_dim,
        Anchor::CENTER,
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(110.0, -330.0),
            Vec2::new(110.0, 34.0),
            t(l, "Next ›", "下一页 ›"),
            Act::ManualPage(1),
        )
        .tone(Tone::Secondary)
        .font(14.0)
        .enabled(page + 1 < total),
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(560.0, -330.0),
            Vec2::new(140.0, 34.0),
            t(l, "‹ Grid Hall", "‹ 栅厅"),
            Act::Back,
        )
        .tone(Tone::Ghost)
        .font(14.0),
    );
}

/// The pill in the corner: how much of this section the player has reached.
fn progress(sec: u8, save: &SaveData, l: Lang) -> (&'static str, usize, usize) {
    match sec {
        0 => {
            let ops = manual::all_ops();
            let have = ops
                .iter()
                .filter(|&&k| reached(manual::op_level(k), save))
                .count();
            (t(l, "OPS UNLOCKED", "已解锁指令"), have, ops.len())
        }
        1 => {
            let ms = content::machines();
            let have = ms
                .iter()
                .filter(|m| hub::chapter_open(m.first, save))
                .count();
            (t(l, "REBUILT", "已复刻"), have, ms.len())
        }
        2 => {
            let n = manual::TOOLS.len();
            let have = (0..n)
                .filter(|&i| reached(manual::tool_level(i), save))
                .count();
            (t(l, "TOOLS OPEN", "已开放工具"), have, n)
        }
        _ => {
            let ts = content::tasks();
            let have = ts.iter().filter(|t| save.campaign.is_solved(t.id)).count();
            (t(l, "CARDS", "洞见卡"), have, ts.len())
        }
    }
}

/// `3-4`: chapter and slot.
fn level_no(t: &Task) -> String {
    format!("{}-{}", t.chapter, t.slot)
}

/// Rough rendered width, for deciding how many lines a value wraps to.
fn est_width(s: &str, size: f32) -> f32 {
    s.chars()
        .map(|c| if c.is_ascii() { 0.5 } else { 1.0 })
        .sum::<f32>()
        * size
}

fn est_lines(s: &str, size: f32, width: f32) -> usize {
    ((est_width(s, size) / width).ceil() as usize).max(1)
}

// ---- Section 1: the instruction set ----

enum IsaPage {
    Operands,
    Nodes,
    Groups(Vec<usize>),
}

const ROW_H: f32 = 36.0;
const HEAD_H: f32 = 40.0;
/// Height the op rows of one page may fill.
const ISA_ROOM: f32 = 480.0;

/// Operands and nodes first, then the op groups packed onto pages whole.
fn isa_pages() -> Vec<IsaPage> {
    let mut v = vec![IsaPage::Operands, IsaPage::Nodes];
    let (mut cur, mut h) = (Vec::new(), 0.0);
    for (i, g) in manual::GROUPS.iter().enumerate() {
        let gh = HEAD_H + g.ops.len() as f32 * ROW_H;
        if !cur.is_empty() && h + gh > ISA_ROOM {
            v.push(IsaPage::Groups(std::mem::take(&mut cur)));
            h = 0.0;
        }
        cur.push(i);
        h += gh;
    }
    if !cur.is_empty() {
        v.push(IsaPage::Groups(cur));
    }
    v
}

/// Column left edges of an op row: syntax, meaning, latency, unlock.
const COLS: [f32; 4] = [-584.0, -350.0, 222.0, 428.0];

fn draw_isa(ui: &mut Ui, v: &View, page: usize) {
    let pal = ui.pal;
    let (at, size) = PAGE;
    ui.card(at, size, pal.secondary);
    match &isa_pages()[page] {
        IsaPage::Operands => draw_operands(ui, v),
        IsaPage::Nodes => draw_nodes(ui, v),
        IsaPage::Groups(gs) => {
            let l = v.lang;
            let top = at.y + size.y / 2.0;
            let heads = [
                t(l, "SYNTAX", "写法"),
                t(l, "WHAT IT DOES", "作用"),
                t(l, "LATENCY", "延迟"),
                t(l, "UNLOCKS", "解锁"),
            ];
            for (x, h) in COLS.iter().zip(heads) {
                ui.caption(
                    h,
                    Vec2::new(*x, top - 26.0),
                    pal.text_faint,
                    Anchor::CENTER_LEFT,
                );
            }
            ui.rect(
                Vec2::new(0.0, top - 40.0),
                Vec2::new(size.x - 60.0, 1.0),
                pal.line,
                6.0,
            );
            let mut y = top - 62.0;
            for &g in gs {
                let g = &manual::GROUPS[g];
                ui.text(
                    g.title.get(l),
                    Vec2::new(COLS[0], y),
                    16.0,
                    pal.text,
                    Anchor::CENTER_LEFT,
                );
                y -= HEAD_H - 4.0;
                for (n, &k) in g.ops.iter().enumerate() {
                    draw_op(ui, v, k, y, n % 2 == 0);
                    y -= ROW_H;
                }
                y -= 4.0;
            }
        }
    }
}

fn draw_op(ui: &mut Ui, v: &View, k: OpKind, y: f32, stripe: bool) {
    let l = v.lang;
    let pal = ui.pal;
    let d = manual::op_doc(k);
    let lv = manual::op_level(k);
    let open = reached(lv, &v.game.save);
    if stripe {
        ui.rect(
            Vec2::new(0.0, y),
            Vec2::new(1196.0, ROW_H - 2.0),
            pal.glass_raised.with_alpha(0.45),
            5.0,
        );
    }
    let (ink, dim) = if open {
        (pal.secondary, pal.text)
    } else {
        (pal.text_faint, pal.text_faint)
    };
    ui.text(
        d.syntax,
        Vec2::new(COLS[0], y),
        15.0,
        ink,
        Anchor::CENTER_LEFT,
    );
    ui.wrapped(
        d.meaning.get(l),
        Vec2::new(COLS[1], y),
        COLS[2] - COLS[1] - 24.0,
        14.0,
        dim,
        Anchor::CENTER_LEFT,
    );
    if let Some(s) = manual::latency(k, l) {
        let c = if open { pal.text_dim } else { pal.text_faint };
        ui.text(s, Vec2::new(COLS[2], y), 13.0, c, Anchor::CENTER_LEFT);
    }
    draw_unlock(ui, lv, open, Vec2::new(COLS[3], y), l);
}

/// `chapter 2 · 2-1`, or the chapter still to come.
fn draw_unlock(ui: &mut Ui, lv: Option<&Task>, open: bool, at: Vec2, l: Lang) {
    let pal = ui.pal;
    let Some(lv) = lv else { return };
    if open {
        let s = t(l, "chapter {} · {}", "第 {} 章 · {}");
        let s = oo_base::fill(s, &[&lv.chapter, &level_no(lv)]);
        ui.dot(at + Vec2::new(4.0, 0.0), 6.0, pal.primary, 8.0);
        ui.text(
            s,
            at + Vec2::new(14.0, 0.0),
            13.0,
            pal.text_dim,
            Anchor::CENTER_LEFT,
        );
    } else {
        let s = oo_base::fill(
            t(l, "unlocks in chapter {}", "第 {} 章解锁"),
            &[&lv.chapter],
        );
        ui.chip(
            at + Vec2::new(4.0, 0.0),
            Vec2::splat(7.0),
            pal.text_faint,
            8.0,
        );
        ui.text(
            s,
            at + Vec2::new(14.0, 0.0),
            13.0,
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
    }
}

fn draw_operands(ui: &mut Ui, v: &View) {
    let l = v.lang;
    let pal = ui.pal;
    let (at, size) = PAGE;
    let top = at.y + size.y / 2.0;
    let left = at.x - size.x / 2.0 + 36.0;
    ui.caption(
        t(
            l,
            "OC-1 · ONE ISA FOR EVERY MACHINE",
            "OC-1 · 每台机器都跑这一套",
        ),
        Vec2::new(left, top - 30.0),
        pal.secondary,
        Anchor::CENTER_LEFT,
    );
    ui.text(
        t(l, "Operands", "操作数"),
        Vec2::new(left, top - 58.0),
        22.0,
        pal.text,
        Anchor::CENTER_LEFT,
    );
    ui.wrapped(
        t(
            l,
            "A kernel runs once per thread: every pixel or array element gets its own copy of the registers. There are no jumps, only IF and LOOP, so every program draws as an even band on the Warp River.",
            "内核对每个线程各跑一次：每个像素或数组元素都有自己的一套寄存器。没有跳转，只有 IF 和 LOOP，所以每段程序在 Warp River 上都是整齐的带。",
        ),
        Vec2::new(left, top - 84.0),
        1160.0,
        15.0,
        pal.text_dim,
        Anchor::TOP_LEFT,
    );
    let mut y = top - 150.0;
    for (n, (form, what)) in manual::OPERANDS.iter().enumerate() {
        if n % 2 == 0 {
            ui.rect(
                Vec2::new(0.0, y),
                Vec2::new(1196.0, 34.0),
                pal.glass_raised.with_alpha(0.45),
                5.0,
            );
        }
        ui.text(
            *form,
            Vec2::new(left, y),
            15.0,
            pal.secondary,
            Anchor::CENTER_LEFT,
        );
        ui.text(
            what.get(l),
            Vec2::new(left + 300.0, y),
            14.0,
            pal.text,
            Anchor::CENTER_LEFT,
        );
        y -= 36.0;
    }
    // The design's first kernel, as a sample of the notation.
    let w = Vec2::new(left + 380.0, y - 70.0);
    ui.well(w, Vec2::new(760.0, 96.0));
    ui.caption(
        t(l, "A FIRST KERNEL", "第一个内核"),
        w + Vec2::new(-364.0, 34.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let lines = [
        (
            "MUL  r1, x, 16",
            t(l, "; brightness = x × 16", "; 亮度 = x × 16"),
        ),
        ("PIX  r1", t(l, "; write own pixel", "; 写到自己的像素")),
    ];
    for (i, (code, note)) in lines.into_iter().enumerate() {
        let p = w + Vec2::new(-364.0, 6.0 - i as f32 * 26.0);
        ui.text(code, p, 15.0, pal.secondary, Anchor::CENTER_LEFT);
        ui.text(
            note,
            p + Vec2::new(200.0, 0.0),
            14.0,
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
    }
    ui.wrapped(
        t(
            l,
            "Greyed ops are not open yet: the line says which chapter brings them.",
            "灰色的指令还没开放：行尾写着哪一章解锁。",
        ),
        Vec2::new(left + 790.0, y - 70.0),
        360.0,
        14.0,
        pal.text_dim,
        Anchor::CENTER_LEFT,
    );
}

fn draw_nodes(ui: &mut Ui, v: &View) {
    let l = v.lang;
    let pal = ui.pal;
    let (at, size) = PAGE;
    let top = at.y + size.y / 2.0;
    let left = at.x - size.x / 2.0 + 36.0;
    ui.caption(
        t(
            l,
            "BLUEPRINT · WIRED, THEN COMPILED TO OC-1",
            "蓝图 · 先接线，再编译成 OC-1",
        ),
        Vec2::new(left, top - 30.0),
        pal.secondary,
        Anchor::CENTER_LEFT,
    );
    ui.text(
        t(l, "Blueprint nodes", "蓝图节点"),
        Vec2::new(left, top - 58.0),
        22.0,
        pal.text,
        Anchor::CENTER_LEFT,
    );
    ui.wrapped(
        t(
            l,
            "Every pixel runs the same circuit with its own x and y. An unwired input is a constant you can edit. The editor shows the same circuit as code.",
            "每个像素都跑同一块电路，只是 x、y 各不相同。没接线的输入是一个可改的常数。编辑器会把同一块电路显示成代码。",
        ),
        Vec2::new(left, top - 84.0),
        1160.0,
        15.0,
        pal.text_dim,
        Anchor::TOP_LEFT,
    );
    let half = manual::NODES.len().div_ceil(2);
    for (i, &k) in manual::NODES.iter().enumerate() {
        let (col, row) = (i / half, i % half);
        let x = left + col as f32 * 600.0;
        let y = top - 156.0 - row as f32 * 50.0;
        draw_node(ui, v, k, Vec2::new(x, y));
    }
}

fn draw_node(ui: &mut Ui, v: &View, k: NodeKind, at: Vec2) {
    let l = v.lang;
    let pal = ui.pal;
    let lv = manual::node_level(k);
    let open = reached(lv, &v.game.save);
    let ink = if open { pal.secondary } else { pal.text_faint };
    // A node block like the editor's, with its input ports on the left.
    let block = Vec2::new(80.0, 34.0);
    let c = at + Vec2::new(block.x / 2.0 + 10.0, 0.0);
    ui.chip(c, block, pal.glass_raised, 6.0);
    ui.rect(
        c + Vec2::new(-block.x / 2.0 + 1.5, 0.0),
        Vec2::new(3.0, block.y - 8.0),
        ink,
        6.5,
    );
    ui.text(k.label(), c, 14.0, ink, Anchor::CENTER);
    let ports = k.ports();
    for (j, _) in ports.iter().enumerate() {
        let py = (ports.len() as f32 - 1.0) / 2.0 * 10.0 - j as f32 * 10.0;
        ui.dot(
            at + Vec2::new(8.0, py),
            6.0,
            pal.primary.with_alpha(if open { 0.9 } else { 0.3 }),
            7.0,
        );
    }
    if !k.is_source() || k == NodeKind::Out {
        ui.caption(
            ports.join(" "),
            at + Vec2::new(100.0, 9.0),
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
    }
    let text = if open { pal.text } else { pal.text_faint };
    ui.wrapped(
        manual::node_doc(k).get(l),
        at + Vec2::new(100.0, -6.0),
        250.0,
        14.0,
        text,
        Anchor::CENTER_LEFT,
    );
    draw_unlock(ui, lv, open, at + Vec2::new(370.0, 0.0), l);
}

// ---- Section 2: the machines ----

fn draw_machine(ui: &mut Ui, v: &View, i: usize) {
    let l = v.lang;
    let pal = ui.pal;
    let m = &content::machines()[i];
    let open = hub::chapter_open(m.first, &v.game.save);
    let accent = hub::chapter_accent(content::chapter(m.first), &pal);
    let (at, size) = PAGE;
    ui.card(at, size, if open { accent } else { pal.text_faint });
    let top = at.y + size.y / 2.0;
    let left = at.x - size.x / 2.0 + 36.0;
    let chapters = oo_base::fill(
        t(
            l,
            "GENERATION {} · CHAPTERS {}–{}",
            "第 {} 代 · 第 {}–{} 章",
        ),
        &[&(i + 1), &m.first, &m.last],
    );
    ui.caption(
        chapters,
        Vec2::new(left, top - 30.0),
        if open { accent } else { pal.text_faint },
        Anchor::CENTER_LEFT,
    );
    ui.text(
        m.name.get(l),
        Vec2::new(left, top - 58.0),
        22.0,
        if open { pal.text } else { pal.text_dim },
        Anchor::CENTER_LEFT,
    );
    let board = (
        Vec2::new(at.x + 300.0, top - 200.0),
        Vec2::new(540.0, 220.0),
    );
    if !open {
        pattern_board(ui, i as u32 + 1, board.0, board.1);
        ui.text(
            t(l, "NOT REBUILT YET", "尚未复刻"),
            Vec2::new(left, top - 120.0),
            18.0,
            pal.text_dim,
            Anchor::CENTER_LEFT,
        );
        ui.wrapped(
            oo_base::fill(
                t(
                    l,
                    "Its table in the Grid Hall holds only the pattern board traced from the stone. Chapter {} rebuilds it.",
                    "栅厅里它的复刻台上只有一块照着石头描的图样板。第 {} 章会把它复刻出来。",
                ),
                &[&m.first],
            ),
            Vec2::new(left, top - 150.0),
            520.0,
            15.0,
            pal.text_faint,
            Anchor::TOP_LEFT,
        );
        return;
    }
    // Left: the numbers, read from the machine profile.
    let mut y = top - 104.0;
    for (k, val) in manual::machine_facts(i, l) {
        let lines = est_lines(&val, 14.0, 400.0);
        ui.text(
            k.get(l),
            Vec2::new(left, y),
            13.0,
            pal.text_faint,
            Anchor::TOP_LEFT,
        );
        ui.wrapped(
            val,
            Vec2::new(left + 150.0, y),
            400.0,
            14.0,
            pal.text,
            Anchor::TOP_LEFT,
        );
        y -= 18.0 * lines as f32 + 14.0;
    }
    // Right: a drawing, then what is new and what goes wrong.
    ui.well(board.0, board.1);
    draw_diagram(ui, i, board.0, accent);
    let d = &manual::MACHINE_DOCS[i];
    let rx = board.0.x - board.1.x / 2.0;
    let mut y = board.0.y - board.1.y / 2.0 - 22.0;
    for (head, body, c) in [
        (t(l, "WHAT IT ADDS", "新增的部件"), d.units, pal.primary),
        (
            t(l, "THE MISTAKES IT BRINGS", "带来的新错误"),
            d.mistakes,
            pal.warn,
        ),
    ] {
        let h = 26.0 + 17.0 * est_lines(body.get(l), 14.0, 520.0).max(2) as f32;
        ui.rect(
            Vec2::new(rx + 1.5, y + 8.0 - h / 2.0),
            Vec2::new(3.0, h),
            c,
            8.0,
        );
        ui.caption(head, Vec2::new(rx + 14.0, y), c, Anchor::CENTER_LEFT);
        ui.wrapped(
            body.get(l),
            Vec2::new(rx + 14.0, y - 14.0),
            520.0,
            14.0,
            pal.text_dim,
            Anchor::TOP_LEFT,
        );
        y -= h + 20.0;
    }
}

/// A hash per cell, so the same board always shows the same pattern.
fn traced(n: u32, i: u32) -> u32 {
    let mut x = n.wrapping_mul(0x9E37_79B9) ^ i.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^ (x >> 12)
}

/// The grey pattern board an empty table holds.
fn pattern_board(ui: &mut Ui, n: u32, at: Vec2, size: Vec2) {
    let pal = ui.pal;
    ui.well(at, size);
    let (cols, rows) = (20u32, 8u32);
    let cell = Vec2::new((size.x - 40.0) / cols as f32, (size.y - 40.0) / rows as f32);
    let origin = at - size / 2.0 + Vec2::splat(20.0) + cell / 2.0;
    for r in 0..rows {
        for c in 0..cols {
            let h = traced(n, r * cols + c);
            if h % 5 >= 2 && !(c % (2 + n % 4) == 0 && r % 2 == 0) {
                continue;
            }
            let p = origin + Vec2::new(c as f32 * cell.x, r as f32 * cell.y);
            ui.chip(
                p,
                cell - Vec2::splat(5.0),
                pal.text_faint.with_alpha(0.14 + (h % 3) as f32 * 0.04),
                7.0,
            );
        }
    }
}

/// A labelled box: an SM, a cache, a memory.
fn unit(ui: &mut Ui, at: Vec2, size: Vec2, label: &str, ink: Color) {
    let pal = ui.pal;
    ui.chip(at, size, pal.glass_raised, 6.0);
    ui.rect(
        at + Vec2::new(0.0, size.y / 2.0 - 1.0),
        Vec2::new(size.x - 8.0, 2.0),
        ink.with_alpha(0.7),
        6.5,
    );
    ui.caption(
        label,
        at + Vec2::new(-size.x / 2.0 + 8.0, size.y / 2.0 - 12.0),
        ink,
        Anchor::CENTER_LEFT,
    );
}

/// A row of 32 lanes.
fn lanes(ui: &mut Ui, at: Vec2, w: f32, h: f32, ink: Color, gap: usize) {
    let step = w / 32.0;
    for k in 0..32 {
        let on = gap == 0 || k % gap != 0;
        let c = if on {
            ink.with_alpha(0.8)
        } else {
            ink.with_alpha(0.2)
        };
        ui.rect(
            at + Vec2::new(-w / 2.0 + step * (k as f32 + 0.5), 0.0),
            Vec2::new(step - 1.5, h),
            c,
            7.0,
        );
    }
}

/// A small drawing of machine `i` inside a 540 × 220 well centred on `at`.
fn draw_diagram(ui: &mut Ui, i: usize, at: Vec2, accent: Color) {
    let pal = ui.pal;
    match i {
        0 => {
            // A 16×16 screen, one thread per pixel, fed by 8 warps of 32 lanes.
            let o = at + Vec2::new(-170.0, 0.0);
            for y in 0..16 {
                for x in 0..16 {
                    let v = (x * 16) as f32 / 255.0;
                    let p = o + Vec2::new((x as f32 - 7.5) * 10.0, (7.5 - y as f32) * 10.0);
                    ui.rect(
                        p,
                        Vec2::splat(8.5),
                        pal.secondary.with_alpha(0.15 + 0.8 * v),
                        7.0,
                    );
                }
            }
            for w in 0..8 {
                let p = at + Vec2::new(110.0, 70.0 - w as f32 * 20.0);
                lanes(ui, p, 240.0, 12.0, accent, if w == 3 { 2 } else { 0 });
                ui.caption(
                    format!("w{w}"),
                    p + Vec2::new(-136.0, 0.0),
                    pal.text_faint,
                    Anchor::CENTER_RIGHT,
                );
            }
        }
        1 => {
            // A triangle over the screen, and the beats a result takes.
            let o = at + Vec2::new(-150.0, 0.0);
            ui.rect(o, Vec2::splat(170.0), pal.bg_deep, 6.0);
            let tri = [
                o + Vec2::new(-70.0, -65.0),
                o + Vec2::new(70.0, -40.0),
                o + Vec2::new(-10.0, 72.0),
            ];
            for k in 0..3 {
                ui.line(tri[k], tri[(k + 1) % 3], 2.0, accent, 7.5);
            }
            for y in 0..8 {
                for x in 0..8 {
                    let p = o + Vec2::new((x as f32 - 3.5) * 20.0, (y as f32 - 3.5) * 20.0);
                    ui.dot(p, 4.0, pal.text_faint.with_alpha(0.5), 7.0);
                }
            }
            let stages = ["ISSUE", "+1", "+2", "LAND"];
            for (k, s) in stages.iter().enumerate() {
                let p = at + Vec2::new(60.0 + k as f32 * 56.0, 30.0);
                let c = if k == 3 { pal.primary } else { accent };
                ui.chip(p, Vec2::new(48.0, 30.0), c.with_alpha(0.25), 7.0);
                ui.caption(*s, p, c, Anchor::CENTER);
            }
            ui.chip(
                at + Vec2::new(116.0, -24.0),
                Vec2::new(48.0, 30.0),
                pal.bad.with_alpha(0.35),
                7.0,
            );
            ui.caption(
                "READ",
                at + Vec2::new(116.0, -24.0),
                pal.bad,
                Anchor::CENTER,
            );
            ui.caption("RAW", at + Vec2::new(116.0, -54.0), pal.bad, Anchor::CENTER);
        }
        2 => {
            // An SM, its L1 and shared banks, and the far global memory.
            unit(
                ui,
                at + Vec2::new(-170.0, 30.0),
                Vec2::new(170.0, 130.0),
                "SM",
                accent,
            );
            for w in 0..4 {
                lanes(
                    ui,
                    at + Vec2::new(-170.0, 50.0 - w as f32 * 20.0),
                    140.0,
                    10.0,
                    accent,
                    0,
                );
            }
            unit(
                ui,
                at + Vec2::new(10.0, 60.0),
                Vec2::new(150.0, 70.0),
                "L1",
                pal.primary,
            );
            for s in 0..4 {
                for w in 0..4 {
                    let p = at + Vec2::new(-30.0 + w as f32 * 26.0, 70.0 - s as f32 * 12.0);
                    ui.rect(p, Vec2::new(22.0, 8.0), pal.primary.with_alpha(0.5), 7.0);
                }
            }
            unit(
                ui,
                at + Vec2::new(10.0, -40.0),
                Vec2::new(150.0, 70.0),
                "SHARED",
                pal.tertiary,
            );
            lanes(
                ui,
                at + Vec2::new(10.0, -52.0),
                130.0,
                22.0,
                pal.tertiary,
                0,
            );
            unit(
                ui,
                at + Vec2::new(195.0, 10.0),
                Vec2::new(110.0, 170.0),
                "GLOBAL",
                pal.text_dim,
            );
            ui.line(
                at + Vec2::new(85.0, 60.0),
                at + Vec2::new(140.0, 60.0),
                2.0,
                pal.primary.with_alpha(0.6),
                6.8,
            );
            ui.caption(
                "400",
                at + Vec2::new(195.0, -10.0),
                pal.warn,
                Anchor::CENTER,
            );
        }
        _ => draw_sms(ui, i, at, accent),
    }
}

/// Atlas, Lyra and the origin machine: four SMs.
fn draw_sms(ui: &mut Ui, i: usize, at: Vec2, accent: Color) {
    let pal = ui.pal;
    let ring = i == 5;
    let pos = |k: usize| {
        if ring {
            at + Vec2::new(
                if k.is_multiple_of(3) { -130.0 } else { 130.0 },
                if k < 2 { 50.0 } else { -50.0 },
            )
        } else {
            at + Vec2::new(-195.0 + k as f32 * 130.0, 28.0)
        }
    };
    let size = if ring {
        Vec2::new(150.0, 80.0)
    } else {
        Vec2::new(118.0, 120.0)
    };
    if ring {
        for k in 0..SMS {
            let (a, b) = (pos(k), pos((k + 1) % SMS));
            ui.line(a, b, 3.0, pal.primary.with_alpha(0.55), 5.6);
            ui.dot((a + b) / 2.0, 8.0, pal.primary, 6.8);
        }
    } else {
        unit(
            ui,
            at + Vec2::new(0.0, -78.0),
            Vec2::new(500.0, 40.0),
            "GLOBAL",
            pal.text_dim,
        );
    }
    for k in 0..SMS {
        let p = pos(k);
        unit(ui, p, size, &format!("SM {k}"), accent);
        for w in 0..3 {
            lanes(
                ui,
                p + Vec2::new(0.0, 14.0 - w as f32 * 14.0) + Vec2::new(-size.x / 2.0 + 48.0, 0.0),
                80.0,
                8.0,
                accent,
                0,
            );
        }
        if i >= 4 {
            // The tensor core: a 4×4 block.
            let o = p + Vec2::new(size.x / 2.0 - 28.0, 0.0);
            for r in 0..4 {
                for c in 0..4 {
                    let q = o + Vec2::new((c as f32 - 1.5) * 9.0, (1.5 - r as f32) * 9.0);
                    ui.rect(q, Vec2::splat(7.5), pal.tertiary.with_alpha(0.75), 7.2);
                }
            }
        } else {
            ui.rect(
                p + Vec2::new(size.x / 2.0 - 26.0, 0.0),
                Vec2::new(28.0, 40.0),
                pal.primary.with_alpha(0.3),
                7.0,
            );
        }
        if !ring {
            ui.line(
                p + Vec2::new(0.0, -size.y / 2.0),
                p + Vec2::new(0.0, -58.0),
                2.0,
                pal.line_bright,
                5.6,
            );
        }
    }
    if ring {
        ui.caption("SEND · RECV", at, pal.primary, Anchor::CENTER);
    }
}

// ---- Section 3: the observation tools ----

const TOOLS_PER_PAGE: usize = 2;

fn draw_tools(ui: &mut Ui, v: &View, page: usize) {
    let pal = ui.pal;
    let (at, size) = PAGE;
    ui.card(at, size, pal.primary);
    ui.rect(at, Vec2::new(1.0, size.y - 60.0), pal.line, 6.0);
    for k in 0..TOOLS_PER_PAGE {
        let i = page * TOOLS_PER_PAGE + k;
        if i < manual::TOOLS.len() {
            let x = at.x + (k as f32 - 0.5) * size.x / 2.0;
            draw_tool(ui, v, i, Vec2::new(x, at.y));
        }
    }
}

/// One tool in a 580-wide column centred on `at`.
fn draw_tool(ui: &mut Ui, v: &View, i: usize, at: Vec2) {
    let l = v.lang;
    let pal = ui.pal;
    let d = &manual::TOOLS[i];
    let lv = manual::tool_level(i);
    let open = reached(lv, &v.game.save);
    let top = at.y + PAGE.1.y / 2.0;
    let left = at.x - 280.0;
    ui.caption(
        oo_base::fill(
            t(l, "TOOL {} OF {}", "工具 {} / {}"),
            &[&(i + 1), &manual::TOOLS.len()],
        ),
        Vec2::new(left, top - 30.0),
        if open { pal.primary } else { pal.text_faint },
        Anchor::CENTER_LEFT,
    );
    ui.text(
        d.name.get(l),
        Vec2::new(left, top - 58.0),
        22.0,
        if open { pal.text } else { pal.text_dim },
        Anchor::CENTER_LEFT,
    );
    let when = match lv {
        Some(t) if t.chapter == 1 && t.slot == 1 => {
            crate::state::t(l, "open from the first level", "第一关起就有").to_string()
        }
        Some(t) if open => oo_base::fill(
            crate::state::t(l, "open since chapter {} · {}", "第 {} 章起开放 · {}"),
            &[&t.chapter, &level_no(t)],
        ),
        Some(t) => oo_base::fill(
            crate::state::t(l, "opens in chapter {} · {}", "第 {} 章开放 · {}"),
            &[&t.chapter, &level_no(t)],
        ),
        None => String::new(),
    };
    ui.text(
        when,
        Vec2::new(at.x + 280.0, top - 58.0),
        13.0,
        if open { pal.text_dim } else { pal.text_faint },
        Anchor::CENTER_RIGHT,
    );
    let well = (Vec2::new(at.x, top - 168.0), Vec2::new(560.0, 170.0));
    if open {
        ui.well(well.0, well.1);
        draw_tool_art(ui, i, well.0);
    } else {
        pattern_board(ui, 40 + i as u32, well.0, well.1);
        ui.chip(well.0, Vec2::new(200.0, 40.0), pal.glass_raised, 7.6);
        ui.text(
            t(l, "NOT OPEN YET", "尚未开放"),
            well.0,
            16.0,
            pal.text_dim,
            Anchor::CENTER,
        );
    }
    let mut y = well.0.y - well.1.y / 2.0 - 24.0;
    for (head, body, c) in [
        (t(l, "WHAT IT SHOWS", "显示什么"), d.shows, pal.primary),
        (t(l, "HOW TO READ IT", "怎么看"), d.read, pal.secondary),
    ] {
        let c = if open { c } else { pal.text_faint };
        let lines = est_lines(body.get(l), 15.0, 540.0);
        let h = 20.0 + lines as f32 * 21.0;
        ui.rect(
            Vec2::new(left + 1.5, y - h / 2.0 + 6.0),
            Vec2::new(3.0, h),
            c,
            8.0,
        );
        ui.caption(head, Vec2::new(left + 14.0, y), c, Anchor::CENTER_LEFT);
        ui.wrapped(
            body.get(l),
            Vec2::new(left + 14.0, y - 14.0),
            546.0,
            15.0,
            if open { pal.text } else { pal.text_faint },
            Anchor::TOP_LEFT,
        );
        y -= h + 26.0;
    }
}

/// A miniature of tool `i` in a 560 × 170 well centred on `at`.
fn draw_tool_art(ui: &mut Ui, i: usize, at: Vec2) {
    let pal = ui.pal;
    let full = crate::river::river_colour(crate::river::RIVER_FULL);
    let split = crate::river::river_colour(crate::river::RIVER_SPLIT);
    let idle = crate::river::river_colour(crate::river::RIVER_IDLE);
    let stale = crate::river::river_colour(crate::river::RIVER_STALE);
    match i {
        0 => {
            // Output beside target, one pixel probed.
            for (k, dx) in [-130.0f32, 130.0].into_iter().enumerate() {
                for y in 0..12 {
                    for x in 0..12 {
                        let v = ((x + y) as f32 / 22.0).min(1.0);
                        let p = at
                            + Vec2::new(
                                dx + (x as f32 - 5.5) * 11.0,
                                (5.5 - y as f32) * 11.0 + 8.0,
                            );
                        let wrong = k == 0 && x == 8 && y == 3;
                        let c = if wrong {
                            pal.bad
                        } else {
                            pal.secondary.with_alpha(0.15 + 0.8 * v)
                        };
                        ui.rect(p, Vec2::splat(9.5), c, 7.0);
                    }
                }
            }
            let p = at + Vec2::new(-130.0 + 2.5 * 11.0, 2.5 * 11.0 + 8.0);
            ui.glow(p, 40.0, pal.primary.with_alpha(0.5), 7.5);
            ui.caption(
                "OUT",
                at + Vec2::new(-130.0, -70.0),
                pal.text_faint,
                Anchor::CENTER,
            );
            ui.caption(
                "TARGET",
                at + Vec2::new(130.0, -70.0),
                pal.text_faint,
                Anchor::CENTER,
            );
        }
        1 => {
            // Columns of 32 lanes: full, split by IF, one stale read.
            let cols = 64;
            for c in 0..cols {
                let split_col = (20..34).contains(&c);
                for lane in 0..32 {
                    let colour = if c == 50 {
                        stale
                    } else if split_col && (lane < 16) == (c < 27) {
                        idle
                    } else if split_col {
                        split
                    } else {
                        full
                    };
                    let p = at + Vec2::new((c as f32 - 31.5) * 8.4, (15.5 - lane as f32) * 4.6);
                    ui.rect(p, Vec2::new(7.4, 3.8), colour, 7.0);
                }
            }
        }
        2 => {
            // 8 × 4 lane cells with values.
            for lane in 0..32 {
                let (c, r) = ((lane % 8) as f32, (lane / 8) as f32);
                let p = at + Vec2::new((c - 3.5) * 62.0, (1.5 - r) * 36.0);
                let probed = lane == 5;
                ui.chip(
                    p,
                    Vec2::new(56.0, 31.0),
                    if probed {
                        mix_primary(&pal)
                    } else {
                        pal.glass_raised
                    },
                    7.0,
                );
                ui.caption(
                    lane.to_string(),
                    p + Vec2::new(-22.0, 8.0),
                    pal.text_faint,
                    Anchor::CENTER_LEFT,
                );
                ui.text(
                    ((lane * 7) % 32).to_string(),
                    p + Vec2::new(4.0, -4.0),
                    13.0,
                    pal.text,
                    Anchor::CENTER,
                );
            }
        }
        3 => {
            // Instruction rows against beats; one read lands too early.
            let rows = ["MUL r1, x, 3", "ADD r2, r1, 1", "NOP", "PIX r2"];
            for (k, s) in rows.iter().enumerate() {
                let y = at.y + 54.0 - k as f32 * 36.0;
                ui.text(
                    *s,
                    Vec2::new(at.x - 260.0, y),
                    13.0,
                    pal.secondary,
                    Anchor::CENTER_LEFT,
                );
                let issue = k as f32;
                ui.chip(
                    Vec2::new(at.x - 60.0 + issue * 48.0, y),
                    Vec2::new(42.0, 26.0),
                    pal.primary.with_alpha(0.35),
                    7.0,
                );
                if k == 0 {
                    ui.chip(
                        Vec2::new(at.x - 60.0 + 4.0 * 48.0, y),
                        Vec2::new(42.0, 26.0),
                        pal.primary.with_alpha(0.8),
                        7.0,
                    );
                }
                if k == 1 {
                    ui.chip(
                        Vec2::new(at.x - 60.0 + 48.0, y),
                        Vec2::new(42.0, 26.0),
                        pal.bad.with_alpha(0.7),
                        7.2,
                    );
                }
            }
        }
        4 => {
            // Cache lines against cycles: a coalesced burst, then scatter.
            for k in 0..14 {
                let y = at.y + 66.0 - k as f32 * 10.0;
                let (x0, w) = if k < 3 {
                    (-240.0, 160.0)
                } else {
                    (-40.0 + (k * 37 % 90) as f32, 60.0)
                };
                let c = if k < 3 {
                    pal.primary
                } else if k % 4 == 0 {
                    pal.bad
                } else {
                    pal.secondary
                };
                ui.rect(
                    Vec2::new(at.x + x0 + w / 2.0, y),
                    Vec2::new(w, 7.0),
                    c.with_alpha(0.75),
                    7.0,
                );
            }
            ui.line(
                at + Vec2::new(-260.0, -76.0),
                at + Vec2::new(260.0, -76.0),
                1.0,
                pal.line_bright,
                6.5,
            );
        }
        5 => {
            // Four SMs' cycle split, and a roofline with the kernel's point.
            let parts = [
                (0.55, pal.primary),
                (0.25, pal.secondary),
                (0.12, pal.tertiary),
                (0.08, idle),
            ];
            for sm in 0..4 {
                let y = at.y + 54.0 - sm as f32 * 30.0;
                let mut x = at.x - 260.0;
                for (k, (f, c)) in parts.iter().enumerate() {
                    let w = 260.0
                        * (f + if k == 0 {
                            -0.08 * sm as f32
                        } else if k == 3 {
                            0.08 * sm as f32
                        } else {
                            0.0
                        });
                    ui.rect(Vec2::new(x + w / 2.0, y), Vec2::new(w - 2.0, 18.0), *c, 7.0);
                    x += w;
                }
            }
            let o = at + Vec2::new(60.0, -60.0);
            ui.line(o, o + Vec2::new(110.0, 120.0), 2.0, pal.text_dim, 7.0);
            ui.line(
                o + Vec2::new(110.0, 120.0),
                o + Vec2::new(200.0, 120.0),
                2.0,
                pal.text_dim,
                7.0,
            );
            ui.glow(
                o + Vec2::new(70.0, 50.0),
                30.0,
                pal.primary.with_alpha(0.6),
                7.4,
            );
            ui.dot(o + Vec2::new(70.0, 50.0), 9.0, pal.primary, 7.5);
        }
        6 => {
            // A × B + C = D, one D cell overflowed.
            let names = ["A", "B", "C", "D"];
            for (k, n) in names.iter().enumerate() {
                let o = at + Vec2::new(-200.0 + k as f32 * 134.0, -6.0);
                for r in 0..4 {
                    for c in 0..4 {
                        let q = o + Vec2::new((c as f32 - 1.5) * 20.0, (1.5 - r as f32) * 20.0);
                        let h = traced(k as u32 + 9, (r * 4 + c) as u32) % 5;
                        let col = if k == 3 && r == 1 && c == 2 {
                            pal.bad
                        } else {
                            pal.tertiary.with_alpha(0.25 + h as f32 * 0.14)
                        };
                        ui.rect(q, Vec2::splat(17.0), col, 7.0);
                    }
                }
                ui.caption(*n, o + Vec2::new(0.0, 56.0), pal.text_dim, Anchor::CENTER);
                if k < 3 {
                    ui.caption(
                        ["×", "+", "="][k],
                        o + Vec2::new(67.0, 0.0),
                        pal.text_dim,
                        Anchor::CENTER,
                    );
                }
            }
        }
        _ => {
            // Four link timelines with messages, gaps where an SM waits.
            for k in 0..4 {
                let y = at.y + 54.0 - k as f32 * 34.0;
                ui.caption(
                    format!("{k}→{}", (k + 1) % 4),
                    Vec2::new(at.x - 262.0, y),
                    pal.text_faint,
                    Anchor::CENTER_LEFT,
                );
                ui.rect(
                    Vec2::new(at.x + 20.0, y),
                    Vec2::new(500.0, 2.0),
                    pal.line,
                    6.8,
                );
                for m in 0..3 {
                    let x0 = -200.0 + (m * 150 + k * 40) as f32;
                    let w = (50.0 + (traced(k as u32, m as u32) % 60) as f32).min(255.0 - x0);
                    let c = if k == 2 && m == 1 {
                        pal.warn
                    } else {
                        pal.primary
                    };
                    ui.rect(
                        Vec2::new(at.x + x0 + w / 2.0, y),
                        Vec2::new(w, 14.0),
                        c.with_alpha(0.7),
                        7.0,
                    );
                }
            }
        }
    }
}

fn mix_primary(pal: &oo_kit::theme::Palette) -> Color {
    oo_kit::theme::mix(pal.glass_raised, pal.primary, 0.35)
}

// ---- Section 4: the concept cards ----

/// Centre of the card on the right; the chapter list sits on the left.
const CARD_AT: Vec2 = Vec2::new(300.0, -27.0);
const LIST: (Vec2, Vec2) = (Vec2::new(-335.0, -27.0), Vec2::new(570.0, 540.0));

fn draw_concept(ui: &mut Ui, v: &View, page: usize) {
    let l = v.lang;
    let pal = ui.pal;
    let save = &v.game.save;
    let tasks = content::tasks();
    let task = &tasks[page];
    let ch = content::chapter(task.chapter);
    let accent = hub::chapter_accent(ch, &pal);
    let (at, size) = LIST;
    ui.card(at, size, accent);
    let top = at.y + size.y / 2.0;
    let left = at.x - size.x / 2.0 + 30.0;
    ui.caption(
        format!(
            "{} · {}",
            hub::chapter_word(task.chapter, l),
            ch.machine.get(l)
        ),
        Vec2::new(left, top - 28.0),
        accent,
        Anchor::CENTER_LEFT,
    );
    let open = hub::chapter_open(task.chapter, save);
    ui.text(
        if open {
            ch.title.get(l)
        } else {
            t(l, "A chapter still ahead", "还没到的一章")
        },
        Vec2::new(left, top - 56.0),
        20.0,
        pal.text,
        Anchor::CENTER_LEFT,
    );
    // The chapter's levels; solved ones open their card.
    for (k, (i, lv)) in tasks
        .iter()
        .enumerate()
        .filter(|(_, x)| x.chapter == task.chapter)
        .enumerate()
    {
        let y = top - 104.0 - k as f32 * 44.0;
        let solved = save.campaign.is_solved(lv.id);
        let mut b = ButtonSpec::new(
            Vec2::new(at.x, y),
            Vec2::new(size.x - 44.0, 38.0),
            "",
            Act::ManualAt(i),
        )
        .selected(i == page);
        if i != page {
            b = b.tone(Tone::Ghost);
        }
        ui.button(b);
        let x = at.x - size.x / 2.0 + 36.0;
        ui.text(
            level_no(lv),
            Vec2::new(x, y),
            14.0,
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
        let (name, c) = if solved {
            (lv.insight.title.get(l).to_string(), pal.text)
        } else if save.is_unlocked(lv) {
            (lv.title.get(l).to_string(), pal.text_dim)
        } else {
            ("· · ·".to_string(), pal.text_faint)
        };
        let name = clip(&name, 15.0, 380.0);
        ui.text(name, Vec2::new(x + 50.0, y), 15.0, c, Anchor::CENTER_LEFT);
        let tag = if solved {
            t(l, "card", "已得")
        } else {
            t(l, "locked", "未解锁")
        };
        let tc = if solved { pal.primary } else { pal.text_faint };
        ui.caption(
            tag,
            Vec2::new(at.x + size.x / 2.0 - 36.0, y),
            tc,
            Anchor::CENTER_RIGHT,
        );
        if solved {
            ui.dot(
                Vec2::new(at.x + size.x / 2.0 - 30.0, y),
                6.0,
                pal.primary,
                12.0,
            );
        }
    }
    // Jump to another chapter's first card.
    for c in 1..=CHAPTERS {
        let first = tasks.iter().position(|x| x.chapter == c).unwrap_or(0);
        let x = at.x - size.x / 2.0 + 36.0 + (c - 1) as f32 * 33.0;
        let mut b = ButtonSpec::new(
            Vec2::new(x, top - 500.0),
            Vec2::new(30.0, 28.0),
            c.to_string(),
            Act::ManualAt(first),
        )
        .font(12.0)
        .selected(c == task.chapter);
        if c != task.chapter {
            b = b.tone(Tone::Ghost);
        }
        ui.button(b);
    }
    if save.campaign.is_solved(task.id) {
        results::draw_insight_card(ui, &task.insight, CARD_AT, l, Some(page));
    } else {
        draw_locked_card(ui, v, task);
    }
}

/// An unsolved level's card: same frame, title hidden.
fn draw_locked_card(ui: &mut Ui, v: &View, task: &Task) {
    let l = v.lang;
    let pal = ui.pal;
    let at = CARD_AT;
    ui.card(at, Vec2::new(640.0, 540.0), pal.text_faint);
    let x0 = at.x - 290.0;
    ui.caption(
        t(l, "INSIGHT CARD", "洞见卡"),
        Vec2::new(x0, at.y + 242.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    ui.text(
        t(l, "Locked card", "未解锁的卡"),
        Vec2::new(x0, at.y + 210.0),
        24.0,
        pal.text_dim,
        Anchor::CENTER_LEFT,
    );
    let why = if v.game.save.is_unlocked(task) {
        oo_base::fill(
            t(
                l,
                "Solve level {} to read this card.",
                "通过第 {} 关后可以读这张卡。",
            ),
            &[&level_no(task)],
        )
    } else {
        oo_base::fill(
            t(
                l,
                "This card waits in chapter {}.",
                "这张卡在第 {} 章等你。",
            ),
            &[&task.chapter],
        )
    };
    ui.wrapped(
        why,
        Vec2::new(x0, at.y + 184.0),
        580.0,
        16.0,
        pal.text_dim,
        Anchor::TOP_LEFT,
    );
    pattern_board(
        ui,
        100 + task.chapter as u32 * 8 + task.slot as u32,
        at + Vec2::new(0.0, -50.0),
        Vec2::new(580.0, 300.0),
    );
}

/// Cuts `s` with an ellipsis so it fits in `width` at `size`.
fn clip(s: &str, size: f32, width: f32) -> String {
    if est_width(s, size) <= width {
        return s.to_string();
    }
    let mut out = String::new();
    for c in s.chars() {
        if est_width(&out, size) + size * 1.2 > width {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}
