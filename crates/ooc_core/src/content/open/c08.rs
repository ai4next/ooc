//! Chapter 8 open problems: Douya's post office on Vault, with shared memory,
//! the barrier and atomics.

use oo_base::Tr;

use super::Open;
use crate::content::prism::rom_case;
use crate::content::vault::{ALL_WARPS, VAULT_OPS, base};
use crate::content::{below, rng};
use crate::task::*;

fn cells(v: impl IntoIterator<Item = i32>) -> Vec<Option<i32>> {
    v.into_iter().map(Some).collect()
}

fn values(seed: u64, n: usize, lo: i32, hi: i32) -> Vec<i32> {
    let mut r = rng(seed);
    (0..n).map(|_| lo + below(&mut r, hi - lo)).collect()
}

fn post(id: &'static str, slot: u8, out_len: usize) -> Task {
    Task {
        chapter: 8,
        ..base(id, slot, ALL_WARPS, out_len, VAULT_OPS)
    }
}

pub fn problems() -> Vec<Open> {
    vec![transpose_64(), mail_total(), busy_box()]
}

fn transpose_64() -> Open {
    Open {
        task: Task {
            title: Tr::new("The Big Turn", "大翻身"),
            brief: Tr::new(
                "The post office's sorting table has grown to 64 by 64, at rom[0..4096]. Turn it on its side: out[64c + r] = rom[64r + c].\nAll 16 warps run, 512 threads. A read misses for 400 cycles and the L1 holds 16 lines; stores cost nothing extra.\nScore: cycles.",
                "邮局的分拣表长到了 64×64，放在 rom[0..4096]。把它翻个身：out[64c + r] = rom[64r + c]。\n16 个 warp 全部运行，共 512 个线程。一次读缺失要等 400 个周期，L1 只放得下 16 行；写不额外花时间。\n得分：周期数。",
            ),
            cases: || {
                [811, 812]
                    .map(|s| rom_case(values(s, 4096, 0, 1000)))
                    .to_vec()
            },
            hidden: || {
                vec![
                    rom_case(values(813, 4096, -500, 500)),
                    rom_case((0..4096).collect()),
                ]
            },
            expect: |c| cells((0..4096).map(|i| c.rom[(i % 64) * 64 + i / 64])),
            reference: TURN_SCATTER,
            ..post("transpose-64", 9, 4096)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[910, 1648, 16786],
    }
}

/// Thread t reads rom[t + 512k], whole lines at a time, all eight before it
/// uses any; since stores are free, the turn happens on the way out.
const TURN_SCATTER: Source = Source::Code(concat!(
    "MOV r9, tid\nLD r0, [r9]\nADD r9, r9, 512\nLD r1, [r9]\nADD r9, r9, 512\nLD r2, [r9]\nADD r9, r9, 512\nLD r3, [r9]\n",
    "ADD r9, r9, 512\nLD r4, [r9]\nADD r9, r9, 512\nLD r5, [r9]\nADD r9, r9, 512\nLD r6, [r9]\nADD r9, r9, 512\nLD r7, [r9]\n",
    "AND r8, tid, 63\nSHL r8, r8, 6\nSHR r10, tid, 6\nADD r8, r8, r10\n",
    "ST [r8], r0\nADD r8, r8, 8\nST [r8], r1\nADD r8, r8, 8\nST [r8], r2\nADD r8, r8, 8\nST [r8], r3\n",
    "ADD r8, r8, 8\nST [r8], r4\nADD r8, r8, 8\nST [r8], r5\nADD r8, r8, 8\nST [r8], r6\nADD r8, r8, 8\nST [r8], r7\n",
));

/// The textbook tile: rows in, parked in shared memory at a pitch of 65 so
/// neither the writes nor the reads share a bank, then rows out.
#[cfg(test)]
const TURN_TILE: Source = Source::Code(concat!(
    "MOV r9, tid\nLD r0, [r9]\nADD r9, r9, 512\nLD r1, [r9]\nADD r9, r9, 512\nLD r2, [r9]\nADD r9, r9, 512\nLD r3, [r9]\n",
    "ADD r9, r9, 512\nLD r4, [r9]\nADD r9, r9, 512\nLD r5, [r9]\nADD r9, r9, 512\nLD r6, [r9]\nADD r9, r9, 512\nLD r7, [r9]\n",
    "AND r8, tid, 63\nMUL r8, r8, 65\nSHR r10, tid, 6\nADD r8, r8, r10\n",
    "STS [r8], r0\nADD r8, r8, 8\nSTS [r8], r1\nADD r8, r8, 8\nSTS [r8], r2\nADD r8, r8, 8\nSTS [r8], r3\n",
    "ADD r8, r8, 8\nSTS [r8], r4\nADD r8, r8, 8\nSTS [r8], r5\nADD r8, r8, 8\nSTS [r8], r6\nADD r8, r8, 8\nSTS [r8], r7\nBAR\n",
    "SHR r8, tid, 6\nMUL r8, r8, 65\nAND r10, tid, 63\nADD r8, r8, r10\nMOV r9, tid\n",
    "LOOP 8\n  LDS r0, [r8]\n  ADD r8, r8, 520\n  ST [r9], r0\n  ADD r9, r9, 512\nENDLOOP\n",
));

/// Thread t writes out[t + 512k] in order and fetches each word down a column:
/// every read touches 32 lines.
#[cfg(test)]
const TURN_GATHER: Source = Source::Code(concat!(
    "AND r9, tid, 63\nSHL r9, r9, 6\nSHR r10, tid, 6\nADD r9, r9, r10\n",
    "LD r0, [r9]\nADD r9, r9, 8\nLD r1, [r9]\nADD r9, r9, 8\nLD r2, [r9]\nADD r9, r9, 8\nLD r3, [r9]\n",
    "ADD r9, r9, 8\nLD r4, [r9]\nADD r9, r9, 8\nLD r5, [r9]\nADD r9, r9, 8\nLD r6, [r9]\nADD r9, r9, 8\nLD r7, [r9]\n",
    "MOV r8, tid\nST [r8], r0\nADD r8, r8, 512\nST [r8], r1\nADD r8, r8, 512\nST [r8], r2\nADD r8, r8, 512\nST [r8], r3\n",
    "ADD r8, r8, 512\nST [r8], r4\nADD r8, r8, 512\nST [r8], r5\nADD r8, r8, 512\nST [r8], r6\nADD r8, r8, 512\nST [r8], r7\n",
));

fn mail_total() -> Open {
    Open {
        task: Task {
            title: Tr::new("The Day's Postage", "一天的邮资"),
            brief: Tr::new(
                "Douya adds up the day's postage: 4096 stamps at rom[0..4096]. Write their total to out[0].\nAll 16 warps run, 512 threads, in one block. ATOM adds into out, ATOMS into shared memory, and lanes that hit the same word wait their turn.\nScore: cycles.",
                "豆芽要把一天的邮资加起来：rom[0..4096] 里有 4096 张邮票。把总数写到 out[0]。\n16 个 warp 全部运行，共 512 个线程，在同一个块里。ATOM 往 out 里加，ATOMS 往共享内存里加，打到同一个字上的车道要排队。\n得分：周期数。",
            ),
            cases: || {
                [821, 822]
                    .map(|s| rom_case(values(s, 4096, 0, 1000)))
                    .to_vec()
            },
            hidden: || {
                vec![
                    rom_case(values(823, 4096, -1000, 1000)),
                    rom_case(vec![999; 4096]),
                ]
            },
            expect: |c| vec![Some(c.rom.iter().sum())],
            reference: SUM_WARP_ATOM,
            ..post("mail-total", 10, 1)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[1105, 1143, 1657],
    }
}

/// Each thread sums its eight stamps; the code shared by the totals below.
macro_rules! eight {
    () => {
        concat!(
            "MOV r9, tid\nLD r0, [r9]\nADD r9, r9, 512\nLD r1, [r9]\nADD r9, r9, 512\nLD r2, [r9]\nADD r9, r9, 512\nLD r3, [r9]\n",
            "ADD r9, r9, 512\nLD r4, [r9]\nADD r9, r9, 512\nLD r5, [r9]\nADD r9, r9, 512\nLD r6, [r9]\nADD r9, r9, 512\nLD r7, [r9]\n",
            "ADD r0, r0, r1\nADD r2, r2, r3\nADD r4, r4, r5\nADD r6, r6, r7\nADD r0, r0, r2\nADD r4, r4, r6\nADD r0, r0, r4\n",
        )
    };
}

/// A butterfly sum across the warp's 32 lanes.
macro_rules! butterfly {
    () => {
        concat!(
            "XOR r10, lane, 16\nSHFL r11, r0, r10\nADD r0, r0, r11\nXOR r10, lane, 8\nSHFL r11, r0, r10\nADD r0, r0, r11\n",
            "XOR r10, lane, 4\nSHFL r11, r0, r10\nADD r0, r0, r11\nXOR r10, lane, 2\nSHFL r11, r0, r10\nADD r0, r0, r11\n",
            "XOR r10, lane, 1\nSHFL r11, r0, r10\nADD r0, r0, r11\n",
        )
    };
}

/// Each warp sums itself with SHFL, then one lane per warp adds it into out[0].
const SUM_WARP_ATOM: Source = Source::Code(concat!(
    eight!(),
    butterfly!(),
    "SETP.EQ p0, lane, 0\nIF p0\n  ATOM r1, [0], r0\nENDIF\n",
));

/// The warps' sums parked in shared memory; after the barrier warp 0 adds them.
#[cfg(test)]
const SUM_BAR: Source = Source::Code(concat!(
    eight!(),
    butterfly!(),
    "STS [warp], r0\nBAR\nSETP.EQ p0, warp, 0\nIF p0\n  AND r1, lane, 15\n  LDS r0, [r1]\n",
    "  XOR r10, lane, 8\n  SHFL r11, r0, r10\n  ADD r0, r0, r11\n  XOR r10, lane, 4\n  SHFL r11, r0, r10\n  ADD r0, r0, r11\n",
    "  XOR r10, lane, 2\n  SHFL r11, r0, r10\n  ADD r0, r0, r11\n  XOR r10, lane, 1\n  SHFL r11, r0, r10\n  ADD r0, r0, r11\n",
    "  ST [0], r0\nENDIF\n",
));

/// Every thread adds its eight stamps straight into out[0].
#[cfg(test)]
const SUM_ATOM: Source = Source::Code(concat!(eight!(), "ATOM r1, [0], r0\n"));

/// 4096 letters for boxes 0..32; `busy` in a hundred go to box 0.
fn letters(seed: u64, busy: i32) -> Case {
    let mut r = rng(seed);
    rom_case(
        (0..4096)
            .map(|_| {
                if below(&mut r, 100) < busy {
                    0
                } else {
                    below(&mut r, 32)
                }
            })
            .collect(),
    )
}

fn busy_box() -> Open {
    Open {
        task: Task {
            title: Tr::new("The Busiest Box", "最忙的格口"),
            brief: Tr::new(
                "4096 letters at rom[0..4096] are bound for boxes 0 to 31; on a festival day most go to box 0. Write how many go to box b to out[b].\nAll 16 warps run, 512 threads, in one block. Lanes of one warp that ATOM or ATOMS the same word wait their turn, one after another.\nScore: cycles.",
                "rom[0..4096] 里的 4096 封信要送往 0 到 31 号格口；过节那天大多数都去 0 号。把去 b 号格口的信数写到 out[b]。\n16 个 warp 全部运行，共 512 个线程，在同一个块里。同一个 warp 里对同一个字做 ATOM 或 ATOMS 的车道要排队，一个接一个。\n得分：周期数。",
            ),
            cases: || vec![letters(831, 50), letters(832, 80)],
            hidden: || vec![rom_case(vec![7; 4096]), letters(833, 0)],
            expect: |c| cells((0..32).map(|b| c.rom.iter().filter(|&&v| v == b).count() as i32)),
            reference: BOX_PRIVATE,
            ..post("busy-box", 11, 32)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[1159, 4501, 8593],
    }
}

/// The eight letters of a thread, loaded before any is used.
macro_rules! eight_letters {
    () => {
        concat!(
            "MOV r9, tid\nLD r0, [r9]\nADD r9, r9, 512\nLD r1, [r9]\nADD r9, r9, 512\nLD r2, [r9]\nADD r9, r9, 512\nLD r3, [r9]\n",
            "ADD r9, r9, 512\nLD r4, [r9]\nADD r9, r9, 512\nLD r5, [r9]\nADD r9, r9, 512\nLD r6, [r9]\nADD r9, r9, 512\nLD r7, [r9]\n",
        )
    };
}

/// A counter per box and per lane, at shared word 32·box + lane: no two lanes
/// of a warp ever hit one word or one bank. After the barrier, lane b of warp 0
/// adds up row b, starting at a different column in each lane.
const BOX_PRIVATE: Source = Source::Code(concat!(
    eight_letters!(),
    "SHL r0, r0, 5\nADD r0, r0, lane\nATOMS r8, [r0], 1\nSHL r1, r1, 5\nADD r1, r1, lane\nATOMS r8, [r1], 1\n",
    "SHL r2, r2, 5\nADD r2, r2, lane\nATOMS r8, [r2], 1\nSHL r3, r3, 5\nADD r3, r3, lane\nATOMS r8, [r3], 1\n",
    "SHL r4, r4, 5\nADD r4, r4, lane\nATOMS r8, [r4], 1\nSHL r5, r5, 5\nADD r5, r5, lane\nATOMS r8, [r5], 1\n",
    "SHL r6, r6, 5\nADD r6, r6, lane\nATOMS r8, [r6], 1\nSHL r7, r7, 5\nADD r7, r7, lane\nATOMS r8, [r7], 1\n",
    "BAR\nSETP.EQ p0, warp, 0\nIF p0\n  SHL r1, lane, 5\n  MOV r2, lane\n  MOV r0, 0\n",
    "  LOOP 32\n    AND r3, r2, 31\n    ADD r3, r3, r1\n    LDS r3, [r3]\n    ADD r2, r2, 1\n    ADD r0, r0, r3\n  ENDLOOP\n",
    "  ST [lane], r0\nENDIF\n",
));

/// One shared counter per box.
#[cfg(test)]
const BOX_SHARED: Source = Source::Code(concat!(
    eight_letters!(),
    "ATOMS r8, [r0], 1\nATOMS r8, [r1], 1\nATOMS r8, [r2], 1\nATOMS r8, [r3], 1\n",
    "ATOMS r8, [r4], 1\nATOMS r8, [r5], 1\nATOMS r8, [r6], 1\nATOMS r8, [r7], 1\n",
    "BAR\nSETP.EQ p0, warp, 0\nIF p0\n  LDS r0, [lane]\n  ST [lane], r0\nENDIF\n",
));

/// Every letter goes straight to its counter in out.
#[cfg(test)]
const BOX_GLOBAL: Source = Source::Code(concat!(
    eight_letters!(),
    "ATOM r8, [r0], 1\nATOM r8, [r1], 1\nATOM r8, [r2], 1\nATOM r8, [r3], 1\n",
    "ATOM r8, [r4], 1\nATOM r8, [r5], 1\nATOM r8, [r6], 1\nATOM r8, [r7], 1\n",
));

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grade::grade;

    #[test]
    fn alternatives_pass_and_score_on_marks() {
        let alts: [&[Source]; 3] = [
            &[TURN_TILE, TURN_GATHER],
            &[SUM_BAR, SUM_ATOM],
            &[BOX_SHARED, BOX_GLOBAL],
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
