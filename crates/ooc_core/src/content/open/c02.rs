//! Chapter 2 open problems: Douya's arcade cabinet and the cartridge.

use oo_base::Tr;

use super::Open;
use crate::content::{picture, screen, stub};
use crate::isa::{Alu, FB_LEN, OpKind};
use crate::task::*;
use crate::vm::Launch;

const fn alu(a: Alu) -> OpKind {
    OpKind::Alu(a)
}

/// What chapter 2 has unlocked: no LOOP and no IF yet.
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
    OpKind::Ld,
    OpKind::St,
    OpKind::Pix,
];

fn base(id: &'static str, slot: u8) -> Task {
    stub(
        id,
        2,
        slot,
        View::Screen,
        Editor::Code(OPS),
        Launch::SCREEN,
        FB_LEN,
    )
}

pub fn problems() -> Vec<Open> {
    vec![beam_sweep(), high_score(), quarter_turn()]
}

/// The beam's eight directions, clockwise from east: x steps, then y steps.
const DIR_X: [i32; 8] = [1, 1, 0, -1, -1, -1, 0, 1];
const DIR_Y: [i32; 8] = [0, 1, 1, 1, 0, -1, -1, -1];

fn beam_rom(frame: i32) -> Case {
    Case {
        rom: DIR_X.iter().chain(&DIR_Y).copied().collect(),
        frame,
    }
}

/// Whether (x, y) lies on the beam: on the ray from (8, 8) in direction CLOCK AND 7.
fn on_beam(frame: i32, x: i32, y: i32) -> bool {
    let k = (frame & 7) as usize;
    let (dx, dy) = (x - 8, y - 8);
    dx * DIR_Y[k] == dy * DIR_X[k] && dx * DIR_X[k] + dy * DIR_Y[k] >= 0
}

fn beam_sweep() -> Open {
    Open {
        task: Task {
            title: Tr::new("The Wind Tower Beam", "风塔的光束"),
            brief: Tr::new(
                "The cabinet's attract screen shows the wind tower's beam: a ray from (8,8) that turns 45° each frame.\nIts direction is (rom[k], rom[8 + k]) with k = CLOCK AND 7. Light the ray, centre included, in colour 14.\nScore: instructions in your program. Fewer is better.",
                "街机的待机画面上是风塔的光束：一条从 (8,8) 射出的射线，每帧转 45°。\n方向是 (rom[k], rom[8 + k])，k = CLOCK AND 7。把射线（含中心）点成颜色 14。\n得分：程序的指令条数，越少越好。",
            ),
            cases: || [0, 1, 2].map(beam_rom).to_vec(),
            hidden: || [5, 11, 23].map(beam_rom).to_vec(),
            expect: |c| screen(|x, y| if on_beam(c.frame, x, y) { 14 } else { 0 }),
            reference: BEAM_TWO_SEL,
            ..base("beam-sweep", 9)
        },
        metric: Metric::Size,
        score: None,
        score_label: None,
        marks: &[16, 18],
    }
}

/// On the line (cross product 0) and in front (dot product ≥ 0): two SELs.
const BEAM_TWO_SEL: Source = Source::Code(
    "AND r0, frame, 7\nLD r1, [r0]\nADD r0, r0, 8\nLD r2, [r0]\nSUB r3, x, 8\nSUB r4, y, 8\nMUL r5, r3, r2\nMUL r6, r4, r1\nSETP.EQ p0, r5, r6\nMUL r3, r3, r1\nMUL r4, r4, r2\nADD r3, r3, r4\nSETP.GE p1, r3, 0\nSEL r7, p0, 14, 0\nSEL r7, p1, r7, 0\nPIX r7\n",
);

/// Folds both tests into one number: MIN(dot, 0) − cross² is 0 only on the ray.
#[cfg(test)]
const BEAM_FOLD: Source = Source::Code(
    "AND r0, frame, 7\nLD r1, [r0]\nADD r0, r0, 8\nLD r2, [r0]\nSUB r3, x, 8\nSUB r4, y, 8\nMUL r5, r3, r2\nMUL r6, r4, r1\nSUB r5, r5, r6\nMUL r5, r5, r5\nMUL r3, r3, r1\nMUL r4, r4, r2\nADD r3, r3, r4\nMIN r3, r3, 0\nSUB r3, r3, r5\nSETP.EQ p0, r3, 0\nSEL r7, p0, 14, 0\nPIX r7\n",
);

/// 3×5 digits, one row per word; bit c is column c, counted from the left.
const FONT: [[i32; 5]; 10] = [
    [7, 5, 5, 5, 7],
    [2, 3, 2, 2, 7],
    [7, 4, 7, 1, 7],
    [7, 4, 7, 4, 7],
    [5, 5, 7, 4, 4],
    [7, 1, 7, 4, 7],
    [7, 1, 7, 5, 7],
    [7, 4, 4, 4, 4],
    [7, 5, 7, 5, 7],
    [7, 5, 7, 4, 7],
];

/// rom[0], rom[1]: the two digits; digit d's rows at rom[8 + 8d ..], padded with zeros.
fn score_rom(score: i32) -> Case {
    let mut rom = vec![0; 88];
    rom[0] = score / 10;
    rom[1] = score % 10;
    for (d, rows) in FONT.iter().enumerate() {
        rom[8 + 8 * d..13 + 8 * d].copy_from_slice(rows);
    }
    Case { rom, frame: 0 }
}

/// Each digit is drawn twice the font's size: tens in x 0..8, units in x 8..16, rows 3..13.
fn digit_pixel(rom: &[i32], x: i32, y: i32) -> i32 {
    if !(3..13).contains(&y) {
        return 0;
    }
    let d = rom[(x >> 3) as usize] as usize;
    let w = FONT[d][((y - 3) >> 1) as usize];
    ((w >> ((x >> 1) & 3)) & 1) * 11
}

fn high_score() -> Open {
    Open {
        task: Task {
            title: Tr::new("High Score", "最高分"),
            brief: Tr::new(
                "Douya wants the high score on the cabinet: tens digit rom[0], units rom[1], drawn at twice the font size in colour 11.\nDigit d's five rows are rom[8 + 8d + row] (bit c = column c, from the left; rows 5 to 7 are 0). Tens fill x 0..7, units x 8..15, both on y 3..12.\nScore: instructions in your program. Fewer is better.",
                "豆芽想把最高分显示在街机上：十位是 rom[0]，个位是 rom[1]，按字体两倍大小画成颜色 11。\n数字 d 的五行在 rom[8 + 8d + 行]（第 c 位 = 从左数第 c 列；第 5 到 7 行为 0）。十位占 x 0..7，个位占 x 8..15，都在 y 3..12。\n得分：程序的指令条数，越少越好。",
            ),
            cases: || [42, 7, 90].map(score_rom).to_vec(),
            hidden: || [13, 68, 85].map(score_rom).to_vec(),
            expect: |c| screen(|x, y| digit_pixel(&c.rom, x, y)),
            reference: SCORE_FOLDED,
            ..base("high-score", 10)
        },
        metric: Metric::Size,
        score: None,
        score_label: None,
        marks: &[13, 14, 18],
    }
}

/// One address for digit and row together: (16d + y + 13) SHR 1. The padding
/// rows read 0 above and below the digits, so no range test is needed.
const SCORE_FOLDED: Source = Source::Code(
    "SHR r0, x, 3\nLD r0, [r0]\nSHL r0, r0, 4\nADD r0, r0, y\nADD r0, r0, 13\nSHR r0, r0, 1\nLD r0, [r0]\nSHR r1, x, 1\nAND r1, r1, 3\nSHR r0, r0, r1\nAND r0, r0, 1\nMUL r0, r0, 11\nPIX r0\n",
);

/// Digit address and row computed apart; still relies on the padding.
#[cfg(test)]
const SCORE_PADDED: Source = Source::Code(
    "SHR r0, x, 3\nLD r0, [r0]\nSHL r0, r0, 3\nADD r0, r0, 8\nSUB r1, y, 3\nSHR r1, r1, 1\nADD r0, r0, r1\nLD r0, [r0]\nSHR r1, x, 1\nAND r1, r1, 3\nSHR r0, r0, r1\nAND r0, r0, 1\nMUL r0, r0, 11\nPIX r0\n",
);

/// Tests the row range explicitly with two SELs.
#[cfg(test)]
const SCORE_RANGE: Source = Source::Code(
    "SHR r0, x, 3\nLD r0, [r0]\nSHL r0, r0, 3\nADD r0, r0, 8\nSUB r1, y, 3\nSHR r2, r1, 1\nADD r0, r0, r2\nLD r0, [r0]\nSHR r3, x, 1\nAND r3, r3, 3\nSHR r0, r0, r3\nAND r0, r0, 1\nMUL r0, r0, 11\nSETP.GE p0, r1, 0\nSEL r0, p0, r0, 0\nSETP.LT p0, r1, 10\nSEL r0, p0, r0, 0\nPIX r0\n",
);

/// The 16×16 picture at rom[0..256] and the turn count at rom[256].
fn turn_rom(seed: u64, k: i32) -> Case {
    let mut rom = picture(seed, 16, 16);
    rom.push(k);
    Case { rom, frame: 0 }
}

/// The picture turned clockwise k quarter turns.
fn turned(rom: &[i32], x: i32, y: i32) -> i32 {
    let (u, v) = match rom[256] & 3 {
        0 => (x, y),
        1 => (y, 15 - x),
        2 => (15 - x, 15 - y),
        _ => (15 - y, x),
    };
    rom[(v * 16 + u) as usize]
}

fn quarter_turn() -> Open {
    Open {
        task: Task {
            title: Tr::new("Quarter Turns", "转个直角"),
            brief: Tr::new(
                "The cabinet is being mounted on its side. Show the 16×16 picture in rom[0..256], turned clockwise rom[256] quarter turns (0 to 3).\nAfter one turn, the picture's top-left pixel sits at the top right of the screen.\nScore: instructions in your program. Fewer is better.",
                "街机要侧过来装。把 rom[0..256] 里 16×16 的图顺时针转 rom[256] 个直角（0 到 3）再显示。\n转一次之后，图的左上角像素在屏幕右上角。\n得分：程序的指令条数，越少越好。",
            ),
            cases: || vec![turn_rom(31, 1), turn_rom(32, 2), turn_rom(33, 3)],
            hidden: || vec![turn_rom(34, 0), turn_rom(35, 3), turn_rom(36, 1)],
            expect: |c| screen(|x, y| turned(&c.rom, x, y)),
            reference: TURN_MASK,
            ..base("quarter-turn", 11)
        },
        metric: Metric::Size,
        score: None,
        score_label: None,
        marks: &[14, 16, 17],
    }
}

/// Odd turns swap x and y; then one XOR flips the right nibbles of the address.
const TURN_MASK: Source = Source::Code(
    "LD r0, [256]\nAND r1, r0, 1\nSETP.NE p0, r1, 0\nSHL r2, x, 4\nADD r2, r2, y\nSEL r2, p0, r2, tid\nSHR r3, r0, 1\nXOR r4, r3, r1\nMUL r4, r4, 240\nMUL r3, r3, 15\nADD r3, r3, r4\nXOR r2, r2, r3\nLD r2, [r2]\nPIX r2\n",
);

/// Swap, then flip u and v separately with XOR 15.
#[cfg(test)]
const TURN_UV: Source = Source::Code(
    "LD r0, [256]\nAND r1, r0, 1\nSETP.NE p0, r1, 0\nSEL r2, p0, y, x\nSEL r3, p0, x, y\nSHR r4, r0, 1\nMUL r5, r4, 15\nXOR r2, r2, r5\nXOR r4, r4, r0\nAND r4, r4, 1\nMUL r4, r4, 15\nXOR r3, r3, r4\nSHL r3, r3, 4\nADD r3, r3, r2\nLD r3, [r3]\nPIX r3\n",
);

/// All four addresses, then a SEL per turn.
#[cfg(test)]
const TURN_ALL: Source = Source::Code(
    "LD r0, [256]\nXOR r1, x, 15\nXOR r2, y, 15\nSHL r3, r1, 4\nADD r3, r3, y\nSHL r4, r2, 4\nADD r4, r4, r1\nSHL r5, x, 4\nADD r5, r5, r2\nSETP.EQ p0, r0, 1\nSEL r6, p0, r3, tid\nSETP.EQ p0, r0, 2\nSEL r6, p0, r4, r6\nSETP.EQ p0, r0, 3\nSEL r6, p0, r5, r6\nLD r6, [r6]\nPIX r6\n",
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grade::grade;

    #[test]
    fn alternatives_pass_and_score_on_marks() {
        let alts: [&[Source]; 3] = [
            &[BEAM_FOLD],
            &[SCORE_PADDED, SCORE_RANGE],
            &[TURN_UV, TURN_ALL],
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
