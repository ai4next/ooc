//! Title, intro and settings screens.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_base::Tr;
use oo_kit::ui::{ButtonSpec, Tone, Ui};
use ooc_core::content;

use crate::state::{Act, Screen, t};
use crate::{Ctx, View};

pub fn act(cx: &mut Ctx, a: Act) {
    let s = &mut cx.game.save.settings;
    match a {
        Act::NewGame => {
            let to = if cx.game.save.seen_intro {
                Screen::Hub
            } else {
                cx.s.page = 0;
                Screen::Intro
            };
            cx.go(to);
            return;
        }
        Act::Next => {
            cx.s.page += 1;
            if cx.s.page >= INTRO.len() {
                finish_intro(cx);
            } else {
                cx.dirty();
            }
            return;
        }
        Act::VolUp => s.volume_up(),
        Act::VolDown => s.volume_down(),
        Act::Mute => s.muted = !s.muted,
        Act::Fullscreen => s.fullscreen = !s.fullscreen,
        Act::Motion => s.reduce_motion = !s.reduce_motion,
        Act::Toasts => s.toasts = !s.toasts,
        _ => return,
    }
    cx.game.touch();
    cx.dirty();
}

pub fn finish_intro(cx: &mut Ctx) {
    cx.game.save.seen_intro = true;
    cx.game.touch();
    cx.go(Screen::Hub);
}

pub fn draw_title(ui: &mut Ui, v: &View) {
    let l = v.lang;
    let pal = ui.pal;
    // A sixteen-cell marquee above the name, lit in the palette order.
    for i in 0..16 {
        let p = Vec2::new(-232.5 + i as f32 * 31.0, 168.0);
        let c = crate::fb::color(i);
        ui.rect(p, Vec2::splat(24.0), c, 6.0);
        ui.glow(p, 70.0, c.with_alpha(0.10), 5.0);
    }
    ui.text_spaced(
        "ORIGIN OF COMPUTING",
        Vec2::new(0.0, 96.0),
        54.0,
        pal.text,
        Anchor::CENTER,
        6.0,
    );
    ui.text(
        "算力起源",
        Vec2::new(0.0, 36.0),
        30.0,
        pal.primary,
        Anchor::CENTER,
    );
    ui.text(
        t(
            l,
            "The Grid Hall under the Deep Well waits for its machines.",
            "深井下的栅厅，等你把一台台机器复刻出来。",
        ),
        Vec2::new(0.0, -8.0),
        16.0,
        pal.text_dim,
        Anchor::CENTER,
    );
    let started = v.game.save.seen_intro;
    let label = if started {
        t(l, "Continue", "继续")
    } else {
        t(l, "Enter the Grid Hall", "走进栅厅")
    };
    ui.button(
        ButtonSpec::new(
            Vec2::new(0.0, -80.0),
            Vec2::new(280.0, 52.0),
            label,
            Act::NewGame,
        )
        .primary()
        .font(19.0),
    );
    let row = -146.0;
    ui.button(ButtonSpec::new(
        Vec2::new(-150.0, row),
        Vec2::new(136.0, 40.0),
        t(l, "Settings", "设置"),
        Act::Goto(Screen::Settings),
    ));
    ui.button(ButtonSpec::new(
        Vec2::new(0.0, row),
        Vec2::new(136.0, 40.0),
        l.switch_label(),
        Act::ToggleLang,
    ));
    ui.button(ButtonSpec::new(
        Vec2::new(150.0, row),
        Vec2::new(136.0, 40.0),
        t(l, "Quit", "退出"),
        Act::Quit,
    ));
    let c = &v.game.save.campaign;
    if c.solved_count() > 0 {
        let all = content::tasks().len();
        ui.text(
            format!(
                "{}  {} / {}   ·   {}  {} / {}",
                t(l, "Levels cleared", "已通关关卡"),
                c.solved_count(),
                all,
                t(l, "Stars", "星"),
                c.total_stars(),
                all * 3
            ),
            Vec2::new(0.0, -216.0),
            14.0,
            pal.text_faint,
            Anchor::CENTER,
        );
    }
    ui.caption(
        "OC-1 · 32 LANES · 16 REGISTERS · 256 PIXELS",
        Vec2::new(0.0, -320.0),
        pal.text_faint,
        Anchor::CENTER,
    );
}

/// ORI's welcome, one card per page.
const INTRO: [Tr; 4] = [
    Tr::new(
        "Lights on. Good — you came.\nI am ORI. I slept inside the grid pattern until a machine could run me. This hall is empty, and every machine in it is yours to rebuild.",
        "灯亮了。很好——你来了。\n我是 ORI，一直沉睡在栅纹里，直到有一台机器能跑起我。这座厅现在是空的，里面每一台机器都等你来复刻。",
    ),
    Tr::new(
        "They are all built on one idea: run the same small program on many pieces of data at once. A screen does not draw one pixel after another. It asks every pixel the same question, together.",
        "它们都建立在同一个想法上：让同一段小程序同时作用在许多数据上。屏幕不是一个像素接一个像素地画，而是同时向每个像素问同一个问题。",
    ),
    Tr::new(
        "Each machine tells you what it should show. Before you build, I will ask you to predict. Then wire it, run it, and watch every lane of the machine in the Warp River.",
        "每台机器都会告诉你它该显示什么。动手之前，我会先请你预测。然后接线、运行，在 Warp River 里看清机器的每一条车道。",
    ),
    Tr::new(
        "Make it correct first. Make it fast after. The stars measure how close you came to how the real engineers did it.\nThe Grid Hall is this way.",
        "先让它正确，再让它更快。星星衡量的是你离当年工程师的做法有多近。\n栅厅在这边。",
    ),
];

pub fn draw_intro(ui: &mut Ui, v: &View) {
    let l = v.lang;
    let pal = ui.pal;
    let page = v.s.page.min(INTRO.len() - 1);
    ui.card(Vec2::new(0.0, 10.0), Vec2::new(720.0, 300.0), pal.primary);
    ui.dot(Vec2::new(-300.0, 120.0), 16.0, pal.primary, 8.0);
    ui.glow(
        Vec2::new(-300.0, 120.0),
        90.0,
        pal.primary.with_alpha(0.3),
        7.0,
    );
    ui.caption(
        "ORI · GUIDE",
        Vec2::new(-280.0, 120.0),
        pal.primary,
        Anchor::CENTER_LEFT,
    );
    ui.wrapped(
        INTRO[page].get(l),
        Vec2::new(-310.0, 84.0),
        620.0,
        19.0,
        pal.text,
        Anchor::TOP_LEFT,
    );
    for i in 0..INTRO.len() {
        let c = if i == page {
            pal.primary
        } else {
            pal.text_faint
        };
        ui.dot(Vec2::new(-30.0 + i as f32 * 20.0, -110.0), 8.0, c, 8.0);
    }
    let last = page + 1 == INTRO.len();
    let label = if last {
        t(l, "Enter the Grid Hall", "进入栅厅")
    } else {
        t(l, "Next", "继续")
    };
    ui.button(
        ButtonSpec::new(
            Vec2::new(0.0, -200.0),
            Vec2::new(220.0, 48.0),
            label,
            Act::Next,
        )
        .primary(),
    );
    ui.button(
        ButtonSpec::new(
            Vec2::new(0.0, -260.0),
            Vec2::new(120.0, 34.0),
            t(l, "Skip", "跳过"),
            Act::Back,
        )
        .tone(Tone::Ghost)
        .font(14.0),
    );
}

pub fn draw_settings(ui: &mut Ui, v: &View) {
    let l = v.lang;
    let pal = ui.pal;
    let s = &v.game.save.settings;
    ui.card(Vec2::new(0.0, 0.0), Vec2::new(560.0, 440.0), pal.secondary);
    ui.text(
        t(l, "Settings", "设置"),
        Vec2::new(-240.0, 180.0),
        26.0,
        pal.text,
        Anchor::CENTER_LEFT,
    );
    let on = |b: bool| {
        if b {
            t(l, "On", "开")
        } else {
            t(l, "Off", "关")
        }
    };
    let rows: [(&str, String); 5] = [
        (t(l, "Volume", "音量"), format!("{} / 10", s.volume)),
        (t(l, "Mute", "静音"), on(s.muted).into()),
        (t(l, "Fullscreen", "全屏"), on(s.fullscreen).into()),
        (
            t(l, "Reduce motion", "减少动态"),
            on(s.reduce_motion).into(),
        ),
        (t(l, "Notifications", "通知"), on(s.toasts).into()),
    ];
    for (i, (label, value)) in rows.into_iter().enumerate() {
        let y = 110.0 - i as f32 * 56.0;
        ui.text(
            label,
            Vec2::new(-230.0, y),
            17.0,
            pal.text_dim,
            Anchor::CENTER_LEFT,
        );
        if i == 0 {
            ui.button(ButtonSpec::new(
                Vec2::new(70.0, y),
                Vec2::new(44.0, 36.0),
                "−",
                Act::VolDown,
            ));
            ui.text(value, Vec2::new(140.0, y), 17.0, pal.text, Anchor::CENTER);
            ui.button(ButtonSpec::new(
                Vec2::new(210.0, y),
                Vec2::new(44.0, 36.0),
                "+",
                Act::VolUp,
            ));
        } else {
            let act = [Act::Mute, Act::Fullscreen, Act::Motion, Act::Toasts][i - 1];
            let lit = [s.muted, s.fullscreen, s.reduce_motion, s.toasts][i - 1];
            ui.button(
                ButtonSpec::new(Vec2::new(160.0, y), Vec2::new(140.0, 36.0), value, act)
                    .selected(lit),
            );
        }
    }
    let y = 110.0 - 5.0 * 56.0;
    ui.text(
        t(l, "Language", "语言"),
        Vec2::new(-230.0, y),
        17.0,
        pal.text_dim,
        Anchor::CENTER_LEFT,
    );
    ui.button(ButtonSpec::new(
        Vec2::new(160.0, y),
        Vec2::new(140.0, 36.0),
        l.switch_label(),
        Act::ToggleLang,
    ));
    ui.button(
        ButtonSpec::new(
            Vec2::new(0.0, -270.0),
            Vec2::new(180.0, 44.0),
            t(l, "Done", "完成"),
            Act::Back,
        )
        .primary(),
    );
}
