//! A chapter's opening: up to three lines from its client and ORI, shown
//! over the hub the first time the chapter is opened.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_kit::theme::mix;
use oo_kit::ui::{ButtonSpec, Tone, Ui};
use ooc_core::content::{self, Speaker, opening};

use super::{brass, cyan};
use crate::state::{Act, t};
use crate::{Ctx, View, hub};

pub fn act(cx: &mut Ctx, a: Act) {
    match a {
        Act::Opening(n) => {
            cx.s.opening = Some((n, 0));
            cx.s.chapter = Some(n);
            cx.s.ori_menu = false;
            cx.dirty();
        }
        Act::Line => match cx.s.opening {
            Some((n, i)) if i + 1 < opening(n).len() => {
                cx.s.opening = Some((n, i + 1));
                cx.dirty();
            }
            Some(_) => finish(cx),
            None => {}
        },
        Act::SkipOpening => finish(cx),
        _ => {}
    }
}

fn finish(cx: &mut Ctx) {
    if let Some((n, _)) = cx.s.opening.take() {
        cx.game.save.seen_openings.insert(n);
        cx.game.touch();
    }
    cx.dirty();
}

fn cat_color(c: oo_base::Cat) -> Color {
    let v = c.color();
    Color::srgb_u8((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// The opening's dialogue card, where the level panel usually is.
pub fn draw(ui: &mut Ui, v: &View) {
    let Some((n, i)) = v.s.opening else { return };
    let lines = opening(n);
    let Some(line) = lines.get(i) else { return };
    let pal = ui.pal;
    let l = v.lang;
    let ch = content::chapter(n);
    // The same frame as the level panel it stands in for.
    let center = Vec2::new(0.0, -140.0);
    let size = Vec2::new(1264.0, 290.0);
    let (name, accent) = match line.who {
        Speaker::Cat(c) => (c.name().get(l).to_string(), cat_color(c)),
        Speaker::Ori => ("ORI".to_string(), cyan()),
    };
    ui.card(center, size, accent);
    let left = center.x - size.x / 2.0;
    let top = center.y + size.y / 2.0;
    // The speaker's portrait, or ORI's lens.
    let at = Vec2::new(left + 126.0, center.y + 10.0);
    ui.well(at, Vec2::splat(196.0));
    match line.who {
        Speaker::Cat(c) => {
            ui.glow(at, 220.0, accent.with_alpha(0.12), 5.4);
            oo_kit::portrait::spawn(ui, c, at, 188.0, 20.0);
        }
        Speaker::Ori => {
            ui.glow(at, 230.0, cyan().with_alpha(0.22), 5.4);
            ui.dot(at, 96.0, mix(pal.bg_deep, brass(), 0.7), 6.0);
            ui.dot(at, 78.0, Color::srgb(0.01, 0.015, 0.03), 6.1);
            ui.glow(at, 90.0, cyan().with_alpha(0.55), 6.2);
            ui.dot(at, 30.0, cyan(), 6.3);
            ui.dot(at + Vec2::new(-5.0, 5.0), 9.0, Color::WHITE.with_alpha(0.85), 6.4);
            ui.line(at + Vec2::new(0.0, 48.0), at + Vec2::new(0.0, 88.0), 3.0, brass(), 6.0);
        }
    }
    let x = left + 260.0;
    ui.caption(
        format!("{} · {} · {}", hub::chapter_word(n, l), ch.machine.get(l), ch.title.get(l)),
        Vec2::new(x, top - 30.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    ui.dot(Vec2::new(x + 6.0, top - 66.0), 10.0, accent, 13.0);
    let who = match line.who {
        Speaker::Cat(c) => format!("{name} · {}", c.role().get(l)),
        Speaker::Ori => name,
    };
    ui.text(who, Vec2::new(x + 22.0, top - 66.0), 17.0, accent, Anchor::CENTER_LEFT);
    ui.wrapped(
        line.line.get(l),
        Vec2::new(x, top - 94.0),
        size.x - 260.0 - 80.0,
        23.0,
        pal.text,
        Anchor::TOP_LEFT,
    );
    // Progress, the hint, and the buttons along the bottom edge.
    let bottom = center.y - size.y / 2.0;
    for k in 0..lines.len() {
        let c = if k == i { accent } else if k < i { pal.text_dim } else { pal.text_faint.with_alpha(0.5) };
        ui.dot(Vec2::new(x + 5.0 + k as f32 * 16.0, bottom + 36.0), 7.0, c, 13.0);
    }
    ui.caption(
        t(l, "CLICK OR PRESS SPACE TO CONTINUE", "点击或按空格继续"),
        Vec2::new(x + 16.0 + lines.len() as f32 * 16.0, bottom + 36.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let last = i + 1 == lines.len();
    let right = center.x + size.x / 2.0;
    ui.button(
        ButtonSpec::new(
            Vec2::new(right - 96.0, bottom + 36.0),
            Vec2::new(150.0, 38.0),
            if last { t(l, "Begin", "开始") } else { t(l, "Next", "继续") },
            Act::Line,
        )
        .primary()
        .font(16.0),
    );
    if !last {
        ui.button(
            ButtonSpec::new(
                Vec2::new(right - 236.0, bottom + 36.0),
                Vec2::new(110.0, 38.0),
                t(l, "Skip", "跳过"),
                Act::SkipOpening,
            )
            .tone(Tone::Ghost)
            .font(14.0),
        );
    }
}
