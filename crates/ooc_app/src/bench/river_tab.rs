use super::*;

pub(super) fn draw_river(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let (at, size) = river::RIVER;
    ui.chip(at, size + Vec2::splat(6.0), pal.line.with_alpha(0.6), 6.0);
    ui.commands.spawn((
        Sprite {
            image: v.fb.river.clone(),
            custom_size: Some(size),
            ..default()
        },
        Transform::from_xyz(at.x, at.y, 7.0),
        oo_kit::ui::ScreenEntity,
    ));
    let left = at.x - size.x / 2.0;
    let top = at.y + size.y / 2.0;
    ui.caption(
        "0",
        Vec2::new(left - 8.0, top - 3.0),
        pal.text_faint,
        Anchor::CENTER_RIGHT,
    );
    ui.caption(
        "31",
        Vec2::new(left - 8.0, top - size.y + 3.0),
        pal.text_faint,
        Anchor::CENTER_RIGHT,
    );
    let clue = s
        .task()
        .clue
        .filter(|_| !v.game.save.has_evidence(s.task()));
    match clue {
        // Fault levels: the river is where the evidence is found.
        Some(c) => ui.caption(
            format!(
                "{} {} · {}",
                t(l, "FIND", "找出"),
                c.what.get(l),
                t(l, "CLICK A COLUMN TO REWIND", "点击某一列可倒回")
            ),
            Vec2::new(left, top + 14.0),
            pal.warn,
            Anchor::CENTER_LEFT,
        ),
        None => ui.caption(
            t(
                l,
                "EACH COLUMN IS ONE WARP-INSTRUCTION · NEWEST AT THE RIGHT",
                "每一列是一条 warp 指令 · 最新的在右边",
            ),
            Vec2::new(left, top + 14.0),
            pal.text_faint,
            Anchor::CENTER_LEFT,
        ),
    };
    let legend = [
        (
            river::river_colour(river::RIVER_FULL),
            t(l, "all 32 lanes work", "32 条车道都在干活"),
        ),
        (
            river::river_colour(river::RIVER_SPLIT),
            t(l, "divergent: some lanes wait", "发散：部分车道在等"),
        ),
        (
            river::river_colour(river::RIVER_IDLE),
            t(l, "idle lane", "闲置车道"),
        ),
        (
            river::river_colour(river::RIVER_STALE),
            t(l, "read too early", "读得太早"),
        ),
    ];
    let shown = if s.task().profile.latency > 1 { 4 } else { 3 };
    for (i, (c, label)) in legend.into_iter().take(shown).enumerate() {
        let p = Vec2::new(left + [0.0, 170.0, 410.0, 540.0][i], -318.0);
        ui.chip(p + Vec2::new(5.0, 0.0), Vec2::splat(10.0), c, 7.0);
        ui.text(
            label,
            p + Vec2::new(16.0, 0.0),
            12.0,
            pal.text_dim,
            Anchor::CENTER_LEFT,
        );
    }
    let x = -90.0;
    let e = ui.text(
        "",
        Vec2::new(x, -146.0),
        15.0,
        pal.text,
        Anchor::CENTER_LEFT,
    );
    ui.commands.entity(e).insert(river::RiverStat(0));
    let e = ui.text(
        "",
        Vec2::new(x, -172.0),
        13.0,
        pal.secondary,
        Anchor::CENTER_LEFT,
    );
    ui.commands.entity(e).insert(river::RiverStat(1));
    let Some(rep) = &s.report else {
        let help = t(
            l,
            "Run the program to watch the warps flow. A warp is 32 lanes that issue one instruction together; when lanes disagree on a branch, the warp runs both sides while some lanes wait.",
            "运行程序，看 warp 如何流动。一个 warp 是 32 条一起发指令的车道；当车道在分支上意见不一，warp 会两边都跑一遍，另一部分车道只能等着。",
        );
        ui.wrapped(
            help,
            Vec2::new(x, -200.0),
            690.0,
            13.0,
            pal.text_dim,
            Anchor::TOP_LEFT,
        );
        return;
    };
    let run = &rep.runs[s.case.min(rep.runs.len() - 1)].run;
    let util = run.stats.utilization();
    ui.caption(
        t(l, "THIS CASE", "本用例"),
        Vec2::new(x, -208.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let stats = [
        (t(l, "CYCLES", "周期"), run.stats.cycles.to_string()),
        (t(l, "LOADS", "读取"), run.stats.loads.to_string()),
        (t(l, "STORES", "写入"), run.stats.stores.to_string()),
    ];
    for (i, (label, value)) in stats.into_iter().enumerate() {
        ui.pill(
            Vec2::new(x + 80.0 + i as f32 * 170.0, -236.0),
            160.0,
            label,
            value,
            pal.primary,
        );
    }
    ui.caption(
        t(l, "LANE UTILIZATION", "车道利用率"),
        Vec2::new(x, -272.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let accent = if util > 0.9 {
        pal.good
    } else if util > 0.6 {
        pal.secondary
    } else {
        pal.warn
    };
    ui.progress(
        Vec2::new(x + 250.0, -292.0),
        Vec2::new(500.0, 6.0),
        util,
        accent,
    );
    ui.text(
        format!("{:.0}%", util * 100.0),
        Vec2::new(x + 510.0, -292.0),
        15.0,
        accent,
        Anchor::CENTER_LEFT,
    );
    if run.stats.dropped > 0 {
        let msg = format!(
            "{} {}",
            run.stats.dropped,
            t(
                l,
                "stores fell outside the buffer and were dropped.",
                "次写入落在缓冲区外，被丢弃了。"
            )
        );
        ui.text(
            msg,
            Vec2::new(x, -318.0),
            12.0,
            pal.warn,
            Anchor::CENTER_LEFT,
        );
    }
}
