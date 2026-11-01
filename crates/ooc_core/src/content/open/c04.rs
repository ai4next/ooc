//! Chapter 4 open problems: Ali's caravan again, now with IF, ELSE and ENDIF.

use oo_base::Tr;

use super::Open;
use crate::content::WARP_OPS;
use crate::content::warp::{base, lanes, rom_cases, values};
use crate::isa::{Alu, LANES, OpKind};
use crate::task::*;

const fn alu(a: Alu) -> OpKind {
    OpKind::Alu(a)
}

/// Chapter 4's set with the branch unit taken away.
const NO_IF_OPS: &[OpKind] = &[
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
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::Ld,
    OpKind::St,
    OpKind::Pix,
    OpKind::Shfl,
    OpKind::Vote,
];

pub fn problems() -> Vec<Open> {
    vec![no_fork(), wind_vane(), price_search()]
}

fn collatz(n: i32) -> i32 {
    if n & 1 == 1 { 3 * n + 1 } else { n / 2 }
}

fn no_fork() -> Open {
    Open {
        task: Task {
            title: Tr::new("One Step, No Fork", "不分岔的一步"),
            brief: Tr::new(
                "The Collatz job again, but the branch unit is out for repair: IF is not available.\nFor each n = rom[lane] (1 to 99 999), write one step to out[lane]: odd → 3n + 1, even → n / 2.\nScore: cycles.",
                "又是考拉兹任务，可分支单元送修了：IF 用不了。\n对每个 n = rom[lane]（1 到 99 999），把走一步的结果写到 out[lane]：奇数 → 3n + 1，偶数 → n / 2。\n得分：周期数。",
            ),
            cases: || rom_cases(|s| values(s, 32, 1, 100_000), &[401, 402]),
            hidden: || {
                let mut v = rom_cases(|s| values(s, 32, 1, 100_000), &[403]);
                v.push(Case {
                    rom: (0..32).map(|i| 2 * i + 1).collect(),
                    frame: 0,
                });
                v
            },
            expect: |c| lanes(|l| collatz(c.rom[l])),
            reference: STEP_SEL,
            ..base(4, "no-fork", 9, NO_IF_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[8, 9, 10],
    }
}

/// Both answers, then SEL keeps one.
const STEP_SEL: Source = Source::Code(
    "LD r0, [lane]\nAND r1, r0, 1\nSETP.NE p0, r1, 0\nMUL r2, r0, 3\nADD r2, r2, 1\nSHR r0, r0, 1\nSEL r0, p0, r2, r0\nST [lane], r0\n",
);

/// With b = n AND 1: (n + b·(2n + 1)) SHR (1 − b).
#[cfg(test)]
const STEP_ARITH: Source = Source::Code(
    "LD r0, [lane]\nAND r1, r0, 1\nSHL r2, r0, 1\nOR r2, r2, 1\nMUL r2, r2, r1\nADD r0, r0, r2\nXOR r1, r1, 1\nSHR r0, r0, r1\nST [lane], r0\n",
);

/// The odd part masked in with AND, the shift as above.
#[cfg(test)]
const STEP_MASK: Source = Source::Code(
    "LD r0, [lane]\nAND r1, r0, 1\nSUB r3, 0, r1\nSHL r2, r0, 1\nOR r2, r2, 1\nAND r2, r2, r3\nADD r0, r0, r2\nXOR r1, r1, 1\nSHR r0, r0, r1\nST [lane], r0\n",
);

/// The sector, 0 to 7, of the angle of (dx, dy) measured from east towards +y.
fn sector(dx: i32, dy: i32) -> i32 {
    let (mut dx, mut dy, mut k) = (dx, dy, 0);
    if dy < 0 || (dy == 0 && dx < 0) {
        (dx, dy, k) = (-dx, -dy, 4);
    }
    if dx <= 0 {
        (dx, dy, k) = (dy, -dx, k + 2);
    }
    k + i32::from(dy >= dx)
}

/// rom[lane] = dx, rom[32 + lane] = dy, in −100..=100 and never both 0.
fn winds(seed: u64) -> Vec<i32> {
    let mut v = values(seed, 64, -100, 101);
    for l in 0..LANES {
        if v[l] == 0 && v[32 + l] == 0 {
            v[l] = 7;
        }
    }
    v
}

/// The eight compass points and the eight diagonals, at four lengths.
fn borders() -> Vec<i32> {
    const D: [(i32, i32); 8] = [
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
        (0, -1),
        (1, -1),
    ];
    let v: Vec<(i32, i32)> = (0..32)
        .map(|i| {
            (
                D[i % 8].0 * (1 + i as i32 / 8 * 33),
                D[i % 8].1 * (1 + i as i32 / 8 * 33),
            )
        })
        .collect();
    v.iter().map(|p| p.0).chain(v.iter().map(|p| p.1)).collect()
}

fn wind_vane() -> Open {
    Open {
        task: Task {
            title: Tr::new("The Wind Vane", "风向标"),
            brief: Tr::new(
                "Each of Ali's 32 vanes reads a wind (dx, dy) = (rom[lane], rom[32 + lane]), each −100 to 100, never both 0.\nWrite its sector to out[lane]: sector k holds the angles from 45k° up to, not including, 45(k + 1)°, measured from +x towards +y.\nScore: cycles.",
                "阿砾的 32 个风向标各读到一阵风 (dx, dy) = (rom[lane], rom[32 + lane])，各在 −100 到 100 之间，不会同时为 0。\n把它的扇区写到 out[lane]：扇区 k 包含从 45k° 起、不到 45(k + 1)° 的角度，从 +x 转向 +y 量。\n得分：周期数。",
            ),
            cases: || rom_cases(winds, &[411, 412]),
            hidden: || {
                let mut v = rom_cases(winds, &[413]);
                v.push(Case {
                    rom: borders(),
                    frame: 0,
                });
                v
            },
            expect: |c| lanes(|l| sector(c.rom[l], c.rom[32 + l])),
            reference: VANE_SEL,
            ..base(4, "wind-vane", 10, WARP_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[21, 24, 28],
    }
}

/// Half turn, quarter turn, then the diagonal, each picked with SEL.
/// dy·256 + dx is negative exactly when the half turn is needed.
const VANE_SEL: Source = Source::Code(concat!(
    "LD r0, [lane]\nADD r1, lane, 32\nLD r1, [r1]\n",
    "SHL r2, r1, 8\nADD r2, r2, r0\nSETP.LT p0, r2, 0\n",
    "SUB r3, 0, r0\nSEL r0, p0, r3, r0\nSUB r3, 0, r1\nSEL r1, p0, r3, r1\nSEL r5, p0, 4, 0\n",
    "SETP.LE p1, r0, 0\nSEL r4, p1, r1, r0\nSUB r3, 0, r0\nSEL r1, p1, r3, r1\nADD r6, r5, 2\nSEL r5, p1, r6, r5\n",
    "SETP.GE p2, r1, r4\nSEL r6, p2, 1, 0\nADD r5, r5, r6\nST [lane], r5\n",
));

/// The same three steps as IF blocks.
#[cfg(test)]
const VANE_IF: Source = Source::Code(concat!(
    "LD r0, [lane]\nADD r1, lane, 32\nLD r1, [r1]\nMOV r5, 0\n",
    "SHL r2, r1, 8\nADD r2, r2, r0\nSETP.LT p0, r2, 0\n",
    "IF p0\n  SUB r0, 0, r0\n  SUB r1, 0, r1\n  MOV r5, 4\nENDIF\n",
    "SETP.LE p1, r0, 0\nIF p1\n  MOV r4, r0\n  MOV r0, r1\n  SUB r1, 0, r4\n  ADD r5, r5, 2\nENDIF\n",
    "SETP.GE p2, r1, r0\nIF p2\n  ADD r5, r5, 1\nENDIF\nST [lane], r5\n",
));

/// Eight sector tests, one per answer, nested as IF/ELSE by quadrant.
#[cfg(test)]
const VANE_TREE: Source = Source::Code(concat!(
    "LD r0, [lane]\nADD r1, lane, 32\nLD r1, [r1]\n",
    "SUB r2, 0, r0\nSUB r3, 0, r1\n",
    "SETP.GE p0, r1, 0\nIF p0\n",
    "  SETP.GT p1, r0, 0\n  IF p1\n    SETP.GE p2, r1, r0\n    SEL r5, p2, 1, 0\n",
    "  ELSE\n    SETP.GE p2, r2, r1\n    SEL r5, p2, 3, 2\n    SETP.EQ p3, r1, 0\n    SEL r5, p3, 4, r5\n  ENDIF\n",
    "ELSE\n",
    "  SETP.LT p1, r0, 0\n  IF p1\n    SETP.GE p2, r3, r2\n    SEL r5, p2, 5, 4\n",
    "  ELSE\n    SETP.GE p2, r0, r3\n    SEL r5, p2, 7, 6\n  ENDIF\n",
    "ENDIF\nST [lane], r5\n",
));

/// rom[0..64]: the sorted price list, ending in 10 000; rom[64..96]: one offer per lane.
fn market(seed: u64) -> Vec<i32> {
    let mut prices = values(seed, 63, 0, 10_000);
    prices.sort();
    prices.push(10_000);
    prices.extend(values(seed + 50, 32, 0, 10_000));
    prices
}

/// Offers equal to listed prices, and the two ends of the list.
fn exact(seed: u64) -> Vec<i32> {
    let mut rom = market(seed);
    for l in 0..LANES {
        rom[64 + l] = rom[(l * 2 + 1) % 64];
    }
    rom[64] = 0;
    rom[65] = 9_999;
    rom
}

fn price_search() -> Open {
    Open {
        task: Task {
            title: Tr::new("The Price List", "价目表"),
            brief: Tr::new(
                "At the oasis market the price list rom[0..64] is sorted from low to high, and rom[63] is 10 000.\nEach lane brings an offer rom[64 + lane], 0 to 9 999. Write how many prices are below it to out[lane].\nScore: cycles.",
                "绿洲集市的价目表 rom[0..64] 从低到高排好，rom[63] 是 10 000。\n每条车道带来一个出价 rom[64 + lane]，0 到 9 999。把低于它的价格个数写到 out[lane]。\n得分：周期数。",
            ),
            cases: || rom_cases(market, &[421, 422]),
            hidden: || {
                vec![
                    Case {
                        rom: exact(423),
                        frame: 0,
                    },
                    Case {
                        rom: market(424),
                        frame: 0,
                    },
                ]
            },
            expect: |c| {
                lanes(|l| c.rom[..64].iter().filter(|&&p| p < c.rom[64 + l]).count() as i32)
            },
            reference: PRICE_HALVE,
            ..base(4, "price-search", 11, WARP_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[27, 55, 390],
    }
}

/// Binary search with SEL. r2 is one below the count found so far; the probe
/// address r2 + step is also the next r2 when the probe is below the offer.
const PRICE_HALVE: Source = Source::Code(concat!(
    "ADD r1, lane, 64\nLD r0, [r1]\n",
    "LD r3, [31]\nSETP.LT p0, r3, r0\nSEL r2, p0, 31, -1\n",
    "ADD r4, r2, 16\nLD r3, [r4]\nSETP.LT p0, r3, r0\nSEL r2, p0, r4, r2\n",
    "ADD r4, r2, 8\nLD r3, [r4]\nSETP.LT p0, r3, r0\nSEL r2, p0, r4, r2\n",
    "ADD r4, r2, 4\nLD r3, [r4]\nSETP.LT p0, r3, r0\nSEL r2, p0, r4, r2\n",
    "ADD r4, r2, 2\nLD r3, [r4]\nSETP.LT p0, r3, r0\nSEL r2, p0, r4, r2\n",
    "ADD r4, r2, 1\nLD r3, [r4]\nSETP.LT p0, r3, r0\nSEL r2, p0, r4, r2\n",
    "ADD r2, r2, 1\nST [lane], r2\n",
));

/// The same search, each step taken under IF.
#[cfg(test)]
const PRICE_IF: Source = Source::Code(concat!(
    "ADD r1, lane, 64\nLD r0, [r1]\nMOV r2, -1\nMOV r5, 32\n",
    "LOOP 6\n  ADD r4, r2, r5\n  LD r3, [r4]\n  SETP.LT p0, r3, r0\n  IF p0\n    MOV r2, r4\n  ENDIF\n  SHR r5, r5, 1\nENDLOOP\n",
    "ADD r2, r2, 1\nST [lane], r2\n",
));

/// Every lane reads the whole list and counts.
#[cfg(test)]
const PRICE_SCAN: Source = Source::Code(concat!(
    "ADD r1, lane, 64\nLD r0, [r1]\nMOV r2, 0\nMOV r5, 0\n",
    "LOOP 64\n  LD r3, [r5]\n  ADD r5, r5, 1\n  SETP.LT p0, r3, r0\n  SEL r4, p0, 1, 0\n  ADD r2, r2, r4\nENDLOOP\n",
    "ST [lane], r2\n",
));

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grade::grade;

    #[test]
    fn alternatives_pass_and_score_on_marks() {
        let alts: [&[Source]; 3] = [
            &[STEP_ARITH, STEP_MASK],
            &[VANE_IF, VANE_TREE],
            &[PRICE_IF, PRICE_SCAN],
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
