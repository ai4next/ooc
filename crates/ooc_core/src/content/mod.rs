//! The Grid Hall's levels. Every task ships a reference solution that the
//! tests hold to three stars, and a starter that must not pass for free.

use std::sync::LazyLock;

use oo_base::{Cat, Rng, Tr};

use crate::isa::{Alu, FB_LEN, FB_W, OpKind};
use crate::task::*;
use crate::vm::Launch;

mod atlas;
mod ch01;
mod ch02;
mod ch03;
mod ch04;
mod ch05;
mod ch06;
mod ch07;
mod ch08;
mod ch09;
mod ch10;
mod ch11;
mod ch12;
mod ch13;
mod ch14;
mod ch15;
mod ch16;
mod lyra;
#[allow(dead_code)]
mod origin;
mod prism;
mod syllabus;
mod vault;
mod warp;

/// In play order: chapter, then slot.
static TASKS: LazyLock<Vec<Task>> = LazyLock::new(|| {
    let mut v = ch01::tasks();
    v.extend(ch02::tasks());
    v.extend(ch03::tasks());
    v.extend(ch04::tasks());
    v.extend(prism::tasks());
    v.extend(ch05::tasks());
    v.extend(ch06::tasks());
    v.extend(ch07::tasks());
    v.extend(ch08::tasks());
    v.extend(ch09::tasks());
    v.extend(ch10::tasks());
    v.extend(ch11::tasks());
    v.extend(ch12::tasks());
    v.extend(ch13::tasks());
    v.extend(ch14::tasks());
    v.extend(ch15::tasks());
    v.extend(ch16::tasks());
    for t in &mut v {
        let &(chapter, slot, _) = syllabus::SYLLABUS
            .iter()
            .find(|row| row.2 == t.id)
            .unwrap_or_else(|| panic!("{} is not in the syllabus", t.id));
        (t.chapter, t.slot) = (chapter, slot);
    }
    v.sort_by_key(|t| (t.chapter, t.slot));
    // Built levels unlock in course order; the gaps are levels not built yet.
    let ids: Vec<&'static str> = v.iter().map(|t| t.id).collect();
    for (t, prev) in v.iter_mut().skip(1).zip(ids) {
        t.requires = Box::leak(Box::new([prev]));
    }
    v
});

const TABLE: [Chapter; CHAPTERS as usize] = [
    Chapter {
        number: 1,
        machine: Tr::new("Lumen · Gen 1", "Lumen · 第一代"),
        title: Tr::new("One Thread per Pixel", "每个像素一个线程"),
        tagline: Tr::new(
            "One circuit, every pixel at once.",
            "同一套电路，同时点亮每个像素。",
        ),
        client: Some(Cat::Juye),
    },
    Chapter {
        number: 2,
        machine: Tr::new("Lumen + cartridge", "Lumen + 卡带"),
        title: Tr::new("Compute, Then Look Up", "算 → 查：寻址"),
        tagline: Tr::new(
            "Sprites, tiles and the race against the beam.",
            "精灵、图块，与扫描线赛跑。",
        ),
        client: Some(Cat::Douya),
    },
    Chapter {
        number: 3,
        machine: Tr::new("Lumen, full load", "Lumen 满载"),
        title: Tr::new("Thirty-Two in Step", "三十二条车道齐步"),
        tagline: Tr::new(
            "Thirty-two lanes, one instruction stream.",
            "三十二条车道，同一条指令流。",
        ),
        client: Some(Cat::Ali),
    },
    Chapter {
        number: 4,
        machine: Tr::new("Lumen, full load", "Lumen 满载"),
        title: Tr::new("Split and Rejoin", "分岔与汇合"),
        tagline: Tr::new(
            "When lanes disagree, the rest wait.",
            "车道意见不一时，其余的得等。",
        ),
        client: Some(Cat::Ali),
    },
    Chapter {
        number: 5,
        machine: Tr::new("Prism · Gen 2", "Prism · 第二代"),
        title: Tr::new("Pipelines and Scheduling", "流水线与指令调度"),
        tagline: Tr::new(
            "Results arrive late; do not read them early.",
            "结果迟到，别提前去读。",
        ),
        client: Some(Cat::Huajuan),
    },
    Chapter {
        number: 6,
        machine: Tr::new("Prism · Gen 2", "Prism · 第二代"),
        title: Tr::new("Three Dimensions and Rasterization", "三维与光栅化"),
        tagline: Tr::new(
            "Triangles, depth, and a pixel asking where it is.",
            "三角形、深度，以及像素问自己在哪。",
        ),
        client: Some(Cat::Huajuan),
    },
    Chapter {
        number: 7,
        machine: Tr::new("Vault · Gen 3", "Vault · 第三代"),
        title: Tr::new("The Memory Wall and Caches", "内存墙与缓存"),
        tagline: Tr::new(
            "Fast arithmetic is useless if the data is late.",
            "算得再快，数据不到也是白搭。",
        ),
        client: Some(Cat::Bohe),
    },
    Chapter {
        number: 8,
        machine: Tr::new("Vault · Gen 3", "Vault · 第三代"),
        title: Tr::new("Coalescing and Shared Memory", "合并访问与共享内存"),
        tagline: Tr::new(
            "Neighbours fetch together; strangers fetch alone.",
            "并肩取一趟，各取各的要走很多趟。",
        ),
        client: Some(Cat::Douya),
    },
    Chapter {
        number: 9,
        machine: Tr::new("Vault · Gen 3", "Vault · 第三代"),
        title: Tr::new(
            "Texture Sampling and Image Processing",
            "纹理采样与图像处理",
        ),
        tagline: Tr::new(
            "Colours that are not in the picture, borrowed from neighbours.",
            "画上没有的颜色，向邻居借。",
        ),
        client: Some(Cat::Bohe),
    },
    Chapter {
        number: 10,
        machine: Tr::new("Atlas · Gen 4", "Atlas · 第四代"),
        title: Tr::new("Grids, Blocks and Stencils", "网格、线程块与模板"),
        tagline: Tr::new(
            "The screen goes dark; arrays and grids remain.",
            "屏幕熄了，剩下数组和网格。",
        ),
        client: Some(Cat::Yinpo),
    },
    Chapter {
        number: 11,
        machine: Tr::new("Atlas · Gen 4", "Atlas · 第四代"),
        title: Tr::new("Parallel Patterns", "并行模式"),
        tagline: Tr::new(
            "A few rounds of asking, and everyone knows their place.",
            "问几轮，每只猫都知道自己排第几。",
        ),
        client: Some(Cat::Yinpo),
    },
    Chapter {
        number: 12,
        machine: Tr::new("Atlas · Gen 4", "Atlas · 第四代"),
        title: Tr::new("Occupancy and Scheduling", "占用率与调度"),
        tagline: Tr::new(
            "Someone is always awake, so the machine never stalls.",
            "总有一只醒着，机器就不会停。",
        ),
        client: Some(Cat::Ali),
    },
    Chapter {
        number: 13,
        machine: Tr::new("Lyra · Gen 5", "Lyra · 第五代"),
        title: Tr::new("Matrix Multiply and Tensor Cores", "矩阵乘与张量核"),
        tagline: Tr::new("Whole matrices per instruction.", "一条指令一整块矩阵。"),
        client: Some(Cat::Cenli),
    },
    Chapter {
        number: 14,
        machine: Tr::new("Lyra · Gen 5", "Lyra · 第五代"),
        title: Tr::new("Precision and Quantization", "精度与量化"),
        tagline: Tr::new("Precision traded for bandwidth.", "用精度换带宽。"),
        client: Some(Cat::Cenli),
    },
    Chapter {
        number: 15,
        machine: Tr::new("The Origin Machine · Gen 6", "起源机 · 第六代"),
        title: Tr::new("Talking Between Chips", "片间通信"),
        tagline: Tr::new("Talking costs more than thinking.", "通信比计算更贵。"),
        client: Some(Cat::Juye),
    },
    Chapter {
        number: 16,
        machine: Tr::new("The Origin Machine · Gen 6", "起源机 · 第六代"),
        title: Tr::new("A Whole Machine Room", "一座机房"),
        tagline: Tr::new("Many machines, one sentence.", "许多台机器，说一句话。"),
        client: None,
    },
];

/// A machine of the course and the chapters that restore it.
#[derive(Clone, Copy, Debug)]
pub struct Machine {
    pub name: Tr,
    pub first: u8,
    pub last: u8,
}

const MACHINES: [Machine; 6] = [
    Machine {
        name: Tr::new("Lumen", "Lumen"),
        first: 1,
        last: 4,
    },
    Machine {
        name: Tr::new("Prism", "Prism"),
        first: 5,
        last: 6,
    },
    Machine {
        name: Tr::new("Vault", "Vault"),
        first: 7,
        last: 9,
    },
    Machine {
        name: Tr::new("Atlas", "Atlas"),
        first: 10,
        last: 12,
    },
    Machine {
        name: Tr::new("Lyra", "Lyra"),
        first: 13,
        last: 14,
    },
    Machine {
        name: Tr::new("The Origin Machine", "起源机"),
        first: 15,
        last: 16,
    },
];

pub fn machines() -> &'static [Machine] {
    &MACHINES
}

/// Index into [`machines`] of the machine that chapter `n` restores.
pub fn machine_of(n: u8) -> usize {
    MACHINES.iter().position(|m| n <= m.last).unwrap_or(5)
}

pub fn chapters() -> &'static [Chapter] {
    &TABLE
}

/// Metadata of chapter `n` (1-based).
pub fn chapter(n: u8) -> &'static Chapter {
    &TABLE[(n.clamp(1, CHAPTERS) - 1) as usize]
}

pub fn tasks() -> &'static [Task] {
    &TASKS
}

pub fn task(id: &str) -> Option<&'static Task> {
    tasks().iter().find(|t| t.id == id)
}

/// The implemented tasks of chapter `n`, in slot order.
pub fn in_chapter(n: u8) -> impl Iterator<Item = &'static Task> {
    tasks().iter().filter(move |t| t.chapter == n)
}

/// Deterministic test data: the same seed always yields the same level.
pub(crate) fn rng(seed: u64) -> Rng {
    Rng::new(seed)
}

pub(crate) fn below(r: &mut Rng, n: i32) -> i32 {
    r.below(n as u64) as i32
}

/// A small picture: colour depends smoothly on position, with some noise,
/// so mistakes in indexing show up as visible tearing.
pub(crate) fn picture(seed: u64, w: i32, h: i32) -> Vec<i32> {
    let mut r = rng(seed);
    let a = below(&mut r, 4) + 1;
    let b = below(&mut r, 4) + 1;
    (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let v = (x * a + y * b) / 3 + below(&mut r, 2);
            1 + v.rem_euclid(15)
        })
        .collect()
}

pub(crate) fn frames(fs: &[i32]) -> Vec<Case> {
    fs.iter().map(|&f| Case::frame(f)).collect()
}

/// A full-screen expectation from a per-pixel rule.
pub(crate) fn screen(f: impl Fn(i32, i32) -> i32) -> Vec<Option<i32>> {
    (0..FB_LEN as i32)
        .map(|t| Some(f(t % FB_W, t / FB_W)))
        .collect()
}

const fn alu(a: Alu) -> OpKind {
    OpKind::Alu(a)
}

/// Everything except the warp-level operations.
pub(crate) const SCALAR_OPS: &[OpKind] = &[
    OpKind::Mov,
    alu(Alu::Add),
    alu(Alu::Sub),
    alu(Alu::Mul),
    alu(Alu::And),
    alu(Alu::Or),
    alu(Alu::Xor),
    alu(Alu::Shl),
    alu(Alu::Shr),
    alu(Alu::Min),
    alu(Alu::Max),
    OpKind::SetP,
    OpKind::Sel,
    OpKind::If,
    OpKind::Else,
    OpKind::EndIf,
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::Ld,
    OpKind::St,
    OpKind::Pix,
];

pub(crate) const WARP_OPS: &[OpKind] = &[
    OpKind::Mov,
    alu(Alu::Add),
    alu(Alu::Sub),
    alu(Alu::Mul),
    alu(Alu::And),
    alu(Alu::Or),
    alu(Alu::Xor),
    alu(Alu::Shl),
    alu(Alu::Shr),
    alu(Alu::Min),
    alu(Alu::Max),
    OpKind::SetP,
    OpKind::Sel,
    OpKind::If,
    OpKind::Else,
    OpKind::EndIf,
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::Ld,
    OpKind::St,
    OpKind::Pix,
    OpKind::Shfl,
    OpKind::Vote,
];

const EMPTY: Tr = Tr::new("", "");

/// A task skeleton; content files fill in the text and data with struct update.
pub(crate) fn stub(
    id: &'static str,
    chapter: u8,
    slot: u8,
    view: View,
    editor: Editor,
    launch: Launch,
    out_len: usize,
) -> Task {
    Task {
        id,
        chapter,
        slot,
        view,
        title: EMPTY,
        brief: EMPTY,
        editor,
        launch,
        profile: crate::vm::Profile::LUMEN,
        out_len,
        cases: || frames(&[0]),
        hidden: Vec::new,
        init: screen_blank,
        expect: |_| Vec::new(),
        tolerance: 0,
        limit: None,
        goals: [Goal::new(Metric::Cycles, 0), Goal::new(Metric::Size, 0)],
        starter: Source::Code(""),
        reference: Source::Code(""),
        live: None,
        steer: None,
        predict: Predict {
            question: EMPTY,
            options: [EMPTY; 3],
            answer: 0,
            why: EMPTY,
        },
        hints: [EMPTY; 3],
        insight: Insight {
            title: EMPTY,
            line: EMPTY,
            reality: EMPTY,
            myth: EMPTY,
            simplified: EMPTY,
        },
        clue: None,
        requires: &[],
        free_warps: false,
        grid: None,
        free_block: false,
        reg_cap: None,
        flips: None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::grade::{Verdict, grade};
    use crate::task::{Editor, Source};

    #[test]
    fn ids_are_unique_and_requirements_exist() {
        let mut seen = HashSet::new();
        for t in tasks() {
            assert!(seen.insert(t.id), "duplicate {}", t.id);
            for r in t.requires {
                assert!(seen.contains(r), "{} requires later/unknown {r}", t.id);
            }
        }
        assert_eq!(tasks().len(), 128);
    }

    #[test]
    fn machines_tile_the_chapters() {
        let mut next = 1;
        for m in machines() {
            assert_eq!(m.first, next);
            assert!(m.name.is_complete());
            next = m.last + 1;
        }
        assert_eq!(next, CHAPTERS + 1);
        assert_eq!(machine_of(1), 0);
        assert_eq!(machine_of(16), 5);
    }

    #[test]
    fn chapters_follow_the_slot_skeleton() {
        use oo_base::LevelKind;
        for (i, c) in chapters().iter().enumerate() {
            assert_eq!(c.number as usize, i + 1);
            for tr in [c.machine, c.title, c.tagline, c.client_name()] {
                assert!(tr.is_complete(), "chapter {}: {tr:?}", c.number);
            }
        }
        let mut seen = HashSet::new();
        for t in tasks() {
            assert!((1..=CHAPTERS).contains(&t.chapter), "{}", t.id);
            assert!((1..=LEVELS_PER_CHAPTER).contains(&t.slot), "{}", t.id);
            assert!(
                seen.insert((t.chapter, t.slot)),
                "{} shares slot {}",
                t.id,
                t.number()
            );
            if t.kind() == LevelKind::Commission {
                assert!(t.live.is_some(), "{}: commissions end in a showcase", t.id);
            }
        }
    }

    #[test]
    fn faults_carry_evidence_in_the_starter_run() {
        use oo_base::LevelKind;
        for t in tasks() {
            assert_eq!(
                t.clue.is_some(),
                t.kind() == LevelKind::Fault,
                "{}: only faults point at evidence",
                t.id
            );
            let Some(clue) = t.clue else { continue };
            assert!(clue.what.is_complete(), "{}", t.id);
            let rep = grade(t, &t.starter.solution()).unwrap();
            let run = rep.runs.iter().find(|r| r.fails(t)).expect("starter fails");
            let ev = (clue.find)(&rep.program, run).unwrap_or_else(|| panic!("{}", t.id));
            let step = &run.run.trace[ev.from as usize];
            assert!(ev.accepts(ev.from, Some(step.warp as u32)), "{}", t.id);
        }
    }

    #[test]
    fn every_reference_earns_three_stars() {
        for t in tasks() {
            let rep =
                grade(t, &t.reference.solution()).unwrap_or_else(|e| panic!("{}: {e:?}", t.id));
            assert_eq!(rep.verdict, Verdict::Pass, "{}", t.id);
            assert_eq!(
                rep.stars(),
                3,
                "{}: {:?} vs {:?}",
                t.id,
                rep.measure,
                t.goals
            );
        }
    }

    #[test]
    fn no_starter_passes() {
        for t in tasks() {
            let rep = grade(t, &t.starter.solution()).unwrap_or_else(|e| panic!("{}: {e:?}", t.id));
            assert!(!rep.passed(), "{} starter passes", t.id);
        }
    }

    #[test]
    fn editors_match_sources() {
        for t in tasks() {
            for s in [t.starter, t.reference] {
                let ok = matches!(
                    (t.editor, s),
                    (Editor::Code(_), Source::Code(_)) | (Editor::Graph(_), Source::Graph(_))
                );
                assert!(ok, "{}", t.id);
            }
        }
    }

    #[test]
    fn text_is_bilingual_and_predictions_valid() {
        for t in tasks() {
            let mut all = vec![t.title, t.brief, t.predict.question, t.predict.why];
            all.extend(t.predict.options);
            all.extend(t.hints);
            let i = t.insight;
            all.extend([i.title, i.line, i.reality, i.myth, i.simplified]);
            for tr in all {
                assert!(tr.is_complete(), "{}: {tr:?}", t.id);
            }
            assert!(t.predict.answer < 3, "{}", t.id);
        }
    }

    #[test]
    fn varying_tasks_hide_fresh_cases() {
        for t in tasks() {
            let (public, hidden) = ((t.cases)(), (t.hidden)());
            if public.len() > 1 {
                assert!(!hidden.is_empty(), "{}: no hidden cases", t.id);
            }
            for h in &hidden {
                assert!(!public.contains(h), "{}: a hidden case is public", t.id);
            }
        }
    }

    #[test]
    fn expectations_fit_buffers() {
        for t in tasks() {
            for c in (t.cases)().into_iter().chain((t.hidden)()) {
                assert_eq!((t.expect)(&c).len(), t.out_len, "{}", t.id);
                assert!((t.init)(&c).len() <= t.out_len, "{}", t.id);
            }
        }
    }
}
