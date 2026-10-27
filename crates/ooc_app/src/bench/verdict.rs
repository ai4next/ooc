use super::draw::{TAB_Y, cell_name};
use super::*;

/// One line on the tab row: build error, running, or the verdict.
pub(super) fn draw_verdict(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let at = Vec2::new(-56.0, TAB_Y);
    if let Some(e) = &s.error {
        ui.dot(at, 9.0, pal.bad, 12.0);
        ui.text(
            format!("{}  {e}", t(l, "Does not build.", "无法构建。")),
            at + Vec2::new(14.0, 0.0),
            14.0,
            pal.bad,
            Anchor::CENTER_LEFT,
        );
        return;
    }
    let Some(rep) = &s.report else {
        ui.text(
            t(
                l,
                "Not run yet — press Run to test every case.",
                "还没有运行——按“运行”测试所有用例。",
            ),
            at,
            14.0,
            pal.text_faint,
            Anchor::CENTER_LEFT,
        );
        return;
    };
    // Fault levels: pointing works mid-replay, so pause on the moment.
    if !v.game.save.has_evidence(s.task()) && clue_at(s).is_some() {
        ui.button(
            ButtonSpec::new(
                Vec2::new(548.0, TAB_Y),
                Vec2::new(120.0, 30.0),
                t(l, "It broke here", "就是这一拍"),
                Act::Point,
            )
            .primary()
            .font(13.0)
            .tip(t(
                l,
                "Point at the replay's latest step (the right edge of the Warp River). Click the river or use ← → to move.",
                "指证回放的最新一步（Warp River 的最右一列）。点击河流或按 ← → 移动。",
            )),
        );
    }
    if !s.play.revealed {
        let msg = format!(
            "{} {} / {}…",
            t(l, "Replaying case", "正在回放用例"),
            s.case + 1,
            rep.public
        );
        ui.text(msg, at, 14.0, pal.text_dim, Anchor::CENTER_LEFT);
        return;
    }
    let view = s.task().view;
    let (msg, color) = match rep.verdict {
        Verdict::Wrong { case, .. } | Verdict::Timeout { case } if rep.is_hidden(case) => (
            format!(
                "× {}",
                t(
                    l,
                    "A hidden case fails: the rule fits the examples, not every input.",
                    "隐藏用例没通过：规则只对上了示例，没对上所有输入。"
                )
            ),
            pal.bad,
        ),
        Verdict::Pass => (
            format!(
                "✓ {} {}{}",
                t(l, "Every case passes:", "全部用例通过："),
                rep.public,
                match rep.runs.len() - rep.public {
                    0 => String::new(),
                    h => format!(" + {h} {}", t(l, "hidden", "隐藏")),
                }
            ),
            pal.good,
        ),
        Verdict::Wrong {
            case,
            cell,
            got,
            want,
        } => (
            format!(
                "× {} {} · {} · {} {got}, {} {want}",
                t(l, "Case", "用例"),
                case + 1,
                cell_name(view, cell, l),
                t(l, "got", "得到"),
                t(l, "want", "应为")
            ),
            pal.bad,
        ),
        Verdict::Timeout { case } => (
            format!(
                "× {} {} · {}",
                t(l, "Case", "用例"),
                case + 1,
                t(
                    l,
                    "the watchdog stopped the kernel. A loop that never ends?",
                    "看门狗停下了内核。是不是循环停不下来？"
                )
            ),
            pal.bad,
        ),
        Verdict::OverLimit { goal, got } => (
            format!(
                "△ {} {got} · {} {}",
                t(l, "Correct, but over the limit:", "结果正确，但超出上限："),
                t(l, "needs", "要求"),
                hub::goal_text(goal, l)
            ),
            pal.warn,
        ),
    };
    ui.text(msg, at, 14.0, color, Anchor::CENTER_LEFT);
    let found = v.game.save.has_evidence(s.task());
    if rep.passed() && !found {
        ui.text(
            t(
                l,
                "Fixed. Now reset, run the starter and point at the cause.",
                "修好了。再还原初始电路、运行，并指出原因。",
            ),
            Vec2::new(608.0, TAB_Y),
            13.0,
            pal.warn,
            Anchor::CENTER_RIGHT,
        );
    } else if rep.passed() {
        ui.stars(Vec2::new(430.0, TAB_Y), rep.stars(), 9.0);
        ui.button(
            ButtonSpec::new(
                Vec2::new(548.0, TAB_Y),
                Vec2::new(120.0, 30.0),
                t(l, "Results ›", "结果 ›"),
                Act::Next,
            )
            .primary()
            .font(14.0),
        );
    }
}
