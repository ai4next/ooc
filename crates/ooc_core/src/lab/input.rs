//! What the free bench feeds a kernel: the level's own cases, a built-in
//! set, or imported data. Everything lands in `Case.rom`, row by row
//! (`rom[y * w + x]`), and `frame` is the case's index.

use oo_base::{Rng, Tr};
use serde::{Deserialize, Serialize};

use crate::task::{Case, Task};

/// Most values an imported file may hold (64 × 64).
pub const MAX_VALUES: usize = 4096;

/// A built-in input set. Same values on every machine and every run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Set {
    Pictures,
    Water,
    Xray,
    Height,
    Digits,
}

impl Set {
    pub const ALL: [Set; 5] = [Set::Pictures, Set::Water, Set::Xray, Set::Height, Set::Digits];

    pub fn id(self) -> &'static str {
        match self {
            Set::Pictures => "pictures",
            Set::Water => "water",
            Set::Xray => "xray",
            Set::Height => "height",
            Set::Digits => "digits",
        }
    }

    pub fn from_id(id: &str) -> Option<Set> {
        Set::ALL.into_iter().find(|s| s.id() == id)
    }

    pub fn name(self) -> Tr {
        match self {
            Set::Pictures => Tr::new("Pictures", "图片"),
            Set::Water => Tr::new("Water levels", "水位记录"),
            Set::Xray => Tr::new("X-ray", "X 光片"),
            Set::Height => Tr::new("Heightmap", "高度图"),
            Set::Digits => Tr::new("Handwritten digits", "手写数字"),
        }
    }

    /// What the ROM holds, in one line.
    pub fn about(self) -> Tr {
        match self {
            Set::Pictures => Tr::new(
                "4 pictures, one palette colour (0–15) per pixel",
                "4 张图，每个像素一个调色板颜色（0–15）",
            ),
            Set::Water => Tr::new(
                "3 years of well readings, 0–999, one per cell",
                "3 段深井水位读数，0–999，每格一个",
            ),
            Set::Xray => Tr::new(
                "2 films, grey 0–255 per pixel",
                "2 张片子，每个像素灰度 0–255",
            ),
            Set::Height => Tr::new(
                "3 terrains, height 0–255 per cell",
                "3 块地形，每格高度 0–255",
            ),
            Set::Digits => Tr::new(
                "digits 0–9, 1 = ink, 0 = paper; frame = the digit",
                "数字 0–9，1 = 笔迹，0 = 纸；frame = 这个数字",
            ),
        }
    }

    /// Largest value in the set (for previews).
    pub fn max(self) -> i32 {
        match self {
            Set::Pictures => 15,
            Set::Water => 999,
            Set::Xray | Set::Height => 255,
            Set::Digits => 1,
        }
    }

    /// The cases for a `side × side` grid.
    pub fn cases(self, side: usize) -> Vec<Case> {
        let roms: Vec<Vec<i32>> = match self {
            Set::Pictures => (0..4).map(|i| picture(i, side)).collect(),
            Set::Water => (0..3).map(|i| water(i, side * side)).collect(),
            Set::Xray => (0..2).map(|i| xray(i, side)).collect(),
            Set::Height => (0..3).map(|i| height(i, side)).collect(),
            Set::Digits => (0..10).map(|d| digit(d, side)).collect(),
        };
        roms.into_iter()
            .enumerate()
            .map(|(i, rom)| Case {
                rom,
                frame: i as i32,
            })
            .collect()
    }
}

/// Imported values: a picture or a table, `w` values per row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Data {
    pub name: String,
    pub w: usize,
    pub values: Vec<i32>,
}

impl Data {
    pub fn h(&self) -> usize {
        self.values.len().div_ceil(self.w.max(1))
    }

    pub fn max(&self) -> i32 {
        self.values.iter().copied().max().unwrap_or(0)
    }
}

/// The input a lab frame runs on.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Input {
    /// The level's own public cases.
    #[default]
    Level,
    Set(Set),
    Data(Data),
}

impl Input {
    /// The cases to run. `side` is the screen width on screen machines.
    pub fn cases(&self, task: &Task, side: usize) -> Vec<Case> {
        match self {
            Input::Level => (task.cases)(),
            Input::Set(s) => s.cases(side),
            Input::Data(d) => vec![Case {
                rom: d.values.clone(),
                frame: 0,
            }],
        }
    }
}

/// Grid side for built-in sets: the screen width, else 32.
pub fn side(task: &Task) -> usize {
    if matches!(task.view, crate::task::View::Screen) {
        task.profile.fb_w.max(1) as usize
    } else {
        32
    }
}

fn grid(side: usize, f: impl Fn(f64, f64) -> i32) -> Vec<i32> {
    let s = side as f64;
    (0..side * side)
        .map(|i| f(((i % side) as f64 + 0.5) / s, ((i / side) as f64 + 0.5) / s))
        .collect()
}

fn disc(u: f64, v: f64, cx: f64, cy: f64, rx: f64, ry: f64) -> bool {
    let (dx, dy) = ((u - cx) / rx, (v - cy) / ry);
    dx * dx + dy * dy <= 1.0
}

/// Distance from `p` to the segment `a`–`b`.
fn seg(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = dx * dx + dy * dy;
    let t = if len == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len).clamp(0.0, 1.0)
    };
    let (x, y) = (a.0 + t * dx - p.0, a.1 + t * dy - p.1);
    (x * x + y * y).sqrt()
}

fn near(p: (f64, f64), line: &[(f64, f64)], r: f64) -> bool {
    line.windows(2).any(|w| seg(p, w[0], w[1]) <= r)
}

/// Palette pictures: a cat, a sunset, colour bars, a tree on a hill.
fn picture(i: usize, side: usize) -> Vec<i32> {
    grid(side, |u, v| match i {
        0 => {
            let ear = |cx: f64, dir: f64| {
                v < 0.5 && v > 0.12 && (u - cx) * dir >= 0.0 && (u - cx).abs() < (v - 0.12) * 0.7
            };
            if disc(u, v, 0.36, 0.5, 0.06, 0.07) || disc(u, v, 0.64, 0.5, 0.06, 0.07) {
                if disc(u, v, 0.36, 0.5, 0.02, 0.06) || disc(u, v, 0.64, 0.5, 0.02, 0.06) {
                    0
                } else {
                    11
                }
            } else if disc(u, v, 0.5, 0.64, 0.05, 0.035) {
                14
            } else if disc(u, v, 0.5, 0.58, 0.32, 0.28) || ear(0.3, 1.0) || ear(0.7, -1.0) {
                if disc(u, v, 0.5, 0.74, 0.16, 0.1) { 15 } else { 9 }
            } else {
                1
            }
        }
        1 => {
            if v < 0.62 {
                if disc(u, v, 0.5, 0.6, 0.2, 0.2) {
                    10
                } else {
                    [1, 2, 8, 9][((v / 0.62) * 4.0) as usize % 4]
                }
            } else if (u - 0.5).abs() < 0.12 - (v - 0.62) * 0.2 && ((v * 40.0) as i32) % 2 == 0 {
                9
            } else {
                1
            }
        }
        2 => {
            if v < 0.66 {
                ((u * 16.0) as i32).min(15)
            } else {
                [0, 5, 6, 7][((u * 4.0) as usize).min(3)]
            }
        }
        _ => {
            let hill = 0.72 + 0.08 * ((u * 5.0).sin());
            if disc(u, v, 0.8, 0.18, 0.09, 0.09) {
                10
            } else if (u - 0.38).abs() < 0.04 && v > 0.42 && v < hill + 0.02 {
                4
            } else if disc(u, v, 0.38, 0.36, 0.2, 0.17) {
                if disc(u, v, 0.43, 0.4, 0.14, 0.12) { 3 } else { 11 }
            } else if v > hill {
                3
            } else {
                12
            }
        }
    })
}

/// Water readings over time: a calm tide, a storm surge, a dry season.
fn water(i: usize, len: usize) -> Vec<i32> {
    let mut rng = Rng::new(0x5A7E_0000 + i as u64);
    let tau = std::f64::consts::TAU;
    (0..len)
        .map(|t| {
            let x = t as f64 / len.max(1) as f64;
            let tide = (tau * x * 8.0).sin();
            let v = match i {
                0 => 500.0 + 180.0 * tide,
                1 => {
                    let surge = (-((x - 0.55) * 9.0).powi(2)).exp();
                    420.0 + 120.0 * tide + 450.0 * surge
                }
                _ => 820.0 - 600.0 * x + 60.0 * tide,
            };
            (v + rng.normal() * 18.0).round().clamp(0.0, 999.0) as i32
        })
        .collect()
}

/// X-ray films: a chest and a hand. Bright is dense.
fn xray(i: usize, side: usize) -> Vec<i32> {
    let mut rng = Rng::new(0x0B0E_0000 + i as u64);
    let mut img = grid(side, |u, v| {
        let p = (u, v);
        if i == 0 {
            let spine = (u - 0.5).abs() < 0.035 && v > 0.08;
            let lung = disc(u, v, 0.33, 0.48, 0.14, 0.3) || disc(u, v, 0.67, 0.48, 0.14, 0.3);
            let rib = lung && (0..6).any(|k| {
                let y = 0.26 + k as f64 * 0.075 + 0.08 * (u - 0.5).abs();
                (v - y).abs() < 0.012
            });
            if spine {
                225
            } else if rib {
                190
            } else if disc(u, v, 0.56, 0.62, 0.11, 0.12) {
                130
            } else if lung {
                45
            } else if disc(u, v, 0.5, 0.52, 0.44, 0.48) {
                100
            } else {
                18
            }
        } else {
            let fingers: [[(f64, f64); 3]; 5] = [
                [(0.3, 0.62), (0.2, 0.5), (0.14, 0.38)],
                [(0.4, 0.5), (0.36, 0.3), (0.34, 0.12)],
                [(0.5, 0.5), (0.5, 0.28), (0.5, 0.08)],
                [(0.6, 0.5), (0.63, 0.3), (0.65, 0.14)],
                [(0.68, 0.54), (0.75, 0.38), (0.79, 0.24)],
            ];
            let bone = fingers.iter().any(|f| near(p, f, 0.022))
                || fingers.iter().any(|f| seg(p, (0.5, 0.86), f[0]) < 0.024);
            let flesh = fingers.iter().any(|f| near(p, f, 0.05))
                || disc(u, v, 0.5, 0.7, 0.2, 0.2)
                || (v > 0.8 && (u - 0.5).abs() < 0.13);
            if bone {
                215
            } else if flesh {
                85
            } else {
                16
            }
        }
    });
    for v in &mut img {
        *v = (*v as f64 + rng.normal() * 6.0).round().clamp(0.0, 255.0) as i32;
    }
    img
}

/// Rolling terrain from three octaves of value noise.
fn height(i: usize, side: usize) -> Vec<i32> {
    let mut rng = Rng::new(0x4E16_0000 + i as u64);
    let lattice: Vec<f64> = (0..17 * 17).map(|_| rng.f64()).collect();
    let at = |x: f64, y: f64| {
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
        let g = |a: f64, b: f64| lattice[(b as usize % 17) * 17 + a as usize % 17];
        let top = g(x0, y0) + (g(x0 + 1.0, y0) - g(x0, y0)) * sx;
        let bottom = g(x0, y0 + 1.0) + (g(x0 + 1.0, y0 + 1.0) - g(x0, y0 + 1.0)) * sx;
        top + (bottom - top) * sy
    };
    grid(side, |u, v| {
        let h = 0.57 * at(u * 3.0, v * 3.0) + 0.29 * at(u * 6.0, v * 6.0) + 0.14 * at(u * 12.0, v * 12.0);
        (h * 255.0).round().clamp(0.0, 255.0) as i32
    })
}

/// Pen strokes for each digit, in a unit box.
const STROKES: [&[&[(f64, f64)]]; 10] = [
    &[&[(0.5, 0.1), (0.25, 0.25), (0.22, 0.75), (0.5, 0.9), (0.78, 0.75), (0.75, 0.25), (0.5, 0.1)]],
    &[&[(0.35, 0.25), (0.55, 0.1), (0.55, 0.9)], &[(0.35, 0.9), (0.75, 0.9)]],
    &[&[(0.25, 0.25), (0.5, 0.1), (0.75, 0.25), (0.72, 0.45), (0.25, 0.9), (0.78, 0.9)]],
    &[&[(0.25, 0.15), (0.72, 0.15), (0.45, 0.45), (0.75, 0.62), (0.6, 0.88), (0.25, 0.85)]],
    &[&[(0.65, 0.9), (0.65, 0.1), (0.22, 0.65), (0.8, 0.65)]],
    &[&[(0.75, 0.12), (0.3, 0.12), (0.27, 0.45), (0.7, 0.5), (0.72, 0.8), (0.45, 0.9), (0.25, 0.8)]],
    &[&[(0.7, 0.12), (0.35, 0.35), (0.25, 0.7), (0.45, 0.9), (0.72, 0.75), (0.65, 0.52), (0.28, 0.6)]],
    &[&[(0.22, 0.12), (0.78, 0.12), (0.42, 0.9)], &[(0.38, 0.5), (0.68, 0.5)]],
    &[&[(0.5, 0.48), (0.28, 0.3), (0.5, 0.1), (0.72, 0.3), (0.5, 0.48), (0.25, 0.7), (0.5, 0.9), (0.75, 0.7), (0.5, 0.48)]],
    &[&[(0.72, 0.4), (0.5, 0.52), (0.28, 0.35), (0.5, 0.1), (0.72, 0.3), (0.65, 0.9)]],
];

/// A digit as ink (1) on paper (0), drawn with a slightly shaky hand.
fn digit(d: usize, side: usize) -> Vec<i32> {
    let mut rng = Rng::new(0xD161_0000 + d as u64);
    let (dx, dy) = (rng.range(-0.05, 0.05), rng.range(-0.04, 0.04));
    let lean = rng.range(-0.12, 0.12);
    let strokes: Vec<Vec<(f64, f64)>> = STROKES[d]
        .iter()
        .map(|s| {
            s.iter()
                .map(|&(x, y)| {
                    let (jx, jy) = (rng.range(-0.03, 0.03), rng.range(-0.03, 0.03));
                    (x + dx + jx + lean * (0.5 - y), y + dy + jy)
                })
                .collect()
        })
        .collect();
    let pen = (1.1 / side as f64).max(0.06);
    grid(side, |u, v| strokes.iter().any(|s| near((u, v), s, pen)) as i32)
}

/// Integers separated by commas, semicolons, tabs, spaces or newlines.
/// Row width is the number of values on the first non-empty line.
pub fn parse_csv(name: &str, text: &str) -> Result<Data, Tr> {
    let mut values = Vec::new();
    let mut w = 0;
    for line in text.lines() {
        let mut row = 0;
        for cell in line.split([',', ';', '\t', ' ']).map(str::trim).filter(|c| !c.is_empty()) {
            let v = cell
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or(Tr::new("Not a number in the file", "文件里有不是数字的内容"))?;
            values.push(v.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32);
            row += 1;
        }
        if w == 0 {
            w = row;
        }
    }
    if values.is_empty() {
        return Err(Tr::new("The file has no numbers", "文件里没有数字"));
    }
    if values.len() > MAX_VALUES {
        return Err(Tr::new("More than 4096 numbers", "超过 4096 个数"));
    }
    Ok(Data {
        name: name.into(),
        w,
        values,
    })
}

/// An RGBA picture for a screen machine: resampled to `side × side` and
/// each pixel replaced by the nearest of `palette`.
pub fn quantise(name: &str, w: usize, h: usize, rgba: &[u8], side: usize, palette: &[[u8; 3]]) -> Data {
    let values = (0..side * side)
        .map(|i| {
            let px = average(w, h, rgba, side, side, i % side, i / side);
            let dist = |c: &[u8; 3]| {
                (0..3)
                    .map(|k| (px[k] - c[k] as f64).powi(2) * [0.3, 0.59, 0.11][k])
                    .sum::<f64>()
            };
            (0..palette.len())
                .min_by(|&a, &b| dist(&palette[a]).total_cmp(&dist(&palette[b])))
                .unwrap_or(0) as i32
        })
        .collect();
    Data {
        name: name.into(),
        w: side,
        values,
    }
}

/// An RGBA picture as luminance 0–255, shrunk to fit 64 × 64.
pub fn luminance(name: &str, w: usize, h: usize, rgba: &[u8]) -> Data {
    let scale = (w.max(h) as f64 / 64.0).max(1.0);
    let (ow, oh) = (
        ((w as f64 / scale).round() as usize).max(1),
        ((h as f64 / scale).round() as usize).max(1),
    );
    let values = (0..ow * oh)
        .map(|i| {
            let p = average(w, h, rgba, ow, oh, i % ow, i / ow);
            (0.299 * p[0] + 0.587 * p[1] + 0.114 * p[2]).round() as i32
        })
        .collect();
    Data {
        name: name.into(),
        w: ow,
        values,
    }
}

/// Mean colour of the source pixels under output cell (`x`, `y`);
/// transparent pixels count as black.
fn average(w: usize, h: usize, rgba: &[u8], ow: usize, oh: usize, x: usize, y: usize) -> [f64; 3] {
    let x0 = x * w / ow;
    let x1 = ((x + 1) * w / ow).max(x0 + 1).min(w);
    let y0 = y * h / oh;
    let y1 = ((y + 1) * h / oh).max(y0 + 1).min(h);
    let mut sum = [0.0; 3];
    let mut n = 0.0;
    for sy in y0..y1 {
        for sx in x0..x1 {
            let Some(p) = rgba.get((sy * w + sx) * 4..(sy * w + sx) * 4 + 4) else {
                continue;
            };
            let a = p[3] as f64 / 255.0;
            for k in 0..3 {
                sum[k] += p[k] as f64 * a;
            }
            n += 1.0;
        }
    }
    sum.map(|s| if n > 0.0 { s / n } else { 0.0 })
}
