//! Chapter 6 open problems: Huajuan's cardboard model on Prism, now with DOT,
//! RCP and the raster unit's EDGE.

use oo_base::Tr;

use super::Open;
use crate::content::prism::{base, edge, rom_case};
use crate::content::{below, rng};
use crate::isa::{Alu, LANES, OpKind, WIDE_LEN, WIDE_W};
use crate::task::*;
use crate::vm::{Launch, Profile};

const fn alu(a: Alu) -> OpKind {
    OpKind::Alu(a)
}

/// Chapter 6's scalar set: Prism's beat set without XOR.
macro_rules! prism_ops {
    ($($extra:expr),*) => {
        &[
            OpKind::Mov,
            alu(Alu::Add),
            alu(Alu::Sub),
            alu(Alu::Mul),
            alu(Alu::And),
            alu(Alu::Or),
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
            OpKind::Nop,
            $($extra),*
        ]
    };
}

/// Everything chapter 6 has unlocked.
const ALL_OPS: &[OpKind] = prism_ops!(OpKind::Edge, OpKind::Dot, OpKind::Rcp, OpKind::Lerp);

/// Prism with per-unit latencies, its screen 32 pixels wide.
const PRISM: Profile = Profile {
    fb_w: WIDE_W,
    ..Profile::PRISM_UNITS
};

fn lanes_task(id: &'static str, slot: u8) -> Task {
    Task {
        chapter: 6,
        profile: Profile::PRISM_UNITS,
        ..base(id, slot, View::Lanes, ALL_OPS)
    }
}

fn lanes(f: impl Fn(usize) -> i32) -> Vec<Option<i32>> {
    (0..LANES).map(|l| Some(f(l))).collect()
}

fn wide_blank(_: &Case) -> Vec<i32> {
    blank(WIDE_LEN)
}

pub fn problems() -> Vec<Open> {
    vec![two_lamps(), the_window(), sweets()]
}

/// rom[lane], rom[32 + lane]: the normal; rom[64..68]: lamps A and B. All in −16..=16.
fn lamps(seed: u64) -> Case {
    let mut r = rng(seed);
    rom_case((0..68).map(|_| below(&mut r, 33) - 16).collect())
}

fn lit_by_two(r: &[i32], l: usize) -> i32 {
    let (nx, ny) = (r[l], r[32 + l]);
    let a = (nx * r[64] + ny * r[65]).max(0);
    let b = (nx * r[66] + ny * r[67]).max(0);
    ((a + b) >> 5).min(15)
}

fn two_lamps() -> Open {
    Open {
        task: Task {
            title: Tr::new("Two Lamps", "两盏灯"),
            brief: Tr::new(
                "Two lamps light Huajuan's model. Face lane has the normal (rom[lane], rom[32 + lane]); lamp A points along (rom[64], rom[65]), lamp B along (rom[66], rom[67]).\nEach lamp adds MAX(n·L, 0). Write MIN(15, (A + B) SHR 5) to out[lane]. MUL and DOT land 4 beats after issue, LD 3, the rest 2.\nScore: cycles.",
                "两盏灯照着花卷的模型。lane 号面的法线是 (rom[lane], rom[32 + lane])；灯 A 的方向是 (rom[64], rom[65])，灯 B 是 (rom[66], rom[67])。\n每盏灯贡献 MAX(n·L, 0)。把 MIN(15, (A + B) SHR 5) 写到 out[lane]。MUL 和 DOT 发出 4 拍后落地，LD 3 拍，其余 2 拍。\n得分：周期数。",
            ),
            cases: || [621, 622, 623].map(lamps).to_vec(),
            hidden: || [624, 625].map(lamps).to_vec(),
            expect: |c| lanes(|l| lit_by_two(&c.rom, l)),
            reference: LAMPS_DOT,
            ..lanes_task("two-lamps", 9)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[22, 26, 29],
    }
}

/// The normal sits in r0, r1 and each lamp in the next pair up, so two DOTs
/// cover all four products; the loads fill the beats in between.
const LAMPS_DOT: Source = Source::Code(concat!(
    "ADD r1, lane, 32\nLD r0, [lane]\nLD r1, [r1]\nLD r2, [64]\nLD r3, [65]\nLD r4, [66]\nLD r5, [67]\n",
    "DOT r6, r0, r2\nNOP\nDOT r7, r0, r4\nNOP\nMAX r6, r6, 0\nNOP\nMAX r7, r7, 0\nNOP\n",
    "ADD r6, r6, r7\nNOP\nSHR r6, r6, 5\nNOP\nMIN r6, r6, 15\nNOP\nST [lane], r6\n",
));

/// Four MULs and two ADDs in place of the DOTs.
#[cfg(test)]
const LAMPS_MUL: Source = Source::Code(concat!(
    "ADD r1, lane, 32\nLD r0, [lane]\nLD r1, [r1]\nLD r2, [64]\nLD r3, [65]\nLD r4, [66]\nLD r5, [67]\n",
    "MUL r6, r0, r2\nMUL r8, r1, r3\nMUL r7, r0, r4\nMUL r9, r1, r5\nNOP\nADD r6, r6, r8\nNOP\nADD r7, r7, r9\n",
    "MAX r6, r6, 0\nNOP\nMAX r7, r7, 0\nNOP\nADD r6, r6, r7\nNOP\nSHR r6, r6, 5\nNOP\nMIN r6, r6, 15\nNOP\nST [lane], r6\n",
));

/// One lamp after the other, each result waited out.
#[cfg(test)]
const LAMPS_WAIT: Source = Source::Code(concat!(
    "ADD r1, lane, 32\nLD r0, [lane]\nLD r1, [r1]\nLD r2, [64]\nLD r3, [65]\nNOP\nNOP\n",
    "DOT r6, r0, r2\nNOP\nNOP\nNOP\nMAX r6, r6, 0\nLD r2, [66]\nLD r3, [67]\nNOP\nNOP\n",
    "DOT r7, r0, r2\nNOP\nNOP\nNOP\nMAX r7, r7, 0\nNOP\n",
    "ADD r6, r6, r7\nNOP\nSHR r6, r6, 5\nNOP\nMIN r6, r6, 15\nNOP\nST [lane], r6\n",
));

/// The edges of a convex polygon in half-pixel units, `e = a·x + b·y + c` at
/// pixel centres, all flipped so that the inside is positive.
fn poly_edges(v: &[(i32, i32)]) -> Vec<i32> {
    let n = v.len();
    let (sx, sy) = v.iter().fold((0, 0), |(x, y), p| (x + p.0, y + p.1));
    let mut out = Vec::new();
    for i in 0..n {
        let (p, q) = (v[i], v[(i + 1) % n]);
        let (a, b, c) = (p.1 - q.1, q.0 - p.0, p.0 * q.1 - q.0 * p.1);
        let flip = if a * sx + b * sy + c * n as i32 > 0 {
            1
        } else {
            -1
        };
        out.extend([2 * a * flip, 2 * b * flip, (a + b + c) * flip]);
    }
    out
}

/// rom[0..12]: the wall's four edges; rom[12..21]: the window's three; rom[21]: the colour.
fn wall(quad: [(i32, i32); 4], hole: [(i32, i32); 3], colour: i32) -> Case {
    let mut rom = poly_edges(&quad);
    rom.extend(poly_edges(&hole));
    rom.push(colour);
    rom_case(rom)
}

fn through(r: &[i32], x: i32, y: i32) -> i32 {
    let on_wall = (0..4).all(|i| edge(r, 3 * i, x, y) > 0);
    let in_hole = (4..7).all(|i| edge(r, 3 * i, x, y) > 0);
    if on_wall && !in_hole { r[21] } else { 0 }
}

/// A full 32×32 expectation from a per-pixel rule.
fn wide(r: &[i32], f: impl Fn(&[i32], i32, i32) -> i32) -> Vec<Option<i32>> {
    (0..WIDE_LEN as i32)
        .map(|t| Some(f(r, t % WIDE_W, t / WIDE_W)))
        .collect()
}

fn the_window() -> Open {
    Open {
        task: Task {
            title: Tr::new("The Window", "窗洞"),
            brief: Tr::new(
                "Huajuan cuts a window into a cardboard wall. The wall is a four-sided shape with edges rom[0..12]; the window is a triangle with edges rom[12..21]. Every edge is positive inside.\nPaint colour rom[21] where all four wall edges are positive and the three window edges are not all positive; the rest stay 0. The screen is 32×32.\nScore: cycles. Every instruction, NOP included, runs once on each of the 32 warps.",
                "花卷在一面纸板墙上剪了个窗洞。墙是一个四边形，边在 rom[0..12]；窗是一个三角形，边在 rom[12..21]。每条边都是内部为正。\n在墙的四条边都为正、而窗的三条边不全为正的地方涂上颜色 rom[21]；其余保持 0。屏幕是 32×32。\n得分：周期数。每条指令（包括 NOP）都要在 32 个 warp 上各跑一次。",
            ),
            cases: || {
                vec![
                    wall(
                        [(4, 6), (58, 2), (62, 56), (8, 60)],
                        [(20, 18), (46, 24), (28, 46)],
                        9,
                    ),
                    wall(
                        [(32, 2), (62, 32), (32, 62), (2, 32)],
                        [(24, 24), (40, 24), (32, 40)],
                        14,
                    ),
                    wall(
                        [(2, 2), (40, 4), (60, 50), (6, 40)],
                        [(10, 8), (36, 10), (14, 34)],
                        5,
                    ),
                ]
            },
            hidden: || {
                vec![
                    wall(
                        [(0, 0), (64, 0), (64, 64), (0, 64)],
                        [(4, 4), (60, 6), (30, 60)],
                        12,
                    ),
                    wall(
                        [(10, 4), (54, 12), (50, 58), (14, 50)],
                        [(0, 30), (64, 20), (40, 64)],
                        7,
                    ),
                ]
            },
            expect: |c| wide(&c.rom, through),
            launch: Launch::WIDE,
            out_len: WIDE_LEN,
            init: wide_blank,
            profile: PRISM,
            chapter: 6,
            reference: WINDOW_KEY,
            ..base("the-window", 10, View::Screen, ALL_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[672, 768],
    }
}

/// One key for both tests: MIN(wall, 1 − window) is positive only on the wall
/// outside the window. The window's edges go first so its MINs overlap the wall's.
const WINDOW_KEY: Source = Source::Code(concat!(
    "EDGE r4, [12]\nEDGE r5, [15]\nEDGE r6, [18]\nEDGE r0, [0]\nEDGE r1, [3]\nMIN r4, r4, r5\nEDGE r2, [6]\n",
    "MIN r4, r4, r6\nEDGE r3, [9]\nSUB r4, 1, r4\nMIN r0, r0, r1\nLD r7, [21]\nMIN r3, r3, r4\nMIN r0, r0, r2\nNOP\n",
    "MIN r0, r0, r3\nNOP\nSETP.GT p0, r0, 0\nSEL r7, p0, r7, 0\nNOP\nPIX r7\n",
));

/// Two predicates and two SELs, each result waited out.
#[cfg(test)]
const WINDOW_TWO: Source = Source::Code(concat!(
    "EDGE r0, [0]\nEDGE r1, [3]\nEDGE r2, [6]\nEDGE r3, [9]\nEDGE r4, [12]\nEDGE r5, [15]\nEDGE r6, [18]\nLD r7, [21]\n",
    "MIN r0, r0, r1\nNOP\nMIN r0, r0, r2\nNOP\nMIN r0, r0, r3\nMIN r4, r4, r5\nNOP\nMIN r4, r4, r6\nNOP\n",
    "SETP.GT p0, r0, 0\nSETP.GT p1, r4, 0\nSEL r7, p0, r7, 0\nNOP\nSEL r7, p1, 0, r7\nNOP\nPIX r7\n",
));

/// The wall and then the window as nested IF blocks.
#[cfg(test)]
const WINDOW_IF: Source = Source::Code(concat!(
    "EDGE r0, [0]\nEDGE r1, [3]\nEDGE r2, [6]\nEDGE r3, [9]\nEDGE r4, [12]\nEDGE r5, [15]\nEDGE r6, [18]\nLD r7, [21]\n",
    "MIN r0, r0, r1\nNOP\nMIN r0, r0, r2\nNOP\nMIN r0, r0, r3\nMIN r4, r4, r5\nNOP\nMIN r4, r4, r6\nNOP\n",
    "SETP.GT p0, r0, 0\nSETP.LE p1, r4, 0\nIF p0\n  IF p1\n    PIX r7\n  ENDIF\nENDIF\n",
));

/// rom[lane]: sweets, 0 to 9 999; rom[32 + lane]: children, 1 to 99.
fn bags(seed: u64) -> Case {
    let mut r = rng(seed);
    let mut rom: Vec<i32> = (0..LANES).map(|_| below(&mut r, 10_000)).collect();
    rom.extend((0..LANES).map(|_| 1 + below(&mut r, 99)));
    rom_case(rom)
}

/// Exact multiples, where a truncated reciprocal falls one short, and the extremes.
fn even_bags() -> Case {
    let mut rom: Vec<i32> = (0..LANES as i32)
        .map(|l| (99 - 3 * l) * (101 - 3 * l))
        .collect();
    rom.extend((0..LANES as i32).map(|l| 99 - 3 * l));
    rom[0] = 9_999;
    rom[32] = 1;
    rom[1] = 0;
    rom[2] = 9_999;
    rom[34] = 99;
    rom_case(rom)
}

fn sweets() -> Open {
    Open {
        task: Task {
            title: Tr::new("Sharing the Sweets", "分糖"),
            brief: Tr::new(
                "Each lane is a bag of rom[lane] sweets (0 to 9 999) shared among rom[32 + lane] children (1 to 99). Write each child's share, rom[lane] / rom[32 + lane] rounded down, to out[lane]. It must be exact.\nRCP gives 65536 / x rounded down. RCP and MUL land 4 beats after issue, LD 3, the rest 2.\nScore: cycles.",
                "每条车道是一袋 rom[lane] 颗糖（0 到 9 999），分给 rom[32 + lane] 个孩子（1 到 99）。把每个孩子分到的数量 rom[lane] / rom[32 + lane]（向下取整）写到 out[lane]，必须精确。\nRCP 给出向下取整的 65536 / x。RCP 和 MUL 发出 4 拍后落地，LD 3 拍，其余 2 拍。\n得分：周期数。",
            ),
            cases: || [631, 632, 633].map(bags).to_vec(),
            hidden: || vec![even_bags(), bags(634)],
            expect: |c| lanes(|l| c.rom[l] / c.rom[32 + l]),
            reference: SHARE_FIX,
            ..lanes_task("sweets", 11)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[23, 27, 149],
    }
}

/// q = (a · RCP(b)) SHR 16 is exact or one short. It is short exactly when
/// q·b ≤ a − b, and a − b is ready long before q·b is.
const SHARE_FIX: Source = Source::Code(concat!(
    "ADD r1, lane, 32\nLD r0, [lane]\nLD r1, [r1]\nNOP\nNOP\n",
    "RCP r2, r1\nSUB r5, r0, r1\nNOP\nNOP\n",
    "MUL r2, r0, r2\nNOP\nNOP\nNOP\nSHR r2, r2, 16\nNOP\n",
    "MUL r4, r2, r1\nADD r3, r2, 1\nNOP\nNOP\n",
    "SETP.LE p0, r4, r5\nSEL r2, p0, r3, r2\nNOP\nST [lane], r2\n",
));

/// The same estimate, checked by the remainder under IF.
#[cfg(test)]
const SHARE_IF: Source = Source::Code(concat!(
    "ADD r1, lane, 32\nLD r0, [lane]\nLD r1, [r1]\nNOP\nNOP\n",
    "RCP r2, r1\nNOP\nNOP\nNOP\nMUL r2, r0, r2\nNOP\nNOP\nNOP\nSHR r2, r2, 16\nNOP\n",
    "MUL r4, r2, r1\nNOP\nNOP\nNOP\nSUB r4, r0, r4\nNOP\n",
    "SETP.GE p0, r4, r1\nIF p0\n  ADD r2, r2, 1\n  NOP\nENDIF\nST [lane], r2\n",
));

/// Long division, one bit of the share per round, from 2^13 down.
#[cfg(test)]
const SHARE_LONG: Source = Source::Code(concat!(
    "ADD r1, lane, 32\nLD r0, [lane]\nLD r1, [r1]\nMOV r2, 0\nMOV r4, 13\nNOP\n",
    "LOOP 14\n  SHL r6, r1, r4\n  SHL r7, 1, r4\n  SUB r4, r4, 1\n  SETP.GE p0, r0, r6\n",
    "  SUB r6, r0, r6\n  ADD r7, r2, r7\n  NOP\n  SEL r0, p0, r6, r0\n  SEL r2, p0, r7, r2\nENDLOOP\n",
    "NOP\nST [lane], r2\n",
));

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grade::grade;

    #[test]
    fn alternatives_pass_and_score_on_marks() {
        let alts: [&[Source]; 3] = [
            &[LAMPS_MUL, LAMPS_WAIT],
            &[WINDOW_TWO, WINDOW_IF],
            &[SHARE_IF, SHARE_LONG],
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
