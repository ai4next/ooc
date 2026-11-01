//! The machine room: the four SMs of the origin machine on their ring, and
//! a timeline of every message on each one-way link.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_kit::ui::Ui;
use ooc_core::vm::{Msg, SMS};

use super::{TITLE_Y, X0, cycle_now, empty, pct, run, swatch};
use crate::View;
use crate::state::t;

/// Ring diagram centre and half-width.
const RING: Vec2 = Vec2::new(-532.0, -240.0);
const RING_R: f32 = 52.0;
/// Timeline left edge, width, top row and row height.
const TL_X: f32 = -390.0;
const TL_W: f32 = 690.0;
const TL_Y: f32 = -164.0;
const TL_H: f32 = 19.0;
const PILL_X: f32 = 470.0;

/// The directed links of the ring, in timeline order.
fn links() -> Vec<(u8, u8)> {
    let n = SMS as u8;
    let mut v = Vec::new();
    for a in 0..n {
        v.push((a, (a + 1) % n));
    }
    for a in 0..n {
        v.push(((a + 1) % n, a));
    }
    v
}

fn node(sm: u8) -> Vec2 {
    // SM 0 top-left, then clockwise.
    let corner = [
        Vec2::new(-1.0, 1.0),
        Vec2::new(1.0, 1.0),
        Vec2::new(1.0, -1.0),
        Vec2::new(-1.0, -1.0),
    ];
    RING + corner[sm as usize % 4] * RING_R
}

pub fn draw(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let title = t(l, "MACHINE ROOM · LINKS BETWEEN SMS", "机房 · SM 之间的链路");
    let Some((_, r)) = run(s) else {
        empty(
            ui,
            title,
            t(
                l,
                "Run the program. The four SMs sit on a ring; each can SEND to its two neighbours. Every message holds its link for a fixed 6 cycles plus one cycle per 4 words, then needs 40 more to arrive. Each row of the timeline is one direction of one link.",
                "运行程序。四个 SM 连成一个环，每个只能 SEND 给左右两个邻居。每条消息先占住链路固定 6 周期，再每 4 个字 1 周期，之后还要 40 周期才到达。时间线的每一行是一条链路的一个方向。",
            ),
        );
        return;
    };
    let msgs = &r.run.msgs;
    let total = r
        .run
        .stats
        .cycles
        .max(msgs.iter().map(|m| m.arrive).max().unwrap_or(0))
        .max(1);
    ui.caption(
        format!("{title} · {} {}", msgs.len(), t(l, "MESSAGES", "条消息")),
        Vec2::new(X0, TITLE_Y),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let now = cycle_now(s);
    draw_ring(ui, v, msgs, now);
    let x_of = |c: u32| TL_X + c.min(total) as f32 / total as f32 * TL_W;
    for (row, (from, to)) in links().into_iter().enumerate() {
        let y = TL_Y - row as f32 * TL_H;
        ui.text(
            format!("{from}→{to}"),
            Vec2::new(TL_X - 10.0, y),
            11.0,
            pal.text_dim,
            Anchor::CENTER_RIGHT,
        );
        ui.rect(
            Vec2::new(TL_X + TL_W / 2.0, y),
            Vec2::new(TL_W, TL_H - 5.0),
            pal.bg_deep.with_alpha(0.8),
            6.0,
        );
        for m in msgs.iter().filter(|m| (m.from, m.to) == (from, to)) {
            let busy = if m.flipped.is_some() { pal.bad } else { pal.primary };
            let (a, b) = (x_of(m.start), x_of(m.end).max(x_of(m.start) + 2.0));
            ui.rect(
                Vec2::new((a + b) / 2.0, y),
                Vec2::new(b - a, TL_H - 7.0),
                busy.with_alpha(0.85),
                7.0,
            );
            // Waiting for the link, then flying to the receiver.
            let q = x_of(m.sent);
            if a - q >= 1.0 {
                ui.rect(
                    Vec2::new((q + a) / 2.0, y),
                    Vec2::new(a - q, 3.0),
                    pal.secondary.with_alpha(0.7),
                    6.8,
                );
            }
            let z = x_of(m.arrive);
            if z - b >= 1.0 {
                ui.rect(
                    Vec2::new((b + z) / 2.0, y),
                    Vec2::new(z - b, 3.0),
                    pal.primary.with_alpha(0.5),
                    6.8,
                );
            }
        }
    }
    let bottom = TL_Y - (links().len() as f32 - 0.5) * TL_H;
    if now > 0 {
        let x = x_of(now);
        ui.rect(
            Vec2::new(x, (TL_Y + TL_H / 2.0 + bottom) / 2.0),
            Vec2::new(2.0, TL_Y + TL_H / 2.0 - bottom + 4.0),
            pal.secondary.with_alpha(0.8),
            8.0,
        );
    }
    let y = bottom - 12.0;
    ui.caption("0", Vec2::new(TL_X, y), pal.text_faint, Anchor::CENTER_LEFT);
    ui.caption(
        format!("{total} {}", t(l, "CYCLES", "周期")),
        Vec2::new(TL_X + TL_W, y),
        pal.text_faint,
        Anchor::CENTER_RIGHT,
    );
    let mut x = TL_X + 60.0;
    for (c, label) in [
        (pal.secondary, t(l, "waiting for the link", "等链路")),
        (pal.primary, t(l, "on the link", "在链路上")),
        (pal.primary.with_alpha(0.5), t(l, "in flight", "飞行中")),
        (pal.bad, t(l, "a bit flipped", "翻了一位")),
    ] {
        x = swatch(ui, Vec2::new(x, y), c, label);
    }
    draw_pills(ui, v);
}

/// The ring: an edge lights while a message is on it at the replay's cycle;
/// SMs caught in a deadlock are red.
fn draw_ring(ui: &mut Ui, v: &View, msgs: &[Msg], now: u32) {
    let pal = ui.pal;
    let Some((_, r)) = run(v.s) else { return };
    let stuck: &[u8] = r.run.deadlock.as_ref().map_or(&[], |d| &d.sms);
    let n = SMS as u8;
    for a in 0..n {
        let b = (a + 1) % n;
        let live = msgs.iter().any(|m| {
            ((m.from, m.to) == (a, b) || (m.from, m.to) == (b, a)) && m.start <= now && now < m.arrive
        });
        let c = if live {
            pal.primary
        } else {
            pal.line_bright.with_alpha(0.3)
        };
        ui.line(node(a), node(b), if live { 3.0 } else { 1.5 }, c, 6.0);
        if live {
            ui.glow((node(a) + node(b)) / 2.0, 30.0, pal.primary.with_alpha(0.35), 5.9);
        }
    }
    for sm in 0..n {
        let p = node(sm);
        let bad = stuck.contains(&sm);
        let c = if bad { pal.bad } else { pal.secondary };
        ui.chip(p, Vec2::new(44.0, 26.0), pal.glass_raised, 7.0);
        ui.chip(p, Vec2::new(40.0, 22.0), c.with_alpha(0.22), 7.2);
        ui.text(format!("SM {sm}"), p, 12.0, c, Anchor::CENTER);
    }
}

fn draw_pills(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let Some((_, r)) = run(v.s) else { return };
    let st = &r.run.stats;
    let a = &r.run.analysis;
    let idle = a
        .sms
        .iter()
        .map(|u| 1.0 - u.issue as f32 / a.total.max(1) as f32)
        .fold(0.0f32, f32::max);
    let flips = r.run.msgs.iter().filter(|m| m.flipped.is_some()).count();
    let rows: [(&str, String, Color); 4] = [
        (t(l, "Words sent", "发送的字"), st.link_words.to_string(), pal.primary),
        (t(l, "Busiest link (words)", "最忙链路（字）"), st.hot_link.to_string(), pal.secondary),
        (t(l, "Most idle SM", "最闲的 SM"), pct(idle), pal.tertiary),
        (
            t(l, "Bits flipped", "翻转的位"),
            flips.to_string(),
            if flips > 0 { pal.bad } else { pal.good },
        ),
    ];
    for (k, (label, value, c)) in rows.into_iter().enumerate() {
        ui.pill(Vec2::new(PILL_X, -152.0 - k as f32 * 34.0), 270.0, label, value, c);
    }
    if let Some(d) = &r.run.deadlock {
        let who = d
            .sms
            .iter()
            .map(|s| format!("SM {s}"))
            .collect::<Vec<_>>()
            .join(" · ");
        let msg = if d.cycle {
            t(
                l,
                "Deadlock at cycle {c}: {who} each wait for a message from the next. Nobody sends first.",
                "第 {c} 周期互等：{who} 都在等下一个的消息，没有谁先发。",
            )
        } else {
            t(
                l,
                "Stuck at cycle {c}: {who} wait for a message nobody will send.",
                "第 {c} 周期卡住：{who} 在等一条没人会发的消息。",
            )
        };
        ui.wrapped(
            msg.replace("{c}", &d.since.to_string()).replace("{who}", &who),
            Vec2::new(PILL_X - 135.0, -288.0),
            270.0,
            12.0,
            pal.bad,
            Anchor::TOP_LEFT,
        );
    }
}
