//! Vault chapter 09: textures and image processing (the-texel .. the-xray).
//!
//! Textures live in `rom` with a row pitch of 32 words, so every row is one cache
//! line: `rom[y*32 + x]`. The small ones keep their side as log2 in `rom[1024]`.

use oo_base::Tr;

use super::prism::rom_case;
use super::vault::{ALL_WARPS, CACHE_OPS, SMEM_OPS, VAULT_OPS, base};
use super::{below, rng};
use crate::isa::{Alu, OpKind};
use crate::task::*;

const fn alu(a: Alu) -> OpKind {
    OpKind::Alu(a)
}

/// Chapter 7 palette plus LERP, for the sampling levels.
const LERP_OPS: &[OpKind] = &[
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
    OpKind::Ldo,
    OpKind::St,
    OpKind::Nop,
    OpKind::Shfl,
    OpKind::Vote,
    OpKind::Lerp,
];

/// Texture patterns, `t` × `t`, row-major, values 0..=255.
fn texture(kind: u8, t: usize, seed: u64) -> Vec<i32> {
    let mut r = rng(seed);
    let n = t as i32;
    let block = (n / 4).max(1);
    (0..n * n)
        .map(|i| {
            let (x, y) = (i % n, i / n);
            match kind {
                0 => x * 255 / (n - 1),
                1 => (x + y) * 255 / (2 * n - 2),
                2 => {
                    if (x / block + y / block) % 2 == 0 {
                        30
                    } else {
                        225
                    }
                }
                3 => below(&mut r, 256),
                4 => {
                    if (x, y) == (n / 2, n / 2) {
                        255
                    } else {
                        0
                    }
                }
                // Right edge bright.
                5 => {
                    if x == n - 1 {
                        255
                    } else {
                        10 + x * 2
                    }
                }
                // Bottom edge bright.
                6 => {
                    if y == n - 1 {
                        255
                    } else {
                        10 + y * 2
                    }
                }
                // Four edges of different brightness.
                _ => {
                    if x == 0 {
                        10
                    } else if x == n - 1 {
                        250
                    } else if y == 0 {
                        130
                    } else if y == n - 1 {
                        200
                    } else {
                        60 + (x * 3 + y * 5) % 64
                    }
                }
            }
        })
        .collect()
}

/// A case holding one texture: pitch-32 rows, then log2 of the side at `rom[1024]`.
fn tex_case(kind: u8, lt: u32, seed: u64) -> Case {
    let t = 1usize << lt;
    let tex = texture(kind, t, seed);
    let mut rom = vec![0; 1025];
    for y in 0..t {
        for x in 0..t {
            rom[y * 32 + x] = tex[y * t + x];
        }
    }
    rom[1024] = lt as i32;
    rom_case(rom)
}

fn texel(rom: &[i32], x: i64, y: i64) -> i64 {
    rom[(y * 32 + x) as usize] as i64
}

/// Nearest texel under the pixel centre.
fn nearest(c: &Case) -> Vec<Option<i32>> {
    let t = 1i64 << c.rom[1024];
    (0..1024i64)
        .map(|i| {
            let (x, y) = (i % 32, i / 32);
            let (tx, ty) = (((2 * x + 1) * t) >> 6, ((2 * y + 1) * t) >> 6);
            Some(texel(&c.rom, tx, ty) as i32)
        })
        .collect()
}

/// Q16 sample coordinate of output pixel `c` on a 32-wide output, clamped to the texture.
fn sample_at(c: i64, lt: i64) -> i64 {
    let t = 1i64 << lt;
    (((2 * c + 1) * t) * 1024 - 32768).clamp(0, (t - 1) << 16)
}

/// Bilinear stretch with clamped edges: the horizontal blend, then the vertical.
fn bilinear(c: &Case) -> Vec<Option<i32>> {
    let lt = c.rom[1024] as i64;
    let t = 1i64 << lt;
    (0..1024i64)
        .map(|i| {
            let (sx, sy) = (sample_at(i % 32, lt), sample_at(i / 32, lt));
            let (x0, y0, fx, fy) = (sx >> 16, sy >> 16, sx & 65535, sy & 65535);
            let (x1, y1) = ((x0 + 1).min(t - 1), (y0 + 1).min(t - 1));
            let top = (texel(&c.rom, x0, y0) * (65536 - fx) + texel(&c.rom, x1, y0) * fx) >> 16;
            let bot = (texel(&c.rom, x0, y1) * (65536 - fx) + texel(&c.rom, x1, y1) * fx) >> 16;
            Some(((top * (65536 - fy) + bot * fy) >> 16) as i32)
        })
        .collect()
}

fn the_texel() -> Task {
    Task {
        title: Tr::new("The Texel", "纹素"),
        brief: Tr::new(
            "Bohe's gallery keeps its pictures as small textures. rom[y*32 + x] is the texel in column x of row y (one row per cache line), and rom[1024] is log2 of the texture's side T, which is 8, 16 or 32.\nStretch the texture over the 32×32 output, out[y*32 + x]. Each output pixel shows the texel under its centre: column ((2x+1)·T) >> 6, row ((2y+1)·T) >> 6. Read T from rom; do not hard-code 16.",
            "薄荷的画廊把画存成小纹理。rom[y*32 + x] 是第 y 行第 x 列的纹素（一行正好一个缓存行），rom[1024] 是纹理边长 T 的 log2，T 为 8、16 或 32。\n把纹理拉伸铺满 32×32 的输出 out[y*32 + x]。每个输出像素显示它中心下面的那个纹素：列 ((2x+1)·T) >> 6，行 ((2y+1)·T) >> 6。T 要从 rom 里读，不能写死 16。",
        ),
        cases: || {
            vec![
                tex_case(0, 4, 901),
                tex_case(2, 4, 902),
                tex_case(3, 4, 903),
            ]
        },
        hidden: || {
            vec![
                tex_case(3, 3, 904),
                tex_case(1, 5, 905),
                tex_case(7, 4, 906),
            ]
        },
        expect: nearest,
        goals: [Goal::new(Metric::Cycles, 1700), Goal::new(Metric::Size, 18)],
        starter: Source::Code(
            "; texel (x, y) of the texture for pixel (x, y) | 像素 (x, y) 直接取纹素 (x, y)\nMOV r0, tid\nLOOP 2\n  LD r1, [r0]\n  ST [r0], r1\n  ADD r0, r0, 512\nENDLOOP\n",
        ),
        reference: Source::Code(
            "LD r0, [1024]\nSHL r1, lane, 1\nADD r1, r1, 1\nSHL r1, r1, r0\nSHR r1, r1, 6\nSHL r4, warp, 1\nADD r4, r4, 1\nMOV r5, tid\nLOOP 2\n  SHL r2, r4, r0\n  SHR r2, r2, 6\n  SHL r2, r2, 5\n  ADD r2, r2, r1\n  LD r3, [r2]\n  ST [r5], r3\n  ADD r5, r5, 512\n  ADD r4, r4, 32\nENDLOOP\n",
        ),
        predict: Predict {
            question: Tr::new(
                "A 16-wide texture is stretched over 32 output columns. Which texture column does output column 5 show?",
                "16 宽的纹理拉伸铺满 32 列输出。输出第 5 列显示纹理的第几列？",
            ),
            options: [
                Tr::new("Column 2", "第 2 列"),
                Tr::new("Column 3", "第 3 列"),
                Tr::new("Column 5", "第 5 列"),
            ],
            answer: 0,
            why: Tr::new(
                "Column 5's centre is at 5.5 of 32, which is 2.75 of 16: it lies inside texel 2. Using the pixel number as the texel number would give 5 and show the picture at the wrong scale.",
                "第 5 列的中心在 32 宽里的 5.5 处，换成 16 宽就是 2.75，落在第 2 个纹素里。把像素号直接当纹素号会得到 5，画面比例就错了。",
            ),
        },
        hints: [
            Tr::new(
                "The starter copies rom[i] to out[i]: it never scales. Run it and look at where the output stops looking like the texture.",
                "初始方案把 rom[i] 原样抄到 out[i]，完全没有缩放。运行一下，看输出从哪里开始不像纹理了。",
            ),
            Tr::new(
                "Map one axis at a time: texel = ((2·pixel + 1) << log2T) >> 6. The column depends only on lane, so work it out once, outside the loop.",
                "一次只处理一个轴：纹素 = ((2·像素 + 1) << log2T) >> 6。列只取决于车道号，在循环外算一次就够了。",
            ),
            Tr::new(
                "Rows are 32 words apart, so the address is (texel row << 5) + texel column. The two rows a thread paints are 16 rows apart: y = warp and y = warp + 16.",
                "行与行相隔 32 个字，所以地址是 (纹素行 << 5) + 纹素列。一个线程画的两行相隔 16 行：y = warp 与 y = warp + 16。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Nearest-texel sampling", "最近邻取样"),
            line: Tr::new(
                "A texture lookup turns a position into an index: scale the coordinate, drop the fraction, read one word.",
                "纹理查找就是把位置变成下标：缩放坐标，去掉小数，读一个字。",
            ),
            reality: Tr::new(
                "GPUs call this point sampling (D3D) or GL_NEAREST. Hardware texture units compute the index for you and fetch through a texture cache.",
                "GPU 把它叫点采样（D3D）或 GL_NEAREST。硬件纹理单元替你算下标，并经纹理缓存取数。",
            ),
            myth: Tr::new(
                "Myth: pixel x simply reads texel x. That only holds when the picture and the screen have the same size.",
                "误解：像素 x 就读纹素 x。只有画和屏幕一样大时才成立。",
            ),
            simplified: Tr::new(
                "Real units take normalised coordinates 0..1 and handle wrap and clamp modes in hardware. Here the scaling is yours.",
                "真实的纹理单元接收 0..1 的归一化坐标，并在硬件里处理回绕和钳位。这里缩放要你自己做。",
            ),
        },
        ..base("the-texel", 1, ALL_WARPS, 1024, CACHE_OPS)
    }
}

/// Q16 bilinear stretch, shared by the reference of 9-2 and the starter of 9-4.
const BILERP: &str = "LD r0, [1024]\nADD r1, r0, 11\nSHL r2, 1024, r0\nSUB r2, r2, 32768\nSHL r15, lane, r1\nADD r15, r15, r2\nSHL r13, warp, r1\nADD r13, r13, r2\nSHL r14, 16, r1\nADD r3, r0, 16\nSHL r11, 1, r3\nSUB r11, r11, 65536\nMAX r15, r15, 0\nMIN r15, r15, r11\nSHR r10, r15, 16\nAND r5, r15, 65535\nSUB r4, 65536, r5\nMOV r12, tid\nLOOP 2\n  MAX r15, r13, 0\n  MIN r15, r15, r11\n  SHR r7, r15, 16\n  SHL r7, r7, 5\n  ADD r7, r7, r10\n  LD r0, [r7]\n  ADD r8, r7, 1\n  LD r1, [r8]\n  ADD r7, r7, 32\n  LD r2, [r7]\n  ADD r8, r7, 1\n  LD r3, [r8]\n  LERP r7, r0, r4\n  LERP r8, r2, r4\n  AND r1, r15, 65535\n  SUB r0, 65536, r1\n  LERP r15, r7, r0\n  ST [r12], r15\n  ADD r12, r12, 512\n  ADD r13, r13, r14\nENDLOOP\n";

fn the_bilinear() -> Task {
    Task {
        title: Tr::new("Borrowed Colour", "借色"),
        brief: Tr::new(
            "Same gallery, smoother brush. Stretch the texture over the 32×32 output again, but blend instead of picking: pixel x samples the texture at s = (2x+1)·T/64 − ½ texels, where texel centres are the whole numbers. That is usually between four centres.\nLERP the four neighbours by distance: left and right by the fraction of s across, then top and bottom by the fraction down. Outside the picture the edge texel repeats (clamp). Layout as in the last level; each result may be off by at most 1.\nLERP d, a, b gives (a0·b0 + a1·b1 + a2·b2) >> 16 over r[a..a+2] and r[b..b+2]. Use Q16 weights that add up to 65536 and keep the third weight at zero.",
            "同一间画廊，换更柔和的笔。再把纹理铺满 32×32 的输出，这次不挑最近的，而是混合：像素 x 在纹理上的取样位置是 s = (2x+1)·T/64 − ½ 个纹素，纹素中心是整数位置，所以它通常落在四个中心之间。\n按距离对四个邻居做 LERP：先按横向的小数部分混合左右，再按纵向的小数部分混合上下。图外的部分重复边缘纹素（钳位）。布局与上一关相同；每个结果允许差 1。\nLERP d, a, b 得到 (a0·b0 + a1·b1 + a2·b2) >> 16，其中 a0..a2 取自 r[a..a+2]，b0..b2 取自 r[b..b+2]。权重用 Q16，两项相加为 65536，第三个权重保持为零。",
        ),
        cases: || {
            vec![
                tex_case(0, 4, 911),
                tex_case(1, 4, 912),
                tex_case(4, 4, 913),
            ]
        },
        hidden: || {
            vec![
                tex_case(3, 3, 914),
                tex_case(7, 5, 915),
                tex_case(5, 4, 916),
            ]
        },
        expect: bilinear,
        tolerance: 1,
        goals: [Goal::new(Metric::Cycles, 2150), Goal::new(Metric::Size, 42)],
        starter: Source::Code(
            "; nearest texel, from the last level | 上一关的最近邻\nLD r0, [1024]\nSHL r1, lane, 1\nADD r1, r1, 1\nSHL r1, r1, r0\nSHR r1, r1, 6\nSHL r4, warp, 1\nADD r4, r4, 1\nMOV r5, tid\nLOOP 2\n  SHL r2, r4, r0\n  SHR r2, r2, 6\n  SHL r2, r2, 5\n  ADD r2, r2, r1\n  LD r3, [r2]\n  ST [r5], r3\n  ADD r5, r5, 512\n  ADD r4, r4, 32\nENDLOOP\n",
        ),
        reference: Source::Code(BILERP),
        predict: Predict {
            question: Tr::new(
                "The sample point is exactly in the middle of four texel centres. What does the blend give?",
                "取样点正好落在四个纹素中心的正中间。混合的结果是什么？",
            ),
            options: [
                Tr::new("The average of the four texels", "四个纹素的平均值"),
                Tr::new("The nearest of the four", "离得最近的那一个"),
                Tr::new("The largest of the four", "四个里最大的那一个"),
            ],
            answer: 0,
            why: Tr::new(
                "Halfway across and halfway down, every weight is ½ × ½ = ¼. All four texels count equally, so the result is their average.",
                "横向走了一半、纵向也走了一半，每个权重都是 ½ × ½ = ¼。四个纹素贡献相同，结果就是它们的平均值。",
            ),
        },
        hints: [
            Tr::new(
                "Work in Q16: s = ((2x+1) << (10 + log2T)) − 32768. The top bits (>> 16) are the texel number, the low 16 bits are the fraction.",
                "用 Q16 计算：s = ((2x+1) << (10 + log2T)) − 32768。高位 (>> 16) 是纹素号，低 16 位是小数部分。",
            ),
            Tr::new(
                "Clamp s itself between 0 and (T−1) << 16 with MAX and MIN. Then the right and lower neighbours can only be read with weight 0 at the edge.",
                "用 MAX 和 MIN 把 s 本身夹在 0 与 (T−1) << 16 之间。这样在边缘处，右边和下边的邻居只会带着 0 权重被读到。",
            ),
            Tr::new(
                "Four LDs at a, a+1, a+32, a+33. LERP the two rows with the horizontal weights (w0, w1, 0), then LERP the results with the vertical weights. The horizontal part depends only on lane: do it before the loop.",
                "四个 LD 取 a、a+1、a+32、a+33。先用横向权重 (w0, w1, 0) 对两行各做一次 LERP，再用纵向权重对两个结果 LERP。横向部分只取决于车道号：放到循环之前去算。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Bilinear filtering", "双线性滤波"),
            line: Tr::new(
                "A sample between texel centres is a weighted mix of its four neighbours; the weights are the distances, and the blend is two LERPs and one more.",
                "落在纹素中心之间的取样，是四个邻居的加权混合；权重就是距离，混合就是两次横向 LERP 加一次纵向 LERP。",
            ),
            reality: Tr::new(
                "Texture units do exactly this in fixed hardware, with a few fractional bits of weight precision (commonly 8), fetching the four texels in one cache-friendly gather.",
                "纹理单元在固定硬件里做的就是这件事，权重只有几位小数精度（常见为 8 位），四个纹素在一次对缓存友好的聚集读取中取回。",
            ),
            myth: Tr::new(
                "Myth: nearest and bilinear differ only in looks. Bilinear reads four times as much memory per sample.",
                "误解：最近邻和双线性只是外观不同。双线性每个样本读的内存是前者的四倍。",
            ),
            simplified: Tr::new(
                "OC-1 has no texture unit: coordinates, clamping and the weights are all your code, and LERP only does the multiply-add.",
                "OC-1 没有纹理单元：坐标、钳位、权重都是你的代码，LERP 只负责乘加。",
            ),
        },
        ..base("the-bilinear", 2, ALL_WARPS, 1024, LERP_OPS)
    }
}

/// 64×64 pictures for the mip level, row pitch 64.
fn mip_pic(kind: u8, seed: u64) -> Vec<i32> {
    let mut r = rng(seed);
    (0..4096)
        .map(|i: i32| {
            let (x, y) = (i % 64, i / 64);
            match kind {
                // Horizontal bars, period 2.
                0 => {
                    if y % 2 == 0 {
                        240
                    } else {
                        16
                    }
                }
                // Checkerboard of single pixels.
                1 => {
                    if (x + y) % 2 == 0 {
                        240
                    } else {
                        16
                    }
                }
                2 => below(&mut r, 256),
                // The bars shifted by one row.
                3 => {
                    if y % 2 == 1 {
                        240
                    } else {
                        16
                    }
                }
                // Vertical bars, period 4.
                4 => {
                    if x % 4 < 2 {
                        250
                    } else {
                        6
                    }
                }
                // Two frequencies on top of each other.
                _ => {
                    (if y % 2 == 0 { 120 } else { 0 })
                        + (if x % 8 < 4 { 100 } else { 0 })
                        + below(&mut r, 24)
                }
            }
        })
        .collect()
}

fn mip_case(kind: u8, seed: u64) -> Case {
    rom_case(mip_pic(kind, seed))
}

/// Three layers of 2×2 averages, each rounded down: 64 → 32 → 16 → 8.
fn mip_expect(c: &Case) -> Vec<Option<i32>> {
    let mut img = c.rom.clone();
    let mut n = 64usize;
    while n > 8 {
        let m = n / 2;
        img = (0..m * m)
            .map(|i| {
                let (x, y) = (i % m, i / m);
                let at = |dx: usize, dy: usize| img[(2 * y + dy) * n + 2 * x + dx];
                (at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1)) >> 2
            })
            .collect();
        n = m;
    }
    let mut v = vec![None; 1024];
    for (i, p) in img.iter().enumerate() {
        v[(i / 8) * 32 + i % 8] = Some(*p);
    }
    v
}

pub fn tasks() -> Vec<Task> {
    vec![
        the_texel(),
        the_bilinear(),
        the_mip(),
        half_texel(),
        the_separable(),
        the_equalize(),
        wrap_mode(),
        the_xray(),
    ]
}

fn the_mip() -> Task {
    Task {
        title: Tr::new("The Far Picture", "远处的画"),
        brief: Tr::new(
            "Bohe hangs a 64×64 picture far down the hall, where it must be drawn only 8×8. rom[y*64 + x] is the picture (rows are 64 apart here).\nDo not just pick one pixel in eight: shrink it by halves. Three times, replace every 2×2 block by its average, (a+b+c+d) >> 2, rounding down: 64 → 32 → 16 → 8. Write the 8×8 result to out[y*32 + x] (x, y < 8); the other cells of out are scratch.\nEach result may be off by at most 2 (rounding down at every layer loses a little).\nShared memory is 8192 words, 32 banks, and BAR makes every warp wait for the others.",
            "薄荷把一幅 64×64 的画挂在走廊尽头，只需要画成 8×8。rom[y*64 + x] 是这幅画（这里行距是 64）。\n不要每 8 个像素只挑 1 个：一半一半地缩。连做三次，把每个 2×2 的块换成它们的平均值 (a+b+c+d) >> 2，向下取整：64 → 32 → 16 → 8。把 8×8 的结果写到 out[y*32 + x]（x、y < 8）；out 的其余单元是草稿区。\n每个结果允许差 2（每层都向下取整，会各丢一点）。\n共享内存有 8192 个字、32 个 bank，BAR 让所有 warp 等齐。",
        ),
        cases: || vec![mip_case(0, 921), mip_case(1, 922), mip_case(2, 923)],
        hidden: || vec![mip_case(3, 924), mip_case(4, 925), mip_case(5, 926)],
        expect: mip_expect,
        tolerance: 2,
        goals: [
            Goal::new(Metric::Cycles, 4800),
            Goal::new(Metric::Loads, 4096),
        ],
        starter: Source::Code(
            "; one texel in eight, straight from the picture | 每 8 个像素取 1 个\nSETP.LT p0, tid, 64\nIF p0\n  SHR r1, tid, 3\n  SHL r1, r1, 9\n  AND r2, tid, 7\n  SHL r3, r2, 3\n  ADD r3, r3, r1\n  LD r4, [r3]\n  SHR r1, tid, 3\n  SHL r1, r1, 5\n  ADD r1, r1, r2\n  ST [r1], r4\nENDIF\n",
        ),
        reference: Source::Code(
            "MOV r0, tid\nLOOP 8\n  LD r1, [r0]\n  STS [r0], r1\n  ADD r0, r0, 512\nENDLOOP\nBAR\nMOV r0, tid\nLOOP 2\n  SHR r1, r0, 5\n  SHL r1, r1, 7\n  AND r2, r0, 31\n  SHL r2, r2, 1\n  ADD r1, r1, r2\n  LDS r3, [r1]\n  ADD r2, r1, 1\n  LDS r4, [r2]\n  ADD r2, r1, 64\n  LDS r5, [r2]\n  ADD r2, r1, 65\n  LDS r6, [r2]\n  ADD r3, r3, r4\n  ADD r5, r5, r6\n  ADD r3, r3, r5\n  SHR r3, r3, 2\n  ADD r1, r0, 4096\n  STS [r1], r3\n  ADD r0, r0, 512\nENDLOOP\nBAR\nSETP.LT p0, tid, 256\nIF p0\n  SHR r1, tid, 4\n  SHL r1, r1, 6\n  AND r2, tid, 15\n  SHL r2, r2, 1\n  ADD r1, r1, r2\n  ADD r1, r1, 4096\n  LDS r3, [r1]\n  ADD r2, r1, 1\n  LDS r4, [r2]\n  ADD r2, r1, 32\n  LDS r5, [r2]\n  ADD r2, r1, 33\n  LDS r6, [r2]\n  ADD r3, r3, r4\n  ADD r5, r5, r6\n  ADD r3, r3, r5\n  SHR r3, r3, 2\n  ADD r1, tid, 5120\n  STS [r1], r3\nENDIF\nBAR\nSETP.LT p0, tid, 64\nIF p0\n  SHR r1, tid, 3\n  SHL r1, r1, 5\n  AND r2, tid, 7\n  SHL r3, r2, 1\n  ADD r1, r1, r3\n  ADD r1, r1, 5120\n  LDS r3, [r1]\n  ADD r4, r1, 1\n  LDS r4, [r4]\n  ADD r5, r1, 16\n  LDS r5, [r5]\n  ADD r6, r1, 17\n  LDS r6, [r6]\n  ADD r3, r3, r4\n  ADD r5, r5, r6\n  ADD r3, r3, r5\n  SHR r3, r3, 2\n  SHR r1, tid, 3\n  SHL r1, r1, 5\n  ADD r1, r1, r2\n  ST [r1], r3\nENDIF\n",
        ),
        predict: Predict {
            question: Tr::new(
                "A picture of black and white stripes, each one pixel wide, is shrunk 8× by keeping one pixel in eight. What does the 8×8 result look like?",
                "一张黑白条纹的画，每条 1 个像素宽，缩小 8 倍，办法是每 8 个像素只留 1 个。8×8 的结果是什么样？",
            ),
            options: [
                Tr::new("One solid colour", "纯色"),
                Tr::new("Middle grey", "中灰"),
                Tr::new("The stripes are still there", "条纹仍在"),
            ],
            answer: 0,
            why: Tr::new(
                "Every eighth row has the same parity, so every picked pixel is on a black stripe or every one is on a white stripe. The real picture averages to grey; averaging before shrinking is what gets that right.",
                "每隔 8 行的奇偶性相同，所以挑中的像素要么全在黑条上，要么全在白条上。这幅画真实的平均是灰色；先平均再缩小才能得到它。",
            ),
        },
        hints: [
            Tr::new(
                "The starter picks one pixel per 8×8 block. On stripes it gets one solid colour, on noise one random pixel. Run it and compare with the expected grid.",
                "初始方案每个 8×8 的块只挑一个像素。遇到条纹得到纯色，遇到噪声得到随机一个点。运行它，和期望的网格比一比。",
            ),
            Tr::new(
                "Shrink by halves: a layer of 2×2 averages is a 32×32 picture, then 16×16, then 8×8. Keep each layer in shared memory (picture at 0, layer 1 at 4096, layer 2 at 5120) so the picture is read from global memory only once.",
                "一半一半地缩：一层 2×2 平均得到 32×32，再是 16×16，再是 8×8。每一层放在共享内存里（原图在 0，第 1 层在 4096，第 2 层在 5120），这样原图只从全局内存读一次。",
            ),
            Tr::new(
                "A BAR must separate each layer from the next, and every warp has to reach it, so put it outside the IF that keeps the extra threads out of the small layers.",
                "每一层和下一层之间要放 BAR，而且每个 warp 都必须执行到它，所以把它放在那个让多余线程退出小层的 IF 外面。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Mip maps", "多级渐远纹理"),
            line: Tr::new(
                "Shrinking by picking pixels throws information away and shimmers; averaging first, one halving at a time, keeps the picture honest.",
                "靠挑像素来缩小会丢信息、还会闪烁；先平均、一次缩一半，画面才诚实。",
            ),
            reality: Tr::new(
                "GPUs store a whole chain of pre-shrunk copies of each texture and pick the level that matches the on-screen size; the chain costs only a third more memory.",
                "GPU 为每张纹理存一整串预先缩小的副本，并按屏幕上的大小选对应的层；这串副本只多占三分之一的内存。",
            ),
            myth: Tr::new(
                "Myth: shrinking is just sampling fewer pixels. Without averaging, thin detail turns into solid blocks or flicker.",
                "误解：缩小就是少采几个点。不先平均，细节要么变成整块纯色，要么闪烁。",
            ),
            simplified: Tr::new(
                "Real hardware also blends between two levels (trilinear) and uses the pixel footprint to choose one; here the level is fixed.",
                "真实硬件还会在两层之间混合（三线性），并根据像素覆盖范围选层；这里层数是固定的。",
            ),
        },
        ..base("the-mip", 3, ALL_WARPS, 1024, SMEM_OPS)
    }
}

fn half_texel() -> Task {
    Task {
        title: Tr::new("Half a Texel", "半格"),
        brief: Tr::new(
            "Bohe's enlarged pictures hang a little off: everything seems nudged toward the top left, and the edges look wrong. The kernel is the bilinear stretch of the last levels (32×32 output, texture side T = 8, 16 or 32 from rom[1024], layout as before, each result may be off by at most 1), but someone changed how the sample position is worked out.\nFind the first wrong pixel being written. Point at it in the Warp River, then fix the kernel.",
            "薄荷放大的画总是挂歪了：整幅图像被推向左上角，边缘也不对。这个内核就是前几关的双线性拉伸（输出 32×32，纹理边长 T = 8、16 或 32，取自 rom[1024]，布局同前，每个结果允许差 1），但有人改了取样位置的算法。\n找到第一个写错的像素，在 Warp River 里指出它，再修好内核。",
        ),
        cases: || {
            vec![
                tex_case(3, 4, 931),
                tex_case(0, 4, 932),
                tex_case(2, 3, 933),
            ]
        },
        hidden: || {
            vec![
                tex_case(1, 5, 934),
                tex_case(4, 3, 935),
                tex_case(7, 4, 936),
            ]
        },
        expect: bilinear,
        tolerance: 1,
        goals: [Goal::new(Metric::Cycles, 2150), Goal::new(Metric::Size, 42)],
        starter: Source::Code(
            "LD r0, [1024]\nADD r1, r0, 11\n; the pixel's corner, no half-texel shift | 取像素的角点，没有半格偏移\nMOV r2, 0\nSHL r15, lane, r1\nADD r15, r15, r2\nSHL r13, warp, r1\nADD r13, r13, r2\nSHL r14, 16, r1\nADD r3, r0, 16\nSHL r11, 1, r3\nSUB r11, r11, 65536\nMAX r15, r15, 0\nMIN r15, r15, r11\nSHR r10, r15, 16\nAND r5, r15, 65535\nSUB r4, 65536, r5\nMOV r12, tid\nLOOP 2\n  MAX r15, r13, 0\n  MIN r15, r15, r11\n  SHR r7, r15, 16\n  SHL r7, r7, 5\n  ADD r7, r7, r10\n  LD r0, [r7]\n  ADD r8, r7, 1\n  LD r1, [r8]\n  ADD r7, r7, 32\n  LD r2, [r7]\n  ADD r8, r7, 1\n  LD r3, [r8]\n  LERP r7, r0, r4\n  LERP r8, r2, r4\n  AND r1, r15, 65535\n  SUB r0, 65536, r1\n  LERP r15, r7, r0\n  ST [r12], r15\n  ADD r12, r12, 512\n  ADD r13, r13, r14\nENDLOOP\n",
        ),
        reference: Source::Code(BILERP),
        predict: Predict {
            question: Tr::new(
                "A 16-wide texture is stretched to 32 columns, but the sample position is taken at the pixel's left edge, x·T/32, instead of its centre. Which way does the picture seem to move?",
                "16 宽的纹理拉伸到 32 列，但取样位置取在像素的左边缘 x·T/32，而不是中心。画面看上去往哪边移？",
            ),
            options: [
                Tr::new("Up and to the left", "向左上"),
                Tr::new("Down and to the right", "向右下"),
                Tr::new("It does not move", "不动"),
            ],
            answer: 0,
            why: Tr::new(
                "The correct position for pixel x is x/2 + 1/4 − 1/2 = x/2 − 1/4 texels; the edge version gives x/2. Every pixel samples a quarter texel further right and lower, so it shows what lies there, and the picture slides up and left.",
                "像素 x 的正确位置是 x/2 + 1/4 − 1/2 = x/2 − 1/4 个纹素；边缘版本给出 x/2。每个像素都往右、往下多取了 1/4 个纹素，显示的是那里的内容，于是画面向左上滑。",
            ),
        },
        hints: [
            Tr::new(
                "The ×1 case is already right and the others are not, so the blend is fine. Compare a single bright dot with where it should be.",
                "×1 的用例已经是对的，其他的不对，所以混合本身没问题。拿单个亮点和它应该在的位置比一比。",
            ),
            Tr::new(
                "Pixel x covers x to x+1, and its centre is x + ½. In texels: s = (2x+1)·T/64 − ½. The starter has s = x·T/32.",
                "像素 x 覆盖 x 到 x+1，中心在 x + ½。换成纹素：s = (2x+1)·T/64 − ½。初始方案里是 s = x·T/32。",
            ),
            Tr::new(
                "In Q16 the starting offset is (1024 << log2T) − 32768. It is added to both lane << (11 + log2T) and warp << (11 + log2T) before clamping. The starter's MOV r2, 0 should compute it.",
                "用 Q16 的话，起始偏移是 (1024 << log2T) − 32768。钳位之前，它要加在 lane << (11 + log2T) 和 warp << (11 + log2T) 上。初始方案里那条 MOV r2, 0 应该把它算出来。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Pixel centres", "像素中心"),
            line: Tr::new(
                "A pixel is a small square, and it samples the world at its middle. Take the corner instead and the whole picture slides by a part of a texel.",
                "像素是一个小方块，它在自己的中央对世界取样。改取角点，整幅图就会滑开一小段纹素。",
            ),
            reality: Tr::new(
                "Direct3D 9 sampled at pixel corners and shifted every picture by half a pixel; D3D10 and later, OpenGL and Vulkan use centres. Old shaders carried a −0.5 patch for years.",
                "Direct3D 9 在像素角点取样，每张图都偏半个像素；D3D10 之后、OpenGL 和 Vulkan 都用中心。老着色器带着一句 −0.5 的补丁用了很多年。",
            ),
            myth: Tr::new(
                "Myth: shift the whole result by half a pixel and the bug is gone. The error is a different size at each scale and zero at ×1, so no single shift fits.",
                "误解：把结果整体挪半个像素就好了。误差在每个倍数下大小不同、在 ×1 时为零，没有哪个单一的平移能对上。",
            ),
            simplified: Tr::new(
                "Here the mistake is in one line of address maths; on real hardware it can sit in a driver, an engine or a texture tool.",
                "这里的错只在一行地址运算里；在真实硬件上，它可能藏在驱动、引擎或贴图工具里。",
            ),
        },
        clue: Some(Clue {
            what: Tr::new(
                "the first pixel written with the wrong colour",
                "第一个写错颜色的像素",
            ),
            find: crate::grade::first_wrong_write,
        }),
        ..base("half-texel", 4, ALL_WARPS, 1024, LERP_OPS)
    }
}

const BIN9: [i64; 9] = [1, 8, 28, 56, 70, 56, 28, 8, 1];

/// A `w`×`h` picture in the top-left of a 64×64 area (rows 64 apart), then `w` and `h`
/// at `rom[4096]` and `rom[4097]`.
fn sep_case(kind: u8, w: usize, h: usize, seed: u64) -> Case {
    let mut r = rng(seed);
    let mut rom = vec![0; 4098];
    for y in 0..h {
        for x in 0..w {
            rom[y * 64 + x] = match kind {
                0 => below(&mut r, 256),
                // Vertical stripes, 3 wide.
                1 => {
                    if (x / 3) % 2 == 0 {
                        230
                    } else {
                        20
                    }
                }
                // One bright dot on a dark ground.
                2 => {
                    if (x, y) == (w / 2, h / 2) {
                        255
                    } else {
                        10
                    }
                }
                // A bright line on every edge.
                3 => {
                    if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
                        255
                    } else {
                        below(&mut r, 90)
                    }
                }
                // Diagonal ramp with a bright last column.
                _ => {
                    if x == w - 1 {
                        255
                    } else {
                        ((x + y) * 2) as i32 % 200
                    }
                }
            };
        }
    }
    rom[4096] = w as i32;
    rom[4097] = h as i32;
    rom_case(rom)
}

/// Binomial 9×9 blur, edges clamped: sums along x unshifted, then along y, then >> 16.
fn blur(c: &Case) -> Vec<Option<i32>> {
    let (w, h) = (c.rom[4096] as i64, c.rom[4097] as i64);
    let at = |x: i64, y: i64| c.rom[(y * 64 + x) as usize] as i64;
    let horiz = |x: i64, y: i64| -> i64 {
        (0..9)
            .map(|k| BIN9[k] * at((x + k as i64 - 4).clamp(0, w - 1), y))
            .sum()
    };
    let mut v = vec![None; 4096];
    for y in 0..h {
        for x in 0..w {
            let s: i64 = (0..9)
                .map(|k| BIN9[k] * horiz(x, (y + k as i64 - 4).clamp(0, h - 1)))
                .sum();
            v[(y * 64 + x) as usize] = Some((s >> 16) as i32);
        }
    }
    v
}

fn the_separable() -> Task {
    Task {
        title: Tr::new("Two Passes", "拆成两遍"),
        brief: Tr::new(
            "Bohe wants the gallery pictures softened. rom[y*64 + x] is a grey picture of width W = rom[4096] and height H = rom[4097] (each at most 64); out[y*64 + x] gets the blurred one.\nThe blur is a 9×9 binomial kernel: the weight of tap (i, j) is w[i]·w[j], with w = 1 8 28 56 70 56 28 8 1 (they add up to 256, so both together 65536). Taps outside the picture repeat the edge pixel (clamp).\nThe kernel splits: blur every row with w, keeping the sums as they are, then every column of that with w, and shift right by 16 once at the end. The result matches the 2D kernel exactly. Cells outside the W×H picture are not checked.\nShared memory is 8192 words; BAR waits for every warp.",
            "薄荷想把画廊里的画变柔和。rom[y*64 + x] 是一幅灰度图，宽 W = rom[4096]，高 H = rom[4097]（都不超过 64）；模糊后的图写到 out[y*64 + x]。\n模糊用 9×9 的二项式核：第 (i, j) 个抽头的权重是 w[i]·w[j]，w = 1 8 28 56 70 56 28 8 1（和为 256，两个方向合起来是 65536）。图外的抽头重复边缘像素（钳位）。\n这个核可以拆开：先把每一行用 w 卷一遍，和不要移位；再把得到的每一列用 w 卷一遍，最后只右移 16 位一次。结果与二维核逐像素一致。W×H 之外的单元不检查。\n共享内存有 8192 个字；BAR 让所有 warp 等齐。",
        ),
        cases: || {
            vec![
                sep_case(0, 64, 64, 941),
                sep_case(1, 64, 64, 942),
                sep_case(2, 64, 64, 943),
            ]
        },
        hidden: || {
            vec![
                sep_case(3, 64, 32, 944),
                sep_case(4, 32, 64, 945),
                sep_case(3, 64, 64, 946),
            ]
        },
        expect: blur,
        goals: [
            Goal::new(Metric::Cycles, 36500),
            Goal::new(Metric::Loads, 5200),
        ],
        starter: Source::Code(
            "; the picture as it is, no blur yet | 原样输出，还没有模糊\nMOV r0, tid\nLOOP 8\n  LD r1, [r0]\n  ST [r0], r1\n  ADD r0, r0, 512\nENDLOOP\n",
        ),
        reference: Source::Code(
            "; the kernel weights, 1 8 28 56 70 56 28 8 1, into shared words 4096.. | 核权重放进共享内存\nSTS [4096], 1\nSTS [4097], 8\nSTS [4098], 28\nSTS [4099], 56\nSTS [4100], 70\nSTS [4101], 56\nSTS [4102], 28\nSTS [4103], 8\nSTS [4104], 1\n; the picture, once, into shared words 0..4095 | 图像只读一遍，放进共享内存\nMOV r0, tid\nLOOP 8\n  LD r1, [r0]\n  STS [r0], r1\n  ADD r0, r0, 512\nENDLOOP\nLD r10, [4096]\nLD r11, [4097]\nSUB r10, r10, 1\nSUB r11, r11, 1\nBAR\n; pass 0 blurs along x in place, pass 1 along y into out | 第 0 遍沿 x 就地模糊，第 1 遍沿 y 写入 out\nMOV r9, 0\nLOOP 2\n  SETP.EQ p1, r9, 0\n  SEL r8, p1, 1, 64\n  SEL r6, p1, r10, r11\n  MUL r6, r6, r8\n  MOV r12, warp\n  LOOP 4\n    MOV r13, lane\n    LOOP 2\n      SHL r5, r12, 6\n      SEL r14, p1, r13, r12\n      SEL r5, p1, r5, r13\n      MUL r14, r14, r8\n      SHL r15, r8, 2\n      SUB r2, r14, r15\n      MOV r4, 4096\n      MOV r0, 0\n      LOOP 9\n        MAX r3, r2, 0\n        MIN r3, r3, r6\n        ADD r3, r3, r5\n        LDS r1, [r3]\n        LDS r15, [r4]\n        MUL r1, r1, r15\n        ADD r0, r0, r1\n        ADD r2, r2, r8\n        ADD r4, r4, 1\n      ENDLOOP\n      IF p1\n        SHL r1, warp, 6\n        ADD r1, r1, r13\n        ADD r1, r1, 5120\n        STS [r1], r0\n      ELSE\n        SHR r0, r0, 16\n        SHL r1, r12, 6\n        ADD r1, r1, r13\n        ST [r1], r0\n      ENDIF\n      ADD r13, r13, 32\n    ENDLOOP\n    IF p1\n      SHL r1, warp, 6\n      ADD r1, r1, lane\n      ADD r1, r1, 5120\n      LDS r2, [r1]\n      ADD r1, r1, 32\n      LDS r3, [r1]\n      SHL r4, r12, 6\n      ADD r4, r4, lane\n      STS [r4], r2\n      ADD r4, r4, 32\n      STS [r4], r3\n    ENDIF\n    ADD r12, r12, 16\n  ENDLOOP\n  BAR\n  ADD r9, r9, 1\nENDLOOP\n",
        ),
        predict: Predict {
            question: Tr::new(
                "A 9×9 kernel is split into a row pass and a column pass of 9 taps each. How many multiply-adds does each output pixel need now?",
                "9×9 的核拆成一遍横向、一遍纵向，每遍 9 个抽头。现在每个输出像素要做多少次乘加？",
            ),
            options: [Tr::new("9", "9"), Tr::new("18", "18"), Tr::new("81", "81")],
            answer: 1,
            why: Tr::new(
                "9 for the row pass plus 9 for the column pass is 18, against 81 for the full 2D kernel. The saving grows with the kernel: 2n instead of n².",
                "横向 9 次加纵向 9 次，一共 18 次，而完整二维核要 81 次。核越大省得越多：是 2n 而不是 n²。",
            ),
        },
        hints: [
            Tr::new(
                "The starter just copies the picture. Doing 81 taps per pixel is out of reach: the machine gives up long before it finishes. Split the kernel.",
                "初始方案只是原样抄图。每个像素做 81 个抽头根本做不完：机器还没算完就放弃了。把核拆开。",
            ),
            Tr::new(
                "Both passes are the same loop: nine taps at f−4 … f+4 along one axis, clamped to 0 … size−1, weighted by w. Only the axis and the clamp limit change. Keep the weights in shared memory so the tap loop can read them.",
                "两遍其实是同一个循环：沿一根轴取 f−4 … f+4 九个抽头，钳位到 0 … 尺寸−1，再乘权重 w。变的只是轴和钳位上限。把权重放进共享内存，抽头循环就能读到它们。",
            ),
            Tr::new(
                "Read the picture from global memory once into shared memory (that is all the global reads you need). The row pass can write back in place if each warp owns its row and parks the result in a small buffer first; a BAR then separates the passes.",
                "把图从全局内存只读一遍，放进共享内存（全局读取只需要这么多）。如果每个 warp 管一行、先把结果放进一个小缓冲区，横向这一遍可以就地写回；两遍之间用 BAR 隔开。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Separable filters", "可分离滤波"),
            line: Tr::new(
                "A kernel that is a product of two 1D kernels can be applied as two 1D passes, turning n² multiplies into 2n.",
                "如果一个核是两个一维核的乘积，就能拆成两遍一维处理，把 n² 次乘法变成 2n 次。",
            ),
            reality: Tr::new(
                "Gaussian and box blurs in image editors and game bloom passes are all done as a horizontal and a vertical pass, often through shared memory tiles.",
                "图像编辑器里的高斯和方框模糊、游戏里的泛光，都是做成一遍横向加一遍纵向，常常借助共享内存分块。",
            ),
            myth: Tr::new(
                "Myth: caching makes the 2D loop fast enough. The reads become hits, but the 81 multiply-adds per pixel are still 81.",
                "误解：有缓存，二维循环就够快了。读取变成了命中，但每个像素 81 次乘加还是 81 次。",
            ),
            simplified: Tr::new(
                "Only some kernels split exactly; a general 2D kernel does not, and integer rounding between the passes can differ in the last bit.",
                "只有部分核能精确拆开；一般的二维核不行，两遍之间的整数舍入也可能在最后一位上有差别。",
            ),
        },
        ..base("the-separable", 5, ALL_WARPS, 4096, SMEM_OPS)
    }
}

/// 64×64 grey pictures, 4096 pixels, `rom[y*64 + x]`.
fn eq_case(kind: u8, seed: u64) -> Case {
    let mut r = rng(seed);
    let rom = (0..4096)
        .map(|i| match kind {
            // Low contrast: 80..=140.
            0 => 80 + below(&mut r, 61),
            // Two humps.
            1 => {
                if below(&mut r, 2) == 0 {
                    40 + below(&mut r, 21)
                } else {
                    190 + below(&mut r, 21)
                }
            }
            // Dark picture.
            2 => below(&mut r, 8) * below(&mut r, 8),
            // Nearly uniform.
            3 => below(&mut r, 256),
            // Almost all black, a few bright points.
            4 => {
                if i % 700 == 5 {
                    120 + below(&mut r, 136)
                } else {
                    0
                }
            }
            // One grey for the whole picture.
            _ => 100,
        })
        .collect();
    rom_case(rom)
}

/// Histogram equalisation: (cdf[v] - cdf_min) * 255 / (4096 - cdf_min), and 0 if every pixel is alike.
fn equalize(c: &Case) -> Vec<Option<i32>> {
    let mut hist = [0i64; 256];
    for &v in &c.rom {
        hist[v as usize] += 1;
    }
    let mut cdf = [0i64; 256];
    let mut run = 0;
    for (b, h) in hist.iter().enumerate() {
        run += h;
        cdf[b] = run;
    }
    let cmin = cdf.iter().copied().find(|&c| c > 0).unwrap_or(0);
    let den = 4096 - cmin;
    c.rom
        .iter()
        .map(|&v| {
            Some(if den == 0 {
                0
            } else {
                ((cdf[v as usize] - cmin) * 255 / den) as i32
            })
        })
        .collect()
}

fn the_equalize() -> Task {
    Task {
        title: Tr::new("Even Out", "均衡"),
        brief: Tr::new(
            "Bohe's scans are too flat: nearly every pixel is the same dull grey. rom[y*64 + x] is a 64×64 picture, grey levels 0..255. Spread them out: out[y*64 + x] = (cdf[v] − cdf_min) · 255 / (4096 − cdf_min), where v is the pixel, cdf[v] counts the pixels with grey ≤ v, and cdf_min is cdf at the darkest grey that occurs. Divide rounding down.\nIf every pixel is alike, the denominator is 0 and the answer is 0, not garbage. Each result may be off by at most 1, so RCP (65536 / x, and i32::MAX for 0) can stand in for the division.\nATOMS d, [a], v adds v to shared word a and gives back the old value. Counting takes a barrier; the running total over 256 buckets can be done by one lane.",
            "薄荷的扫描片太平了：几乎每个像素都是同一种灰。rom[y*64 + x] 是 64×64 的图，灰度 0..255。把它们拉开：out[y*64 + x] = (cdf[v] − cdf_min) · 255 / (4096 − cdf_min)，其中 v 是像素值，cdf[v] 是灰度 ≤ v 的像素个数，cdf_min 是出现过的最暗灰度处的 cdf。除法向下取整。\n如果全图所有像素都一样，分母为 0，答案应是 0，而不是乱值。每个结果允许差 1，所以可以用 RCP（65536 / x，0 时给 i32::MAX）来代替除法。\nATOMS d, [a], v 把 v 加到共享内存的字 a 上，并返回旧值。计数要靠屏障；256 个桶的累计可以由一个车道完成。",
        ),
        cases: || vec![eq_case(0, 951), eq_case(1, 952), eq_case(2, 953)],
        hidden: || vec![eq_case(3, 954), eq_case(4, 955), eq_case(5, 956)],
        expect: equalize,
        tolerance: 1,
        goals: [
            Goal::new(Metric::Cycles, 12000),
            Goal::new(Metric::Size, 52),
        ],
        starter: Source::Code(
            "; the picture as it is | 原样输出\nMOV r0, tid\nLOOP 8\n  LD r1, [r0]\n  ST [r0], r1\n  ADD r0, r0, 512\nENDLOOP\n",
        ),
        reference: Source::Code(
            "; histogram: every thread counts 8 pixels in shared words 0..255 | 直方图：每个线程数 8 个像素\nMOV r0, tid\nLOOP 8\n  LD r1, [r0]\n  ATOMS r2, [r1], 1\n  ADD r0, r0, 512\nENDLOOP\nBAR\n; one lane turns the counts into a running total and finds the first non-zero one | 一个车道把计数变成累计，并找出第一个非零值\nSETP.EQ p0, tid, 0\nIF p0\n  MOV r1, 0\n  MOV r2, 0\n  MOV r4, 65536\n  LOOP 4\n    LOOP 64\n      LDS r3, [r2]\n      ADD r1, r1, r3\n      STS [r2], r1\n      SETP.EQ p1, r1, 0\n      SEL r5, p1, 65536, r1\n      MIN r4, r4, r5\n      ADD r2, r2, 1\n    ENDLOOP\n  ENDLOOP\n  SUB r6, 4096, r4\n  RCP r7, r6\n  STS [256], r4\n  STS [257], r6\n  STS [258], r7\nENDIF\nBAR\nLDS r4, [256]\nLDS r6, [257]\nLDS r5, [258]\n; remap: (cdf - cdf_min) * 255 / (4096 - cdf_min), divided with RCP and one correction | 重映射，除法用 RCP 再修正一次\nMOV r0, tid\nLOOP 8\n  LD r1, [r0]\n  LDS r2, [r1]\n  SUB r2, r2, r4\n  MUL r2, r2, 255\n  MUL r3, r2, r5\n  SHR r3, r3, 16\n  MUL r8, r3, r6\n  SUB r8, r2, r8\n  MUL r8, r8, r5\n  SHR r8, r8, 16\n  ADD r3, r3, r8\n  ST [r0], r3\n  ADD r0, r0, 512\nENDLOOP\n",
        ),
        predict: Predict {
            question: Tr::new(
                "90% of a picture's pixels have grey levels between 100 and 120. After equalising, what happens to those pixels?",
                "一张图 90% 的像素灰度都在 100 到 120 之间。均衡之后，这些像素会怎样？",
            ),
            options: [
                Tr::new("They are spread far apart", "被拉开"),
                Tr::new("They hardly change", "基本不变"),
                Tr::new("They are squeezed together", "被压缩"),
            ],
            answer: 0,
            why: Tr::new(
                "The mapping follows the running total of pixels. Those 21 grey levels hold nine tenths of it, so they get nine tenths of the output range, about 230 levels instead of 21.",
                "映射跟着像素的累计数走。这 21 个灰度占了九成像素，于是它们分到九成的输出范围，大约 230 级，而不是 21 级。",
            ),
        },
        hints: [
            Tr::new(
                "The starter copies the picture. Stretching between the darkest and the brightest pixel is not the answer either: one bright speck would set the scale.",
                "初始方案原样抄图。按最暗和最亮的像素线性拉伸也不对：一粒亮点就会决定整个比例。",
            ),
            Tr::new(
                "Three steps with a BAR between each: every thread ATOMS 8 pixels into 256 shared counters; one lane walks the counters keeping a running total and the first non-zero total; then every thread looks its pixels up.",
                "三步，每步之间一个 BAR：每个线程把 8 个像素用 ATOMS 计入 256 个共享计数器；一个车道走过计数器，记下累计值和第一个非零的累计值；最后每个线程查自己的像素。",
            ),
            Tr::new(
                "RCP gives r = 65536 / den, a little too small. q = (num · r) >> 16 undershoots; add the same estimate of what is left: rem = num − q · den, then q += (rem · r) >> 16. With den = 0 the numerator is 0, so the answer comes out 0.",
                "RCP 给出 r = 65536 / den，略微偏小。q = (num · r) >> 16 会偏低；再对剩下的部分做同样的估计：rem = num − q · den，然后 q += (rem · r) >> 16。den = 0 时分子为 0，所以结果自然是 0。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Histogram equalisation", "直方图均衡"),
            line: Tr::new(
                "Count how many pixels are at or below each grey, and use that running share as the new grey: crowded levels get spread out.",
                "数出每个灰度及以下有多少像素，再把这个累计占比当作新的灰度：挤在一起的灰度就被拉开了。",
            ),
            reality: Tr::new(
                "Medical viewers, photo editors and cameras use it to bring out detail; the histogram is built with atomics or per-block copies, and the 256-entry scan is a classic parallel prefix sum.",
                "医学影像查看器、修图软件和相机用它来凸显细节；直方图用原子操作或每块私有副本来建，256 项的累计是经典的并行前缀和。",
            ),
            myth: Tr::new(
                "Myth: stretch the darkest pixel to black and the brightest to white. A single outlier then sets the scale and nothing improves.",
                "误解：把最暗的拉成黑、最亮的拉成白就行。一个离群点就会决定比例，画面毫无改善。",
            ),
            simplified: Tr::new(
                "One lane does the 256-step scan here; later chapters do it in parallel. RCP stands in for division, which OC-1 does not have.",
                "这里 256 步累计由一个车道完成，后面的章节会并行地做。OC-1 没有除法，用 RCP 代替。",
            ),
        },
        ..base("the-equalize", 6, ALL_WARPS, 4096, VAULT_OPS)
    }
}

fn wrap_mode() -> Task {
    Task {
        title: Tr::new("Edge Rule", "边界模式"),
        brief: Tr::new(
            "Bohe's enlarged pictures have grown bright lines along their borders: a bright right edge leaks onto the left edge, and the bottom onto the top. The kernel is the bilinear stretch again (32×32 output, texture side T = 8, 16 or 32 from rom[1024], layout as before, each result may be off by at most 1), but now it picks the four neighbours by index, and something is wrong at the picture's edge.\nFind the first pixel written with the wrong colour. Point at it in the Warp River, then fix the kernel so that the edge texel repeats beyond the picture (clamp).",
            "薄荷放大的画沿边出现了亮线：右边缘的亮色漏到了左边缘，下边缘漏到了上边缘。这个内核还是双线性拉伸（输出 32×32，纹理边长 T = 8、16 或 32，取自 rom[1024]，布局同前，每个结果允许差 1），不过这回是按下标挑四个邻居的，图的边缘处出了问题。\n找到第一个写错颜色的像素，在 Warp River 里指出它，再修好内核：图外的部分要重复边缘纹素（钳位）。",
        ),
        cases: || {
            vec![
                tex_case(5, 4, 961),
                tex_case(6, 4, 962),
                tex_case(7, 4, 963),
            ]
        },
        hidden: || {
            vec![
                tex_case(7, 3, 964),
                tex_case(5, 5, 965),
                tex_case(6, 3, 966),
            ]
        },
        expect: bilinear,
        tolerance: 1,
        goals: [Goal::new(Metric::Cycles, 2200), Goal::new(Metric::Size, 44)],
        starter: Source::Code(
            "LD r0, [1024]\nADD r1, r0, 11\nSHL r2, 1024, r0\nSUB r2, r2, 32768\nSHL r12, 1, r0\nSUB r12, r12, 1\nSHL r10, lane, r1\nADD r10, r10, r2\nSHL r14, warp, r1\nADD r14, r14, r2\nSHL r15, 16, r1\nAND r5, r10, 65535\nSUB r4, 65536, r5\nSHR r10, r10, 16\nADD r11, r10, 1\n; wrap round with AND (T-1) | 用 AND (T-1) 回绕\nAND r11, r11, r12\nAND r10, r10, r12\nMOV r13, tid\nLOOP 2\n  SHR r0, r14, 16\n  ADD r1, r0, 1\n  AND r1, r1, r12\n  AND r0, r0, r12\n  SHL r0, r0, 5\n  SHL r1, r1, 5\n  ADD r2, r0, r10\n  ADD r3, r0, r11\n  ADD r7, r1, r10\n  ADD r8, r1, r11\n  LD r0, [r2]\n  LD r1, [r3]\n  LD r2, [r7]\n  LD r3, [r8]\n  LERP r7, r0, r4\n  LERP r8, r2, r4\n  AND r1, r14, 65535\n  SUB r0, 65536, r1\n  LERP r0, r7, r0\n  ST [r13], r0\n  ADD r13, r13, 512\n  ADD r14, r14, r15\nENDLOOP\n",
        ),
        reference: Source::Code(
            "LD r0, [1024]\nADD r1, r0, 11\nSHL r2, 1024, r0\nSUB r2, r2, 32768\nSHL r12, 1, r0\nSUB r12, r12, 1\nSHL r10, lane, r1\nADD r10, r10, r2\nSHL r14, warp, r1\nADD r14, r14, r2\nSHL r15, 16, r1\nAND r5, r10, 65535\nSUB r4, 65536, r5\nSHR r10, r10, 16\nADD r11, r10, 1\nMIN r11, r11, r12\nMAX r10, r10, 0\nMOV r13, tid\nLOOP 2\n  SHR r0, r14, 16\n  ADD r1, r0, 1\n  MIN r1, r1, r12\n  MAX r0, r0, 0\n  SHL r0, r0, 5\n  SHL r1, r1, 5\n  ADD r2, r0, r10\n  ADD r3, r0, r11\n  ADD r7, r1, r10\n  ADD r8, r1, r11\n  LD r0, [r2]\n  LD r1, [r3]\n  LD r2, [r7]\n  LD r3, [r8]\n  LERP r7, r0, r4\n  LERP r8, r2, r4\n  AND r1, r14, 65535\n  SUB r0, 65536, r1\n  LERP r0, r7, r0\n  ST [r13], r0\n  ADD r13, r13, 512\n  ADD r14, r14, r15\nENDLOOP\n",
        ),
        predict: Predict {
            question: Tr::new(
                "A pixel near the right border of a 16-wide texture blends column 15 with a neighbour one step to the right, which is outside the picture. With clamping, what colour does that neighbour have?",
                "16 宽纹理右边界附近的像素，要把第 15 列和它右边一格的邻居混合，而那一格在图外。用钳位的话，这个邻居是什么颜色？",
            ),
            options: [
                Tr::new("Column 15 again", "还是第 15 列"),
                Tr::new("Column 0, from the other side", "从另一侧绕来的第 0 列"),
                Tr::new("Black", "黑色"),
            ],
            answer: 0,
            why: Tr::new(
                "Clamp means the edge texel goes on forever, so the blend is column 15 with itself: nothing changes. Wrapping would mix in column 0 and paint the opposite edge's colour onto the border.",
                "钳位的意思是边缘纹素一直延伸下去，所以混合的是第 15 列和它自己：什么也不变。回绕则会混进第 0 列，把对面边缘的颜色涂到边上。",
            ),
        },
        hints: [
            Tr::new(
                "Only pixels next to a border are wrong, and only when the texture is enlarged. Compare the first wrong pixel with the picture's edge column.",
                "只有紧挨着边界的像素是错的，而且只在纹理被放大时出错。拿第一个错的像素和图的边缘列比一比。",
            ),
            Tr::new(
                "The starter reduces the indices with AND (T−1). That wraps: −1 becomes T−1 and T becomes 0. Which instruction keeps an index from going below 0, and which one from going above T−1?",
                "初始方案用 AND (T−1) 来约束下标。那是回绕：−1 变成 T−1，T 变成 0。哪条指令能防止下标小于 0，哪条能防止大于 T−1？",
            ),
            Tr::new(
                "Replace the two ANDs of each axis: the left index with MAX against 0, the right index (x0 + 1) with MIN against T−1. The weights stay as they are.",
                "把每个轴上的两个 AND 换掉：左下标用 MAX 对 0，右下标（x0 + 1）用 MIN 对 T−1。权重不用动。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Edge rules", "边界规则"),
            line: Tr::new(
                "Beyond a texture's edge you must say what exists: the edge repeated, the picture tiled, or a mirror. Each is a different line of index maths.",
                "纹理的边缘之外有什么，必须你来规定：重复边缘、平铺整幅图，或是镜像。每一种都是不同的一行下标运算。",
            ),
            reality: Tr::new(
                "APIs call these clamp-to-edge, repeat and mirror. Skyboxes need clamp, brick walls need repeat, and a wrong choice shows as a seam along the border.",
                "各种 API 把它们叫作 clamp-to-edge、repeat 和 mirror。天空盒需要钳位，砖墙需要平铺，选错了就会在边上出现接缝。",
            ),
            myth: Tr::new(
                "Myth: painting the border black hides the seam. It only swaps one wrong colour for another, and the outermost ring no longer matches clamp.",
                "误解：把边框涂黑就能盖住接缝。那只是换了一种错的颜色，最外一圈也不再符合钳位。",
            ),
            simplified: Tr::new(
                "OC-1 gives you only integer index maths with AND, MIN and MAX; the hardware picks the rule with one state bit.",
                "OC-1 只给你 AND、MIN、MAX 这样的整数下标运算；真实硬件用一个状态位就选好了规则。",
            ),
        },
        clue: Some(Clue {
            what: Tr::new(
                "the first pixel written with the wrong colour",
                "第一个写错颜色的像素",
            ),
            find: crate::grade::first_wrong_write,
        }),
        ..base("wrap-mode", 7, ALL_WARPS, 1024, LERP_OPS)
    }
}

/// A clinic scan: 128×128 grey pixels at `rom[y*128 + x]`, then the viewer's
/// settings: `rom[16384]` = log2 zoom, `rom[16385]`/`rom[16386]` = pan.
fn xray_case(kind: u8, seed: u64, view: (i32, i32, i32)) -> Case {
    let mut r = rng(seed);
    let mut rom = Vec::with_capacity(16387);
    for i in 0..16384i32 {
        let (x, y) = (i % 128, i / 128);
        let v = match kind {
            // Chest: soft tissue, a bright spine, two dark lungs.
            0 => {
                let lung = ((x - 36) * (x - 36) + (y - 64) * (y - 64) < 900)
                    || ((x - 92) * (x - 92) + (y - 64) * (y - 64) < 900);
                let bone = (x - 64).abs() < 7 || (y % 24 < 3 && (x - 64).abs() < 40);
                (if bone {
                    170
                } else if lung {
                    35
                } else {
                    80
                }) + below(&mut r, 14)
            }
            // Flat: everything between 100 and 130.
            1 => 100 + x / 8 + y / 16 + below(&mut r, 8),
            // Dark plate with a few bright blobs.
            2 => {
                let blob = (x % 40 - 20).pow(2) + (y % 48 - 24).pow(2) < 60;
                if blob {
                    60 + below(&mut r, 40)
                } else {
                    below(&mut r, 6)
                }
            }
            // Smooth ramp with rings.
            3 => {
                (((x + y) * 255 / 254) + ((x - 64) * (x - 64) + (y - 64) * (y - 64)) / 64 % 40)
                    % 256
            }
            // Low-dose: dark and noisy, 0..=90.
            4 => (x / 3 + y / 5 + below(&mut r, 30)) % 91,
            // Noise with one bright patch.
            _ => {
                if (x - 90).abs() < 12 && (y - 30).abs() < 12 {
                    200 + below(&mut r, 56)
                } else {
                    below(&mut r, 150)
                }
            }
        };
        rom.push(v.clamp(0, 255));
    }
    rom.extend([view.0, view.1, view.2]);
    Case { rom, frame: 0 }
}

/// Bilinear sample of the scan at (x, y) in Q16 texels, edges clamped, rounding down at each blend.
fn xray_sample(img: &[i32], sx: i64, sy: i64) -> i64 {
    let hi = 127i64 << 16;
    let (sx, sy) = (sx.clamp(0, hi), sy.clamp(0, hi));
    let (ix, iy) = ((sx >> 16) as usize, (sy >> 16) as usize);
    let (fx, fy) = (sx & 65535, sy & 65535);
    let at = |x: usize, y: usize| img[y.min(127) * 128 + x.min(127)] as i64;
    let top = (at(ix, iy) * (65536 - fx) + at(ix + 1, iy) * fx) >> 16;
    let bot = (at(ix, iy + 1) * (65536 - fx) + at(ix + 1, iy + 1) * fx) >> 16;
    (top * (65536 - fy) + bot * fy) >> 16
}

/// The viewport: LUT[sample], with the LUT from equalising the whole scan.
fn xray(c: &Case) -> Vec<Option<i32>> {
    let img = &c.rom[..16384];
    let (s, ox, oy) = (c.rom[16384], c.rom[16385] as i64, c.rom[16386] as i64);
    let mut cdf = [0i64; 256];
    for &v in img {
        cdf[v as usize] += 1;
    }
    for b in 1..256 {
        cdf[b] += cdf[b - 1];
    }
    let cmin = cdf.iter().copied().find(|&c| c > 0).unwrap_or(0);
    let den = 16384 - cmin;
    (0..4096i64)
        .map(|p| {
            let (px, py) = (p & 63, p >> 6);
            let step = |q: i64, o: i64| (((2 * q + 1) << (15 - s)) - 32768) + (o << 16);
            let v = xray_sample(img, step(px, ox), step(py, oy)) as usize;
            Some(if den == 0 {
                0
            } else {
                ((cdf[v] - cmin).max(0) * 255 / den) as i32
            })
        })
        .collect()
}

/// Three viewers' settings for a scan: (log2 zoom, pan x, pan y).
const VIEWS: [(i32, i32, i32); 3] = [(0, 32, 32), (1, 90, 4), (2, 108, 112)];
const VIEWS_PAST: [(i32, i32, i32); 3] = [(0, 100, 90), (1, -20, 110), (2, 120, -6)];
const VIEWS_PAST_B: [(i32, i32, i32); 3] = [(0, -30, -30), (1, 110, 20), (2, -5, 40)];

fn the_xray() -> Task {
    Task {
        title: Tr::new("The X-ray Viewer", "透视观片"),
        brief: Tr::new(
            "Bohe runs the clinic's viewer. rom[y*128 + x] is a 128×128 scan, grey 0..255. After it: rom[16384] = log2 of the zoom (0, 1, 2 for ×1, ×2, ×4), rom[16385] and rom[16386] = the pan, the scan texel under the viewport's top-left corner (it may be negative or past the edge).\nThe viewport is 64×64: out[Y*64 + X] = LUT[v]. v is the scan sampled bilinearly at s = ((2X+1) << (15 − zoom)) − 32768 + (pan << 16) in Q16 texels, for x with X and pan x, for y with Y and pan y; the edge texel repeats outside the scan, and every blend rounds down as in 9-2. LUT is the equalisation of the whole scan, as in 9-6: LUT[g] = (cdf[g] − cdf_min) · 255 / (16384 − cdf_min). Each result may be off by at most 1, but v itself must be exact: one grey off can move the LUT by more than 1.\nThe machine is yours: 16 warps, 8192 shared words. The clinic pays by the frame, and reads from the scan are what the memory bus bills.",
            "薄荷在管诊所的观片机。rom[y*128 + x] 是 128×128 的片子，灰度 0..255。其后：rom[16384] = 缩放的对数（0、1、2 对应 ×1、×2、×4），rom[16385] 和 rom[16386] = 平移，即视口左上角下面的那个纹素（可以是负数，也可以越出片子边缘）。\n视口是 64×64：out[Y*64 + X] = LUT[v]。v 是对片子的双线性取样，位置 s = ((2X+1) << (15 − 缩放)) − 32768 + (平移 << 16)（Q16 纹素），横向用 X 和横向平移，纵向用 Y 和纵向平移；片子之外重复边缘纹素，每次混合都向下取整，和 9-2 一样。LUT 是对整张片子的均衡，和 9-6 一样：LUT[g] = (cdf[g] − cdf_min) · 255 / (16384 − cdf_min)。每个结果允许差 1，但 v 本身必须精确：灰度差一级，LUT 的结果可能就差得更多。\n机器随你组装：16 个线程束，8192 个共享字。诊所按帧付钱，而内存总线按读取片子的次数收费。",
        ),
        cases: || {
            let mut v = Vec::new();
            for (k, kind) in [0u8, 1, 2, 3].into_iter().enumerate() {
                for view in VIEWS {
                    v.push(xray_case(kind, 971 + k as u64, view));
                }
            }
            v
        },
        hidden: || {
            let mut v = Vec::new();
            for view in VIEWS_PAST {
                v.push(xray_case(4, 981, view));
            }
            for view in VIEWS_PAST_B {
                v.push(xray_case(5, 982, view));
            }
            v
        },
        expect: xray,
        tolerance: 1,
        goals: [
            Goal::new(Metric::Cycles, 27500),
            Goal::new(Metric::Loads, 24000),
        ],
        starter: Source::Code(
            "; rom[0..16383] is the 128x128 scan; rom[16384] = log2 zoom, rom[16385] = pan x, rom[16386] = pan y | rom[0..16383] 是 128x128 的片子；rom[16384] = 缩放的对数，rom[16385] = 横向平移，rom[16386] = 纵向平移\n; out[0..4095] is the 64x64 viewport | out[0..4095] 是 64x64 的视口\n",
        ),
        reference: Source::Code(
            "; header: pan and zoom; the tile origin is one texel up-left of the viewport | 表头：平移和缩放；瓦片原点在视口左上再偏一个纹素\nLD r10, [16385]\nSUB r10, r10, 1\nLD r11, [16386]\nSUB r11, r11, 1\n; histogram of the whole scan, 32 pixels per thread | 整张片子的直方图，每个线程 32 个像素\nMOV r0, tid\nLOOP 32\n  LD r1, [r0]\n  ATOMS r2, [r1], 1\n  ADD r0, r0, 512\nENDLOOP\n; copy the 66x66 tile the viewport can touch into shared words 512.. (edges clamp) | 把视口可能碰到的 66x66 瓦片抄进共享内存 512..（边缘钳位）\nMOV r0, tid\nLOOP 9\n  MUL r1, r0, 993\n  SHR r1, r1, 16\n  MUL r2, r1, 66\n  SUB r2, r0, r2\n  ADD r2, r2, r10\n  MAX r2, r2, 0\n  MIN r2, r2, 127\n  ADD r1, r1, r11\n  MAX r1, r1, 0\n  MIN r1, r1, 127\n  SHL r1, r1, 7\n  ADD r1, r1, r2\n  LD r1, [r1]\n  ADD r2, r0, 512\n  STS [r2], r1\n  ADD r0, r0, 512\nENDLOOP\nBAR\n; one lane: running total, first non-zero total, and the reciprocal of the denominator | 一个车道：累计、第一个非零累计、分母的倒数\nSETP.EQ p0, tid, 0\nIF p0\n  MOV r1, 0\n  MOV r2, 0\n  MOV r4, 65536\n  LOOP 4\n    LOOP 64\n      LDS r3, [r2]\n      ADD r1, r1, r3\n      STS [r2], r1\n      SETP.EQ p1, r1, 0\n      SEL r5, p1, 65536, r1\n      MIN r4, r4, r5\n      ADD r2, r2, 1\n    ENDLOOP\n  ENDLOOP\n  SUB r7, 16384, r4\n  SETP.GE p1, r7, 4096\n  SEL r8, p1, 2, 0\n  SHR r10, r7, r8\n  RCP r10, r10\n  ADD r8, r8, 16\n  STS [5000], r4\n  STS [5001], r7\n  STS [5002], r10\n  STS [5003], r8\nENDIF\nBAR\n; every thread turns one running total into its grey (256 threads matter, the rest write unused words) | 每个线程把一个累计值变成灰度（前 256 个有用，其余写的是没用的字）\nLDS r4, [5000]\nLDS r7, [5001]\nLDS r5, [5002]\nLDS r8, [5003]\nLDS r1, [tid]\nSUB r2, r1, r4\nMUL r2, r2, 255\nMUL r3, r2, r5\nSHR r3, r3, r8\nMUL r10, r3, r7\nSUB r10, r2, r10\nMUL r10, r10, r5\nSHR r10, r10, r8\nADD r3, r3, r10\nSTS [tid], r3\nBAR\n; sampling constants: step A per output pixel, offsets Bx By | 取样常数：每个输出像素的步长 A，偏移 Bx By\nLD r0, [16384]\nSUB r0, 15, r0\nSHL r10, 2, r0\nSHL r3, 1, r0\nADD r11, r3, 33587200\nADD r12, r3, 32768\nMOV r14, tid\nLOOP 8\n  AND r0, r14, 63\n  MUL r0, r0, r10\n  ADD r0, r0, r11\n  AND r5, r0, 65535\n  SUB r4, 65536, r5\n  SHR r0, r0, 16\n  SHR r1, r14, 6\n  MUL r1, r1, r10\n  ADD r15, r1, r12\n  SHR r1, r15, 16\n  MUL r1, r1, 66\n  ADD r0, r0, r1\n  ADD r1, r0, 1\n  ADD r2, r0, 66\n  ADD r3, r0, 67\n  LDS r0, [r0]\n  LDS r1, [r1]\n  LDS r2, [r2]\n  LDS r3, [r3]\n  LERP r7, r0, r4\n  LERP r8, r2, r4\n  AND r1, r15, 65535\n  SUB r0, 65536, r1\n  LERP r0, r7, r0\n  LDS r0, [r0]\n  ST [r14], r0\n  ADD r14, r14, 512\nENDLOOP\n",
        ),
        live: Some(xray_live),
        steer: Some(Tr::new("PAN THE VIEWER", "平移观片窗")),
        predict: Predict {
            question: Tr::new(
                "4096 output pixels each need four texels, but neighbouring pixels share most of them. What is the cheapest way to feed them?",
                "4096 个输出像素每个都要四个纹素，但相邻像素共用其中的大部分。怎样喂给它们最省？",
            ),
            options: [
                Tr::new(
                    "Every pixel reads its four texels from global memory",
                    "每个像素从全局内存读自己的四个纹素",
                ),
                Tr::new(
                    "Copy the patch the viewport can touch into shared memory once, then read from there",
                    "把视口可能碰到的那块一次抄进共享内存，之后都从那里读",
                ),
                Tr::new(
                    "It makes no difference: the cache makes repeat reads free",
                    "没区别：缓存会让重复的读取不要钱",
                ),
            ],
            answer: 1,
            why: Tr::new(
                "Four reads per pixel is 16384 lane reads, each billed even when it hits the cache. The viewport touches at most 66×66 = 4356 texels, so copying them once and sampling from shared memory cuts the bill to about a quarter.",
                "每个像素读四次，就是 16384 次车道读取，哪怕命中缓存也照样计费。视口最多碰到 66×66 = 4356 个纹素，抄一次再从共享内存取样，读取次数能降到大约四分之一。",
            ),
        },
        hints: [
            Tr::new(
                "Four jobs, a BAR between each: count the grey levels of all 16384 pixels (32 per thread, ATOMS); one lane turns the counts into running totals and a reciprocal; every thread turns one running total into a LUT entry, in place; then every thread samples 8 output pixels.",
                "四件事，每件之间一个 BAR：数出全部 16384 个像素的灰度（每个线程 32 个，用 ATOMS）；一个车道把计数变成累计值并算出倒数；每个线程把一个累计值就地变成一项 LUT；最后每个线程取样 8 个输出像素。",
            ),
            Tr::new(
                "With 16384 pixels the denominator can be 16000, and RCP gives only 4: one correction no longer reaches ±1. When den ≥ 4096, take RCP of den >> 2 and shift 2 less afterwards; the product stays under 2^31.",
                "有 16384 个像素时分母可达 16000，RCP 只给出 4：修正一次已经够不到 ±1。当 den ≥ 4096 时，对 den >> 2 取 RCP，之后少移 2 位；乘积仍在 2^31 以内。",
            ),
            Tr::new(
                "Sampling straight from global memory takes 4 reads per pixel. The viewport only touches a 66×66 patch, starting one texel up-left of the pan: copy it into shared words (clamp the coordinates while copying), and the sampling needs no clamp at all. Mind where the LUT lives.",
                "直接从全局内存取样，每像素要读 4 次。视口只碰到一块 66×66 的区域，起点是平移位置的左上再偏一个纹素：把它抄进共享字（抄的时候钳位坐标），取样时就完全不用钳位了。留意 LUT 放在哪里。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Tile, then sample", "分块，再取样"),
            line: Tr::new(
                "Fetch the patch many threads share once, put it where reads are cheap, and let every thread sample from there.",
                "把许多线程共用的那块数据取一次，放到读取便宜的地方，让每个线程都从那里取样。",
            ),
            reality: Tr::new(
                "Medical viewers and games alike keep a tile of the texture in on-chip memory; texture units add the clamp and the bilinear blend in hardware, so a sample is one instruction.",
                "医学影像查看器和游戏都会把纹理的一块放在片上内存里；纹理单元用硬件完成钳位和双线性混合，一次取样就是一条指令。",
            ),
            myth: Tr::new(
                "Myth: the cache already does this, so tiling is pointless. The cache saves time, but the bus still counts every request.",
                "误解：缓存已经做了这件事，分块没意义。缓存省的是时间，但总线仍会数每一次请求。",
            ),
            simplified: Tr::new(
                "OC-1 has no texture unit: the clamp, the weights and both blends are your instructions.",
                "OC-1 没有纹理单元：钳位、权重和两次混合都得靠你自己的指令。",
            ),
        },
        ..base("the-xray", 8, ALL_WARPS, 4096, VAULT_OPS)
    }
}

/// The showcase: the viewer cycles through the three zooms; the pan follows the player's point.
fn xray_live(frame: i32, at: [i32; 2]) -> Case {
    let zoom = (frame / 90).rem_euclid(3);
    xray_case(0, 971, (zoom, 32 + (at[0] - 4) * 8, 32 + (at[1] - 4) * 8))
}

//@@NEXT@@
