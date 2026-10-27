//! Chapter 7 · Lyra: precision traded for bandwidth.

use oo_base::{Rng, Tr};

use super::lyra::{LYRA_OPS, base as lbase};
use super::{SCALAR_OPS, below, rng, screen, stub};
use crate::fp::{self, f32_bits, h_to_f64};
use crate::isa::{Alu, Cvt, FB_LEN, OpKind};
use crate::task::*;
use crate::vm::Launch;

fn base(id: &'static str, slot: u8) -> Task {
    stub(
        id,
        7,
        slot,
        View::Screen,
        Editor::Code(SCALAR_OPS),
        Launch::SCREEN,
        FB_LEN,
    )
}

pub fn tasks() -> Vec<Task> {
    vec![
        the_float(),
        half(),
        the_absorb(),
        overflow(),
        the_int8(),
        the_palette(),
        the_clipped(),
        the_sorter(),
    ]
}

/// rom[0..16]: one packed row each, 2 bits per pixel (pixel x in bits 2x..2x+1);
/// rom[16..20]: the four-colour palette.
fn packed(seed: u64) -> Vec<i32> {
    let mut r = rng(seed);
    let mut rom: Vec<i32> = (0..16).map(|_| r.next_u64() as u32 as i32).collect();
    let mut pal = [0, 0, 0, 0];
    for p in &mut pal {
        *p = 1 + below(&mut r, 15);
    }
    rom.extend(pal);
    rom
}

fn the_palette() -> Task {
    Task {
        title: Tr::new("The Palette", "调色板"),
        brief: Tr::new(
            "Memory was dear: this cabinet packs a whole row into one number, 2 bits per pixel.\nPixel x of row y is bits 2x and 2x+1 of rom[y] — a number 0..3.\nThat number picks a colour from the palette at rom[16..20]. Unpack the screen.",
            "那时内存很贵：这台机器把一整行塞进一个数里，每像素 2 位。\n第 y 行的像素 x 是 rom[y] 的第 2x 和 2x+1 位——一个 0..3 的数。\n这个数再从 rom[16..20] 的调色板里挑颜色。把屏幕解包出来。",
        ),
        cases: || {
            [51, 52, 53]
                .map(|s| Case {
                    rom: packed(s),
                    frame: 0,
                })
                .to_vec()
        },
        hidden: || {
            [54, 55]
                .map(|s| Case {
                    rom: packed(s),
                    frame: 0,
                })
                .to_vec()
        },
        expect: |c| screen(|x, y| c.rom[(16 + ((c.rom[y as usize] >> (2 * x)) & 3)) as usize]),
        goals: [Goal::new(Metric::Cycles, 56), Goal::new(Metric::Size, 7)],
        starter: Source::Code(
            "LD r0, [y]\nSHR r0, r0, x\nAND r0, r0, 3\nADD r0, r0, 16\nLD r0, [r0]\nPIX r0\n",
        ),
        reference: Source::Code(
            "LD r0, [y]\nSHL r1, x, 1\nSHR r0, r0, r1\nAND r0, r0, 3\nADD r0, r0, 16\nLD r0, [r0]\nPIX r0\n",
        ),
        predict: Predict {
            question: Tr::new(
                "16 colours need 4 bits per pixel. How much smaller is a 2-bit screen plus a 4-entry palette?",
                "16 种颜色每像素要 4 位。2 位画面加 4 色调色板能小多少？",
            ),
            options: [
                Tr::new("About half", "大约一半"),
                Tr::new("About a quarter", "大约四分之一"),
                Tr::new(
                    "The same — the palette eats the savings",
                    "一样大——调色板吃掉了节省",
                ),
            ],
            answer: 0,
            why: Tr::new(
                "512 bits instead of 1024, plus four tiny palette entries. Fewer bits per pixel, fewer colours on screen at once.",
                "512 位对 1024 位，外加四个小小的调色板条目。每像素位数少了，同屏颜色也少了。",
            ),
        },
        hints: [
            Tr::new(
                "The starter's colours shift and stripe. Is it reading the right bits for each x?",
                "初始代码的颜色错位、出现条纹。它为每个 x 读的位对吗？",
            ),
            Tr::new(
                "Each pixel is 2 bits wide, so pixel x starts at bit 2x, not bit x.",
                "每个像素宽 2 位，所以像素 x 从第 2x 位开始，而不是第 x 位。",
            ),
            Tr::new(
                "Add SHL r1, x, 1 after the first LD and shift by r1 instead of x.",
                "在第一个 LD 之后加 SHL r1, x, 1，然后改为按 r1 右移，而不是 x。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Packing and palettes", "打包与调色板"),
            line: Tr::new(
                "Shift, mask, look up: the three moves that turned tiny memories into colourful screens.",
                "移位、掩码、查表：三步就把小小的内存变成了多彩的屏幕。",
            ),
            reality: Tr::new(
                "Texture compression (BC, ASTC) still packs pixels into a few bits plus a small colour table per block.",
                "纹理压缩（BC、ASTC）至今仍是把像素压成几位，再给每个块配一张小色表。",
            ),
            myth: Tr::new(
                "Myth: palettes are obsolete. Swap four entries and every sprite changes colour — games still do it.",
                "误解：调色板早已过时。换掉四个条目，所有精灵就都换了颜色——游戏至今还在这么做。",
            ),
            simplified: Tr::new(
                "Real formats decompress in fixed-function hardware, not in your kernel.",
                "真实的格式由固定功能硬件解压，而不是在你的内核里。",
            ),
        },
        ..base("the-palette", 4)
    }
}

/// Uniform real in `lo..hi`.
fn unif(r: &mut Rng, lo: f64, hi: f64) -> f64 {
    lo + (hi - lo) * below(r, 1 << 20) as f64 / (1 << 20) as f64
}

fn case(rom: Vec<i32>) -> Case {
    Case { rom, frame: 0 }
}

/// Integer path only: the FP units are still locked.
const INT_OPS: &[OpKind] = &[
    OpKind::Mov,
    OpKind::Alu(Alu::Add),
    OpKind::Alu(Alu::Sub),
    OpKind::Alu(Alu::Mul),
    OpKind::Alu(Alu::And),
    OpKind::Alu(Alu::Or),
    OpKind::Alu(Alu::Xor),
    OpKind::Alu(Alu::Shl),
    OpKind::Alu(Alu::Shr),
    OpKind::Alu(Alu::Min),
    OpKind::Alu(Alu::Max),
    OpKind::SetP,
    OpKind::Sel,
    OpKind::If,
    OpKind::Else,
    OpKind::EndIf,
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::Ld,
    OpKind::St,
];

// ---------------------------------------------------------------- 14-1

/// One FP16 per lane: sign, exponent range and mantissa all vary.
fn halves(seed: u64, e_lo: i32, e_hi: i32, signed: bool) -> Case {
    let mut r = rng(seed);
    case(
        (0..32)
            .map(|_| {
                let e = e_lo + below(&mut r, e_hi - e_lo + 1);
                let m = below(&mut r, 1024);
                (if signed { below(&mut r, 2) << 15 } else { 0 }) | (e << 10) | m
            })
            .collect(),
    )
}

fn float_cases() -> Vec<Case> {
    vec![
        halves(1401, 15, 15, false),
        halves(1402, 11, 25, true),
        case((1..=32).map(|i| fp::h(i as f64)).collect()),
    ]
}

fn float_hidden() -> Vec<Case> {
    let mut zero = halves(1403, 0, 0, true);
    zero.rom[..4].copy_from_slice(&[0, 0x8000, 1, 0x83ff]);
    let mut top = halves(1404, 29, 30, true);
    top.rom[..2].copy_from_slice(&[0x7bff, 0xfbff]);
    vec![zero, top, halves(1405, 6, 8, true)]
}

fn float_expect(c: &Case) -> Vec<Option<i32>> {
    c.rom
        .iter()
        .map(|&b| Some((h_to_f64(b) * 256.0).trunc() as i32))
        .collect()
}

const FLOAT_STARTER: &str = "; reads the 16 bits as a fixed-point number | 把 16 位当成定点数来读\n\
    LD r0, [lane]\n\
    SHR r0, r0, 2\n\
    ST [lane], r0\n";

const FLOAT_REF: &str = "LD r0, [lane]\n\
    SHR r1, r0, 10\n\
    AND r1, r1, 31\n\
    MAX r1, r1, 6\n\
    AND r2, r0, 1023\n\
    OR r2, r2, 1024\n\
    SHL r2, r2, 20\n\
    SUB r1, 37, r1\n\
    SHR r2, r2, r1\n\
    SHL r3, r0, 16\n\
    SHR r3, r3, 31\n\
    XOR r2, r2, r3\n\
    SUB r2, r2, r3\n\
    ST [lane], r2\n";

fn the_float() -> Task {
    Task {
        title: Tr::new("The Float", "浮点"),
        brief: Tr::new(
            "Cenli's echo stones keep every reading as a half float: 16 bits, sign s (bit 15), exponent e (bits 10..14), mantissa m (bits 0..9). For e = 1..30 the value is (-1)^s · (1 + m/1024) · 2^(e-15). With e = 0 the value is tiny (a subnormal), well under 1/256.\nrom[0..32] holds one half per lane, in the low 16 bits of the word. Write out[lane] = value × 256, cut toward zero to an integer.\nThe FP units are still locked: decode it with integer instructions.",
            "岑理的回响石把每个读数存成半精度浮点：16 位，符号 s（第 15 位）、指数 e（第 10..14 位）、尾数 m（第 0..9 位）。e = 1..30 时值为 (-1)^s · (1 + m/1024) · 2^(e-15)。e = 0 时值极小（次正规数），远小于 1/256。\nrom[0..32] 每条车道一个半精度数，放在字的低 16 位。写出 out[lane] = 值 × 256，向零取整。\n浮点单元还没解锁：用整数指令把它拆开。",
        ),
        cases: float_cases,
        hidden: float_hidden,
        expect: float_expect,
        goals: [Goal::new(Metric::Cycles, 420), Goal::new(Metric::Size, 14)],
        starter: Source::Code(FLOAT_STARTER),
        reference: Source::Code(FLOAT_REF),
        predict: Predict {
            question: Tr::new(
                "A half float has a 10-bit mantissa. Between 1024 and 2048, how far apart are neighbouring halves?",
                "半精度浮点的尾数有 10 位。在 1024 到 2048 之间，相邻的两个半精度数相差多少？",
            ),
            options: [
                Tr::new("1/1024", "1/1024"),
                Tr::new("1", "1"),
                Tr::new("16", "16"),
            ],
            answer: 1,
            why: Tr::new(
                "Values in [1024, 2048) have exponent 10, so the last mantissa bit is worth 2^(10-10) = 1. Past 2048 the gap is 2 and odd integers are gone.",
                "[1024, 2048) 里的数指数是 10，尾数最低位的权重是 2^(10-10) = 1。过了 2048，间隔变成 2，奇数就没有了。",
            ),
        },
        hints: [
            Tr::new(
                "The starter treats the 16 bits as one fixed-point number. Pull the three fields apart first: SHR, then AND.",
                "初始代码把 16 位当成一个定点数。先把三个字段拆开：SHR，再 AND。",
            ),
            Tr::new(
                "value × 256 = (1024 + m) · 2^(e-17). Shift left when e > 17, right when e < 17. Tiny exponents just give 0.",
                "值 × 256 = (1024 + m) · 2^(e-17)。e > 17 时左移，e < 17 时右移。指数很小时结果就是 0。",
            ),
            Tr::new(
                "One SHR is enough: put 1024 + m at bit 20, then shift right by 37 - max(e, 6). Negate with a sign mask: XOR, then SUB the mask.",
                "一次 SHR 就够：把 1024 + m 放到第 20 位，再右移 37 - max(e, 6)。用符号掩码取负：先 XOR，再减掉掩码。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Floating point is bits", "浮点就是位"),
            line: Tr::new(
                "A half float is scientific notation in 16 bits: a sign, a power of two and a short mantissa.",
                "半精度浮点就是 16 位的科学计数法：一个符号、一个 2 的幂、一小段尾数。",
            ),
            reality: Tr::new(
                "IEEE 754 binary16 is exactly this layout. GPUs, phones and neural-net accelerators use it to halve memory traffic.",
                "IEEE 754 的 binary16 正是这个布局。GPU、手机和神经网络加速器用它把访存量减半。",
            ),
            myth: Tr::new(
                "Myth: floats are fuzzy decimals. They are exact binary fractions; 0.1 just is not one of them.",
                "误解：浮点数是模糊的小数。它们是精确的二进制分数，只是 0.1 恰好不在其中。",
            ),
            simplified: Tr::new(
                "OC-1 rounds to nearest-even only and keeps no exception flags.",
                "OC-1 只做就近舍入（偶数优先），不保留异常标志。",
            ),
        },
        ..lbase("the-float", 14, 1, Launch::ONE_WARP, 32, INT_OPS)
    }
}

// ---------------------------------------------------------------- 14-2

/// Scalar FP path: halves in, FP32 sums, no tensor unit yet.
const FP_OPS: &[OpKind] = &[
    OpKind::Mov,
    OpKind::Alu(Alu::Add),
    OpKind::Alu(Alu::Sub),
    OpKind::Alu(Alu::Mul),
    OpKind::Alu(Alu::And),
    OpKind::Alu(Alu::Or),
    OpKind::Alu(Alu::Xor),
    OpKind::Alu(Alu::Shl),
    OpKind::Alu(Alu::Shr),
    OpKind::Alu(Alu::HAdd),
    OpKind::Alu(Alu::HMul),
    OpKind::Alu(Alu::FAdd),
    OpKind::Alu(Alu::FMul),
    OpKind::Cvt(Cvt::FH),
    OpKind::Cvt(Cvt::HF),
    OpKind::Cvt(Cvt::FI),
    OpKind::Cvt(Cvt::IF),
    OpKind::SetP,
    OpKind::Sel,
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::Ld,
    OpKind::St,
];

const DOT_N: usize = 256;
const DOT_X: usize = 32 + 32 * DOT_N;

/// rom[0]: output scale (FP32); rom[32 + 32k + lane]: W[lane][k];
/// rom[DOT_X + k]: x[k]. Halves everywhere. The scale puts the largest
/// |y| of the case at 4096; lanes from `small_from` on get small weights.
fn dot_case(seed: u64, wr: (f64, f64), xr: (f64, f64), small_from: usize) -> Case {
    let mut r = rng(seed);
    let mut rom = vec![0; DOT_X + DOT_N];
    for k in 0..DOT_N {
        for lane in 0..32 {
            rom[32 + 32 * k + lane] = fp::h(unif(&mut r, wr.0, wr.1));
        }
        rom[DOT_X + k] = fp::h(unif(&mut r, xr.0, xr.1));
    }
    // Lanes from `small_from` on hold small weights (0 = none).
    if small_from > 0 {
        for k in 0..DOT_N {
            for lane in small_from..32 {
                rom[32 + 32 * k + lane] = fp::h(unif(&mut r, 0.01, 0.1));
            }
        }
    }
    let ymax = dot_y(&rom).iter().fold(0f64, |m, y| m.max(y.abs()));
    rom[0] = f32_bits((4096.0 / ymax) as f32);
    case(rom)
}

fn dot_y(rom: &[i32]) -> Vec<f64> {
    (0..32)
        .map(|lane| {
            (0..DOT_N)
                .map(|k| h_to_f64(rom[32 + 32 * k + lane]) * h_to_f64(rom[DOT_X + k]))
                .sum()
        })
        .collect()
}

fn dot_expect(c: &Case) -> Vec<Option<i32>> {
    let s = fp::f_to_f32(c.rom[0]) as f64;
    dot_y(&c.rom)
        .iter()
        .map(|y| Some((y * s).round_ties_even() as i32))
        .collect()
}

const HALF_STARTER: &str = "LD r30, [0]\n\
    ADD r1, lane, 32\n\
    MOV r2, 8224\n\
    MOV r3, 0\n\
    LOOP 16\n\
    LOOP 16\n\
    LD r4, [r1]\n\
    LD r5, [r2]\n\
    HMUL r4, r4, r5\n\
    HADD r3, r3, r4 ; the running sum stays in FP16 | 累加和一直留在 FP16\n\
    ADD r1, r1, 32\n\
    ADD r2, r2, 1\n\
    ENDLOOP\n\
    ENDLOOP\n\
    CVT.F.H r3, r3\n\
    FMUL r3, r3, r30\n\
    CVT.I.F r3, r3\n\
    ST [lane], r3\n";

/// The starter with a wide accumulator: right, but one load at a time.
#[cfg(test)]
const HALF_SLOW: &str = "LD r30, [0]\n\
    ADD r1, lane, 32\n\
    MOV r2, 8224\n\
    MOV r3, 0\n\
    LOOP 16\n\
    LOOP 16\n\
    LD r4, [r1]\n\
    LD r5, [r2]\n\
    HMUL r4, r4, r5\n\
    CVT.F.H r4, r4\n\
    FADD r3, r3, r4\n\
    ADD r1, r1, 32\n\
    ADD r2, r2, 1\n\
    ENDLOOP\n\
    ENDLOOP\n\
    FMUL r3, r3, r30\n\
    CVT.I.F r3, r3\n\
    ST [lane], r3\n";

fn half_ref() -> String {
    let mut s = String::from("LD r30, [0]\nADD r1, lane, 32\nMOV r2, 8224\nMOV r3, 0\nLOOP 32\n");
    for i in 0..8 {
        s += &format!("LD r{}, [r1]\nADD r1, r1, 32\n", 4 + i);
    }
    for i in 0..8 {
        s += &format!("LD r{}, [r2]\nADD r2, r2, 1\n", 12 + i);
    }
    for i in 0..8 {
        s += &format!(
            "HMUL r{a}, r{a}, r{b}\nCVT.F.H r{a}, r{a}\nFADD r3, r3, r{a}\n",
            a = 4 + i,
            b = 12 + i
        );
    }
    s += "ENDLOOP\nFMUL r3, r3, r30\nCVT.I.F r3, r3\nST [lane], r3\n";
    s
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn half() -> Task {
    Task {
        title: Tr::new("Half", "半精度"),
        brief: Tr::new(
            "Cenli's deep-well probe mixes 256 readings per lane: y[lane] = sum over k of W[k][lane] · x[k]. Everything is stored as halves: W[k][lane] at rom[32 + 32k + lane], x[k] at rom[8224 + k].\nrom[0] is a scale s (FP32). Write out[lane] = round(y · s). The host picked s so the largest |y| of each case lands near 4096, so an error of 4 in out is about 0.1% of full scale.\nHalves are cheap to fetch but carry only 11 significant bits. Decide where each product and each sum is allowed to live.",
            "岑理的深井探头每条车道要混合 256 个读数：y[lane] = 对 k 求和 W[k][lane] · x[k]。全部以半精度存放：W[k][lane] 在 rom[32 + 32k + lane]，x[k] 在 rom[8224 + k]。\nrom[0] 是缩放 s（FP32）。写出 out[lane] = round(y · s)。主机挑 s，让每组数据里最大的 |y| 落在 4096 附近，所以 out 差 4 大约是满量程的 0.1%。\n半精度取数便宜，但只有 11 位有效数字。想清楚每个乘积、每个和该放在哪种精度里。",
        ),
        cases: || {
            vec![
                dot_case(1411, (-2.0, 2.0), (-2.0, 2.0), 0),
                dot_case(1412, (-2.0, 2.0), (-2.0, 2.0), 0),
                dot_case(1413, (-2.0, 2.0), (-2.0, 2.0), 0),
            ]
        },
        hidden: || {
            vec![
                dot_case(1414, (20.0, 60.0), (20.0, 60.0), 0),
                dot_case(1415, (-60.0, 60.0), (-60.0, 60.0), 0),
                dot_case(1416, (20.0, 60.0), (20.0, 60.0), 16),
            ]
        },
        expect: dot_expect,
        tolerance: 4,
        goals: [Goal::new(Metric::Cycles, 18000), Goal::new(Metric::Err, 2)],
        starter: Source::Code(HALF_STARTER),
        reference: Source::Code(leak(half_ref())),
        predict: Predict {
            question: Tr::new(
                "A running half-precision sum has reached 1000. You add 0.2. What is the sum afterwards?",
                "一个半精度的累加和已经到了 1000。再加 0.2，和变成多少？",
            ),
            options: [
                Tr::new("1000.2", "1000.2"),
                Tr::new("1000", "1000"),
                Tr::new("1000.5", "1000.5"),
            ],
            answer: 1,
            why: Tr::new(
                "Between 512 and 1024 neighbouring halves are 0.5 apart. 0.2 is less than half a step, so the add rounds back to 1000. Do that 200 times and the sum has not moved.",
                "512 到 1024 之间相邻的半精度数相隔 0.5。0.2 不到半格，所以这次加法又舍回 1000。这样加 200 次，和一点没动。",
            ),
        },
        hints: [
            Tr::new(
                "The starter keeps its running sum in FP16. Once the sum is large, small terms fall below half a step and vanish; every lane drifts.",
                "初始代码把累加和留在 FP16 里。和一大，小项就不到半格，被舍掉；每条车道都在漂。",
            ),
            Tr::new(
                "Multiply in FP16 (HMUL), widen the product with CVT.F.H, add in FP32 (FADD). Rounding each product once costs far less than rounding every partial sum.",
                "乘法用 FP16（HMUL），用 CVT.F.H 把乘积拓宽，再用 FP32 加（FADD）。每个乘积只舍入一次，比每个部分和都舍入便宜得多。",
            ),
            Tr::new(
                "A load waits 400 cycles. Issue eight loads of W and eight of x before you use any, then do the eight multiply-adds: 32 passes instead of 256.",
                "一次取数要等 400 个周期。先发八个 W 和八个 x 的取数，再做这八次乘加：32 趟而不是 256 趟。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Multiply narrow, accumulate wide", "窄乘宽加"),
            line: Tr::new(
                "Inputs can be half precision. The running sum must not be.",
                "输入可以是半精度，累加和不行。",
            ),
            reality: Tr::new(
                "Tensor cores and TPUs multiply FP16 or BF16 inputs but accumulate in FP32, for exactly this reason.",
                "张量核心和 TPU 用 FP16 或 BF16 做乘法，却用 FP32 累加，原因就在这里。",
            ),
            myth: Tr::new(
                "Myth: half-precision inputs make a half-precision sum good enough. The sum rounds 256 times; the inputs round once.",
                "误解：输入是半精度，半精度求和就够了。输入只舍入一次，求和要舍入 256 次。",
            ),
            simplified: Tr::new(
                "OC-1 rounds every operation to nearest-even and never fuses or reorders.",
                "OC-1 每步都就近舍入（偶数优先），不融合也不重排。",
            ),
        },
        ..lbase("half", 14, 2, Launch::ONE_WARP, 32, FP_OPS)
    }
}

// ---------------------------------------------------------------- 14-3

/// Only FP16 addition: no FP32 accumulator on this bench.
const ABSORB_OPS: &[OpKind] = &[
    OpKind::Mov,
    OpKind::Alu(Alu::Add),
    OpKind::Alu(Alu::Sub),
    OpKind::Alu(Alu::Mul),
    OpKind::Alu(Alu::And),
    OpKind::Alu(Alu::Or),
    OpKind::Alu(Alu::Xor),
    OpKind::Alu(Alu::Shl),
    OpKind::Alu(Alu::Shr),
    OpKind::Alu(Alu::HAdd),
    OpKind::Cvt(Cvt::IH),
    OpKind::Cvt(Cvt::HI),
    OpKind::SetP,
    OpKind::Sel,
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::Ld,
    OpKind::St,
    OpKind::Lds,
    OpKind::Sts,
    OpKind::Ldl,
    OpKind::Stl,
    OpKind::Bar,
    OpKind::Dma,
];

const ABSORB_N: usize = 64;

/// vals[lane][k] as halves at rom[32k + lane].
fn absorb_case(f: impl Fn(&mut Rng, usize, usize) -> f64, seed: u64) -> Case {
    let mut r = rng(seed);
    let mut rom = vec![0; 32 * ABSORB_N];
    for lane in 0..32 {
        for k in 0..ABSORB_N {
            rom[32 * k + lane] = fp::h(f(&mut r, lane, k));
        }
    }
    case(rom)
}

fn absorb_expect(c: &Case) -> Vec<Option<i32>> {
    (0..32)
        .map(|lane| {
            let y: f64 = (0..ABSORB_N).map(|k| h_to_f64(c.rom[32 * k + lane])).sum();
            Some(y.round_ties_even() as i32)
        })
        .collect()
}

/// One 2048 at `pos`, 63 ones elsewhere.
fn big_at(pos: usize) -> Case {
    absorb_case(|_, _, k| if k == pos { 2048.0 } else { 1.0 }, pos as u64)
}

fn absorb_cases() -> Vec<Case> {
    vec![big_at(0), big_at(32), big_at(63)]
}

fn absorb_hidden() -> Vec<Case> {
    vec![
        absorb_case(|_, _, k| if k == 0 || k == 63 { 2048.0 } else { 1.0 }, 1431),
        absorb_case(|_, _, k| if k == 0 { 4096.0 } else { 0.5 }, 1432),
        {
            let mut pos = rng(1433);
            let at: Vec<usize> = (0..32).map(|_| below(&mut pos, 64) as usize).collect();
            absorb_case(
                move |r, lane, k| {
                    if k == at[lane] {
                        2048.0 + below(r, 1024) as f64 * 2.0
                    } else {
                        0.5 * (1 + below(r, 4)) as f64
                    }
                },
                1434,
            )
        },
    ]
}

const ABSORB_STARTER: &str = "; one running sum, front to back | 一个累加和，从头加到尾\n\
    MOV r0, lane\n\
    MOV r1, 0\n\
    LOOP 64\n\
    LD r2, [r0]\n\
    HADD r1, r1, r2\n\
    ADD r0, r0, 32\n\
    ENDLOOP\n\
    CVT.I.H r1, r1\n\
    ST [lane], r1\n";

fn absorb_ref() -> String {
    let mut s = String::from(".smem 2048\nDMA 0, 0, 2048\nBAR\n");
    let mut h = 32;
    while h >= 1 {
        s += &format!(
            "MOV r1, lane\nADD r2, lane, {}\nLOOP {h}\nLDS r3, [r1]\nLDS r4, [r2]\nHADD r3, r3, r4\nSTS [r1], r3\nADD r1, r1, 32\nADD r2, r2, 32\nENDLOOP\n",
            32 * h
        );
        h /= 2;
    }
    s += "LDS r3, [lane]\nCVT.I.H r3, r3\nST [lane], r3\n";
    s
}

fn the_absorb() -> Task {
    Task {
        title: Tr::new("Absorbed", "吞没"),
        brief: Tr::new(
            "Cenli's tally stone adds 64 readings per lane in half precision. Reading k of lane l is the half at rom[32k + l]; only FP16 addition is on the bench (HADD), the FP32 accumulator is not.\nWrite out[lane] = the sum, converted to an integer (CVT.I.H). Most readings are small, but each lane also holds a big one, somewhere. The same 64 numbers always have the same true sum. Make the half-precision answer land near it.",
            "岑理的计数石要对每条车道的 64 个读数做半精度求和。第 l 条车道的第 k 个读数是 rom[32k + l] 处的半精度数；工作台上只开放 FP16 加法（HADD），没有 FP32 累加器。\n写出 out[lane] = 总和，转成整数（CVT.I.H）。大多数读数很小，但每条车道里还藏着一个大的，位置不定。同样的 64 个数，真实的和只有一个。让半精度的答案落在它附近。",
        ),
        cases: absorb_cases,
        hidden: absorb_hidden,
        expect: absorb_expect,
        tolerance: 8,
        goals: [Goal::new(Metric::Cycles, 1500), Goal::new(Metric::Err, 4)],
        starter: Source::Code(ABSORB_STARTER),
        reference: Source::Code(leak(absorb_ref())),
        predict: Predict {
            question: Tr::new(
                "Add one 2048 and sixty-three 1.0s in half precision, in that order. Which value is the result closest to?",
                "用半精度把一个 2048 和 63 个 1.0 按这个顺序逐个相加，结果最接近哪个？",
            ),
            options: [
                Tr::new("2048", "2048"),
                Tr::new("2111", "2111"),
                Tr::new("2176", "2176"),
            ],
            answer: 0,
            why: Tr::new(
                "Above 2048 neighbouring halves are 2 apart. 2048 + 1 is exactly halfway, and ties go to the even neighbour, 2048. Every one of the 63 ones is swallowed the same way.",
                "2048 以上相邻的半精度数相隔 2。2048 + 1 恰在正中，平局取偶数一侧，也就是 2048。63 个 1.0 每一个都这样被吞掉。",
            ),
        },
        hints: [
            Tr::new(
                "When the 2048 comes first, every later 1.0 is added to a sum that is already too big to notice it. Order matters.",
                "2048 排在最前时，后面每个 1.0 都是加到一个大得看不见它的和上。顺序有影响。",
            ),
            Tr::new(
                "Let the small numbers meet each other first. Add the first half of the list to the second half, element by element; then do it again on the result.",
                "让小数先彼此相遇。把列表前一半和后一半对应相加，再对结果重复这一步。",
            ),
            Tr::new(
                "64 numbers need 32, 16, 8, 4, 2, 1 additions per round: six rounds. Keep them in shared memory (DMA 0, 0, 2048 brings in the whole list), one row of 32 per index.",
                "64 个数每轮做 32、16、8、4、2、1 次加法，共六轮。放在共享内存里（DMA 0, 0, 2048 把整个列表搬进来），每个下标一行 32 个。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Big swallows small", "大数吞小数"),
            line: Tr::new(
                "Floating-point addition is not associative. The order you add in is part of the answer.",
                "浮点加法不满足结合律。相加的顺序也是答案的一部分。",
            ),
            reality: Tr::new(
                "Parallel reductions on GPUs use tree sums partly for speed and partly because they lose less than a running total. Kahan summation attacks the same loss.",
                "GPU 上的并行归约用树形求和，一半为了速度，一半因为它比滚动累加丢得少。Kahan 求和对付的是同一种损失。",
            ),
            myth: Tr::new(
                "Myth: (a + b) + c equals a + (b + c). Not once rounding is involved.",
                "误解：(a + b) + c 等于 a + (b + c)。一旦有舍入就不成立。",
            ),
            simplified: Tr::new(
                "Real kernels also sort, scale or compensate; here only the order changes.",
                "真实的内核还会排序、缩放或做补偿；这里只改变顺序。",
            ),
        },
        ..lbase("the-absorb", 14, 3, Launch::ONE_WARP, 32, ABSORB_OPS)
    }
}

// ---------------------------------------------------------------- 14-4

/// Half-precision softmax parts: exp, reciprocal, lane shuffles.
const SOFTMAX_OPS: &[OpKind] = &[
    OpKind::Mov,
    OpKind::Alu(Alu::Add),
    OpKind::Alu(Alu::Sub),
    OpKind::Alu(Alu::And),
    OpKind::Alu(Alu::Or),
    OpKind::Alu(Alu::Xor),
    OpKind::Alu(Alu::Shl),
    OpKind::Alu(Alu::Shr),
    OpKind::Alu(Alu::HAdd),
    OpKind::Alu(Alu::HSub),
    OpKind::Alu(Alu::HMul),
    OpKind::Alu(Alu::HMax),
    OpKind::Cvt(Cvt::HExp),
    OpKind::Cvt(Cvt::HRcp),
    OpKind::Cvt(Cvt::IH),
    OpKind::Cvt(Cvt::HI),
    OpKind::SetP,
    OpKind::Sel,
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::Ld,
    OpKind::St,
    OpKind::Shfl,
];

/// One row of scores: the largest is `max`, the rest lie within `spread`
/// below it. `hot` extra lanes sit right under the maximum.
fn scores(seed: u64, max: f64, spread: f64, hot: usize) -> Case {
    let mut r = rng(seed);
    let mut v: Vec<f64> = (0..32)
        .map(|_| max - spread * unif(&mut r, 0.0, 1.0))
        .collect();
    v[7] = max;
    for i in 0..hot {
        v[11 + 5 * i] = max - 0.1 * (i as f64 + 1.0);
    }
    case(v.iter().map(|&x| fp::h(x)).collect())
}

fn softmax_expect(c: &Case) -> Vec<Option<i32>> {
    let v: Vec<f64> = c.rom.iter().take(32).map(|&b| h_to_f64(b)).collect();
    let m = v.iter().cloned().fold(f64::MIN, f64::max);
    let e: Vec<f64> = v.iter().map(|x| (x - m).exp()).collect();
    let z: f64 = e.iter().sum();
    e.iter()
        .map(|x| Some((1000.0 * x / z).round_ties_even() as i32))
        .collect()
}

/// Butterfly over the lanes with `op` on r{a}; clobbers r20, r21.
fn butterfly(op: &str, a: u8) -> String {
    let mut s = String::new();
    for m in [16, 8, 4, 2, 1] {
        s += &format!("XOR r20, lane, {m}\nSHFL r21, r{a}, r20\n{op} r{a}, r{a}, r21\n");
    }
    s
}

fn softmax_tail() -> String {
    // sum, reciprocal, scale to 0..1000, convert; the exp is in r1
    format!(
        "MOV r5, r1\n{}HRCP r5, r5\nHMUL r1, r1, r5\nHMUL r1, r1, 25552\nCVT.I.H r1, r1\nST [lane], r1\n",
        butterfly("HADD", 5)
    )
}

fn overflow_starter() -> String {
    format!(
        "; exp of the raw score, no shift | 直接对原始分数取 exp，没有平移\nLD r1, [lane]\nHEXP r1, r1\n{}",
        softmax_tail()
    )
}

fn overflow_ref() -> String {
    format!(
        "LD r1, [lane]\nMOV r2, r1\n{}HSUB r1, r1, r2\nHEXP r1, r1\n{}",
        butterfly("HMAX", 2),
        softmax_tail()
    )
}

fn overflow() -> Task {
    Task {
        title: Tr::new("Overflow", "溢出"),
        brief: Tr::new(
            "Cenli's listening array scores 32 echoes at once. rom[0..32] holds one half-precision score per lane. Turn the row into weights, p[i] = exp(s[i]) / sum of exp(s[j]), and write out[lane] = round(1000 · p).\nThe old kernel does exactly that formula in half precision, and some rows come back as garbage. Run it on the second public row and find the first exp that goes wrong.",
            "岑理的聆听阵列一次给 32 个回声打分。rom[0..32] 每条车道一个半精度分数。把这一行变成权重：p[i] = exp(s[i]) / 所有 exp(s[j]) 之和，写出 out[lane] = round(1000 · p)。\n旧内核恰好是按这个公式在半精度里算的，有些行出来是乱码。在第二个公开行上跑一遍，找出第一个出错的 exp。",
        ),
        cases: || {
            vec![
                scores(1441, 4.0, 8.0, 0),
                scores(1442, 12.0, 8.0, 0),
                scores(1443, 20.0, 8.0, 0),
            ]
        },
        hidden: || {
            vec![
                scores(1444, 11.0, 8.0, 2),
                case(vec![fp::h(-20.0); 32]),
                scores(1445, 100.0, 10.0, 0),
            ]
        },
        expect: softmax_expect,
        tolerance: 3,
        goals: [Goal::new(Metric::Cycles, 560), Goal::new(Metric::Err, 2)],
        starter: Source::Code(leak(overflow_starter())),
        reference: Source::Code(leak(overflow_ref())),
        clue: Some(Clue {
            what: Tr::new(
                "the first exp that came back as infinity",
                "第一个算出无穷大的 exp",
            ),
            find: crate::grade::first_inf,
        }),
        predict: Predict {
            question: Tr::new(
                "What does HEXP return for the half-precision input 12? (e^12 is about 162755; the largest half is 65504.)",
                "对半精度输入 12，HEXP 返回什么？（e^12 约为 162755；最大的半精度数是 65504。）",
            ),
            options: [
                Tr::new("162755", "162755"),
                Tr::new("Infinity", "无穷大"),
                Tr::new("65504", "65504"),
            ],
            answer: 1,
            why: Tr::new(
                "Anything past 65504 rounds to infinity in half precision, and the largest input that survives is about 11.09. Infinity in the sum turns the reciprocal into 0 and every p into NaN.",
                "半精度里超过 65504 的数都舍成无穷大，能幸存的最大输入约为 11.09。和里一出现无穷大，倒数就成了 0，每个 p 都变成 NaN。",
            ),
        },
        hints: [
            Tr::new(
                "On the second public row the first exp already overflows. Infinity in the sum makes the reciprocal 0, and 0 times infinity is NaN.",
                "第二个公开行里，第一个 exp 就已经溢出。和里出现无穷大，倒数就是 0，而 0 乘无穷大是 NaN。",
            ),
            Tr::new(
                "softmax(s) is the same as softmax(s - c) for any c. Subtract the row's maximum first: the biggest term becomes exp(0) = 1 and nothing can overflow.",
                "对任意 c，softmax(s) 与 softmax(s - c) 相同。先减去整行的最大值：最大项变成 exp(0) = 1，什么都不会溢出。",
            ),
            Tr::new(
                "Find the maximum with a SHFL butterfly: XOR the lane with 16, 8, 4, 2, 1 and HMAX with the value you fetch. The sum is the same butterfly with HADD.",
                "用 SHFL 蝶形求最大值：把车道号依次与 16、8、4、2、1 异或，每步与取来的值做 HMAX。求和是同样的蝶形，换成 HADD。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Subtract the maximum", "先减最大值"),
            line: Tr::new(
                "Shifting every score by the same number leaves softmax unchanged, and keeps exp in range.",
                "把所有分数平移同一个数，softmax 不变，exp 也不会越界。",
            ),
            reality: Tr::new(
                "Every softmax in a deep-learning library does this, and attention kernels carry a running maximum for the same reason.",
                "深度学习库里的每个 softmax 都这么做，注意力内核带着一个滚动的最大值，也是这个原因。",
            ),
            myth: Tr::new(
                "Myth: more precision fixes overflow. FP32 exp overflows at 88, so scores of 90 to 100 still blow up. The shift fixes every format.",
                "误解：更高的精度能解决溢出。FP32 的 exp 在 88 就溢出，90 到 100 的分数照样爆。平移对所有格式都有效。",
            ),
            simplified: Tr::new(
                "OC-1 has HEXP and HRCP in half precision only; there is no fused softmax unit.",
                "OC-1 只有半精度的 HEXP 和 HRCP，没有融合的 softmax 单元。",
            ),
        },
        ..lbase("overflow", 14, 4, Launch::ONE_WARP, 32, SOFTMAX_OPS)
    }
}

// ---------------------------------------------------------------- 14-5

/// INT8 path: scale, round to INT8, multiply and add as plain integers.
const INT8_OPS: &[OpKind] = &[
    OpKind::Mov,
    OpKind::Alu(Alu::Add),
    OpKind::Alu(Alu::Sub),
    OpKind::Alu(Alu::Mul),
    OpKind::Alu(Alu::And),
    OpKind::Alu(Alu::Or),
    OpKind::Alu(Alu::Xor),
    OpKind::Alu(Alu::Shl),
    OpKind::Alu(Alu::Shr),
    OpKind::Alu(Alu::Min),
    OpKind::Alu(Alu::Max),
    OpKind::Alu(Alu::FMul),
    OpKind::Cvt(Cvt::FH),
    OpKind::Cvt(Cvt::FI),
    OpKind::Cvt(Cvt::IF),
    OpKind::Cvt(Cvt::BF),
    OpKind::Cvt(Cvt::BI),
    OpKind::SetP,
    OpKind::Sel,
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::Ld,
    OpKind::St,
];

const Q_N: usize = 64;
const Q_X: usize = 32 + 32 * Q_N;

/// rom[0] = 1/s_w, rom[1] = 1/s_x, rom[2] = s_w·s_x·S (all FP32), where
/// s = max|.| / 127 and S puts the largest |y| of the case at 1000;
/// rom[32 + 32k + lane] = W[lane][k], rom[Q_X + k] = x[k] (halves).
fn q_case(seed: u64, wr: (f64, f64), xr: (f64, f64)) -> Case {
    let mut r = rng(seed);
    let mut rom = vec![0; Q_X + Q_N];
    for k in 0..Q_N {
        for lane in 0..32 {
            rom[32 + 32 * k + lane] = fp::h(unif(&mut r, wr.0, wr.1));
        }
        rom[Q_X + k] = fp::h(unif(&mut r, xr.0, xr.1));
    }
    let amax = |v: &mut dyn Iterator<Item = i32>| v.fold(0f64, |m, b| m.max(h_to_f64(b).abs()));
    let sw = amax(&mut (0..32 * Q_N).map(|i| rom[32 + i])) / 127.0;
    let sx = amax(&mut (0..Q_N).map(|k| rom[Q_X + k])) / 127.0;
    let ymax = q_y(&rom).iter().fold(0f64, |m, y| m.max(y.abs()));
    rom[0] = f32_bits((1.0 / sw) as f32);
    rom[1] = f32_bits((1.0 / sx) as f32);
    rom[2] = f32_bits((sw * sx * 1000.0 / ymax) as f32);
    case(rom)
}

fn q_y(rom: &[i32]) -> Vec<f64> {
    (0..32)
        .map(|lane| {
            (0..Q_N)
                .map(|k| h_to_f64(rom[32 + 32 * k + lane]) * h_to_f64(rom[Q_X + k]))
                .sum()
        })
        .collect()
}

/// out = round(S · y), with S recovered from rom[2] / (s_w · s_x).
fn q_expect(c: &Case) -> Vec<Option<i32>> {
    let (iw, ix) = (fp::f_to_f32(c.rom[0]) as f64, fp::f_to_f32(c.rom[1]) as f64);
    let s = fp::f_to_f32(c.rom[2]) as f64 * iw * ix;
    q_y(&c.rom)
        .iter()
        .map(|y| Some((y * s).round_ties_even() as i32))
        .collect()
}

/// One element: load, scale into INT8 range, round and clamp, multiply, add.
const Q_STARTER: &str = "LD r28, [0]\n\
    LD r29, [1]\n\
    LD r30, [2]\n\
    ADD r1, lane, 32\n\
    MOV r2, 2080\n\
    MOV r3, 0\n\
    LOOP 8\n\
    LOOP 8\n\
    LD r4, [r1]\n\
    LD r5, [r2]\n\
    CVT.F.H r4, r4\n\
    FMUL r4, r4, r28\n\
    CVT.B.F r4, r4\n\
    CVT.F.H r5, r5\n\
    FMUL r5, r5, r29\n\
    CVT.B.F r5, r5\n\
    MUL r4, r4, r5\n\
    ADD r3, r3, r4\n\
    CVT.B.I r3, r3 ; the sum is kept as INT8 too | 和也按 INT8 存\n\
    ADD r1, r1, 32\n\
    ADD r2, r2, 1\n\
    ENDLOOP\n\
    ENDLOOP\n\
    CVT.F.I r3, r3\n\
    FMUL r3, r3, r30\n\
    CVT.I.F r3, r3\n\
    ST [lane], r3\n";

/// The starter with a plain integer sum: right, one load at a time.
#[cfg(test)]
fn q_slow() -> String {
    Q_STARTER.replace(
        "CVT.B.I r3, r3 ; the sum is kept as INT8 too | 和也按 INT8 存\n",
        "",
    )
}

fn q_ref() -> String {
    let mut s = String::from(
        "LD r28, [0]\nLD r29, [1]\nLD r30, [2]\nADD r1, lane, 32\nMOV r2, 2080\nMOV r3, 0\nLOOP 8\n",
    );
    for i in 0..8 {
        s += &format!("LD r{}, [r1]\nADD r1, r1, 32\n", 4 + i);
    }
    for i in 0..8 {
        s += &format!("LD r{}, [r2]\nADD r2, r2, 1\n", 12 + i);
    }
    for i in 0..8 {
        let (a, b) = (4 + i, 12 + i);
        s += &format!(
            "CVT.F.H r{a}, r{a}\nFMUL r{a}, r{a}, r28\nCVT.B.F r{a}, r{a}\nCVT.F.H r{b}, r{b}\nFMUL r{b}, r{b}, r29\nCVT.B.F r{b}, r{b}\nMUL r{a}, r{a}, r{b}\nADD r3, r3, r{a}\n"
        );
    }
    s += "ENDLOOP\nCVT.F.I r3, r3\nFMUL r3, r3, r30\nCVT.I.F r3, r3\nST [lane], r3\n";
    s
}

fn the_int8() -> Task {
    Task {
        title: Tr::new("Eight Bits", "八位整数"),
        brief: Tr::new(
            "Cenli's relay stone is short on bandwidth, so the probe now works on 8-bit integers. Each lane takes a 64-term dot product: y[lane] = sum over k of W[k][lane] · x[k], with W at rom[32 + 32k + lane] and x at rom[2080 + k], both halves.\nThe host gives three FP32 constants: rom[0] = 1/s_w and rom[1] = 1/s_x (each s is the largest |value| divided by 127), and rom[2] = s_w · s_x · S, where S puts the largest |y| of the case at 1000.\nWrite out[lane] = round(S · y). Quantize both operands to INT8 (CVT.B.F rounds and clamps to ±127), multiply the integers, and keep the sum somewhere that cannot wrap. The FP adders are off this bench.",
            "岑理的中继石带宽不够，探头改用 8 位整数。每条车道做一个 64 项点积：y[lane] = 对 k 求和 W[k][lane] · x[k]，W 在 rom[32 + 32k + lane]，x 在 rom[2080 + k]，都是半精度。\n主机给出三个 FP32 常数：rom[0] = 1/s_w，rom[1] = 1/s_x（s 是各自最大 |值| 除以 127），rom[2] = s_w · s_x · S，S 把每组数据里最大的 |y| 放到 1000。\n写出 out[lane] = round(S · y)。把两个操作数量化成 INT8（CVT.B.F 四舍五入并钳在 ±127），按整数相乘，把和放在不会回绕的地方。这张工作台上没有浮点加法器。",
        ),
        cases: || {
            vec![
                q_case(1451, (-1.0, 1.0), (-1.0, 1.0)),
                q_case(1452, (-1.0, 1.0), (-1.0, 1.0)),
                q_case(1453, (-1.0, 1.0), (-1.0, 1.0)),
            ]
        },
        hidden: || {
            vec![
                q_case(1454, (-8.0, 8.0), (-0.5, 0.5)),
                q_case(1455, (0.0, 1.0), (0.0, 1.0)),
                q_case(1456, (-1.0, 1.0), (-1.0, 1.0)),
            ]
        },
        expect: q_expect,
        tolerance: 10,
        goals: [Goal::new(Metric::Cycles, 4700), Goal::new(Metric::Err, 8)],
        starter: Source::Code(Q_STARTER),
        reference: Source::Code(leak(q_ref())),
        predict: Predict {
            question: Tr::new(
                "Multiply 64 pairs of INT8 numbers and add the products. How many bits must the accumulator have so it can never overflow?",
                "把 64 对 INT8 数相乘再累加，累加器至少要多少位才一定不会溢出？",
            ),
            options: [Tr::new("8", "8"), Tr::new("16", "16"), Tr::new("32", "32")],
            answer: 2,
            why: Tr::new(
                "One product reaches 127 × 127 = 16129, and 64 of them reach about a million: 21 bits plus sign. Only the 32-bit register is safe.",
                "一个乘积最大 127 × 127 = 16129，64 个加起来约一百万：21 位再加符号位。只有 32 位寄存器才安全。",
            ),
        },
        hints: [
            Tr::new(
                "The starter clamps the running sum back into INT8 after every add. One product is already as big as 16129.",
                "初始代码每加一次就把累加和钳回 INT8。单个乘积就已经有 16129 那么大。",
            ),
            Tr::new(
                "Scale only at the ends: q = CVT.B.F(CVT.F.H(v) · 1/s) on the way in; at the end CVT.F.I the integer sum, FMUL by rom[2], CVT.I.F.",
                "只在两端做缩放：进来时 q = CVT.B.F(CVT.F.H(v) · 1/s)；最后把整数和 CVT.F.I，乘 rom[2]，再 CVT.I.F。",
            ),
            Tr::new(
                "Loads wait 400 cycles each. Fetch eight W and eight x before converting any, then convert, multiply and add the batch: eight passes instead of 64.",
                "每次取数要等 400 个周期。先取八个 W 和八个 x 再转换，然后对这一批做转换、相乘、累加：八趟而不是 64 趟。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Scale at the ends", "两端缩放"),
            line: Tr::new(
                "Quantize on the way in, accumulate in a wide integer, rescale once on the way out.",
                "进来时量化，用宽整数累加，出去时缩放一次。",
            ),
            reality: Tr::new(
                "INT8 inference on GPUs and phones works like this: INT8 multiplies, INT32 accumulators, floating-point scales at the ends.",
                "GPU 和手机上的 INT8 推理就是这样：INT8 相乘，INT32 累加，浮点缩放放在两端。",
            ),
            myth: Tr::new(
                "Myth: 8-bit data needs an 8-bit accumulator. The products need 14 bits and the sum of 64 of them 21.",
                "误解：8 位数据就用 8 位累加器。乘积要 14 位，64 个乘积之和要 21 位。",
            ),
            simplified: Tr::new(
                "OC-1 uses one symmetric scale per tensor; real kernels often keep one per channel or per block.",
                "OC-1 每个张量只用一个对称缩放；真实内核常常每通道或每块各一个。",
            ),
        },
        ..lbase("the-int8", 14, 5, Launch::ONE_WARP, 32, INT8_OPS)
    }
}

// ---------------------------------------------------------------- 14-7

/// Dynamic scale: abs-max, butterfly, reciprocal, INT8 conversion.
const CLIP_OPS: &[OpKind] = &[
    OpKind::Mov,
    OpKind::Alu(Alu::Add),
    OpKind::Alu(Alu::Sub),
    OpKind::Alu(Alu::Mul),
    OpKind::Alu(Alu::And),
    OpKind::Alu(Alu::Or),
    OpKind::Alu(Alu::Xor),
    OpKind::Alu(Alu::Shl),
    OpKind::Alu(Alu::Shr),
    OpKind::Alu(Alu::Min),
    OpKind::Alu(Alu::Max),
    OpKind::Alu(Alu::HMul),
    OpKind::Alu(Alu::HMax),
    OpKind::Alu(Alu::FMul),
    OpKind::Cvt(Cvt::HRcp),
    OpKind::Cvt(Cvt::FH),
    OpKind::Cvt(Cvt::FI),
    OpKind::Cvt(Cvt::IF),
    OpKind::Cvt(Cvt::BF),
    OpKind::Cvt(Cvt::BI),
    OpKind::SetP,
    OpKind::Sel,
    OpKind::Loop,
    OpKind::EndLoop,
    OpKind::Ld,
    OpKind::St,
    OpKind::Shfl,
];

const C_K: usize = 16;

/// rom[0..16] = INT8 weights, rom[16] = the weight scale folded with the
/// output scale (FP32; the largest |out| of the case is 1000); rom[32 + 32k + lane] = a[lane][k] (halves).
/// `limit` bounds every activation; `peaks` lanes get one value of ±`top`.
fn clip_case(seed: u64, limit: f64, top: f64, peaks: &[usize]) -> Case {
    let mut r = rng(seed);
    let mut rom = vec![0; 32 + 32 * C_K];
    for w in rom.iter_mut().take(C_K) {
        *w = below(&mut r, 201) - 100;
    }
    for k in 0..C_K {
        for lane in 0..32 {
            rom[32 + 32 * k + lane] = fp::h(unif(&mut r, -limit, limit));
        }
    }
    for (i, &lane) in peaks.iter().enumerate() {
        let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
        rom[32 + 32 * (3 * i % C_K) + lane] = fp::h(sign * top);
    }
    let ymax = clip_y(&rom).iter().fold(0f64, |m, y| m.max(y.abs()));
    rom[C_K] = f32_bits((1000.0 / ymax) as f32);
    case(rom)
}

/// Σ a·w (unscaled).
fn clip_y(rom: &[i32]) -> Vec<f64> {
    (0..32)
        .map(|lane| {
            (0..C_K)
                .map(|k| h_to_f64(rom[32 + 32 * k + lane]) * rom[k] as f64)
                .sum()
        })
        .collect()
}

fn clip_expect(c: &Case) -> Vec<Option<i32>> {
    let s = fp::f_to_f32(c.rom[C_K]) as f64;
    clip_y(&c.rom)
        .iter()
        .map(|y| Some((y * s).round_ties_even() as i32))
        .collect()
}

/// Second pass: quantize with r4, multiply by the weights, sum in INT32,
/// dequantize with r5 and apply rom[16].
const CLIP_PASS: &str = "LD r30, [16]\n\
    ADD r1, lane, 32\n\
    MOV r6, 0\n\
    MOV r7, 0\n\
    LOOP 16\n\
    LD r3, [r1]\n\
    LD r8, [r6]\n\
    CVT.F.H r3, r3\n\
    FMUL r3, r3, r4\n\
    CVT.B.F r3, r3\n\
    MUL r3, r3, r8\n\
    ADD r7, r7, r3\n\
    ADD r1, r1, 32\n\
    ADD r6, r6, 1\n\
    ENDLOOP\n\
    CVT.F.I r7, r7\n\
    FMUL r7, r7, r5\n\
    FMUL r7, r7, r30\n\
    CVT.I.F r7, r7\n\
    ST [lane], r7\n";

fn clip_starter() -> String {
    format!(
        "; scale fixed from the calibration set, range +-1 | 缩放写死，来自校准集（范围 ±1）\nMOV r4, {}\nMOV r5, {}\n{CLIP_PASS}",
        f32_bits(127.0),
        f32_bits(1.0 / 127.0)
    )
}

fn clip_ref() -> String {
    format!(
        "ADD r1, lane, 32\nMOV r2, 0\nLOOP 16\nLD r3, [r1]\nAND r3, r3, 32767\nHMAX r2, r2, r3\nADD r1, r1, 32\nENDLOOP\n{}HRCP r4, r2\nHMUL r4, r4, 22512\nCVT.F.H r4, r4\nCVT.F.H r5, r2\nFMUL r5, r5, {}\n{CLIP_PASS}",
        butterfly("HMAX", 2),
        f32_bits(1.0 / 127.0)
    )
}

fn the_clipped() -> Task {
    Task {
        title: Tr::new("Clipped", "削顶"),
        brief: Tr::new(
            "Cenli's relay stone sends each lane 16 activations, halves at rom[32 + 32k + lane], to be dotted with 16 shared INT8 weights, plain integers at rom[0..16]. The weight scale is already folded in: rom[16] is an FP32 constant, and out[lane] = round(rom[16] · sum of a[k] · w[k]). The host chose it so the largest |out| of a case is about 1000.\nThe old kernel turns the activations into INT8 with a scale it learned on the calibration set, where everything stayed within ±1. Run it on the second public case and find the first activation it has to clamp.",
            "岑理的中继石给每条车道送来 16 个激活，半精度，在 rom[32 + 32k + lane]，要和 16 个共享的 INT8 权重（普通整数，rom[0..16]）做点积。权重的缩放已经折进去了：rom[16] 是一个 FP32 常数，out[lane] = round(rom[16] · 对 k 求和 a[k] · w[k])。主机选它是为了让每组数据里最大的 |out| 约为 1000。\n旧内核用在校准集上学到的缩放把激活变成 INT8，那批数据全在 ±1 以内。在第二个公开用例上跑一遍，找出它第一个被迫钳住的激活。",
        ),
        cases: || {
            vec![
                clip_case(1471, 1.0, 1.0, &[]),
                clip_case(1472, 6.0, 6.0, &[3, 14, 25]),
            ]
        },
        hidden: || {
            vec![
                clip_case(1473, 40.0, 40.0, &[5, 9, 20]),
                clip_case(1474, 0.05, 0.05, &[]),
                clip_case(1475, 1.0, 1.5, &[31]),
            ]
        },
        expect: clip_expect,
        tolerance: 10,
        goals: [Goal::new(Metric::Cycles, 10800), Goal::new(Metric::Err, 7)],
        starter: Source::Code(leak(clip_starter())),
        reference: Source::Code(leak(clip_ref())),
        clue: Some(Clue {
            what: Tr::new(
                "the first activation clamped to the INT8 limit",
                "第一个被钳到 INT8 上限的激活",
            ),
            find: crate::grade::first_clamp,
        }),
        predict: Predict {
            question: Tr::new(
                "The calibration scale maps ±1.0 to ±127. What INT8 value does an activation of 6.0 become?",
                "校准缩放把 ±1.0 映射到 ±127。一个等于 6.0 的激活会变成什么 INT8 值？",
            ),
            options: [
                Tr::new("127", "127"),
                Tr::new("762", "762"),
                Tr::new("6", "6"),
            ],
            answer: 0,
            why: Tr::new(
                "6.0 × 127 = 762 is far outside ±127, so the conversion clamps it to 127: the activation is read as 1.0, and the dot product comes out too small.",
                "6.0 × 127 = 762，远在 ±127 之外，转换把它钳成 127：这个激活被当成了 1.0，点积于是偏小。",
            ),
        },
        hints: [
            Tr::new(
                "In the second case the lanes holding ±6 are clamped to ±127, which reads as ±1. Their dot products come out far too small.",
                "第二个用例里，持有 ±6 的车道被钳在 ±127，读出来就是 ±1，点积整体偏小。",
            ),
            Tr::new(
                "Let the data set the scale. Each lane takes the largest |a| of its 16 values (AND 32767 drops the sign of a half), then a SHFL butterfly with HMAX spreads the largest over the warp.",
                "让数据决定缩放。每条车道取自己 16 个值的最大 |a|（AND 32767 去掉半精度数的符号），再用 SHFL 蝶形配 HMAX 把最大值铺到整个 warp。",
            ),
            Tr::new(
                "Quantize with 127 / max (HRCP, then HMUL by 127). Undo it on the INT32 sum with max / 127 before the rom[16] scale. A scale made wide to be safe turns ±0.05 data into zeros.",
                "用 127 / max 量化（HRCP，再 HMUL 乘 127）。在乘 rom[16] 之前，用 max / 127 还原 INT32 和。为安全而写得很宽的缩放，会把 ±0.05 的数据变成零。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Calibrate to the data", "让缩放跟着数据走"),
            line: Tr::new(
                "A fixed scale is a bet on the range. A scale taken from the data cannot lose that bet.",
                "写死的缩放是对数据范围的一次赌注。从数据里取的缩放不会输。",
            ),
            reality: Tr::new(
                "INT8 inference computes activation scales on the fly, per tensor or per token. Static scales from a calibration set break on the first outlier.",
                "INT8 推理会即时算出激活的缩放，按张量或按 token。校准集给出的静态缩放，遇到第一个离群值就会出错。",
            ),
            myth: Tr::new(
                "Myth: calibrate once and ship. The first outlier is clamped, and a scale widened to be safe rounds the small values to zero.",
                "误解：校准一次就能上线。第一个离群值会被钳住，为安全而放宽的缩放又会把小值舍成零。",
            ),
            simplified: Tr::new(
                "OC-1 uses one scale for the whole tensor; real kernels often scale per row or per channel.",
                "OC-1 对整个张量只用一个缩放；真实内核常常按行或按通道各取一个。",
            ),
        },
        ..lbase("the-clipped", 14, 7, Launch::ONE_WARP, 32, CLIP_OPS)
    }
}

// ---- 14-8 the-sorter ----

const S_B: usize = 64;
const S_HID: usize = 32;
/// rom: [0, 4096) images (64 pixels each); W2 as FP16 fragments; W1 as INT8
/// words; W1 scales (FP32); W1 as FP16 fragments.
const S_W2H: usize = 4096;
const S_W1Q: usize = 4480;
const S_S1: usize = 4992;
const S_W1H: usize = 5024;
const S_LEN: usize = 7072;

/// The fixed network: 64 pixels -> 32 hidden (ReLU, no bias) -> 10 classes.
struct Net {
    glyph: Vec<[bool; 64]>,
    w1: Vec<[f64; S_HID]>,
    w2: Vec<[f64; 12]>,
    s1: Vec<f32>,
}

/// A glyph slid by (sx, sy) pixels, as 0 / 1 per pixel.
fn slid(t: &[bool; 64], sx: i32, sy: i32) -> [f64; 64] {
    std::array::from_fn(|i| {
        let (x, y) = ((i % 8) as i32 - sx, (i / 8) as i32 - sy);
        ((0..8).contains(&x) && (0..8).contains(&y) && t[(y * 8 + x) as usize]) as u8 as f64
    })
}

/// Weights for nine 4 x 4 cell patterns (plus the white block as class 9).
fn make_net(pats: &[u16]) -> Net {
    let mut glyph: Vec<[bool; 64]> = pats
        .iter()
        .map(|&p| std::array::from_fn(|i| (p >> ((i / 16) * 4 + i % 8 / 2)) & 1 == 1))
        .collect();
    glyph.push([true; 64]);
    let mut w1 = vec![[0.0; S_HID]; 64];
    for (v, &(sx, sy)) in [(0, 0), (1, 0), (-1, 0)].iter().enumerate() {
        let blurs: Vec<[f64; 64]> = (0..9).map(|c| slid(&glyph[c], sx, sy)).collect();
        // what sets one glyph apart is its difference from the average glyph
        let avg: Vec<f64> = (0..64)
            .map(|i| blurs.iter().map(|b| b[i]).sum::<f64>() / 9.0)
            .collect();
        for c in 0..9 {
            let j = 3 * c + v;
            let d: Vec<f64> = (0..64).map(|i| blurs[c][i] - avg[i]).collect();
            let mu = d.iter().sum::<f64>() / 64.0;
            let b: Vec<f64> = d;
            // an ideal picture of the shifted glyph answers with about 48000
            let ideal = |i: usize| {
                let (x, y) = ((i % 8) as i32 - sx, (i / 8) as i32 - sy);
                let on =
                    (0..8).contains(&x) && (0..8).contains(&y) && glyph[c][(y * 8 + x) as usize];
                if on { 175.0 } else { 12.0 }
            };
            let a = 48000.0 / (0..64).map(|i| (b[i] - mu) * ideal(i)).sum::<f64>();
            for i in 0..64 {
                w1[i][j] = h_to_f64(fp::h(a * (b[i] - mu)));
            }
        }
    }
    for row in w1.iter_mut() {
        // two copies of the brightness unit: class 9 reads half of their difference,
        // so an infinity in both turns the logit into NaN
        row[27] = h_to_f64(fp::h(4.4));
        row[28] = h_to_f64(fp::h(4.4));
    }
    let mut w2 = vec![[0.0; 12]; S_HID];
    for (j, row) in w2.iter_mut().enumerate().take(27) {
        row[j / 3] = 1.0;
    }
    w2[27][9] = 1.0;
    w2[28][9] = -0.5;
    let s1 = (0..S_HID)
        .map(|n| {
            let m = (0..64).fold(0f64, |m, k| m.max(w1[k][n].abs()));
            if m > 0.0 { (m / 127.0) as f32 } else { 1.0 }
        })
        .collect();
    Net { glyph, w1, w2, s1 }
}

/// Smallest top1 / top2 logit ratio over every clean picture of every class
/// at all nine shifts (0 when one is misread).
fn clean_margin(n: &Net) -> f64 {
    let mut worst = f64::MAX;
    for c in 0..10 {
        for (dx, dy) in (-1..=1).map(|a| (a, 0)) {
            let (dx, dy) = if c == 9 { (0, 0) } else { (dx, dy) };
            let pix: Vec<i32> = (0..64)
                .map(|i| {
                    let (x, y) = ((i % 8) - dx, (i / 8) - dy);
                    let on = (0..8).contains(&x)
                        && (0..8).contains(&y)
                        && n.glyph[c][(y * 8 + x) as usize];
                    if on { 190 } else { 20 }
                })
                .collect();
            let mut l = logits_of(n, &pix);
            if argmax(&l) as usize != c {
                return 0.0;
            }
            l.sort_by(|a, b| b.total_cmp(a));
            worst = worst.min(l[0] / l[1].max(1e-9));
        }
    }
    worst
}

/// The best of 400 glyph sets (by seed) for reading clean pictures with room to spare.
/// The best of 400 glyph sets (by seed) for reading clean pictures with room to spare.
fn build_net(seed: u64) -> Net {
    let mut r = rng(seed);
    let mut best: Option<(f64, Net)> = None;
    for _ in 0..400 {
        let mut pats: Vec<u16> = Vec::new();
        while pats.len() < 9 {
            let p = below(&mut r, 1 << 16) as u16;
            if (5..=6).contains(&p.count_ones()) && pats.iter().all(|q| (p ^ q).count_ones() >= 7) {
                pats.push(p);
            }
        }
        let n = make_net(&pats);
        let m = clean_margin(&n);
        if best.as_ref().is_none_or(|b| m > b.0) {
            best = Some((m, n));
        }
    }
    best.unwrap().1
}

fn net() -> &'static Net {
    static NET: std::sync::OnceLock<Net> = std::sync::OnceLock::new();
    NET.get_or_init(|| build_net(1480))
}

/// INT8 weight of W1[k][n].
fn w1_q(n: &Net, k: usize, j: usize) -> i32 {
    (n.w1[k][j] / n.s1[j] as f64)
        .round_ties_even()
        .clamp(-127.0, 127.0) as i32
}

/// Pixels of one picture: the glyph of `class`, shifted by up to one pixel,
/// bright strokes on a dim ground, three random hot pixels.
fn picture9(r: &mut Rng, class: usize) -> [i32; 64] {
    let g = &net().glyph[class];
    // class 9 lights the whole frame, so a shift changes nothing
    let (dx, dy) = if class == 9 {
        (0, 0)
    } else {
        (below(r, 3) - 1, 0)
    };
    std::array::from_fn(|i| {
        let (x, y) = ((i % 8) as i32 - dx, (i / 8) as i32 - dy);
        let on = (0..8).contains(&x) && (0..8).contains(&y) && g[(y * 8 + x) as usize];
        if on { 170 + below(r, 46) } else { below(r, 40) }
    })
}

/// A batch of 64 pictures; `white` of them are all 255 (spread evenly).
fn sorter_case(seed: u64, white: usize) -> Case {
    let n = net();
    let mut r = rng(seed);
    let mut rom = vec![0; S_LEN];
    for i in 0..S_B {
        let p = if (i * white) % S_B < white {
            [255; 64]
        } else {
            let c = below(&mut r, 10) as usize;
            picture9(&mut r, c)
        };
        rom[64 * i..64 * i + 64].copy_from_slice(&p);
    }
    for kt in 0..8 {
        for ct in 0..3 {
            for e in 0..16 {
                let (row, col) = (4 * kt + e / 4, 4 * ct + e % 4);
                rom[S_W2H + (kt * 3 + ct) * 16 + e] = fp::h(n.w2[row][col]);
            }
        }
    }
    for b in 0..128 {
        let (nt, kt) = (b / 16, b % 16);
        for e in 0..16 {
            let (k, j) = (4 * kt + e / 4, 4 * nt + e % 4);
            rom[S_W1H + 16 * b + e] = fp::h(n.w1[k][j]);
            rom[S_W1Q + 4 * b + e / 4] |= (w1_q(n, k, j) & 255) << (8 * (e % 4));
        }
    }
    for (j, s) in n.s1.iter().enumerate() {
        rom[S_S1 + j] = f32_bits(*s);
    }
    case(rom)
}

fn argmax(l: &[f64]) -> i32 {
    let mut b = 0;
    for (c, &v) in l.iter().enumerate() {
        if v > l[b] {
            b = c;
        }
    }
    b as i32
}

/// Logits of one picture from the exact network.
fn sorter_logits(pix: &[i32]) -> Vec<f64> {
    logits_of(net(), pix)
}

fn logits_of(n: &Net, pix: &[i32]) -> Vec<f64> {
    let h: Vec<f64> = (0..S_HID)
        .map(|j| {
            (0..64)
                .map(|k| n.w1[k][j] * pix[k] as f64)
                .sum::<f64>()
                .max(0.0)
        })
        .collect();
    (0..10)
        .map(|c| (0..S_HID).map(|j| n.w2[j][c] * h[j]).sum())
        .collect()
}

fn sorter_expect(c: &Case) -> Vec<Option<i32>> {
    (0..S_B)
        .map(|i| Some(argmax(&sorter_logits(&c.rom[64 * i..64 * i + 64]))))
        .collect()
}

fn sorter_live(frame: i32, at: [i32; 2]) -> Case {
    sorter_case(
        1490 + frame.rem_euclid(8) as u64,
        4 * at[0].rem_euclid(5) as usize,
    )
}

/// The tensor-unit sorter. 8 pairs of 4-picture tiles (one per half-warp);
/// per pair: 8 hidden tiles, each a 16-step MMA chain, ReLU, FP16 into shared
/// memory, then 8 steps of the second layer and an argmax by SHFL.
/// `packed`: weights come as INT8 words (scale applied to the FP32 sums).
fn sorter_ref(packed: bool) -> String {
    let (smem, hbase) = if packed { (5280, 5024) } else { (6784, 6528) };
    let mut s = format!(".smem {smem}\n");
    s += &if packed {
        "DMA 0, 0, 5024\n".to_string()
    } else {
        "DMA 0, 0, 4480\nDMA 4480, 5024, 2048\n".to_string()
    };
    s += "AND r1, lane, 15\nSHR r2, lane, 2\nAND r2, r2, 3\nSHR r3, lane, 4\nAND r4, lane, 3\n\
          SHL r5, r3, 8\nSHL r11, r2, 6\nADD r5, r5, r11\nADD r5, r5, r4\n\
          ADD r7, r1, 4096\nSHL r9, r3, 2\nADD r9, r9, r2\nMOV r8, 0\nMOV r10, 0\n";
    if packed {
        s += "ADD r6, r2, 4480\nSHL r23, r4, 3\nMOV r22, 24\nSUB r23, r22, r23\n";
    } else {
        s += "ADD r6, r1, 4480\n";
    }
    s += "BAR\nLOOP 8\nMOV r11, r6\n";
    s += &format!("ADD r12, lane, {hbase}\n");
    if packed {
        s += "ADD r21, r4, 4992\n";
    }
    s += "LOOP 8\nADD r13, r8, r5\nMOV r14, 0\nLOOP 16\nLDS r15, [r13]\nCVT.H.I r15, r15\nHMUL r15, r15, 7168\nLDS r16, [r11]\n";
    if packed {
        s += "SHL r16, r16, r23\nSHR r16, r16, 24\nCVT.H.I r16, r16\n";
    }
    s += "MMA r14, r15, r16, r14\nADD r13, r13, 4\n";
    s += if packed {
        "ADD r11, r11, 4\n"
    } else {
        "ADD r11, r11, 16\n"
    };
    s += "ENDLOOP\n";
    if packed {
        s += "LDS r22, [r21]\nADD r21, r21, 4\nFMUL r14, r14, r22\n";
    }
    s += "FMAX r14, r14, 0\nCVT.H.F r14, r14\nSTS [r12], r14\nADD r12, r12, 32\nENDLOOP\n";
    s += &format!("ADD r12, lane, {hbase}\n");
    s += "MOV r13, r7\nMOV r17, 0\nMOV r18, 0\nMOV r19, 0\nLOOP 8\nLDS r15, [r12]\nADD r12, r12, 32\n";
    for acc in [17, 18, 19] {
        s += &format!("LDS r16, [r13]\nADD r13, r13, 16\nMMA r{acc}, r15, r16, r{acc}\n");
    }
    s += "ENDLOOP\n";
    // argmax of the 12 logits of a picture: max over its 3 registers and 4 lanes,
    // then the lowest class index that equals it
    s += "FMAX r24, r17, r18\nFMAX r24, r24, r19\n";
    for m in [1, 2] {
        s += &format!("XOR r25, lane, {m}\nSHFL r26, r24, r25\nFMAX r24, r24, r26\n");
    }
    s += "MOV r27, 99\nADD r28, r4, 8\nSETP.EQ p1, r19, r24\nSEL r27, p1, r28, r27\n\
          ADD r28, r4, 4\nSETP.EQ p1, r18, r24\nSEL r27, p1, r28, r27\n\
          SETP.EQ p1, r17, r24\nSEL r27, p1, r4, r27\n";
    for m in [1, 2] {
        s += &format!("XOR r25, lane, {m}\nSHFL r26, r27, r25\nMIN r27, r27, r26\n");
    }
    s += "SETP.EQ p0, r4, 0\nADD r20, r10, r9\nIF p0\nST [r20], r27\nENDIF\nADD r8, r8, 512\nADD r10, r10, 8\nENDLOOP\n";
    s
}

fn the_sorter() -> Task {
    Task {
        title: Tr::new("The Sorter", "分拣器"),
        brief: Tr::new(
            "The last layer of the Grid carries a sorting network, and Cenli wants proof that it runs safely on the replica. It reads a picture of an alien glyph, 8 x 8 pixels of 0..255, and names one of 10 classes. rom[64i .. 64i + 64] is picture i, 64 pictures per batch, written to out[i] as the class number.\nThe network has no biases: hidden[j] = max(0, sum over k of W1[k][j] · x[k]) for 32 hidden units, then logit[c] = sum over j of W2[j][c] · hidden[j] for 10 classes. out[i] is the class with the largest logit, the lowest number on a tie. The answer must match the exact network on every picture, including the all-255 white frames.\nThe weights sit in rom twice, cut into 4 x 4 tiles for the tensor unit, r = row and c = column inside a tile. FP16: W1[4kt + r][4nt + c] is rom[5024 + 16 · (16nt + kt) + 4r + c], and W2[4kt + r][4ct + c] is rom[4096 + 16 · (3kt + ct) + 4r + c] (columns 10 and 11 are zero). INT8: rom[4480 + 4 · (16nt + kt) + r] holds four signed bytes, byte c = W1[4kt + r][4nt + c] / s[4nt + c] rounded; the FP32 scale s[j] is rom[4992 + j]. W2 has no INT8 form.\nNo starter. The post office is waiting. Fewer cycles earn the second star; fewer words read from the depot earn the third.",
            "栅纹的末层带着一个分拣网络，岑理要先证明它在复刻机上跑得安全。它读入一张外星字符的图，8 × 8 像素，值 0..255，报出 10 类中的一类。rom[64i .. 64i + 64] 是第 i 张图，每批 64 张，类号写到 out[i]。\n网络没有偏置：32 个隐藏单元，hidden[j] = max(0, 对 k 求和 W1[k][j] · x[k])；再对 10 类 logit[c] = 对 j 求和 W2[j][c] · hidden[j]。out[i] 是 logit 最大的类，并列取号小的。每一张都要和精确网络的答案一致，全白（全 255）的图也一样。\n权重在 rom 里存了两份，都切成 4 × 4 的块给张量单元用，r 为块内行、c 为块内列。FP16：W1[4kt + r][4nt + c] 在 rom[5024 + 16 · (16nt + kt) + 4r + c]，W2[4kt + r][4ct + c] 在 rom[4096 + 16 · (3kt + ct) + 4r + c]（第 10、11 列为 0）。INT8：rom[4480 + 4 · (16nt + kt) + r] 装四个有符号字节，第 c 个字节 = W1[4kt + r][4nt + c] / s[4nt + c] 取整；FP32 缩放 s[j] 在 rom[4992 + j]。W2 没有 INT8 形式。\n没有初始方案，邮局在等。周期少拿第二颗星，从仓库读的字少拿第三颗。",
        ),
        cases: || {
            vec![
                sorter_case(1481, 0),
                sorter_case(1482, 0),
                sorter_case(1483, 0),
            ]
        },
        hidden: || {
            vec![
                sorter_case(1484, 0),
                sorter_case(1485, 0),
                sorter_case(1486, 16),
            ]
        },
        expect: sorter_expect,
        limit: Some(Goal::new(Metric::Cycles, 29500)),
        goals: [
            Goal::new(Metric::Cycles, 25000),
            Goal::new(Metric::Loads, 5600),
        ],
        starter: Source::Code(
            "; no starter: the post office is waiting | 没有初始方案：邮局在等\n",
        ),
        reference: Source::Code(leak(sorter_ref(true))),
        live: Some(sorter_live),
        steer: Some(Tr::new("SET THE GLARE", "调亮度")),
        predict: Predict {
            question: Tr::new(
                "A hidden unit sums 64 white pixels (255 each) with weights near 4.4. The sum is 71800. What does it become after CVT.H.F?",
                "一个隐藏单元把 64 个白像素（各 255）乘上约 4.4 的权重再求和，和是 71800。CVT.H.F 之后它变成什么？",
            ),
            options: [
                Tr::new("71800, a little rounded", "71800，略有舍入"),
                Tr::new("infinity", "无穷"),
                Tr::new("a negative number", "一个负数"),
            ],
            answer: 1,
            why: Tr::new(
                "FP16 stops at 65504 and anything beyond becomes infinity. The next layer then multiplies infinity by zero weights and gets NaN. Scale the pixels down first.",
                "FP16 只到 65504，再大就是无穷。下一层再拿无穷去乘零权重，得到 NaN。要先把像素缩小。",
            ),
        },
        hints: [
            Tr::new(
                "Each half-warp is its own 4 x 4 tile: give half-warp h the pictures 8i + 4h .. 8i + 4h + 3 and the same weight tile. The result D has the layout the next layer wants as its A tile.",
                "每个半 warp 是独立的 4 × 4 块：半 warp h 负责第 8i + 4h .. 8i + 4h + 3 张图，两边用同一块权重。结果 D 的排布，正好是下一层要的 A 块。",
            ),
            Tr::new(
                "White frames push the hidden sums past 65504. Multiply each pixel by 1/256 (HMUL by 7168) right after CVT.H.I: ReLU is homogeneous, so the winner does not change.",
                "白图会把隐藏层的和推过 65504。CVT.H.I 之后立刻把每个像素乘 1/256（HMUL 乘 7168）：ReLU 是齐次的，赢家不会变。",
            ),
            Tr::new(
                "Packed path: LDS the word, SHL by 24 - 8c, SHR by 24, CVT.H.I, MMA; multiply the FP32 sum by the channel scale after the K loop, then ReLU. Argmax: FMAX butterfly, then the lowest class equal to the maximum.",
                "打包路径：LDS 取字，SHL 24 - 8c，SHR 24，CVT.H.I，MMA；K 循环结束后把 FP32 和乘该通道的缩放，再做 ReLU。argmax：FMAX 蝶形，再取等于最大值的最小类号。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Fewer bits, same answer", "位数少了，答案不变"),
            line: Tr::new(
                "Weights survive eight bits. Sums and activations need their range and a wide accumulator.",
                "权重撑得住八位。求和与激活需要量程和宽累加器。",
            ),
            reality: Tr::new(
                "Inference kernels store weights as INT8 or INT4 with one scale per channel and accumulate in FP32; the price is a dequantize step, and the test set must include the worst inputs.",
                "推理内核把权重存成 INT8 或 INT4，每个通道一个缩放，用 FP32 累加；代价是多一步反量化，测试集必须带上最坏的输入。",
            ),
            myth: Tr::new(
                "Myth: fewer bits always cost accuracy. Rounded weights barely move the answer; an overflowing sum or a clamped activation destroys it.",
                "误解：位数少了准确率必降。舍入的权重几乎不动答案；溢出的和、被钳住的激活才会毁掉它。",
            ),
            simplified: Tr::new(
                "OC-1 packs only W1 and applies the scale after the MMA; real kernels pack both layers and fold the scale into the epilogue.",
                "OC-1 只打包 W1，缩放在 MMA 之后再乘；真实内核两层都打包，把缩放并进收尾阶段。",
            ),
        },
        ..lbase("the-sorter", 14, 8, Launch::ONE_WARP, S_B, LYRA_OPS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grade::{Verdict, grade};

    fn run(t: &Task, src: &'static str) -> crate::grade::Report {
        grade(t, &Source::Code(src).solution()).unwrap()
    }

    fn code(s: &Source) -> &'static str {
        match s {
            Source::Code(c) => c,
            _ => unreachable!(),
        }
    }

    #[test]
    fn half_slow_loop_is_right_but_misses_the_second_star() {
        let t = half();
        let rep = run(&t, HALF_SLOW);
        assert!(matches!(rep.verdict, Verdict::Pass), "{:?}", rep.verdict);
        assert_eq!(rep.stars(), 2);
        // the all-FP16 starter is wrong already on the public cases
        let st = run(&t, code(&t.starter));
        assert!(
            matches!(st.verdict, Verdict::Wrong { case: 0..=2, .. }),
            "{:?}",
            st.verdict
        );
    }

    #[test]
    fn absorb_sequential_sum_swallows_the_ones() {
        let t = the_absorb();
        let st = run(&t, code(&t.starter));
        assert!(
            matches!(
                st.verdict,
                Verdict::Wrong {
                    case: 0,
                    cell: 0,
                    got: 2048,
                    want: 2111
                }
            ),
            "{:?}",
            st.verdict
        );
    }

    #[test]
    fn softmax_without_the_shift_breaks_on_the_big_rows() {
        let t = overflow();
        let st = run(&t, code(&t.starter));
        assert!(
            matches!(st.verdict, Verdict::Wrong { case: 1, .. }),
            "{:?}",
            st.verdict
        );
    }

    #[test]
    fn int8_accumulator_wraps_and_the_wide_sum_is_slower() {
        let t = the_int8();
        let st = run(&t, code(&t.starter));
        assert!(
            matches!(st.verdict, Verdict::Wrong { case: 0, .. }),
            "{:?}",
            st.verdict
        );
        let slow = run(&t, leak(q_slow()));
        assert!(matches!(slow.verdict, Verdict::Pass), "{:?}", slow.verdict);
        assert_eq!(slow.stars(), 2);
    }

    #[test]
    fn fixed_scale_is_fine_on_the_calibration_range_only() {
        let t = the_clipped();
        let st = run(&t, code(&t.starter));
        assert!(
            matches!(st.verdict, Verdict::Wrong { case: 1, .. }),
            "{:?}",
            st.verdict
        );
    }

    #[test]
    fn sorter_fp16_route_is_right_but_reads_twice_the_words() {
        let t = the_sorter();
        let a = run(&t, code(&t.reference));
        assert_eq!((a.stars(), a.measure.loads), (3, 5024));
        let b = run(&t, leak(sorter_ref(false)));
        assert!(matches!(b.verdict, Verdict::Pass), "{:?}", b.verdict);
        assert_eq!(b.stars(), 2);
        assert!(b.measure.loads > a.measure.loads + a.measure.loads / 8);
    }

    #[test]
    fn sorter_unscaled_pixels_overflow_only_on_the_white_frames() {
        let t = the_sorter();
        let raw = leak(sorter_ref(false).replace("HMUL r15, r15, 7168\n", ""));
        let rep = run(&t, raw);
        // the three public batches and the two ordinary hidden ones are fine
        assert!(
            matches!(rep.verdict, Verdict::Wrong { case: 5, .. }),
            "{:?}",
            rep.verdict
        );
    }

    #[test]
    fn sorter_margins_leave_room_for_eight_bit_weights() {
        for seed in 1481..1495u64 {
            let c = sorter_case(seed, 0);
            for i in 0..S_B {
                let mut l = sorter_logits(&c.rom[64 * i..64 * i + 64]);
                l.sort_by(|a, b| b.total_cmp(a));
                assert!(l[0] > 1.5 * l[1], "seed {seed} picture {i}");
            }
        }
    }
}
