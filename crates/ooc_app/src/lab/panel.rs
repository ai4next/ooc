//! The free bench's drawing: the lab panel on the bench strip, the kernel
//! chips in the top bar, the input preview and the teaching captions.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use oo_base::Lang;
use oo_kit::ui::{ButtonSpec, Tone, Ui};
use ooc_core::isa::OpKind;
use ooc_core::lab::frame::MAX_KERNELS;
use ooc_core::lab::input::{Input, Set};
use ooc_core::lab::slot::SLOTS;
use ooc_core::lab::{self as core_lab};
use ooc_core::task::{Editor, View as TaskView};

use super::{LabAct, Pick, import, sync};
use crate::View;
use crate::state::{Act, Session, t};

fn la(a: LabAct) -> Act {
    Act::LabDo(a)
}

/// Column left edges and the caption row of the panel.
const COLS: [f32; 4] = [-612.0, -316.0, 4.0, 300.0];
const CAP_Y: f32 = -138.0;

pub fn draw_panel(ui: &mut Ui, v: &View) {
    draw_slots(ui, v);
    draw_input(ui, v);
    draw_machine(ui, v);
    draw_share(ui, v);
}

/// At most `n` characters, with an ellipsis when cut.
fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n.saturating_sub(1)).collect::<String>())
    }
}

fn draw_slots(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let x0 = COLS[0];
    ui.caption(t(l, "SAVE SLOTS", "存档位"), Vec2::new(x0, CAP_Y), pal.text_faint, Anchor::CENTER_LEFT);
    let slots = &v.game.save.lab_slots;
    for i in 0..SLOTS {
        let (c, r) = ((i % 2) as f32, (i / 2) as f32);
        let p = Vec2::new(x0 + 70.0 + c * 142.0, -168.0 - r * 38.0);
        let slot = slots.get(i);
        let label = match slot.filter(|x| !x.is_empty()) {
            None => format!("{}  {}", i + 1, t(l, "free", "空")),
            Some(x) => {
                let name = if x.name.is_empty() {
                    core_lab::index(&x.machine)
                        .map_or_else(String::new, |k| core_lab::task(k).title.get(l).to_string())
                } else {
                    x.name.clone()
                };
                let n = x.kernels.len();
                let ks = if n > 1 { format!(" ×{n}") } else { String::new() };
                format!("{}  {}{ks}", i + 1, clip(&name, 11))
            }
        };
        let on = s.lab_state.slot == Some(i);
        let free = slot.is_none_or(|x| x.is_empty());
        let mut b = ButtonSpec::new(p, Vec2::new(136.0, 32.0), label, la(LabAct::Slot(i)))
            .font(12.0)
            .selected(on)
            .tip(if free {
                t(l, "Keep this frame here.", "把这一帧存到这里。")
            } else {
                t(l, "Load this frame. The current one stays in its slot.", "载入这一帧。当前这一帧留在原来的存档位。")
            });
        if !on {
            b = b.tone(if free { Tone::Ghost } else { Tone::Secondary });
        }
        ui.button(b);
    }
    let y = -288.0;
    match s.lab_state.slot {
        Some(i) => {
            ui.text(
                format!("{} {}", t(l, "Saving into slot", "自动存入存档位"), i + 1),
                Vec2::new(x0, y),
                12.0,
                pal.text_dim,
                Anchor::CENTER_LEFT,
            );
            ui.button(
                ButtonSpec::new(Vec2::new(x0 + 236.0, y), Vec2::new(72.0, 22.0), t(l, "Free it", "清空"), la(LabAct::ClearSlot(i)))
                    .font(11.0)
                    .tone(Tone::Ghost)
                    .tip(t(l, "Empty this slot. The frame stays on the bench.", "清空这个存档位，机台上的这一帧不变。")),
            );
        }
        None => {
            ui.wrapped(
                t(l, "Not saved: click a free slot to keep this frame.", "未保存：点一个空位来保存这一帧。"),
                Vec2::new(x0, y + 8.0),
                280.0,
                12.0,
                pal.warn,
                Anchor::TOP_LEFT,
            );
        }
    }
    ui.wrapped(
        t(l, "Each slot keeps a machine, its kernels, the input and the screen size.", "每个存档位保存机器、内核、输入和屏幕大小。"),
        Vec2::new(x0, -308.0),
        280.0,
        11.0,
        pal.text_faint,
        Anchor::TOP_LEFT,
    );
}

/// What the input holds, in one line.
fn input_about(s: &Session, l: Lang) -> String {
    match &s.lab_state.input {
        Input::Level => t(l, "The level's own public cases.", "关卡自带的公开用例。").into(),
        Input::Set(set) => set.about().get(l).to_string(),
        Input::Data(d) => format!(
            "{} · {}×{} · {} {}",
            clip(&d.name, 18),
            d.w,
            d.h(),
            t(l, "max", "最大"),
            d.max()
        ),
    }
}

fn draw_input(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let x0 = COLS[1];
    ui.caption(t(l, "INPUT · ROM", "输入 · ROM"), Vec2::new(x0, CAP_Y), pal.text_faint, Anchor::CENTER_LEFT);
    let input = &s.lab_state.input;
    let mut picks = vec![(Pick::Level, t(l, "Level", "关卡").to_string(), *input == Input::Level)];
    for set in Set::ALL {
        picks.push((Pick::Set(set), set.name().get(l).to_string(), *input == Input::Set(set)));
    }
    for (i, (pick, label, on)) in picks.into_iter().enumerate() {
        let (c, r) = ((i % 3) as f32, (i / 3) as f32);
        let p = Vec2::new(x0 + 48.0 + c * 100.0, -164.0 - r * 30.0);
        let mut b = ButtonSpec::new(p, Vec2::new(96.0, 26.0), label, la(LabAct::Input(pick)))
            .font(if l == Lang::En { 11.0 } else { 12.0 })
            .selected(on);
        if !on {
            b = b.tone(Tone::Ghost);
        }
        ui.button(b);
    }
    ui.text(input_about(s, l), Vec2::new(x0, -222.0), 11.0, pal.secondary, Anchor::CENTER_LEFT);
    ui.caption(t(l, "IMPORT FOLDER", "导入文件夹"), Vec2::new(x0, -244.0), pal.text_faint, Anchor::CENTER_LEFT);
    let files = &s.lab_state.files;
    let loaded = match input {
        Input::Data(d) => Some(d.name.as_str()),
        _ => None,
    };
    for (i, f) in files.iter().take(3).enumerate() {
        let name = import::name(f);
        let on = loaded == Some(name.as_str());
        let p = Vec2::new(x0 + 40.0 + i as f32 * 84.0, -266.0);
        let mut b = ButtonSpec::new(p, Vec2::new(80.0, 22.0), clip(&name, 11), la(LabAct::Input(Pick::File(i))))
            .font(10.0)
            .selected(on)
            .tip(name.clone());
        if !on {
            b = b.tone(Tone::Ghost);
        }
        ui.button(b);
    }
    if files.is_empty() {
        ui.text(t(l, "(empty)", "（空）"), Vec2::new(x0, -266.0), 11.0, pal.text_faint, Anchor::CENTER_LEFT);
    }
    ui.button(
        ButtonSpec::new(Vec2::new(x0 + 274.0, -266.0), Vec2::new(52.0, 22.0), t(l, "Rescan", "刷新"), la(LabAct::Rescan))
            .font(10.0)
            .tone(Tone::Ghost)
            .tip(t(l, "List the folder again (creates it if missing).", "重新列出文件夹（没有就新建）。")),
    );
    ui.wrapped(
        format!(
            "{} {}",
            t(l, "Drop a .png or .csv on the window, or put it in", "把 .png 或 .csv 拖进窗口，或放进"),
            import::folder().display()
        ),
        Vec2::new(x0, -284.0),
        300.0,
        10.0,
        pal.text_faint,
        Anchor::TOP_LEFT,
    );
}

/// Whether this machine can read its output buffer back (`LDO`).
fn reads_back(s: &Session) -> bool {
    match s.task().editor {
        Editor::Code(ops) => ops.contains(&OpKind::Ldo),
        Editor::Graph(_) => false,
    }
}

fn draw_machine(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    let st = &s.lab_state;
    let x0 = COLS[2];
    let screen = s.task().view == TaskView::Screen;
    ui.caption(t(l, "SCREEN", "屏幕"), Vec2::new(x0, CAP_Y), pal.text_faint, Anchor::CENTER_LEFT);
    let resizable = core_lab::resizable(s.task);
    for (i, w) in [16usize, 32].into_iter().enumerate() {
        let on = screen && st.screen == w;
        let p = Vec2::new(x0 + 46.0 + i as f32 * 96.0, -164.0);
        let mut b = ButtonSpec::new(p, Vec2::new(90.0, 26.0), format!("{w} × {w}"), la(LabAct::Screen(w)))
            .font(12.0)
            .selected(on)
            .enabled(resizable)
            .tip(t(l, "Pixels per row. X, Y and PIX follow the screen.", "每行像素数。X、Y 和 PIX 跟着屏幕走。"));
        if !on {
            b = b.tone(Tone::Ghost);
        }
        ui.button(b);
    }
    if !resizable {
        ui.text(t(l, "fixed", "固定"), Vec2::new(x0 + 200.0, -164.0), 11.0, pal.text_faint, Anchor::CENTER_LEFT);
    }
    let teach = st.teach;
    let mut b = ButtonSpec::new(
        Vec2::new(x0 + 94.0, -198.0),
        Vec2::new(186.0, 26.0),
        if teach { t(l, "Teaching mode: on", "教学模式：开") } else { t(l, "Teaching mode: off", "教学模式：关") },
        la(LabAct::Teach),
    )
    .font(12.0)
    .selected(teach)
    .tip(t(
        l,
        "Spell latencies out: cycles waited on the Warp River, beats left in the pipeline, memory costs.",
        "把延迟写成数字：Warp River 上等了几个周期、流水线还剩几拍、访存要多少周期。",
    ));
    if !teach {
        b = b.tone(Tone::Ghost);
    }
    ui.button(b);
    ui.caption(t(l, "FRAME", "一帧"), Vec2::new(x0, -228.0), pal.text_faint, Anchor::CENTER_LEFT);
    let n = st.kernels.len();
    let line = if st.ran() {
        let worst = st
            .split
            .iter()
            .max_by_key(|c| c.iter().sum::<u32>())
            .cloned()
            .unwrap_or_default();
        let parts: Vec<String> = worst.iter().map(u32::to_string).collect();
        if n > 1 {
            format!("{} = {} {}", parts.join(" + "), st.total, t(l, "cycles", "周期"))
        } else {
            format!("{} {}", st.total, t(l, "cycles", "周期"))
        }
    } else {
        t(l, "Run to measure the frame.", "运行后测量这一帧。").into()
    };
    ui.text(line, Vec2::new(x0, -250.0), 14.0, pal.secondary, Anchor::CENTER_LEFT);
    let about = if n == 1 {
        t(l, "Add kernels in the top bar (up to 4). Each starts from the last one's output, on the same ROM.", "在顶栏添加内核（最多 4 个）。每个都从上一个的输出开始，读同一份 ROM。")
    } else if reads_back(s) {
        t(l, "Each kernel starts from the last one's output (LDO reads it), on the same ROM. Cycles add up.", "每个内核从上一个的输出开始（用 LDO 读），读同一份 ROM。周期相加。")
    } else {
        t(l, "Each kernel starts from the last one's output. This machine cannot read the screen back (LDO comes later), so a kernel only draws over the last.", "每个内核从上一个的输出开始。这台机器还不能读回屏幕（LDO 以后才有），所以后一个只能在上面接着画。")
    };
    ui.wrapped(about, Vec2::new(x0, -268.0), 280.0, 11.0, pal.text_faint, Anchor::TOP_LEFT);
}

/// A long code shown in a few lines.
fn wrap_code(code: &str, width: usize, lines: usize) -> String {
    let chars: Vec<char> = code.chars().collect();
    let mut out: Vec<String> = chars.chunks(width).take(lines).map(|c| c.iter().collect()).collect();
    if chars.len() > width * lines
        && let Some(last) = out.last_mut()
    {
        last.pop();
        last.push('…');
    }
    out.join("\n")
}

fn draw_share(ui: &mut Ui, v: &View) {
    let pal = ui.pal;
    let l = v.lang;
    let st = &v.s.lab_state;
    let x0 = COLS[3];
    let w = 312.0;
    ui.caption(t(l, "SHARE CODE", "分享码"), Vec2::new(x0, CAP_Y), pal.text_faint, Anchor::CENTER_LEFT);
    ui.button(
        ButtonSpec::new(Vec2::new(x0 + 52.0, -164.0), Vec2::new(104.0, 26.0), t(l, "Copy code", "复制分享码"), la(LabAct::Copy))
            .font(12.0)
            .tip(t(l, "Machine, kernels, input, screen size and your cycles, as one code.", "机器、内核、输入、屏幕大小和你的周期，打包成一个码。")),
    );
    let well = Vec2::new(x0 + w / 2.0, -196.0);
    ui.well(well, Vec2::new(w, 34.0));
    let shown = if st.code.is_empty() {
        t(l, "No code yet.", "还没有分享码。").to_string()
    } else {
        wrap_code(&st.code, 48, 2)
    };
    ui.text(shown, well + Vec2::new(-w / 2.0 + 6.0, 0.0), 9.0, pal.text_dim, Anchor::CENTER_LEFT);
    ui.caption(t(l, "IMPORT", "导入"), Vec2::new(x0, -226.0), pal.text_faint, Anchor::CENTER_LEFT);
    let field = Vec2::new(x0 + w / 2.0, -250.0);
    ui.well(field, Vec2::new(w, 30.0));
    let (text, color) = if st.field.is_empty() {
        (t(l, "Ctrl/Cmd+V pastes a code here", "按 Ctrl/Cmd+V 把分享码贴到这里").to_string(), pal.text_faint)
    } else {
        (wrap_code(&st.field, 48, 2), pal.text)
    };
    ui.text(text, field + Vec2::new(-w / 2.0 + 6.0, 0.0), 9.0, color, Anchor::CENTER_LEFT);
    let y = -282.0;
    ui.button(
        ButtonSpec::new(Vec2::new(x0 + 36.0, y), Vec2::new(72.0, 24.0), t(l, "Paste", "贴入"), la(LabAct::Paste))
            .font(12.0)
            .tone(Tone::Ghost)
            .tip("Ctrl/Cmd+V"),
    );
    ui.button(
        ButtonSpec::new(Vec2::new(x0 + 104.0, y), Vec2::new(56.0, 24.0), t(l, "Clear", "清除"), la(LabAct::ClearField))
            .font(12.0)
            .tone(Tone::Ghost)
            .enabled(!st.field.is_empty()),
    );
    ui.button(
        ButtonSpec::new(Vec2::new(x0 + 240.0, y), Vec2::new(144.0, 24.0), t(l, "Import & verify", "导入并验证"), la(LabAct::Import))
            .primary()
            .font(12.0)
            .enabled(!st.field.is_empty())
            .tip(t(l, "Load the frame into a free slot and run it on this machine.", "把这一帧载入空存档位，并在这台机器上运行。")),
    );
    let at = Vec2::new(x0, -308.0);
    if let Some((author, here)) = st.check {
        let (msg, c) = if author == here {
            (format!("✓ {} · {here} {}", t(l, "verified", "已验证"), t(l, "cycles", "周期")), pal.good)
        } else {
            (
                format!("≠ {}: {} {author} · {} {here}", t(l, "differs", "不一致"), t(l, "author", "作者"), t(l, "here", "这里")),
                pal.warn,
            )
        };
        ui.text(msg, at, 13.0, c, Anchor::CENTER_LEFT);
    }
    if let Some((msg, bad)) = &st.note {
        let y = if st.check.is_some() { -322.0 } else { -300.0 };
        ui.wrapped(msg, Vec2::new(x0, y), w, 11.0, if *bad { pal.bad } else { pal.text_dim }, Anchor::TOP_LEFT);
    }
}

/// Kernel chips in the top bar, and "+" / "−".
pub fn draw_kernels(ui: &mut Ui, v: &View, y: f32) {
    let pal = ui.pal;
    let l = v.lang;
    let st = &v.s.lab_state;
    ui.caption(t(l, "KERNELS", "内核"), Vec2::new(-118.0, y), pal.text_faint, Anchor::CENTER_RIGHT);
    let n = st.kernels.len();
    for k in 0..n {
        let p = Vec2::new(-92.0 + k as f32 * 38.0, y);
        let on = st.cur == k;
        let mut b = ButtonSpec::new(p, Vec2::new(34.0, 28.0), (k + 1).to_string(), la(LabAct::Kernel(k)))
            .font(13.0)
            .selected(on)
            .tip(t(l, "Edit and replay this kernel.", "编辑并回放这个内核。"));
        if !on {
            b = b.tone(Tone::Ghost);
        }
        ui.button(b);
    }
    let x = -92.0 + n as f32 * 38.0;
    ui.button(
        ButtonSpec::new(Vec2::new(x, y), Vec2::new(28.0, 28.0), "+", la(LabAct::AddKernel))
            .font(15.0)
            .tone(Tone::Ghost)
            .enabled(n < MAX_KERNELS)
            .tip(t(l, "Add a kernel after the last one.", "在最后加一个内核。")),
    );
    ui.button(
        ButtonSpec::new(Vec2::new(x + 32.0, y), Vec2::new(28.0, 28.0), "−", la(LabAct::DropKernel))
            .font(15.0)
            .tone(Tone::Ghost)
            .enabled(n > 1)
            .tip(t(l, "Remove the kernel being edited.", "删掉正在编辑的内核。")),
    );
}

/// The input preview where a level shows its target.
pub fn draw_preview(ui: &mut Ui, v: &View, at: Vec2, size: Vec2) {
    let pal = ui.pal;
    let l = v.lang;
    let s = v.s;
    if sync::rom_width(s).is_none() {
        return;
    }
    ui.chip(at, size + Vec2::splat(8.0), pal.line.with_alpha(0.5), 6.0);
    ui.chip(at, size + Vec2::splat(6.0), pal.bg_deep, 6.2);
    ui.commands.spawn((
        Sprite {
            image: v.lab.input.clone(),
            custom_size: Some(size),
            ..default()
        },
        Transform::from_xyz(at.x, at.y, 7.0),
        oo_kit::ui::ScreenEntity,
    ));
    ui.caption(
        format!("{} · rom", t(l, "INPUT", "输入")),
        Vec2::new(at.x - size.x / 2.0, at.y + size.y / 2.0 + 10.0),
        pal.text_faint,
        Anchor::CENTER_LEFT,
    );
}

/// The river's caption on the free bench: which kernel is replaying, and
/// what the teaching numbers mean.
pub fn river_caption(s: &Session, l: Lang) -> Option<String> {
    if !s.lab {
        return None;
    }
    let st = &s.lab_state;
    let mut parts = Vec::new();
    if st.kernels.len() > 1 {
        parts.push(format!("{} {} / {}", t(l, "KERNEL", "内核"), st.cur + 1, st.kernels.len()));
    }
    if st.teach {
        let p = s.task().profile;
        parts.push(
            if p.vault {
                t(l, "+N = CYCLES THE SM WAITED BEFORE THIS STEP", "+N = 这一步之前 SM 等了 N 个周期")
            } else if p.latency > 1 || p.units {
                t(l, "−N = READ N BEATS BEFORE THE VALUE LANDED", "−N = 值落地前 N 拍就读了")
            } else {
                t(l, "ONE CYCLE PER STEP · NOTHING WAITS ON THIS MACHINE", "每步一个周期 · 这台机器上没有等待")
            }
            .to_string(),
        );
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// The teaching numbers' entities over the river (moved by `sync_teach`).
pub fn spawn_marks(ui: &mut Ui, s: &Session) {
    if !super::teaching(s) {
        return;
    }
    let pal = ui.pal;
    for i in 0..sync::MARKS {
        let e = ui.chip(Vec2::ZERO, Vec2::new(30.0, 14.0), pal.bg_deep.with_alpha(0.85), 7.8);
        ui.commands.entity(e).insert((sync::TeachBack(i), Visibility::Hidden));
        let e = ui.text("", Vec2::ZERO, 10.0, pal.warn, Anchor::CENTER);
        ui.commands.entity(e).insert((sync::TeachMark(i), Visibility::Hidden));
    }
}

/// Pipeline tab, teaching mode: beats still to go on an in-flight chip.
pub fn beats_left(s: &Session, beat: u32, lands: u32) -> Option<String> {
    super::teaching(s).then(|| (lands - beat).to_string())
}

/// Memory tab, teaching mode: what each colour of the per-step bars
/// costs, in cycles (miss, hit, shared, bank conflict).
pub fn mem_costs(s: &Session, l: Lang) -> Option<[String; 4]> {
    use ooc_core::mem::{GLOBAL_LAT, HIT_LAT};
    let c = t(l, "cycles", "周期");
    super::teaching(s).then(|| {
        [
            format!("{GLOBAL_LAT} {c}"),
            format!("{HIT_LAT} {c}"),
            format!("2 {c}"),
            format!("+1 {}", t(l, "per pass", "每轮")),
        ]
    })
}

/// "Kernel 2 of 3: 140 cycles", for frames of more than one kernel.
pub fn kernel_note(s: &Session, l: Lang) -> Option<String> {
    let st = &s.lab_state;
    let n = st.kernels.len();
    let rep = s.report.as_ref()?;
    (s.lab && n > 1).then(|| {
        format!(
            "{} {} / {}: {} {}",
            t(l, "Kernel", "内核"),
            st.cur + 1,
            n,
            rep.measure.get(ooc_core::task::Metric::Cycles),
            t(l, "cycles", "周期")
        )
    })
}
