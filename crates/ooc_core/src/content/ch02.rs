//! Chapter 2 · Lumen + cartridge: OC-1 kernels over the whole screen.

use oo_base::Tr;

use super::{SCALAR_OPS, below, picture, rng, screen, stub};
use crate::isa::FB_LEN;
use crate::task::*;
use crate::vm::Launch;

fn base(id: &'static str, slot: u8) -> Task {
    stub(
        id,
        2,
        slot,
        View::Screen,
        Editor::Code(SCALAR_OPS),
        Launch::SCREEN,
        FB_LEN,
    )
}

fn rom_cases(seeds: &[u64], make: fn(u64) -> Vec<i32>) -> Vec<Case> {
    seeds
        .iter()
        .map(|&s| Case {
            rom: make(s),
            frame: 0,
        })
        .collect()
}

/// Reads ROM like the VM does: out of range is 0.
fn rd(rom: &[i32], i: i32) -> i32 {
    usize::try_from(i)
        .ok()
        .and_then(|i| rom.get(i))
        .copied()
        .unwrap_or(0)
}

pub fn tasks() -> Vec<Task> {
    vec![
        the_sprite(),
        the_tile(),
        the_lut(),
        the_skew(),
        the_scroll(),
        the_collision(),
        the_flicker(),
        arcade_night(),
    ]
}

/// An 8×8 sprite with transparent (0) corners, so the shape is visible.
fn sprite8(seed: u64) -> Vec<i32> {
    let p = picture(seed, 8, 8);
    (0..64)
        .map(|i| {
            if [0, 7, 56, 63].contains(&i) {
                0
            } else {
                p[i as usize]
            }
        })
        .collect()
}

fn the_sprite() -> Task {
    Task {
        title: Tr::new("The Sprite", "精灵"),
        brief: Tr::new(
            "The arcade's mascot lives in ROM: 8×8 colours, row by row, at rom[0..64].\nDraw it with its top-left corner at (4,4). Everything else stays black.\nEvery pixel is a thread: each one must find its own sprite pixel — or none.",
            "街机吉祥物存放在 ROM 里：8×8 个颜色，按行排列在 rom[0..64]。\n把它画在左上角 (4,4) 处，其余像素保持黑色。\n每个像素都是一个线程：各自去找属于自己的那个精灵像素——或者什么也不画。",
        ),
        cases: || rom_cases(&[11, 12, 13], sprite8),
        hidden: || rom_cases(&[14, 15], sprite8),
        expect: |c| {
            screen(|x, y| {
                let (u, v) = (x - 4, y - 4);
                if (0..8).contains(&u) && (0..8).contains(&v) {
                    c.rom[(v * 8 + u) as usize]
                } else {
                    0
                }
            })
        },
        goals: [Goal::new(Metric::Cycles, 80), Goal::new(Metric::Size, 10)],
        starter: Source::Code(
            "; sprite-local coordinates | 精灵内的局部坐标\nSUB r0, x, 4\nSUB r1, y, 4\n; index = v * 8 + u | 下标 = v * 8 + u\nMUL r1, r1, 8\nADD r1, r1, r0\nLD r2, [r1]\nPIX r2\n",
        ),
        reference: Source::Code(
            "SUB r0, x, 4\nSUB r1, y, 4\nOR r2, r0, r1\nSHR r2, r2, 3\nSHL r1, r1, 3\nADD r1, r1, r0\nLD r3, [r1]\nSETP.EQ p0, r2, 0\nSEL r3, p0, r3, 0\nPIX r3\n",
        ),
        predict: Predict {
            question: Tr::new(
                "Run the starter as is. What happens to pixel (0,5), left of the sprite?",
                "直接运行初始代码。精灵左边的像素 (0,5) 会怎样？",
            ),
            options: [
                Tr::new("It stays black", "保持黑色"),
                Tr::new(
                    "It shows a sprite pixel from the row above",
                    "显示上一行的某个精灵像素",
                ),
                Tr::new("The machine crashes", "机器崩溃"),
            ],
            answer: 1,
            why: Tr::new(
                "u = −4, v = 1, so the index is 1·8 − 4 = 4: a real sprite pixel. Memory has no idea it is a picture.",
                "u = −4、v = 1，下标是 1·8 − 4 = 4：一个真实存在的精灵像素。内存并不知道自己存的是一张图。",
            ),
        },
        hints: [
            Tr::new(
                "The starter draws the sprite, plus smeared copies around it. Which pixels should not read ROM at all?",
                "初始代码画出了精灵，周围却有拖影。哪些像素根本不该去读 ROM？",
            ),
            Tr::new(
                "Inside means 0 ≤ u < 8 and 0 ≤ v < 8. (u OR v) SHR 3 is 0 only then — negatives keep their sign bit.",
                "在内部意味着 0 ≤ u < 8 且 0 ≤ v < 8。只有这时 (u OR v) SHR 3 才是 0——负数会保留符号位。",
            ),
            Tr::new(
                "OR r2, r0, r1 → SHR r2, r2, 3 → after the LD: SETP.EQ p0, r2, 0 and SEL r3, p0, r3, 0 before PIX.",
                "OR r2, r0, r1 → SHR r2, r2, 3 → LD 之后：SETP.EQ p0, r2, 0，再 SEL r3, p0, r3, 0，最后 PIX。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Bounds are your job", "边界由你负责"),
            line: Tr::new(
                "A 2D picture is a 1D array. Step off its edge and you silently read a neighbour, not an error.",
                "二维图片其实是一维数组。越过边缘不会报错，只会悄悄读到邻居。",
            ),
            reality: Tr::new(
                "GPU kernels start with `if (x >= width) return;`. Texture units clamp or wrap so shaders need not.",
                "GPU 内核总以 `if (x >= width) return;` 开头。纹理单元会钳制或环绕坐标，让着色器省掉这一步。",
            ),
            myth: Tr::new(
                "Myth: out-of-bounds reads always crash. On GPUs they often just return wrong data.",
                "误解：越界读取一定会崩溃。在 GPU 上它常常只是返回错误的数据。",
            ),
            simplified: Tr::new(
                "OC-1's ROM returns 0 past its end; real memory past an array is just more memory.",
                "OC-1 的 ROM 越过末尾就返回 0；真实内存里，数组之外只是更多的内存。",
            ),
        },
        ..base("the-sprite", 1)
    }
}

/// rom[0..16]: a 4×4 map of tile ids 0..4; rom[16..80]: four 4×4 tiles.
fn tileset(seed: u64) -> Vec<i32> {
    let mut r = rng(seed);
    let mut rom: Vec<i32> = (0..16).map(|_| below(&mut r, 4)).collect();
    for t in 0..4u64 {
        rom.extend(picture(seed * 10 + t, 4, 4));
    }
    rom
}

/// rom[0..16]: a 4×4 table of arbitrary colours.
fn colour_table(seed: u64) -> Vec<i32> {
    let mut r = rng(seed);
    (0..16).map(|_| 1 + below(&mut r, 15)).collect()
}

fn the_lut() -> Task {
    Task {
        title: Tr::new("The Lookup Table", "查表"),
        brief: Tr::new(
            "The cartridge holds a 4×4 table of colours in rom[0..16], row by row, with no pattern to them.\nTile the screen with it: pixel (x,y) takes the colour at rom[(y AND 3)·4 + (x AND 3)].\nNo formula can make these colours. Stop calculating and look them up.",
            "卡带的 rom[0..16] 里存着一张 4×4 的颜色表，按行排列，毫无规律可言。\n用它铺满屏幕：像素 (x,y) 取 rom[(y AND 3)·4 + (x AND 3)] 处的颜色。\n没有任何公式能造出这些颜色。别再算了，去查。",
        ),
        cases: || rom_cases(&[31, 32, 33], colour_table),
        hidden: || rom_cases(&[34, 35], colour_table),
        expect: |c| screen(|x, y| c.rom[((y & 3) * 4 + (x & 3)) as usize]),
        goals: [Goal::new(Metric::Cycles, 48), Goal::new(Metric::Size, 6)],
        starter: Source::Code(
            "; a formula that looks plausible on one table | 一条在某张表上看起来还行的公式\nADD r0, x, y\nAND r0, r0, 15\nPIX r0\n",
        ),
        reference: Source::Code(
            "AND r0, y, 3\nSHL r0, r0, 2\nAND r1, x, 3\nADD r0, r0, r1\nLD r0, [r0]\nPIX r0\n",
        ),
        predict: Predict {
            question: Tr::new(
                "A 4×4 table tiles a 16×16 screen. How many pixels show the same table entry?",
                "一张 4×4 的表铺满 16×16 的屏幕。同一个表项会出现在多少个像素上？",
            ),
            options: [
                Tr::new("4", "4"),
                Tr::new("16", "16"),
                Tr::new("256", "256"),
            ],
            answer: 1,
            why: Tr::new(
                "256 pixels share 16 entries, so each entry shows up 16 times. The table is read 256 times but stored once.",
                "256 个像素共用 16 个表项，每个表项出现 16 次。表被读了 256 次，却只存了一份。",
            ),
        },
        hints: [
            Tr::new(
                "The colours come from memory, not from arithmetic. The arithmetic you need is only for finding the right entry.",
                "颜色来自内存，而不是算术。你需要算的只是怎么找到对的那一项。",
            ),
            Tr::new(
                "AND with 3 keeps the low two bits: the column inside the table is x AND 3, the row is y AND 3. Index = row·4 + column.",
                "AND 3 保留低两位：表内的列是 x AND 3，行是 y AND 3。下标 = 行·4 + 列。",
            ),
            Tr::new(
                "AND r0, y, 3 / SHL r0, r0, 2 / AND r1, x, 3 / ADD r0, r0, r1 / LD r0, [r0] / PIX r0.",
                "AND r0, y, 3 / SHL r0, r0, 2 / AND r1, x, 3 / ADD r0, r0, r1 / LD r0, [r0] / PIX r0。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Tables beat formulas", "表比公式强"),
            line: Tr::new(
                "When the values have no rule, memory is the only way to produce them. Compute the address, then read.",
                "当数值没有规律时，内存是产生它们的唯一办法。先算地址，再去读。",
            ),
            reality: Tr::new(
                "Palettes, gamma curves and sine tables are all lookup tables; a GPU's texture units are built around fetching table entries fast.",
                "调色板、伽马曲线和正弦表都是查找表；GPU 的纹理单元就是围绕“快速取表项”造出来的。",
            ),
            myth: Tr::new(
                "Myth: computing is always cheaper than remembering. A single load can replace a long formula.",
                "误解：计算总比记忆便宜。一次读取就能顶替一长串公式。",
            ),
            simplified: Tr::new(
                "OC-1 loads finish at once. Real tables live in caches, and a miss costs hundreds of cycles.",
                "OC-1 的读取瞬间完成。真实的表放在缓存里，一次未命中要付出几百个周期。",
            ),
        },
        ..base("the-lut", 1)
    }
}

fn the_tile() -> Task {
    Task {
        title: Tr::new("The Tile Map", "图块地图"),
        brief: Tr::new(
            "Old cabinets could not store a whole screen, so they stored a map.\nrom[0..16] is a 4×4 map of tile numbers; tile t's 4×4 pixels start at rom[16 + 16·t].\nDraw the level: each screen pixel looks up its tile, then its pixel inside the tile.",
            "老街机存不下整屏画面，于是只存一张地图。\nrom[0..16] 是 4×4 的图块编号地图；图块 t 的 4×4 像素从 rom[16 + 16·t] 开始。\n画出关卡：每个像素先查自己属于哪个图块，再查图块内的像素。",
        ),
        cases: || rom_cases(&[21, 22, 23], tileset),
        hidden: || rom_cases(&[24, 25], tileset),
        expect: |c| {
            screen(|x, y| {
                let t = c.rom[((y >> 2) * 4 + (x >> 2)) as usize];
                c.rom[(16 + t * 16 + (y & 3) * 4 + (x & 3)) as usize]
            })
        },
        goals: [Goal::new(Metric::Cycles, 104), Goal::new(Metric::Size, 13)],
        starter: Source::Code(
            "; which map cell? row (y / 4), column (x / 4) | 哪个地图格？行 (y / 4)，列 (x / 4)\nSHR r0, y, 2\nSHL r0, r0, 2\nSHR r1, x, 2\nADD r0, r0, r1\nLD r0, [r0]\n; this draws the tile number, not the tile | 这画出的是图块编号，不是图块\nPIX r0\n",
        ),
        reference: Source::Code(
            "AND r0, y, 12\nSHR r1, x, 2\nADD r0, r0, r1\nLD r0, [r0]\nSHL r0, r0, 4\nAND r1, y, 3\nSHL r1, r1, 2\nADD r0, r0, r1\nAND r1, x, 3\nADD r0, r0, r1\nADD r0, r0, 16\nLD r0, [r0]\nPIX r0\n",
        ),
        predict: Predict {
            question: Tr::new(
                "This map holds 16 numbers plus 64 tile pixels. How many values would a plain 16×16 picture need?",
                "这张地图存 16 个编号加 64 个图块像素。一张普通的 16×16 图片要存多少个值？",
            ),
            options: [
                Tr::new("80", "80"),
                Tr::new("256", "256"),
                Tr::new("1024", "1024"),
            ],
            answer: 1,
            why: Tr::new(
                "256 — one per pixel. Tiles trade memory for an extra lookup, and grow far better on bigger screens.",
                "256——每个像素一个。图块用一次额外的查表换来了内存，屏幕越大越划算。",
            ),
        },
        hints: [
            Tr::new(
                "Two lookups, one after the other: the map tells you which tile, the tile tells you which colour.",
                "两次查表，一前一后：地图告诉你是哪个图块，图块告诉你是什么颜色。",
            ),
            Tr::new(
                "Inside a tile, the pixel is (y AND 3)·4 + (x AND 3). The tile itself starts at 16 + t·16, i.e. (t SHL 4) + 16.",
                "图块内部的像素是 (y AND 3)·4 + (x AND 3)。图块本身从 16 + t·16 开始，也就是 (t SHL 4) + 16。",
            ),
            Tr::new(
                "Keep the starter's first LD, then: SHL r0, r0, 4; add (y AND 3) SHL 2; add x AND 3; add 16; LD again; PIX. For the size star, (y SHR 2) SHL 2 is just y AND 12.",
                "保留初始代码的第一个 LD，然后：SHL r0, r0, 4；加上 (y AND 3) SHL 2；加上 x AND 3；加 16；再 LD；PIX。想拿规模星：(y SHR 2) SHL 2 就是 y AND 12。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Indirection", "间接寻址"),
            line: Tr::new(
                "A load whose address comes from another load. Two small tables drew a whole screen.",
                "一次读取的地址来自另一次读取。两张小表就画满了整个屏幕。",
            ),
            reality: Tr::new(
                "The NES drew every background from 8×8 tiles; modern GPUs still page huge textures through tile tables.",
                "NES 的所有背景都由 8×8 图块拼成；现代 GPU 仍用图块表来分页管理巨大的纹理。",
            ),
            myth: Tr::new(
                "Myth: dependent loads are free. The second waits for the first — GPUs hide that wait by running other warps.",
                "误解：相互依赖的读取没有代价。第二次必须等第一次——GPU 靠切换去运行别的 warp 来掩盖等待。",
            ),
            simplified: Tr::new(
                "OC-1 loads finish instantly. Real memory takes hundreds of cycles, which is why the scheduler matters.",
                "OC-1 的读取瞬间完成。真实内存要几百个周期，这正是调度器重要的原因。",
            ),
        },
        ..base("the-tile", 2)
    }
}

fn the_skew() -> Task {
    Task {
        title: Tr::new("The Skewed Level", "歪掉的关卡"),
        brief: Tr::new(
            "Fault report: the cabinet's level 3 renders, but it looks mirrored along the diagonal — rooms are where the corridors should be.\nThe map and tiles are the same as before. The tile pixels are fine; the map lookup is not. Find the cell that picks the wrong tile.",
            "故障报告：街机的第三关能画出来，但整张图沿对角线翻转了——房间长在走廊的位置上。\n地图和图块都和之前一样。图块内部的像素没问题，出错的是查地图那一步。找出选错图块的那个格子。",
        ),
        cases: || rom_cases(&[26, 27, 28], tileset),
        hidden: || rom_cases(&[29, 30], tileset),
        expect: |c| {
            screen(|x, y| {
                let t = c.rom[((y >> 2) * 4 + (x >> 2)) as usize];
                c.rom[(16 + t * 16 + (y & 3) * 4 + (x & 3)) as usize]
            })
        },
        goals: [Goal::new(Metric::Cycles, 104), Goal::new(Metric::Size, 13)],
        starter: Source::Code(
            "; map cell: column-major | 地图格：按列排列\nAND r0, x, 12\nSHR r1, y, 2\nADD r0, r0, r1\nLD r0, [r0]\nSHL r0, r0, 4\nAND r1, y, 3\nSHL r1, r1, 2\nADD r0, r0, r1\nAND r1, x, 3\nADD r0, r0, r1\nADD r0, r0, 16\nLD r0, [r0]\nPIX r0\n",
        ),
        reference: Source::Code(
            "AND r0, y, 12\nSHR r1, x, 2\nADD r0, r0, r1\nLD r0, [r0]\nSHL r0, r0, 4\nAND r1, y, 3\nSHL r1, r1, 2\nADD r0, r0, r1\nAND r1, x, 3\nADD r0, r0, r1\nADD r0, r0, 16\nLD r0, [r0]\nPIX r0\n",
        ),
        predict: Predict {
            question: Tr::new(
                "The map is stored row by row. The starter reads it column by column. Which map cells are read correctly anyway?",
                "地图是按行存的，初始代码却按列去读。哪些地图格碰巧读对了？",
            ),
            options: [
                Tr::new("None of them", "一个也没有"),
                Tr::new(
                    "Only the diagonal, where row = column",
                    "只有对角线，行 = 列的格子",
                ),
                Tr::new("Every other cell", "隔一格对一格"),
            ],
            answer: 1,
            why: Tr::new(
                "Swapping row and column leaves a cell alone only when they are equal. Everything off the diagonal lands on its mirror image.",
                "行列互换后，只有行 = 列的格子原地不动。对角线之外的格子都落到了它的镜像位置上。",
            ),
        },
        hints: [
            Tr::new(
                "Compare the wrong picture with the right one. Which tile ended up where? The tiles themselves are intact.",
                "对比错的画面和对的画面：哪个图块跑到了哪里？图块本身是完好的。",
            ),
            Tr::new(
                "The map is 4 cells wide, row by row: cell = (y SHR 2)·4 + (x SHR 2). The starter has the two terms the other way round.",
                "地图宽 4 格，按行排列：格号 = (y SHR 2)·4 + (x SHR 2)。初始代码把这两项的位置对调了。",
            ),
            Tr::new(
                "Replace the first three lines with AND r0, y, 12 / SHR r1, x, 2 / ADD r0, r0, r1.",
                "把前三行换成 AND r0, y, 12 / SHR r1, x, 2 / ADD r0, r0, r1。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Row-major or column-major", "行优先还是列优先"),
            line: Tr::new(
                "A 2D grid in 1D memory needs an agreed order. Swap rows and columns and every read still succeeds — it just reads the wrong thing.",
                "二维网格放进一维内存，必须约定顺序。把行列互换，每次读取依然成功——只是读到了错的东西。",
            ),
            reality: Tr::new(
                "Transposed indexing is among the most common GPU bugs, and it rarely crashes: the image or matrix is simply wrong.",
                "转置式的下标错误是 GPU 上最常见的 bug 之一，而且很少崩溃：图像或矩阵只是悄悄变错了。",
            ),
            myth: Tr::new(
                "Myth: a wrong index fails loudly. Inside the array it returns a valid, wrong value.",
                "误解：下标错了会大声报错。只要还在数组里，它返回的就是一个合法却错误的值。",
            ),
            simplified: Tr::new(
                "OC-1 has no bounds faults at all; real hardware only traps accesses outside a whole allocation.",
                "OC-1 完全没有越界故障；真实硬件只会拦截整块分配之外的访问。",
            ),
        },
        clue: Some(Clue {
            what: Tr::new(
                "the first pixel written with the wrong tile",
                "第一个用错图块的像素被写下的那一拍",
            ),
            find: crate::grade::first_wrong_write,
        }),
        ..base("the-skew", 4)
    }
}

fn the_scroll() -> Task {
    Task {
        title: Tr::new("The Scroll", "卷轴"),
        brief: Tr::new(
            "The racing cabinet's road is a 16×16 strip in rom[0..256].\nScroll it left one pixel per frame: pixel (x,y) shows strip column (x + CLOCK) mod 16.\nThe strip loops forever — what leaves on the left comes back on the right.",
            "赛车机的路面是存在 rom[0..256] 里的一段 16×16 画面。\n每帧向左卷动一个像素：像素 (x,y) 显示画面的第 (x + CLOCK) mod 16 列。\n画面无限循环——从左边离开的，会从右边回来。",
        ),
        cases: || {
            [(31, 0), (32, 3), (33, 12), (34, 29)]
                .map(|(s, f)| Case {
                    rom: picture(s, 16, 16),
                    frame: f,
                })
                .to_vec()
        },
        hidden: || {
            [(35, 7), (36, 40)]
                .map(|(s, f)| Case {
                    rom: picture(s, 16, 16),
                    frame: f,
                })
                .to_vec()
        },
        expect: |c| screen(|x, y| c.rom[(y * 16 + ((x + c.frame) & 15)) as usize]),
        goals: [Goal::new(Metric::Cycles, 48), Goal::new(Metric::Size, 6)],
        starter: Source::Code(
            "ADD r0, x, frame\nSHL r1, y, 4\nADD r0, r0, r1\nLD r0, [r0]\nPIX r0\n",
        ),
        reference: Source::Code(
            "ADD r0, x, frame\nAND r0, r0, 15\nSHL r1, y, 4\nADD r0, r0, r1\nLD r0, [r0]\nPIX r0\n",
        ),
        predict: Predict {
            question: Tr::new(
                "Without wrapping, at frame 3 pixel (14,2) reads strip address 2·16 + 17. What does it show?",
                "如果不做环绕，第 3 帧时像素 (14,2) 读取地址 2·16 + 17。它会显示什么？",
            ),
            options: [
                Tr::new("Strip pixel (1,3) — the next row", "画面的 (1,3)——下一行"),
                Tr::new("Strip pixel (1,2) — correct", "画面的 (1,2)——正确"),
                Tr::new("Black", "黑色"),
            ],
            answer: 0,
            why: Tr::new(
                "Address 49 is row 3, column 1. Rows sit end to end in memory, so running off one row walks into the next.",
                "地址 49 是第 3 行第 1 列。各行在内存中首尾相接，跑出一行就会走进下一行。",
            ),
        },
        hints: [
            Tr::new(
                "Watch the right edge of the starter: the picture there is one row too low. Which number grew too big?",
                "观察初始代码画面的右边缘：那里的图像低了一行。哪个数变得太大了？",
            ),
            Tr::new(
                "The column must stay in 0..15. AND with 15 wraps any number into that range.",
                "列号必须保持在 0..15。与 15 做 AND 能把任何数折回这个范围。",
            ),
            Tr::new(
                "Insert AND r0, r0, 15 right after ADD r0, x, frame.",
                "在 ADD r0, x, frame 之后插入 AND r0, r0, 15。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Wrap addressing", "环绕寻址"),
            line: Tr::new(
                "One AND turned a strip into a loop. Power-of-two sizes make wrapping almost free.",
                "一个 AND 把一段画面变成了循环。2 的幂次尺寸让环绕几乎不花代价。",
            ),
            reality: Tr::new(
                "Texture samplers offer a 'repeat' mode in hardware; consoles scrolled backgrounds by changing one register.",
                "纹理采样器在硬件里就提供“重复”模式；老主机只改一个寄存器就能卷动背景。",
            ),
            myth: Tr::new(
                "Myth: scrolling moves pixels around in memory. The picture stays put; only the read address moves.",
                "误解：卷动就是在内存里搬动像素。画面一动不动，移动的只是读取地址。",
            ),
            simplified: Tr::new(
                "Real wrapping often uses mod on non-power-of-two sizes, which is a slower operation.",
                "真实场景里常常要对非 2 的幂次取模，那是更慢的运算。",
            ),
        },
        ..base("the-scroll", 3)
    }
}

/// Front buffer (shown by the beam this frame) holds last frame's picture.
fn flicker_init(c: &Case) -> Vec<i32> {
    let front = (c.frame & 1) as usize * FB_LEN;
    let mut out = vec![9; 2 * FB_LEN];
    let old = picture(c.frame as u64 + 900, 16, 16);
    out[front..front + FB_LEN].copy_from_slice(&old);
    out
}

fn the_flicker() -> Task {
    Task {
        title: Tr::new("The Flicker", "闪烁"),
        brief: Tr::new(
            "Cabinet 7 flickers. It has two screens' worth of memory: out[0..256] and out[256..512].\nWhile the beam shows buffer (CLOCK AND 1), the kernel must draw rom's new picture into the other one.\nFind why the beam catches half-drawn frames — and fix it.",
            "7 号机在闪烁。它有两屏显存：out[0..256] 和 out[256..512]。\n扫描线显示第 (CLOCK AND 1) 块缓冲时，内核必须把 rom 里的新画面画进另一块。\n找出扫描线为何总拍到画了一半的帧——然后修好它。",
        ),
        view: View::Buffers,
        out_len: 2 * FB_LEN,
        cases: || {
            [(41, 0), (42, 1), (43, 6), (44, 11)]
                .map(|(s, f)| Case {
                    rom: picture(s, 16, 16),
                    frame: f,
                })
                .to_vec()
        },
        hidden: || {
            [(45, 3), (46, 14)]
                .map(|(s, f)| Case {
                    rom: picture(s, 16, 16),
                    frame: f,
                })
                .to_vec()
        },
        init: flicker_init,
        expect: |c| {
            let init = flicker_init(c);
            let back = ((c.frame + 1) & 1) as usize * FB_LEN;
            (0..2 * FB_LEN)
                .map(|i| {
                    Some(if (back..back + FB_LEN).contains(&i) {
                        c.rom[i - back]
                    } else {
                        init[i]
                    })
                })
                .collect()
        },
        goals: [Goal::new(Metric::Cycles, 48), Goal::new(Metric::Size, 6)],
        starter: Source::Code(
            "; the beam is showing buffer (frame AND 1) | 电子束正在显示缓冲 (frame AND 1)\nAND r0, frame, 1\nSHL r0, r0, 8\nADD r0, r0, tid\nLD r1, [tid]\nST [r0], r1\n",
        ),
        reference: Source::Code(
            "ADD r0, frame, 1\nAND r0, r0, 1\nSHL r0, r0, 8\nADD r0, r0, tid\nLD r1, [tid]\nST [r0], r1\n",
        ),
        predict: Predict {
            question: Tr::new(
                "The beam reads a buffer top to bottom while warps are still writing it. What does the player see?",
                "扫描线自上而下读取一块缓冲，而 warp 还在往里写。玩家会看到什么？",
            ),
            options: [
                Tr::new("The old picture", "旧画面"),
                Tr::new(
                    "Part old picture, part new — a tear",
                    "一部分旧、一部分新——画面撕裂",
                ),
                Tr::new("The new picture", "新画面"),
            ],
            answer: 1,
            why: Tr::new(
                "Rows the warps reached first are new, the rest are old. Drawing where nobody is looking avoids the race.",
                "warp 先写到的行是新的，其余是旧的。在没人看的地方作画，才能避开这场竞争。",
            ),
        },
        hints: [
            Tr::new(
                "Compare the two buffers after a run. Which one did the kernel write — and which one is on screen?",
                "运行后对比两块缓冲。内核写的是哪一块？屏幕上显示的又是哪一块？",
            ),
            Tr::new(
                "The back buffer is the other one: ((CLOCK + 1) AND 1). Its base is that times 256.",
                "后台缓冲是另一块：((CLOCK + 1) AND 1)。它的起始地址是这个数乘以 256。",
            ),
            Tr::new(
                "Replace the first line with ADD r0, frame, 1 then AND r0, r0, 1. The rest stays.",
                "把第一行换成 ADD r0, frame, 1，再接 AND r0, r0, 1。其余不变。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Double buffering", "双缓冲"),
            line: Tr::new(
                "Never draw into what is being shown. Draw behind, then swap.",
                "永远不要往正在显示的画面上画。在后面画好，再交换。",
            ),
            reality: Tr::new(
                "Every game swaps a back buffer each frame; v-sync times the swap to the display's refresh.",
                "每个游戏每帧都会交换后台缓冲；垂直同步让交换对准显示器的刷新时刻。",
            ),
            myth: Tr::new(
                "Myth: a faster GPU fixes tearing. Tearing is about when you write, not how fast.",
                "误解：更快的 GPU 能解决撕裂。撕裂关乎何时写入，而非写得多快。",
            ),
            simplified: Tr::new(
                "Real swaps flip a pointer in the display controller; OC-1 alternates buffers by frame parity.",
                "真实的交换是在显示控制器里切换指针；OC-1 用帧号奇偶来轮换缓冲。",
            ),
        },
        clue: Some(Clue {
            what: Tr::new(
                "the first store into the buffer the beam is showing",
                "第一次写进扫描线正在显示的那块缓冲的那一拍",
            ),
            find: crate::grade::first_wrong_write,
        }),
        ..base("the-flicker", 5)
    }
}

/// Two boxes `[x0, y0, x1, y1)` at rom[0..4] and rom[4..8], then the colours
/// for none/A/B/both at rom[8..12].
fn boxes(seed: u64) -> Vec<i32> {
    let mut r = rng(seed);
    let mut rom = Vec::new();
    for _ in 0..2 {
        let (x0, y0) = (below(&mut r, 9), below(&mut r, 9));
        rom.extend([x0, y0, x0 + 3 + below(&mut r, 5), y0 + 3 + below(&mut r, 5)]);
    }
    rom.extend([0, 3, 4, 15]);
    rom
}

fn inside(b: &[i32], x: i32, y: i32) -> bool {
    (b[0]..b[2]).contains(&x) && (b[1]..b[3]).contains(&y)
}

/// The box test as a listing: rom[a..=d] → `out` = 0 inside, -1 outside.
macro_rules! box_test {
    ($name:literal, $a:literal, $b:literal, $c:literal, $d:literal, $out:literal) => {
        concat!(
            "; inside ",
            $name,
            "?  x0, y0, x1, y1 from rom[",
            $a,
            "] | 在 ",
            $name,
            " 内吗？x0, y0, x1, y1 来自 rom[",
            $a,
            "]\n",
            "LD r0, [",
            $a,
            "]\nLD r1, [",
            $b,
            "]\nLD r2, [",
            $c,
            "]\nLD r3, [",
            $d,
            "]\n",
            "SUB r0, x, r0       ; x - x0, needs >= 0 | x - x0，需 >= 0\n",
            "SUB r1, y, r1       ; y - y0, needs >= 0 | y - y0，需 >= 0\n",
            "MIN r0, r0, r1\n",
            "SUB r2, r2, x       ; x1 - x, needs >= 1 | x1 - x，需 >= 1\n",
            "SUB r3, r3, y\n",
            "MIN r2, r2, r3\n",
            "SUB r2, r2, 1\n",
            "MIN r0, r0, r2      ; >= 0 only inside | 只有框内才 >= 0\n",
            "SHR ",
            $out,
            ", r0, 31     ; 0 inside, -1 outside | 框内 0，框外 -1\n",
        )
    };
}

fn the_collision() -> Task {
    Task {
        title: Tr::new("The Collision", "碰撞"),
        brief: Tr::new(
            "Two ships, two boxes. The debug overlay paints box A in 3, box B in 4, and their overlap in 15.\nThe A test is wired already; it leaves 0 in r8 inside A and −1 outside.\nAdd B, then let the four colours at rom[8..12] (none, A, B, both) do the rest.",
            "两艘飞船，两个碰撞盒。调试层把 A 盒涂成 3、B 盒涂成 4、重叠处涂成 15。\nA 的检测已经接好：在 A 内 r8 为 0，在 A 外为 −1。\n加上 B 的检测，再让 rom[8..12] 的四种颜色（都不在、A、B、都在）完成其余工作。",
        ),
        cases: || {
            [61, 62, 63, 64, 65]
                .map(|s| Case {
                    rom: boxes(s),
                    frame: 0,
                })
                .to_vec()
        },
        hidden: || {
            [66, 67, 68]
                .map(|s| Case {
                    rom: boxes(s),
                    frame: 0,
                })
                .to_vec()
        },
        expect: |c| {
            screen(|x, y| {
                let k =
                    inside(&c.rom[0..4], x, y) as usize + 2 * inside(&c.rom[4..8], x, y) as usize;
                c.rom[8 + k]
            })
        },
        goals: [Goal::new(Metric::Cycles, 248), Goal::new(Metric::Size, 31)],
        starter: Source::Code(concat!(
            box_test!("A", 0, 1, 2, 3, "r8"),
            "ADD r8, r8, 1\nMUL r8, r8, 3\nPIX r8\n"
        )),
        reference: Source::Code(concat!(
            box_test!("A", 0, 1, 2, 3, "r8"),
            box_test!("B", 4, 5, 6, 7, "r9"),
            "; colour index 8 + inA + 2*inB = 11 + r8 + 2*r9 | 颜色下标 8 + inA + 2*inB = 11 + r8 + 2*r9\nSHL r9, r9, 1\nADD r8, r8, r9\nADD r8, r8, 11\nLD r8, [r8]\nPIX r8\n",
        )),
        predict: Predict {
            question: Tr::new(
                "x − x0 is negative left of the box. What does SHR by 31 turn any negative number into?",
                "盒子左边 x − x0 为负。把任何负数右移 31 位会得到什么？",
            ),
            options: [Tr::new("0", "0"), Tr::new("1", "1"), Tr::new("−1", "−1")],
            answer: 2,
            why: Tr::new(
                "The shift copies the sign bit into every bit: −1 for negatives, 0 otherwise. A comparison with no compare.",
                "移位把符号位复制到每一位：负数得 −1，否则得 0。不用比较指令的比较。",
            ),
        },
        hints: [
            Tr::new(
                "B is the same question with different numbers. Can the A block be reused?",
                "B 是同一个问题，只是数字不同。A 的那段代码能复用吗？",
            ),
            Tr::new(
                "With inA = r8 + 1 and inB = r9 + 1, the colour sits at rom[8 + inA + 2·inB].",
                "设 inA = r8 + 1、inB = r9 + 1，颜色就在 rom[8 + inA + 2·inB]。",
            ),
            Tr::new(
                "Duplicate the A block with 4, 5, 6, 7 and SHR into r9. Then SHL r9, r9, 1; ADD r8, r8, r9; ADD r8, r8, 11; LD r8, [r8]; PIX r8.",
                "把 A 段复制一份，地址改成 4、5、6、7，SHR 写入 r9。然后 SHL r9, r9, 1；ADD r8, r8, r9；ADD r8, r8, 11；LD r8, [r8]；PIX r8。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Arithmetic instead of branches", "用算术代替分支"),
            line: Tr::new(
                "Min, shift and add answered four questions per box without a single IF.",
                "最小值、移位和加法，在一个 IF 都没有的情况下，为每个盒子回答了四个问题。",
            ),
            reality: Tr::new(
                "Physics engines test thousands of boxes per frame on the GPU with exactly this kind of branch-free math.",
                "物理引擎每帧在 GPU 上检测成千上万个盒子，用的正是这种无分支的数学。",
            ),
            myth: Tr::new(
                "Myth: clever bit tricks are unreadable hacks. Here they keep 32 lanes marching in step.",
                "误解：巧妙的位运算只是难懂的黑客把戏。在这里，它让 32 条车道步调一致。",
            ),
            simplified: Tr::new(
                "Real collision runs on objects, not pixels; per-pixel tests are for debug overlays like this one.",
                "真实的碰撞检测针对物体而非像素；逐像素的检测只用于这种调试叠加层。",
            ),
        },
        ..base("the-collision", 4)
    }
}

/// The Tile Map's background lookup, leaving the colour in r0.
macro_rules! tile_bg {
    () => {
        concat!(
            "; background: map cell, then pixel in tile | 背景：先找地图格，再找格内像素\n",
            "AND r0, y, 12\nSHR r1, x, 2\nADD r0, r0, r1\nLD r0, [r0]\n",
            "SHL r0, r0, 4\nAND r1, y, 3\nSHL r1, r1, 2\nADD r0, r0, r1\nAND r1, x, 3\nADD r0, r0, r1\nADD r0, r0, 16\nLD r0, [r0]\n",
        )
    };
}

/// Tile level at rom[0..80], 8×8 sprite at rom[80..144], its position at rom[144..146].
fn night_rom(seed: u64, at: [i32; 2]) -> Vec<i32> {
    let mut rom = tileset(seed);
    rom.extend(sprite8(seed + 5));
    rom.extend(at);
    rom
}

fn arcade_night() -> Task {
    Task {
        title: Tr::new("Arcade Night", "街机之夜"),
        brief: Tr::new(
            "The cabinet reopens tonight. The level from the Tile Map is at rom[0..80].\nThe hero — an 8×8 sprite at rom[80..144] — stands at (rom[144], rom[145]). Colour 0 is see-through.\nComposite them: sprite on top where it is opaque, the level everywhere else. It may hang off the edge.",
            "今晚街机重新开张。图块地图那一关的关卡数据在 rom[0..80]。\n主角——rom[80..144] 里的 8×8 精灵——站在 (rom[144], rom[145])。颜色 0 是透明的。\n把它们合成：精灵不透明的地方盖在上面，其余地方显示关卡。精灵可能伸出屏幕边缘。",
        ),
        cases: || {
            [(71, [4, 4]), (72, [0, 9]), (73, [11, -3]), (74, [-5, 12])]
                .map(|(s, at)| Case {
                    rom: night_rom(s, at),
                    frame: 0,
                })
                .to_vec()
        },
        hidden: || {
            [(75, [7, 2]), (76, [-6, -6]), (77, [13, 13])]
                .map(|(s, at)| Case {
                    rom: night_rom(s, at),
                    frame: 0,
                })
                .to_vec()
        },
        expect: |c| {
            let r = &c.rom;
            screen(|x, y| {
                let (u, v) = (x - r[144], y - r[145]);
                let s = if (0..8).contains(&u) && (0..8).contains(&v) {
                    r[(80 + v * 8 + u) as usize]
                } else {
                    0
                };
                if s != 0 {
                    s
                } else {
                    let t = r[((y >> 2) * 4 + (x >> 2)) as usize];
                    rd(r, 16 + t * 16 + (y & 3) * 4 + (x & 3))
                }
            })
        },
        goals: [Goal::new(Metric::Cycles, 216), Goal::new(Metric::Size, 27)],
        starter: Source::Code("; level, then hero on top | 先画关卡，再把主角盖上去\n"),
        reference: Source::Code(concat!(
            tile_bg!(),
            "; hero: sprite-local u, v and the outside test | 主角：精灵内的 u、v 和框外测试\n",
            "LD r1, [144]\nLD r2, [145]\nSUB r1, x, r1\nSUB r2, y, r2\nOR r3, r1, r2\nSHR r3, r3, 3\n",
            "SHL r2, r2, 3\nADD r1, r1, r2\nADD r1, r1, 80\nLD r1, [r1]\n",
            "; transparent outside the box, and wherever the sprite is 0 | 框外透明，精灵为 0 处也透明\n",
            "SETP.EQ p0, r3, 0\nSEL r1, p0, r1, 0\nSETP.NE p0, r1, 0\nSEL r0, p0, r1, r0\nPIX r0\n",
        )),
        live: Some(|frame, at| Case {
            rom: night_rom(71, at),
            frame,
        }),
        predict: Predict {
            question: Tr::new(
                "Each pixel decides alone. How many pixels need to know where the whole sprite is?",
                "每个像素独自决定。有多少个像素需要知道整个精灵在哪？",
            ),
            options: [
                Tr::new("Only the 64 under the sprite", "只有精灵下面的 64 个"),
                Tr::new(
                    "All 256 — each asks 'am I under it?'",
                    "全部 256 个——每个都要问“我在它下面吗？”",
                ),
                Tr::new("None — the sprite draws itself", "一个都不用——精灵自己会画"),
            ],
            answer: 1,
            why: Tr::new(
                "A pixel kernel cannot skip pixels; every thread reads the position and tests itself. That is the price of 'one rule for all'.",
                "像素内核无法跳过像素；每个线程都读位置、测试自己。这就是“一条规则管所有”的代价。",
            ),
        },
        hints: [
            Tr::new(
                "You have built both halves before: the Tile Map and the Sprite. What is new is choosing between them.",
                "两半你都做过：图块地图和精灵。新的只是在两者之间做选择。",
            ),
            Tr::new(
                "Fetch the sprite pixel like in The Sprite, force it to 0 outside the box, then SETP.NE on it to pick sprite or level.",
                "像“精灵”那关一样取出精灵像素，在盒子外强制为 0，再对它做 SETP.NE 来选择精灵还是关卡。",
            ),
            Tr::new(
                "u = x − rom[144], v = y − rom[145]; out = (u OR v) SHR 3; s = rom[80 + v·8 + u]; SEL s to 0 unless out = 0; SEL r0, p0, s, r0 when s ≠ 0.",
                "u = x − rom[144]，v = y − rom[145]；out = (u OR v) SHR 3；s = rom[80 + v·8 + u]；除非 out = 0，否则 SEL 把 s 置 0；当 s ≠ 0 时 SEL r0, p0, s, r0。",
            ),
        ],
        insight: Insight {
            title: Tr::new("Compositing", "合成"),
            line: Tr::new(
                "Layers, transparency and a per-pixel choice: the arcade's whole visual trick, in one kernel.",
                "图层、透明和逐像素的选择：街机全部的视觉戏法，都在一个内核里。",
            ),
            reality: Tr::new(
                "GPUs blend layers in dedicated hardware (ROPs); old consoles had sprite chips that did exactly your SEL.",
                "GPU 用专门的硬件（ROP）混合图层；老主机的精灵芯片做的正是你的 SEL。",
            ),
            myth: Tr::new(
                "Myth: transparency is a colour. It is a rule — 'colour 0 means show what is underneath'.",
                "误解：透明是一种颜色。它其实是一条规则——“颜色 0 表示显示下面的东西”。",
            ),
            simplified: Tr::new(
                "Real compositing blends with partial alpha; OC-1 has only fully on or fully see-through.",
                "真实的合成会按部分透明度混合；OC-1 只有完全不透明和完全透明。",
            ),
        },
        ..base("arcade-night", 6)
    }
}
