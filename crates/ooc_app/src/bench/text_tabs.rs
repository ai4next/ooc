use super::*;

pub(super) fn draw_hints(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let task = s.task();
    let names = [
        t(l, "WHERE TO LOOK", "往哪里看"),
        t(l, "WHICH MECHANISM", "用什么机制"),
        t(l, "WALKTHROUGH · CAPS AT ★★", "完整解法 · 最多 ★★"),
    ];
    for (i, tier) in HintTier::ALL.into_iter().enumerate() {
        let y = -140.0 - i as f32 * 66.0;
        let open = s.hints.revealed >= tier as u8;
        let color = if open { pal.secondary } else { pal.text_faint };
        ui.dot(Vec2::new(-604.0, y - 2.0), 8.0, color, 12.0);
        ui.caption(
            format!("{} · {}", i + 1, names[i]),
            Vec2::new(-592.0, y - 2.0),
            color,
            Anchor::CENTER_LEFT,
        );
        let body = Vec2::new(-592.0, y - 14.0);
        if open {
            ui.wrapped(
                task.hints[i].get(l),
                body,
                1180.0,
                14.0,
                pal.text,
                Anchor::TOP_LEFT,
            );
            continue;
        }
        if s.hints.next() != Some(tier) {
            continue;
        }
        if s.hints.is_available(tier) {
            let label = if tier == HintTier::Walkthrough {
                t(l, "Show the walkthrough", "显示完整解法")
            } else {
                t(l, "Show this hint", "显示这条提示")
            };
            ui.button(
                ButtonSpec::new(
                    body + Vec2::new(90.0, -12.0),
                    Vec2::new(180.0, 28.0),
                    label,
                    Act::Hint,
                )
                .font(13.0),
            );
        } else {
            let (runs, secs) = if tier == HintTier::Mechanism {
                (1, 45)
            } else {
                (3, 150)
            };
            let left = s.hints.wait_for_next().unwrap_or(0.0).ceil() as i32;
            let need = runs - s.hints.failures.min(runs);
            let msg = oo_base::Tr::new(
                "Opens after {} more failed run(s), or in {} s ({} runs / {} s).",
                "再失败 {} 次，或再过 {} 秒后开放（共 {} 次 / {} 秒）。",
            )
            .fill(l, &[&need, &left, &runs, &secs]);
            ui.text(
                msg,
                body + Vec2::new(0.0, -10.0),
                13.0,
                pal.text_faint,
                Anchor::CENTER_LEFT,
            );
        }
    }
}

/// The program as OC-1 assembly; for circuits, what the board compiles to.
pub(super) fn draw_listing(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let program = match (&s.report, s.work.as_ref()) {
        (Some(r), _) => Some(r.program.clone()),
        (None, Some(sol)) => ooc_core::grade::build(s.task(), sol)
            .ok()
            .map(|b| b.program),
        (None, None) => None,
    };
    let head = match s.task().editor {
        Editor::Graph(_) => t(
            l,
            "YOUR CIRCUIT, COMPILED: EVERY BULB RUNS THESE LINES",
            "你的电路编译后的样子：每个灯泡都运行这些指令",
        ),
        Editor::Code(_) => t(l, "THE PROGRAM AS OC-1 ASSEMBLY", "OC-1 汇编形式的程序"),
    };
    ui.caption(
        head,
        Vec2::new(-612.0, -136.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
    let Some(program) = program else {
        ui.text(
            t(
                l,
                "It does not build yet — see the message above.",
                "现在还无法构建——看上面的提示。",
            ),
            Vec2::new(-612.0, -164.0),
            14.0,
            pal.text_dim,
            Anchor::CENTER_LEFT,
        );
        return;
    };
    let text = ooc_core::isa::listing(&program.code);
    const PER_COL: usize = 9;
    for (i, line) in text.lines().enumerate() {
        let (c, r) = ((i / PER_COL) as f32, (i % PER_COL) as f32);
        let p = Vec2::new(-612.0 + c * 250.0, -160.0 + -r * 20.0);
        ui.text(
            format!("{:>2}", i + 1),
            p,
            11.0,
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
        ui.text(
            line,
            p + Vec2::new(22.0, 0.0),
            13.0,
            pal.text,
            Anchor::CENTER_LEFT,
        );
    }
}
