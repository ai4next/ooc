//! Chapter 9 open problems: Bohe's clinic on the Vault (textures and images).

use oo_base::Tr;

use super::Open;
use crate::content::prism::rom_case;
use crate::content::vault::{ALL_WARPS, VAULT_OPS, base};
use crate::content::{below, rng};
use crate::task::*;

pub fn problems() -> Vec<Open> {
    vec![soft_focus(), colour_table(), magnifier()]
}

/// A 64 x 64 picture, row-major (`rom[64 y + x]`), values 0..=255.
fn picture64(kind: u8, seed: u64) -> Vec<i32> {
    let mut r = rng(seed);
    (0..4096)
        .map(|i| {
            let (x, y) = (i % 64, i / 64);
            match kind {
                0 => below(&mut r, 256),
                // Vertical stripes, 3 wide.
                1 => {
                    if (x / 3) % 2 == 0 {
                        230
                    } else {
                        20
                    }
                }
                // A bright frame on a dim ground.
                2 => {
                    if x == 0 || y == 0 || x == 63 || y == 63 {
                        255
                    } else {
                        below(&mut r, 60)
                    }
                }
                // A diagonal ramp with one hot pixel.
                _ => {
                    if (x, y) == (17, 40) {
                        255
                    } else {
                        (x + 2 * y) % 200
                    }
                }
            }
        })
        .collect()
}

// ---------------------------------------------------------------- soft-focus

fn focus_case(kind: u8, seed: u64) -> Case {
    rom_case(picture64(kind, seed))
}

/// 3 x 3 binomial blur with clamped edges: (corners + 2 · sides + 4 · centre) >> 4.
fn focused(c: &Case) -> Vec<Option<i32>> {
    let at = |x: i32, y: i32| c.rom[(y.clamp(0, 63) * 64 + x.clamp(0, 63)) as usize];
    (0..4096)
        .map(|i| {
            let (x, y) = (i % 64, i / 64);
            let row = |y: i32| at(x - 1, y) + 2 * at(x, y) + at(x + 1, y);
            Some((row(y - 1) + 2 * row(y) + row(y + 1)) >> 4)
        })
        .collect()
}

/// One row of the 3 x 3 window at address `at`: left + 2 · centre + right
/// into `into` (the column offsets to the left and right are in r3 and r4).
fn focus_row(ld: &str, at: &str, into: &str) -> String {
    format!(
        "ADD r8, {at}, r3\n{ld} r9, [r8]\n{ld} r10, [{at}]\nADD r8, {at}, r4\n{ld} r11, [r8]\n\
         ADD r9, r9, r11\nSHL r10, r10, 1\nADD {into}, r9, r10\n"
    )
}

/// Column offsets of the clamped neighbours: x in r2, left in r3, right in r4.
const FOCUS_COLS: &str = "AND r2, tid, 63\nSUB r3, r2, 1\nMAX r3, r3, 0\nSUB r3, r3, r2\n\
    ADD r4, r2, 1\nMIN r4, r4, 63\nSUB r4, r4, r2\n";

/// Thread `tid` walks 8 rows of column `tid AND 63` downwards and keeps the
/// row sums of the two rows above, so each output costs one new row of three
/// reads. `staged`: the picture is first copied to shared memory (eight loads
/// in flight per thread) and read with LDS.
fn focus_slide(staged: bool) -> &'static str {
    let ld = if staged { "LDS" } else { "LD" };
    let mut s = String::new();
    if staged {
        s += "LD r1, [tid]\n";
        for k in 1..8 {
            s += &format!("ADD r0, tid, {}\nLD r{}, [r0]\n", 512 * k, k + 1);
        }
        s += "STS [tid], r1\n";
        for k in 1..8 {
            s += &format!("ADD r0, tid, {}\nSTS [r0], r{}\n", 512 * k, k + 1);
        }
        s += "BAR\n";
    }
    s += FOCUS_COLS;
    s += "SHR r5, tid, 6\nSHL r5, r5, 3\nSUB r6, r5, 1\nMAX r6, r6, 0\nSHL r6, r6, 6\nADD r6, r6, r2\n";
    s += &focus_row(ld, "r6", "r12");
    s += "SHL r0, r5, 6\nADD r0, r0, r2\n";
    s += &focus_row(ld, "r0", "r13");
    s += "LOOP 8\nADD r7, r5, 1\nMIN r7, r7, 63\nSHL r7, r7, 6\nADD r7, r7, r2\n";
    s += &focus_row(ld, "r7", "r9");
    s += "SHL r14, r13, 1\nADD r14, r14, r12\nADD r14, r14, r9\nSHR r14, r14, 4\nST [r0], r14\n\
          MOV r12, r13\nMOV r13, r9\nADD r0, r0, 64\nADD r5, r5, 1\nENDLOOP\n";
    Box::leak(s.into_boxed_str())
}

/// The plain window: nine global reads per output, rows `tid >> 6`, +8, ... +56.
#[cfg(test)]
fn focus_plain() -> &'static str {
    let mut s = String::from(FOCUS_COLS);
    s += "SHR r5, tid, 6\nMOV r0, tid\nLOOP 8\n\
          SUB r6, r5, 1\nMAX r6, r6, 0\nSUB r6, r6, r5\nSHL r6, r6, 6\nADD r6, r6, r0\n\
          ADD r7, r5, 1\nMIN r7, r7, 63\nSUB r7, r7, r5\nSHL r7, r7, 6\nADD r7, r7, r0\n";
    s += &focus_row("LD", "r6", "r12");
    s += &focus_row("LD", "r0", "r13");
    s += &focus_row("LD", "r7", "r9");
    s += "SHL r14, r13, 1\nADD r14, r14, r12\nADD r14, r14, r9\nSHR r14, r14, 4\nST [r0], r14\n\
          ADD r0, r0, 512\nADD r5, r5, 8\nENDLOOP\n";
    Box::leak(s.into_boxed_str())
}

fn soft_focus() -> Open {
    Open {
        task: Task {
            chapter: 9,
            slot: 9,
            title: Tr::new("Soft Focus", "柔焦"),
            brief: Tr::new(
                "Bohe's X-ray is 64 x 64 pixels, rom[64y + x]. Soften it: out[64y + x] = (sum of the 3 x 3 window, weights 1 2 1 / 2 4 2 / 1 2 1) >> 4, edges clamped.\nScored by total cycles: fewer is better.",
                "薄荷的 X 光片是 64 x 64 像素，rom[64y + x]。把它变柔：out[64y + x] = 3 x 3 窗口的加权和（权重 1 2 1 / 2 4 2 / 1 2 1）>> 4，边缘取最近的像素。\n按总周期计分：越少越好。",
            ),
            cases: || vec![focus_case(0, 9101), focus_case(1, 9102), focus_case(2, 9103)],
            hidden: || vec![focus_case(3, 9104), focus_case(0, 9105)],
            expect: focused,
            starter: Source::Code(""),
            reference: Source::Code(focus_slide(true)),
            ..base("soft-focus", 9, ALL_WARPS, 4096, VAULT_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[4239, 5027, 7475],
    }
}

// ---------------------------------------------------------------- colour-table

/// The table is copied to shared memory (threads 0..255, one entry each)
/// and looked up with LDS. `batched`: each thread issues its eight picture
/// reads before the barrier, so they wait together; otherwise one read and
/// one lookup at a time.
fn table_src(batched: bool) -> &'static str {
    let mut s = String::from(
        "SETP.LT p0, tid, 256\nIF p0\nADD r0, tid, 4096\nLD r1, [r0]\nSTS [tid], r1\nENDIF\n",
    );
    if batched {
        for k in 0..8 {
            s += &format!("ADD r0, tid, {}\nLD r{}, [r0]\n", 512 * k, k + 1);
        }
        s += "BAR\n";
        for k in 1..=8 {
            s += &format!("LDS r{k}, [r{k}]\n");
        }
        for k in 0..8 {
            s += &format!("ADD r0, tid, {}\nST [r0], r{}\n", 512 * k, k + 1);
        }
    } else {
        s += "BAR\nMOV r0, tid\nLOOP 8\nLD r1, [r0]\nLDS r1, [r1]\nST [r0], r1\nADD r0, r0, 512\nENDLOOP\n";
    }
    Box::leak(s.into_boxed_str())
}

/// No staging: every lookup is a second global read, scattered over the table.
#[cfg(test)]
const TABLE_GATHER: &str = "MOV r0, tid\n\
    LOOP 8\n\
    LD r1, [r0]\n\
    ADD r1, r1, 4096\n\
    LD r1, [r1]\n\
    ST [r0], r1\n\
    ADD r0, r0, 512\n\
    ENDLOOP\n";

/// A picture at rom[0..4096) and a colour table of 256 entries at rom[4096..4352).
/// `ramp`: the table is a smooth curve instead of random colours.
fn table_case(kind: u8, seed: u64, ramp: bool) -> Case {
    let mut rom = picture64(kind, seed);
    let mut r = rng(seed + 50);
    rom.extend((0..256).map(|v| {
        if ramp {
            v * v / 255
        } else {
            below(&mut r, 256)
        }
    }));
    rom_case(rom)
}

fn tabled(c: &Case) -> Vec<Option<i32>> {
    (0..4096)
        .map(|i| Some(c.rom[4096 + c.rom[i] as usize]))
        .collect()
}

fn colour_table() -> Open {
    Open {
        task: Task {
            chapter: 9,
            slot: 10,
            title: Tr::new("Colour Table", "调色表"),
            brief: Tr::new(
                "Bohe wants the X-ray in false colour. The 64 x 64 picture is rom[0..4096), values 0..255; the table is rom[4096 + v]. Write out[i] = table[rom[i]].\nScored by total cycles: fewer is better.",
                "薄荷想把 X 光片换成伪彩色。64 x 64 的图在 rom[0..4096)，值 0..255；颜色表是 rom[4096 + v]。写 out[i] = 表[rom[i]]。\n按总周期计分：越少越好。",
            ),
            cases: || {
                vec![
                    table_case(0, 9201, false),
                    table_case(3, 9202, true),
                    table_case(1, 9203, false),
                ]
            },
            hidden: || vec![table_case(0, 9204, true), table_case(2, 9205, false)],
            expect: tabled,
            starter: Source::Code(""),
            reference: Source::Code(table_src(true)),
            ..base("colour-table", 10, ALL_WARPS, 4096, VAULT_OPS)
        },
        metric: Metric::Cycles,
        score: None,
        score_label: None,
        marks: &[1088, 3863, 7015],
    }
}

// ---------------------------------------------------------------- magnifier

/// A 32 x 32 texture, rom[32y + x], cut from a 64 x 64 picture.
fn mag_case(kind: u8, seed: u64) -> Case {
    let p = picture64(kind, seed);
    rom_case((0..1024).map(|i| p[(i / 32) * 64 + i % 32]).collect())
}

/// Bilinear x2: each output pixel weighs its own texel 3 and the clamped
/// neighbour on its side 1, first along x, then along y, flooring each time.
fn magnified(c: &Case) -> Vec<Option<i32>> {
    let near_far = |p: i32| {
        let n = p >> 1;
        (n, (n + 2 * (p & 1) - 1).clamp(0, 31))
    };
    (0..4096)
        .map(|i| {
            let ((nx, fx), (ny, fy)) = (near_far(i % 64), near_far(i / 64));
            let row = |y: i32| (3 * c.rom[(32 * y + nx) as usize] + c.rom[(32 * y + fx) as usize]) >> 2;
            Some((3 * row(ny) + row(fy)) >> 2)
        })
        .collect()
}

/// Thread `tid` owns column `tid AND 63` and every eighth row. LERP with the
/// weights 3/4, 1/4 and a zero (r13..r15, r9 stays 0) does each blend. Rows
/// advance by 8, so the side of the far row never changes.
const MAG_REF: &str = "AND r0, tid, 63\n\
    SHR r1, r0, 1\n\
    AND r2, r0, 1\n\
    SHL r2, r2, 1\n\
    ADD r2, r2, r1\n\
    SUB r2, r2, 1\n\
    MAX r2, r2, 0\n\
    MIN r2, r2, 31\n\
    SHR r3, tid, 6\n\
    AND r11, r3, 1\n\
    SHL r11, r11, 6\n\
    SUB r11, r11, 32\n\
    SHR r3, r3, 1\n\
    SHL r12, r3, 5\n\
    MOV r4, tid\n\
    MOV r13, 49152\n\
    MOV r14, 16384\n\
    LOOP 8\n\
    ADD r6, r12, r11\n\
    MAX r6, r6, 0\n\
    MIN r6, r6, 992\n\
    ADD r0, r12, r1\n\
    LD r7, [r0]\n\
    ADD r0, r12, r2\n\
    LD r8, [r0]\n\
    LERP r10, r7, r13\n\
    ADD r0, r6, r1\n\
    LD r7, [r0]\n\
    ADD r0, r6, r2\n\
    LD r8, [r0]\n\
    LERP r8, r7, r13\n\
    MOV r7, r10\n\
    LERP r7, r7, r13\n\
    ST [r4], r7\n\
    ADD r4, r4, 512\n\
    ADD r12, r12, 128\n\
    ENDLOOP\n";

/// The general Q16 sampler from the bilinear level, with LERP doing the blends.
#[cfg(test)]
const MAG_LERP: &str = "AND r0, tid, 63\n\
    SHL r15, r0, 15\n\
    SUB r15, r15, 16384\n\
    MAX r15, r15, 0\n\
    MIN r15, r15, 2031616\n\
    SHR r10, r15, 16\n\
    ADD r11, r10, 1\n\
    MIN r11, r11, 31\n\
    AND r5, r15, 65535\n\
    SUB r4, 65536, r5\n\
    MOV r6, 0\n\
    MOV r2, 0\n\
    SHR r13, tid, 6\n\
    SHL r13, r13, 15\n\
    SUB r13, r13, 16384\n\
    MOV r12, tid\n\
    LOOP 8\n\
    MAX r14, r13, 0\n\
    MIN r14, r14, 2031616\n\
    SHR r7, r14, 16\n\
    SHL r7, r7, 5\n\
    ADD r8, r7, 32\n\
    MIN r8, r8, 992\n\
    ADD r9, r7, r10\n\
    LD r0, [r9]\n\
    ADD r9, r7, r11\n\
    LD r1, [r9]\n\
    LERP r3, r0, r4\n\
    ADD r9, r8, r10\n\
    LD r0, [r9]\n\
    ADD r9, r8, r11\n\
    LD r1, [r9]\n\
    LERP r1, r0, r4\n\
    MOV r0, r3\n\
    AND r8, r14, 65535\n\
    SUB r7, 65536, r8\n\
    MOV r9, 0\n\
    LERP r3, r0, r7\n\
    ST [r12], r3\n\
    ADD r12, r12, 512\n\
    ADD r13, r13, 262144\n\
    ENDLOOP\n";

fn magnifier() -> Open {
    Open {
        task: Task {
            chapter: 9,
            slot: 11,
            title: Tr::new("The Magnifier", "放大镜"),
            brief: Tr::new(
                "Bohe's magnifier doubles a 32 x 32 texture (rom[32y + x]) into 64 x 64. Along x, out pixel X weighs texel X >> 1 by 3 and its neighbour on the side of X (left if X is even, right if odd, clamped) by 1, then >> 2; the same along y on those results.\nScored by instruction count: fewer is better.",
                "薄荷的放大镜把 32 x 32 的纹理（rom[32y + x]）放大成 64 x 64。沿 x，输出像素 X 给纹素 X >> 1 权重 3，给 X 那一侧的邻居（X 偶数取左边，奇数取右边，越界取边缘）权重 1，再 >> 2；再沿 y 对这些结果做同样的事。\n按指令数计分：越少越好。",
            ),
            cases: || vec![mag_case(0, 9301), mag_case(1, 9302), mag_case(2, 9303)],
            hidden: || vec![mag_case(3, 9304), mag_case(0, 9305)],
            expect: magnified,
            starter: Source::Code(""),
            reference: Source::Code(MAG_REF),
            ..base("magnifier", 11, ALL_WARPS, 4096, VAULT_OPS)
        },
        metric: Metric::Size,
        score: None,
        score_label: None,
        marks: &[37, 42],
    }
}

/// Checks shared by the chapter 9-16 open problems: the chapter's three
/// problems sit in slots 9-11 with bilingual text of at most three lines,
/// hidden cases differ from the public ones, the reference scores `marks[0]`
/// and every alternative passes with a score listed in `marks`. Returns the
/// alternatives' scores.
#[cfg(test)]
pub(super) fn check(chapter: u8, opens: &[Open], alts: &[(&str, &'static str)]) -> Vec<u32> {
    use crate::grade::grade;
    let score = |id: &str, src: &'static str| {
        let o = opens.iter().find(|o| o.task.id == id).unwrap();
        let rep = grade(&o.task, &Source::Code(src).solution()).unwrap();
        let s = o.score_of(&rep);
        println!("{id}: {s:?} {:?} {:?}", rep.verdict, rep.measure);
        (o, s)
    };
    let refs: Vec<_> = opens
        .iter()
        .map(|o| match o.task.reference {
            Source::Code(src) => score(o.task.id, src),
            _ => unreachable!(),
        })
        .collect();
    let got: Vec<_> = alts.iter().map(|&(id, src)| score(id, src)).collect();
    let slots: Vec<u8> = opens.iter().map(|o| o.task.slot).collect();
    assert_eq!(slots, vec![9, 10, 11]);
    for o in opens {
        let t = &o.task;
        assert_eq!(t.chapter, chapter, "{}", t.id);
        assert!(crate::content::task(t.id).is_none(), "{} is a level id", t.id);
        assert!(matches!(t.starter, Source::Code("")), "{} starter", t.id);
        for tr in [t.title, t.brief] {
            assert!(!tr.en.is_empty() && !tr.zh.is_empty(), "{}", t.id);
            assert!(tr.en.lines().count() <= 3 && tr.zh.lines().count() <= 3);
        }
        let public = (t.cases)();
        let hidden = (t.hidden)();
        assert!((2..=3).contains(&public.len()) && (1..=3).contains(&hidden.len()));
        for h in &hidden {
            assert!(public.iter().all(|p| p.rom != h.rom), "{} hidden", t.id);
        }
        for c in public.iter().chain(&hidden) {
            assert_eq!((t.expect)(c).len(), t.out_len, "{} expect", t.id);
        }
        assert!(o.marks.is_sorted(), "{} marks", t.id);
    }
    for (o, s) in refs {
        assert_eq!(s, o.marks.first().copied(), "{} reference", o.task.id);
    }
    got.into_iter()
        .map(|(o, s)| {
            let s = s.unwrap_or_else(|| panic!("{} alternative fails", o.task.id));
            assert!(o.marks.contains(&s), "{}: {s} not in {:?}", o.task.id, o.marks);
            s
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_9_alternatives_pass_and_score_as_marked() {
        check(
            9,
            &problems(),
            &[
                ("soft-focus", focus_slide(false)),
                ("soft-focus", focus_plain()),
                ("colour-table", table_src(false)),
                ("colour-table", TABLE_GATHER),
                ("magnifier", MAG_LERP),
            ],
        );
    }
}
