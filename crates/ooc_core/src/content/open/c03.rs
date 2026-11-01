//! Chapter 3 open problems: Ali's caravan, one warp of thirty-two lanes.

use oo_base::Tr;

use super::Open;
use crate::content::warp::{base, lanes, rom_cases, values};
use crate::isa::{Alu, LANES, OpKind};
use crate::task::*;

const fn alu(a: Alu) -> OpKind {
    OpKind::Alu(a)
}

/// What chapter 3 has unlocked: loops and the warp operations, no IF yet.
const OPS: &[OpKind] = &[
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
    vec![heaviest_camel(), cargo_tally(), caravan_sort()]
}

/// The lowest lane holding the largest value.
fn heaviest(rom: &[i32]) -> i32 {
    let top = rom[..LANES].iter().max().unwrap();
    rom.iter().position(|v| v == top).unwrap() as i32
}

fn heaviest_camel() -> Open {
    Open {
        task: Task {
            title: Tr::new("The Heaviest Camel", "最重的骆驼"),
            brief: Tr::new(
                "Ali weighs the 32 camels of the caravan: camel lane carries rom[lane]. Every lane writes the number of the heaviest camel to out[lane]; on a tie, the lowest number wins.\nScore: cycles.",
                "阿砾给商队的 32 头骆驼称重：lane 号骆驼驮着 rom[lane]。每条车道都把最重那头骆驼的编号写到 out[lane]；一样重时取编号最小的。\n得分：周期数。",
            ),
            cases: || rom_cases(|s| values(s, 32, 0, 1000), &[301, 302, 303]),
            hidden: || rom_cases(|s| values(s, 32, 0, 6), &[304, 305]),
            expect: |c| lanes(|_| heaviest(&c.rom)),
            reference: HEAVY_KEY,
            ..base(3, "heaviest-camel", 9, OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[21, 33, 47],
    }
}

/// Packs weight and lane into one key, weight·32 − lane, and takes a butterfly MAX.
const HEAVY_KEY: Source = Source::Code(concat!(
    "LD r0, [lane]\nSHL r0, r0, 5\nSUB r0, r0, lane\n",
    "XOR r1, lane, 16\nSHFL r1, r0, r1\nMAX r0, r0, r1\n",
    "XOR r1, lane, 8\nSHFL r1, r0, r1\nMAX r0, r0, r1\n",
    "XOR r1, lane, 4\nSHFL r1, r0, r1\nMAX r0, r0, r1\n",
    "XOR r1, lane, 2\nSHFL r1, r0, r1\nMAX r0, r0, r1\n",
    "XOR r1, lane, 1\nSHFL r1, r0, r1\nMAX r0, r0, r1\n",
    "SUB r0, 0, r0\nAND r0, r0, 31\nST [lane], r0\n",
));

/// The same key, with the five rounds in a loop.
#[cfg(test)]
const HEAVY_LOOP: Source = Source::Code(concat!(
    "LD r0, [lane]\nSHL r0, r0, 5\nSUB r0, r0, lane\nMOV r2, 16\n",
    "LOOP 5\n  XOR r1, lane, r2\n  SHFL r1, r0, r1\n  MAX r0, r0, r1\n  SHR r2, r2, 1\nENDLOOP\n",
    "SUB r0, 0, r0\nAND r0, r0, 31\nST [lane], r0\n",
));

/// The weight first, then the lowest lane that has it: two butterflies.
#[cfg(test)]
const HEAVY_TWICE: Source = Source::Code(concat!(
    "LD r0, [lane]\nMOV r5, r0\n",
    "XOR r1, lane, 16\nSHFL r1, r0, r1\nMAX r0, r0, r1\n",
    "XOR r1, lane, 8\nSHFL r1, r0, r1\nMAX r0, r0, r1\n",
    "XOR r1, lane, 4\nSHFL r1, r0, r1\nMAX r0, r0, r1\n",
    "XOR r1, lane, 2\nSHFL r1, r0, r1\nMAX r0, r0, r1\n",
    "XOR r1, lane, 1\nSHFL r1, r0, r1\nMAX r0, r0, r1\n",
    "SETP.EQ p0, r5, r0\nSEL r0, p0, lane, 32\nMOV r2, 16\n",
    "LOOP 5\n  XOR r1, lane, r2\n  SHFL r1, r0, r1\n  MIN r0, r0, r1\n  SHR r2, r2, 1\nENDLOOP\n",
    "ST [lane], r0\n",
));

/// rom[lane] is one of four kinds of cargo, 0 to 3.
fn tally(rom: &[i32]) -> Vec<Option<i32>> {
    (0..LANES as i32)
        .map(|k| (k < 4).then(|| rom.iter().filter(|&&v| v == k).count() as i32))
        .collect()
}

fn cargo_tally() -> Open {
    Open {
        task: Task {
            title: Tr::new("The Cargo Tally", "货物清点"),
            brief: Tr::new(
                "Each camel carries one kind of cargo, rom[lane]: 0 water, 1 salt, 2 cloth, 3 dates.\nWrite how many camels carry each kind to out[0..4]. The other cells are not checked.\nScore: cycles.",
                "每头骆驼驮一种货，rom[lane]：0 水，1 盐，2 布，3 枣。\n把驮每种货的骆驼数写到 out[0..4]。其余的格子不检查。\n得分：周期数。",
            ),
            cases: || rom_cases(|s| values(s, 32, 0, 4), &[311, 312, 313]),
            hidden: || {
                let mut v = rom_cases(|s| values(s, 32, 0, 4), &[314]);
                v.push(Case {
                    rom: vec![3; 32],
                    frame: 0,
                });
                v
            },
            expect: |c| tally(&c.rom),
            reference: TALLY_PACKED,
            ..base(3, "cargo-tally", 10, OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[22, 34, 229],
    }
}

/// One 8-bit counter per kind, packed into a word: 1 SHL (8 · kind). A single
/// butterfly sum adds all four counters at once.
const TALLY_PACKED: Source = Source::Code(concat!(
    "LD r0, [lane]\nSHL r0, r0, 3\nSHL r0, 1, r0\n",
    "XOR r1, lane, 16\nSHFL r1, r0, r1\nADD r0, r0, r1\n",
    "XOR r1, lane, 8\nSHFL r1, r0, r1\nADD r0, r0, r1\n",
    "XOR r1, lane, 4\nSHFL r1, r0, r1\nADD r0, r0, r1\n",
    "XOR r1, lane, 2\nSHFL r1, r0, r1\nADD r0, r0, r1\n",
    "XOR r1, lane, 1\nSHFL r1, r0, r1\nADD r0, r0, r1\n",
    "SHL r1, lane, 3\nSHR r0, r0, r1\nAND r0, r0, 255\nST [lane], r0\n",
));

/// The packed counters with the rounds in a loop.
#[cfg(test)]
const TALLY_LOOP: Source = Source::Code(concat!(
    "LD r0, [lane]\nSHL r0, r0, 3\nSHL r0, 1, r0\nMOV r2, 16\n",
    "LOOP 5\n  XOR r1, lane, r2\n  SHFL r1, r0, r1\n  ADD r0, r0, r1\n  SHR r2, r2, 1\nENDLOOP\n",
    "SHL r1, lane, 3\nSHR r0, r0, r1\nAND r0, r0, 255\nST [lane], r0\n",
));

/// Every lane walks the whole caravan, counting the camels of its own kind.
#[cfg(test)]
const TALLY_WALK: Source = Source::Code(concat!(
    "LD r0, [lane]\nMOV r1, lane\nMOV r2, 0\n",
    "LOOP 32\n  ADD r1, r1, 1\n  AND r1, r1, 31\n  SHFL r3, r0, r1\n  SETP.EQ p0, r3, lane\n  SEL r4, p0, 1, 0\n  ADD r2, r2, r4\nENDLOOP\n",
    "ST [lane], r2\n",
));

/// The values in ascending order.
fn sorted(rom: &[i32]) -> Vec<Option<i32>> {
    let mut v = rom[..LANES].to_vec();
    v.sort();
    v.into_iter().map(Some).collect()
}

fn caravan_sort() -> Open {
    Open {
        task: Task {
            title: Tr::new("Line Up the Caravan", "给商队排队"),
            brief: Tr::new(
                "Before the dunes, Ali lines the camels up from lightest to heaviest. Camel lane weighs rom[lane].\nWrite the 32 weights to out[0..32] in ascending order.\nScore: cycles.",
                "进沙丘之前，阿砾要让骆驼从轻到重排好队。lane 号骆驼重 rom[lane]。\n把 32 个重量从小到大写到 out[0..32]。\n得分：周期数。",
            ),
            cases: || rom_cases(|s| values(s, 32, 0, 1000), &[321, 322]),
            hidden: || {
                let mut v = rom_cases(|s| values(s, 32, 0, 9), &[323]);
                v.push(Case {
                    rom: (0..32).map(|i| 990 - 30 * i).collect(),
                    frame: 0,
                });
                v
            },
            expect: |c| sorted(&c.rom),
            reference: SORT_RANK,
            ..base(3, "caravan-sort", 11, OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[147, 165, 377],
    }
}

/// Rank sort. The key weight·32 + lane makes every key distinct; each lane counts
/// the smaller keys as they rotate past, two per round, with the sign bit of the
/// difference standing in for a compare. The rank is where the weight goes.
const SORT_RANK: Source = Source::Code(concat!(
    "LD r0, [lane]\nSHL r0, r0, 5\nADD r0, r0, lane\nADD r1, lane, 1\nAND r1, r1, 31\n",
    "SHFL r3, r0, r1\nSUB r2, r3, r0\nSHR r2, r2, 31\n",
    "LOOP 15\n  SHFL r3, r3, r1\n  SUB r4, r3, r0\n  SHR r4, r4, 31\n  ADD r2, r2, r4\n",
    "  SHFL r3, r3, r1\n  SUB r4, r3, r0\n  SHR r4, r4, 31\n  ADD r2, r2, r4\nENDLOOP\n",
    "SUB r2, 0, r2\nSHR r0, r0, 5\nST [r2], r0\n",
));

/// The same rank, one key per round.
#[cfg(test)]
const SORT_ONE: Source = Source::Code(concat!(
    "LD r0, [lane]\nSHL r0, r0, 5\nADD r0, r0, lane\nADD r1, lane, 1\nAND r1, r1, 31\n",
    "MOV r3, r0\nMOV r2, 0\n",
    "LOOP 31\n  SHFL r3, r3, r1\n  SUB r4, r3, r0\n  SHR r4, r4, 31\n  SUB r2, r2, r4\nENDLOOP\n",
    "SHR r0, r0, 5\nST [r2], r0\n",
));

/// Compares weights and lanes directly, recomputing the source lane each round.
#[cfg(test)]
const SORT_SEL: Source = Source::Code(concat!(
    "LD r0, [lane]\nMOV r1, lane\nMOV r2, 0\n",
    "LOOP 31\n  ADD r1, r1, 1\n  AND r1, r1, 31\n  SHFL r3, r0, r1\n",
    "  SETP.LT p0, r3, r0\n  SEL r4, p0, 1, 0\n  ADD r2, r2, r4\n",
    "  SETP.EQ p0, r3, r0\n  SETP.LT p1, r1, lane\n  SEL r4, p1, 1, 0\n  SEL r4, p0, r4, 0\n  ADD r2, r2, r4\nENDLOOP\n",
    "ST [r2], r0\n",
));

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grade::grade;

    #[test]
    fn alternatives_pass_and_score_on_marks() {
        let alts: [&[Source]; 3] = [
            &[HEAVY_LOOP, HEAVY_TWICE],
            &[TALLY_LOOP, TALLY_WALK],
            &[SORT_ONE, SORT_SEL],
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
