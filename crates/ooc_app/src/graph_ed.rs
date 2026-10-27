//! The blueprint editor for Grid Hall circuits: drag parts, wire outputs into
//! ports, type constants into unwired ports.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_kit::audio::Cue;
use oo_kit::ui::{ButtonSpec, Cursor, Tone, Ui};
use ooc_core::graph::{Graph, MAX_NODES, Node, NodeKind};
use ooc_core::isa::FB_W;
use ooc_core::task::{Editor, Solution};

use crate::state::{Act, Drag, Sel, Session, t};
use crate::{Ctx, View};

/// Canvas centre on screen and graph-to-screen scale.
const C: Vec2 = Vec2::new(-180.0, 112.0);
const SCALE: f32 = 0.92;
/// The canvas the nodes may live in (screen space).
const AREA: (Vec2, Vec2) = (Vec2::new(-216.0, 112.0), Vec2::new(820.0, 288.0));
const NODE_W: f32 = 88.0;
const HEAD: f32 = 22.0;
const PORT_H: f32 = 19.0;

pub fn to_screen(p: [f32; 2]) -> Vec2 {
    C + Vec2::from(p) * SCALE
}

fn to_graph(p: Vec2) -> [f32; 2] {
    let lo = AREA.0 - AREA.1 / 2.0 + Vec2::new(NODE_W / 2.0, 30.0);
    let hi = AREA.0 + AREA.1 / 2.0 - Vec2::new(NODE_W / 2.0, 30.0);
    ((p.clamp(lo, hi) - C) / SCALE).into()
}

fn in_area(p: Vec2) -> bool {
    (p - AREA.0).abs().cmple(AREA.1 / 2.0).all()
}

fn height(k: NodeKind) -> f32 {
    HEAD + PORT_H * k.arity().max(1) as f32 + 4.0
}

/// Node rectangle (centre, size) on screen.
fn rect(n: &Node) -> (Vec2, Vec2) {
    let h = height(n.kind);
    let top = to_screen(n.pos) + Vec2::new(0.0, h / 2.0);
    (top - Vec2::new(0.0, h / 2.0), Vec2::new(NODE_W, h))
}

pub fn port_pos(n: &Node, p: usize) -> Vec2 {
    let (c, s) = rect(n);
    Vec2::new(
        c.x - s.x / 2.0,
        c.y + s.y / 2.0 - HEAD - PORT_H * (p as f32 + 0.5),
    )
}

pub fn out_pos(n: &Node) -> Vec2 {
    let (c, s) = rect(n);
    Vec2::new(c.x + s.x / 2.0, c.y + s.y / 2.0 - HEAD / 2.0)
}

fn graph(s: &Session) -> Option<&Graph> {
    match s.work.as_ref()? {
        Solution::Graph(g) => Some(g),
        Solution::Code(_) => None,
    }
}

fn graph_mut(s: &mut Session) -> Option<&mut Graph> {
    match s.edit() {
        Solution::Graph(g) => Some(g),
        Solution::Code(_) => None,
    }
}

/// What lies under the pointer, topmost first.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Hit {
    Out(usize),
    Port(usize, usize),
    Node(usize),
}

fn hit(g: &Graph, p: Vec2) -> Option<Hit> {
    for (i, n) in g.nodes.iter().enumerate().rev() {
        if n.kind != NodeKind::Out && p.distance(out_pos(n)) < 11.0 {
            return Some(Hit::Out(i));
        }
        for k in 0..n.inputs.len() {
            let pp = port_pos(n, k);
            if (p - pp - Vec2::new(31.0, 0.0))
                .abs()
                .cmple(Vec2::new(41.0, PORT_H / 2.0))
                .all()
            {
                return Some(Hit::Port(i, k));
            }
        }
        let (c, s) = rect(n);
        if (p - c).abs().cmple(s / 2.0).all() {
            return Some(Hit::Node(i));
        }
    }
    None
}

/// Pointer handling for the canvas: select, drag nodes, draw wires.
pub fn input(cursor: Res<Cursor>, mut cx: Ctx) {
    if !matches!(cx.s.task().editor, Editor::Graph(_)) {
        return;
    }
    let Some(p) = cursor.world else { return };
    // A copy: the handlers below edit the session (at most 18 small nodes).
    let Some(g) = graph(&cx.s).cloned() else {
        return;
    };
    let g = &g;
    if cursor.pressed && !cursor.over_button && cx.s.drag.is_none() {
        if !in_area(p) {
            return;
        }
        let h = hit(g, p);
        cx.s.typing = false;
        match h {
            Some(Hit::Out(i)) => cx.s.drag = Some(Drag::Wire { from: i }),
            Some(Hit::Port(i, k)) => match g.nodes[i].inputs[k].from {
                // Pulling a wired port unplugs it and carries the wire.
                Some(from) => {
                    if let Some(g) = graph_mut(&mut cx.s) {
                        g.nodes[i].inputs[k].from = None;
                    }
                    cx.s.drag = Some(Drag::Wire { from });
                    cx.s.sel = Sel::None;
                }
                None => cx.s.sel = Sel::Port(i, k),
            },
            Some(Hit::Node(i)) => {
                let grab = p - to_screen(g.nodes[i].pos);
                cx.s.drag = Some(Drag::Node {
                    node: i,
                    grab,
                    moved: false,
                });
                cx.s.sel = Sel::Node(i);
            }
            None => cx.s.sel = Sel::None,
        }
        cx.dirty();
        return;
    }
    match cx.s.drag {
        Some(Drag::Node { node, grab, moved }) if cursor.down => {
            let at = to_graph(p - grab);
            if g.nodes.get(node).is_some_and(|n| n.pos != at) {
                // Moving a part changes nothing about what it computes: keep the run.
                if !moved && let Some(w) = cx.s.work.clone() {
                    cx.s.undo.push(w);
                }
                if let Some(Solution::Graph(g)) = cx.s.work.as_mut() {
                    g.nodes[node].pos = at;
                }
                cx.s.drag = Some(Drag::Node {
                    node,
                    grab,
                    moved: true,
                });
                cx.dirty();
            }
        }
        Some(Drag::Wire { from }) if !cursor.down => {
            cx.s.drag = None;
            drop_wire(&mut cx, from, p);
            cx.dirty();
        }
        Some(Drag::Node { .. }) if !cursor.down => {
            cx.s.drag = None;
            cx.dirty();
        }
        _ => {}
    }
}

/// Where a dropped wire lands: a port, or the first free port of a node.
fn drop_target(g: &Graph, p: Vec2) -> Option<(usize, usize)> {
    match hit(g, p)? {
        Hit::Port(n, k) => Some((n, k)),
        Hit::Node(n) | Hit::Out(n) => {
            let ins = &g.nodes[n].inputs;
            let k = ins
                .iter()
                .position(|q| q.from.is_none())
                .or((!ins.is_empty()).then_some(0))?;
            Some((n, k))
        }
    }
}

fn drop_wire(cx: &mut Ctx, from: usize, p: Vec2) {
    let Some(g) = graph(&cx.s) else { return };
    let Some((to, k)) = drop_target(g, p) else {
        return;
    };
    if to == from || g.nodes[to].inputs[k].from == Some(from) {
        return;
    }
    if g.reaches(to, from) {
        let l = cx.lang();
        cx.toasts.push(
            t(l, "That would loop", "这会形成回路"),
            t(
                l,
                "A part cannot feed its own input — every pixel needs an answer in one pass.",
                "部件不能喂给自己的输入——每个像素都要一次算出答案。",
            ),
        );
        cx.cues.play(Cue::Error);
        return;
    }
    if let Some(g) = graph_mut(&mut cx.s) {
        g.connect(from, to, k);
    }
    cx.s.sel = Sel::None;
    cx.cues.play(Cue::Place);
}

/// A free spot for a new part, scanning the canvas left to right.
fn free_spot(g: &Graph) -> [f32; 2] {
    for gx in 0..6 {
        for gy in 0..4 {
            let at = [-360.0 + gx as f32 * 130.0, 110.0 - gy as f32 * 70.0];
            if g.nodes
                .iter()
                .all(|n| (n.pos[0] - at[0]).abs() > 90.0 || (n.pos[1] - at[1]).abs() > 55.0)
            {
                return at;
            }
        }
    }
    [0.0, 0.0]
}

pub fn act(cx: &mut Ctx, a: Act) {
    let l = cx.lang();
    let Some(g) = graph(&cx.s) else { return };
    match a {
        Act::AddNode(kind) => {
            if g.nodes.len() >= MAX_NODES {
                cx.toasts.push(
                    t(l, "The board is full", "电路板满了"),
                    format!("{} {MAX_NODES}", t(l, "Parts allowed:", "允许的部件数：")),
                );
                cx.cues.play(Cue::Error);
                return;
            }
            let at = free_spot(g);
            if let Some(g) = graph_mut(&mut cx.s) {
                let i = g.push(Node::new(kind, at));
                cx.s.sel = Sel::Node(i);
            }
            cx.cues.play(Cue::Place);
        }
        Act::DeleteSel => match cx.s.sel {
            Sel::Node(n) if n > 0 && n < g.nodes.len() => {
                if let Some(g) = graph_mut(&mut cx.s) {
                    g.remove(n);
                }
                cx.s.sel = Sel::None;
            }
            Sel::Port(n, k)
                if g.nodes
                    .get(n)
                    .and_then(|x| x.inputs.get(k))
                    .is_some_and(|q| q.from.is_some()) =>
            {
                if let Some(g) = graph_mut(&mut cx.s) {
                    g.nodes[n].inputs[k].from = None;
                }
            }
            _ => {}
        },
        Act::Bump(d) => {
            if let Sel::Port(n, k) = cx.s.sel
                && let Some(q) = g.nodes.get(n).and_then(|x| x.inputs.get(k))
                && q.from.is_none()
            {
                let v = (q.value + d).clamp(-9999, 9999);
                if v != q.value
                    && let Some(g) = graph_mut(&mut cx.s)
                {
                    g.nodes[n].inputs[k].value = v;
                }
            }
        }
        _ => {}
    }
}

/// The wire being drawn follows the pointer.
#[derive(Component)]
pub struct GhostWire;

pub fn sync_ghost(
    cursor: Res<Cursor>,
    s: Res<Session>,
    mut q: Query<(&mut Transform, &mut Sprite), With<GhostWire>>,
) {
    let (Some(Drag::Wire { from }), Some(p)) = (s.drag, cursor.world) else {
        return;
    };
    let Some(n) = graph(&s).and_then(|g| g.nodes.get(from)) else {
        return;
    };
    let a = out_pos(n);
    let d = p - a;
    for (mut tf, mut sprite) in &mut q {
        let mid = (a + p) / 2.0;
        tf.translation.x = mid.x;
        tf.translation.y = mid.y;
        tf.rotation = Quat::from_rotation_z(d.y.atan2(d.x));
        sprite.custom_size = Some(Vec2::new(d.length(), 2.5));
    }
}

/// Each node's value for the probed pixel, after a run.
fn probe_values(s: &Session, g: &Graph) -> Option<Vec<Option<i32>>> {
    let rep = s.report.as_ref()?;
    let th = s.probe?;
    let case = &rep.runs[s.case.min(rep.runs.len() - 1)].case;
    let p = ooc_core::grade::probe(s.task(), &rep.program, case, th)?;
    let regs = g.compile().ok()?.reg;
    let (x, y) = (th as i32 % FB_W, th as i32 / FB_W);
    Some(
        g.nodes
            .iter()
            .enumerate()
            .map(|(i, n)| match n.kind {
                NodeKind::X => Some(x),
                NodeKind::Y => Some(y),
                NodeKind::Tid => Some(th as i32),
                NodeKind::Frame => Some(case.frame),
                NodeKind::Out => s.play.buf.get(th).copied(),
                _ => regs.get(i).copied().flatten().map(|r| p.regs[r as usize]),
            })
            .collect(),
    )
}

fn wire(ui: &mut Ui, a: Vec2, b: Vec2, color: Color, z: f32) {
    let k = ((b.x - a.x).abs() * 0.5).max(40.0);
    let (c1, c2) = (a + Vec2::new(k, 0.0), b - Vec2::new(k, 0.0));
    let at = |t: f32| {
        let u = 1.0 - t;
        a * u * u * u + c1 * 3.0 * u * u * t + c2 * 3.0 * u * t * t + b * t * t * t
    };
    let mut prev = a;
    for i in 1..=14 {
        let p = at(i as f32 / 14.0);
        ui.line(prev, p, 2.5, color, z);
        prev = p;
    }
}

fn kind_color(ui: &Ui, k: NodeKind) -> Color {
    let pal = ui.pal;
    match k {
        NodeKind::X | NodeKind::Y | NodeKind::Tid | NodeKind::Frame => pal.tertiary,
        NodeKind::Lt | NodeKind::Ge | NodeKind::Eq | NodeKind::Mux => pal.warn,
        NodeKind::Out => pal.secondary,
        _ => pal.primary,
    }
}

fn kind_tip(k: NodeKind, l: oo_base::Lang) -> &'static str {
    match k {
        NodeKind::X => t(l, "This bulb's column, 0–15.", "本灯泡所在的列，0–15。"),
        NodeKind::Y => t(l, "This bulb's row, 0–15.", "本灯泡所在的行，0–15。"),
        NodeKind::Tid => t(
            l,
            "This bulb's address: Y·16 + X.",
            "本灯泡的地址：Y·16 + X。",
        ),
        NodeKind::Frame => t(
            l,
            "The frame counter. Same for every bulb.",
            "帧计数器，所有灯泡都一样。",
        ),
        NodeKind::Add => t(l, "A + B", "A + B"),
        NodeKind::Sub => t(l, "A − B", "A − B"),
        NodeKind::Mul => t(l, "A × B", "A × B"),
        NodeKind::And => t(
            l,
            "Bitwise A and B. AND 15 keeps the low four bits.",
            "A 与 B 按位与。AND 15 保留低四位。",
        ),
        NodeKind::Shr => t(
            l,
            "A shifted right by B: A ÷ 2^B, rounding down.",
            "A 右移 B 位：A ÷ 2^B，向下取整。",
        ),
        NodeKind::Lt => t(l, "1 if A < B, else 0.", "A < B 时为 1，否则为 0。"),
        NodeKind::Ge => t(l, "1 if A ≥ B, else 0.", "A ≥ B 时为 1，否则为 0。"),
        NodeKind::Eq => t(l, "1 if A = B, else 0.", "A = B 时为 1，否则为 0。"),
        NodeKind::Mux => t(
            l,
            "If ? is not 0, pass 1; otherwise pass 0.",
            "? 不为 0 时取 1 端，否则取 0 端。",
        ),
        NodeKind::Out => t(l, "The colour of this bulb.", "这个灯泡的颜色。"),
    }
}

pub fn draw(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let Editor::Graph(kinds) = s.task().editor else {
        return;
    };
    let Some(g) = graph(s) else { return };
    // Palette along the top of the card.
    ui.caption(
        t(l, "PARTS", "部件"),
        Vec2::new(-612.0, 286.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    for (i, k) in kinds
        .iter()
        .copied()
        .filter(|k| *k != NodeKind::Out)
        .enumerate()
    {
        let p = Vec2::new(-540.0 + i as f32 * 60.0, 286.0);
        ui.button(
            ButtonSpec::new(p, Vec2::new(56.0, 24.0), k.label(), Act::AddNode(k))
                .font(12.0)
                .tone(Tone::Ghost)
                .tip(kind_tip(k, l)),
        );
        let c = kind_color(ui, k);
        ui.rect(p + Vec2::new(0.0, -10.0), Vec2::new(30.0, 2.0), c, 11.5);
    }
    ui.well(AREA.0, AREA.1);
    // Parts used, in the canvas corner: the palette row is full at 13 kinds.
    let used = g.nodes.len();
    ui.caption(
        format!("{used} / {MAX_NODES}"),
        AREA.0 + AREA.1 / 2.0 - Vec2::new(12.0, 12.0),
        if used >= MAX_NODES {
            pal.primary_hot
        } else {
            pal.text_faint
        },
        Anchor::CENTER_RIGHT,
    );
    // A faint dot grid makes the canvas read as a board.
    let lo = AREA.0 - AREA.1 / 2.0;
    for gx in 0..21 {
        for gy in 0..7 {
            let p = lo + Vec2::new(20.0 + gx as f32 * 39.0, 16.0 + gy as f32 * 39.0);
            ui.rect(p, Vec2::splat(2.0), pal.line.with_alpha(0.35), 5.3);
        }
    }
    let values = probe_values(s, g);
    let dragging = matches!(s.drag, Some(Drag::Node { moved: true, .. }));
    for n in &g.nodes {
        for (k, q) in n.inputs.iter().enumerate() {
            if let Some(f) = q.from {
                let c = kind_color(ui, g.nodes[f].kind).with_alpha(0.8);
                wire(ui, out_pos(&g.nodes[f]), port_pos(n, k), c, 6.0);
            }
        }
    }
    if let Some(Drag::Wire { from }) = s.drag
        && let Some(n) = g.nodes.get(from)
    {
        let a = out_pos(n);
        let e = ui.line(a, a + Vec2::X, 2.5, pal.primary_hot, 9.0);
        ui.commands.entity(e).insert(GhostWire);
    }
    for (i, n) in g.nodes.iter().enumerate() {
        let (c, size) = rect(n);
        let color = kind_color(ui, n.kind);
        let sel = s.sel == Sel::Node(i);
        let z = if sel && dragging { 8.0 } else { 7.0 };
        ui.chip(
            c,
            size + Vec2::splat(2.0),
            if sel {
                color
            } else {
                pal.line_bright.with_alpha(0.5)
            },
            z,
        );
        ui.chip(c, size, pal.glass_raised, z + 0.1);
        let top = c.y + size.y / 2.0;
        ui.chip(
            Vec2::new(c.x, top - HEAD / 2.0),
            Vec2::new(size.x, HEAD),
            color.with_alpha(0.28),
            z + 0.2,
        );
        ui.text(
            n.kind.label(),
            Vec2::new(c.x - size.x / 2.0 + 8.0, top - HEAD / 2.0),
            12.0,
            pal.text,
            Anchor::CENTER_LEFT,
        );
        if let Some(Some(val)) = values.as_ref().and_then(|v| v.get(i)) {
            let tag = Vec2::new(c.x + size.x / 2.0 - 6.0, top - HEAD / 2.0);
            ui.text(
                val.to_string(),
                tag,
                12.0,
                pal.secondary,
                Anchor::CENTER_RIGHT,
            );
        }
        if n.kind == NodeKind::Out {
            ui.caption(
                t(l, "EVERY BULB", "每个灯泡"),
                Vec2::new(c.x, c.y - size.y / 2.0 - 10.0),
                pal.text_faint,
                Anchor::CENTER,
            );
        } else {
            ui.dot(out_pos(n), 11.0, color, z + 0.4);
        }
        if n.kind.is_source() {
            ui.text(
                source_line(n.kind, l),
                Vec2::new(c.x - size.x / 2.0 + 8.0, c.y - 8.0),
                10.0,
                pal.text_faint,
                Anchor::CENTER_LEFT,
            );
        }
        for (k, q) in n.inputs.iter().enumerate() {
            let pp = port_pos(n, k);
            let name = n.kind.ports().get(k).copied().unwrap_or("");
            ui.dot(
                pp,
                9.0,
                if q.from.is_some() {
                    color
                } else {
                    pal.text_faint
                },
                z + 0.4,
            );
            ui.text(
                name,
                pp + Vec2::new(10.0, 0.0),
                11.0,
                pal.text_dim,
                Anchor::CENTER_LEFT,
            );
            if q.from.is_none() {
                let on = s.sel == Sel::Port(i, k);
                let cp = pp + Vec2::new(48.0, 0.0);
                ui.chip(
                    cp,
                    Vec2::new(46.0, PORT_H - 3.0),
                    if on {
                        pal.secondary.with_alpha(0.35)
                    } else {
                        pal.bg_deep
                    },
                    z + 0.3,
                );
                ui.text(
                    q.value.to_string(),
                    cp,
                    12.0,
                    if on { pal.text } else { pal.secondary },
                    Anchor::CENTER,
                );
            }
        }
    }
    draw_help(ui, s, g, l);
}

fn source_line(k: NodeKind, l: oo_base::Lang) -> &'static str {
    match k {
        NodeKind::X => t(l, "column", "列"),
        NodeKind::Y => t(l, "row", "行"),
        NodeKind::Tid => t(l, "y·16 + x", "y·16 + x"),
        _ => t(l, "frame", "帧"),
    }
}

/// One line under the canvas about what is selected.
fn draw_help(ui: &mut Ui, s: &Session, g: &Graph, l: oo_base::Lang) {
    let pal = ui.pal;
    let y = AREA.0.y - AREA.1.y / 2.0 - 14.0;
    let text = match s.sel {
        Sel::Port(n, k) if g.nodes.get(n).and_then(|x| x.inputs.get(k)).is_some() => {
            ui.button(
                ButtonSpec::new(
                    Vec2::new(-40.0, y),
                    Vec2::new(40.0, 20.0),
                    "−1",
                    Act::Bump(-1),
                )
                .font(12.0),
            );
            ui.button(
                ButtonSpec::new(Vec2::new(6.0, y), Vec2::new(40.0, 20.0), "+1", Act::Bump(1))
                    .font(12.0),
            );
            ui.button(
                ButtonSpec::new(
                    Vec2::new(60.0, y),
                    Vec2::new(50.0, 20.0),
                    "+10",
                    Act::Bump(10),
                )
                .font(12.0),
            );
            t(
                l,
                "Constant selected: type digits, − flips the sign. Or wire an output into it.",
                "已选中常数：直接输入数字，− 键取反。也可以把输出接进来。",
            )
        }
        Sel::Node(0) => kind_tip(NodeKind::Out, l),
        Sel::Node(n) if n < g.nodes.len() => {
            ui.button(
                ButtonSpec::new(
                    Vec2::new(40.0, y),
                    Vec2::new(80.0, 20.0),
                    t(l, "Remove", "移除"),
                    Act::DeleteSel,
                )
                .font(12.0),
            );
            kind_tip(g.nodes[n].kind, l)
        }
        _ => t(
            l,
            "Drag from a part's right dot to a port to wire it. Pull a wire off a port to unplug it.",
            "从部件右侧的圆点拖到端口即可接线。把线从端口拉开就能拔掉。",
        ),
    };
    ui.text(
        text,
        Vec2::new(-612.0, y),
        12.0,
        pal.text_dim,
        Anchor::CENTER_LEFT,
    );
}
