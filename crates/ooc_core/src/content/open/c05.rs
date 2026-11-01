//! Chapter 5 open problems: Huajuan's school on Prism, where every result lands
//! a few beats after issue and nothing waits for it.

use oo_base::Tr;

use super::Open;
use crate::content::prism::{BEAT_OPS, base, rom_case};
use crate::content::{below, rng};
use crate::isa::{LANES, WIDE_LEN, WIDE_W};
use crate::task::*;
use crate::vm::{Launch, Profile};

/// Prism with per-unit latencies, its screen 32 pixels wide.
const PRISM: Profile = Profile {
    fb_w: WIDE_W,
    ..Profile::PRISM_UNITS
};

fn lanes_task(id: &'static str, slot: u8) -> Task {
    Task {
        chapter: 5,
        profile: Profile::PRISM_UNITS,
        ..base(id, slot, View::Lanes, BEAT_OPS)
    }
}

fn lanes(f: impl Fn(usize) -> i32) -> Vec<Option<i32>> {
    (0..LANES).map(|l| Some(f(l))).collect()
}

fn wide_blank(_: &Case) -> Vec<i32> {
    blank(WIDE_LEN)
}

/// A full 32×32 expectation from a per-pixel rule.
fn wide(f: impl Fn(i32, i32) -> i32) -> Vec<Option<i32>> {
    (0..WIDE_LEN as i32)
        .map(|t| Some(f(t % WIDE_W, t / WIDE_W)))
        .collect()
}

pub fn problems() -> Vec<Open> {
    vec![ball_toss(), spotlight(), report_card()]
}

/// rom[lane] = t in 0..16; rom[32..36] = a, b, c, d in −9..=9.
fn toss(seed: u64) -> Case {
    let mut r = rng(seed);
    let mut rom: Vec<i32> = (0..LANES).map(|_| below(&mut r, 16)).collect();
    rom.extend((0..4).map(|_| below(&mut r, 19) - 9));
    rom_case(rom)
}

fn cubic(r: &[i32], t: i32) -> i32 {
    ((r[32] * t + r[33]) * t + r[34]) * t + r[35]
}

fn ball_toss() -> Open {
    Open {
        task: Task {
            title: Tr::new("The Ball Toss", "抛球"),
            brief: Tr::new(
                "Huajuan's class fits a curve to a ball toss: h(t) = a·t³ + b·t² + c·t + d, with a, b, c, d at rom[32..36].\nEach lane has a time t = rom[lane]. Write h(t) to out[lane]. MUL lands 4 beats after issue, LD 3, the rest 2.\nScore: cycles.",
                "花卷的班上给一次抛球拟合了一条曲线：h(t) = a·t³ + b·t² + c·t + d，a、b、c、d 在 rom[32..36]。\n每条车道有一个时刻 t = rom[lane]。把 h(t) 写到 out[lane]。MUL 发出 4 拍后落地，LD 3 拍，其余 2 拍。\n得分：周期数。",
            ),
            cases: || [501, 502, 503].map(toss).to_vec(),
            hidden: || [504, 505].map(toss).to_vec(),
            expect: |c| lanes(|l| cubic(&c.rom, c.rom[l])),
            reference: TOSS_SPLIT,
            ..lanes_task("ball-toss", 9)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[17, 23, 24],
    }
}

/// (a·t + b)·t² + (c·t + d): the two halves and t² are independent, so their
/// multiplies overlap.
const TOSS_SPLIT: Source = Source::Code(concat!(
    "LD r0, [lane]\nLD r1, [32]\nLD r2, [33]\nLD r3, [34]\n",
    "MUL r6, r1, r0\nMUL r5, r0, r0\nMUL r7, r3, r0\nLD r4, [35]\n",
    "ADD r6, r6, r2\nNOP\nMUL r6, r6, r5\nADD r7, r7, r4\nNOP\nNOP\n",
    "ADD r6, r6, r7\nNOP\nST [lane], r6\n",
));

/// Horner's rule: one long chain, the loads tucked into its gaps.
#[cfg(test)]
const TOSS_HORNER: Source = Source::Code(concat!(
    "LD r0, [lane]\nLD r1, [32]\nLD r2, [33]\nLD r3, [34]\nMUL r5, r1, r0\nLD r4, [35]\nNOP\nNOP\n",
    "ADD r5, r5, r2\nNOP\nMUL r5, r5, r0\nNOP\nNOP\nNOP\n",
    "ADD r5, r5, r3\nNOP\nMUL r5, r5, r0\nNOP\nNOP\nNOP\n",
    "ADD r5, r5, r4\nNOP\nST [lane], r5\n",
));

/// Horner's rule with every load up front.
#[cfg(test)]
const TOSS_NAIVE: Source = Source::Code(concat!(
    "LD r0, [lane]\nLD r1, [32]\nLD r2, [33]\nLD r3, [34]\nLD r4, [35]\nMUL r5, r1, r0\nNOP\nNOP\nNOP\n",
    "ADD r5, r5, r2\nNOP\nMUL r5, r5, r0\nNOP\nNOP\nNOP\n",
    "ADD r5, r5, r3\nNOP\nMUL r5, r5, r0\nNOP\nNOP\nNOP\n",
    "ADD r5, r5, r4\nNOP\nST [lane], r5\n",
));

/// rom[0], rom[1]: the centre; rom[2]: the radius squared; rom[3] = 12·256 / rom[2].
fn stage(cx: i32, cy: i32, r: i32) -> Case {
    rom_case(vec![cx, cy, r * r, 12 * 256 / (r * r)])
}

fn lit(rom: &[i32], x: i32, y: i32) -> i32 {
    let d = (x - rom[0]).pow(2) + (y - rom[1]).pow(2);
    if d < rom[2] {
        15 - ((d * rom[3]) >> 8)
    } else {
        0
    }
}

fn spotlight() -> Open {
    Open {
        task: Task {
            title: Tr::new("The Spotlight", "聚光灯"),
            brief: Tr::new(
                "The school play needs a spotlight centred on (rom[0], rom[1]). A pixel at squared distance d < rom[2] from the centre gets colour 15 − ((d · rom[3]) SHR 8); the rest stay 0.\nrom[3] is 12·256 / rom[2] in Q8, so the edge fades to about 3. The stage is 32×32.\nScore: cycles. Every instruction, NOP included, runs once on each of the 32 warps.",
                "学校的话剧需要一盏聚光灯，中心在 (rom[0], rom[1])。与中心距离平方 d < rom[2] 的像素颜色为 15 − ((d · rom[3]) SHR 8)，其余保持 0。\nrom[3] 是 Q8 格式的 12·256 / rom[2]，所以边缘暗到 3 左右。舞台是 32×32。\n得分：周期数。每条指令（包括 NOP）都要在 32 个 warp 上各跑一次。",
            ),
            cases: || vec![stage(16, 16, 10), stage(8, 20, 6), stage(25, 5, 13)],
            hidden: || vec![stage(0, 31, 15), stage(19, 12, 3)],
            expect: |c| wide(|x, y| lit(&c.rom, x, y)),
            launch: Launch::WIDE,
            out_len: WIDE_LEN,
            init: wide_blank,
            profile: PRISM,
            chapter: 5,
            reference: SPOT_FOLD,
            ..base("spotlight", 10, View::Screen, BEAT_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[736, 819],
    }
}

/// 15 − (a SHR 8) is (4095 − a) SHR 8, and the compare and the last load sit in
/// the multiplies' gaps.
const SPOT_FOLD: Source = Source::Code(concat!(
    "LD r0, [0]\nLD r1, [1]\nLD r5, [3]\nSUB r3, x, r0\nSUB r4, y, r1\n",
    "MUL r3, r3, r3\nMUL r4, r4, r4\nLD r2, [2]\nNOP\nNOP\nADD r3, r3, r4\nNOP\n",
    "MUL r6, r3, r5\nSETP.LT p0, r3, r2\nNOP\nNOP\n",
    "SUB r6, 4095, r6\nNOP\nSHR r6, r6, 8\nNOP\nSEL r6, p0, r6, 0\nNOP\nPIX r6\n",
));

/// The formula as written, waiting out each result, and an IF around the pixel.
#[cfg(test)]
const SPOT_PLAIN: Source = Source::Code(concat!(
    "LD r0, [0]\nLD r1, [1]\nLD r2, [2]\nLD r5, [3]\nSUB r3, x, r0\nSUB r4, y, r1\nNOP\n",
    "MUL r3, r3, r3\nMUL r4, r4, r4\nNOP\nNOP\nNOP\nADD r3, r3, r4\nNOP\n",
    "MUL r6, r3, r5\nNOP\nNOP\nNOP\nSHR r6, r6, 8\nNOP\nSUB r6, 15, r6\nNOP\n",
    "SETP.LT p0, r3, r2\nIF p0\n  PIX r6\nENDIF\n",
));

/// rom[32k + lane]: score k of student lane, 0 to 100; rom[256..260]: the weights, 0 to 8.
fn marks_of(seed: u64) -> Case {
    let mut r = rng(seed);
    let mut rom: Vec<i32> = (0..4 * LANES).map(|_| below(&mut r, 101)).collect();
    rom.resize(256, 0);
    rom.extend((0..4).map(|_| below(&mut r, 9)));
    rom_case(rom)
}

fn report(r: &[i32], l: usize) -> i32 {
    (0..4).map(|k| r[32 * k + l] * r[256 + k]).sum::<i32>() >> 4
}

fn report_card() -> Open {
    Open {
        task: Task {
            title: Tr::new("Report Cards", "成绩单"),
            brief: Tr::new(
                "Huajuan writes the report cards. Student lane has four scores, rom[lane], rom[32 + lane], rom[64 + lane], rom[96 + lane]; the subject weights w0..w3 are rom[256..260], in sixteenths.\nWrite (score0·w0 + score1·w1 + score2·w2 + score3·w3) SHR 4 to out[lane].\nScore: cycles. MUL lands 4 beats after issue, LD 3, the rest 2.",
                "花卷在写成绩单。lane 号学生有四门分数：rom[lane]、rom[32 + lane]、rom[64 + lane]、rom[96 + lane]；各科权重 w0..w3 在 rom[256..260]，单位是十六分之一。\n把 (分数0·w0 + 分数1·w1 + 分数2·w2 + 分数3·w3) SHR 4 写到 out[lane]。\n得分：周期数。MUL 发出 4 拍后落地，LD 3 拍，其余 2 拍。",
            ),
            cases: || [511, 512, 513].map(marks_of).to_vec(),
            hidden: || [514, 515].map(marks_of).to_vec(),
            expect: |c| lanes(|l| report(&c.rom, l)),
            reference: CARD_WOVEN,
            ..lanes_task("report-card", 11)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[24, 25, 48],
    }
}

/// Each product starts as soon as its two loads land; the next loads fill the gaps.
const CARD_WOVEN: Source = Source::Code(concat!(
    "ADD r1, lane, 32\nADD r2, lane, 64\nADD r3, lane, 96\nLD r0, [lane]\nLD r4, [256]\nLD r1, [r1]\nLD r5, [257]\n",
    "MUL r0, r0, r4\nLD r2, [r2]\nLD r6, [258]\nMUL r1, r1, r5\nLD r3, [r3]\nLD r7, [259]\nMUL r2, r2, r6\n",
    "ADD r0, r0, r1\nMUL r3, r3, r7\nNOP\nADD r0, r0, r2\nNOP\nADD r0, r0, r3\nNOP\nSHR r0, r0, 4\nNOP\nST [lane], r0\n",
));

/// Every load first, then every multiply, then the sum.
#[cfg(test)]
const CARD_PHASES: Source = Source::Code(concat!(
    "ADD r1, lane, 32\nADD r2, lane, 64\nADD r3, lane, 96\n",
    "LD r0, [lane]\nLD r1, [r1]\nLD r2, [r2]\nLD r3, [r3]\nLD r4, [256]\nLD r5, [257]\nLD r6, [258]\nLD r7, [259]\n",
    "MUL r0, r0, r4\nMUL r1, r1, r5\nMUL r2, r2, r6\nMUL r3, r3, r7\nNOP\n",
    "ADD r0, r0, r1\nNOP\nADD r2, r2, r3\nNOP\nADD r0, r0, r2\nNOP\nSHR r0, r0, 4\nNOP\nST [lane], r0\n",
));

/// One subject at a time, waiting out every result.
#[cfg(test)]
const CARD_ONE_BY_ONE: Source = Source::Code(concat!(
    "MOV r0, 0\nMOV r1, lane\nMOV r5, 256\n",
    "LOOP 4\n  LD r2, [r1]\n  LD r3, [r5]\n  ADD r1, r1, 32\n  ADD r5, r5, 1\n  MUL r2, r2, r3\n  NOP\n  NOP\n  NOP\n  ADD r0, r0, r2\nENDLOOP\n",
    "NOP\nSHR r0, r0, 4\nNOP\nST [lane], r0\n",
));

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grade::grade;

    #[test]
    fn alternatives_pass_and_score_on_marks() {
        let alts: [&[Source]; 3] = [
            &[TOSS_HORNER, TOSS_NAIVE],
            &[SPOT_PLAIN],
            &[CARD_PHASES, CARD_ONE_BY_ONE],
        ];
        for (o, alts) in problems().iter().zip(alts) {
            for a in alts {
                let rep = grade(&o.task, &a.solution()).unwrap();
                let s = o.score_of(&rep);
                assert!(
                    s.is_some_and(|s| o.marks.contains(&s)),
                    "{}: {s:?}",
                    o.task.id
                );
            }
        }
    }
}
