//! Bai Yan's notes (one page per chapter, ORI's correction beside it once
//! the chapter's commission is done) and the ending after the last one.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_base::Cat;
use oo_kit::ui::{ButtonSpec, Tone, Ui};
use ooc_core::content;
use ooc_core::save::{Ending, SaveData};
use ooc_core::task::{CHAPTERS, LEVELS_PER_CHAPTER};

use crate::state::{Act, Screen, t};
use crate::{Ctx, View, hub};

pub fn act(cx: &mut Ctx, a: Act) {
    match a {
        Act::Page(n) if hub::chapter_open(n, &cx.game.save) => cx.s.chapter = Some(n),
        Act::Choose(stay) if cx.game.save.ending.is_none() => {
            cx.game.save.ending = Some(if stay { Ending::Stay } else { Ending::Migrate });
            cx.game.touch();
            cx.cues.play(oo_kit::audio::Cue::Success);
        }
        _ => {}
    }
    cx.dirty();
}

/// Chapter `n`'s commission (its last level) has been delivered.
pub fn chapter_done(n: u8, save: &SaveData) -> bool {
    content::in_chapter(n)
        .find(|t| t.slot == LEVELS_PER_CHAPTER)
        .is_some_and(|t| save.campaign.is_solved(t.id))
}

/// The ending is offered once the last chapter's commission is done.
pub fn ending_open(save: &SaveData) -> bool {
    chapter_done(CHAPTERS, save)
}

/// A few bits per cell from the chapter number: the same page always shows
/// the same hand-traced pattern.
fn traced(n: u8, i: usize) -> u32 {
    let mut x = (n as u32).wrapping_mul(0x9E37_79B9) ^ (i as u32).wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^ (x >> 12)
}

/// A strip of the grid pattern as Bai Yan traced it from the stone.
fn draw_trace(ui: &mut Ui, n: u8, at: Vec2, size: Vec2, lit: bool) {
    let pal = ui.pal;
    ui.well(at, size);
    let (cols, rows) = (24usize, 6usize);
    let cell = Vec2::new((size.x - 24.0) / cols as f32, (size.y - 20.0) / rows as f32);
    let origin = at - size / 2.0 + Vec2::new(12.0, 10.0) + cell / 2.0;
    for r in 0..rows {
        for c in 0..cols {
            let h = traced(n, r * cols + c);
            // Rows repeat in groups of the chapter's width: a pattern, not noise.
            let on = (h % 5 < 2) || (c % (2 + n as usize % 4) == 0 && r % 2 == 0);
            if !on {
                continue;
            }
            let p = origin + Vec2::new(c as f32 * cell.x, (rows - 1 - r) as f32 * cell.y);
            let a = if lit { 0.35 + (h % 7) as f32 * 0.07 } else { 0.18 };
            let colour = if lit { pal.primary } else { pal.text_faint };
            ui.chip(p, cell - Vec2::splat(4.0), colour.with_alpha(a), 7.0);
        }
    }
    // One cell he went over twice.
    let k = traced(n, 999) as usize % (cols * rows);
    let p = origin + Vec2::new((k % cols) as f32 * cell.x, (k / cols) as f32 * cell.y);
    ui.glow(p, 26.0, pal.secondary.with_alpha(if lit { 0.4 } else { 0.15 }), 7.5);
}

pub fn draw_notes(ui: &mut Ui, v: &View) {
    let l = v.lang;
    let pal = ui.pal;
    let save = &v.game.save;
    let n = hub::shown_chapter(v).clamp(1, CHAPTERS);
    ui.text_spaced(
        t(l, "BAI YAN'S NOTES", "白砚译注"),
        Vec2::new(-600.0, 318.0),
        24.0,
        pal.text,
        Anchor::CENTER_LEFT,
        3.0,
    );
    let read = (1..=CHAPTERS).filter(|&c| chapter_done(c, save)).count();
    ui.pill(
        Vec2::new(400.0, 318.0),
        200.0,
        t(l, "CORRECTED", "已订正"),
        format!("{read} / {CHAPTERS}"),
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
    // The page tabs: one per chapter; unopened chapters stay blank.
    for c in 1..=CHAPTERS {
        let open = hub::chapter_open(c, save);
        let x = -578.0 + (c - 1) as f32 * 70.0;
        let mut b = ButtonSpec::new(
            Vec2::new(x, 262.0),
            Vec2::new(62.0, 30.0),
            if open { c.to_string() } else { "·".into() },
            Act::Page(c),
        )
        .font(13.0)
        .enabled(open)
        .selected(c == n);
        if c != n {
            b = b.tone(Tone::Ghost);
        }
        ui.button(b);
        if chapter_done(c, save) {
            ui.dot(Vec2::new(x + 22.0, 271.0), 6.0, pal.primary, 12.0);
        }
    }
    draw_page(ui, v, n);
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

const PAGE: (Vec2, Vec2) = (Vec2::new(0.0, 10.0), Vec2::new(1240.0, 460.0));

fn draw_page(ui: &mut Ui, v: &View, n: u8) {
    let l = v.lang;
    let pal = ui.pal;
    let save = &v.game.save;
    let ch = content::chapter(n);
    let note = content::note(n);
    let done = chapter_done(n, save);
    let (at, size) = PAGE;
    ui.card(at, size, hub::chapter_accent(ch, &pal));
    let top = at.y + size.y / 2.0;
    let left = at.x - size.x / 2.0 + 36.0;
    ui.caption(
        format!("{} · {}", hub::chapter_word(n, l), ch.machine.get(l)),
        Vec2::new(left, top - 30.0),
        hub::chapter_accent(ch, &pal),
        Anchor::CENTER_LEFT,
    );
    ui.text(
        ch.title.get(l),
        Vec2::new(left, top - 58.0),
        22.0,
        pal.text,
        Anchor::CENTER_LEFT,
    );
    // Left: Bai Yan's page, in his hand.
    let col = 560.0;
    ui.caption(
        t(l, "BAI YAN · TRACED FROM THE STONE", "白砚 · 从石头上手描"),
        Vec2::new(left, top - 100.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    draw_trace(
        ui,
        n,
        Vec2::new(left + col / 2.0, top - 170.0),
        Vec2::new(col, 110.0),
        done,
    );
    ui.wrapped(
        note.line.get(l),
        Vec2::new(left, top - 250.0),
        col,
        21.0,
        pal.secondary,
        Anchor::TOP_LEFT,
    );
    ui.text(
        t(l, "In the margin: the stone echoed again today.", "页边：石头今天又回响了。"),
        Vec2::new(left, top - 372.0),
        13.0,
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    // Right: ORI's correction beside it, once the chapter is delivered.
    let rx = at.x + 40.0;
    let rw = size.x / 2.0 - 76.0;
    ui.rect(
        Vec2::new(at.x + 10.0, at.y - 10.0),
        Vec2::new(1.0, size.y - 150.0),
        pal.line,
        6.0,
    );
    ui.dot(Vec2::new(rx + 6.0, top - 100.0), 10.0, pal.primary, 8.0);
    ui.caption(
        t(l, "ORI · CORRECTION", "ORI · 订正"),
        Vec2::new(rx + 20.0, top - 100.0),
        pal.primary,
        Anchor::CENTER_LEFT,
    );
    if done {
        ui.glow(Vec2::new(rx + 6.0, top - 100.0), 60.0, pal.primary.with_alpha(0.3), 7.0);
        ui.wrapped(
            note.meaning.get(l),
            Vec2::new(rx, top - 132.0),
            rw,
            19.0,
            pal.text,
            Anchor::TOP_LEFT,
        );
        ui.wrapped(
            t(
                l,
                "He had no machine to check it on. It was right anyway.",
                "他没有机器可以验证。可他写对了。",
            ),
            Vec2::new(rx, top - 250.0),
            rw,
            14.0,
            pal.text_dim,
            Anchor::TOP_LEFT,
        );
        ui.text(
            format!("{} · {}", t(l, "Delivered to", "已交付"), ch.client_name().get(l)),
            Vec2::new(rx, top - 372.0),
            13.0,
            pal.good,
            Anchor::CENTER_LEFT,
        );
    } else {
        ui.wrapped(
            t(
                l,
                "Not yet. Deliver this chapter's commission and I will correct the page.",
                "还没到时候。交付这一章的委托，我再来订正这一页。",
            ),
            Vec2::new(rx, top - 132.0),
            rw,
            16.0,
            pal.text_faint,
            Anchor::TOP_LEFT,
        );
    }
    let y = at.y - size.y / 2.0 + 40.0;
    ui.text(
        t(
            l,
            "Both versions stay in the archive, side by side.",
            "新旧两版并排留在档案里。",
        ),
        Vec2::new(left, y),
        13.0,
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    if ending_open(save) {
        ui.button(
            ButtonSpec::new(
                Vec2::new(at.x + size.x / 2.0 - 130.0, y),
                Vec2::new(200.0, 34.0),
                t(l, "Cen Li's offer ›", "岑理的方案 ›"),
                Act::Goto(Screen::Ending),
            )
            .primary()
            .font(14.0),
        );
    }
}

/// Cen Li's offer, the choice, and what follows it.
pub fn draw_ending(ui: &mut Ui, v: &View) {
    let l = v.lang;
    let pal = ui.pal;
    let save = &v.game.save;
    let y = if save.ending.is_some() { -100.0 } else { 20.0 };
    draw_window(ui, save, Vec2::new(0.0, y), 52.0);
    let Some(end) = save.ending else {
        let at = Vec2::new(-470.0, 150.0);
        ui.well(at, Vec2::splat(124.0));
        oo_kit::portrait::spawn(ui, Cat::Cenli, at, 120.0, 20.0);
        ui.caption(
            t(l, "CEN LI · ACADEMY EXAMINER", "岑理 · 科学院审查官"),
            Vec2::new(-380.0, 196.0),
            pal.tertiary,
            Anchor::CENTER_LEFT,
        );
        ui.wrapped(
            t(
                l,
                "\"The quarter is up, and the notes hold. The Academy will take the work, and ORI, into its own keeping. In the Star Capital's machine room she would run a thousand times faster.\"",
                "“一个季度到了，译注站住了。科学院打算把成果和 ORI 一起收去集中保管。她在星都的机房里会快一千倍。”",
            ),
            Vec2::new(-380.0, 172.0),
            860.0,
            19.0,
            pal.text,
            Anchor::TOP_LEFT,
        );
        choice(
            ui,
            Vec2::new(-260.0, -150.0),
            t(l, "Migrate · Star Capital", "迁移 · 星都"),
            t(
                l,
                "Hand the work and ORI to the Academy. She will be fast, and the Grid Hall will be sealed.",
                "把成果和 ORI 交给科学院。她会很快，栅厅会被封存。",
            ),
            Act::Choose(false),
            pal.secondary,
        );
        choice(
            ui,
            Vec2::new(260.0, -150.0),
            t(l, "Stay · Keep the lamp on", "留下 · 留灯"),
            t(
                l,
                "Turn the Grid Hall into the town's machine room. ORI stays slow, and stays here.",
                "把栅厅改成镇上的机房。ORI 依然慢，依然在这里。",
            ),
            Act::Choose(true),
            pal.primary,
        );
        return;
    };
    let (title, body) = match end {
        Ending::Migrate => (
            t(l, "STAR CAPITAL", "星都"),
            t(
                l,
                "The work and ORI went to the Academy. In the Star Capital's machine room she is astonishingly fluent, and no longer remembers the town's water level, the arcade or the sand-wind map. The Grid Hall is sealed.",
                "成果和 ORI 交给了科学院。她在星都的机房里流利得惊人，却不再记得镇上的水位、街机和沙风图。栅厅封存了。",
            ),
        ),
        Ending::Stay => (
            t(l, "KEEP THE LAMP ON", "留灯"),
            t(
                l,
                "The Grid Hall is the town's machine room now. The replicas run the kernels you wrote: the water board, the arcade, the sand-wind map. ORI is still slow, and still correcting the notes.",
                "栅厅成了镇上的机房。复刻机跑着你写的内核：水位牌、街机、沙风图。ORI 依然慢，依然在订正译注。",
            ),
        ),
    };
    ui.text_spaced(title, Vec2::new(0.0, 250.0), 30.0, pal.text, Anchor::CENTER, 6.0);
    ui.wrapped(body, Vec2::new(-430.0, 210.0), 860.0, 18.0, pal.text_dim, Anchor::TOP_LEFT);
    ui.wrapped(
        t(
            l,
            "Bai Yan was not mad. Every strange sentence in his notes has its place in the grid pattern. The stone does not answer the fastest machine, only the one that really did something for someone.",
            "白砚没有疯。译注里每一句怪话，在栅纹里都有对应。回响石不回应最快的机器，只回应真的替谁做成事的机器。",
        ),
        Vec2::new(-430.0, 110.0),
        860.0,
        16.0,
        pal.text_faint,
        Anchor::TOP_LEFT,
    );
    crate::credits::draw(ui, v, end);
}

fn choice(ui: &mut Ui, at: Vec2, title: &str, body: &str, act: Act, accent: Color) {
    let pal = ui.pal;
    ui.card(at, Vec2::new(460.0, 190.0), accent);
    ui.text(title, at + Vec2::new(0.0, 56.0), 20.0, accent, Anchor::CENTER);
    ui.wrapped(body, at + Vec2::new(-200.0, 30.0), 400.0, 14.0, pal.text_dim, Anchor::TOP_LEFT);
    ui.button(
        ButtonSpec::new(at + Vec2::new(0.0, -62.0), Vec2::new(180.0, 36.0), title, act)
            .tone(Tone::Secondary)
            .font(13.0),
    );
}

/// The observation window over the stone room: sixteen segments of the grid
/// pattern, one lit per delivered chapter, and once the choice is made the
/// first warm node of the layer below. `w` is the pitch of one segment.
pub(crate) fn draw_window(ui: &mut Ui, save: &SaveData, at: Vec2, w: f32) {
    let pal = ui.pal;
    let y = at.y;
    let x0 = at.x - (CHAPTERS as f32 - 1.0) * w / 2.0;
    ui.well(at, Vec2::new(CHAPTERS as f32 * w + 24.0, 40.0));
    for c in 1..=CHAPTERS {
        let p = Vec2::new(x0 + (c - 1) as f32 * w, y);
        let lit = chapter_done(c, save);
        let colour = if lit { pal.primary } else { pal.line };
        ui.chip(p, Vec2::new(w - 8.0, 18.0), colour.with_alpha(if lit { 0.75 } else { 0.3 }), 7.0);
    }
    if save.ending.is_some() {
        let warm = Color::srgb(0.95, 0.66, 0.36);
        let p = Vec2::new(at.x, y - 46.0);
        ui.line(Vec2::new(at.x, y - 20.0), p, 2.0, warm.with_alpha(0.7), 7.0);
        ui.glow(p, 70.0, warm.with_alpha(0.45), 7.0);
        ui.dot(p, 12.0, warm, 7.5);
    }
}
